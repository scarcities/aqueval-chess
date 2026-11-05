use aqueval::board::Board;
use aqueval::movegen::types::{Move, MoveList};
use aqueval::types::{Bitboard, Color, PieceType};
use shakmaty::{CastlingMode, Chess, Position};
use std::collections::HashSet;

// Deterministic PRNG for reproducible test sequences
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn gen_range(&mut self, upper: usize) -> usize {
        (self.next_u64() as usize) % upper
    }
}

fn verify_board_internal_invariants(board: &Board, msg_ctx: &str) {
    // 1. Incremental Zobrist hash equals scratch calculation
    assert_eq!(
        board.zobrist_hash(),
        board.calculate_zobrist(),
        "[{}] Incremental Zobrist hash desynchronized from scratch calculation",
        msg_ctx
    );

    // 2. Occupancy consistency
    let occ = board.occupied();
    assert_eq!(
        occ,
        board.colors[0] | board.colors[1],
        "[{}] Board occupied() mismatch with union of colors",
        msg_ctx
    );

    // 3. Color bitboards disjoint
    assert_eq!(
        board.colors[0] & board.colors[1],
        Bitboard::EMPTY,
        "[{}] White and Black color bitboards overlap",
        msg_ctx
    );

    // 4. Piece bitboards disjoint and sum to occupancy
    let mut piece_union = Bitboard::EMPTY;
    for pt1 in 0..6 {
        assert_eq!(
            piece_union & board.pieces[pt1],
            Bitboard::EMPTY,
            "[{}] Piece type bitboard {} overlaps with prior piece bitboards",
            msg_ctx,
            pt1
        );
        piece_union |= board.pieces[pt1];
    }
    assert_eq!(
        piece_union, occ,
        "[{}] Union of piece bitboards does not equal occupancy",
        msg_ctx
    );

    // 5. Piece at array consistency
    for sq in 0..64u8 {
        match board.piece_at[sq as usize] {
            Some(piece) => {
                assert!(
                    board.pieces[piece.piece_type.index()].contains(sq),
                    "[{}] piece_at[{}]={:?} not in pieces[{:?}]",
                    msg_ctx,
                    sq,
                    piece,
                    piece.piece_type
                );
                assert!(
                    board.colors[piece.color.index()].contains(sq),
                    "[{}] piece_at[{}]={:?} not in colors[{:?}]",
                    msg_ctx,
                    sq,
                    piece,
                    piece.color
                );
            }
            None => {
                assert!(
                    !occ.contains(sq),
                    "[{}] piece_at[{}] is None but occupied contains sq",
                    msg_ctx,
                    sq
                );
            }
        }
    }

    // 6. King square consistency
    let white_king = board.pieces[PieceType::King.index()] & board.colors[Color::White.index()];
    let black_king = board.pieces[PieceType::King.index()] & board.colors[Color::Black.index()];
    assert_eq!(
        white_king.count(),
        1,
        "[{}] Expected exactly 1 white king",
        msg_ctx
    );
    assert_eq!(
        black_king.count(),
        1,
        "[{}] Expected exactly 1 black king",
        msg_ctx
    );
    assert_eq!(
        board.king_square(Color::White),
        white_king.lsb(),
        "[{}] king_square(White) mismatch",
        msg_ctx
    );
    assert_eq!(
        board.king_square(Color::Black),
        black_king.lsb(),
        "[{}] king_square(Black) mismatch",
        msg_ctx
    );

    // 7. FEN round-trip consistency
    let fen = board.to_fen();
    let reloaded = Board::from_fen(&fen).unwrap_or_else(|e| {
        panic!("[{}] Failed to re-parse generated FEN '{}': {:?}", msg_ctx, fen, e)
    });
    assert_eq!(reloaded.pieces, board.pieces, "[{}] FEN pieces mismatch", msg_ctx);
    assert_eq!(reloaded.colors, board.colors, "[{}] FEN colors mismatch", msg_ctx);
    assert_eq!(reloaded.side_to_move, board.side_to_move, "[{}] FEN side_to_move mismatch", msg_ctx);
    assert_eq!(reloaded.castling_rights, board.castling_rights, "[{}] FEN castling_rights mismatch", msg_ctx);
    assert_eq!(reloaded.ep_square, board.ep_square, "[{}] FEN ep_square mismatch", msg_ctx);
    assert_eq!(reloaded.halfmove_clock, board.halfmove_clock, "[{}] FEN halfmove_clock mismatch", msg_ctx);
    assert_eq!(reloaded.fullmove_number, board.fullmove_number, "[{}] FEN fullmove_number mismatch", msg_ctx);
    assert_eq!(reloaded.zobrist_hash, board.zobrist_hash, "[{}] FEN zobrist_hash mismatch", msg_ctx);
}

#[test]
fn test_adversarial_deep_random_walk_make_unmake_invariants() {
    let test_fens = [
        ("startpos", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("kiwipete", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"),
        ("pos3", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
        ("pos4", "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"),
        ("pos5", "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"),
        ("pos6", "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10"),
    ];

    let mut rng = SimpleRng::new(0xDEADBEEFCAFE1234);
    let mut total_branches_tested = 0usize;

    for (name, fen) in test_fens {
        let mut board = Board::from_fen(fen).expect("valid FEN");
        verify_board_internal_invariants(&board, &format!("{}_init", name));

        for ply in 0..75 {
            let mut moves = MoveList::new();
            board.generate_legal_moves(&mut moves);

            if moves.is_empty() {
                // Checkmate or stalemate reached
                break;
            }

            let initial_fen = board.to_fen();
            let initial_hash = board.zobrist_hash();
            let initial_clone = board.clone();

            // Adversarially test EVERY legal branch at this ply
            for &m in moves.as_slice() {
                total_branches_tested += 1;
                let undo = board.make_move(m);

                verify_board_internal_invariants(
                    &board,
                    &format!("{}_ply{}_{}", name, ply, m.to_uci()),
                );

                board.unmake_move(m, &undo);

                // Verify exact state recovery
                assert_eq!(
                    board, initial_clone,
                    "[{}_ply{}_{}] Board state not restored after unmake",
                    name, ply, m.to_uci()
                );
                assert_eq!(
                    board.to_fen(),
                    initial_fen,
                    "[{}_ply{}_{}] FEN not restored after unmake",
                    name, ply, m.to_uci()
                );
                assert_eq!(
                    board.zobrist_hash(),
                    initial_hash,
                    "[{}_ply{}_{}] Zobrist hash not restored after unmake",
                    name, ply, m.to_uci()
                );
            }

            // Advance random walk along trunk
            let chosen_move = moves.as_slice()[rng.gen_range(moves.len())];
            board.make_move(chosen_move);
        }
    }

    println!(
        "Deep random walk completed successfully across {} branch make/unmake tests",
        total_branches_tested
    );
    assert!(
        total_branches_tested > 5000,
        "Expected at least 5000 branch validations, got {}",
        total_branches_tested
    );
}

#[test]
fn test_adversarial_deep_unmake_rewind() {
    let mut rng = SimpleRng::new(0x4299BEEF01234567);
    let mut board = Board::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    )
    .unwrap();

    let initial_board = board.clone();
    let mut history_stack = Vec::new();

    // Play 60 consecutive moves deep
    for ply in 0..60 {
        let mut moves = MoveList::new();
        board.generate_legal_moves(&mut moves);
        if moves.is_empty() {
            break;
        }

        let chosen = moves.as_slice()[rng.gen_range(moves.len())];
        let pre_board = board.clone();
        let undo = board.make_move(chosen);
        history_stack.push((chosen, undo, pre_board));

        verify_board_internal_invariants(&board, &format!("rewind_forward_ply{}", ply));
    }

    let actual_depth = history_stack.len();
    assert!(actual_depth >= 40, "Expected deep tree walk of at least 40 plies");

    // Rewind all moves in reverse order
    for (step, (m, undo, expected_board)) in history_stack.into_iter().rev().enumerate() {
        board.unmake_move(m, &undo);
        assert_eq!(
            board, expected_board,
            "Board mismatch during rewind at step {}",
            step
        );
        verify_board_internal_invariants(&board, &format!("rewind_backward_step{}", step));
    }

    assert_eq!(
        board, initial_board,
        "Board must return to exact initial state after full rewind"
    );
}

#[test]
fn test_adversarial_threefold_repetition_exhaustive() {
    let mut board = Board::startpos();

    // 1. Startpos repetition:
    // Moves: 1. Nf3 Nf6 2. Ng1 Ng8 (occ 2) 3. Nf3 Nf6 4. Ng1 Ng8 (occ 3 -> Threefold!)
    let nf3 = Move::new(aqueval::types::Square::G1 as u8, aqueval::types::Square::F3 as u8, Move::QUIET);
    let nf6 = Move::new(aqueval::types::Square::G8 as u8, aqueval::types::Square::F6 as u8, Move::QUIET);
    let ng1 = Move::new(aqueval::types::Square::F3 as u8, aqueval::types::Square::G1 as u8, Move::QUIET);
    let ng8 = Move::new(aqueval::types::Square::F6 as u8, aqueval::types::Square::G8 as u8, Move::QUIET);

    assert!(!board.is_repetition(), "Start position is 1st occurrence");

    board.make_move(nf3);
    assert!(!board.is_repetition());
    board.make_move(nf6);
    assert!(!board.is_repetition());
    board.make_move(ng1);
    assert!(!board.is_repetition());
    board.make_move(ng8);
    assert!(!board.is_repetition(), "2nd occurrence must NOT trigger 3-fold repetition");

    board.make_move(nf3);
    assert!(!board.is_repetition());
    board.make_move(nf6);
    assert!(!board.is_repetition());
    board.make_move(ng1);
    assert!(!board.is_repetition());
    let u_ng8 = board.make_move(ng8);
    assert!(board.is_repetition(), "3rd occurrence MUST trigger is_repetition()");

    // 2. Unmake of 3rd occurrence must clear 3-fold repetition
    board.unmake_move(ng8, &u_ng8);
    assert!(!board.is_repetition(), "Unmaking move out of 3-fold clears repetition flag");

    board.make_move(ng8);
    assert!(board.is_repetition(), "Re-making restores 3-fold repetition");

    // 3. Advancing with an irreversible pawn move must clear repetition
    let e4 = Move::new(aqueval::types::Square::E2 as u8, aqueval::types::Square::E4 as u8, Move::DOUBLE_PAWN_PUSH);
    let u_e4 = board.make_move(e4);
    assert!(!board.is_repetition(), "Pawn move must reset repetition status");

    board.unmake_move(e4, &u_e4);
    assert!(board.is_repetition(), "Unmaking pawn move restores 3-fold repetition");

    // 4. Non-consecutive 3-fold repetition
    // From startpos: 1. Na3 Na6 2. Nb1 Nb8 (occ 2) 3. Nc3 Nc6 4. Nb1 Nb8 (occ 3)
    let mut b2 = Board::startpos();
    let na3 = Move::new(aqueval::types::Square::B1 as u8, aqueval::types::Square::A3 as u8, Move::QUIET);
    let na6 = Move::new(aqueval::types::Square::B8 as u8, aqueval::types::Square::A6 as u8, Move::QUIET);
    let nb1_from_a = Move::new(aqueval::types::Square::A3 as u8, aqueval::types::Square::B1 as u8, Move::QUIET);
    let nb8_from_a = Move::new(aqueval::types::Square::A6 as u8, aqueval::types::Square::B8 as u8, Move::QUIET);

    let nc3 = Move::new(aqueval::types::Square::B1 as u8, aqueval::types::Square::C3 as u8, Move::QUIET);
    let nc6 = Move::new(aqueval::types::Square::B8 as u8, aqueval::types::Square::C6 as u8, Move::QUIET);
    let nb1_from_c = Move::new(aqueval::types::Square::C3 as u8, aqueval::types::Square::B1 as u8, Move::QUIET);
    let nb8_from_c = Move::new(aqueval::types::Square::C6 as u8, aqueval::types::Square::B8 as u8, Move::QUIET);

    b2.make_move(na3);
    b2.make_move(na6);
    b2.make_move(nb1_from_a);
    b2.make_move(nb8_from_a);
    assert!(!b2.is_repetition(), "2nd occurrence of startpos via Na3/Na6 is not 3-fold");

    b2.make_move(nc3);
    b2.make_move(nc6);
    b2.make_move(nb1_from_c);
    b2.make_move(nb8_from_c);
    assert!(
        b2.is_repetition(),
        "3rd occurrence of startpos via Nc3/Nc6 MUST trigger 3-fold (non-consecutive repetition)"
    );

    // 5. Castling rights change prevents repetition:
    // 1. e4 e5 2. Ke2 Ke7 3. Ke1 Ke8 (pawns on e4/e5, both kings lost castling rights!)
    // 4. Ke2 Ke7 5. Ke1 Ke8 (occ 2 of uncastled position)
    // 6. Ke2 Ke7 7. Ke1 Ke8 (occ 3 of uncastled position -> 3-fold!)
    let mut b3 = Board::startpos();
    let e5 = Move::new(aqueval::types::Square::E7 as u8, aqueval::types::Square::E5 as u8, Move::DOUBLE_PAWN_PUSH);
    let ke2 = Move::new(aqueval::types::Square::E1 as u8, aqueval::types::Square::E2 as u8, Move::QUIET);
    let ke7 = Move::new(aqueval::types::Square::E8 as u8, aqueval::types::Square::E7 as u8, Move::QUIET);
    let ke1 = Move::new(aqueval::types::Square::E2 as u8, aqueval::types::Square::E1 as u8, Move::QUIET);
    let ke8 = Move::new(aqueval::types::Square::E7 as u8, aqueval::types::Square::E8 as u8, Move::QUIET);

    b3.make_move(e4);
    b3.make_move(e5);

    b3.make_move(ke2);
    b3.make_move(ke7);
    b3.make_move(ke1);
    b3.make_move(ke8);
    // This is 1st occurrence of the state (pawns on e4/e5, no castling rights)
    assert!(!b3.is_repetition(), "1st occurrence of uncastled e4-e5 position");

    b3.make_move(ke2);
    b3.make_move(ke7);
    b3.make_move(ke1);
    b3.make_move(ke8);
    assert!(!b3.is_repetition(), "2nd occurrence of uncastled e4-e5 position");

    b3.make_move(ke2);
    b3.make_move(ke7);
    b3.make_move(ke1);
    b3.make_move(ke8);
    assert!(b3.is_repetition(), "3rd occurrence of uncastled e4-e5 position MUST be 3-fold");
}

#[test]
fn test_adversarial_shakmaty_oracle_cross_check() {
    let mut rng = SimpleRng::new(0x9876543210FEDCBA);

    // Run 100 random games of 35 plies against Shakmaty reference oracle
    let num_games = 100;
    let max_plies = 35;
    let mut total_positions_checked = 0usize;

    for _game in 0..num_games {
        let mut aqueval_board = Board::startpos();
        let mut shak_pos = Chess::default();

        for _ply in 0..max_plies {
            total_positions_checked += 1;

            // 1. Compare check status
            assert_eq!(
                aqueval_board.is_in_check(),
                shak_pos.is_check(),
                "Check status mismatch at FEN: {}",
                aqueval_board.to_fen()
            );

            // 2. Generate legal moves from aqueval
            let mut aqueval_moves = MoveList::new();
            aqueval_board.generate_legal_moves(&mut aqueval_moves);
            let aqueval_uci_set: HashSet<String> = aqueval_moves.iter().map(|m| m.to_uci()).collect();

            // 3. Generate legal moves from Shakmaty
            let shak_moves = shak_pos.legal_moves();
            let shak_uci_set: HashSet<String> = shak_moves
                .iter()
                .map(|m| m.to_uci(CastlingMode::Standard).to_string())
                .collect();

            // 4. Assert exact legal move sets equality
            assert_eq!(
                aqueval_uci_set,
                shak_uci_set,
                "Legal move mismatch at FEN: {}\naqueval: {:?}\nShakmaty: {:?}",
                aqueval_board.to_fen(),
                aqueval_uci_set,
                shak_uci_set
            );

            if aqueval_moves.is_empty() {
                break;
            }

            // Pick a random move to advance both engines
            let chosen_aqueval_move = aqueval_moves.as_slice()[rng.gen_range(aqueval_moves.len())];
            let chosen_uci = chosen_aqueval_move.to_uci();

            let shak_move = shak_moves
                .iter()
                .find(|m| m.to_uci(CastlingMode::Standard).to_string() == chosen_uci)
                .expect("Move exists in shakmaty");

            aqueval_board.make_move(chosen_aqueval_move);
            shak_pos.play_unchecked(shak_move);

            // Verify FEN pieces and side to move match shakmaty
            let aqueval_fen = aqueval_board.to_fen();
            let shak_fen = shakmaty::fen::Fen::from_position(shak_pos.clone(), shakmaty::EnPassantMode::Legal).to_string();
            let aqueval_parts: Vec<&str> = aqueval_fen.split_whitespace().collect();
            let shak_parts: Vec<&str> = shak_fen.split_whitespace().collect();

            assert_eq!(aqueval_parts[0], shak_parts[0], "Board piece placement FEN mismatch");
            assert_eq!(aqueval_parts[1], shak_parts[1], "Side to move FEN mismatch");
            assert_eq!(aqueval_parts[2], shak_parts[2], "Castling rights FEN mismatch");
        }
    }

    println!(
        "Successfully cross-checked {} positions against Shakmaty oracle with 100% agreement",
        total_positions_checked
    );
    assert!(
        total_positions_checked >= 2000,
        "Expected at least 2000 positions checked, got {}",
        total_positions_checked
    );
}

#[test]
fn test_adversarial_diagonal_en_passant_discovered_check() {
    // White King on H1, Black Bishop on A8.
    // Pawns on E4 (White) and D5 (Black).
    // Diagonal A8-H1: A8(7,0), B7(6,1), C6(5,2), D5(4,3), E4(3,4), F3(2,5), G2(1,6), H1(0,7).
    // Both D5 and E4 are on this exact diagonal.
    // Black just played d7-d5, EP target square is d6.
    // If White plays exd6 e.p.: E4 moves to D6 (off diagonal), D5 is captured and removed.
    // Diagonal A8-H1 is completely opened -> King on H1 in check!
    // Therefore exd6 e.p. MUST be ILLEGAL.
    let fen = "b5k1/8/8/3p4/4P3/8/8/7K w - d6 0 1";
    let board = Board::from_fen(fen).expect("valid FEN");
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let ep_move = Move::new(
        aqueval::types::Square::E4 as u8,
        aqueval::types::Square::D6 as u8,
        Move::EN_PASSANT,
    );
    assert!(
        !moves.contains(&ep_move),
        "e4xd6 e.p. must be illegal due to discovered diagonal check along a8-h1 ray"
    );

    // Verify e4-e5 is legal though
    let e5_move = Move::new(
        aqueval::types::Square::E4 as u8,
        aqueval::types::Square::E5 as u8,
        Move::QUIET,
    );
    assert!(
        moves.contains(&e5_move),
        "e4-e5 should be legal as it remains on the diagonal"
    );
}

#[test]
fn test_adversarial_vertical_pinned_pawn_cannot_en_passant() {
    // White King on E1, White Pawn on E5, Black Rook on E8.
    // Black just played d7-d5, EP square is d6.
    // White Pawn on E5 is vertically pinned to E1 King.
    // exd6 e.p. moves to D6 (changing files), which would expose King on E1 to E8 Rook.
    // Therefore exd6 e.p. MUST be ILLEGAL.
    let fen = "k3r3/8/8/3pP3/8/8/8/4K3 w - d6 0 1";
    let board = Board::from_fen(fen).expect("valid FEN");
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let ep_move = Move::new(
        aqueval::types::Square::E5 as u8,
        aqueval::types::Square::D6 as u8,
        Move::EN_PASSANT,
    );
    assert!(
        !moves.contains(&ep_move),
        "e5xd6 e.p. must be illegal when pawn is vertically pinned along E-file"
    );
}

#[test]
fn test_adversarial_double_check_only_king_can_move() {
    // White King on E1. Black Knight on D3 checks King, Black Rook on E8 checks King.
    // Double check!
    // White has a Bishop on C2 that could theoretically capture Knight on D3.
    // White has a Queen on D1 that could block E8 check on E2.
    // But in double check, ONLY King moves are legal!
    let fen = "k3r3/8/8/8/8/3n4/2B5/3QK3 w - - 0 1";
    let board = Board::from_fen(fen).expect("valid FEN");
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    assert!(!moves.is_empty(), "King has evasions");
    for &m in moves.as_slice() {
        assert_eq!(
            m.from_sq(),
            aqueval::types::Square::E1 as u8,
            "In double check, only king moves are permitted. Found move from sq {}",
            m.from_sq()
        );
    }
}

