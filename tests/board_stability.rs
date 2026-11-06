use aqueval::board::Board;
use aqueval::movegen::types::MoveList;
use aqueval::perft::{perft, perft_divide, perft_no_bulk};
use aqueval::types::PieceType;
use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, Position};
use std::collections::BTreeSet;

// ============================================================================
// 1. HELPER: BOARD STATE STABILITY VERIFIER
// ============================================================================

fn assert_board_stability_after_operation<F>(board: &mut Board, op_name: &str, op: F)
where
    F: FnOnce(&mut Board),
{
    let pre_board = board.clone();
    let pre_fen = board.to_fen();
    let pre_zobrist = board.zobrist_hash();
    let pre_calc_zobrist = board.calculate_zobrist();
    assert_eq!(
        pre_zobrist, pre_calc_zobrist,
        "Precondition failed: Zobrist desynchronized from calculate_zobrist"
    );
    let pre_clock = board.halfmove_clock;
    let pre_fullmove = board.fullmove_number;
    let pre_castling = board.castling_rights;
    let pre_ep = board.ep_square;
    let pre_history_len = board.history.len();

    op(board);

    // Deep struct comparison (Board derives PartialEq, Eq)
    assert_eq!(
        *board, pre_board,
        "[{op_name}] Board struct not identical after operation"
    );
    assert_eq!(
        board.to_fen(),
        pre_fen,
        "[{op_name}] FEN not restored after operation"
    );
    assert_eq!(
        board.zobrist_hash(),
        pre_zobrist,
        "[{op_name}] Zobrist hash not restored after operation"
    );
    assert_eq!(
        board.zobrist_hash(),
        board.calculate_zobrist(),
        "[{op_name}] Incremental Zobrist hash desynchronized after operation"
    );
    assert_eq!(
        board.halfmove_clock, pre_clock,
        "[{op_name}] Halfmove clock not restored after operation"
    );
    assert_eq!(
        board.fullmove_number, pre_fullmove,
        "[{op_name}] Fullmove number not restored after operation"
    );
    assert_eq!(
        board.castling_rights, pre_castling,
        "[{op_name}] Castling rights not restored after operation"
    );
    assert_eq!(
        board.ep_square, pre_ep,
        "[{op_name}] EP square not restored after operation"
    );
    assert_eq!(
        board.history.len(),
        pre_history_len,
        "[{op_name}] History stack length changed after operation"
    );
}

// Helper to compute Shakmaty perft for arbitrary depths
fn perft_shakmaty(pos: &Chess, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut nodes = 0;
    for m in moves {
        let mut next = pos.clone();
        next.play_unchecked(&m);
        nodes += perft_shakmaty(&next, depth - 1);
    }
    nodes
}

// Helper to assert aqueval perft matches Shakmaty perft on legal positions
fn assert_perft_matches_oracle(fen: &str, depth: usize, desc: &str) {
    let mut board = Board::from_fen(fen).expect("valid FEN for aqueval");
    let parsed_fen: Fen = fen.parse().expect("valid FEN for Shakmaty");
    let shak_pos: Chess = parsed_fen
        .into_position(CastlingMode::Standard)
        .expect("legal position for Shakmaty");

    let aqueval_nodes = perft(&mut board, depth);
    let shak_nodes = perft_shakmaty(&shak_pos, depth);

    assert_eq!(
        aqueval_nodes, shak_nodes,
        "Perft mismatch on [{desc}] at depth {depth}: aqueval={aqueval_nodes}, Oracle={shak_nodes}"
    );

    // Also assert non-bulk matches bulk
    let non_bulk_nodes = perft_no_bulk(&mut board, depth);
    assert_eq!(
        aqueval_nodes, non_bulk_nodes,
        "Bulk vs Non-Bulk mismatch on [{desc}] at depth {depth}"
    );

    // Assert divide sum matches
    let divide_res = perft_divide(&mut board, depth);
    let divide_sum: u64 = divide_res.iter().map(|(_, n)| *n).sum();
    assert_eq!(
        aqueval_nodes, divide_sum,
        "Divide sum mismatch on [{desc}] at depth {depth}"
    );

    // Assert stability
    assert_board_stability_after_operation(
        &mut board,
        &format!("Oracle test [{desc}] depth {depth}"),
        |b| {
            let _ = perft(b, depth);
        },
    );
}

// Helper for synthetic tactical edge cases
fn assert_synthetic_tactical_position(
    fen: &str,
    expected_present: &[&str],
    disallowed: &[&str],
    max_depth: usize,
    desc: &str,
) {
    let mut board = Board::from_fen(fen).expect("valid FEN for aqueval");
    let mut move_list = MoveList::new();
    board.generate_legal_moves(&mut move_list);

    let uci_moves: Vec<String> = move_list.iter().map(|m| m.to_uci()).collect();

    for &exp in expected_present {
        assert!(
            uci_moves.iter().any(|m| m == exp),
            "[{desc}] Expected move '{exp}' was not generated. Generated moves: {uci_moves:?}"
        );
    }

    for &dis in disallowed {
        assert!(
            !uci_moves.iter().any(|m| m == dis),
            "[{desc}] Disallowed illegal move '{dis}' was generated! Generated moves: {uci_moves:?}"
        );
    }

    // Verify perft consistency and board API stability across depths
    for d in 1..=max_depth {
        let bulk = perft(&mut board, d);
        let non_bulk = perft_no_bulk(&mut board, d);
        assert_eq!(
            bulk, non_bulk,
            "[{desc}] Bulk ({bulk}) != Non-bulk ({non_bulk}) at depth {d}"
        );

        let divide_res = perft_divide(&mut board, d);
        let divide_sum: u64 = divide_res.iter().map(|(_, n)| *n).sum();
        assert_eq!(
            bulk, divide_sum,
            "[{desc}] Divide sum ({divide_sum}) != Perft ({bulk}) at depth {d}"
        );

        // Assert Board API stability after each operation
        assert_board_stability_after_operation(
            &mut board,
            &format!("[{desc}] perft depth {d}"),
            |b| {
                let _ = perft(b, d);
            },
        );
        assert_board_stability_after_operation(
            &mut board,
            &format!("[{desc}] perft_no_bulk depth {d}"),
            |b| {
                let _ = perft_no_bulk(b, d);
            },
        );
        assert_board_stability_after_operation(
            &mut board,
            &format!("[{desc}] perft_divide depth {d}"),
            |b| {
                let _ = perft_divide(b, d);
            },
        );
    }
}

// ============================================================================
// 2. BOARD API STABILITY TESTS (PERFT & DIVIDE)
// ============================================================================

#[test]
fn test_board_api_stability_startpos() {
    let mut board = Board::startpos();

    // Test perft depths 1 through 4
    for d in 1..=4 {
        assert_board_stability_after_operation(
            &mut board,
            &format!("perft startpos depth {d}"),
            |b| {
                let _ = perft(b, d);
            },
        );
        assert_board_stability_after_operation(
            &mut board,
            &format!("perft_no_bulk startpos depth {d}"),
            |b| {
                let _ = perft_no_bulk(b, d);
            },
        );
        assert_board_stability_after_operation(
            &mut board,
            &format!("perft_divide startpos depth {d}"),
            |b| {
                let _ = perft_divide(b, d);
            },
        );
    }
}

#[test]
fn test_board_api_stability_kiwipete() {
    let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let mut board = Board::from_fen(fen).unwrap();

    for d in 1..=3 {
        assert_board_stability_after_operation(
            &mut board,
            &format!("perft kiwipete depth {d}"),
            |b| {
                let _ = perft(b, d);
            },
        );
        assert_board_stability_after_operation(
            &mut board,
            &format!("perft_no_bulk kiwipete depth {d}"),
            |b| {
                let _ = perft_no_bulk(b, d);
            },
        );
        assert_board_stability_after_operation(
            &mut board,
            &format!("perft_divide kiwipete depth {d}"),
            |b| {
                let _ = perft_divide(b, d);
            },
        );
    }
}

#[test]
fn test_board_api_stability_standard_positions_3_to_6() {
    let fens = [
        ("Position 3", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
        (
            "Position 4",
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        ),
        (
            "Position 5",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        ),
        (
            "Position 6",
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        ),
    ];

    for (name, fen) in fens {
        let mut board = Board::from_fen(fen).unwrap();
        for d in 1..=3 {
            assert_board_stability_after_operation(
                &mut board,
                &format!("perft {name} depth {d}"),
                |b| {
                    let _ = perft(b, d);
                },
            );
            assert_board_stability_after_operation(
                &mut board,
                &format!("perft_no_bulk {name} depth {d}"),
                |b| {
                    let _ = perft_no_bulk(b, d);
                },
            );
            assert_board_stability_after_operation(
                &mut board,
                &format!("perft_divide {name} depth {d}"),
                |b| {
                    let _ = perft_divide(b, d);
                },
            );
        }
    }
}

// ============================================================================
// 3. TACTICAL COMPLEXITY 1: PROMOTIONS & UNDERPROMOTIONS
// ============================================================================

#[test]
fn test_sedlak_promotion_position() {
    // Martin Sedlak's perft promotion position:
    // Heavy promotion into check, underpromotions, captures
    let fen = "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1";
    let mut board = Board::from_fen(fen).expect("valid Sedlak FEN");

    // CPW Ground Truth:
    // D1: 24
    // D2: 496
    // D3: 9483
    // D4: 182838
    // D5: 3605103
    let expected = [(1, 24), (2, 496), (3, 9483), (4, 182838), (5, 3605103)];

    for (depth, expected_nodes) in expected {
        let nodes = perft(&mut board, depth);
        assert_eq!(
            nodes, expected_nodes,
            "Sedlak position depth {depth} failed: got {nodes}, expected {expected_nodes}"
        );
        let non_bulk = perft_no_bulk(&mut board, depth);
        assert_eq!(nodes, non_bulk);
    }

    // Verify stability after D4 perft
    assert_board_stability_after_operation(&mut board, "Sedlak D4 stability", |b| {
        let _ = perft(b, 4);
    });
}

#[test]
fn test_promotions_into_check_and_underpromotions() {
    // 1. Knight underpromotion giving check (royal fork):
    let fen_knight_check = "8/4P3/3k4/8/8/8/8/4K3 w - - 0 1";
    let board = Board::from_fen(fen_knight_check).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    // e7e8=N gives check to King on d6!
    let knight_promo = moves
        .iter()
        .find(|m| m.from_sq() == 52 && m.to_sq() == 60 && m.promotion_piece() == Some(PieceType::Knight))
        .expect("e7e8n must be legal");

    let mut b_test = board.clone();
    b_test.make_move(*knight_promo);
    assert!(b_test.is_in_check(), "e7e8=N must give check to black king on d6");

    // Verify perft matches Shakmaty oracle on this legal position
    assert_perft_matches_oracle(fen_knight_check, 4, "Knight underpromotion check");

    // 2. Underpromotion to Bishop avoiding stalemate:
    let fen_stalemate_trap = "7k/4P3/5K2/8/8/8/8/8 w - - 0 1";
    assert_perft_matches_oracle(fen_stalemate_trap, 4, "Stalemate avoidance promotion");

    // 3. Promote out of check:
    let fen_promo_out_of_check = "2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1";
    assert_perft_matches_oracle(fen_promo_out_of_check, 4, "Promote out of check");
}

// ============================================================================
// 4. TACTICAL COMPLEXITY 2: EN PASSANT UNDER EXTREME PINS & CHECKS
// ============================================================================

#[test]
fn test_en_passant_pins_and_checks() {
    // (a) En passant pins king horizontally (rank 4/5 pin)
    // White King on e5, White pawn on d5, Black pawn on c5, Black Rook on a5, Black King on h8.
    // e.p. square is c6. d5xc6 e.p. removes d5 and c5 pawns, opening a5 rook check on e5 king -> ILLEGAL!
    assert_synthetic_tactical_position(
        "7k/8/8/r1pPK3/8/8/8/8 w - c6 0 1",
        &[],
        &["d5c6"],
        3,
        "EP horizontal rank pin (king on e5)",
    );

    // (b) King on h5, Rook on a5, White pawn on d5, Black pawn on c5, Black King on a8.
    // Pawn e.p. is still illegal because king is on the same rank 5!
    assert_synthetic_tactical_position(
        "k7/8/8/r1pP3K/8/8/8/8 w - c6 0 1",
        &[],
        &["d5c6"],
        3,
        "EP horizontal rank pin (king on h5)",
    );

    // (c) Horizontal rank 5 with a blocker: White King on h5, White Knight on f5, Black Rook on a5, Black pawn on c5, White pawn on d5, Black King on a8.
    // Here, knight on f5 shields king on h5. d5xc6 e.p. IS LEGAL!
    assert_synthetic_tactical_position(
        "k7/8/8/r1pP1N1K/8/8/8/8 w - c6 0 1",
        &["d5c6"],
        &[],
        3,
        "EP horizontal shielded by knight",
    );

    // (d) Diagonal pin on the moving pawn:
    // White King on a1(0), Black Bishop on h8(63). White pawn on e5(36). Black King on e8.
    // White pawn on e5 is pinned along the diagonal to King on a1!
    // Black pawn on d5(35), EP is d6(43).
    // Destination is d6(43), NOT on diagonal ray.
    // Thus e5xd6 e.p. is ILLEGAL because White pawn is pinned to king!
    assert_synthetic_tactical_position(
        "4k2b/8/8/3pP3/8/8/8/K7 w - d6 0 1",
        &[],
        &["e5d6"],
        3,
        "EP diagonal pinned moving pawn",
    );

    // (e) Discovered check on enemy king when pawn moves e.p.:
    // White Queen on d1, Black King on d8, White King on h1. White pawn on d5, Black pawn on e5.
    // White plays d5xe6 e.p. Pawn vacates d-file, Queen on d1 discovers check!
    let fen_ep_discovers = "3k4/8/8/3Pp3/8/8/8/3Q3K w - e6 0 1";
    let board = Board::from_fen(fen_ep_discovers).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);
    let ep_move = moves.iter().find(|m| m.to_uci() == "d5e6").expect("d5e6 must be legal");
    let mut test_b = board.clone();
    test_b.make_move(*ep_move);
    assert!(test_b.is_in_check(), "d5xe6 e.p. must deliver discovered check to black king on d8");
    assert_synthetic_tactical_position(
        fen_ep_discovers,
        &["d5e6"],
        &[],
        3,
        "EP discovers check on enemy king",
    );

    // (f) En passant resolving check:
    // White King on d4, Black King on h8. Black pawn on c5 gives check to White King!
    // White pawn on d5 captures c5 e.p. (d5c6), taking the checking piece and resolving check!
    assert_synthetic_tactical_position(
        "7k/8/8/2pP4/3K4/8/8/8 w - c6 0 1",
        &["d5c6"],
        &[],
        3,
        "EP resolves check",
    );

    // (g) Legal en passant rich position oracle cross-check:
    assert_perft_matches_oracle(
        "8/8/4k3/8/2pP4/8/B5K1/8 b - d3 0 1",
        3,
        "Legal diagonal pin / en passant rich",
    );
}

// ============================================================================
// 5. TACTICAL COMPLEXITY 3: DISCOVERED & DOUBLE CHECKS
// ============================================================================

#[test]
fn test_double_check_and_discovered_checks() {
    // (a) Double check forces king move:
    // White King on e1. Black Rook on e8 (straight check), Black Bishop on b4 (diagonal check), Black King on h8.
    // White has Queen on d1, Knights on b1 and g1. NONE of them can move; only King on e1 can move!
    let fen_double_check = "4r2k/8/8/8/1b6/8/8/1N1QK1N1 w - - 0 1";
    let board = Board::from_fen(fen_double_check).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    // All legal moves must be King moves (from e1)
    for m in moves.iter() {
        assert_eq!(
            m.from_sq(),
            4,
            "During double check, only king moves are legal; got move from {}",
            m.from_sq()
        );
    }
    assert_synthetic_tactical_position(
        fen_double_check,
        &["e1f1", "e1f2"],
        &["d1d2", "b1c3", "g1f3"],
        3,
        "Double check forces king move",
    );

    // (b) Discovered check by piece moving away:
    // White Queen on b1, White Knight on b4, Black King on b8, White King on h1.
    // Knight on b4 blocks Queen on b1 from checking King on b8.
    // When Knight moves away from b4, Queen discovers check!
    let fen_disc_knight = "1k6/8/8/8/1N6/8/8/1Q5K w - - 0 1";
    let board = Board::from_fen(fen_disc_knight).unwrap();
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);
    let knight_move = moves.iter().find(|m| m.from_sq() == 25 && m.to_sq() == 42).expect("b4c6 must be legal");
    let mut b_test = board.clone();
    b_test.make_move(*knight_move);
    assert!(b_test.is_in_check(), "Moving knight from b4 must discover check on b8 king");

    assert_perft_matches_oracle(fen_disc_knight, 3, "Discovered check by knight");
}

// ============================================================================
// 6. TACTICAL COMPLEXITY 4: CASTLING UNDER ATTACK AND PATH CLEARANCE
// ============================================================================

#[test]
fn test_castling_edge_cases() {
    // Castling with b1 attacked (Queenside castle IS legal):
    // White King on e1, Rook on a1. Black Bishop on a2 attacks b1. Black King on b8.
    // E1(4) -> C1(2), D1(3) is transit, B1(1) is only for rook traversal, King does not cross b1!
    let fen_b1_attacked = "1k6/8/8/8/8/8/b7/R3K2R w KQ - 0 1";
    assert_synthetic_tactical_position(
        fen_b1_attacked,
        &["e1c1"],
        &[],
        3,
        "Castling with b1 attacked",
    );

    // Castling through check (f1 attacked by Rook on f8 - Kingside castle ILLEGAL):
    let fen_f1_attacked = "1k3r2/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    assert_synthetic_tactical_position(
        fen_f1_attacked,
        &[],
        &["e1g1"],
        3,
        "Castling through check on f1",
    );

    // Castling out of check (King in check on e1 - castling ILLEGAL):
    let fen_e1_check = "1k2r3/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    assert_synthetic_tactical_position(
        fen_e1_check,
        &[],
        &["e1g1", "e1c1"],
        3,
        "Castling out of check",
    );

    // Castling rights tricky:
    assert_perft_matches_oracle(
        "r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1",
        3,
        "Castling rights tricky",
    );

    // Castling prevented:
    assert_perft_matches_oracle(
        "r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1",
        3,
        "Castling prevented",
    );
}

// ============================================================================
// 7. RANDOMIZED MULTI-PLY ORACLE & API STABILITY HARNESS
// ============================================================================

struct SimpleRng(u64);
impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 32) as u32
    }
}

#[test]
fn test_random_walk_perft_and_stability_cross_check() {
    let mut rng = SimpleRng::new(0x42A5FE9B7301C84D);

    let test_seeds = [
        // Startpos
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        // Kiwipete
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        // Position 3
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        // Position 4
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        // Position 5
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        // Position 6
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        // Sedlak Promotion
        "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1",
    ];

    let mut total_positions_checked = 0;

    for &start_fen in &test_seeds {
        for _game in 0..25 {
            let mut aqueval_board = Board::from_fen(start_fen).unwrap();
            let parsed_fen: Fen = start_fen.parse().unwrap();
            let mut shak_pos: Chess = parsed_fen
                .into_position(CastlingMode::Standard)
                .expect("valid Shakmaty position");

            for _ply in 0..30 {
                total_positions_checked += 1;

                // 1. Move generation equality check
                let mut aqueval_moves = MoveList::new();
                aqueval_board.generate_legal_moves(&mut aqueval_moves);

                let aqueval_set: BTreeSet<String> = aqueval_moves.iter().map(|m| m.to_uci()).collect();
                let shak_set: BTreeSet<String> = shak_pos
                    .legal_moves()
                    .iter()
                    .map(|m| m.to_uci(CastlingMode::Standard).to_string())
                    .collect();

                assert_eq!(
                    aqueval_set, shak_set,
                    "Move generation mismatch at FEN: {}",
                    aqueval_board.to_fen()
                );

                if aqueval_moves.is_empty() {
                    break;
                }

                // 2. Board stability during and after perft and perft_divide
                if total_positions_checked % 10 == 0 {
                    assert_board_stability_after_operation(
                        &mut aqueval_board,
                        "Random Walk Perft D2 Stability",
                        |b| {
                            let q_d2 = perft(b, 2);
                            let s_d2 = perft_shakmaty(&shak_pos, 2);
                            assert_eq!(q_d2, s_d2, "Perft D2 mismatch during random walk");
                        },
                    );

                    assert_board_stability_after_operation(
                        &mut aqueval_board,
                        "Random Walk Perft Divide D2 Stability",
                        |b| {
                            let div = perft_divide(b, 2);
                            let div_sum: u64 = div.iter().map(|(_, n)| *n).sum();
                            let q_d2 = perft(b, 2);
                            assert_eq!(div_sum, q_d2, "Divide sum mismatch during random walk");
                        },
                    );
                }

                // 3. Play random move
                let idx = (rng.next_u32() as usize) % aqueval_moves.len();
                let chosen_m = aqueval_moves[idx];
                let chosen_uci = chosen_m.to_uci();

                aqueval_board.make_move(chosen_m);

                let shak_uci: UciMove = chosen_uci.parse().unwrap();
                let shak_m = shak_uci.to_move(&shak_pos).unwrap();
                shak_pos.play_unchecked(&shak_m);
            }
        }
    }

    println!(
        "Successfully verified {} complex game positions with full API stability and Shakmaty oracle cross-checks!",
        total_positions_checked
    );
}
