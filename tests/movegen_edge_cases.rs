use quant::board::fen::{WHITE_OO, WHITE_OOO};
use quant::board::Board;
use quant::movegen::types::{Move, MoveList};
use quant::types::{PieceType, Square};

fn perft(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut move_list = MoveList::new();
    board.generate_legal_moves(&mut move_list);

    if depth == 1 {
        return move_list.len() as u64;
    }

    let mut nodes = 0;
    for &m in move_list.as_slice() {
        let undo = board.make_move(m);
        nodes += perft(board, depth - 1);
        board.unmake_move(m, &undo);
    }
    nodes
}

#[test]
fn test_startpos_move_count() {
    let board = Board::startpos();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);
    assert_eq!(moves.len(), 20, "Startpos should have exactly 20 legal moves");
}

#[test]
fn test_perft_startpos() {
    let mut board = Board::startpos();
    assert_eq!(perft(&mut board, 1), 20);
    assert_eq!(perft(&mut board, 2), 400);
    assert_eq!(perft(&mut board, 3), 8902);
    assert_eq!(perft(&mut board, 4), 197281);
    assert_eq!(perft(&mut board, 5), 4865609);
}

#[test]
fn test_perft_kiwipete() {
    let mut board = Board::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
    ).unwrap();
    assert_eq!(perft(&mut board, 1), 48);
    assert_eq!(perft(&mut board, 2), 2039);
    assert_eq!(perft(&mut board, 3), 97862);
    assert_eq!(perft(&mut board, 4), 4085603);
}

#[test]
fn test_castle_cannot_through_check() {
    // White king on e1, rooks on a1 and h1.
    // Black rook on f8 attacks f1 (transit square for White kingside castle).
    let fen = "4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    // O-O is e1g1 (4 -> 6)
    let can_castle_kingside = moves.iter().any(|m| m.is_king_castle());
    let can_castle_queenside = moves.iter().any(|m| m.is_queen_castle());

    assert!(!can_castle_kingside, "White cannot castle kingside through f1 check");
    assert!(can_castle_queenside, "White can castle queenside");
}

#[test]
fn test_castle_cannot_out_of_check() {
    // White king on e1 in check by black rook on e8
    let fen = "4r3/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let can_castle = moves.iter().any(|m| m.is_castle());
    assert!(!can_castle, "Cannot castle out of check");
}

#[test]
fn test_castle_rights_revoked_on_rook_capture() {
    // White rook on a1, Black bishop on e5.
    // Bishop takes rook on a1 (e5a1).
    let fen = "r3k2r/8/8/4b3/8/8/8/R3K2R b KQkq - 0 1";
    let mut board = Board::from_fen(fen).unwrap();

    let b_takes_a1 = Move::new(Square::E5 as u8, Square::A1 as u8, Move::CAPTURE);
    let undo = board.make_move(b_takes_a1);

    // White should now have lost Queenside castling rights!
    assert_eq!(
        board.castling_rights & WHITE_OOO,
        0,
        "White Queenside castling right must be revoked when a1 rook is captured"
    );
    assert_ne!(
        board.castling_rights & WHITE_OO,
        0,
        "White Kingside castling right must still be intact"
    );

    board.unmake_move(b_takes_a1, &undo);
    assert_ne!(
        board.castling_rights & WHITE_OOO,
        0,
        "Queenside right restored on unmake"
    );
}

#[test]
fn test_castle_rights_revoked_on_king_move() {
    let mut board = Board::startpos();
    // 1. e4 e5 2. Ke2
    let e4 = Move::new(Square::E2 as u8, Square::E4 as u8, Move::DOUBLE_PAWN_PUSH);
    let undo1 = board.make_move(e4);
    let e5 = Move::new(Square::E7 as u8, Square::E5 as u8, Move::DOUBLE_PAWN_PUSH);
    let undo2 = board.make_move(e5);

    let ke2 = Move::new(Square::E1 as u8, Square::E2 as u8, Move::QUIET);
    let undo3 = board.make_move(ke2);

    assert_eq!(
        board.castling_rights & (WHITE_OO | WHITE_OOO),
        0,
        "White must lose all castling rights after moving king"
    );

    board.unmake_move(ke2, &undo3);
    assert_eq!(
        board.castling_rights & (WHITE_OO | WHITE_OOO),
        WHITE_OO | WHITE_OOO,
        "Castling rights restored on unmake"
    );
    board.unmake_move(e5, &undo2);
    board.unmake_move(e4, &undo1);
}

#[test]
fn test_en_passant_horizontal_pin_illegal() {
    // White King on a5, White Pawn on c5, Black Pawn on d5, Black Rook on h5.
    // FEN: 8/8/8/K1Pp3r/8/8/8/8 w - d6 0 2
    // If White plays c5xd6 e.p., both pawns on rank 5 disappear and black rook checks White king!
    let fen = "8/8/8/K1Pp3r/8/8/8/8 w - d6 0 2";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let ep_move = Move::new(Square::C5 as u8, Square::D6 as u8, Move::EN_PASSANT);
    let has_ep = moves.contains(&ep_move);
    assert!(
        !has_ep,
        "c5xd6 e.p. must be illegal due to discovered horizontal check along 5th rank"
    );
}

#[test]
fn test_en_passant_diagonal_pin_illegal() {
    // Black pawn on c4 pinned diagonally to King on e6 by White Bishop on a2.
    // White just played d2-d4, EP square is d3.
    // FEN: 8/8/4k3/8/2pP4/8/B5K1/8 b - d3 0 1
    let fen = "8/8/4k3/8/2pP4/8/B5K1/8 b - d3 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let ep_move = Move::new(Square::C4 as u8, Square::D3 as u8, Move::EN_PASSANT);
    let has_ep = moves.contains(&ep_move);
    assert!(
        !has_ep,
        "c4xd3 e.p. must be illegal due to diagonal pin to King"
    );
}

#[test]
fn test_pinned_knight_cannot_move() {
    // White King on e1, White Knight on e4, Black Rook on e8.
    let fen = "4r3/8/8/8/4N3/8/8/4K3 w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let knight_moves = moves.iter().filter(|m| m.from_sq() == Square::E4 as u8).count();
    assert_eq!(knight_moves, 0, "Pinned knight must have 0 legal moves");
}

#[test]
fn test_promotions_all_four_types() {
    // White pawn on a7, king on h1, black king on h8.
    let fen = "7k/P7/8/8/8/8/8/7K w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let a8_promos: Vec<Move> = moves
        .iter()
        .filter(|m| m.from_sq() == Square::A7 as u8 && m.to_sq() == Square::A8 as u8)
        .copied()
        .collect();

    assert_eq!(a8_promos.len(), 4, "Must have exactly 4 promotion moves (Q, R, B, N)");
    assert!(a8_promos.iter().any(|m| m.promotion_piece() == Some(PieceType::Queen)));
    assert!(a8_promos.iter().any(|m| m.promotion_piece() == Some(PieceType::Rook)));
    assert!(a8_promos.iter().any(|m| m.promotion_piece() == Some(PieceType::Bishop)));
    assert!(a8_promos.iter().any(|m| m.promotion_piece() == Some(PieceType::Knight)));
}

#[test]
fn test_make_unmake_zobrist_integrity() {
    // Test on Kiwipete: for every legal move, make_move, verify hash == calculate_zobrist(), unmake_move, verify hash matches start.
    let mut board = Board::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
    ).unwrap();

    let initial_fen = board.to_fen();
    let initial_hash = board.zobrist_hash();

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    for &m in moves.as_slice() {
        let undo = board.make_move(m);
        assert_eq!(
            board.zobrist_hash(),
            board.calculate_zobrist(),
            "Incremental Zobrist must match full calculation after move {}",
            m.to_uci()
        );
        board.unmake_move(m, &undo);
        assert_eq!(
            board.zobrist_hash(),
            initial_hash,
            "Zobrist must be restored after unmake of {}",
            m.to_uci()
        );
        assert_eq!(
            board.to_fen(),
            initial_fen,
            "FEN must match initial after unmake of {}",
            m.to_uci()
        );
    }
}

#[test]
fn test_threefold_repetition_detection() {
    let mut board = Board::startpos();

    // 1. Nf3 Nf6 2. Ng1 Ng8 (position repeated once, total 2 occurrences)
    // 3. Nf3 Nf6 4. Ng1 Ng8 (position repeated twice, total 3 occurrences -> threefold!)
    let nf3 = Move::new(Square::G1 as u8, Square::F3 as u8, Move::QUIET);
    let nf6 = Move::new(Square::G8 as u8, Square::F6 as u8, Move::QUIET);
    let ng1 = Move::new(Square::F3 as u8, Square::G1 as u8, Move::QUIET);
    let ng8 = Move::new(Square::F6 as u8, Square::G8 as u8, Move::QUIET);

    assert!(!board.is_repetition());

    board.make_move(nf3);
    board.make_move(nf6);
    board.make_move(ng1);
    board.make_move(ng8);
    assert!(!board.is_repetition(), "2 occurrences is not threefold");

    board.make_move(nf3);
    board.make_move(nf6);
    board.make_move(ng1);
    board.make_move(ng8);
    assert!(board.is_repetition(), "3 occurrences must trigger is_repetition()");
}

#[test]
fn test_epd_tricky_positions() {
    // 1. castle-rights
    let mut b1 = Board::from_fen("r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1").unwrap();
    assert_eq!(perft(&mut b1, 1), 26);
    assert_eq!(perft(&mut b1, 2), 1141);
    assert_eq!(perft(&mut b1, 3), 27826);
    assert_eq!(perft(&mut b1, 4), 1274206);

    // 2. castling-prevented
    let mut b2 = Board::from_fen("r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1").unwrap();
    assert_eq!(perft(&mut b2, 1), 44);
    assert_eq!(perft(&mut b2, 2), 1494);
    assert_eq!(perft(&mut b2, 3), 50509);

    // 3. promote-out-of-check
    let mut b3 = Board::from_fen("2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1").unwrap();
    assert_eq!(perft(&mut b3, 1), 11);
    assert_eq!(perft(&mut b3, 2), 133);
    assert_eq!(perft(&mut b3, 3), 1442);

    // 4. self-stalemate
    let mut b4 = Board::from_fen("K1k5/8/P7/8/8/8/8/8 w - - 0 1").unwrap();
    assert_eq!(perft(&mut b4, 1), 2);
    assert_eq!(perft(&mut b4, 2), 6);
    assert_eq!(perft(&mut b4, 3), 13);

    // 5. stalemate-and-checkmate-1
    let mut b5 = Board::from_fen("8/k1P5/8/1K6/8/8/8/8 w - - 0 1").unwrap();
    assert_eq!(perft(&mut b5, 1), 10);
    assert_eq!(perft(&mut b5, 2), 25);
    assert_eq!(perft(&mut b5, 3), 268);
}

