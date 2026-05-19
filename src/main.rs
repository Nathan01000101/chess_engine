use macroquad::{ prelude::*};
use crate::ai::Player;
use crate::human::HumanPlayer;
use crate::minimax_ai_multithread::MinimaxMTAI;
use crate::random_ai::RandomAI;
use crate::minimax_ai::MinimaxAI;
use std::collections::HashSet;
use std::time::Duration;
use std::thread;
use std::env;

mod ai;
mod human;
mod random_ai;
mod minimax_ai;
mod minimax_ai_multithread;

const WINDOW_SIZE: f32 = 600.0;
pub const DEPTH: u8 = 5;
const GAMES: u8 = 16;

#[derive(Clone, Copy, PartialEq)]
enum PieceType {
    Pawn, Knight, Bishop, Rook, Queen, King
}

#[derive(Clone, Copy, PartialEq)]
pub enum Side {
    White, Black
}

#[derive(Clone, Copy, PartialEq)]
struct Piece {
    piece_type: PieceType,
    color: Side,
    has_moved: bool,
    pawn_doubled_moved: bool
}

pub type Board = [[Option<Piece>; 8]; 8];



// board logic
fn new_board() -> Board {
    let mut board = [[None; 8]; 8];

    // Helper closure to place a piece
    let w = |pt| Some(Piece { piece_type: pt, color: Side::White, has_moved: false, pawn_doubled_moved: false});
    let b = |pt| Some(Piece { piece_type: pt, color: Side::Black, has_moved: false, pawn_doubled_moved: false });

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
    board[coord.0][coord.1].is_some()
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
fn is_in_check(board: &Board, side: Side) -> bool{
    let mut opposing_pieces: HashSet<(usize, usize)> = HashSet::new();
    let mut attacked_squares: HashSet<(usize, usize)> = HashSet::new();
    let mut king: Option<(usize, usize)> = None; 

    //find king and opposing pieces
    for i in 0..64{
        let c: usize = i % 8;
        let r: usize = i / 8;
        
        if let Some(p) = board[r][c]{
            if p.color != side{
                opposing_pieces.insert((r, c));
            }else{
                if is_piece_type(board, (r, c), PieceType::King){
                    king = Some((r, c));
                      
                }
            }
        }
    }

    // check if we never updated the king coords
    let king = king.expect("NO KING FOUND");

    // get all valid moves from all opposing pieces
    for piece in opposing_pieces{
        for valid in get_attacked_squares(board, piece){
            attacked_squares.insert(valid);
        }
    }

    attacked_squares.contains(&king)
}

// determines if a given square is attacked by a given side
fn is_square_attacked_by(board: &Board, coords: (usize, usize), side: Side) -> bool{
    //check all pieces
    for i in 0..64{
        let c: usize = i % 8;
        let r: usize = i / 8;
        
        if let Some(p) = board[r][c]{
            if p.color == side{
                for mv in get_attacked_squares(board, (r, c)){
                    if mv == coords {return true}
                }
                
            }
        }
    }
    false
}

fn move_piece_to(board: &mut Board, old: (usize, usize), new: (usize, usize)){
    if let Some(mut p) = board[old.0][old.1] {
        p.has_moved = true;
        
        // remove all pawns doubled moved flag
        for i in 0..64 {
            let r = i / 8;
            let c = i % 8;
            if let Some(ref mut p) = board[r][c] {
                if p.piece_type == PieceType::Pawn {
                    p.pawn_doubled_moved = false;
                }
            }
        }
        if p.piece_type == PieceType::Pawn{
            
            let dy = (new.0 as i16 - old.0 as i16).abs();
            if  dy == 2{
                p.pawn_doubled_moved = true;
            }
            // check for en passant and for updating doubled moved
            if new.1 as i16 - old.1 as i16 != 0{
                if board[new.0][new.1].is_none(){
                    board[old.0][new.1] = None;
                } 
            }

            // check if promoted

            if new.0 == 0 || new.0 == 7{
                p.piece_type = PieceType::Queen;
            }
        }else if p.piece_type == PieceType::King {
            // check for castling move
            let dx = new.1 as i16 - old.1 as i16;
            //king side
            if dx == 2{
                move_piece_to(board, (new.0, 7), (new.0, 5));
            }else if dx == -2{ // queen side
                move_piece_to(board, (new.0, 0), (new.0, 3));
            }
        }
        board[new.0][new.1] = Some(p);
        board[old.0][old.1] = None;
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
                let dirs: Vec<(i16, i16)> = vec![(-1, 1), (1, 1), (-1, -1), (1, -1)];

                // generate possible moves
                for dir in dirs{
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
                let dirs: Vec<(i16, i16)> = vec![(-2, 0), (2, 0), (0, -2), (0, 2)];

                // generate possible moves
                for dir in dirs{
                    if dir.0 == 0 {
                        possible.push(((coord.0 as i16 - 1), (dir.1 + coord.1 as i16)));
                        possible.push(((coord.0 as i16 + 1), (dir.1 + coord.1 as i16)));
                    }else{
                        possible.push(((coord.0 as i16 + dir.0), (coord.1 as i16 - 1)));
                        possible.push(((coord.0 as i16+ dir.0), (coord.1 as i16 + 1)));
                    }
                }
            },
            PieceType::King => {
                for dx in -1..2{
                    for dy in -1..2{
                        if dx == 0 && dy == 0 {continue;}
                        possible.push((coord.0 as i16 + dy, coord.1 as i16 + dx))
                    }
                }

            },
            PieceType::Pawn => {
                if p.color == Side::White{

                    // capturing tiles
                    if in_bounds( coord.0 as i16 - 1, coord.1 as i16 - 1){
                        if is_piece(board, (coord.0 - 1, coord.1 - 1)){
                            possible.push((coord.0 as i16 - 1, coord.1 as i16 - 1));
                        }
                    }
                    if in_bounds(coord.0 as i16 - 1, coord.1 as i16 + 1){
                        if is_piece(board, (coord.0 - 1, coord.1 + 1)){
                            possible.push((coord.0 as i16 - 1, coord.1 as i16 + 1));
                        }
                    }

                    
                    // en passant
                    if in_bounds(coord.0 as i16, coord.1 as i16 - 1){
                        if let Some(sp) = board[coord.0][coord.1 - 1]{
                            if sp.piece_type == PieceType::Pawn && sp.color != p.color && sp.pawn_doubled_moved{
                                possible.push((coord.0 as i16 - 1, coord.1 as i16 - 1));
                            }        
                        }
                    }
                    if in_bounds(coord.0 as i16, coord.1 as i16 + 1){
                            if let Some(sp) = board[coord.0][coord.1 + 1]{
                                if sp.piece_type == PieceType::Pawn && sp.color != p.color && sp.pawn_doubled_moved{
                                    possible.push((coord.0 as i16 - 1, coord.1 as i16 + 1));
                                }        
                            }
                    }

                    // regular moving
                    if in_bounds(coord.0 as i16 -1, coord.1 as i16){
                        if board[coord.0 - 1][coord.1].is_none(){
                            possible.push((coord.0 as i16 - 1, coord.1 as i16));
                            if in_bounds(coord.0 as i16 - 2, coord.1 as i16){
                                if board[coord.0 - 2][coord.1].is_none() && !p.has_moved{
                                    possible.push((coord.0 as i16 - 2, coord.1 as i16));
                                }
                            }
                        }
                    }
 

                }else{
                    // capturing tiles
                    if in_bounds( coord.0 as i16 + 1, coord.1 as i16 - 1){
                        if is_piece(board, (coord.0 + 1, coord.1 - 1)){
                            possible.push((coord.0 as i16 + 1, coord.1 as i16 - 1));
                        }
                    }
                    if in_bounds(coord.0 as i16 + 1, coord.1 as i16 + 1){
                        if is_piece(board, (coord.0 + 1, coord.1 + 1)){
                            possible.push((coord.0 as i16 + 1, coord.1 as i16 + 1));
                        }
                    }

                    
                    // en passant
                    if in_bounds(coord.0 as i16, coord.1 as i16 - 1){
                        if let Some(sp) = board[coord.0][coord.1 - 1]{
                            if sp.piece_type == PieceType::Pawn && sp.color != p.color && sp.pawn_doubled_moved{
                                possible.push((coord.0 as i16 + 1, coord.1 as i16 - 1));
                            }        
                        }
                    }
                    if in_bounds(coord.0 as i16, coord.1 as i16 + 1){
                            if let Some(sp) = board[coord.0][coord.1 + 1]{
                                if sp.piece_type == PieceType::Pawn && sp.color != p.color && sp.pawn_doubled_moved{
                                    possible.push((coord.0 as i16 + 1, coord.1 as i16 + 1));
                                }        
                            }
                    }

                    // regular moving
                    if in_bounds(coord.0 as i16 + 1, coord.1 as i16){
                        if board[coord.0 + 1][coord.1].is_none(){
                            possible.push((coord.0 as i16 + 1, coord.1 as i16));
                            if in_bounds(coord.0 as i16 + 2, coord.1 as i16){
                                if board[coord.0 + 2][coord.1].is_none() && !p.has_moved{
                                    possible.push((coord.0 as i16 + 2, coord.1 as i16));
                                }
                            }
                        }
                    }
                }
            },
            PieceType::Queen => {
                let mut dirs: Vec<(i16, i16)> = vec![(-1, 0), (1, 0), (0, -1), (0, 1)];

                // generate possible moves
                for dir in dirs{
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

                dirs = vec![(-1, 1), (1, 1), (-1, -1), (1, -1)];

                // generate possible moves
                for dir in dirs{
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
                let dirs: Vec<(i16, i16)> = vec![(-1, 0), (1, 0), (0, -1), (0, 1)];

                // generate possible moves
                for dir in dirs{
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


fn get_valid_moves(board: &Board, coord: (usize, usize) ) -> Vec<(usize, usize)> {
    if board[coord.0][coord.1].is_none() {return Vec::new()}
    let piece = board[coord.0][coord.1].unwrap();
    let mut not_checked: Vec<(usize, usize)> = get_attacked_squares(&board, coord);
    let mut checked: Vec<(usize, usize)> = Vec::new();

    // add castling move if neccesary
    if piece.piece_type == PieceType::King{
            // castling - belongs HERE, not in get_attacked_squares
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
    for m in not_checked {
        // dont let us take same coloured pieces
        if let Some(sp) = board[m.0 as usize][m.1 as usize]{
            if sp.color == board[coord.0][coord.1].unwrap().color{
                continue;
            }
        }

        // dont let king be in check
        let mut nb = board.clone();


        move_piece_to(&mut nb, coord, (m.0 , m.1 ));
        if is_in_check(&nb, board[coord.0][coord.1].unwrap().color){
            continue;
        }
        checked.push((m.0 as usize, m.1 as usize));
        
    }
    checked
}



// gets all moves that a side can make ((from), (to))
fn get_all_moves(board: &Board, side: Side) -> Vec<((usize, usize), (usize, usize))>{
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


fn draw_board(tile_size: f32) {
    for row in 0..8 {
        for col in 0..8 {
            let color = if (row + col) % 2 == 0 {
                Color::from_rgba(255,219,187, 255) // light square
            } else {
                Color::from_rgba(250, 128, 114, 255)  // dark square
            };
            draw_rectangle(col as f32 * tile_size, row as f32 * tile_size, tile_size, tile_size, color);
        }
    }
}

fn draw_moves(tile_size: f32, board: &Board, selected_piece: (usize, usize), flipped: bool){
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
    let move_color:Color = Color::from_rgba(228,217,111, 255);

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
    let player1: Box<dyn Player> = match type1.as_str() {
        "human"     => Box::new(HumanPlayer),
        "random"    => Box::new(RandomAI),
        "minimax"   => Box::new(MinimaxAI {depth: DEPTH}),
        "minimaxmt" => Box::new(MinimaxMTAI {depth: 6}),
        _           => panic!("unknown player type: {type1}")
    };

    let player2: Box<dyn Player> = match type2.as_str() {
        "human"     => Box::new(HumanPlayer),
        "random"    => Box::new(RandomAI),
        "minimax"   => Box::new(MinimaxAI {depth: DEPTH}),
        "minimaxmt" => Box::new(MinimaxMTAI {depth: 6}),
        _           => panic!("unknown player type: {type2}")
    };
    
    
    

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
                    let col: usize = if board_flipped{
                        7 - (x / tile_size) as usize
                    }else{
                        (x / tile_size) as usize
                    };

                    let row: usize = if board_flipped{
                        7 - (y / tile_size) as usize
                    }else{
                        (y / tile_size) as usize
                    };


                    if selected_piece.is_none(){
                        if board[row][col].is_some(){
                            selected_piece = board[row][col];
                            selected_coords = Some((row, col));
                        }
                    }else{
        
                        if get_valid_moves(&board, selected_coords.unwrap()).contains(&(row, col)) && board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == current_turn{
                            move_piece_to(&mut board, selected_coords.unwrap(), (row, col));
                            last_move = Some(((row, col), selected_coords.unwrap()));
                            current_turn = if current_turn == Side::White {Side::Black} else {Side::White};
                            println!("eval: {}",minimax_ai::evaluate(&board));
                            selected_piece = None;
                            selected_coords = None;
                        }else{
                            selected_piece = board[row][col];
                            selected_coords = Some((row, col));
                        }
        
                    }
                }
            }else{
                let mv: ((usize, usize), (usize, usize)) = player1.get_move(&board, current_turn);
                move_piece_to(&mut board, mv.0, mv.1);
                last_move = Some(mv);
                println!("eval: {}",minimax_ai::evaluate(&board));
                if get_all_moves(&board, Side::Black).len() == 0{
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
            }
        }else{
            if player2.as_any().is::<HumanPlayer>(){
                let (x,y) = mouse_position();

                // get any input
                if macroquad::input::is_mouse_button_pressed(MouseButton::Left){
                    let col: usize = if board_flipped{
                        (x / tile_size) as usize
                    }else{
                        (x / tile_size) as usize
                    };

                    let row: usize = if board_flipped{
                        7 - (y / tile_size) as usize
                    }else{
                        (y / tile_size) as usize
                    };
        
                    if selected_piece.is_none(){
                        if board[row][col].is_some(){
                            selected_piece = board[row][col];
                            selected_coords = Some((row, col));
                        }
                    }else{
        
                        if get_valid_moves(&board, selected_coords.unwrap()).contains(&(row, col)) && board[selected_coords.unwrap().0][selected_coords.unwrap().1].unwrap().color == current_turn{
                            move_piece_to(&mut board, selected_coords.unwrap(), (row, col));
                            last_move = Some(((row, col), selected_coords.unwrap()));
                            println!("eval: {}",minimax_ai::evaluate(&board));
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
                let mv: ((usize, usize), (usize, usize)) = player2.get_move(&board, current_turn);
                move_piece_to(&mut board, mv.0, mv.1);
                last_move = Some(mv);
                println!("eval: {}",minimax_ai::evaluate(&board));
                if get_all_moves(&board, Side::White).len() == 0{
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
            }
        }
        
        thread::sleep(Duration::from_secs(1));
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
                draw_moves(tile_size, &board, selected_coords.unwrap(), board_flipped);
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
