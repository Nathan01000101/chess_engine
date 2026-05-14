use std::any::Any;
use crate::Board;
use crate::Side;

pub trait Player {
    fn as_any(&self) -> &dyn Any;
    fn get_move(&self, board: &Board, side: Side) -> ((usize, usize), (usize, usize));
}