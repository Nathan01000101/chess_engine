//! Multithreaded minimax chess AI.
//!
//! Search design
//! -------------
//! * Iterative deepening at the root (depth 1, 2, ... up to `depth`). Each pass
//!   warms the transposition table and improves move ordering for the next,
//!   deeper pass, so the deep searches prune far more aggressively.
//! * The root moves are searched in parallel with Rayon. The first
//!   (best-ordered) move is searched alone to establish a real alpha/beta
//!   bound, then the remaining moves are spread across all cores, sharing that
//!   bound through an atomic so they can still prune.
//! * A concurrent transposition table (`DashMap`) keyed by a Zobrist hash is
//!   shared by every thread and every deepening pass. Entries store a depth and
//!   a bound flag, so results coming out of alpha/beta-pruned searches are
//!   reused correctly.
//!
//! Bugs fixed from the original
//! ----------------------------
//! * `struct MinimaxMTAI` / `impl ... for MinimaxAI` name mismatch.
//! * Zobrist hash did not include side-to-move, so a White-to-move position and
//!   the otherwise-identical Black-to-move position collided in the table.
//! * The table stored every score as if it were exact, ignoring search depth.
//!   A shallow, alpha/beta-bounded score could be returned for a position that
//!   needed a deep, exact answer. Entries now carry `depth` + `Bound`.
//!
//! Cargo.toml needs:
//!   rayon   = "1"
//!   dashmap = "6"

use std::any::Any;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::OnceLock;

use dashmap::DashMap;
use rayon::prelude::*;

use macroquad::miniquad::date;
use macroquad::prelude::*;

use crate::ai::Player;
use crate::{get_all_moves, move_piece_to, Board, Piece, PieceType, Side};

/// (from, to) in (row, col) coordinates.
type Move = ((usize, usize), (usize, usize));

/// Concurrent transposition table, shared across threads and deepening passes.
type Tt = DashMap<u64, TtEntry>;

#[derive(Clone, Copy)]
enum Bound {
    Exact, // value is the true score
    Lower, // value is a lower bound (search failed high / caused a cutoff)
    Upper, // value is an upper bound (search failed low / never beat alpha)
}

#[derive(Clone, Copy)]
struct TtEntry {
    depth: u8,
    value: i32,
    bound: Bound,
    best_move: Option<Move>,
}

/// Built once on first use, then only ever read, so it is safe to share.
static ZOBRIST: OnceLock<ZobristTable> = OnceLock::new();

pub struct MinimaxMTAI {
    pub depth: u8,
}

impl Player for MinimaxMTAI {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_move(&self, board: &Board, side: Side) -> ((usize, usize), (usize, usize)) {
        let zobrist = ZOBRIST.get_or_init(ZobristTable::new);
        let tt: Tt = DashMap::new();

        let maximizing = side == Side::White;

        let mut moves = get_all_moves(board, side);
        if moves.is_empty() {
            return ((0, 0), (0, 0));
        }

        // Shuffle once so that, when several moves are equally good, the AI
        // does not always play the exact same game.
        rand::srand(date::now() as u64);
        shuffle(&mut moves);

        // In the old code `depth == 0` meant "make a move, then evaluate", i.e.
        // a 1-ply search. Keep that: the target search depth is at least 1.
        let target = self.depth.max(1);

        let mut best: (Move, i32) = (moves[0], if maximizing { i32::MIN } else { i32::MAX });

        for d in 1..=target {
            let results = search_root(board, &moves, d, maximizing, zobrist, &tt);

            // Only randomise tie-breaks on the final, deepest pass.
            best = pick_best(&results, maximizing, d == target);

            // Search the current best move first on the next, deeper pass.
            if let Some(pos) = moves.iter().position(|&m| m == best.0) {
                moves.swap(0, pos);
            }
        }

        best.0
    }
}

/// Search every legal root move and return `(move, score)` for each.
///
/// `moves[0]` is treated as the best guess (set up by iterative deepening) and
/// is searched first, sequentially, to get a real bound. The rest are searched
/// in parallel, sharing that bound through an atomic so alpha/beta still prunes.
fn search_root(
    board: &Board,
    moves: &[Move],
    depth: u8,
    maximizing: bool,
    zobrist: &ZobristTable,
    tt: &Tt,
) -> Vec<(Move, i32)> {
    let child_depth = depth.saturating_sub(1);

    // Eldest brother: search alone, full window, to seed the shared bound.
    let first = moves[0];
    let first_score = minimax(
        &apply(board, first),
        child_depth,
        i32::MIN + 1,
        i32::MAX - 1,
        !maximizing,
        zobrist,
        tt,
    );

    let mut results: Vec<(Move, i32)> = Vec::with_capacity(moves.len());
    results.push((first, first_score));

    if maximizing {
        // White to move: maximise. Share the best (highest) score as alpha.
        let shared_alpha = AtomicI32::new(first_score);
        let mut rest: Vec<(Move, i32)> = moves[1..]
            .par_iter()
            .map(|&mv| {
                let alpha = shared_alpha.load(Ordering::Relaxed);
                let score = minimax(
                    &apply(board, mv),
                    child_depth,
                    alpha,
                    i32::MAX - 1,
                    false,
                    zobrist,
                    tt,
                );
                if score > alpha {
                    shared_alpha.fetch_max(score, Ordering::Relaxed);
                }
                (mv, score)
            })
            .collect();
        results.append(&mut rest);
    } else {
        // Black to move: minimise. Share the best (lowest) score as beta.
        let shared_beta = AtomicI32::new(first_score);
        let mut rest: Vec<(Move, i32)> = moves[1..]
            .par_iter()
            .map(|&mv| {
                let beta = shared_beta.load(Ordering::Relaxed);
                let score = minimax(
                    &apply(board, mv),
                    child_depth,
                    i32::MIN + 1,
                    beta,
                    true,
                    zobrist,
                    tt,
                );
                if score < beta {
                    shared_beta.fetch_min(score, Ordering::Relaxed);
                }
                (mv, score)
            })
            .collect();
        results.append(&mut rest);
    }

    results
}

fn minimax(
    board: &Board,
    depth: u8,
    mut alpha: i32,
    mut beta: i32,
    maximizing: bool,
    zobrist: &ZobristTable,
    tt: &Tt,
) -> i32 {
    let alpha_orig = alpha;
    let beta_orig = beta;
    let hash = zobrist.hash(board, maximizing);

    // --- Transposition table probe -------------------------------------
    let mut tt_move: Option<Move> = None;
    if let Some(entry) = tt.get(&hash) {
        if entry.depth >= depth {
            match entry.bound {
                Bound::Exact => return entry.value,
                Bound::Lower => {
                    if entry.value >= beta {
                        return entry.value;
                    }
                }
                Bound::Upper => {
                    if entry.value <= alpha {
                        return entry.value;
                    }
                }
            }
        }
        // Even a too-shallow entry gives us a strong move to try first.
        tt_move = entry.best_move;
    } // the read-lock on the DashMap shard is released here

    if depth == 0 {
        let score = evaluate(board);
        tt.insert(
            hash,
            TtEntry { depth: 0, value: score, bound: Bound::Exact, best_move: None },
        );
        return score;
    }

    let side = if maximizing { Side::White } else { Side::Black };
    let mut moves = get_all_moves(board, side);

    if moves.is_empty() {
        // No legal moves: treat as a loss for the side to move. (A real engine
        // would test for check here to tell checkmate from stalemate.)
        let score = if maximizing { i32::MIN + 1 } else { i32::MAX - 1 };
        tt.insert(
            hash,
            TtEntry { depth, value: score, bound: Bound::Exact, best_move: None },
        );
        return score;
    }

    order_moves(&mut moves, board, tt_move);

    let mut best_score = if maximizing { i32::MIN } else { i32::MAX };
    let mut best_move = moves[0];

    for mv in &moves {
        let score = minimax(&apply(board, *mv), depth - 1, alpha, beta, !maximizing, zobrist, tt);

        if maximizing {
            if score > best_score {
                best_score = score;
                best_move = *mv;
            }
            alpha = alpha.max(best_score);
        } else {
            if score < best_score {
                best_score = score;
                best_move = *mv;
            }
            beta = beta.min(best_score);
        }

        if alpha >= beta {
            break; // cutoff
        }
    }

    // --- Store the result with the correct bound flag ------------------
    let bound = if best_score <= alpha_orig {
        Bound::Upper // never beat alpha -> only an upper bound on the truth
    } else if best_score >= beta_orig {
        Bound::Lower // caused a cutoff -> only a lower bound on the truth
    } else {
        Bound::Exact
    };

    let new_entry = TtEntry { depth, value: best_score, bound, best_move: Some(best_move) };
    // Keep the deeper of the two entries (deeper searches are more valuable).
    tt.entry(hash)
        .and_modify(|e| {
            if depth >= e.depth {
                *e = new_entry;
            }
        })
        .or_insert(new_entry);

    best_score
}

/// Pick the best `(move, score)` from the root results. When `randomize` is set,
/// choose uniformly among the moves that tie for the best score so the AI varies
/// its play. (Tie-breaking is heuristic: a tying move searched under a narrowed
/// window can be a hair worse than the true best, which is fine here.)
fn pick_best(results: &[(Move, i32)], maximizing: bool, randomize: bool) -> (Move, i32) {
    let best_score = if maximizing {
        results.iter().map(|&(_, s)| s).max().unwrap()
    } else {
        results.iter().map(|&(_, s)| s).min().unwrap()
    };

    if randomize {
        let tied: Vec<Move> = results
            .iter()
            .filter(|&&(_, s)| s == best_score)
            .map(|&(m, _)| m)
            .collect();
        (tied[rand::gen_range(0, tied.len())], best_score)
    } else {
        let mv = results.iter().find(|&&(_, s)| s == best_score).unwrap().0;
        (mv, best_score)
    }
}

/// Order moves to maximise alpha/beta cutoffs: the transposition-table move
/// first, then captures (most valuable victim first), then everything else.
fn order_moves(moves: &mut [Move], board: &Board, tt_move: Option<Move>) {
    moves.sort_by_cached_key(|&mv| {
        if Some(mv) == tt_move {
            return i32::MIN;
        }
        match board[mv.1.0][mv.1.1] {
            Some(victim) => -piece_value(victim, mv.1),
            None => 0,
        }
    });
}

/// Clone the board and apply a move to the copy.
///
/// NOTE: cloning the board for every node is the biggest remaining cost. The
/// real fix is a make/unmake pair on a single mutable board, but that needs the
/// internals of `move_piece_to` (castling rights, en passant, captured-piece
/// restoration) so it can be reversed correctly.
#[inline]
fn apply(board: &Board, mv: Move) -> Board {
    let mut next = board.clone();
    move_piece_to(&mut next, mv.0, mv.1);
    next
}

/// In-place Fisher-Yates shuffle using macroquad's RNG.
fn shuffle(moves: &mut [Move]) {
    for i in (1..moves.len()).rev() {
        moves.swap(i, rand::gen_range(0, i + 1));
    }
}

pub fn evaluate(board: &Board) -> i32 {
    let mut score: i32 = 0;
    for i in 0..64 {
        let r = i / 8;
        let c = i % 8;
        if let Some(p) = board[r][c] {
            if p.color == Side::White {
                score += piece_value(p, (r, c));
            } else {
                score -= piece_value(p, (r, c));
            }
        }
    }

    score
}

fn piece_value(piece: Piece, coord: (usize, usize)) -> i32 {
    if piece.color == Side::White {
        match piece.piece_type {
            PieceType::Pawn => return 100 + PAWN_TABLE[coord.0][coord.1],
            PieceType::Knight => return 300 + KNIGHT_TABLE[coord.0][coord.1],
            PieceType::Bishop => return 300 + BISHOP_TABLE[coord.0][coord.1],
            PieceType::Rook => return 500 + ROOK_TABLE[coord.0][coord.1],
            PieceType::Queen => return 900 + QUEEN_TABLE[coord.0][coord.1],
            PieceType::King => return 100000 + KING_TABLE[coord.0][coord.1],
        };
    } else {
        match piece.piece_type {
            PieceType::Pawn => return 100 + PAWN_TABLE[7 - coord.0][coord.1],
            PieceType::Knight => return 300 + KNIGHT_TABLE[7 - coord.0][coord.1],
            PieceType::Bishop => return 300 + BISHOP_TABLE[7 - coord.0][coord.1],
            PieceType::Rook => return 500 + ROOK_TABLE[7 - coord.0][coord.1],
            PieceType::Queen => return 900 + QUEEN_TABLE[7 - coord.0][coord.1],
            PieceType::King => return 100000 + KING_TABLE[7 - coord.0][coord.1],
        };
    }
}

struct ZobristTable {
    pieces: [[[u64; 64]; 2]; 6], // [piece_type][color][square]
    black_to_move: u64,
}

impl ZobristTable {
    fn new() -> Self {
        // splitmix64: deterministic, high-quality 64-bit values, and no RNG
        // state that would have to be shared between threads.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };

        let mut pieces = [[[0u64; 64]; 2]; 6];
        for pt in 0..6 {
            for color in 0..2 {
                for sq in 0..64 {
                    pieces[pt][color][sq] = next();
                }
            }
        }

        ZobristTable { pieces, black_to_move: next() }
    }

    /// Hash a position. `white_to_move` MUST be folded in or positions that
    /// differ only by whose turn it is will collide in the table.
    fn hash(&self, board: &Board, white_to_move: bool) -> u64 {
        let mut h: u64 = 0;
        for sq in 0..64 {
            if let Some(p) = board[sq / 8][sq % 8] {
                h ^= self.pieces[p.piece_type as usize][p.color as usize][sq];
            }
        }
        if !white_to_move {
            h ^= self.black_to_move;
        }
        h
    }
}

// ALL OF THESE ARE FROM WHITES PERSPECTIVE, USE 7 - ROW WHEN INDEXING FOR BLACK

const PAWN_TABLE: [[i32; 8]; 8] = [
    [ 0,    0,    0,    0,    0,    0,    0,    0   ],  // promotion
    [ 175,  175,  175,  175,  175,  175,  175,  175 ],
    [ 25,   25,   50,   75,   75,   50,   25,   25  ],
    [ 10,   10,   25,   60,   60,   25,   10,   10  ],
    [ 5,    5,    20,   25,   25,   20,   5,    5   ],
    [ 5,    5,    10,   5,    5,   10,    5,    5   ],
    [ 5,    5,    5,   -10,  -10,   5,    5,    5   ],  // slight penalty for blocking center
    [ 0,    0,    0,    0,    0,    0,    0,    0   ],  // starting rank
];

const KNIGHT_TABLE: [[i32; 8]; 8] = [
    [ -50,    -50,  -50,   -50,  -50,   -50,   -50,  -50  ],  // avoid edges
    [ -50,     0,    0,     5,    5,     0,     0,   -50  ],
    [ -50,     5,    10,    15,   15,    10,    5,   -50  ],
    [ -50,     5,    10,    25,   25,    10,    5,   -50  ],
    [ -50,     5,    10,    25,   25,    10,    5,   -50  ],
    [ -50,     0,    10,    15,   15,    10,    5,   -50  ],
    [ -50,    -5,   -5,     5,    5,    -5,    -5,   -50  ],
    [ -50,    -50,  -50,   -50,  -50,   -50,   -50,  -50  ],  // starting rank
];

const BISHOP_TABLE: [[i32; 8]; 8] = [
    [ -20,   -10,   -10,   -10,   -10,   -10,   -10,  -20 ], // avoid edges
    [ -10,    0,     0,     0,     0,     0,     0,   -10 ],
    [ -10,    0,     5,     10,    10,    5,     0,   -10 ],
    [ -10,    5,     5,     10,    10,    5,     5,   -10 ],
    [ -10,    0,     10,    10,    10,    10,    0,   -10 ],
    [ -10,    10,    10,    10,    10,    10,    10,  -10 ],
    [ -10,    5,     0,     0,     0,     0,     5,   -10 ],
    [ -20,   -10,   -10,   -10,   -10,   -10,   -10,  -20 ], //starting rank
];

const ROOK_TABLE: [[i32; 8]; 8] = [
    [  0,   0,   0,   0,   0,   0,   0,   0 ],  // 8th rank
    [ 10,  15,  15,  15,  15,  15,  15,  10 ],  // 7th rank bonus
    [ -5,   0,   0,   0,   0,   0,   0,  -5 ],
    [ -5,   0,   0,   0,   0,   0,   0,  -5 ],
    [ -5,   0,   0,   0,   0,   0,   0,  -5 ],
    [ -5,   0,   0,   0,   0,   0,   0,  -5 ],
    [ -5,   0,   0,   0,   0,   0,   0,  -5 ],
    [  0,   0,   5,  10,  10,   5,   0,   0 ],  // starting rank
];

const QUEEN_TABLE: [[i32; 8]; 8] = [
    [ -20, -10, -10,  -5,  -5, -10, -10, -20 ], // avoid edges
    [ -10,   0,   0,   0,   0,   0,   0, -10 ],
    [ -10,   0,   5,   5,   5,   5,   0, -10 ],
    [  -5,   0,   5,   5,   5,   5,   0,  -5 ],
    [   0,   0,   5,   5,   5,   5,   0,  -5 ],
    [ -10,   5,   5,   5,   5,   5,   0, -10 ],
    [ -10,   0,   5,   0,   0,   0,   0, -10 ],
    [ -20, -10, -10,  -5,  -5, -10, -10, -20 ], // staring rank
];

const KING_TABLE: [[i32; 8]; 8] = [
    [ -30, -40, -40, -50, -50, -40, -40, -30 ], // RUN AWAY!!
    [ -30, -40, -40, -50, -50, -40, -40, -30 ],
    [ -30, -40, -40, -50, -50, -40, -40, -30 ],
    [ -30, -40, -40, -50, -50, -40, -40, -30 ],
    [ -20, -30, -30, -40, -40, -30, -30, -20 ],
    [ -10, -20, -20, -20, -20, -20, -20, -10 ],
    [  20,  20,   0,   0,   0,   0,  20,  20 ],
    [  20,  30,  10,   0,   0,  10,  30,  20 ],  // castled positions rewarded
];