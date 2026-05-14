use std::any::Any;
use crate::Board;
use crate::PieceType;
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
        let mut best_eval: f32 = if side == Side::White {f32::MIN} else {f32::MAX};

        rand::srand(date::now() as u64);
        let mut moves = get_all_moves(board, side);
        moves.sort_by_key(|mv| if board[mv.1.0][mv.1.1].is_some() { 0 } else { 1 });
        for mv in moves{
            let mut new_board = board.clone();
            move_piece_to(&mut new_board, mv.0,mv.1);
            let eval = minimax(&new_board, self.depth.saturating_sub(1), f32::MIN, f32::MAX, side == Side::Black);

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

fn minimax(board: &Board, depth: u8, alpha: f32, beta: f32, maximizing_player: bool) -> f32{
    if depth == 0 {
        return evaluate(board);
    }

    if maximizing_player {
        let mut eval: f32 = f32::MIN;
        let mut alpha = alpha; 
        for mv in get_all_moves(board, Side::White) {
            let mut new_board = board.clone();
            move_piece_to(&mut new_board, mv.0, mv.1);
            eval = minimax(&new_board, depth.saturating_sub(1), alpha, beta, false).max(eval);
            alpha = alpha.max(eval); // update alpha
            if eval >= beta { break; }
        }
        eval
    } else {
        let mut eval: f32 = f32::MAX;
        let mut beta = beta; 
        for mv in get_all_moves(board, Side::Black) {
            let mut new_board = board.clone();
            move_piece_to(&mut new_board, mv.0, mv.1);
            eval = minimax(&new_board, depth.saturating_sub(1), alpha, beta, true).min(eval);
            beta = beta.min(eval); // update beta
            if eval <= alpha { break; }
        }
        eval
    }
}

fn evaluate(board: &Board) -> f32{
    let mut score: f32 = 0.0;
    for i in 0..64 {
        let r = i / 8;
        let c = i % 8;
        if let Some(p) = board[r][c] {
            let value = match p.piece_type {
                PieceType::Pawn   => 1.0,
                PieceType::Knight => 3.0,
                PieceType::Bishop => 3.0,
                PieceType::Rook   => 5.0,
                PieceType::Queen  => 9.0,
                PieceType::King   => 1000.0,
            };
            if p.color == Side::White {
                score += value;
            } else {
                score -= value;
            }
        }
    }

    score
}