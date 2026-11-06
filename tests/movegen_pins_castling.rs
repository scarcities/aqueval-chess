use aqueval::board::fen::{BLACK_OO, BLACK_OOO, WHITE_OO, WHITE_OOO};
use aqueval::board::Board;
use aqueval::movegen::types::{Move, MoveList};
use aqueval::types::Square;

// Helper: check if a specific move exists in MoveList
fn has_move(moves: &MoveList, from: Square, to: Square) -> bool {
    moves.iter().any(|m| m.from_sq() == from as u8 && m.to_sq() == to as u8)
}

// ----------------------------------------------------------------------------
// 1. EN PASSANT HORIZONTAL PIN
// ----------------------------------------------------------------------------

#[test]
fn test_ep_horizontal_pin_white_king_rook() {
    // White King on e5, White Pawn on d5, Black Pawn on c5, Black Rook on a5.
    // Black just played c7-c5 (EP square is c6).
    // If White plays d5xc6 e.p., both pawns vacate rank 5 and Black Rook on a5 gives check!
    let fen = "7k/8/8/r1pPK3/8/8/8/8 w - c6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !has_move(&moves, Square::D5, Square::C6),
        "d5xc6 e.p. must be illegal due to discovered horizontal rook check"
    );
}

#[test]
fn test_ep_horizontal_pin_white_king_queen() {
    // White King on e5, White Pawn on d5, Black Pawn on c5, Black Queen on a5.
    let fen = "7k/8/8/q1pPK3/8/8/8/8 w - c6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !has_move(&moves, Square::D5, Square::C6),
        "d5xc6 e.p. must be illegal due to discovered horizontal queen check"
    );
}

#[test]
fn test_ep_horizontal_pin_black_king_rook() {
    // Black King on e4, Black Pawn on d4, White Pawn on c4, White Rook on a4.
    // White just played c2-c4 (EP square is c3).
    // If Black plays d4xc3 e.p., both pawns vacate rank 4 and White Rook on a4 checks Black King!
    let fen = "8/8/8/8/R1Ppk3/8/8/7K b - c3 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !has_move(&moves, Square::D4, Square::C3),
        "d4xc3 e.p. must be illegal for Black due to discovered horizontal check"
    );
}

#[test]
fn test_ep_horizontal_pin_king_on_opposite_side() {
    // White King on h5, White Pawn on f5, Black Pawn on e5, Black Rook on a5.
    // All on rank 5. Black just played e7-e5.
    // If f5xe6, both pawns vacate rank 5, Ra5 checks Kh5!
    let fen = "k7/8/8/r3pP1K/8/8/8/8 w - e6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !has_move(&moves, Square::F5, Square::E6),
        "f5xe6 e.p. must be illegal when king is on opposite side of pawns"
    );
}

#[test]
fn test_ep_horizontal_safe_when_king_not_on_rank() {
    // White King on h8 (not on rank 5!).
    // White pawn on e5, Black pawn on f5 (just played f7-f5, EP square f6). Black Rook on a5.
    // e5xf6 e.p. leaves rank 5, but White King is safe on h8.
    let fen = "7K/8/8/r3Pp2/8/8/8/k7 w - f6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        has_move(&moves, Square::E5, Square::F6),
        "e5xf6 e.p. must be legal when king is NOT on rank 5"
    );
}

// ----------------------------------------------------------------------------
// 2. EN PASSANT DIAGONAL PIN
// ----------------------------------------------------------------------------

#[test]
fn test_ep_diagonal_pin_moving_pawn() {
    // White King on a1, Black Bishop on h8, White Pawn on e5.
    // Diagonal: a1 - b2 - c3 - d4 - e5 - f6 - g7 - h8.
    // Black plays d7-d5 (EP square d6).
    // White pawn on e5 is pinned diagonally to King on a1 by Bh8.
    // If White plays e5xd6 e.p., pawn leaves diagonal, Bh8 checks Ka1!
    let fen = "7b/8/8/3pP3/8/8/8/K6k w - d6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !has_move(&moves, Square::E5, Square::D6),
        "e5xd6 e.p. must be illegal because moving pawn is diagonally pinned"
    );
}

#[test]
fn test_ep_diagonal_pin_captured_pawn_shielding_king() {
    // White King on a1, Black Bishop on h8.
    // Diagonal: a1 - b2 - c3 - d4 - e5 - f6 - g7 - h8.
    // Black pawn on e5 (just played e7-e5, EP square e6).
    // White pawn on d5 (rank 5, file d).
    // Notice: White pawn d5 is NOT on diagonal a1-h8.
    // The CAPTURED pawn e5 WAS shielding White King on a1 from Bh8!
    // If White plays d5xe6 e.p.:
    // White pawn moves d5 -> e6 (not on diagonal a1-h8).
    // Black pawn on e5 is removed.
    // Square e5 is now empty -> Bh8 checks Ka1!
    let fen = "7b/8/8/3Pp3/8/8/8/K6k w - e6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !has_move(&moves, Square::D5, Square::E6),
        "d5xe6 e.p. must be illegal because captured pawn was shielding King from diagonal check"
    );
}

#[test]
fn test_ep_diagonal_pin_black_captured_pawn_shielding_king() {
    // Symmetric for Black:
    // Black King on a8, White Bishop on h1.
    // Diagonal: a8 - b7 - c6 - d5 - e4 - f3 - g2 - h1.
    // White pawn just played e2-e4 (EP square e3).
    // Black pawn on d4.
    // White pawn on e4 was shielding Black King on a8 from White Bishop on h1.
    // If Black plays d4xe3 e.p., e4 is removed and Bh1 checks Ka8!
    let fen = "k7/8/8/8/3pP3/8/8/K6B b - e3 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !has_move(&moves, Square::D4, Square::E3),
        "d4xe3 e.p. must be illegal because captured pawn was shielding Black King from Bh1"
    );
}

// ----------------------------------------------------------------------------
// 3. CASTLING THROUGH, OUT OF, AND INTO CHECK
// ----------------------------------------------------------------------------

#[test]
fn test_castle_out_of_check_illegal() {
    // White king on e1 in check by Black Queen on e7.
    let fen = "r3k2r/4q3/8/8/8/8/8/R3K2R w KQkq - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        !moves.iter().any(|m| m.is_castle()),
        "Cannot castle out of check (Kingside or Queenside)"
    );
}

#[test]
fn test_castle_through_check_illegal_kingside_and_queenside() {
    // White King on e1.
    // Black Rook on f8 attacks f1 (transit square for White O-O).
    // Black Bishop on a6 attacks d1 (transit square for White O-O-O: a6-b5-c4-d3-e2? No, a6 attacks f1? Let's use direct lines).
    // Let's test kingside through check:
    let fen_k = "4k2r/8/8/8/8/8/5r2/R3K2R w KQk - 0 1"; // Rf2 attacks f1? Rf2 on 2nd rank attacks f1 directly!
    let board_k = Board::from_fen(fen_k).unwrap();
    let mut moves_k = MoveList::new();
    board_k.generate_legal_moves(&mut moves_k);

    assert!(
        !moves_k.iter().any(|m| m.is_king_castle()),
        "Cannot castle Kingside through f1 check"
    );
    assert!(
        moves_k.iter().any(|m| m.is_queen_castle()),
        "Queenside castle should be valid"
    );

    // Let's test queenside through check:
    // Black rook on d8 attacks d1.
    let fen_q = "3rk2r/8/8/8/8/8/8/R3K2R w KQk - 0 1";
    let board_q = Board::from_fen(fen_q).unwrap();
    let mut moves_q = MoveList::new();
    board_q.generate_legal_moves(&mut moves_q);

    assert!(
        !moves_q.iter().any(|m| m.is_queen_castle()),
        "Cannot castle Queenside through d1 check"
    );
    assert!(
        moves_q.iter().any(|m| m.is_king_castle()),
        "Kingside castle should be valid"
    );
}

#[test]
fn test_castle_into_check_illegal() {
    // Black rook on g8 attacks g1 (White king castle destination).
    let fen_k = "4k1r1/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    let board_k = Board::from_fen(fen_k).unwrap();
    let mut moves_k = MoveList::new();
    board_k.generate_legal_moves(&mut moves_k);

    assert!(
        !moves_k.iter().any(|m| m.is_king_castle()),
        "Cannot castle Kingside into g1 check"
    );

    // Black rook on c8 attacks c1 (White queen castle destination).
    let fen_q = "2r1k3/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    let board_q = Board::from_fen(fen_q).unwrap();
    let mut moves_q = MoveList::new();
    board_q.generate_legal_moves(&mut moves_q);

    assert!(
        !moves_q.iter().any(|m| m.is_queen_castle()),
        "Cannot castle Queenside into c1 check"
    );
}

#[test]
fn test_castle_queenside_b1_attacked_is_legal() {
    // In FIDE chess, b1 can be attacked by enemy piece, and O-O-O is STILL LEGAL!
    // Black rook on b8 attacks b1 (rook transit square, NOT king transit square).
    let fen = "1r2k3/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        moves.iter().any(|m| m.is_queen_castle()),
        "Queenside castling is legal even when b1 is attacked"
    );
}

#[test]
fn test_castle_queenside_b8_attacked_is_legal_black() {
    // Symmetric for Black: White rook on b1 attacks b8. Black O-O-O is still legal.
    let fen = "r3k2r/8/8/8/8/8/8/1R2K2R b Kkq - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(
        moves.iter().any(|m| m.is_queen_castle()),
        "Black Queenside castling is legal even when b8 is attacked"
    );
}

// ----------------------------------------------------------------------------
// 4. CASTLING RIGHTS REVOCATION ON ROOK CAPTURE ON A1, H1, A8, H8
// ----------------------------------------------------------------------------

#[test]
fn test_castle_rights_revoked_on_all_four_rook_captures() {
    // Test a1 capture: Black Bishop captures White Rook on a1
    {
        let fen = "r3k2r/8/8/4b3/8/8/8/R3K2R b KQkq - 0 1";
        let mut board = Board::from_fen(fen).unwrap();
        let m = Move::new(Square::E5 as u8, Square::A1 as u8, Move::CAPTURE);
        let undo = board.make_move(m);

        assert_eq!(board.castling_rights & WHITE_OOO, 0, "White Q-castle revoked on a1 capture");
        assert_ne!(board.castling_rights & WHITE_OO, 0, "White K-castle intact");
        assert_ne!(board.castling_rights & BLACK_OO, 0, "Black K-castle intact");
        assert_ne!(board.castling_rights & BLACK_OOO, 0, "Black Q-castle intact");

        board.unmake_move(m, &undo);
        assert_ne!(board.castling_rights & WHITE_OOO, 0, "White Q-castle restored on unmake");
    }

    // Test h1 capture: Black Bishop captures White Rook on h1
    {
        let fen = "r3k2r/8/8/3b4/8/8/8/R3K2R b KQkq - 0 1";
        let mut board = Board::from_fen(fen).unwrap();
        let m = Move::new(Square::D5 as u8, Square::H1 as u8, Move::CAPTURE);
        let undo = board.make_move(m);

        assert_eq!(board.castling_rights & WHITE_OO, 0, "White K-castle revoked on h1 capture");
        assert_ne!(board.castling_rights & WHITE_OOO, 0, "White Q-castle intact");

        board.unmake_move(m, &undo);
        assert_ne!(board.castling_rights & WHITE_OO, 0, "White K-castle restored on unmake");
    }

    // Test a8 capture: White Bishop captures Black Rook on a8
    {
        let fen = "r3k2r/8/8/8/8/4B3/8/R3K2R w KQkq - 0 1";
        let mut board = Board::from_fen(fen).unwrap();
        let m = Move::new(Square::E3 as u8, Square::A8 as u8, Move::CAPTURE);
        let undo = board.make_move(m);

        assert_eq!(board.castling_rights & BLACK_OOO, 0, "Black Q-castle revoked on a8 capture");
        assert_ne!(board.castling_rights & BLACK_OO, 0, "Black K-castle intact");

        board.unmake_move(m, &undo);
        assert_ne!(board.castling_rights & BLACK_OOO, 0, "Black Q-castle restored on unmake");
    }

    // Test h8 capture: White Bishop captures Black Rook on h8
    {
        let fen = "r3k2r/8/8/8/8/3B4/8/R3K2R w KQkq - 0 1";
        let mut board = Board::from_fen(fen).unwrap();
        let m = Move::new(Square::D3 as u8, Square::H8 as u8, Move::CAPTURE);
        let undo = board.make_move(m);

        assert_eq!(board.castling_rights & BLACK_OO, 0, "Black K-castle revoked on h8 capture");
        assert_ne!(board.castling_rights & BLACK_OOO, 0, "Black Q-castle intact");

        board.unmake_move(m, &undo);
        assert_ne!(board.castling_rights & BLACK_OO, 0, "Black K-castle restored on unmake");
    }
}

// ----------------------------------------------------------------------------
// 5. PINNED PIECES CAN ONLY MOVE ALONG THEIR PIN RAY
// ----------------------------------------------------------------------------

#[test]
fn test_pinned_rook_can_only_move_along_pin_file() {
    // White King on e1, White Rook on e4, Black Rook on e8.
    // White Rook is pinned along file e.
    // Legal rook moves should only be along e-file: e2, e3, e5, e6, e7, e8 (capture).
    // Moving along rank 4 (a4, b4, c4, d4, f4, g4, h4) must be illegal!
    let fen = "k3r3/8/8/8/4R3/8/8/4K3 w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let rook_moves: Vec<Move> = moves
        .iter()
        .filter(|m| m.from_sq() == Square::E4 as u8)
        .copied()
        .collect();

    // Along e-file: e2, e3, e5, e6, e7, e8 (6 squares)
    assert_eq!(rook_moves.len(), 6, "Pinned rook can only move to 6 squares on e-file");
    for m in &rook_moves {
        assert_eq!(
            m.to_sq() % 8,
            Square::E4 as u8 % 8,
            "Target square must be on e-file"
        );
    }
}

#[test]
fn test_pinned_bishop_can_only_move_along_diagonal() {
    // White King on a1, White Bishop on c3, Black Queen on e5.
    // Bishop on c3 is pinned along diagonal a1-e5.
    // Bishop can move to b2 or capture e5.
    // Bishop cannot move to b4, d2, etc.!
    let fen = "7k/8/8/4q3/8/2B5/8/K7 w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let bishop_moves: Vec<Move> = moves
        .iter()
        .filter(|m| m.from_sq() == Square::C3 as u8)
        .copied()
        .collect();

    assert_eq!(bishop_moves.len(), 3, "Pinned bishop can only move to b2, d4, or e5");
    assert!(has_move(&moves, Square::C3, Square::B2));
    assert!(has_move(&moves, Square::C3, Square::D4));
    assert!(has_move(&moves, Square::C3, Square::E5));
}

#[test]
fn test_pinned_queen_along_rank_cannot_move_along_file_or_diagonal() {
    // White King on a4, White Queen on d4, Black Rook on h4.
    // Queen is pinned along rank 4.
    // Queen can only move along rank 4: b4, c4, e4, f4, g4, h4 (capture).
    let fen = "k7/8/8/8/K2Q3r/8/8/8 w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let queen_moves: Vec<Move> = moves
        .iter()
        .filter(|m| m.from_sq() == Square::D4 as u8)
        .copied()
        .collect();

    assert_eq!(queen_moves.len(), 6, "Rank-pinned queen can only move along rank 4");
    for m in &queen_moves {
        assert_eq!(m.to_sq() / 8, 3, "All moves must stay on rank 4 (0-indexed rank 3)");
    }
}

#[test]
fn test_pinned_pawn_along_file_can_push_but_cannot_capture() {
    // White King on e1, White Pawn on e2, Black Rook on e8.
    // Black pawn on d3 (attacks c2 and e2, does NOT check e1).
    // Pawn can push e2-e3 or e2-e4 along pin file, but CANNOT capture on d3!
    let fen = "k3r3/8/8/8/8/3p4/4P3/4K3 w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(has_move(&moves, Square::E2, Square::E3), "e2-e3 is legal along pin file");
    assert!(has_move(&moves, Square::E2, Square::E4), "e2-e4 is legal along pin file");
    assert!(!has_move(&moves, Square::E2, Square::D3), "e2xd3 capture off pin file is illegal");
}

#[test]
fn test_pinned_pawn_diagonally_can_capture_pinner_but_cannot_push() {
    // White King on a1, White Pawn on b2, Black Bishop on c3.
    // Pawn can capture b2xc3, but CANNOT push b2-b3 or b2-b4!
    let fen = "7k/8/8/8/8/2b5/1P6/K7 w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(has_move(&moves, Square::B2, Square::C3), "b2xc3 captures pinner along pin ray");
    assert!(!has_move(&moves, Square::B2, Square::B3), "b2-b3 leaves pin ray");
    assert!(!has_move(&moves, Square::B2, Square::B4), "b2-b4 leaves pin ray");
}

// ----------------------------------------------------------------------------
// 6. DOUBLE CHECK FORCES KING MOVE ONLY
// ----------------------------------------------------------------------------

#[test]
fn test_double_check_forces_king_move_only() {
    // White King on e1.
    // Black Rook on e8 gives check along e-file.
    // Black Bishop on a5 gives check along diagonal a5-e1.
    // White has Queen on d1, Rook on h1, Knight on c3, etc.
    // Double check! ONLY King moves are legal.
    let fen = "4r1k1/8/8/b7/8/8/8/R2QK2R w KQ - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(!moves.is_empty(), "King should have at least 1 legal move");
    for m in moves.iter() {
        assert_eq!(
            m.from_sq(),
            Square::E1 as u8,
            "In double check, EVERY legal move must be a King move!"
        );
    }

    // King cannot move to e2 (still in check by e8 rook)
    assert!(!has_move(&moves, Square::E1, Square::E2));
    // King cannot move to d2 (still in check by a5 bishop: a5-b4-c3-d2-e1)
    assert!(!has_move(&moves, Square::E1, Square::D2));
    // King can move to f1 (safe) or f2 (safe)
    assert!(has_move(&moves, Square::E1, Square::F1));
    assert!(has_move(&moves, Square::E1, Square::F2));
}

#[test]
fn test_double_check_king_cannot_capture_defended_checker() {
    // White King on e4.
    // Black Knight on d6 gives check, defended by Black Pawn on c7.
    // Black Rook on e8 gives check along e-file.
    // Double check!
    // King on e4 attacks Knight on d6, but d6 is defended by pawn on c7!
    // King cannot capture d6!
    let fen = "k3r3/2p5/3n4/8/4K3/8/8/8 w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    for m in moves.iter() {
        assert_eq!(m.from_sq(), Square::E4 as u8, "Only king moves");
    }
    assert!(
        !has_move(&moves, Square::E4, Square::D6),
        "King cannot capture protected checker d6 in double check"
    );
}

// ----------------------------------------------------------------------------
// 7. EN PASSANT CHECK EVASIONS & CHECKMATE / STALEMATE INTEGRITY
// ----------------------------------------------------------------------------

#[test]
fn test_ep_can_capture_checking_pawn() {
    // White King on e4, Black pawn just played d7-d5, giving check to King on e4!
    // White pawn on e5.
    // e5xd6 e.p. captures the checking pawn on d5 and evades check!
    let fen = "k7/8/8/3pP3/4K3/8/8/8 w - d6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(board.is_in_check(), "White King should be in check from d5 pawn");
    assert!(
        has_move(&moves, Square::E5, Square::D6),
        "e5xd6 e.p. must be legal to capture checking pawn"
    );
}

#[test]
fn test_ep_cannot_be_played_if_it_does_not_resolve_check() {
    // White King on f1 in check by Black Rook on f8 (f-file is open).
    // Black just played d7-d5 (EP square d6). White pawn on e5.
    // e5xd6 e.p. does NOT resolve check from f8 rook!
    // Therefore e5xd6 e.p. must be illegal!
    let fen = "k4r2/8/8/3pP3/8/8/8/5K2 w - d6 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(board.is_in_check());
    assert!(
        !has_move(&moves, Square::E5, Square::D6),
        "e5xd6 e.p. cannot be played when it doesn't resolve check from f8"
    );
}

#[test]
fn test_fools_mate_checkmate_zero_moves() {
    // 1. f3 e5 2. g4 Qh4# (Fool's Mate)
    let fen = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(board.is_in_check(), "Must be in check");
    assert_eq!(moves.len(), 0, "Checkmate must have exactly 0 legal moves");
}

#[test]
fn test_stalemate_zero_moves_not_in_check() {
    // King on a8, Queen on b6, White King on h1.
    // Black to move: Ka8 is not in check, but has 0 legal moves.
    let fen = "k7/8/1Q6/8/8/8/8/7K b - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(!board.is_in_check(), "Must NOT be in check");
    assert_eq!(moves.len(), 0, "Stalemate must have exactly 0 legal moves");
}

#[test]
fn test_castle_rights_not_regained_after_king_moves_back() {
    // Startpos: 1. e4 e5 2. Ke2 Ke7 3. Ke1 Ke8
    let mut board = Board::startpos();
    let e4 = Move::new(Square::E2 as u8, Square::E4 as u8, Move::DOUBLE_PAWN_PUSH);
    board.make_move(e4);
    let e5 = Move::new(Square::E7 as u8, Square::E5 as u8, Move::DOUBLE_PAWN_PUSH);
    board.make_move(e5);

    let ke2 = Move::new(Square::E1 as u8, Square::E2 as u8, Move::QUIET);
    board.make_move(ke2);
    let ke7 = Move::new(Square::E8 as u8, Square::E7 as u8, Move::QUIET);
    board.make_move(ke7);

    let ke1 = Move::new(Square::E2 as u8, Square::E1 as u8, Move::QUIET);
    board.make_move(ke1);
    let ke8 = Move::new(Square::E7 as u8, Square::E8 as u8, Move::QUIET);
    board.make_move(ke8);

    assert_eq!(
        board.castling_rights,
        0,
        "All castling rights must remain 0 after kings moved away and back"
    );

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);
    assert!(
        !moves.iter().any(|m| m.is_castle()),
        "Cannot castle after king returned to original square"
    );
}

#[test]
fn test_castle_rights_not_regained_after_rook_moves_back() {
    // 1. h4 d5 2. Rh3 Bxh3 3. gxh3 ...
    // Test moving h1 rook to h2 and back to h1
    // Board: White King on e1, Rook on h1, a1
    let fen = "4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    let mut board = Board::from_fen(fen).unwrap();

    // Move Rh1 to h2
    let rh2 = Move::new(Square::H1 as u8, Square::H2 as u8, Move::QUIET);
    board.make_move(rh2);
    assert_eq!(board.castling_rights & WHITE_OO, 0, "White Kingside castle revoked");
    assert_ne!(board.castling_rights & WHITE_OOO, 0, "White Queenside castle still intact");

    // Dummy black move: Ke8-e7
    let ke7 = Move::new(Square::E8 as u8, Square::E7 as u8, Move::QUIET);
    board.make_move(ke7);

    // Move Rh2 back to h1
    let rh1 = Move::new(Square::H2 as u8, Square::H1 as u8, Move::QUIET);
    board.make_move(rh1);

    assert_eq!(
        board.castling_rights & WHITE_OO,
        0,
        "White Kingside castle right MUST NOT be regained when Rook moves back to h1"
    );
}

