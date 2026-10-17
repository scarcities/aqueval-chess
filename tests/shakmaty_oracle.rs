use quant::board::Board;
use quant::movegen::types::MoveList;
use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, Position};
use std::collections::BTreeSet;

fn perft_quant(board: &mut Board, depth: usize) -> u64 {
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
        nodes += perft_quant(board, depth - 1);
        board.unmake_move(m, &undo);
    }
    nodes
}

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

// Simple LCG PRNG for reproducible test sequences
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
fn test_shakmaty_random_walk_10k_positions() {
    let mut rng = SimpleRng::new(0xDEADBEEFCAFE1234);
    let mut total_positions = 0;

    let start_fens = [
        // Standard startpos
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        // Kiwipete
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        // Castle rights tricky
        "r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1",
        // Castling prevented
        "r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1",
        // Promote out of check
        "2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1",
        // Diagonal pin / en passant rich
        "8/8/4k3/8/2pP4/8/B5K1/8 b - d3 0 1",
    ];

    for &start_fen in &start_fens {
        for _game in 0..150 {
            let mut quant_board = Board::from_fen(start_fen).unwrap();
            let parsed_fen: Fen = start_fen.parse().expect("valid fen");
            let mut shak_pos: Chess = parsed_fen.into_position(CastlingMode::Standard).expect("legal pos");

            for _ply in 0..40 {
                total_positions += 1;

                // 1. Move generation comparison
                let mut quant_moves = MoveList::new();
                quant_board.generate_legal_moves(&mut quant_moves);

                let quant_set: BTreeSet<String> = quant_moves.iter().map(|m| m.to_uci()).collect();

                let shak_legal = shak_pos.legal_moves();
                let shak_set: BTreeSet<String> = shak_legal
                    .iter()
                    .map(|m| m.to_uci(CastlingMode::Standard).to_string())
                    .collect();

                let current_fen = quant_board.to_fen();
                if quant_set != shak_set {
                    panic!(
                        "Move mismatch at FEN: {}\nQuant moves ({:?}):\n{:?}\nShakmaty moves ({:?}):\n{:?}",
                        current_fen,
                        quant_set.len(),
                        quant_set,
                        shak_set.len(),
                        shak_set
                    );
                }

                // 2. In-check verification
                assert_eq!(
                    quant_board.is_in_check(),
                    shak_pos.is_check(),
                    "Check mismatch at FEN: {}",
                    current_fen
                );

                // 3. Perft D2 cross-check (spot check every 5 positions to keep test fast)
                if total_positions % 5 == 0 {
                    let q_d2 = perft_quant(&mut quant_board, 2);
                    let s_d2 = perft_shakmaty(&shak_pos, 2);
                    assert_eq!(
                        q_d2, s_d2,
                        "Perft D2 mismatch at FEN: {}. Quant: {}, Shakmaty: {}",
                        current_fen, q_d2, s_d2
                    );
                }

                // If game over (no legal moves), stop game
                if quant_moves.is_empty() {
                    break;
                }

                // Pick a random legal move
                let move_idx = (rng.next_u32() as usize) % quant_moves.len();
                let chosen_m = quant_moves[move_idx];
                let chosen_uci = chosen_m.to_uci();

                // Make move in Quant
                let undo = quant_board.make_move(chosen_m);

                // Verify incremental hash matches recalculation
                assert_eq!(
                    quant_board.zobrist_hash(),
                    quant_board.calculate_zobrist(),
                    "Zobrist mismatch after move {} at FEN {}",
                    chosen_uci,
                    current_fen
                );

                // Make move in Shakmaty
                let shak_move: UciMove = chosen_uci.parse().unwrap();
                let shak_m = shak_move.to_move(&shak_pos).expect("shakmaty accepts legal move");
                shak_pos.play_unchecked(&shak_m);

                // Verify unmake consistency
                let mut temp_board = quant_board.clone();
                temp_board.unmake_move(chosen_m, &undo);
                assert_eq!(
                    temp_board.to_fen(),
                    current_fen,
                    "Unmake failed to restore FEN after move {}",
                    chosen_uci
                );
            }
        }
    }

    println!("Successfully verified {} random game positions against shakmaty oracle!", total_positions);
    assert!(total_positions >= 10000, "Must verify at least 10,000 positions");
}
