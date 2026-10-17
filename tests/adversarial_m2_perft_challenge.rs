use quant::board::Board;
use quant::movegen::types::MoveList;
use quant::perft::positions::{
    POS_3, POS_4, POS_5, POS_6, POS_KIWIPETE, POS_STARTPOS,
};
use quant::perft::{format_perft_divide, perft, perft_divide, perft_no_bulk};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Position};

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

// ----------------------------------------------------------------------------
// 1. STANDARD POSITIONS BULK VS NON-BULK & BOARD INVARIANCE
// ----------------------------------------------------------------------------

#[test]
fn test_standard_suite_bulk_vs_non_bulk_full_fidelity() {
    let positions = [
        (&POS_STARTPOS, if cfg!(debug_assertions) { 4 } else { 5 }),
        (&POS_KIWIPETE, if cfg!(debug_assertions) { 3 } else { 4 }),
        (&POS_3, if cfg!(debug_assertions) { 4 } else { 5 }),
        (&POS_4, if cfg!(debug_assertions) { 4 } else { 5 }),
        (&POS_5, if cfg!(debug_assertions) { 3 } else { 4 }),
        (&POS_6, if cfg!(debug_assertions) { 3 } else { 4 }),
    ];

    for (pos, max_depth) in positions {
        let mut board = Board::from_fen(pos.fen).expect("valid FEN");
        let initial_board = board.clone();

        for depth in 1..=max_depth {
            let bulk_count = perft(&mut board, depth);
            assert_eq!(
                board, initial_board,
                "[{}] perft(depth {}) mutated board state",
                pos.name, depth
            );

            let non_bulk_count = perft_no_bulk(&mut board, depth);
            assert_eq!(
                board, initial_board,
                "[{}] perft_no_bulk(depth {}) mutated board state",
                pos.name, depth
            );

            assert_eq!(
                bulk_count, non_bulk_count,
                "[{}] Bulk vs Non-Bulk mismatch at depth {}: bulk={}, non_bulk={}",
                pos.name, depth, bulk_count, non_bulk_count
            );

            let expected = pos.expected_nodes(depth).expect("expected node count");
            assert_eq!(
                bulk_count, expected,
                "[{}] Node count mismatch at depth {}: got {}, expected {}",
                pos.name, depth, bulk_count, expected
            );
        }
    }
}

// ----------------------------------------------------------------------------
// 2. DEPTH 0 AND TERMINAL POSITIONS (CHECKMATE / STALEMATE)
// ----------------------------------------------------------------------------

#[test]
fn test_adversarial_depth_zero_and_terminal_positions() {
    // 2.1 Startpos depth 0
    let mut startpos = Board::startpos();
    assert_eq!(perft(&mut startpos, 0), 1);
    assert_eq!(perft_no_bulk(&mut startpos, 0), 1);
    let divide_0 = perft_divide(&mut startpos, 0);
    assert!(
        divide_0.is_empty(),
        "perft_divide at depth 0 must return an empty list"
    );

    // 2.2 Fool's Mate (White is checkmated)
    let fools_mate_fen = "rnb1kbnr/pppp1ppp/4p3/8/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
    let mut checkmate_board = Board::from_fen(fools_mate_fen).unwrap();
    assert_eq!(perft(&mut checkmate_board, 0), 1);
    assert_eq!(perft_no_bulk(&mut checkmate_board, 0), 1);
    assert_eq!(
        perft(&mut checkmate_board, 1),
        0,
        "Checkmate position perft(1) must be 0"
    );
    assert_eq!(
        perft_no_bulk(&mut checkmate_board, 1),
        0,
        "Checkmate position perft_no_bulk(1) must be 0"
    );
    let divide_mate = perft_divide(&mut checkmate_board, 1);
    assert!(
        divide_mate.is_empty(),
        "Checkmate divide must have 0 moves"
    );
    let formatted_mate = format_perft_divide(&divide_mate);
    assert_eq!(
        formatted_mate, "\nNodes searched: 0\n",
        "Formatted divide on checkmate must show 0 nodes searched"
    );

    // 2.3 Stalemate
    let stalemate_fen = "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1";
    let mut stalemate_board = Board::from_fen(stalemate_fen).unwrap();
    assert_eq!(perft(&mut stalemate_board, 0), 1);
    assert_eq!(perft_no_bulk(&mut stalemate_board, 0), 1);
    assert_eq!(
        perft(&mut stalemate_board, 1),
        0,
        "Stalemate perft(1) must be 0"
    );
    assert_eq!(
        perft_no_bulk(&mut stalemate_board, 1),
        0,
        "Stalemate perft_no_bulk(1) must be 0"
    );
    let divide_stale = perft_divide(&mut stalemate_board, 1);
    assert!(divide_stale.is_empty(), "Stalemate divide must have 0 moves");
}

// ----------------------------------------------------------------------------
// 3. PERFT DIVIDE INVARIANTS: CHILD PERFT, SUM EQUALITY, AND SORTING
// ----------------------------------------------------------------------------

#[test]
fn test_perft_divide_rigorous_oracle_invariants() {
    let test_cases = [
        ("Startpos", POS_STARTPOS.fen, 3),
        ("Kiwipete", POS_KIWIPETE.fen, 2),
        ("Position 3", POS_3.fen, 3),
        ("Position 4", POS_4.fen, 2),
        ("Position 5", POS_5.fen, 2),
        ("Position 6", POS_6.fen, 2),
    ];

    for (name, fen, max_depth) in test_cases {
        let mut board = Board::from_fen(fen).expect("valid fen");
        let initial_board = board.clone();

        for depth in 1..=max_depth {
            let divide_results = perft_divide(&mut board, depth);

            // Invariant A: Board state unmodified
            assert_eq!(
                board, initial_board,
                "[{}] perft_divide(depth {}) mutated root board",
                name, depth
            );

            // Invariant B & C: Sum equals perft and perft_no_bulk
            let divide_sum: u64 = divide_results.iter().map(|(_, n)| *n).sum();
            let perft_bulk = perft(&mut board, depth);
            let perft_non_bulk = perft_no_bulk(&mut board, depth);

            assert_eq!(
                divide_sum, perft_bulk,
                "[{}] Divide sum ({}) != perft bulk ({}) at depth {}",
                name, divide_sum, perft_bulk, depth
            );
            assert_eq!(
                divide_sum, perft_non_bulk,
                "[{}] Divide sum ({}) != perft non-bulk ({}) at depth {}",
                name, divide_sum, perft_non_bulk, depth
            );

            // Invariant D: Root moves strictly sorted alphabetically
            for w in divide_results.windows(2) {
                let uci_a = w[0].0.to_uci();
                let uci_b = w[1].0.to_uci();
                assert!(
                    uci_a < uci_b,
                    "[{}] Moves not strictly sorted alphabetically: '{}' >= '{}'",
                    name, uci_a, uci_b
                );
            }

            // Invariant E: Completeness matching root legal moves
            let mut root_moves = MoveList::new();
            board.generate_legal_moves(&mut root_moves);
            assert_eq!(
                divide_results.len(),
                root_moves.len(),
                "[{}] Divide move count ({}) != root legal move count ({})",
                name, divide_results.len(), root_moves.len()
            );

            // Invariant F: Each root move's leaf count matches child perft
            for &(m, divide_child_count) in &divide_results {
                let mut child = board.clone();
                let undo = child.make_move(m);

                let expected_child_nodes = if depth == 1 {
                    1
                } else {
                    perft(&mut child, depth - 1)
                };

                let expected_non_bulk_child_nodes = if depth == 1 {
                    1
                } else {
                    perft_no_bulk(&mut child, depth - 1)
                };

                assert_eq!(
                    divide_child_count, expected_child_nodes,
                    "[{}] Move {} divide child count ({}) != child perft bulk ({}) at depth {}",
                    name, m.to_uci(), divide_child_count, expected_child_nodes, depth
                );
                assert_eq!(
                    divide_child_count, expected_non_bulk_child_nodes,
                    "[{}] Move {} divide child count ({}) != child perft non-bulk ({}) at depth {}",
                    name, m.to_uci(), divide_child_count, expected_non_bulk_child_nodes, depth
                );

                child.unmake_move(m, &undo);
                assert_eq!(
                    child, board,
                    "[{}] child make/unmake for move {} left residue",
                    name, m.to_uci()
                );
            }

            // Invariant G: Formatted output adheres to standard format
            let formatted = format_perft_divide(&divide_results);
            assert!(
                formatted.ends_with(&format!("\nNodes searched: {}\n", divide_sum)),
                "[{}] Formatter missing standard total trailer",
                name
            );
        }
    }
}

// ----------------------------------------------------------------------------
// 4. ADVERSARIAL TACTICAL POSITIONS (EP PINS, DISCOVERED CHECKS, DOUBLE CHECKS)
// ----------------------------------------------------------------------------

#[test]
fn test_adversarial_tactical_positions_bulk_vs_non_bulk_and_divide() {
    let tactical_positions = [
        // EP horizontal pin: White King e5, White Pawn d5, Black Pawn c5, Black Rook a5, Black King h8
        ("EP Horizontal Pin (White)", "7k/8/8/r1PpK3/8/8/8/8 w - c6 0 1", 3),
        // EP horizontal pin: Black King e4, Black Pawn d4, White Pawn c4, White Rook a4, White King h1
        ("EP Horizontal Pin (Black)", "8/8/8/8/R1pPk3/8/8/7K b - c3 0 1", 3),
        // EP diagonal pin: moving pawn pinned, Black King a8
        ("EP Diagonal Pin (Moving Pawn)", "k6b/8/8/3pP3/8/8/8/K7 w - d6 0 1", 3),
        // EP diagonal pin: captured pawn was shielding King, Black King a8
        ("EP Diagonal Pin (Shielding Pawn)", "k6b/8/8/3Pp3/8/8/8/K7 w - e6 0 1", 3),
        // Double check: King MUST move, Black King h8
        ("Double Check", "4r2k/8/8/b7/8/8/8/R2QK2R w KQ - 0 1", 2),
        // Castling through check
        ("Castling Through Check", "4k2r/8/8/8/8/8/5r2/R3K2R w KQk - 0 1", 3),
        // Promotion rich position
        ("Promotions under check and capture", "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1", 3),
        // King and Rook endgame with pawn promotions
        ("KRPKR endgame", "8/8/4k3/8/8/4K3/4P3/8 w - - 0 1", 4),
    ];

    for (name, fen, depth) in tactical_positions {
        let mut board = Board::from_fen(fen).expect("valid tactical FEN");
        let initial_board = board.clone();

        let bulk_nodes = perft(&mut board, depth);
        assert_eq!(board, initial_board);

        let non_bulk_nodes = perft_no_bulk(&mut board, depth);
        assert_eq!(board, initial_board);

        assert_eq!(
            bulk_nodes, non_bulk_nodes,
            "[{}] Bulk ({}) != Non-bulk ({}) at depth {}",
            name, bulk_nodes, non_bulk_nodes, depth
        );

        let divide = perft_divide(&mut board, depth);
        assert_eq!(board, initial_board);

        let divide_sum: u64 = divide.iter().map(|(_, n)| *n).sum();
        assert_eq!(
            divide_sum, bulk_nodes,
            "[{}] Divide sum ({}) != bulk ({})",
            name, divide_sum, bulk_nodes
        );

        // Verify divide child perfts
        for &(m, child_count) in &divide {
            let mut child = board.clone();
            child.make_move(m);
            let expected_child = if depth == 1 { 1 } else { perft(&mut child, depth - 1) };
            assert_eq!(
                child_count, expected_child,
                "[{}] Move {} divide child count mismatch",
                name, m.to_uci()
            );
        }

        // Verify sorted
        for w in divide.windows(2) {
            assert!(
                w[0].0.to_uci() < w[1].0.to_uci(),
                "[{}] Divide not sorted alphabetically: {} >= {}",
                name, w[0].0.to_uci(), w[1].0.to_uci()
            );
        }
    }
}

// ----------------------------------------------------------------------------
// 5. RANDOM WALK REACHABLE POSITIONS CROSS-CHECK AGAINST SHAKMATY
// ----------------------------------------------------------------------------

#[test]
fn test_adversarial_random_walk_divide_and_bulk_oracle() {
    let mut rng = SimpleRng::new(0x1337BEEF98765432);
    let seeds = [
        POS_STARTPOS.fen,
        POS_KIWIPETE.fen,
        POS_3.fen,
        POS_4.fen,
        POS_5.fen,
    ];

    let mut positions_tested = 0;

    for &start_fen in &seeds {
        for _game in 0..10 {
            let mut board = Board::from_fen(start_fen).expect("valid start fen");
            let parsed_fen: Fen = start_fen.parse().expect("valid fen for shakmaty");
            let mut shak_pos: Chess = parsed_fen
                .into_position(CastlingMode::Standard)
                .expect("legal shakmaty position");

            for _ply in 0..15 {
                let mut moves = MoveList::new();
                board.generate_legal_moves(&mut moves);

                if moves.is_empty() {
                    break;
                }

                // Sample random legal move to advance the game
                let idx = (rng.next_u32() as usize) % moves.len();
                let m = moves[idx];

                let _undo = board.make_move(m);

                // Advance shakmaty
                let uci_str = m.to_uci();
                let shak_move = shak_pos
                    .legal_moves()
                    .into_iter()
                    .find(|sm| sm.to_uci(CastlingMode::Standard).to_string() == uci_str)
                    .unwrap_or_else(|| panic!("Shakmaty did not have legal move {}", uci_str));
                shak_pos.play_unchecked(&shak_move);

                positions_tested += 1;

                // Test depth 1 & depth 2 bulk vs non-bulk and divide
                let initial_state = board.clone();

                for d in 1..=2 {
                    let bulk_n = perft(&mut board, d);
                    let non_bulk_n = perft_no_bulk(&mut board, d);

                    assert_eq!(
                        bulk_n, non_bulk_n,
                        "Random position bulk != non-bulk at depth {}: bulk={}, non_bulk={}, FEN={}",
                        d, bulk_n, non_bulk_n, board.to_fen()
                    );

                    let divide = perft_divide(&mut board, d);
                    let divide_sum: u64 = divide.iter().map(|(_, n)| *n).sum();
                    assert_eq!(
                        divide_sum, bulk_n,
                        "Random position divide sum != bulk at depth {}: sum={}, bulk={}, FEN={}",
                        d, divide_sum, bulk_n, board.to_fen()
                    );

                    // Cross check with Shakmaty oracle at depth d
                    let shak_n = perft_shakmaty(&shak_pos, d);
                    assert_eq!(
                        bulk_n, shak_n,
                        "Quant perft ({}) != Shakmaty perft ({}) at depth {} for FEN={}",
                        bulk_n, shak_n, d, board.to_fen()
                    );

                    // Verify strict sorting of divide
                    for w in divide.windows(2) {
                        assert!(
                            w[0].0.to_uci() < w[1].0.to_uci(),
                            "Divide not strictly sorted: {} >= {}",
                            w[0].0.to_uci(), w[1].0.to_uci()
                        );
                    }
                }

                assert_eq!(
                    board, initial_state,
                    "perft / divide operations corrupted board state!"
                );
            }
        }
    }

    assert!(
        positions_tested >= 50,
        "Expected at least 50 random positions tested; got {}",
        positions_tested
    );
}
