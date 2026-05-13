use macroquad::prelude::*;

const WINDOW_SIZE: f32 = 500.0;

#[derive(Clone, Copy, PartialEq)]
enum PieceType {
    Pawn, Knight, Bishop, Rook, Queen, King
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    White, Black
}

#[derive(Clone, Copy)]
struct Piece {
    piece_type: PieceType,
    color: Side
}

type Board = [[Option<Piece>; 8]; 8];

fn new_board() -> Board {
    let mut board = [[None; 8]; 8];

    // Helper closure to place a piece
    let w = |pt| Some(Piece { piece_type: pt, color: Side::White });
    let b = |pt| Some(Piece { piece_type: pt, color: Side::Black });

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
    let board = new_board();
    let tile_size: f32 = WINDOW_SIZE / 8.0; 
    let font = load_ttf_font("assets/FreeSerif.ttf").await.unwrap();

    // set screen size
    macroquad::window::request_new_screen_size(WINDOW_SIZE, WINDOW_SIZE);

    loop {
        clear_background(WHITE);
        draw_board(tile_size);
        draw_pieces(&board, &font, tile_size);
        next_frame().await;
    }
}
