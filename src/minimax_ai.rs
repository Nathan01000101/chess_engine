use std::any::Any;
use rustc_hash::FxHashMap;
use crate::BLACK_SHORT;
use crate::BLACK_LONG;
use crate::Board;
use crate::PieceType;
use crate::Piece;
use crate::Side;
use crate::WHITE_SHORT;
use crate::WHITE_TO_MOVE;
use crate::WHITE_LONG;
use crate::ai::Player;
use crate::undo_move;
use crate::can_castle_kingside;
use crate::can_castle_queenside;
use macroquad::{ prelude::*};
use macroquad::miniquad::date;
use crate::get_all_moves;
use crate::get_all_captures;
use crate::make_move;
use crate::is_in_check;
use crate::DEPTH;

pub struct MinimaxAI { pub depth: usize}
impl Player for MinimaxAI {
    fn as_any(&self) -> &dyn Any { self }
    fn get_move(&self, board: &Board, side: Side) -> ((usize, usize), (usize, usize)) {
        let mut b = board.clone();
        let mut best_moves: Vec<((usize, usize), (usize, usize))> = Vec::new();

        //transposition table
        let mut transposition_table: FxHashMap<u64, TTEntry> = FxHashMap::default();
        let zobrist_table: ZobristTable = ZobristTable::new();

        let mut best_eval: i32 = if side == Side::White {i32::MIN} else {i32::MAX};

        rand::srand(date::now() as u64);
        let mut moves = get_all_moves(&mut b, side);
        // move ordering
        moves.sort_by_key(|mv| { match board[mv.1.0][mv.1.1]{ Some(p) => -piece_value(p, mv.1, board.moves), None => 0}});
        for mv in moves{
            let undo = make_move(&mut b, mv.0,mv.1);
            let eval = minimax(&mut b, self.depth.saturating_sub(1), 1,  i32::MIN, i32::MAX, &zobrist_table, &mut transposition_table);
            undo_move(&mut b, undo);
            if side == Side::White && eval > best_eval || side == Side::Black && eval < best_eval{
                best_eval = eval;
                best_moves.clear();
                best_moves.push(mv);
            }else if eval == best_eval{
                best_moves.push(mv);
            }
        }
        println!("minimax's current eval: {}", best_eval);
        best_moves[rand::gen_range(0, best_moves.len())]
    }
}

fn minimax(board: &mut Board, depth: usize, ply: i32, mut alpha: i32, mut beta: i32,
        ztable: &ZobristTable,
        tt: &mut FxHashMap<u64, TTEntry>) -> i32 {
    let side = if board.state & WHITE_TO_MOVE != 0 { Side::White } else { Side::Black };
    let hash = ztable.hash(board);
    let alpha_orig = alpha;
    let beta_orig = beta;

    // Only trust an entry searched at least as deep as we need.
    if let Some(entry) = tt.get(&hash) {
        if entry.depth >= depth {
            match entry.bound {
                Bound::Exact => return entry.value,
                Bound::Lower => alpha = alpha.max(entry.value),
                Bound::Upper => beta  = beta.min(entry.value),
            }
            if alpha >= beta { return entry.value; }
        }
    } 

    if depth == 0 {
        return quiescence(board, ply, alpha, beta, ztable, tt);
    }

    let mut moves = get_all_moves(board, side);
    moves.sort_by_key(|mv| match board[mv.1.0][mv.1.1] {
    Some(p) => -piece_value(p, mv.1, board.moves),
    None => 0,
    });

    // mate check
    if moves.is_empty() {
        if is_in_check(board, side) {
            // side to move is mated; score from White's perspective
            return if board.state & WHITE_TO_MOVE != 0 { -500000 + ply  }
                else                 {  500000 - ply  };
        } else {
            return 0; // stalemate
        }
    }

    let value = if board.state & WHITE_TO_MOVE != 0 {
        let mut eval = i32::MIN;
        for mv in &moves {
            let undo = make_move(board, mv.0, mv.1);
            eval = minimax(board, depth -1, ply + 1, alpha, beta, ztable, tt).max(eval);
            undo_move(board, undo);

            alpha = alpha.max(eval);
            if alpha >= beta { break; }
        }
        eval
    } else {
        let mut eval = i32::MAX;
        for mv in moves {
            let undo = make_move(board, mv.0, mv.1);
            eval = minimax(board, depth - 1, ply + 1, alpha, beta, ztable, tt).min(eval);
            undo_move(board, undo);

            beta = beta.min(eval);
            if beta <= alpha { break; }
        }
        eval
    };

    let bound = if value <= alpha_orig { Bound::Upper }
            else if value >= beta_orig { Bound::Lower }
            else { Bound::Exact };
    tt.insert(hash, TTEntry { depth, value, bound });
    value
}

fn quiescence(
    board: &mut Board,
    ply: i32,
    mut alpha: i32,
    mut beta: i32,
    ztable: &ZobristTable,
    tt: &mut FxHashMap<u64, TTEntry>,
) -> i32 {
    let side = if board.state & WHITE_TO_MOVE != 0 { Side::White } else { Side::Black };
    let in_check = is_in_check(board, side);

    // Stand-pat — but only if we're not in check (can't "pass" out of check)
    let stand_pat = evaluate(board);
    if !in_check {
        if side == Side::White {
            if stand_pat >= beta { return beta; }
            if stand_pat > alpha { alpha = stand_pat; }
        } else {
            if stand_pat <= alpha { return alpha; }
            if stand_pat < beta { beta = stand_pat; }
        }
    }

    // Generate moves. If in check, search ALL moves. Otherwise only captures.
    let mut moves = if in_check {
        get_all_moves(board, side)
    } else {
        get_all_captures(board, side)  
    };

    // Mate / stalemate detection when in check with no legal moves
    if moves.is_empty() {
        if in_check {
            return if side == Side::White { -500000 + ply } else { 500000 - ply };
        }
        return stand_pat; // quiet position, no captures to consider
    }

    // MVV-style ordering: capture biggest victim first
    moves.sort_by_key(|mv| match board[mv.1.0][mv.1.1] {
        Some(p) => -piece_value(p, mv.1, board.moves),
        None => 0,
    });

    if side == Side::White {
        for mv in moves {
            let undo = make_move(board, mv.0, mv.1);
            let score = quiescence(board, ply + 1, alpha, beta, ztable, tt);
            undo_move(board, undo);
            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
        alpha
    } else {
        for mv in moves {
            let undo = make_move(board, mv.0, mv.1);
            let score = quiescence(board, ply + 1, alpha, beta, ztable, tt);
            undo_move(board, undo);
            if score <= alpha { return alpha; }
            if score < beta { beta = score; }
        }
        beta
    }
}

pub fn evaluate(board: &Board) -> i32{
    let mut score: i32 = 0;
    for i in 0..64 {
        let r = i / 8;
        let c = i % 8;
        if let Some(p) = board[r][c] {
            if p.color == Side::White {
                score += piece_value(p, (r, c), board.moves);
            } else {
                score -= piece_value(p, (r, c), board.moves);
            }
        }
    }

    score
}

fn piece_value(piece: Piece, coord: (usize, usize), moves: u8) -> i32{
    if piece.color == Side::White{
        match piece.piece_type {
            PieceType::Pawn   => return 100 + PAWN_TABLE[coord.0][coord.1],
            PieceType::Knight => return 305 + KNIGHT_TABLE[coord.0][coord.1],
            PieceType::Bishop => return 333 + BISHOP_TABLE[coord.0][coord.1],
            PieceType::Rook   => return 563 + ROOK_TABLE[coord.0][coord.1],
            PieceType::Queen  => if moves < 16 {return 950 + QUEEN_TABLE_EARLY[coord.0][coord.1]} else {return 950 + QUEEN_TABLE_LATE[coord.0][coord.1]},
            PieceType::King   => if moves < 40 {return 100000 + KING_TABLE_EARLY[coord.0][coord.1]} else {return 100000 + KING_TABLE_LATE[coord.0][coord.1] },
        };
    }else{
        match piece.piece_type {
            PieceType::Pawn   => return 100 + PAWN_TABLE[7 - coord.0][coord.1],
            PieceType::Knight => return 305 + KNIGHT_TABLE[7 - coord.0][coord.1],
            PieceType::Bishop => return 333 + BISHOP_TABLE[7 - coord.0][coord.1],
            PieceType::Rook   => return 563 + ROOK_TABLE[7 - coord.0][coord.1],
            PieceType::Queen  =>  if moves < 16 {return 950 + QUEEN_TABLE_EARLY[7 - coord.0][coord.1]} else {return 950 + QUEEN_TABLE_LATE[ 7 -coord.0][coord.1]},
            PieceType::King   => if moves < 40 {return 100000 + KING_TABLE_EARLY[7 - coord.0][coord.1]} else {return 100000 + KING_TABLE_LATE[7 - coord.0][coord.1] },
        };
    }

}

#[derive(Clone, Copy)]
enum Bound { Exact, Lower, Upper }

#[derive(Clone, Copy)]
struct TTEntry { depth: usize, value: i32, bound: Bound }

struct ZobristTable {
    pieces: [[[u64; 64]; 2]; 6],
    black_to_move: u64,
    en_passant_file: [u64; 8],
    castling: [u64; 4],  // WK, WQ, BK, BQ
}

impl ZobristTable {
    fn new() -> Self {
        let mut pieces = [[[0u64; 64]; 2]; 6];
        for pt in 0..6 {
            for color in 0..2 {
                for sq in 0..64 {
                    pieces[pt][color][sq] = rand::gen_range(0, u64::MAX);
                }
            }
        }

        let mut en_passant_file = [0u64; 8];
        for f in 0..8 {
            en_passant_file[f] = rand::gen_range(0, u64::MAX);
        }

        let mut castling = [0u64; 4];
        for i in 0..4 {
            castling[i] = rand::gen_range(0, u64::MAX);
        }

        ZobristTable {
            pieces,
            black_to_move: rand::gen_range(0, u64::MAX),
            en_passant_file,
            castling,
        }
    }

    fn hash(&self, board: &Board) -> u64 {
    let mut h: u64 = 0;
    for sq in 0..64 {
        if let Some(p) = board[sq / 8][sq % 8] {
            h ^= self.pieces[p.piece_type as usize][p.color as usize][sq];
        }
    }
    if !board.state & WHITE_TO_MOVE == 0 { h ^= self.black_to_move; }

    if let Some(target) = board.en_passant_target {
        h ^= self.en_passant_file[target.1 as usize];
    }

    if board.state & WHITE_SHORT != 0  { h ^= self.castling[0]; }
    if board.state & WHITE_LONG != 0 { h ^= self.castling[1]; }
    if board.state & BLACK_SHORT != 0 { h ^= self.castling[2]; }
    if board.state & BLACK_LONG != 0 { h ^= self.castling[3]; }

    h
}

}

// ALL OF THESE ARE FROM WHITES PERSPECTIVE, USE 7 - ROW WHEN INDEXING FOR BLACK

const PAWN_TABLE: [[i32; 8]; 8] = [
    [ 0,    0,    0,    0,    0,    0,    0,    0   ],  // promotion 
    [ 90,   90,   90,   90,   90,   90,   90,   90  ],
    [ 25,   25,   50,   55,   55,   50,   25,   25  ],
    [ 10,   10,   25,   50,   50,   25,   10,   10  ],
    [ 5,    5,    25,   45,   45,   25,   5,    5   ],
    [ 5,    5,    10,   5,    5,   10,    5,    5   ],
    [ 5,    5,    5,   -10,  -10,   5,    5,    5   ],  // slight penalty for blocking center
    [ 0,    0,    0,    0,    0,    0,    0,    0   ],  // starting rank
];

const KNIGHT_TABLE: [[i32; 8]; 8] = [
    [ -50,    -30,  -30,   -30,  -30,   -30,   -30,  -50  ],  // avoid edges
    [ -50,     0,    0,     5,    5,     0,     0,   -50  ],
    [ -50,     5,    10,    15,   15,    10,    5,   -50  ],
    [ -50,     5,    10,    25,   25,    10,    5,   -50  ],
    [ -50,     5,    10,    25,   25,    10,    5,   -50  ],
    [ -50,     0,    10,    15,   15,    10,    5,   -50  ],
    [ -50,    -5,   -5,     5,    5,    -5,    -5,   -50  ],  
    [ -50,    -10,  -30,   -30,  -30,   -30,   -10,  -50  ],  // starting rank
];

const BISHOP_TABLE: [[i32; 8]; 8] = [
    [ -20,   -10,   -10,   -10,   -10,   -10,   -10,  -20 ], // avoid edges
    [ -10,    0,     0,     0,     0,     0,     0,   -10 ],
    [ -10,    0,     5,     10,    10,    5,     0,   -10 ],
    [ -10,    5,     5,     10,    10,    5,     5,   -10 ],
    [ -10,    0,     10,    10,    10,    10,    0,   -10 ],
    [ -10,    10,    10,    10,    10,    10,    10,  -10 ],
    [ -10,    15,     0,     0,     0,     0,    15,  -10 ],
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



const QUEEN_TABLE_EARLY: [[i32; 8]; 8] = [
    [-30, -20, -20, -20, -20, -20, -20, -30], // Avoid early queen moves
    [-20, -15, -15, -15, -15, -15, -15, -20],
    [-20, -20, -20, -20, -20, -20, -20, -20],
    [-20, -20, -20, -25, -25, -20, -20, -20],
    [-20, -20, -20, -25, -25, -20, -20, -20],
    [-10, -15, -10, -15, -15, -10, -15, -10],
    [ -5,   5,   5,   5,   5,   5,   5,  -5],
    [-10,   0,   0,  15,   0,   0,   0, -10], // Keep king near king to start
];

const QUEEN_TABLE_LATE: [[i32; 8]; 8] = [
    [ -20, -10, -10,  -5,  -5, -10, -10, -20 ], // avoid edges
    [ -10,   0,   0,   0,   0,   0,   0, -10 ],
    [ -10,   0,   5,   5,   5,   5,   0, -10 ],
    [  -5,   0,   5,   5,   5,   5,   0,  -5 ],
    [  -5,   0,   5,   5,   5,   5,   0,  -5 ],
    [ -10,   5,   5,   5,   5,   5,   0, -10 ],
    [ -10,   0,   5,   0,   0,   0,   0, -10 ],
    [ -20, -10, -10,  -5,  -5, -10, -10, -20 ], // staring rank
];

const KING_TABLE_EARLY: [[i32; 8]; 8] = [
    [ -30, -40, -40, -50, -50, -40, -40, -30 ], // RUN AWAY!!
    [ -30, -40, -40, -50, -50, -40, -40, -30 ],
    [ -30, -40, -40, -50, -50, -40, -40, -30 ],
    [ -30, -40, -40, -50, -50, -40, -40, -30 ],
    [ -20, -30, -30, -40, -40, -30, -30, -20 ],
    [ -10, -20, -20, -20, -20, -20, -20, -10 ],
    [  20,  20,   0,   0,   0,   0,  20,  20 ],
    [  20,  30,  30,  10,  10,  10,  30,  20 ],  // castled positions rewarded
];

const KING_TABLE_LATE: [[i32; 8]; 8] = [
    [ -30, -30, -30, -30, -30, -30, -30, -30 ], // GET IN THE MIX!!
    [ -30, -10, -10, -10, -10, -10, -10, -30 ],
    [ -30, -10,   5,   5,   5,   5, -10, -30 ],
    [ -30, -10,   5,   5,   5,   5, -10, -30 ],
    [ -30, -10,   5,   5,   5,   5, -10, -30 ],
    [ -30, -10,   5,   5,   5,   5, -10, -30 ],
    [ -30, -10, -10, -10, -10, -10, -10, -30 ],
    [ -30, -30, -30, -30, -30, -30, -30, -30 ],  // middle positions rewarded
];