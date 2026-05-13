use macroquad::{ prelude::*};
use std::collections::HashSet;

const WINDOW_SIZE: f32 = 600.0;

#[derive(Clone, Copy, PartialEq)]
enum PieceType {
    Pawn, Knight, Bishop, Rook, Queen, King
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    White, Black
}

#[derive(Clone, Copy, PartialEq)]
struct Piece {
    piece_type: PieceType,
    color: Side,
    has_moved: bool,
    pawn_doubled_moved: bool
}

type Board = [[Option<Piece>; 8]; 8];

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
            println!("{dy}");
            if  dy == 2{
                p.pawn_doubled_moved = true;
            }
            // check for en passant and for updating doubled moved
            if (new.1 as i16 - old.1 as i16) != 0{
                if board[new.0][new.1].is_none(){
                    board[old.0][new.1] = None;
                } 
            }
        }
        board[new.0][new.1] = Some(p);
        board[old.0][old.1] = None;
    }
}

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

                // dont let king be in check
                let mut potential_board: Board = board.clone();
                move_piece_to(&mut potential_board, coord, (m.0 as usize, m.1 as usize));
                if is_in_check(&potential_board, p.color){
                    continue;
                }

                valid.push((m.0 as usize, m.1 as usize));
            }
        }
    }

    valid
}




fn draw_board(tile_size: f32) {
    for row in 0..8 {
        for col in 0..8 {
            let color = if (row + col) % 2 == 0 {
                Color::from_rgba(240, 217, 181, 255) // light square
            } else {
                Color::from_rgba(181, 136, 99, 255)  // dark square
            };
            draw_rectangle(col as f32 * tile_size, row as f32 * tile_size, tile_size, tile_size, color);
        }
    }
}

fn draw_moves(tile_size: f32, board: &Board, selected_piece: (usize, usize)){
    let moves: Vec<(usize, usize)> = get_valid_moves(board, selected_piece);
    let color = Color::from_rgba(100, 0, 0, 100);
    for mv in moves {
        //draw_rectangle(mv.1 as f32 * tile_size, mv.0 as f32 * tile_size, tile_size, tile_size, color);
        draw_circle(mv.1 as f32 * tile_size + tile_size*0.5, mv.0 as f32 * tile_size + tile_size*0.5, tile_size/2.5, color);
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

fn draw_pieces(board: &Board, font: &Font, tile_size: f32) {
    for row in 0..8 {
        for col in 0..8 {
            if let Some(piece) = &board[row][col] {
                let x = col as f32 * tile_size + tile_size*0.1;
                let y = row as f32 * tile_size + tile_size*0.8125;
                draw_text_ex(
                    piece_label(piece),
                    x, y,
                    TextParams {
                        font: Some(font),
                        font_size: (tile_size) as u16,
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
    let tile_size: f32 = WINDOW_SIZE / 8.0; 
    let font = load_ttf_font("assets/FreeSerif.ttf").await.unwrap();

    // set screen size
    macroquad::window::request_new_screen_size(WINDOW_SIZE, WINDOW_SIZE);

    let mut selected_piece: Option<Piece> = None;
    let mut selected_coords: Option<(usize, usize)> = None; 

    loop {
        
        let (x,y) = mouse_position();

        // get any input
        if macroquad::input::is_mouse_button_pressed(MouseButton::Left){
            let col: usize = (x / tile_size) as usize;
            let row: usize = (y / tile_size) as usize;
            println!("clicked at: {row}, {col}");

            if selected_piece.is_none(){
                if board[row][col].is_some(){
                    selected_piece = board[row][col];
                    selected_coords = Some((row, col));
                }
            }else{

                if get_valid_moves(&board, selected_coords.unwrap()).contains(&(row, col)){
                    move_piece_to(&mut board, selected_coords.unwrap(), (row, col));
                }else{
                    selected_piece = board[row][col];
                    selected_coords = Some((row, col));
                }

            }
        }
        if macroquad::input::is_mouse_button_down(MouseButton::Right){
            selected_piece = None;
            selected_coords = None;
        }

        // display visuals
        clear_background(WHITE);
        draw_board(tile_size);
        if selected_coords.is_some(){
            draw_moves(tile_size, &board, selected_coords.unwrap());
        }
        draw_pieces(&board, &font, tile_size);
        next_frame().await;
    }
}
