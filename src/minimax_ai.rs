use std::any::Any;
use std::collections::HashMap;
use crate::Board;
use crate::PieceType;
use crate::Piece;
use crate::Side;
use crate::ai::Player;
use macroquad::{ prelude::*};
use macroquad::miniquad::date;
use crate::get_all_moves;
use crate::move_piece_to;

pub struct MinimaxAI { pub depth: u8}
impl Player for MinimaxAI {
    fn as_any(&self) -> &dyn Any { self }
    fn get_move(&self, board: &Board, side: Side) -> ((usize, usize), (usize, usize)) {

        //transposition table
        let mut transposition_table: HashMap<u64, TTEntry> = HashMap::new();
        let zobrist_table: ZobristTable = ZobristTable::new();

        let mut best_move: ((usize, usize), (usize, usize)) = ((0,0), (0,0));
        let mut best_eval: i32 = if side == Side::White {i32::MIN} else {i32::MAX};

        rand::srand(date::now() as u64);
        let mut moves = get_all_moves(board, side);
        // move ordering
        moves.sort_by_key(|mv| { match board[mv.1.0][mv.1.1]{ Some(p) => -piece_value(p, mv.1), None => 0}});
        for mv in moves{
            let mut new_board = board.clone();
            move_piece_to(&mut new_board, mv.0,mv.1);
            let eval = minimax(&new_board, self.depth.saturating_sub(1), i32::MIN, i32::MAX, side == Side::Black, &zobrist_table, &mut transposition_table);

            if side == Side::White && eval > best_eval || side == Side::Black && eval < best_eval{
                best_eval = eval;
                best_move = mv;
            }else if eval == best_eval{

                if rand::gen_range(0, 2) == 1{
                    best_eval = eval;
                    best_move = mv;
                }
            }
        }
        best_move
    }
}

fn minimax(board: &Board, depth: u8, mut alpha: i32, mut beta: i32,
    maximizing_player: bool, ztable: &ZobristTable,
    tt: &mut HashMap<u64, TTEntry>) -> i32 {
let side = if maximizing_player { Side::White } else { Side::Black };
let hash = ztable.hash(board, side);
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
 let eval = evaluate(board);
 tt.insert(hash, TTEntry { depth: 0, value: eval, bound: Bound::Exact });
 return eval;
}

let mut moves = get_all_moves(board, side);
moves.sort_by_key(|mv| match board[mv.1.0][mv.1.1] {
 Some(p) => -piece_value(p, mv.1),
 None => 0,
});

let value = if maximizing_player {
 let mut eval = i32::MIN;
 for mv in moves {
     let mut nb = board.clone();
     move_piece_to(&mut nb, mv.0, mv.1);
     if get_all_moves(&nb, Side::Black).len() == 0 {
        return i32::MAX
     }
     eval = minimax(&nb, depth - 1, alpha, beta, false, ztable, tt).max(eval);
     alpha = alpha.max(eval);
     if alpha >= beta { break; }
 }
 eval
} else {
 let mut eval = i32::MAX;
 for mv in moves {
     let mut nb = board.clone();
     move_piece_to(&mut nb, mv.0, mv.1);
     if get_all_moves(&nb, Side::White).len() == 0 {
        return i32::MIN
     }
     eval = minimax(&nb, depth - 1, alpha, beta, true, ztable, tt).min(eval);
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

pub fn evaluate(board: &Board) -> i32{
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

fn piece_value(piece: Piece, coord: (usize, usize)) -> i32{
    if piece.color == Side::White{
        match piece.piece_type {
            PieceType::Pawn   => return 100 + PAWN_TABLE[coord.0][coord.1],
            PieceType::Knight => return 305 + KNIGHT_TABLE[coord.0][coord.1],
            PieceType::Bishop => return 333 + BISHOP_TABLE[coord.0][coord.1],
            PieceType::Rook   => return 563 + ROOK_TABLE[coord.0][coord.1],
            PieceType::Queen  => return 950 + QUEEN_TABLE[coord.0][coord.1],
            PieceType::King   => return 100000 + KING_TABLE[coord.0][coord.1],
        };
    }else{
        match piece.piece_type {
            PieceType::Pawn   => return 100 + PAWN_TABLE[7 - coord.0][coord.1],
            PieceType::Knight => return 305 + KNIGHT_TABLE[7 - coord.0][coord.1],
            PieceType::Bishop => return 333 + BISHOP_TABLE[7 - coord.0][coord.1],
            PieceType::Rook   => return 563 + ROOK_TABLE[7 - coord.0][coord.1],
            PieceType::Queen  => return 950 + QUEEN_TABLE[7 - coord.0][coord.1],
            PieceType::King   => return 100000 + KING_TABLE[7 - coord.0][coord.1],
        };
    }

}

#[derive(Clone, Copy)]
enum Bound { Exact, Lower, Upper }

#[derive(Clone, Copy)]
struct TTEntry { depth: u8, value: i32, bound: Bound }

struct ZobristTable {
    pieces: [[[u64; 64]; 2]; 6],
    black_to_move: u64,
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
        ZobristTable { pieces, black_to_move: rand::gen_range(0, u64::MAX) }
    }

    fn hash(&self, board: &Board, side: Side) -> u64 {
        let mut h: u64 = 0;
        for sq in 0..64 {
            if let Some(p) = board[sq / 8][sq % 8] {
                h ^= self.pieces[p.piece_type as usize][p.color as usize][sq];
            }
        }
        if side == Side::Black { h ^= self.black_to_move; }
        h
    }
}

// ALL OF THESE ARE FROM WHITES PERSPECTIVE, USE 7 - ROW WHEN INDEXING FOR BLACK

const PAWN_TABLE: [[i32; 8]; 8] = [
    [ 0,    0,    0,    0,    0,    0,    0,    0   ],  // promotion 
    [ 175,  175,  175,  175,  175,  175,  175,  175 ],
    [ 25,   25,   50,   75,   75,   50,   25,   25  ],
    [ 10,   10,   25,   60,   60,   25,   10,   10  ],
    [ 5,    5,    20,   50,   50,   20,   5,    5   ],
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