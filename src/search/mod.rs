pub mod evaluate;
pub mod evaluator;
pub mod time;
pub mod see;
pub mod tt;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::board::Board;
use crate::movegen::types::{Move, MoveList};
use crate::perft::calculate_nps;
use crate::search::evaluate::piece_value;
use crate::search::evaluator::Evaluator;
use crate::types::{Color, PieceType};
use crate::search::time::TimeLimits;
use crate::search::see::see_ge;
use crate::search::tt::{Bound, TranspositionTable};

pub const MATE_SCORE: i32 = 30000;
pub const MATE_THRESHOLD: i32 = 25000;

#[derive(Clone, Debug, Default)]
pub struct SearchLimits {
    pub depth: Option<usize>,
    pub nodes: Option<u64>,
    /// Do not start another iteration once this many nodes are searched (data generation).
    pub soft_nodes: Option<u64>,
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
    tt: &'a mut TranspositionTable,
    heur: &'a mut Heuristics,
    eval: &'a mut Evaluator,
    /// `board.history.len()` at the root: history entries from here on belong to the search path.
    root_history: usize,
}

const MAX_PLY: usize = 128;
const CONT_MAX: i32 = 16384;
const NO_PIECE: usize = 12;
const NO_EVAL: i32 = i32::MIN;
const CORR_SIZE: usize = 16384;
const CORR_GRAIN: i32 = 256;
const CORR_MAX: i32 = 128 * CORR_GRAIN;

/// Index of the current pawn structure in the correction table (14-bit multiplicative hash).
fn pawn_index(board: &Board) -> usize {
    let pawns = board.pieces[PieceType::Pawn.index()];
    let w = (pawns & board.colors[Color::White.index()]).0;
    let b = (pawns & board.colors[Color::Black.index()]).0;
    ((w.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)) >> 50) as usize
}

/// Quiet-move ordering data for one search: two killers per ply and butterfly history
/// indexed by [side][from][to]. Boxed once per `go`, never allocated in the tree.
struct Heuristics {
    killers: [[Move; 2]; MAX_PLY],
    history: [[[i32; 64]; 64]; 2],
    /// Move made at each ply on the current path (NULL for a null move), for counter moves.
    moves_at: [Move; MAX_PLY],
    /// Counter move: the quiet move that last refuted [previous piece (colour·6 + type)][previous to].
    counter: [[Move; 64]; 12],
    /// Moving piece (colour·6 + type) of `moves_at[ply]`; NO_PIECE for a null move.
    piece_at_ply: [usize; MAX_PLY],
    /// Static eval per ply on the current path (NO_EVAL when in check), for "improving".
    eval_at: [i32; MAX_PLY],
    /// Pawn-structure correction history [side][pawn hash]: learned (search − static eval)
    /// in 1/CORR_GRAIN centipawns, added to the static eval.
    /// shortcut: reset every `go` with the other heuristics; keep it across moves like the TT once it has passed SPRT.
    corr: Vec<i32>,
    /// Continuation history [prev piece][prev to][piece][to], flat, shared by the 1- and 2-ply lookups.
    cont: Vec<i32>,
    /// `null_at[ply]`: the move made at `ply` was a null move (no two in a row).
    null_at: [bool; MAX_PLY],
}

impl Heuristics {
    fn new() -> Box<Self> {
        Box::new(Self {
            killers: [[Move::NULL; 2]; MAX_PLY],
            history: [[[0; 64]; 64]; 2],
            moves_at: [Move::NULL; MAX_PLY],
            counter: [[Move::NULL; 64]; 12],
            piece_at_ply: [NO_PIECE; MAX_PLY],
            eval_at: [NO_EVAL; MAX_PLY],
            corr: vec![0; 2 * CORR_SIZE],
            cont: vec![0; 12 * 64 * 12 * 64],
            null_at: [false; MAX_PLY],
        })
    }

    /// Records a quiet move that caused a beta cutoff.
    fn update(&mut self, side: usize, ply: usize, m: Move, depth: usize) {
        if self.killers[ply][0] != m {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = m;
        }
        let h = &mut self.history[side][m.from_sq() as usize][m.to_sq() as usize];
        // Capped below the killer scores so killers stay ahead of history.
        *h = (*h + (depth * depth) as i32).min(700_000);
    }

    /// Slot of the counter move to the previous move, if there was a real previous move.
    fn counter_slot(&mut self, board: &Board, ply: usize) -> Option<&mut Move> {
        let prev = *self.moves_at.get(ply.checked_sub(1)?)?;
        let piece = board.piece_at(prev.to_sq()).filter(|_| prev != Move::NULL)?;
        Some(&mut self.counter[piece.color.index() * 6 + piece.piece_type.index()][prev.to_sq() as usize])
    }

    /// Records the move about to be made at `ply` (NULL for a null move).
    fn record(&mut self, board: &Board, ply: usize, m: Move) {
        self.moves_at[ply] = m;
        self.piece_at_ply[ply] = board
            .piece_at(m.from_sq())
            .filter(|_| m != Move::NULL)
            .map_or(NO_PIECE, |p| p.color.index() * 6 + p.piece_type.index());
    }

    /// Index into `cont` for move `m` following the move made `back` plies earlier, if any.
    fn cont_index(&self, board: &Board, ply: usize, back: usize, m: Move) -> Option<usize> {
        let prev_ply = ply.checked_sub(back)?;
        let prev_piece = self.piece_at_ply[prev_ply];
        let piece = board.piece_at(m.from_sq())?;
        (prev_piece != NO_PIECE).then(|| {
            let prev_to = self.moves_at[prev_ply].to_sq() as usize;
            let cur = piece.color.index() * 6 + piece.piece_type.index();
            ((prev_piece * 64 + prev_to) * 12 + cur) * 64 + m.to_sq() as usize
        })
    }

    fn cont_score(&self, board: &Board, ply: usize, m: Move) -> i32 {
        [1, 2].iter().filter_map(|&b| self.cont_index(board, ply, b, m)).map(|i| self.cont[i]).sum()
    }

    /// Rewards the quiet move that failed high and punishes the quiets tried before it,
    /// for the 1-ply and 2-ply continuations.
    fn update_cont(&mut self, board: &Board, ply: usize, best: Move, tried: &[Move], depth: usize) {
        let bonus = (16 * depth * depth).min(1200) as i32;
        for &m in tried {
            let b = if m == best { bonus } else { -bonus };
            for back in [1, 2] {
                if let Some(i) = self.cont_index(board, ply, back, m) {
                    let h = &mut self.cont[i];
                    // Gravity: entries saturate at ±CONT_MAX instead of growing without bound.
                    *h += b - *h * b.abs() / CONT_MAX;
                }
            }
        }
    }

    fn corr_slot(&mut self, board: &Board) -> &mut i32 {
        &mut self.corr[board.side_to_move.index() * CORR_SIZE + pawn_index(board)]
    }

    fn correction(&mut self, board: &Board) -> i32 {
        *self.corr_slot(board) / CORR_GRAIN
    }

    /// Moves the correction for this pawn structure toward (search score − raw eval),
    /// weighted by depth.
    fn update_corr(&mut self, board: &Board, static_eval: i32, score: i32, depth: usize) {
        if score.abs() >= MATE_THRESHOLD {
            return;
        }
        let raw = static_eval - *self.corr_slot(board) / CORR_GRAIN;
        let w = (depth as i32 + 1).min(16);
        let c = self.corr_slot(board);
        *c = ((*c * (256 - w) + (score - raw) * CORR_GRAIN * w) / 256).clamp(-CORR_MAX, CORR_MAX);
    }

    /// Ordering key: TT/best move, good captures (SEE ≥ 0, by MVV-LVA), killers, quiet moves
    /// by history, then losing captures last.
    fn order_key(&self, board: &Board, m: Move, tt_move: Move, counter: Move, ply: usize) -> i32 {
        if m == tt_move {
            i32::MAX
        } else if m.is_capture() {
            let base = if see_ge(board, m, 0) { 1_000_000 } else { -1_000_000 };
            base + mvv_lva(board, m)
        } else if m == self.killers[ply][0] {
            900_000
        } else if m == self.killers[ply][1] {
            800_000
        } else if m == counter {
            750_000
        } else {
            self.history[board.side_to_move.index()][m.from_sq() as usize][m.to_sq() as usize]
                + 2 * self.cont_score(board, ply, m)
        }
    }
}

/// Recursively evaluates positions using alpha-beta pruning.
/// Zero heap allocations in recursion.
fn alpha_beta(
    board: &mut Board,
    mut depth: usize,
    ply: usize,
    mut alpha: i32,
    beta: i32,
    ctx: &mut SearchContext,
) -> i32 {
    ctx.stats.nodes += 1;

    // Periodic time and node limit check (every 1024 nodes)
    if (ctx.stats.nodes & 1023) == 0 && should_stop(ctx) {
        return 0;
    }

    // Check extension: search one ply deeper when in check (also stops qsearch at depth 0
    // from standing pat in check).
    let in_check = board.is_in_check();
    if in_check {
        depth += 1;
    }

    if depth == 0 {
        return quiescence(board, alpha, beta, ctx);
    }
    // Extensions can lengthen lines; the ply tables end at MAX_PLY.
    if ply >= MAX_PLY - 1 {
        return ctx.eval.evaluate(board);
    }

    // 50-move rule, or a repetition: once inside the search path, threefold before the root.
    if board.halfmove_clock >= 100 || is_search_repetition(board, ctx.root_history) {
        return 0;
    }

    let key = board.zobrist_hash();
    let mut tt_move = Move::NULL;
    if let Some(e) = ctx.tt.probe(key, ply) {
        tt_move = e.mv;
        let score = e.score as i32;
        // Cut only at null-window nodes so the PV keeps coming from real search, and not near
        // the 50-move rule, where the stored score may ignore a draw that is now close.
        if beta - alpha == 1
            && board.halfmove_clock < 90
            && e.depth as usize >= depth
            && match e.bound {
                Bound::Exact => true,
                Bound::Lower => score >= beta,
                Bound::Upper => score <= alpha,
            }
        {
            return score;
        }
    }

    let prunable = beta - alpha == 1 && ply > 0 && !in_check;
    let static_eval = if in_check { NO_EVAL } else { ctx.eval.evaluate(board) + ctx.heur.correction(board) };
    ctx.heur.eval_at[ply] = static_eval;
    // "Improving": static eval is higher than at our previous turn (two plies back, or four
    // if we were in check then). Positions getting worse are pruned and reduced more.
    let improving = !in_check
        && [2, 4]
            .iter()
            .filter_map(|&b| ply.checked_sub(b).map(|p| ctx.heur.eval_at[p]))
            .find(|&e| e != NO_EVAL)
            .is_none_or(|e| static_eval > e);

    // Reverse futility pruning: near the leaves, a static eval far above beta holds.
    if prunable && depth <= 6 && beta.abs() < MATE_THRESHOLD && static_eval - 80 * (depth as i32 - improving as i32) >= beta {
        return static_eval;
    }

    // Null move pruning: if passing still fails high, a real move will too. Not in PV nodes,
    // in check, twice in a row, or without pieces (pawn endgames are prone to zugzwang).
    if prunable
        && depth >= 3
        && !ctx.heur.null_at[ply - 1]
        && has_non_pawn_material(board)
        && static_eval >= beta
    {
        let r = 3 + depth / 4;
        board.make_null_move();
        ctx.heur.null_at[ply] = true;
        ctx.heur.record(board, ply, Move::NULL);
        let score = -alpha_beta(board, depth.saturating_sub(1 + r), ply + 1, -beta, -beta + 1, ctx);
        ctx.heur.null_at[ply] = false;
        board.unmake_null_move();
        if ctx.stop_flag.load(Ordering::Relaxed) {
            return 0;
        }
        if score >= beta {
            return if score >= MATE_THRESHOLD { beta } else { score };
        }
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

    let counter = ctx.heur.counter_slot(board, ply).map_or(Move::NULL, |c| *c);
    // Keys computed once per move (SEE is not free), sorted on the stack: no allocation.
    let n = moves.count;
    let mut keyed = [(0i32, Move::NULL); 256];
    for (slot, &m) in keyed.iter_mut().zip(moves.as_slice()) {
        *slot = (ctx.heur.order_key(board, m, tt_move, counter, ply), m);
    }
    keyed[..n].sort_unstable_by_key(|&(k, _)| std::cmp::Reverse(k));
    for (dst, &(_, m)) in moves.moves[..n].iter_mut().zip(&keyed[..n]) {
        *dst = m;
    }

    let mut best_move = Move::NULL;
    let mut best_score = -MATE_SCORE;
    let pv_node = beta - alpha > 1;
    let mut tried_quiets = [Move::NULL; 64];
    let mut quiets_tried = 0;
    for (i, &m) in moves.as_slice().iter().enumerate() {
        let quiet = !m.is_capture() && m.promotion_piece().is_none() && !ctx.heur.killers[ply].contains(&m);
        // Late move pruning: at shallow null-window nodes skip quiet moves after the first
        // (3 + depth²) / 2, or 3 + depth² when the position is improving.
        if prunable && depth <= 6 && quiet && alpha > -MATE_THRESHOLD && i >= (3 + depth * depth) / (2 - improving as usize) {
            continue;
        }
        // SEE pruning: at shallow null-window nodes skip moves that lose too much material.
        if prunable && i > 0 && depth <= 6 && alpha > -MATE_THRESHOLD {
            let margin = if m.is_capture() { -100 } else { -50 } * depth as i32;
            if !see_ge(board, m, margin) {
                continue;
            }
        }
        ctx.heur.record(board, ply, m);
        let undo = ctx.eval.make_move(board, m);
        // Futility pruning: at frontier null-window nodes whose static eval is far below
        // alpha, quiet moves that do not give check cannot catch up.
        if prunable
            && depth <= 4
            && i > 0
            && quiet
            && alpha > -MATE_THRESHOLD
            && static_eval + 80 * depth as i32 + if improving { 80 } else { 40 } <= alpha
            && !board.is_in_check()
        {
            ctx.eval.unmake_move(board, m, &undo);
            continue;
        }
        // Late move reductions: quiet, non-checking moves late in the list get a shallower
        // null-window search (log formula, one ply less at PV nodes).
        let mut reduction = 0;
        if depth >= 3 && i >= 3 && quiet && !in_check && !board.is_in_check() {
            let r = 0.75 + (depth as f64).ln() * (i as f64).ln() / 2.25;
            reduction = (r as usize + !improving as usize).saturating_sub(pv_node as usize).min(depth - 2);
        }
        let score = pvs_child(board, depth - 1, ply + 1, alpha, beta, (i > 0).then_some(reduction), ctx);
        ctx.eval.unmake_move(board, m, &undo);

        if ctx.stop_flag.load(Ordering::Relaxed) {
            return 0;
        }

        if !m.is_capture() && quiets_tried < tried_quiets.len() {
            tried_quiets[quiets_tried] = m;
            quiets_tried += 1;
        }
        if score >= beta {
            if !m.is_capture() {
                ctx.heur.update(board.side_to_move.index(), ply, m, depth);
                if let Some(c) = ctx.heur.counter_slot(board, ply) {
                    *c = m;
                }
                ctx.heur.update_cont(board, ply, m, &tried_quiets[..quiets_tried], depth);
            }
            if !in_check && !m.is_capture() && score > static_eval {
                ctx.heur.update_corr(board, static_eval, score, depth);
            }
            ctx.tt.store(key, m, score, depth as i32, Bound::Lower, ply);
            return score;
        }
        best_score = best_score.max(score);
        if score > alpha {
            alpha = score;
            best_move = m;
        }
    }
    // Every move pruned: fall back to the fail-hard bound.
    if best_score == -MATE_SCORE {
        best_score = alpha;
    }

    let bound = if best_move == Move::NULL { Bound::Upper } else { Bound::Exact };
    let quiet_best = best_move == Move::NULL || !best_move.is_capture();
    if !in_check && quiet_best && (bound == Bound::Exact || best_score < static_eval) {
        ctx.heur.update_corr(board, static_eval, best_score, depth);
    }
    ctx.tt.store(key, best_move, best_score, depth as i32, bound, ply);
    best_score
}

/// Principal variation search for one child (already made on `board`), from the parent's view:
/// the first move gets the full window, later moves a null window that is re-searched
/// only if it beats alpha.
fn pvs_child(
    board: &mut Board,
    depth: usize,
    ply: usize,
    alpha: i32,
    beta: i32,
    // None: first move, full window. Some(r): later move, null window reduced by r plies.
    late: Option<usize>,
    ctx: &mut SearchContext,
) -> i32 {
    let Some(reduction) = late else {
        return -alpha_beta(board, depth, ply, -beta, -alpha, ctx);
    };
    let mut score = -alpha_beta(board, depth - reduction, ply, -alpha - 1, -alpha, ctx);
    // A reduced move that beats alpha is re-searched at full depth first.
    if score > alpha && reduction > 0 {
        score = -alpha_beta(board, depth, ply, -alpha - 1, -alpha, ctx);
    }
    if score > alpha && score < beta {
        -alpha_beta(board, depth, ply, -beta, -alpha, ctx)
    } else {
        score
    }
}

fn has_non_pawn_material(board: &Board) -> bool {
    let ours = board.colors[board.side_to_move.index()];
    let pawns_kings = board.pieces[PieceType::Pawn.index()] | board.pieces[PieceType::King.index()];
    (ours & !pawns_kings).is_not_empty()
}

/// Draw by repetition for the search: any earlier occurrence of this position on the search
/// path (history index >= `root_history`) counts, game positions before the root need two.
fn is_search_repetition(board: &Board, root_history: usize) -> bool {
    let h = &board.history;
    let key = board.zobrist_hash();
    let limit = h.len().saturating_sub(board.halfmove_clock as usize);
    let mut before_root = 0;
    for i in (limit..h.len()).rev().skip(1).step_by(2) {
        if h[i].zobrist_hash == key {
            if i >= root_history {
                return true;
            }
            before_root += 1;
            if before_root >= 2 {
                return true;
            }
        }
    }
    false
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

/// Captures-only search at the horizon so the score does not depend on who made the
/// last capture (otherwise odd and even depths flip between winning and losing material).
fn quiescence(board: &mut Board, mut alpha: i32, beta: i32, ctx: &mut SearchContext) -> i32 {
    ctx.stats.nodes += 1;
    if (ctx.stats.nodes & 1023) == 0 && should_stop(ctx) {
        return 0;
    }

    let stand_pat = ctx.eval.evaluate(board);
    if stand_pat >= beta {
        return stand_pat;
    }
    alpha = alpha.max(stand_pat);
    let mut best_score = stand_pat;

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);
    let captures = &mut moves.moves[..moves.count];
    captures.sort_unstable_by_key(|&m| std::cmp::Reverse(if m.is_capture() { mvv_lva(board, m) } else { i32::MIN }));

    for &m in captures.iter().take_while(|m| m.is_capture()) {
        // Captures that lose material cannot raise a stand-pat score.
        if !see_ge(board, m, 0) {
            continue;
        }
        let undo = ctx.eval.make_move(board, m);
        let score = -quiescence(board, -beta, -alpha, ctx);
        ctx.eval.unmake_move(board, m, &undo);

        if ctx.stop_flag.load(Ordering::Relaxed) {
            return 0;
        }
        if score >= beta {
            return score;
        }
        best_score = best_score.max(score);
        alpha = alpha.max(score);
    }
    best_score
}

/// Time/node limit check shared by the main search and quiescence; sets the stop flag.
fn should_stop(ctx: &SearchContext) -> bool {
    let out_of_time = ctx
        .limits
        .time_limits
        .hard_limit
        .is_some_and(|hard| ctx.stats.start_time.elapsed() >= hard);
    let out_of_nodes = ctx.limits.nodes.is_some_and(|max| ctx.stats.nodes >= max);
    if out_of_time || out_of_nodes {
        ctx.stop_flag.store(true, Ordering::Relaxed);
    }
    ctx.stop_flag.load(Ordering::Relaxed)
}

/// Executes synchronous search with iterative deepening.
/// Emits search progress via `info_callback` if provided.
/// Runs `search` with a fresh 16 MB transposition table (bench, tests).
pub fn search_sync(
    board: &mut Board,
    limits: &SearchLimits,
    stop_flag: &AtomicBool,
    info_callback: Option<&dyn Fn(&SearchInfo)>,
) -> SearchResult {
    search(board, limits, stop_flag, info_callback, &mut TranspositionTable::new(16))
}

/// Iterative deepening search; `tt` persists between calls (UCI keeps it across moves).
pub fn search(
    board: &mut Board,
    limits: &SearchLimits,
    stop_flag: &AtomicBool,
    info_callback: Option<&dyn Fn(&SearchInfo)>,
    tt: &mut TranspositionTable,
) -> SearchResult {
    tt.new_search();
    let mut stats = SearchStats {
        nodes: 0,
        start_time: Instant::now(),
    };
    let mut heur = Heuristics::new();
    let mut evaluator = Evaluator::new(board);
    let root_history = board.history.len();

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
        // Aspiration window around the last score from depth 4; it widens by half on each
        // fail, and a fail-high is re-searched one ply shallower (at most two plies).
        let mut delta = 20;
        let (mut alpha_w, mut beta_w) = if current_depth >= 4 {
            ((best_score - delta).max(-32000), (best_score + delta).min(32000))
        } else {
            (-32000, 32000)
        };
        let mut fail_highs = 0;

        // Order root moves: place current best_move first, then captures
        candidate_moves.sort_by_key(|&m| std::cmp::Reverse(heur.order_key(board, m, best_move, Move::NULL, 0)));

        let mut ctx = SearchContext {
            stop_flag,
            limits,
            stats: &mut stats,
            tt: &mut *tt,
            heur: &mut heur,
            eval: &mut evaluator,
            root_history,
        };

        let (depth_best_move, depth_best_score) = loop {
            let search_depth = current_depth.saturating_sub(fail_highs).max(1);
            let mut alpha = alpha_w;
            let mut depth_best_move = candidate_moves[0];
            let mut depth_best_score = -32000;

            for (i, &m) in candidate_moves.iter().enumerate() {
                ctx.heur.eval_at[0] = if board.is_in_check() { NO_EVAL } else { ctx.eval.evaluate(board) };
                ctx.heur.record(board, 0, m);
                let undo = ctx.eval.make_move(board, m);
                let score = pvs_child(board, search_depth - 1, 1, alpha, beta_w, (i > 0).then_some(0), &mut ctx);
                ctx.eval.unmake_move(board, m, &undo);

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
                if score >= beta_w {
                    break;
                }
            }

            if stop_flag.load(Ordering::Relaxed) {
                break (depth_best_move, depth_best_score);
            }
            if depth_best_score <= alpha_w && alpha_w > -32000 {
                beta_w = (alpha_w + beta_w) / 2;
                alpha_w = (depth_best_score - delta).max(-32000);
                fail_highs = 0;
            } else if depth_best_score >= beta_w && beta_w < 32000 {
                beta_w = (depth_best_score + delta).min(32000);
                fail_highs = (fail_highs + 1).min(2);
                // Search the move that failed high first next time.
                let pos = candidate_moves.iter().position(|&m| m == depth_best_move).unwrap();
                candidate_moves[..=pos].rotate_right(1);
            } else {
                break (depth_best_move, depth_best_score);
            }
            delta += delta / 2;
        };

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
        if limits.soft_nodes.is_some_and(|soft| stats.nodes >= soft) {
            break;
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
        // With quiescence, startpos stays near level at every depth (only small tempo swings,
        // never a pawn up/down alternation).
        let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        for score in scores_by_depth(start, 5) {
            let cp: i32 = score.strip_prefix("cp ").unwrap().parse().unwrap();
            assert!(cp.abs() < 50, "startpos {score}");
        }
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

    #[test]
    fn test_search_repetition_rules() {
        let mut b = Board::startpos();
        let mv = |b: &Board, uci: &str| {
            let mut l = MoveList::new();
            b.generate_legal_moves(&mut l);
            *l.as_slice().iter().find(|m| m.to_uci() == uci).unwrap()
        };
        let shuffle = ["g1f3", "g8f6", "f3g1", "f6g8"];
        for u in shuffle {
            let m = mv(&b, u);
            b.make_move(m);
        }
        // Startpos has occurred twice in the game: one repeat before the root is not enough.
        assert!(!is_search_repetition(&b, b.history.len()));
        // The same repeat inside the search path counts.
        assert!(is_search_repetition(&b, 0));
        for u in shuffle {
            let m = mv(&b, u);
            b.make_move(m);
        }
        // Third occurrence before the root: draw.
        assert!(is_search_repetition(&b, b.history.len()));
    }
}