pub mod evaluate;
pub mod time;
pub mod tt;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::board::Board;
use crate::movegen::types::{Move, MoveList};
use crate::perft::calculate_nps;
use crate::search::evaluate::{evaluate, piece_value};
use crate::search::time::TimeLimits;
use crate::search::tt::{Bound, TranspositionTable};
use crate::types::PieceType;

pub const MATE_SCORE: i32 = 30000;
pub const MATE_THRESHOLD: i32 = 25000;
const INF: i32 = 32000;
const MAX_PLY: usize = 128;

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
    pub seldepth: usize,
    pub score_str: String,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
    pub hashfull: u64,
    pub pv: Vec<Move>,
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

/// MVV-LVA key: most valuable victim first, then least valuable attacker.
fn mvv_lva(board: &Board, m: Move) -> i32 {
    let victim = if m.is_en_passant() {
        PieceType::Pawn
    } else {
        board.piece_at(m.to_sq()).map_or(PieceType::Pawn, |p| p.piece_type)
    };
    let attacker = board.piece_at(m.from_sq()).map_or(PieceType::Pawn, |p| p.piece_type);
    piece_value(victim) * 8 - attacker.index() as i32
}

/// Moves the highest-scored remaining move to `i` (lazy selection sort, no allocation).
#[inline(always)]
fn pick_move(moves: &mut [Move], scores: &mut [i32], i: usize) {
    let mut best = i;
    for j in i + 1..moves.len() {
        if scores[j] > scores[best] {
            best = j;
        }
    }
    moves.swap(i, best);
    scores.swap(i, best);
}

/// Per-search state. Heap parts are allocated once per `go`, never inside the tree.
struct Searcher<'a> {
    board: &'a mut Board,
    tt: &'a mut TranspositionTable,
    limits: &'a SearchLimits,
    stop: &'a AtomicBool,
    start: Instant,
    nodes: u64,
    seldepth: usize,
    killers: [[Move; 2]; MAX_PLY],
    history: Box<[[[i32; 64]; 64]; 2]>,
    pv: Box<[[Move; MAX_PLY]; MAX_PLY]>,
    pv_len: [usize; MAX_PLY],
}

impl Searcher<'_> {
    fn should_stop(&mut self) -> bool {
        if (self.nodes & 1023) == 0 {
            let out_of_time = self
                .limits
                .time_limits
                .hard_limit
                .is_some_and(|hard| self.start.elapsed() >= hard);
            let out_of_nodes = self.limits.nodes.is_some_and(|max| self.nodes >= max);
            if out_of_time || out_of_nodes {
                self.stop.store(true, Ordering::Relaxed);
            }
        }
        self.stop.load(Ordering::Relaxed)
    }

    /// Any earlier occurrence since the last irreversible move counts as a draw inside the search.
    fn is_repetition(&self) -> bool {
        let h = &self.board.history;
        let key = self.board.zobrist_hash();
        let limit = h.len().saturating_sub(self.board.halfmove_clock() as usize);
        (limit..h.len()).rev().skip(1).step_by(2).any(|i| h[i].zobrist_hash == key)
    }

    fn negamax(&mut self, mut depth: i32, ply: usize, mut alpha: i32, mut beta: i32) -> i32 {
        self.pv_len[ply] = ply;
        let pv_node = beta - alpha > 1;

        if ply > 0 {
            if self.board.halfmove_clock() >= 100 || self.is_repetition() {
                return 0;
            }
            // Mate distance pruning
            alpha = alpha.max(-MATE_SCORE + ply as i32);
            beta = beta.min(MATE_SCORE - ply as i32 - 1);
            if alpha >= beta {
                return alpha;
            }
        }

        let in_check = self.board.is_in_check();
        if in_check {
            depth += 1;
        }
        if depth <= 0 {
            return self.quiescence(ply, alpha, beta);
        }
        self.nodes += 1;
        if self.should_stop() {
            return 0;
        }
        if ply >= MAX_PLY - 1 {
            return evaluate(self.board);
        }

        let key = self.board.zobrist_hash();
        let mut tt_move = Move::NULL;
        if let Some(e) = self.tt.probe(key, ply) {
            tt_move = e.mv;
            let score = e.score as i32;
            if !pv_node
                && e.depth as i32 >= depth
                && match e.bound {
                    Bound::Exact => true,
                    Bound::Lower => score >= beta,
                    Bound::Upper => score <= alpha,
                }
            {
                return score;
            }
        }

        let mut list = MoveList::new();
        self.board.generate_legal_moves(&mut list);
        if list.is_empty() {
            return if in_check { -MATE_SCORE + ply as i32 } else { 0 };
        }

        let n = list.count;
        let stm = self.board.side_to_move.index();
        let mut scores = [0i32; 256];
        for (i, &m) in list.moves[..n].iter().enumerate() {
            scores[i] = if m == tt_move {
                2_000_000
            } else if m.is_capture() {
                1_000_000 + mvv_lva(self.board, m)
            } else if m.promotion_piece() == Some(PieceType::Queen) {
                950_000
            } else if m == self.killers[ply][0] {
                900_000
            } else if m == self.killers[ply][1] {
                800_000
            } else {
                self.history[stm][m.from_sq() as usize][m.to_sq() as usize]
            };
        }

        let original_alpha = alpha;
        let mut best_score = -INF;
        let mut best_move = Move::NULL;
        let mut searched = 0;

        for i in 0..n {
            pick_move(&mut list.moves[..n], &mut scores[..n], i);
            let m = list.moves[i];
            if ply == 0 {
                if let Some(allowed) = &self.limits.searchmoves {
                    if !allowed.is_empty() && !allowed.contains(&m) {
                        continue;
                    }
                }
            }

            let undo = self.board.make_move(m);
            // PVS: full window for the first move, null window for the rest, re-search on improvement.
            let score = if searched == 0 {
                -self.negamax(depth - 1, ply + 1, -beta, -alpha)
            } else {
                let s = -self.negamax(depth - 1, ply + 1, -alpha - 1, -alpha);
                if s > alpha && s < beta {
                    -self.negamax(depth - 1, ply + 1, -beta, -alpha)
                } else {
                    s
                }
            };
            self.board.unmake_move(m, &undo);
            searched += 1;

            if self.stop.load(Ordering::Relaxed) {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = m;
                if score > alpha {
                    alpha = score;
                    self.pv[ply][ply] = m;
                    for j in ply + 1..self.pv_len[ply + 1] {
                        self.pv[ply][j] = self.pv[ply + 1][j];
                    }
                    self.pv_len[ply] = self.pv_len[ply + 1].max(ply + 1);
                    if score >= beta {
                        if !m.is_capture() {
                            if self.killers[ply][0] != m {
                                self.killers[ply][1] = self.killers[ply][0];
                                self.killers[ply][0] = m;
                            }
                            let h = &mut self.history[stm][m.from_sq() as usize][m.to_sq() as usize];
                            *h = (*h + depth * depth).min(700_000);
                        }
                        break;
                    }
                }
            }
        }

        let bound = if best_score >= beta {
            Bound::Lower
        } else if alpha > original_alpha {
            Bound::Exact
        } else {
            Bound::Upper
        };
        let stored_move = if bound == Bound::Upper { Move::NULL } else { best_move };
        self.tt.store(key, stored_move, best_score, depth, bound, ply);
        best_score
    }

    /// Captures-only search at the horizon so the score does not depend on who made the
    /// last capture (otherwise odd and even depths flip between winning and losing material).
    fn quiescence(&mut self, ply: usize, mut alpha: i32, beta: i32) -> i32 {
        self.nodes += 1;
        self.seldepth = self.seldepth.max(ply);
        if self.should_stop() {
            return 0;
        }

        let stand_pat = evaluate(self.board);
        if stand_pat >= beta || ply >= MAX_PLY - 1 {
            return stand_pat;
        }
        alpha = alpha.max(stand_pat);

        let mut list = MoveList::new();
        self.board.generate_legal_moves(&mut list);
        let n = list.count;
        let mut scores = [0i32; 256];
        for (i, &m) in list.moves[..n].iter().enumerate() {
            scores[i] = if m.is_capture() { mvv_lva(self.board, m) } else { i32::MIN };
        }

        let mut best = stand_pat;
        for i in 0..n {
            pick_move(&mut list.moves[..n], &mut scores[..n], i);
            let m = list.moves[i];
            if !m.is_capture() {
                break;
            }
            let undo = self.board.make_move(m);
            let score = -self.quiescence(ply + 1, -beta, -alpha);
            self.board.unmake_move(m, &undo);

            if self.stop.load(Ordering::Relaxed) {
                return 0;
            }
            if score > best {
                best = score;
                if score >= beta {
                    break;
                }
                alpha = alpha.max(score);
            }
        }
        best
    }
}

/// Executes synchronous search with iterative deepening, using a fresh 16 MB table.
pub fn search_sync(
    board: &mut Board,
    limits: &SearchLimits,
    stop_flag: &AtomicBool,
    info_callback: Option<&dyn Fn(&SearchInfo)>,
) -> SearchResult {
    let mut tt = TranspositionTable::new(16);
    search(board, limits, stop_flag, info_callback, &mut tt)
}

/// Iterative deepening with aspiration windows. Emits progress via `info_callback`.
pub fn search(
    board: &mut Board,
    limits: &SearchLimits,
    stop_flag: &AtomicBool,
    info_callback: Option<&dyn Fn(&SearchInfo)>,
    tt: &mut TranspositionTable,
) -> SearchResult {
    tt.new_search();
    let mut root_moves = MoveList::new();
    board.generate_legal_moves(&mut root_moves);

    let mut result = SearchResult {
        best_move: root_moves.as_slice().first().copied().unwrap_or(Move::NULL),
        score: if root_moves.is_empty() && board.is_in_check() { -MATE_SCORE } else { 0 },
        depth: 0,
        nodes: 0,
    };

    if !root_moves.is_empty() {
        let mut s = Searcher {
            board,
            tt,
            limits,
            stop: stop_flag,
            start: Instant::now(),
            nodes: 0,
            seldepth: 0,
            killers: [[Move::NULL; 2]; MAX_PLY],
            history: Box::new([[[0; 64]; 64]; 2]),
            pv: Box::new([[Move::NULL; MAX_PLY]; MAX_PLY]),
            pv_len: [0; MAX_PLY],
        };
        if let Some(allowed) = limits.searchmoves.as_ref().and_then(|a| a.first()) {
            result.best_move = *allowed;
        }

        let max_depth = limits.depth.unwrap_or(MAX_PLY - 8).min(MAX_PLY - 8);
        for depth in 1..=max_depth {
            s.seldepth = 0;
            // Aspiration window around the last score from depth 4 on; widen on failure.
            let mut delta = 25;
            let (mut alpha, mut beta) = if depth >= 4 {
                ((result.score - delta).max(-INF), (result.score + delta).min(INF))
            } else {
                (-INF, INF)
            };
            let score = loop {
                let score = s.negamax(depth as i32, 0, alpha, beta);
                if stop_flag.load(Ordering::Relaxed) {
                    break score;
                }
                if score <= alpha {
                    alpha = (score - delta).max(-INF);
                } else if score >= beta {
                    beta = (score + delta).min(INF);
                } else {
                    break score;
                }
                delta *= 2;
            };

            // A depth cut short by stop/limits is discarded.
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            result.best_move = s.pv[0][0];
            result.score = score;
            result.depth = depth;

            let elapsed = s.start.elapsed();
            if let Some(callback) = info_callback {
                callback(&SearchInfo {
                    depth,
                    seldepth: s.seldepth.max(depth),
                    score_str: format_score(score),
                    nodes: s.nodes,
                    nps: calculate_nps(s.nodes, elapsed),
                    time_ms: elapsed.as_millis() as u64,
                    hashfull: s.tt.hashfull(),
                    pv: s.pv[0][..s.pv_len[0]].to_vec(),
                });
            }

            if limits.nodes.is_some_and(|max| s.nodes >= max) || score.abs() > MATE_THRESHOLD && limits.depth.is_none() && !limits.infinite {
                break;
            }
            // Check soft time limit: the next iteration costs several times the last one,
            // so do not start it once half the soft budget is gone.
            if limits.time_limits.soft_limit.is_some_and(|soft| elapsed >= soft / 2) {
                break;
            }
        }
        result.nodes = s.nodes;
    }

    // On infinite or ponder, wait until stop_flag is signaled before finishing
    if limits.infinite || limits.ponder {
        while !stop_flag.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    result
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

    fn scores_by_depth(fen: &str, depth: usize) -> Vec<String> {
        let mut board = Board::from_fen(fen).unwrap();
        let limits = SearchLimits {
            depth: Some(depth),
            ..Default::default()
        };
        let scores = std::cell::RefCell::new(Vec::new());
        let cb = |info: &SearchInfo| scores.borrow_mut().push(info.score_str.clone());
        search_sync(&mut board, &limits, &AtomicBool::new(false), Some(&cb));
        scores.into_inner()
    }

    #[test]
    fn test_score_does_not_flip_with_depth() {
        // Material-only eval + quiescence: startpos is level at every depth.
        let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert_eq!(scores_by_depth(start, 5), vec!["cp 0"; 5]);
    }

    #[test]
    fn test_score_is_from_side_to_move() {
        // White is a queen up (no captures available): + for White to move, - for Black to move.
        for (stm, sign) in [("w", 1), ("b", -1)] {
            for score in scores_by_depth(&format!("4k3/pppp4/8/8/8/8/PPPP4/3QK3 {stm} - - 0 1"), 4) {
                let cp: i32 = score.strip_prefix("cp ").unwrap().parse().unwrap();
                assert!(cp * sign >= 800, "{stm} to move: {score}");
            }
        }
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
