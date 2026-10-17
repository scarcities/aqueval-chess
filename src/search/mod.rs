pub mod evaluate;
pub mod time;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::board::Board;
use crate::movegen::types::{Move, MoveList};
use crate::perft::calculate_nps;
use crate::search::evaluate::evaluate;
use crate::search::time::TimeLimits;

pub const MATE_SCORE: i32 = 30000;
pub const MATE_THRESHOLD: i32 = 25000;

#[derive(Clone, Debug, Default)]
pub struct SearchLimits {
    pub depth: Option<usize>,
    pub nodes: Option<u64>,
    pub time_limits: TimeLimits,
    pub infinite: bool,
    pub ponder: bool,
    pub searchmoves: Option<Vec<Move>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub best_move: Move,
    pub score: i32,
    pub depth: usize,
    pub nodes: u64,
}

#[derive(Clone, Debug)]
pub struct SearchInfo {
    pub depth: usize,
    pub score_str: String,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
    pub pv_move: Move,
}

struct SearchStats {
    nodes: u64,
    start_time: Instant,
}

/// Formats evaluation score for UCI info output:
/// `cp <x>` or `mate <y>` (where y is moves to mate, positive if winning).
pub fn format_score(score: i32) -> String {
    if score > MATE_THRESHOLD {
        let moves_to_mate = (MATE_SCORE - score + 1) / 2;
        format!("mate {}", moves_to_mate)
    } else if score < -MATE_THRESHOLD {
        let moves_to_mate = -((MATE_SCORE + score + 1) / 2);
        format!("mate {}", moves_to_mate)
    } else {
        format!("cp {}", score)
    }
}

struct SearchContext<'a> {
    stop_flag: &'a AtomicBool,
    limits: &'a SearchLimits,
    stats: &'a mut SearchStats,
}

/// Recursively evaluates positions using alpha-beta pruning.
/// Zero heap allocations in recursion.
fn alpha_beta(
    board: &mut Board,
    depth: usize,
    ply: usize,
    mut alpha: i32,
    beta: i32,
    ctx: &mut SearchContext,
) -> i32 {
    ctx.stats.nodes += 1;

    // Periodic time and node limit check (every 1024 nodes)
    if (ctx.stats.nodes & 1023) == 0 {
        if ctx.stop_flag.load(Ordering::Relaxed) {
            return 0;
        }
        if let Some(hard) = ctx.limits.time_limits.hard_limit {
            if ctx.stats.start_time.elapsed() >= hard {
                ctx.stop_flag.store(true, Ordering::Relaxed);
                return 0;
            }
        }
        if let Some(max_nodes) = ctx.limits.nodes {
            if ctx.stats.nodes >= max_nodes {
                ctx.stop_flag.store(true, Ordering::Relaxed);
                return 0;
            }
        }
    }

    if depth == 0 {
        return evaluate(board);
    }

    // Threefold repetition or 50-move rule
    if board.halfmove_clock >= 100 || board.is_repetition() {
        return 0;
    }

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    if moves.is_empty() {
        if board.is_in_check() {
            return -MATE_SCORE + (ply as i32);
        } else {
            return 0; // Stalemate
        }
    }

    // In-place sort captures before quiets (zero heap allocation)
    moves.moves[..moves.count].sort_by_key(|a| std::cmp::Reverse(a.is_capture()));

    for &m in moves.as_slice() {
        let undo = board.make_move(m);
        let score = -alpha_beta(
            board,
            depth - 1,
            ply + 1,
            -beta,
            -alpha,
            ctx,
        );
        board.unmake_move(m, &undo);

        if ctx.stop_flag.load(Ordering::Relaxed) {
            return 0;
        }

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

/// Executes synchronous search with iterative deepening.
/// Emits search progress via `info_callback` if provided.
pub fn search_sync(
    board: &mut Board,
    limits: &SearchLimits,
    stop_flag: &AtomicBool,
    info_callback: Option<&dyn Fn(&SearchInfo)>,
) -> SearchResult {
    let mut stats = SearchStats {
        nodes: 0,
        start_time: Instant::now(),
    };

    let mut root_moves = MoveList::new();
    board.generate_legal_moves(&mut root_moves);

    // If no legal moves, return immediately
    if root_moves.is_empty() {
        let score = if board.is_in_check() {
            -MATE_SCORE
        } else {
            0
        };

        // If infinite or ponder, wait for stop before returning
        if limits.infinite || limits.ponder {
            while !stop_flag.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        return SearchResult {
            best_move: Move::NULL,
            score,
            depth: 0,
            nodes: 1,
        };
    }

    // Filter root moves by searchmoves if requested
    let mut candidate_moves: Vec<Move> = if let Some(ref allowed) = limits.searchmoves {
        let filtered: Vec<Move> = root_moves
            .as_slice()
            .iter()
            .copied()
            .filter(|m| allowed.contains(m))
            .collect();
        if filtered.is_empty() {
            root_moves.as_slice().to_vec()
        } else {
            filtered
        }
    } else {
        root_moves.as_slice().to_vec()
    };

    let mut best_move = candidate_moves[0];
    let mut best_score = -MATE_SCORE;
    let mut completed_depth = 0;

    let max_depth = limits.depth.unwrap_or(64);

    for current_depth in 1..=max_depth {
        let mut alpha = -32000;
        let beta = 32000;
        let mut depth_best_move = candidate_moves[0];
        let mut depth_best_score = -32000;

        // Order root moves: place current best_move first, then captures
        candidate_moves.sort_by(|a, b| {
            if *a == best_move {
                std::cmp::Ordering::Less
            } else if *b == best_move {
                std::cmp::Ordering::Greater
            } else {
                b.is_capture().cmp(&a.is_capture())
            }
        });

        let mut ctx = SearchContext {
            stop_flag,
            limits,
            stats: &mut stats,
        };

        for &m in &candidate_moves {
            let undo = board.make_move(m);
            let score = -alpha_beta(
                board,
                current_depth - 1,
                1,
                -beta,
                -alpha,
                &mut ctx,
            );
            board.unmake_move(m, &undo);

            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            if score > depth_best_score {
                depth_best_score = score;
                depth_best_move = m;
            }

            if score > alpha {
                alpha = score;
            }
        }

        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        best_move = depth_best_move;
        best_score = depth_best_score;
        completed_depth = current_depth;

        let elapsed = stats.start_time.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;
        let nps = calculate_nps(stats.nodes, elapsed);
        let score_str = format_score(best_score);

        if let Some(callback) = info_callback {
            let info = SearchInfo {
                depth: current_depth,
                score_str,
                nodes: stats.nodes,
                nps,
                time_ms: elapsed_ms,
                pv_move: best_move,
            };
            callback(&info);
        }

        // Check node limit
        if let Some(max_nodes) = limits.nodes {
            if stats.nodes >= max_nodes {
                break;
            }
        }

        // Check soft time limit: the next iteration costs several times the last one,
        // so do not start it once half the soft budget is gone.
        if let Some(soft) = limits.time_limits.soft_limit {
            if elapsed >= soft / 2 {
                break;
            }
        }
    }

    // On infinite or ponder, wait until stop_flag is signaled before finishing
    if limits.infinite || limits.ponder {
        while !stop_flag.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    SearchResult {
        best_move,
        score: best_score,
        depth: completed_depth,
        nodes: stats.nodes,
    }
}

/// Spawns an asynchronous search worker thread with `Arc<AtomicBool>` stop signal.
pub fn spawn_search_worker<F>(
    mut board: Board,
    limits: SearchLimits,
    stop_flag: Arc<AtomicBool>,
    on_info: F,
    on_complete: Box<dyn FnOnce(SearchResult) + Send + 'static>,
) -> std::thread::JoinHandle<()>
where
    F: Fn(SearchInfo) + Send + Sync + 'static,
{
    std::thread::spawn(move || {
        let info_cb = |info: &SearchInfo| {
            on_info(info.clone());
        };
        let result = search_sync(&mut board, &limits, &stop_flag, Some(&info_cb));
        on_complete(result);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_score() {
        assert_eq!(format_score(150), "cp 150");
        assert_eq!(format_score(-80), "cp -80");
        assert_eq!(format_score(0), "cp 0");
        assert_eq!(format_score(29999), "mate 1");
        assert_eq!(format_score(-29999), "mate -1");
    }

    #[test]
    fn test_search_depth_1_finds_legal_move() {
        let mut board = Board::startpos();
        let limits = SearchLimits {
            depth: Some(1),
            ..Default::default()
        };
        let stop_flag = AtomicBool::new(false);
        let res = search_sync(&mut board, &limits, &stop_flag, None);
        assert_ne!(res.best_move, Move::NULL);
        assert_eq!(res.depth, 1);
        assert!(res.nodes > 0);
    }

    #[test]
    fn test_search_captures_hanging_queen() {
        // White: King d1, Queen e2
        // Black: King e8, hanging Queen e5
        let mut board = Board::from_fen("4k3/8/8/4q3/8/8/4Q3/3K4 w - - 0 1").unwrap();
        let limits = SearchLimits {
            depth: Some(2),
            ..Default::default()
        };
        let stop_flag = AtomicBool::new(false);
        let res = search_sync(&mut board, &limits, &stop_flag, None);
        // Best move should capture e5 (e2e5)
        assert_eq!(res.best_move.to_uci(), "e2e5");
    }
}
