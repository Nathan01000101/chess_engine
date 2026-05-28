use macroquad::{ prelude::*};
use macroquad::audio::{load_sound, play_sound_once};
use crate::ai::Player;
use crate::human::HumanPlayer;
use crate::random_ai::RandomAI;
use crate::minimax_ai::MinimaxAI;
use std::fmt::Write;
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
pub const DEPTH: usize = 5;

// for accessing board struct
const WHITE_SHORT:  u8 = 0b00001;
const WHITE_LONG:   u8 = 0b00010;
const BLACK_SHORT:  u8 = 0b00100;
const BLACK_LONG:   u8 = 0b01000;
const WHITE_TO_MOVE: u8 = 0b10000;

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
    color: Side
}


#[derive(Clone, Copy, PartialEq, Debug)]
struct Board{
    squares: [[Option<Piece>; 8]; 8],
    moves: u8,
    white_king: (u8, u8),
    black_king: (u8, u8),
    en_passant_target: Option<(u8, u8)>,
    state: u8 
    // 5th bit -> white to move,  4th bit -> black long castle right, 3rd -> black short castle right
    // 2nd bit -> white long castle right, 1st bit -> white short castle right
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
    previous_en_passant_target: Option<(u8, u8)>, // stores where the en_passant target was before moving
    previous_state: u8
}

// board logic
fn new_board() -> Board {
    from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
}

fn is_piece(board: &Board, coord: (usize, usize)) -> bool {
    if in_bounds(coord.0 as i16, coord.1 as i16){
        board[coord.0][coord.1].is_some()
    }else{
        false
    }
}

fn to_fen(board: &Board) -> String{
        let mut s = String::with_capacity(80);
    
    for row in 0..8 {
        let mut empty = 0;
        for col in 0..8 {
            match board[row][col] {
                None => empty += 1,
                Some(p) => {
                    if empty > 0 {
                        // write the digit, then reset
                        s.push(char::from_digit(empty, 10).unwrap());
                        empty = 0;
                    }
                    s.push(piece_to_char(p));
                }
            }
        }
        if empty > 0 {
            s.push(char::from_digit(empty, 10).unwrap());
        }
        if row < 7 {
            s.push('/');
        }
    }
    
    s.push(' ');
    s.push(if board.state & WHITE_TO_MOVE != 0 { 'w' } else { 'b' });
    
    s.push(' ');
    let castle_start = s.len();
    if board.state & WHITE_SHORT  != 0  { s.push('K'); }
    if board.state & WHITE_LONG   != 0  { s.push('Q'); }
    if board.state & BLACK_SHORT  != 0  { s.push('k'); }
    if board.state & BLACK_LONG   != 0  { s.push('q'); }
    if s.len() == castle_start { s.push('-'); }  // no rights at all
    
    s.push(' ');
    match board.en_passant_target {
        None => s.push('-'),
        Some((r, c)) => {
            s.push((b'a' + c as u8) as char);
            s.push(char::from_digit(8 - r as u32, 10).unwrap());
        }
    }
    
    // halfmove clock + fullmove (you don't track halfmove clock, so just use 0)
    write!(s, " 0 {}", board.moves / 2 + 1).unwrap();
    
    s
}

fn from_fen(fen: &str) -> Board{
    let mut board = Board {squares: [[None; 8]; 8], white_king: (9, 9), black_king: (9, 9), moves: 0, en_passant_target: None, state: 0b10000};

    let parts: Vec<&str> = fen.split(' ').collect();

    let placements = parts[0];
    let mut row = 0;
    let mut col = 0;
    for c in placements.chars(){
        if c == '/'{
            row += 1;
            col = 0;
            continue;
        }
        if c.is_ascii_digit(){
            col += c.to_digit(10).unwrap();
            continue;
        }
        
        let side = if c.is_uppercase() {Side::White} else {Side::Black};
        let p_type = char_to_piece_type(&c);
        if p_type == PieceType::King{
            if side == Side::White{
                board.white_king = (row as u8, col as u8);
            }else{
                board.black_king = (row as u8, col as u8);
            }
        }
        board[row][col as usize] = Some(Piece { piece_type: p_type, color: side});
        col += 1;
    }


    let turn = parts[1];
    if turn.chars().next() == Some('b') { board.state ^= WHITE_TO_MOVE};

    let castling_rights = parts[2];
    for c in castling_rights.chars(){
        if c == 'K'{
            board.state ^= WHITE_SHORT;
        }
        if c == 'Q'{
            board.state ^= WHITE_LONG;
        }
        if c == 'k'{
            board.state ^= BLACK_SHORT;
        }
        if c == 'q'{
            board.state ^= BLACK_LONG;
        }
    }
    let en_passant_target = parts[3];
    if en_passant_target.chars().next() != Some('-'){
        let mut ep_r = 8;
        let mut ep_c = 8;
        let c = en_passant_target.chars().nth(0).unwrap();
        let r = en_passant_target.chars().nth(1).unwrap();
        match c{
            'a' => ep_c = 0,
            'b' => ep_c = 1,
            'c' => ep_c = 2,
            'd' => ep_c = 3,
            'e' => ep_c = 4,
            'f' => ep_c = 5,
            'g' => ep_c = 6,
            'h' => ep_c = 7,
            _ => ep_c = 8
        }
        ep_r = 8 - r.to_digit(10).unwrap();

        board.en_passant_target = Some((ep_r as u8, ep_c as u8));
    }
    board.moves = parts[5].parse::<u8>().unwrap();
    board
}

fn char_to_piece_type(c: &char) -> PieceType{
    match c.to_ascii_lowercase(){
        'k' => return PieceType::King,
        'q' => return PieceType::Queen,
        'r' => return PieceType::Rook,
        'b' => return PieceType::Bishop,
        'n' => return PieceType::Knight,
        'p' => return PieceType::Pawn,
        _   => panic!("unknown piece character: {}", c)
    }
}

fn piece_type_to_char(piece: PieceType) -> char{
    match piece{
        PieceType::King => 'k',
        PieceType::Queen => 'q',
        PieceType::Bishop => 'b',
        PieceType::Knight => 'n',
        PieceType::Rook => 'r',
        PieceType::Pawn => 'p'
    }
}

fn piece_to_char(piece: Piece) -> char{
    let mut c = 'b';
    match piece.piece_type{
        PieceType::King => c = 'k',
        PieceType::Queen => c = 'q',
        PieceType::Bishop => c = 'b',
        PieceType::Knight => c = 'n',
        PieceType::Rook => c = 'r',
        PieceType::Pawn => c = 'p'
    }
    c = if piece.color == Side::White {c.to_ascii_uppercase()} else {c};
    c
}

fn in_bounds(x: i16, y: i16) -> bool{
    x < 8 && x >= 0 && y < 8 && y >= 0
}

// determines if a side is in check with a given board state
fn is_in_check(board: &Board, side: Side) -> bool {
    
    let enemy = if side == Side::White {Side::Black} else {Side::White};
    let king: (usize, usize) = if side == Side::White { (board.white_king.0 as usize,  board.white_king.1 as usize)} else { (board.black_king.0 as usize, board.black_king.1 as usize) };

    is_square_attacked_by(board, king, enemy)
}

// determines if a given square is attacked by a given side
fn is_square_attacked_by(board: &Board, coords: (usize, usize), side: Side) -> bool{
    
    // pawns?
    let pawn_dir = if side == Side::White { 1 } else { -1 };
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
                        previous_en_passant_target: None,
                        previous_state: board.state};
        undo.previous_en_passant_target = board.en_passant_target;
        undo.captured_piece = board[new.0][new.1];
        
        // remove en passant target
        board.en_passant_target = None;
        board.moves += 1;
        board.state ^= WHITE_TO_MOVE;
        if p.piece_type == PieceType::Pawn{
            
            let dy = new.0 as i16 - old.0 as i16;
            if  dy == 2 || dy == -2{
                board.en_passant_target = Some(((old.0 as i16 + dy / 2) as u8, old.1 as u8));
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
                board.state &= !(WHITE_SHORT | WHITE_LONG);
            }else{
                board.black_king = (new.0 as u8, new.1 as u8);
                board.state &= !(BLACK_SHORT | BLACK_LONG);
            }
            let dx = new.1 as i16 - old.1 as i16;
            if dx == 2 {
                if let Some(rook) = board[new.0][7] {
                    board[new.0][5] = Some(rook);
                    board[new.0][7] = None;
                }
            } else if dx == -2 {
                if let Some(rook) = board[new.0][0] {
                    board[new.0][3] = Some(rook);
                    board[new.0][0] = None;
                }
            }
        } else if p.piece_type == PieceType::Rook {
            if p.color == Side::White {
                if undo.last_move.from == (7, 0) { board.state &= !WHITE_LONG; }
                else if undo.last_move.from == (7, 7) { board.state &= !WHITE_SHORT; }
            } else {
                if undo.last_move.from == (0, 0) { board.state &= !BLACK_LONG; }
                else if undo.last_move.from == (0, 7) { board.state &= !BLACK_SHORT; }
            }
        }

        if let Some(piece) = undo.captured_piece{
            if piece.piece_type == PieceType::Rook{
                if piece.color == Side::White{
                    if undo.last_move.to == (7, 0) && undo.previous_state & WHITE_LONG != 0{
                        board.state ^= WHITE_LONG
                    }
                    else if undo.last_move.to == (7,7) && undo.previous_state & WHITE_SHORT != 0{
                        board.state ^= WHITE_SHORT
                    }
                }else{
                    if undo.last_move.to == (0, 0) && undo.previous_state & BLACK_LONG != 0{
                        board.state ^= BLACK_LONG
                    }
                    else if undo.last_move.to == (0, 7) && undo.previous_state & BLACK_SHORT != 0{
                        board.state ^= BLACK_SHORT;
                    }
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
    board.state = undo.previous_state;
    board.moves -= 1;
    board[undo.last_move.from.0][undo.last_move.from.1] = Some(undo.moving_piece_before);
    board[undo.last_move.to.0][undo.last_move.to.1] = undo.captured_piece;
    board.en_passant_target = undo.previous_en_passant_target;
    if undo.moving_piece_before.piece_type == PieceType::Pawn{
        if let Some(target) = undo.previous_en_passant_target{
            if undo.last_move.to == (target.0 as usize, target.1 as usize) && undo.last_move.to.1 != undo.last_move.from.1{
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
            if dx == 2 {
                if let Some(restored_rook) = board[undo.last_move.to.0][5] {
                    board[undo.last_move.to.0][7] = Some(restored_rook);
                    board[undo.last_move.to.0][5] = None;
                }
            } else {
                // queenside
                if let Some(restored_rook) = board[undo.last_move.to.0][3] {
                    board[undo.last_move.to.0][0] = Some(restored_rook);
                    board[undo.last_move.to.0][3] = None;
                }
            }
        }

    }
}

// gets all moves for a piece DOES NOT INCLUDE CHECKING THAT KING IS LEFT VISIBLE OR TAKES SAME COLOURED PIECES
fn get_pseudo_legal_moves(board: &Board, coord: (usize, usize), list: &mut Vec<(usize, usize)>){
    let piece: Option<Piece> = board[coord.0][coord.1];

    if let Some(p) = piece {
        match p.piece_type{
            PieceType::Bishop => {
                // generate possible moves
                for dir in BISHOP_DIRS{
                    for i in 1..8{
                        if in_bounds(coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i){
                            list.push(((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize));
                            if is_piece(board, ((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize)){    
                                break;
                            }
                        }else{
                            break;
                        }
                    }
                }
            },
            PieceType::Knight => {

                for offset in KNIGHT_OFFSETS{
                    if in_bounds(coord.0 as i16 - offset.0, coord.1 as i16 - offset.1){
                        list.push(((coord.0 as i16 - offset.0) as usize, (coord.1 as i16 - offset.1) as usize));
                    } 
                }
            },
            PieceType::King => {
                for offset in KING_OFFSETS{
                    if in_bounds(coord.0 as i16 - offset.0, coord.1 as i16 - offset.1){
                        list.push(((coord.0 as i16 - offset.0) as usize, (coord.1 as i16 - offset.1) as usize));
                    } 
                }
            },
            PieceType::Pawn => {
                if p.color == Side::White {

                    // capturing tiles

                    if coord.1 > 0 && is_piece(board, (coord.0 - 1, coord.1 - 1)) {
                        list.push((coord.0.saturating_sub(1), coord.1.saturating_sub(1)));
                    }
                    

                    if is_piece(board, (coord.0 - 1, coord.1 + 1)) {
                        list.push((coord.0.saturating_sub(1), coord.1 + 1));
                    }

                    // en passant
                    if let Some(target) = board.en_passant_target {
                        if target.0 == coord.0.wrapping_sub(1) as u8
                            && (target.1 as i16 - coord.1 as i16).abs() == 1
                        {
                            list.push((target.0 as usize , target.1 as usize));
                        }
                    }

                    // moving forward
                    if coord.0 > 0 && board[coord.0 - 1][coord.1].is_none() {
                        list.push((coord.0.saturating_sub(1), coord.1));
                        if coord.0 == 6 && board[coord.0 - 2][coord.1].is_none(){
                            list.push((coord.0.saturating_sub(2), coord.1));
                        }
                    }
                } else {
                    // Black moves toward row 7 (increasing rows)

                    // capturing tiles
                    if coord.1 > 0 && is_piece(board, (coord.0 + 1, coord.1 - 1)) {
                        list.push((coord.0 + 1, coord.1.saturating_sub(1)) );
                    }
                    if is_piece(board, (coord.0 + 1, coord.1 + 1)) {
                        list.push((coord.0 + 1, coord.1 + 1));
                    }

                    // en passant
                    if let Some(target) = board.en_passant_target {
                        if target.0 == coord.0 as u8 + 1
                            && (target.1 as i16 - coord.1 as i16).abs() == 1
                        {
                            list.push((target.0 as usize, target.1 as usize));
                        }
                    }

                    // moving forward
                    if board[coord.0 + 1][coord.1].is_none() {
                        list.push((coord.0 + 1, coord.1));
                        if coord.0 == 1 && board[coord.0 + 2][coord.1].is_none(){
                            list.push((coord.0 + 2, coord.1));
                        }
                    }
                }
            },
            PieceType::Queen => {
                // generate possible moves
                for dir in QUEEN_DIRS{
                    for i in 1..8{
                        if in_bounds(coord.0 as i16 + dir.0 * i, coord.1 as i16 + dir.1 * i){
                            list.push(((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize));
                            if is_piece(board, ((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize)){
                                break;
                            }
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
                            list.push(((coord.0 as i16 + dir.0 * i) as usize, (coord.1 as i16 + dir.1 * i) as usize));
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
    }
}

// Returns true if `sq` lies on the same rank, file, or diagonal as `king`.
fn is_on_ray_from(king: (usize, usize), sq: (usize, usize)) -> bool {
    if king == sq { return false; }
    let dr = sq.0 as i16 - king.0 as i16;
    let dc = sq.1 as i16 - king.1 as i16;
    // Same rank, same file, or same diagonal (|dr| == |dc|).
    dr == 0 || dc == 0 || dr.abs() == dc.abs()
}

// returns all valid moves for a piece 
fn get_valid_moves(board: &mut Board, coord: (usize, usize) ) -> Vec<(usize, usize)> {
    if board[coord.0][coord.1].is_none() {return Vec::new()}
    let piece = board[coord.0][coord.1].unwrap();
    let mut not_checked = Vec::with_capacity(28);
    get_pseudo_legal_moves(&board, coord, &mut not_checked);
    let mut checked = Vec::with_capacity(28);

    // add castling move if neccesary
    if piece.piece_type == PieceType::King{
        // castling

        let opposite_side = if piece.color == Side::White { Side::Black } else { Side::White };
        let can_long = if piece.color == Side::White {board.state & WHITE_LONG != 0} else {board.state & BLACK_LONG != 0};
        let can_short = if piece.color == Side::White {board.state & WHITE_SHORT != 0} else {board.state & BLACK_SHORT != 0};
        // queenside
        if can_long{
            if board[coord.0][1].is_none() && board[coord.0][2].is_none() && board[coord.0][3].is_none() {
                if 
                    !is_square_attacked_by(&board, (coord.0, 2), opposite_side) 
                    && !is_square_attacked_by(&board, (coord.0, 3), opposite_side) {
                    if !is_in_check(&board, piece.color) {
                        not_checked.push((coord.0, 2));
                    }
                }
            }
        }

        // kingside
        if can_short {
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

    
    // check validity of each move
    // --- legality filtering ---
    let moving_color = piece.color;
    let king_sq = if moving_color == Side::White {
        (board.white_king.0 as usize, board.white_king.1 as usize)
    } else {
        (board.black_king.0 as usize, board.black_king.1 as usize)
    };

    // Hoisted out of the loop: these don't change as we iterate destinations.
    let currently_in_check = is_in_check(board, moving_color);
    let piece_is_king = piece.piece_type == PieceType::King;
    let piece_on_king_ray = is_on_ray_from(king_sq, coord);

    // En passant is a special case: removing the captured pawn can expose
    // the king along the rank even if the moving pawn wasn't pinned.
    // We detect it per-destination since it depends on `to`.
    let ep_target = board.en_passant_target;

    for m in not_checked {
        if let Some(sp) = board[m.0][m.1] {
            if sp.color == moving_color { continue; }
        }

        // Is this move en passant? Pawn moving diagonally to the EP target,
        let is_en_passant = piece.piece_type == PieceType::Pawn
            && m.1 != coord.1
            && ep_target.map(|(r,c)| (r as usize, c as usize)) == Some(m);

        let needs_full_check = piece_is_king
            || currently_in_check
            || piece_on_king_ray
            || is_en_passant;

        if !needs_full_check {
            checked.push(m);
            continue;
        }

        // Slow path: actually try the move and see.
        let undo = make_move(board, coord, m);
        let in_check = is_in_check(board, moving_color);
        undo_move(board, undo);
        if !in_check {
            checked.push(m);
        }
    }
    checked
}


// returns only moves that capture pieces (including en passant) for a single piece
fn get_capture_moves(board: &mut Board, coord: (usize, usize)) -> Vec<(usize, usize)> {
    if board[coord.0][coord.1].is_none() { return Vec::new(); }
    let piece = board[coord.0][coord.1].unwrap();
    let mut pseudo: Vec<(usize, usize)> = Vec::with_capacity(28);
    get_pseudo_legal_moves(&board, coord, &mut pseudo);
    let mut captures: Vec<(usize, usize)> = Vec::with_capacity(8);

    let moving_color = piece.color;
    let king_sq = if moving_color == Side::White {
        (board.white_king.0 as usize, board.white_king.1 as usize)
    } else {
        (board.black_king.0 as usize, board.black_king.1 as usize)
    };

    let currently_in_check = is_in_check(board, moving_color);
    let piece_is_king = piece.piece_type == PieceType::King;
    let piece_on_king_ray = is_on_ray_from(king_sq, coord);
    let ep_target = board.en_passant_target;

    for m in pseudo {
        let is_en_passant = piece.piece_type == PieceType::Pawn
            && m.1 != coord.1
            && ep_target.map(|(r,c)| (r as usize, c as usize)) == Some(m);

        let is_capture = match board[m.0][m.1] {
            Some(p) => p.color != moving_color,
            None => is_en_passant,
        };
        if !is_capture { continue; }

        let needs_full_check = piece_is_king
            || currently_in_check
            || piece_on_king_ray
            || is_en_passant;

        if !needs_full_check {
            captures.push(m);
            continue;
        }

        let undo = make_move(board, coord, m);
        let in_check = is_in_check(board, moving_color);
        undo_move(board, undo);
        if !in_check {
            captures.push(m);
        }
    }

    captures
}

// gets all captures a side can make ((from), (to))
fn get_all_captures(board: &mut Board, side: Side) -> Vec<((usize, usize), (usize, usize))> {
    let mut moves: Vec<((usize, usize), (usize, usize))> = Vec::with_capacity(128);
    for i in 0..64 {
        let r = i / 8;
        let c = i % 8;
        if let Some(p) = board[r][c] {
            if p.color == side {
                for mv in get_capture_moves(board, (r, c)) {
                    moves.push(((r, c), mv));
                }
            }
        }
    }
    moves
}

// gets all moves that a side can make ((from), (to))
fn get_all_moves(board: &mut Board, side: Side) -> Vec<((usize, usize), (usize, usize))>{
    let mut moves: Vec<((usize, usize), (usize, usize))> = Vec::with_capacity(218);
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
    let tile_size: f32 = WINDOW_SIZE / 8.0; 
    let font = load_ttf_font("assets/FreeSerif.ttf").await.unwrap();
    let mut board_flipped = false;
    let move_color:Color = Color::from_rgba(228,217,111, 100);
    let check_color: Color = Color::from_rgba(250, 50, 50, 180);

    //load sfx
    let move_check = load_sound("assets/move-check.wav").await.unwrap();
    let move_normal = load_sound("assets/move-self.wav").await.unwrap();
    let move_capture = load_sound("assets/capture.wav").await.unwrap();
    let move_castle = load_sound("assets/castle.wav").await.unwrap();
    let game_finished = load_sound("assets/game-end.wav").await.unwrap();

    let mut white_wins: f32 = 0.0;
    let mut black_wins: f32 = 0.0;
    let mut game_over = false;
    let mut winner: Option<bool> = None;

    let args: Vec<String> = env::args().collect();

    let default_white = String::from("human");
    let default_black = String::from("minimax");
    
    let type1 = args.get(1).unwrap_or(&default_white);
    let type2 = args.get(2).unwrap_or(&default_black);

    let fen = args.get(3).map(String::as_str).unwrap_or("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let mut board = from_fen(fen);

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
            last_move = None;
        }


        if macroquad::input::is_key_pressed(KeyCode::F){
            board_flipped = !board_flipped;
        }

        if board.state & WHITE_TO_MOVE != 0{
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
        
                        if get_valid_moves(&mut board, selected_coords.unwrap()).contains(&(row, col)) && board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == if board.state & WHITE_TO_MOVE != 0 {Side::White} else {Side::Black}{
                            let move_info: Undo = make_move(&mut board, selected_coords.unwrap(), (row, col));
                            println!("\nmove {}:", board.moves);
                            println!("eval: {}",minimax_ai::evaluate(&board));
                            last_move = Some(((row, col), selected_coords.unwrap()));

                            if is_in_check(&board, Side::Black){
                                if get_all_moves(&mut board, Side::Black).len() == 0{
                                    game_over = true;
                                    winner = Some(true); // true for white
                                    white_wins += 1.0;
                                }
                                play_sound_once(&move_check); 
                            }else{
                                if get_all_moves(&mut board, Side::Black).len() == 0 && get_all_moves(&mut board, Side::White).len() == 0{
                                    winner = None; // draw
                                    game_over = true;
                                    black_wins += 0.5;
                                    white_wins += 0.5;
                                }else{ // regular move
                                    if move_info.captured_piece.is_some(){
                                        play_sound_once(&move_capture);
                                    }else{
                                        // check for castle
                                        if move_info.moving_piece_before.piece_type == PieceType::King && (move_info.last_move.to.1 as i32 - move_info.last_move.from.1 as i32).abs() == 2{
                                            play_sound_once(&move_castle);
                                        }else{
                                            play_sound_once(&move_normal);
                                        }
                                    }
                                }
                            }

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
                    let side = Side::White;
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        let mv = player.get_move(&board_snapshot, side);
                        let _ = tx.send(mv);
                    });
                    thinking = Some(rx);
                }

                if let Some(rx) = &thinking {
                    if let Ok(mv) = rx.try_recv() {
                        let move_info = make_move(&mut board, mv.0, mv.1);
                        println!("\nmove {}:", board.moves);
                        println!("eval: {}",minimax_ai::evaluate(&board));
                        last_move = Some(mv);
                        if is_in_check(&board, Side::Black){
                            if get_all_moves(&mut board, Side::Black).len() == 0{
                                game_over = true;
                                play_sound_once(&game_finished);
                                winner = Some(true); // true for white
                                white_wins += 1.0;
                            }
                            play_sound_once(&move_check); 
                        }else{
                            if get_all_moves(&mut board, Side::Black).len() == 0 && get_all_moves(&mut board, Side::White).len() == 0{
                                winner = None; // draw
                                game_over = true;
                                play_sound_once(&game_finished);
                                black_wins += 0.5;
                                white_wins += 0.5;
                            }else{ // regular move
                                if move_info.captured_piece.is_some(){
                                    play_sound_once(&move_capture);
                                }else{
                                    // check for castle
                                    if move_info.moving_piece_before.piece_type == PieceType::King && (move_info.last_move.to.1 as i32 - move_info.last_move.from.1 as i32).abs() == 2{
                                        play_sound_once(&move_castle);
                                    }else{
                                        play_sound_once(&move_normal);
                                    }
                                }
                            }
                        }

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
        
                        if get_valid_moves(&mut board, selected_coords.unwrap()).contains(&(row, col)) && board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == if board.state & WHITE_TO_MOVE != 0 {Side::White} else {Side::Black}{
                            let move_info = make_move(&mut board, selected_coords.unwrap(), (row, col));
                            println!("\nmove {}:", board.moves);
                            println!("eval: {}",minimax_ai::evaluate(&board));
                            last_move = Some(((row, col), selected_coords.unwrap()));

                            if is_in_check(&board, Side::White){
                                if get_all_moves(&mut board, Side::White).len() == 0{
                                    game_over = true;
                                    play_sound_once(&game_finished);
                                    winner = Some(false); // true for white
                                    black_wins += 1.0;
                                }
                                play_sound_once(&move_check); 
                            }else{
                                if get_all_moves(&mut board, Side::Black).len() == 0 && get_all_moves(&mut board, Side::White).len() == 0{
                                    winner = None; // draw
                                    game_over = true;
                                    play_sound_once(&game_finished);
                                    black_wins += 0.5;
                                    white_wins += 0.5;
                                }else{ // regular move
                                    if move_info.captured_piece.is_some(){
                                        play_sound_once(&move_capture);
                                    }else{
                                        // check for castle
                                        if move_info.moving_piece_before.piece_type == PieceType::King && (move_info.last_move.to.1 as i32 - move_info.last_move.from.1 as i32).abs() == 2{
                                            play_sound_once(&move_castle);
                                        }else{
                                            play_sound_once(&move_normal);
                                        }
                                    }
                                }
                            }   
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
                    let side = Side::Black;
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        let mv = player.get_move(&board_snapshot, side);
                        let _ = tx.send(mv);
                    });
                    thinking = Some(rx);
                }

                if let Some(rx) = &thinking {
                    if let Ok(mv) = rx.try_recv() {
                        let move_info = make_move(&mut board, mv.0, mv.1);
                        println!("\nmove {}:", board.moves);
                        println!("eval: {}",minimax_ai::evaluate(&board));
                        last_move = Some(mv);

                        if is_in_check(&board, Side::White){
                            if get_all_moves(&mut board, Side::White).len() == 0{
                                game_over = true;
                                play_sound_once(&game_finished);
                                winner = Some(false); // true for white
                                black_wins += 1.0;
                            }
                            play_sound_once(&move_check); 
                        }else{
                            if get_all_moves(&mut board, Side::Black).len() == 0 && get_all_moves(&mut board, Side::White).len() == 0{
                                winner = None; // draw
                                game_over = true;
                                play_sound_once(&game_finished);
                                black_wins += 0.5;
                                white_wins += 0.5;
                            }else{ // regular move
                                if move_info.captured_piece.is_some(){
                                    play_sound_once(&move_capture);
                                }else{
                                    // check for castle
                                    if move_info.moving_piece_before.piece_type == PieceType::King && (move_info.last_move.to.1 as i32 - move_info.last_move.from.1 as i32).abs() == 2{
                                        play_sound_once(&move_castle);
                                    }else{
                                        play_sound_once(&move_normal);
                                    }
                                }
                            }
                        }   
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
            if board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == if board.state & WHITE_TO_MOVE != 0 {Side::White} else {Side::Black}{
                draw_moves(tile_size, &mut board, selected_coords.unwrap(), board_flipped);
            }
            
        }
        if is_in_check(&board, Side::White){
            if board_flipped{
                draw_rectangle((7 - board.white_king.1) as f32 * tile_size, ( 7 - board.white_king.0) as f32 * tile_size, tile_size, tile_size, check_color);
            }else{
                draw_rectangle(board.white_king.1 as f32 * tile_size, board.white_king.0 as f32 * tile_size, tile_size, tile_size, check_color);
            }
        }
        if is_in_check(&board, Side::Black){
            if board_flipped{
                draw_rectangle((7 - board.black_king.1) as f32 * tile_size, ( 7 - board.black_king.0) as f32 * tile_size, tile_size, tile_size, check_color);
            }else{
                draw_rectangle(board.black_king.1 as f32 * tile_size, board.black_king.0 as f32 * tile_size, tile_size, tile_size, check_color);
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
