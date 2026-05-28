use crate::Board;
use crate::Side;
use crate::get_all_moves;
use crate::undo_move;
use crate::make_move;
use crate::new_board;

#[cfg(test)]
mod make_unmake_tests {
    use super::*;

    /// Walks the move tree to `depth`, asserting board equality after every make/unmake.
    /// Returns the node count (a perft result) — useful for comparing against known values.
    fn perft_with_undo_check(board: &mut Board, depth: usize) -> u64 {
        if depth == 0 {
            return 1;
        }

        let side = if board.white_to_move { Side::White } else { Side::Black };
        let moves = get_all_moves(board, side);
        let mut nodes = 0u64;

        for mv in moves {
            // snapshot the board BEFORE making the move
            let before = *board;

            let undo = make_move(board, mv.0, mv.1);
            nodes += perft_with_undo_check(board, depth - 1);
            undo_move(board, undo);

            // board must be byte-identical to the snapshot
            assert_eq!(
                *board, before,
                "Board not restored after make/unmake of move {:?} -> {:?} at depth {}",
                mv.0, mv.1, depth
            );
        }

        nodes
    }

    #[test]
    fn roundtrip_from_starting_position_depth_1() {
        let mut board = new_board();  // adjust to your constructor
        let nodes = perft_with_undo_check(&mut board, 1);
        // Starting position has 20 legal moves
        assert_eq!(nodes, 20, "Starting position should have 20 legal moves");
    }

    #[test]
    fn roundtrip_from_starting_position_depth_2() {
        let mut board = new_board();
        let nodes = perft_with_undo_check(&mut board, 2);
        assert_eq!(nodes, 400, "Depth 2 from start should be 400 nodes");
    }

    #[test]
    fn roundtrip_from_starting_position_depth_3() {
        let mut board = new_board();
        let nodes = perft_with_undo_check(&mut board, 3);
        assert_eq!(nodes, 8902, "Depth 3 from start should be 8902 nodes");
    }

    #[test]
    fn roundtrip_from_starting_position_depth_4() {
        let mut board = new_board();
        let nodes = perft_with_undo_check(&mut board, 4);
        assert_eq!(nodes, 197_281, "Depth 4 from start should be 197,281 nodes");
    }

        #[test]
    fn roundtrip_from_starting_position_depth_5() {
        let mut board = new_board();
        let nodes = perft_with_undo_check(&mut board, 5);
        assert_eq!(nodes, 4_865_609, "Depth 5 from start should be 4,865,609 nodes");
    }
    #[test]
    fn roundtrip_from_starting_position_depth_7() {
        let mut board = new_board();
        let nodes = perft_with_undo_check(&mut board, 7);
        assert_eq!(nodes, 3_195_901_860, "Depth 5 from start should be 3,195,901,860 nodes");
    }

    fn divide_perft(board: &mut Board, depth: usize) {
        let side = if board.white_to_move { Side::White } else { Side::Black };
        let moves = get_all_moves(board, side);
        let mut total = 0u64;
        for mv in moves {
            let undo = make_move(board, mv.0, mv.1);
            let nodes = perft_with_undo_check(board, depth - 1);
            undo_move(board, undo);
            println!("{:?} -> {:?}: {}", mv.0, mv.1, nodes);
            total += nodes;
        }
        println!("total: {}", total);
    }

    #[test]
    fn divide_depth_5() {
        let mut board = new_board();
        divide_perft(&mut board, 5);
    }
}