pub mod positions;

use std::time::{Duration, Instant};
use crate::board::Board;
use crate::movegen::types::{Move, MoveList};

/// Performance test results containing node counts, timing, and NPS.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PerftStats {
    pub depth: usize,
    pub nodes: u64,
    pub elapsed: Duration,
    pub nps: u64,
    pub bulk: bool,
}

/// Computes nodes per second from node count and elapsed duration.
#[inline]
pub fn calculate_nps(nodes: u64, elapsed: Duration) -> u64 {
    let secs = elapsed.as_secs_f64();
    if secs > 0.0 {
        (nodes as f64 / secs) as u64
    } else {
        0
    }
}

/// Recursively counts leaf nodes up to `depth` using bulk counting.
/// At depth 1, returns legal move count directly without recursive make/unmake.
/// Zero heap allocations in recursion.
pub fn perft(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    if depth == 1 {
        return moves.len() as u64;
    }

    let mut nodes = 0u64;
    for &m in moves.as_slice() {
        let undo = board.make_move(m);
        nodes += perft(board, depth - 1);
        board.unmake_move(m, &undo);
    }
    nodes
}

/// Recursively counts leaf nodes up to `depth` without bulk counting.
/// Traverses all moves down to depth 0 making and unmaking each move.
/// Zero heap allocations in recursion.
pub fn perft_no_bulk(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let mut nodes = 0u64;
    for &m in moves.as_slice() {
        let undo = board.make_move(m);
        nodes += perft_no_bulk(board, depth - 1);
        board.unmake_move(m, &undo);
    }
    nodes
}

/// Performs perft divide: counts sub-tree nodes for each root legal move.
/// Moves are sorted alphabetically by their UCI move string.
pub fn perft_divide(board: &mut Board, depth: usize) -> Vec<(Move, u64)> {
    if depth == 0 {
        return Vec::new();
    }

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let mut results = Vec::with_capacity(moves.len());

    for &m in moves.as_slice() {
        let undo = board.make_move(m);
        let count = if depth == 1 {
            1
        } else {
            perft(board, depth - 1)
        };
        board.unmake_move(m, &undo);
        results.push((m, count));
    }

    results.sort_by_key(|a| a.0.to_uci());
    results
}

/// Formats perft divide results into the standard output string:
/// `<move>: <nodes>` for each move, followed by `\nNodes searched: <total>\n`.
pub fn format_perft_divide(divide_results: &[(Move, u64)]) -> String {
    let mut out = String::new();
    let mut total = 0u64;
    for (m, count) in divide_results {
        out.push_str(&format!("{}: {}\n", m.to_uci(), count));
        total += count;
    }
    out.push_str(&format!("\nNodes searched: {}\n", total));
    out
}

/// Runs perft with timing and NPS computation.
pub fn run_perft_timed(board: &mut Board, depth: usize, bulk: bool) -> PerftStats {
    let start = Instant::now();
    let nodes = if bulk {
        perft(board, depth)
    } else {
        perft_no_bulk(board, depth)
    };
    let elapsed = start.elapsed();
    let nps = calculate_nps(nodes, elapsed);
    PerftStats {
        depth,
        nodes,
        elapsed,
        nps,
        bulk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perft_depth_0() {
        let mut board = Board::startpos();
        assert_eq!(perft(&mut board, 0), 1);
        assert_eq!(perft_no_bulk(&mut board, 0), 1);
    }

    #[test]
    fn test_perft_bulk_matches_non_bulk() {
        let mut board = Board::startpos();
        for depth in 1..=3 {
            let bulk_count = perft(&mut board, depth);
            let non_bulk_count = perft_no_bulk(&mut board, depth);
            assert_eq!(
                bulk_count, non_bulk_count,
                "Bulk and non-bulk must match at depth {depth}"
            );
        }
    }

    #[test]
    fn test_perft_divide_sum_matches_perft() {
        let mut board = Board::startpos();
        let depth = 3;
        let divide_res = perft_divide(&mut board, depth);
        let divide_total: u64 = divide_res.iter().map(|(_, n)| *n).sum();
        let perft_total = perft(&mut board, depth);
        assert_eq!(divide_total, perft_total);
        assert_eq!(divide_total, 8902);
    }

    #[test]
    fn test_format_perft_divide() {
        let mut board = Board::startpos();
        let divide_res = perft_divide(&mut board, 1);
        let formatted = format_perft_divide(&divide_res);
        assert!(formatted.contains("a2a3: 1"));
        assert!(formatted.contains("e2e4: 1"));
        assert!(formatted.contains("Nodes searched: 20"));
    }

    #[test]
    fn test_calculate_nps() {
        let nps = calculate_nps(1_000_000, Duration::from_millis(500));
        assert_eq!(nps, 2_000_000);
        let nps_zero = calculate_nps(1_000_000, Duration::ZERO);
        assert_eq!(nps_zero, 0);
    }
}
