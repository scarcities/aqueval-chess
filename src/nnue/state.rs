//! Accumulators along the current search path, updated lazily.
//!
//! `push` (before `make_move`) only records which features the move adds and removes;
//! the accumulators are brought up to date when a position is actually evaluated, by
//! walking up from the deepest entry that is already computed. Nodes that are pruned
//! before their static eval never pay for an update.

use std::sync::Arc;

use super::simd::{has_avx2, update};
use super::{feature_index, Accumulator, Network};
use crate::board::Board;
use crate::movegen::types::Move;
use crate::types::{Color, Piece, PieceType};

/// Deepest path the search can build: 128 plies of main search plus quiescence, which
/// only makes captures (at most 30 on a board).
const STACK_SIZE: usize = 256;

/// Feature changes of one move, as indices for [White's view, Black's view].
#[derive(Clone, Copy, Default)]
struct Delta {
    adds: [[u16; 2]; 2],
    subs: [[u16; 2]; 2],
    n_adds: u8,
    n_subs: u8,
}

impl Delta {
    fn add(&mut self, piece: Piece, sq: u8) {
        self.adds[self.n_adds as usize] = Self::indices(piece, sq);
        self.n_adds += 1;
    }

    fn sub(&mut self, piece: Piece, sq: u8) {
        self.subs[self.n_subs as usize] = Self::indices(piece, sq);
        self.n_subs += 1;
    }

    fn indices(piece: Piece, sq: u8) -> [u16; 2] {
        [Color::White, Color::Black].map(|c| feature_index(c, piece, sq) as u16)
    }
}

#[derive(Clone, Copy)]
struct Entry {
    /// Indexed by colour (White, Black), not by side to move.
    acc: [Accumulator; 2],
    /// Changes from the previous entry to this one.
    delta: Delta,
    computed: bool,
}

pub struct NnueState {
    net: Arc<Network>,
    stack: Vec<Entry>,
    top: usize,
    avx2: bool,
}

impl NnueState {
    /// Allocates the stack once (per search) and computes the root accumulators.
    pub fn new(net: Arc<Network>, board: &Board) -> Self {
        let empty = Entry { acc: [Accumulator::ZERO; 2], delta: Delta::default(), computed: false };
        let mut state = Self { net, stack: vec![empty; STACK_SIZE], top: 0, avx2: has_avx2() };
        state.reset(board);
        state
    }

    /// Recomputes the bottom entry from `board` and forgets the path above it.
    pub fn reset(&mut self, board: &Board) {
        self.top = 0;
        let root = &mut self.stack[0];
        root.acc = [Color::White, Color::Black].map(|c| self.net.refresh(board, c));
        root.computed = true;
    }

    /// Records `m`, about to be made on `board` (call before `make_move`).
    #[inline]
    pub fn push(&mut self, board: &Board, m: Move) {
        let from = m.from_sq();
        let to = m.to_sq();
        let piece = board.piece_at(from).expect("moving piece must exist on board");
        let us = piece.color;
        let mut delta = Delta::default();
        if m.is_castle() {
            let (rook_from, rook_to) = match (us, m.is_king_castle()) {
                (Color::White, true) => (7, 5),
                (Color::White, false) => (0, 3),
                (Color::Black, true) => (63, 61),
                (Color::Black, false) => (56, 59),
            };
            let rook = Piece::new(us, PieceType::Rook);
            delta.sub(piece, from);
            delta.add(piece, to);
            delta.sub(rook, rook_from);
            delta.add(rook, rook_to);
        } else {
            delta.sub(piece, from);
            let placed = m.promotion_piece().map_or(piece, |pt| Piece::new(us, pt));
            delta.add(placed, to);
            if m.is_en_passant() {
                let victim_sq = if us == Color::White { to - 8 } else { to + 8 };
                delta.sub(Piece::new(!us, PieceType::Pawn), victim_sq);
            } else if let Some(captured) = board.piece_at(to) {
                delta.sub(captured, to);
            }
        }
        self.top += 1;
        debug_assert!(self.top < STACK_SIZE, "NNUE stack overflow");
        let e = &mut self.stack[self.top];
        e.delta = delta;
        e.computed = false;
    }

    /// Drops the entry of the move just unmade (call after `unmake_move`).
    #[inline]
    pub fn pop(&mut self) {
        self.top -= 1;
    }

    /// Static evaluation of `board` (the position at the top of the stack), side to move's view.
    #[inline]
    pub fn evaluate(&mut self, board: &Board) -> i32 {
        self.catch_up();
        let e = &self.stack[self.top];
        let stm = board.side_to_move.index();
        self.net.output(&e.acc[stm], &e.acc[stm ^ 1], self.avx2)
    }

    /// Computes every entry above the deepest computed one, up to the top.
    fn catch_up(&mut self) {
        let mut first = self.top;
        while !self.stack[first].computed {
            first -= 1;
        }
        for i in first + 1..=self.top {
            let (below, above) = self.stack.split_at_mut(i);
            let (prev, cur) = (&below[i - 1], &mut above[0]);
            let d = cur.delta;
            let w = &self.net.feature_weights;
            for c in 0..2 {
                let add = |k: usize| &w[d.adds[k][c] as usize];
                let sub = |k: usize| &w[d.subs[k][c] as usize];
                let (dst, src) = (&mut cur.acc[c], &prev.acc[c]);
                match (d.n_adds, d.n_subs) {
                    (1, 1) => update(self.avx2, dst, src, [add(0)], [sub(0)]),
                    (1, 2) => update(self.avx2, dst, src, [add(0)], [sub(0), sub(1)]),
                    (2, 2) => update(self.avx2, dst, src, [add(0), add(1)], [sub(0), sub(1)]),
                    _ => unreachable!("a move adds 1-2 and removes 1-2 features"),
                }
            }
            cur.computed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::zobrist::SplitMix64;
    use crate::movegen::types::MoveList;
    use crate::nnue::tests::random_net;

    /// Random walks through positions with castling, en passant, promotions and captures;
    /// the incremental eval must equal a full refresh after every make and unmake.
    #[test]
    fn incremental_matches_refresh() {
        let net: Arc<Network> = Arc::from(random_net(5));
        let mut rng = SplitMix64::new(99);
        for fen in [
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ] {
            for _ in 0..20 {
                let mut board = Board::from_fen(fen).unwrap();
                let mut state = NnueState::new(net.clone(), &board);
                let mut path = Vec::new();
                for _ in 0..40 {
                    let mut moves = MoveList::new();
                    board.generate_legal_moves(&mut moves);
                    if moves.is_empty() {
                        break;
                    }
                    let m = moves.as_slice()[(rng.next_u64() % moves.len() as u64) as usize];
                    state.push(&board, m);
                    let undo = board.make_move(m);
                    path.push((m, undo));
                    // Evaluate only sometimes, so several lazy deltas pile up in between.
                    if rng.next_u64().is_multiple_of(3) {
                        assert_eq!(state.evaluate(&board), net.evaluate_from_scratch(&board), "{}", board.to_fen());
                    }
                }
                while let Some((m, undo)) = path.pop() {
                    board.unmake_move(m, &undo);
                    state.pop();
                    if rng.next_u64().is_multiple_of(4) {
                        assert_eq!(state.evaluate(&board), net.evaluate_from_scratch(&board), "{}", board.to_fen());
                    }
                }
                assert_eq!(state.evaluate(&board), net.evaluate_from_scratch(&board));
            }
        }
    }
}
