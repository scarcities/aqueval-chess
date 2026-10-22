//! Static evaluation as the search sees it. Every make/unmake and static eval in the search
//! goes through here, so that with the `nnue` feature and a loaded net the accumulators
//! follow the search path. Without the feature this is the handcrafted eval and plain
//! make/unmake, and compiles to the same code as calling them directly.

use crate::board::make_move::UndoState;
use crate::board::Board;
use crate::movegen::types::Move;
use crate::search::evaluate::evaluate;

pub struct Evaluator {
    #[cfg(feature = "nnue")]
    nnue: Option<crate::nnue::NnueState>,
}

impl Evaluator {
    /// Evaluator for a search from `board`: NNUE when a net is active, otherwise the HCE.
    pub fn new(board: &Board) -> Self {
        let _ = board;
        Self {
            #[cfg(feature = "nnue")]
            nnue: crate::nnue::active().map(|net| crate::nnue::NnueState::new(net, board)),
        }
    }

    #[inline(always)]
    pub fn make_move(&mut self, board: &mut Board, m: Move) -> UndoState {
        #[cfg(feature = "nnue")]
        if let Some(n) = &mut self.nnue {
            n.push(board, m);
        }
        board.make_move(m)
    }

    #[inline(always)]
    pub fn unmake_move(&mut self, board: &mut Board, m: Move, undo: &UndoState) {
        board.unmake_move(m, undo);
        #[cfg(feature = "nnue")]
        if let Some(n) = &mut self.nnue {
            n.pop();
        }
    }

    /// Static eval in centipawns from the side to move's view.
    #[inline(always)]
    pub fn evaluate(&mut self, board: &Board) -> i32 {
        #[cfg(feature = "nnue")]
        if let Some(n) = &mut self.nnue {
            return n.evaluate(board);
        }
        evaluate(board)
    }
}
