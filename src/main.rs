use macroquad::{ prelude::*};
use crate::ai::Player;
use crate::human::HumanPlayer;
use crate::random_ai::RandomAI;
use crate::minimax_ai::MinimaxAI;
use std::collections::HashSet;
use std::time::Duration;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::ops::{Index, IndexMut};
use std::thread;
use std::env;

mod ai;
mod human;
mod random_ai;
mod minimax_ai;
mod tests;

const WINDOW_SIZE: f32 = 600.0;
pub const DEPTH: usize = 6;
const GAMES: usize = 16;

const BISHOP_DIRS: [(i16, i16); 4] = [(-1, 1), (1, 1), (-1, -1), (1, -1)];
const ROOK_DIRS: [(i16, i16); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const QUEEN_DIRS: [(i16, i16); 8] = [
    (-1, 0), (1, 0), (0, -1), (0, 1),
    (-1, 1), (1, 1), (-1, -1), (1, -1),
];
const KNIGHT_OFFSETS: [(i16, i16); 8] = [
    (-2, -1), (-2, 1), (-1, -2), (-1, 2),
    (1, -2), (1, 2), (2, -1), (2, 1),
];
const KING_OFFSETS: [(i16, i16); 8] = [
    (-1, -1), (-1, 0), (-1, 1), (0, -1),
    (0, 1), (1, -1), (1, 0), (1, 1),
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum PieceType {
    Pawn, Knight, Bishop, Rook, Queen, King
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    White, Black
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Piece {
    piece_type: PieceType,
    color: Side,
    has_moved: bool,
}


#[derive(Clone, Copy, PartialEq, Debug)]
struct Board{
    squares: [[Option<Piece>; 8]; 8],
    moves: u8,
    white_king: (u8, u8),
    black_king: (u8, u8),
    en_passant_target: Option<(usize, usize)>,
    white_to_move: bool
}
impl Index<usize> for Board {
    type Output = [Option<Piece>; 8];
    fn index(&self, index: usize) -> &Self::Output {
        &self.squares[index]
    }
}
impl IndexMut<usize> for Board {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.squares[index]
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Move{
    from: (usize, usize),
    to: (usize, usize)
}

struct Undo{
    last_move: Move, // (from, to)
    moving_piece_before: Piece,
    captured_piece: Option<Piece>, // what piece occupied the square before moving
    previous_en_passant_target: Option<(usize, usize)> // stores where the en_passant target was before moving
}

// board logic
fn new_board() -> Board {
    let mut board = Board {squares: [[None; 8]; 8], white_king: (7, 4), black_king: (0, 4), moves: 0, en_passant_target: None, white_to_move: true };

    // Helper closure to place a piece
    let w = |pt| Some(Piece { piece_type: pt, color: Side::White, has_moved: false});
    let b = |pt| Some(Piece { piece_type: pt, color: Side::Black, has_moved: false});

    // Back ranks
    let back_row = [
        PieceType::Rook, PieceType::Knight, PieceType::Bishop, PieceType::Queen,
        PieceType::King, PieceType::Bishop, PieceType::Knight, PieceType::Rook,
    ];

    for col in 0..8 {
        board[0][col] = b(back_row[col]); // black back rank
        board[1][col] = b(PieceType::Pawn);
        board[6][col] = w(PieceType::Pawn);
        board[7][col] = w(back_row[col]); // white back rank
    }

    board
}

fn is_piece(board: &Board, coord: (usize, usize)) -> bool {
    if in_bounds(coord.0 as i16, coord.1 as i16){
        board[coord.0][coord.1].is_some()
    }else{
        false
    }
}

// checks if a piece is a given type *does not care about side*
fn is_piece_type(board: &Board, coord: (usize, usize), piece_type: PieceType) -> bool{
    if let Some(p) = board[coord.0][coord.1]{
        p.piece_type == piece_type
    }else{
        false
    }
}

fn in_bounds(x: i16, y: i16) -> bool{
    x < 8 && x >= 0 && y < 8 && y >= 0
}

// determines if a side is in check with a given board state
fn is_in_check(board: &Board, side: Side) -> bool {
    
    let enemy = if side == Side::White {Side::Black} else {Side::White};
    let king: (i16, i16) = if side == Side::White { (board.white_king.0 as i16, board.white_king.1 as i16)} else { (board.black_king.0 as i16, board.black_king.1 as i16) };

    // any opposing piece attacking the king?
    // pawns?
    let pawn_dir = if side == Side::White { -1 } else { 1 };
    for dc in [1, -1]{
        let pc = king.1 + dc;
        let pr = king.0 + pawn_dir;
        if in_bounds(pc, pr){
            if let Some(p) = board[pr as usize][pc as usize]{
                if p.color == enemy{
                    if p.piece_type == PieceType::Pawn{
                        return true;
                    }

                }
            }
        }
    }

    //knights?
    for offset in KNIGHT_OFFSETS{
        let kc = king.1 + offset.1;
        let kr = king.0 + offset.0;
        if in_bounds(kc, kr){
            if let Some(p) = board[kr as usize][kc as usize]{
                if p.color == enemy{
                    if p.piece_type == PieceType::Knight{
                        return true;
                    }    
                }
            }
        }
    }

    // bishops or queens?
    for dir in BISHOP_DIRS{
        for i in 1..8{
            if in_bounds(king.0 + dir.0 * i, king.1 + dir.1 * i){
                if let Some(p) = board[(king.0 + dir.0 * i) as usize][ (king.1 + dir.1 * i) as usize]{
                    if p.color == enemy{
                        if p.piece_type == PieceType::Bishop || p.piece_type == PieceType::Queen{
                            return true;
                        }
                    }
                    break;
                }
            }else{
                break;
            }
        }
    }

    // rooks or queens?
    for dir in ROOK_DIRS{
        for i in 1..8{
            if in_bounds(king.0 + dir.0 * i, king.1 + dir.1 * i){
                if let Some(p) = board[(king.0 + dir.0 * i) as usize][ (king.1 + dir.1 * i) as usize]{
                    if p.color == enemy{
                        if p.piece_type == PieceType::Rook || p.piece_type == PieceType::Queen{
                            return true;
                        }
                    }
                    break;
                }
            }else{
                break;
            }
        }
    }
    false
}

// determines if a given square is attacked by a given side
fn is_square_attacked_by(board: &Board, coords: (usize, usize), side: Side) -> bool{
    
    // pawns?
    let pawn_dir = if side == Side::White { -1 } else { 1 };
    for dc in [1, -1]{
        let pc = coords.1 as i16 + dc;
        let pr = coords.0 as i16+ pawn_dir;
        if in_bounds(pc, pr){
            if let Some(p) = board[pr as usize][pc as usize]{
                if p.color == side{
                    if p.piece_type == PieceType::Pawn{
                        return true;
                    }

                }
            }
        }
    }

    //knights?
    for offset in KNIGHT_OFFSETS{
        let kc = coords.1 as i16 + offset.1;
        let kr = coords.0 as i16 + offset.0;
        if in_bounds(kc, kr){
            if let Some(p) = board[kr as usize][kc as usize]{
                if p.color == side{
                    if p.piece_type == PieceType::Knight{
                        return true;
                    }    
                }
            }
        }
    }

    // bishops or queens?
    for dir in BISHOP_DIRS{
        for i in 1..8{
            if in_bounds(coords.0 as i16 + dir.0 * i, coords.1 as i16 + dir.1 * i){
                if let Some(p) = board[(coords.0 as i16 + dir.0 * i) as usize][ (coords.1 as i16 + dir.1 * i) as usize]{
                    if p.color == side{
                        if p.piece_type == PieceType::Bishop || p.piece_type == PieceType::Queen{
                            return true;
                        }
                    }
                    break;
                }
            }else{
                break;
            }
        }
    }

    // rooks or queens?
    for dir in ROOK_DIRS{
        for i in 1..8{
            if in_bounds(coords.0 as i16 + dir.0 * i, coords.1 as i16+ dir.1 * i){
                if let Some(p) = board[(coords.0 as i16 + dir.0 * i) as usize][ (coords.1 as i16 + dir.1 * i) as usize]{
                    if p.color == side{
                        if p.piece_type == PieceType::Rook || p.piece_type == PieceType::Queen{
                            return true;
                        }
                    }
                    break;
                }
            }else{
                break;
            }
        }
    }

    // king ? 
    for offset in KING_OFFSETS{
        let kc = coords.1 as i16 + offset.1;
        let kr = coords.0 as i16 + offset.0;
        if in_bounds(kc, kr){
            if let Some(p) = board[kr as usize][kc as usize]{
                if p.color == side{
                    if p.piece_type == PieceType::King{
                        return true;
                    }    
                }
            }
        }
    }

    false
}

fn make_move(board: &mut Board, old: (usize, usize), new: (usize, usize)) -> Undo{
    if let Some(mut p) = board[old.0][old.1] {
        let mut undo: Undo = Undo {last_move: Move {to: new, from: old},
                        moving_piece_before: p,
                        captured_piece: None, 
                        previous_en_passant_target: None};
        undo.previous_en_passant_target = board.en_passant_target;
        undo.captured_piece = board[new.0][new.1];
        p.has_moved = true;
        
        // remove en passant target
        board.en_passant_target = None;
        board.moves += 1;
        board.white_to_move = !board.white_to_move;
        if p.piece_type == PieceType::Pawn{
            
            let dy = new.0 as i16 - old.0 as i16;
            if  dy == 2 || dy == -2{
                board.en_passant_target = Some(((old.0 as i16 + dy / 2) as usize, old.1));
            }
            // check for en passant and for updating doubled moved
            if new.1 as i16 - old.1 as i16 != 0{
                if board[new.0][new.1].is_none(){
                    undo.captured_piece = board[old.0][new.1];
                    board[old.0][new.1] = None;
                } 
            }

            // check if promoted
            if new.0 == 0 || new.0 == 7{
                p.piece_type = PieceType::Queen;
            }
        }else if p.piece_type == PieceType::King {
            if p.color == Side::White{
                board.white_king = (new.0 as u8, new.1 as u8);
            }else{
                board.black_king = (new.0 as u8, new.1 as u8);
            }
            let dx = new.1 as i16 - old.1 as i16;
            if dx == 2 {
                if let Some(mut rook) = board[new.0][7] {
                    rook.has_moved = true;
                    board[new.0][5] = Some(rook);
                    board[new.0][7] = None;
                }
            } else if dx == -2 {
                if let Some(mut rook) = board[new.0][0] {
                    rook.has_moved = true;
                    board[new.0][3] = Some(rook);
                    board[new.0][0] = None;
                }
            }
        }
        board[new.0][new.1] = Some(p);
        board[old.0][old.1] = None;
        undo
    }else{
        panic!("CANNOT MOVE EMPTY SQUARE");
    }
    
}

fn undo_move(board: &mut Board, undo: Undo){
    board.white_to_move = !board.white_to_move;
    board.moves -= 1;
    board[undo.last_move.from.0][undo.last_move.from.1] = Some(undo.moving_piece_before);
    board[undo.last_move.to.0][undo.last_move.to.1] = undo.captured_piece;
    board.en_passant_target = undo.previous_en_passant_target;
    if undo.moving_piece_before.piece_type == PieceType::Pawn{
        if let Some(target) = undo.previous_en_passant_target{
            if undo.last_move.to == target && undo.last_move.to.1 != undo.last_move.from.1{
                board[undo.last_move.to.0][undo.last_move.to.1] = None;
                board[undo.last_move.from.0][undo.last_move.to.1] = undo.captured_piece;
            }
        }
    }else if undo.moving_piece_before.piece_type == PieceType::King {
        if undo.moving_piece_before.color == Side::White{
            board.white_king = (undo.last_move.from.0 as u8, undo.last_move.from.1 as u8);
        }else{
            board.black_king = (undo.last_move.from.0 as u8, undo.last_move.from.1 as u8);
        }
        //check for castling move
        let dx: i16 = undo.last_move.to.1 as i16 - undo.last_move.from.1 as i16;
        if dx.abs() == 2{
            if dx == 2{
                let mut restored_rook = board[undo.last_move.to.0][5].unwrap();
                restored_rook.has_moved = false;
                board[undo.last_move.to.0][7] = Some(restored_rook);
                board[undo.last_move.to.0][5] = None;

            }else{
                let mut restored_rook = board[undo.last_move.to.0][3].unwrap();
                restored_rook.has_moved = false;
                board[undo.last_move.to.0][0] = Some(restored_rook);
                board[undo.last_move.to.0][3] = None;
            }
        }
    }
}

// gets all moves for a piece DOES NOT INCLUDE CHECKING THAT KING IS LEFT VISIBLE
fn get_attacked_squares(board: &Board, coord: (usize, usize)) -> Vec<(usize, usize)> {
    let mut possible: Vec<(i16, i16)> = Vec::new();
    let mut valid: Vec<(usize, usize)> = Vec::new();
    let piece: Option<Piece> = board[coord.0][coord.1];

    if let Some(p) = piece {
        match p.piece_type{
            PieceType::Bishop => {
                // generate possible moves
                for dir in BISHOP_DIRS{
                    for i in 1..8{
                        if in_bounds(coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i){
                            if is_piece(board, ((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize)){
                                possible.push((coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i));
                                break;
                            }
                            possible.push((coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i));
                        }else{
                            break;
                        }
                    }
                }
            },
            PieceType::Knight => {

                for offset in KNIGHT_OFFSETS{
                    possible.push((coord.0 as i16 - offset.0, coord.1 as i16 - offset.1));
                }
            },
            PieceType::King => {
                for offset in KING_OFFSETS{
                    possible.push((coord.0 as i16 - offset.0, coord.1 as i16 - offset.1));
                }
            },
            PieceType::Pawn => {
                if p.color == Side::White {
                    // White moves toward row 0 (decreasing rows)

                    // capturing tiles
                    if coord.0 > 0 {
                        if coord.1 > 0 && is_piece(board, (coord.0 - 1, coord.1 - 1)) {
                            possible.push((coord.0 as i16 - 1, coord.1 as i16 - 1));
                        }
                        if coord.1 < 7 && is_piece(board, (coord.0 - 1, coord.1 + 1)) {
                            possible.push((coord.0 as i16 - 1, coord.1 as i16 + 1));
                        }
                    }

                    // en passant
                    if let Some(target) = board.en_passant_target {
                        if target.0 == coord.0.wrapping_sub(1)
                            && (target.1 as i16 - coord.1 as i16).abs() == 1
                        {
                            possible.push((target.0 as i16, target.1 as i16));
                        }
                    }

                    // moving forward
                    if coord.0 > 0 && board[coord.0 - 1][coord.1].is_none() {
                        possible.push((coord.0 as i16 - 1, coord.1 as i16));
                        if coord.0 > 1 && board[coord.0 - 2][coord.1].is_none() && !p.has_moved {
                            possible.push((coord.0 as i16 - 2, coord.1 as i16));
                        }
                    }
                } else {
                    // Black moves toward row 7 (increasing rows)

                    // capturing tiles
                    if coord.0 < 7 {
                        if coord.1 > 0 && is_piece(board, (coord.0 + 1, coord.1 - 1)) {
                            possible.push((coord.0 as i16 + 1, coord.1 as i16 - 1));
                        }
                        if coord.1 < 7 && is_piece(board, (coord.0 + 1, coord.1 + 1)) {
                            possible.push((coord.0 as i16 + 1, coord.1 as i16 + 1));
                        }
                    }

                    // en passant
                    if let Some(target) = board.en_passant_target {
                        if target.0 == coord.0 + 1
                            && (target.1 as i16 - coord.1 as i16).abs() == 1
                        {
                            possible.push((target.0 as i16, target.1 as i16));
                        }
                    }

                    // moving forward
                    if coord.0 < 7 && board[coord.0 + 1][coord.1].is_none() {
                        possible.push((coord.0 as i16 + 1, coord.1 as i16));
                        if coord.0 < 6 && board[coord.0 + 2][coord.1].is_none() && !p.has_moved {
                            possible.push((coord.0 as i16 + 2, coord.1 as i16));
                        }
                    }
                }
            },
            PieceType::Queen => {
                // generate possible moves
                for dir in QUEEN_DIRS{
                    for i in 1..8{
                        if in_bounds(coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i){
                            if is_piece(board, ((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize)){
                                possible.push((coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i));
                                break;
                            }
                            possible.push((coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i));
                        }else{
                            break;
                        }
                    }
                }
            }
            PieceType::Rook => {
                // generate possible moves
                for dir in ROOK_DIRS{
                    for i in 1..8{
                        if in_bounds(coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i){
                            possible.push((coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i));
                            if is_piece(board, ((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize)){
                                break;
                            }
                            
                        }else{
                            break;
                        }
                    }
                }
            }

        }

        // check validity of each move
        for m in possible {
            if in_bounds(m.0, m.1){

                // dont let us take same coloured pieces
                if let Some(sp) = board[m.0 as usize][m.1 as usize]{
                    if sp.color == p.color{
                        continue;
                    }
                }
                valid.push((m.0 as usize, m.1 as usize));
            }
        }
    }

    valid
}

// returns all valid moves for a piece 
fn get_valid_moves(board: &mut Board, coord: (usize, usize) ) -> Vec<(usize, usize)> {
    if board[coord.0][coord.1].is_none() {return Vec::new()}
    let piece = board[coord.0][coord.1].unwrap();
    let mut not_checked: Vec<(usize, usize)> = get_attacked_squares(&board, coord);
    let mut checked: Vec<(usize, usize)> = Vec::new();

    // add castling move if neccesary
    if piece.piece_type == PieceType::King{
        // castling
        if !piece.has_moved {
            let opposite_side = if piece.color == Side::White { Side::Black } else { Side::White };
            // queenside
            if let Some(sp) = board[coord.0][0] {
                if sp.piece_type == PieceType::Rook && !sp.has_moved {
                    if board[coord.0][1].is_none() && board[coord.0][2].is_none() && board[coord.0][3].is_none() {
                        if !is_square_attacked_by(&board, (coord.0, 1), opposite_side) 
                            && !is_square_attacked_by(&board, (coord.0, 2), opposite_side) 
                            && !is_square_attacked_by(&board, (coord.0, 3), opposite_side) {
                            if !is_in_check(&board, piece.color) {
                                not_checked.push((coord.0, 2));
                            }
                        }
                    }
                }
            }
            // kingside
            if let Some(sp) = board[coord.0][7] {
                if sp.piece_type == PieceType::Rook && !sp.has_moved {
                    if board[coord.0][6].is_none() && board[coord.0][5].is_none() {
                        if !is_square_attacked_by(&board, (coord.0, 6), opposite_side) 
                            && !is_square_attacked_by(&board, (coord.0, 5), opposite_side) {
                            if !is_in_check(&board, piece.color) {
                                not_checked.push((coord.0, 6));
                            }
                        }
                    }
                }
            }
        }
    }


    // check validity of each move
    let moving_color = board[coord.0][coord.1].unwrap().color;
    for m in not_checked {
        if let Some(sp) = board[m.0][m.1] {
            if sp.color == moving_color { continue; }
        }
        let undo = make_move(board, coord, m);
        let in_check = is_in_check(board, moving_color);
        undo_move(board, undo);
        if !in_check {
            checked.push(m);
        }
    }
    checked
}



// gets all moves that a side can make ((from), (to))
fn get_all_moves(board: &mut Board, side: Side) -> Vec<((usize, usize), (usize, usize))>{
    let mut moves: Vec<((usize, usize), (usize, usize))> = Vec::new();
    for i in 0..64 {
        let r = i / 8;
        let c = i % 8;
        if let Some(p) = board[r][c] {
            if p.color == side {
                for mv in get_valid_moves(board, (r, c)) {
                    moves.push(((r, c), mv));
                }
            }
        }
    }

    moves
}

fn can_castle_kingside(board: &Board, side: Side) -> bool{
    // kingside
    let back_row = if side == Side::White { 7 } else { 0 };
    let opposite_side = if side == Side::White {Side::Black} else {Side::White};
    if let Some(sp) = board[back_row][7] {
        if sp.piece_type == PieceType::Rook && !sp.has_moved {
            if board[back_row][6].is_none() && board[back_row][5].is_none() {
                if !is_square_attacked_by(&board, (back_row, 6), opposite_side) 
                    && !is_square_attacked_by(&board, (back_row, 5), opposite_side) {
                    if !is_in_check(&board, side) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn can_castle_queenside(board: &Board, side: Side) -> bool{
    // kingside
    let back_row = if side == Side::White { 7 } else { 0 };
    let opposite_side = if side == Side::White {Side::Black} else {Side::White};
    if let Some(sp) = board[back_row][7] {
        if sp.piece_type == PieceType::Rook && !sp.has_moved {
            if board[back_row][1].is_none() && board[back_row][2].is_none() && board[back_row][3].is_none(){
                if !is_square_attacked_by(&board, (back_row, 2), opposite_side) 
                    && !is_square_attacked_by(&board, (back_row, 3), opposite_side){
                    if !is_in_check(&board, side) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn draw_board(tile_size: f32) {
    for row in 0..8 {
        for col in 0..8 {
            let color = if (row + col) % 2 == 0 {
                Color::from_rgba(255,219,187, 255) // light square
            } else {
                Color::from_rgba(214, 110, 100, 255)  // dark square
            };
            draw_rectangle(col as f32 * tile_size, row as f32 * tile_size, tile_size, tile_size, color);
        }
    }
}

fn draw_moves(tile_size: f32, board: &mut Board, selected_piece: (usize, usize), flipped: bool){
    let moves: Vec<(usize, usize)> = get_valid_moves(board, selected_piece);
    let color = Color::from_rgba(100, 0, 0, 100);
    for mv in moves {
        if flipped{
            draw_circle((7 - mv.1) as f32 * tile_size + tile_size*0.5, ( 7 - mv.0) as f32 * tile_size + tile_size * 0.5, tile_size / 2.5, color);
        }else{
            draw_circle(mv.1 as f32 * tile_size + tile_size*0.5, mv.0 as f32 * tile_size + tile_size*0.5, tile_size/2.5, color);
        }
        
    }
}

fn piece_label(piece: &Piece) -> &str {
    match (&piece.color, &piece.piece_type) {
        (Side::White, PieceType::King)   => "♔",
        (Side::White, PieceType::Queen)  => "♕",
        (Side::White, PieceType::Rook)   => "♖",
        (Side::White, PieceType::Bishop) => "♗",
        (Side::White, PieceType::Knight) => "♘",
        (Side::White, PieceType::Pawn)   => "♙",
        (Side::Black, PieceType::King)   => "♚",
        (Side::Black, PieceType::Queen)  => "♛",
        (Side::Black, PieceType::Rook)   => "♜",
        (Side::Black, PieceType::Bishop) => "♝",
        (Side::Black, PieceType::Knight) => "♞",
        (Side::Black, PieceType::Pawn)   => "♟",
    }
}

fn draw_pieces(board: &Board, font: &Font, tile_size: f32, flipped: bool) {
    for row in 0..8 {
        for col in 0..8 {
            if let Some(piece) = &board[row][col] {
                let (draw_col, draw_row) = if flipped {
                    (7 - col, 7 - row)
                } else {
                    (col, row)
                };
                let x = draw_col as f32 * tile_size + tile_size * 0.1;
                let y = draw_row as f32 * tile_size + tile_size * 0.8125;
                draw_text_ex(
                    piece_label(piece),
                    x, y,
                    TextParams {
                        font: Some(font),
                        font_size: tile_size as u16,
                        color: BLACK,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

// --- Main Loop ---

#[macroquad::main("Chess Engine")]
async fn main() {
    let mut board = new_board();
    let mut current_turn = Side::White;
    let tile_size: f32 = WINDOW_SIZE / 8.0; 
    let font = load_ttf_font("assets/FreeSerif.ttf").await.unwrap();
    let mut board_flipped = false;
    let move_color:Color = Color::from_rgba(228,217,111, 100);

    let mut white_wins: f32 = 0.0;
    let mut black_wins: f32 = 0.0;
    let mut game_over = false;
    let mut winner: Option<bool> = None;

    let args: Vec<String> = env::args().collect();

    let default_white = String::from("human");
    let default_black = String::from("minimax");
    
    let type1 = args.get(1).unwrap_or(&default_white);
    let type2 = args.get(2).unwrap_or(&default_black);

    //let depth = args.get(3).map(String::as_str) == Some();

    // make sure player1 and player2 are correct options
    let player1: Arc<dyn Player + Send + Sync> = match type1.as_str() {
        "human"     => Arc::new(HumanPlayer),
        "random"    => Arc::new(RandomAI),
        "minimax"   => Arc::new(MinimaxAI { depth: DEPTH }),
        _           => panic!("unknown player type: {type1}"),
    };

    let player2: Arc<dyn Player + Send + Sync> = match type2.as_str() {
        "human"     => Arc::new(HumanPlayer),
        "random"    => Arc::new(RandomAI),
        "minimax"   => Arc::new(MinimaxAI { depth: DEPTH }),
        _           => panic!("unknown player type: {type2}"),
    };

    let mut thinking: Option<Receiver<((usize, usize), (usize, usize))>> = None;

    // set screen size
    macroquad::window::request_new_screen_size(WINDOW_SIZE, WINDOW_SIZE);

    let mut selected_piece: Option<Piece> = None;
    let mut selected_coords: Option<(usize, usize)> = None; 
    let mut last_move: Option<((usize, usize),(usize, usize))> = None;


    loop {
        // restart
        if game_over{
            thread::sleep(Duration::from_secs_f32(5.0));
            board = new_board();
            game_over = false;
            winner = None;

        }


        if macroquad::input::is_key_pressed(KeyCode::F){
            board_flipped = !board_flipped;
        }

        if current_turn == Side::White{
            if player1.as_any().is::<HumanPlayer>(){
                let (x,y) = mouse_position();

                // get any input
                if macroquad::input::is_mouse_button_pressed(MouseButton::Left){
                    let col: usize = if board_flipped{7 - (x / tile_size) as usize}else{(x / tile_size) as usize};
                    let row: usize = if board_flipped{7 - (y / tile_size) as usize}else{ (y / tile_size) as usize};

                    if selected_piece.is_none(){
                        if board[row][col].is_some(){
                            selected_piece = board[row][col];
                            selected_coords = Some((row, col));
                        }
                    }else{
        
                        if get_valid_moves(&mut board, selected_coords.unwrap()).contains(&(row, col)) && board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == current_turn{
                            make_move(&mut board, selected_coords.unwrap(), (row, col));
                            println!("move {}:", board.moves);
                            println!("eval: {}",minimax_ai::evaluate(&board));
                            last_move = Some(((row, col), selected_coords.unwrap()));
                            if get_all_moves(&mut board, Side::Black).len() == 0{
                                if is_in_check(&board, Side::Black){
                                    winner = Some(true); // true for white
                                    white_wins += 1.0;
                                }else{
                                    winner = None; // draw
                                    black_wins += 0.5;
                                    white_wins += 0.5;
                                }
                                game_over = true;
                            }   
                            current_turn = if current_turn == Side::White {Side::Black} else {Side::White};
                            selected_piece = None;
                            selected_coords = None;
                        }else{
                            selected_piece = board[row][col];
                            selected_coords = Some((row, col));
                        }
        
                    }
                }
            }else{

                if thinking.is_none() {
                    let player = Arc::clone(&player1);
                    let board_snapshot = board;   
                    let side = current_turn;
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        let mv = player.get_move(&board_snapshot, side);
                        let _ = tx.send(mv);
                    });
                    thinking = Some(rx);
                }

                if let Some(rx) = &thinking {
                    if let Ok(mv) = rx.try_recv() {
                        make_move(&mut board, mv.0, mv.1);
                        println!("move {}:", board.moves);
                        println!("eval: {}",minimax_ai::evaluate(&board));
                        last_move = Some(mv);
                        if get_all_moves(&mut board, Side::Black).len() == 0{
                            if is_in_check(&board, Side::Black){
                                winner = Some(true); // true for white
                                white_wins += 1.0;
                            }else{
                                winner = None; // draw
                                black_wins += 0.5;
                                white_wins += 0.5;
                            }
                            game_over = true;
                        }   
                        
                        current_turn = if current_turn == Side::White { Side::Black } else { Side::White };
                        thinking = None;
                    }
                }
            }
        }else{
            if player2.as_any().is::<HumanPlayer>(){
                let (x,y) = mouse_position();

                // get any input
                if macroquad::input::is_mouse_button_pressed(MouseButton::Left){
                    let col: usize = if board_flipped{7 - (x / tile_size) as usize}else{(x / tile_size) as usize};
                    let row: usize = if board_flipped{7 - (y / tile_size) as usize}else{ (y / tile_size) as usize};
        
                    if selected_piece.is_none(){
                        if board[row][col].is_some(){
                            selected_piece = board[row][col];
                            selected_coords = Some((row, col));
                        }
                    }else{
        
                        if get_valid_moves(&mut board, selected_coords.unwrap()).contains(&(row, col)) && board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == current_turn{
                            make_move(&mut board, selected_coords.unwrap(), (row, col));
                            println!("move {}:", board.moves);
                            println!("eval: {}",minimax_ai::evaluate(&board));
                            last_move = Some(((row, col), selected_coords.unwrap()));
                            if get_all_moves(&mut board, Side::White).len() == 0{
                                if is_in_check(&board, Side::White){
                                    winner = Some(false); // false for black
                                    black_wins += 1.0;
                                }else{
                                    winner = None; // draw
                                    black_wins += 0.5;
                                    white_wins += 0.5;
                                }
                                game_over = true;
                            }   
                            current_turn = if current_turn == Side::White {Side::Black} else {Side::White};
                            selected_piece = None;
                            selected_coords = None;
                        }else{
                            selected_piece = board[row][col];
                            selected_coords = Some((row, col));
                        }
        
                    }
                }
            }else{

                if thinking.is_none() {
                    let player = Arc::clone(&player2);
                    let board_snapshot = board;   
                    let side = current_turn;
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        let mv = player.get_move(&board_snapshot, side);
                        let _ = tx.send(mv);
                    });
                    thinking = Some(rx);
                }

                if let Some(rx) = &thinking {
                    if let Ok(mv) = rx.try_recv() {
                        make_move(&mut board, mv.0, mv.1);
                        println!("move {}:", board.moves);
                        println!("eval: {}",minimax_ai::evaluate(&board));
                        last_move = Some(mv);
                        if get_all_moves(&mut board, Side::White).len() == 0{
                            if is_in_check(&board, Side::White){
                                winner = Some(false); // false for black
                                black_wins += 1.0;
                            }else{
                                winner = None; // draw
                                black_wins += 0.5;
                                white_wins += 0.5;
                            }
                            game_over = true;
                        }   
                        
                        current_turn = if current_turn == Side::White { Side::Black } else { Side::White };
                        thinking = None;
                    }
                }
            }
        }

        

        // display visuals
        clear_background(WHITE);
        draw_board(tile_size);
        if last_move.is_some(){
            let lm = last_move.unwrap();
            if board_flipped{
                draw_rectangle((7 - lm.1.1) as f32 * tile_size, ( 7 - lm.1.0) as f32 * tile_size, tile_size, tile_size, move_color);
                draw_rectangle((7 - lm.0.1) as f32 * tile_size, ( 7 - lm.0.0) as f32 * tile_size, tile_size, tile_size, move_color);
            }else{
                draw_rectangle(lm.0.1 as f32 * tile_size, lm.0.0 as f32 * tile_size, tile_size, tile_size, move_color);
                draw_rectangle(lm.1.1 as f32 * tile_size, lm.1.0 as f32 * tile_size, tile_size, tile_size, move_color);
            }
        }
        if selected_coords.is_some() && board[selected_coords.unwrap().0][selected_coords.unwrap().1].is_some(){
            if board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == current_turn{
                draw_moves(tile_size, &mut board, selected_coords.unwrap(), board_flipped);
            }
            
        }
        draw_pieces(&board, &font, tile_size, board_flipped);

        if game_over{
            if winner.is_some(){
                if winner.unwrap(){
                    draw_text_ex(
                        "WHITE WINS",
                        100.0, 100.0,
                        TextParams {
                            font: Some(&font),
                            font_size: (tile_size) as u16,
                            color: BLACK,
                            ..Default::default()
                        },
                    );
                }else{
                    draw_text_ex(
                        "BLACK WINS",
                        100.0, 100.0,
                        TextParams {
                            font: Some(&font),
                            font_size: (tile_size) as u16,
                            color: BLACK,
                            ..Default::default()
                        },
                    );
                }
            }else{
                draw_text_ex(
                    "DRAW",
                    100.0, 100.0,
                    TextParams {
                        font: Some(&font),
                        font_size: (tile_size) as u16,
                        color: BLACK,
                        ..Default::default()
                    },
                );
            }
            println!("white wins: {}\nblack wins: {}", white_wins, black_wins);   
        }
        next_frame().await;

    }
}
