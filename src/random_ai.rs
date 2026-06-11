use std::any::Any;
use std::thread;
use std::time::Duration;
use macroquad::{ prelude::*};
use macroquad::miniquad::date;
use crate::{Board, get_valid_moves_standalone};
use crate::Side;
use crate::ai::Player;
use crate::get_valid_moves;

pub struct RandomAI;
impl Player for RandomAI {
    fn as_any(&self) -> &dyn Any { self }

    fn get_move(&self, board: &Board, side: Side) -> ((usize, usize), (usize, usize)) {
        let mut moves: Vec<((usize, usize), (usize, usize))> = Vec::new();

        let mut b = board.clone();
        thread::sleep(Duration::from_secs_f32(0.5));

        for i in 0..64 {
            let r = i / 8;
            let c = i % 8;
            if let Some(p) = board[r][c] {
                if p.color == side {
                    for mv in get_valid_moves_standalone(&mut b, (r, c)) {
                        moves.push(((r, c), mv));
                    }
                }
            }
        }

        rand::srand(date::now() as u64);
        moves[rand::gen_range(0, moves.len())]
    }
}