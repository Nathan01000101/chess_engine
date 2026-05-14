use std::any::Any;
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
        let mut best_move: ((usize, usize), (usize, usize)) = ((0,0), (0,0));
        let mut best_eval: i32 = if side == Side::White {i32::MIN} else {i32::MAX};

        rand::srand(date::now() as u64);
        let mut moves = get_all_moves(board, side);
        // move ordering
        moves.sort_by_key(|mv| { match board[mv.1.0][mv.1.1]{ Some(p) => -piece_value(p, mv.1), None => 0}});
        for mv in moves{
            let mut new_board = board.clone();
            move_piece_to(&mut new_board, mv.0,mv.1);
            let eval = minimax(&new_board, self.depth.saturating_sub(1), i32::MIN, i32::MAX, side == Side::Black);

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
        println!("{}", best_eval);
        best_move
    }
}

fn minimax(board: &Board, depth: u8, alpha: i32, beta: i32, maximizing_player: bool) -> i32{
    if depth == 0 {
        return evaluate(board);
    }

    if maximizing_player {
        let mut eval: i32 = i32::MIN;
        let mut alpha = alpha; 

        // move ordering
        let mut moves = get_all_moves(board, Side::White);
        moves.sort_by_key(|mv| { match board[mv.1.0][mv.1.1]{ Some(p) => -piece_value(p, mv.1), None => 0}});

        for mv in moves {
            let mut new_board = board.clone();
            move_piece_to(&mut new_board, mv.0, mv.1);
            eval = minimax(&new_board, depth.saturating_sub(1), alpha, beta, false).max(eval);
            alpha = alpha.max(eval); // update alpha
            if eval >= beta { break; }
        }
        eval
    } else {
        let mut eval: i32 = i32::MAX;
        let mut beta = beta; 

        // move ordering
        let mut moves = get_all_moves(board, Side::Black);
        moves.sort_by_key(|mv| { match board[mv.1.0][mv.1.1]{ Some(p) => -piece_value(p, mv.1), None => 0}});

        for mv in moves {
            let mut new_board = board.clone();
            move_piece_to(&mut new_board, mv.0, mv.1);
            eval = minimax(&new_board, depth.saturating_sub(1), alpha, beta, true).min(eval);
            beta = beta.min(eval); // update beta
            if eval <= alpha { break; }
        }
        eval
    }
}

fn evaluate(board: &Board) -> i32{
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
            PieceType::Knight => return 300 + KNIGHT_TABLE[coord.0][coord.1],
            PieceType::Bishop => return 300 + BISHOP_TABLE[coord.0][coord.1],
            PieceType::Rook   => return 500 + ROOK_TABLE[coord.0][coord.1],
            PieceType::Queen  => return 900 + QUEEN_TABLE[coord.0][coord.1],
            PieceType::King   => return 100000 + KING_TABLE[coord.0][coord.1],
        };
    }else{
        match piece.piece_type {
            PieceType::Pawn   => return 100 + PAWN_TABLE[7 - coord.0][coord.1],
            PieceType::Knight => return 300 + KNIGHT_TABLE[7 - coord.0][coord.1],
            PieceType::Bishop => return 300 + BISHOP_TABLE[7 - coord.0][coord.1],
            PieceType::Rook   => return 500 + ROOK_TABLE[7 - coord.0][coord.1],
            PieceType::Queen  => return 900 + QUEEN_TABLE[7 - coord.0][coord.1],
            PieceType::King   => return 100000 + KING_TABLE[7 - coord.0][coord.1],
        };
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