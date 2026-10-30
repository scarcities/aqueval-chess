use crate::movegen::types::Move;
use crate::search::MATE_THRESHOLD;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy, Debug)]
pub struct TtEntry {
    pub key: u64,
    pub mv: Move,
    pub score: i16,
    pub depth: i8,
    pub bound: Bound,
    pub age: u8,
}

const EMPTY: TtEntry = TtEntry {
    key: 0,
    mv: Move::NULL,
    score: 0,
    depth: -1,
    bound: Bound::Upper,
    age: 0,
};

/// Single-slot transposition table, allocated once per `Hash` setting (never in the search).
pub struct TranspositionTable {
    entries: Vec<TtEntry>,
    age: u8,
    /// Move-ordering and correction tables of the last search, kept with the TT so they live
    /// exactly as long as one game's hash (cleared by `clear`, fresh per datagen game and bench).
    pub(crate) heur: Option<Box<crate::search::Heuristics>>,
}

impl TranspositionTable {
    pub fn new(mb: usize) -> Self {
        let len = (mb.max(1) * 1024 * 1024 / std::mem::size_of::<TtEntry>()).max(1);
        Self {
            entries: vec![EMPTY; len],
            age: 0,
            heur: None,
        }
    }

    pub fn clear(&mut self) {
        self.entries.fill(EMPTY);
        self.age = 0;
        self.heur = None;
    }

    /// Called once per `go`, so entries from earlier moves lose replacement priority.
    pub fn new_search(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    #[inline(always)]
    fn index(&self, key: u64) -> usize {
        ((key as u128 * self.entries.len() as u128) >> 64) as usize
    }

    #[inline(always)]
    pub fn probe(&self, key: u64, ply: usize) -> Option<TtEntry> {
        let e = self.entries[self.index(key)];
        (e.key == key && e.depth >= 0).then(|| TtEntry {
            score: score_from_tt(e.score as i32, ply) as i16,
            ..e
        })
    }

    #[inline(always)]
    pub fn store(&mut self, key: u64, mv: Move, score: i32, depth: i32, bound: Bound, ply: usize) {
        let idx = self.index(key);
        let old = self.entries[idx];
        // Keep a deeper entry of the same search unless the new one is exact.
        if old.key != key || old.age != self.age || depth >= old.depth as i32 || bound == Bound::Exact {
            self.entries[idx] = TtEntry {
                key,
                // Keep the old best move when this search found none (fail-low).
                mv: if mv == Move::NULL && old.key == key { old.mv } else { mv },
                score: score_to_tt(score, ply) as i16,
                depth: depth.clamp(0, 127) as i8,
                bound,
                age: self.age,
            };
        }
    }

    /// Per-mille of slots written by the current search (UCI `hashfull`).
    pub fn hashfull(&self) -> u64 {
        let n = self.entries.len().min(1000);
        self.entries[..n].iter().filter(|e| e.depth >= 0 && e.age == self.age).count() as u64 * 1000 / n as u64
    }
}

// Mate scores are stored relative to the node, not the root, so they stay valid at other plies.
fn score_to_tt(score: i32, ply: usize) -> i32 {
    if score > MATE_THRESHOLD {
        score + ply as i32
    } else if score < -MATE_THRESHOLD {
        score - ply as i32
    } else {
        score
    }
}

fn score_from_tt(score: i32, ply: usize) -> i32 {
    if score > MATE_THRESHOLD {
        score - ply as i32
    } else if score < -MATE_THRESHOLD {
        score + ply as i32
    } else {
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_probe_and_mate_adjust() {
        let mut tt = TranspositionTable::new(1);
        let m = Move::new(12, 28, Move::DOUBLE_PAWN_PUSH);
        tt.store(42, m, 29_990, 5, Bound::Exact, 4);
        let e = tt.probe(42, 2).unwrap();
        assert_eq!((e.mv, e.depth, e.bound), (m, 5, Bound::Exact));
        // Mate in 10 plies from ply 4 is mate in 12 plies seen from ply 2.
        assert_eq!(e.score as i32, 29_990 + 2);
        assert!(tt.probe(43, 0).is_none());
        tt.clear();
        assert!(tt.probe(42, 0).is_none());
    }
}
