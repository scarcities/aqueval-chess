use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::board::Board;
use crate::perft::calculate_nps;
use crate::perft::positions::ALL_POSITIONS;
use crate::search::{search_sync, SearchLimits};

/// Default fixed search depth for the benchmark suite.
pub const DEFAULT_BENCH_DEPTH: usize = 11;

/// Runs the standard benchmark suite across all 6 standard positions to the specified depth.
/// Outputs benchmark statistics and returns the deterministic node count ("bench signature").
pub fn run_bench_with_writer<W: Write>(depth: Option<usize>, writer: &mut W) -> u64 {
    let bench_depth = depth.unwrap_or(DEFAULT_BENCH_DEPTH);
    let mut total_nodes = 0u64;
    let start_time = Instant::now();

    let stop_flag = AtomicBool::new(false);

    for pos in &ALL_POSITIONS {
        let mut board = Board::from_fen(pos.fen).unwrap_or_else(|_| {
            panic!("Standard bench position {} has valid FEN", pos.id);
        });

        let limits = SearchLimits {
            depth: Some(bench_depth),
            ..Default::default()
        };

        let result = search_sync(&mut board, &limits, &stop_flag, None);
        total_nodes += result.nodes;

        let _ = writeln!(
            writer,
            "Position {:>2} [{}]: {:>10} nodes, bestmove {}",
            pos.id,
            pos.name,
            result.nodes,
            result.best_move.to_uci()
        );
    }

    let elapsed = start_time.elapsed();
    let elapsed_ms = elapsed.as_millis() as u64;
    let nps = calculate_nps(total_nodes, elapsed);

    let _ = writeln!(writer, "===========================");
    let _ = writeln!(writer, "Total time (ms) : {}", elapsed_ms);
    let _ = writeln!(writer, "Nodes searched  : {}", total_nodes);
    let _ = writeln!(writer, "Nodes/second    : {}", nps);
    let _ = writeln!(writer, "===========================");

    total_nodes
}

/// Convenience function running bench and printing to standard output.
pub fn run_bench(depth: Option<usize>) -> u64 {
    run_bench_with_writer(depth, &mut std::io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_deterministic_signature() {
        let mut buf1 = Vec::new();
        let sig1 = run_bench_with_writer(Some(3), &mut buf1);

        let mut buf2 = Vec::new();
        let sig2 = run_bench_with_writer(Some(3), &mut buf2);

        assert_eq!(sig1, sig2, "Bench signature must be strictly deterministic across runs");
        assert!(sig1 > 0, "Bench signature must be greater than zero");
    }
}
