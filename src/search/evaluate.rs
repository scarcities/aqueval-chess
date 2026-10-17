use crate::board::Board;
use crate::types::PieceType;

/// Material piece values in centipawns.
pub const PAWN_VALUE: i32 = 100;
pub const KNIGHT_VALUE: i32 = 320;
pub const BISHOP_VALUE: i32 = 330;
pub const ROOK_VALUE: i32 = 500;
pub const QUEEN_VALUE: i32 = 900;

/// Returns the material value of a piece type in centipawns.
#[inline(always)]
pub fn piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => PAWN_VALUE,
        PieceType::Knight => KNIGHT_VALUE,
        PieceType::Bishop => BISHOP_VALUE,
        PieceType::Rook => ROOK_VALUE,
        PieceType::Queen => QUEEN_VALUE,
        PieceType::King => 0,
    }
}

/// Evaluates board position purely based on material count,
/// relative to the active side to move (positive if active side is ahead).
pub fn evaluate(board: &Board) -> i32 {
    let us = board.side_to_move;
    let them = !us;
    let us_occ = board.colors[us.index()];
    let them_occ = board.colors[them.index()];

    let pawns = (board.pieces[PieceType::Pawn.index()] & us_occ).count() as i32
        - (board.pieces[PieceType::Pawn.index()] & them_occ).count() as i32;
    let knights = (board.pieces[PieceType::Knight.index()] & us_occ).count() as i32
        - (board.pieces[PieceType::Knight.index()] & them_occ).count() as i32;
    let bishops = (board.pieces[PieceType::Bishop.index()] & us_occ).count() as i32
        - (board.pieces[PieceType::Bishop.index()] & them_occ).count() as i32;
    let rooks = (board.pieces[PieceType::Rook.index()] & us_occ).count() as i32
        - (board.pieces[PieceType::Rook.index()] & them_occ).count() as i32;
    let queens = (board.pieces[PieceType::Queen.index()] & us_occ).count() as i32
        - (board.pieces[PieceType::Queen.index()] & them_occ).count() as i32;

    pawns * PAWN_VALUE
        + knights * KNIGHT_VALUE
        + bishops * BISHOP_VALUE
        + rooks * ROOK_VALUE
        + queens * QUEEN_VALUE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_startpos_is_zero() {
        let board = Board::startpos();
        assert_eq!(evaluate(&board), 0);
    }

    #[test]
    fn test_eval_material_advantage() {
        // White to move with an extra queen
        let board_white_plus_queen =
            Board::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        // Black is missing queen (d8)
        assert_eq!(evaluate(&board_white_plus_queen), QUEEN_VALUE);

        // Black to move with an extra knight
        let board_black_plus_knight =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R1BQKBNR b KQkq - 0 1").unwrap();
        // White missing knight (b1)
        assert_eq!(evaluate(&board_black_plus_knight), KNIGHT_VALUE);
    }
}
