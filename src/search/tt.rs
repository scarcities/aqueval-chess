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

const BUCKET: usize = 2;

/// Two entries per 32-byte bucket: slot 0 keeps the deeper entry, slot 1 is always replaced.
#[derive(Clone, Copy)]
#[repr(C, align(32))]
struct Bucket([TtEntry; BUCKET]);

/// Bucketed transposition table, allocated once per `Hash` setting (never in the search).
pub struct TranspositionTable {
    buckets: Vec<Bucket>,
    age: u8,
}

impl TranspositionTable {
    pub fn new(mb: usize) -> Self {
        let len = (mb.max(1) * 1024 * 1024 / std::mem::size_of::<Bucket>()).max(1);
        Self {
            buckets: vec![Bucket([EMPTY; BUCKET]); len],
            age: 0,
        }
    }

    pub fn clear(&mut self) {
        self.buckets.fill(Bucket([EMPTY; BUCKET]));
        self.age = 0;
    }

    /// Called once per `go`, so entries from earlier moves lose replacement priority.
    pub fn new_search(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    #[inline(always)]
    fn index(&self, key: u64) -> usize {
        ((key as u128 * self.buckets.len() as u128) >> 64) as usize
    }

    #[inline(always)]
    pub fn probe(&self, key: u64, ply: usize) -> Option<TtEntry> {
        let b = &self.buckets[self.index(key)].0;
        b.iter().find(|e| e.key == key && e.depth >= 0).map(|e| TtEntry {
            score: score_from_tt(e.score as i32, ply) as i16,
            ..*e
        })
    }

    #[inline(always)]
    pub fn store(&mut self, key: u64, mv: Move, score: i32, depth: i32, bound: Bound, ply: usize) {
        let age = self.age;
        let idx = self.index(key);
        let b = &mut self.buckets[idx].0;
        // Same key; else slot 0 if the new entry is at least as deep or slot 0 is from an older search; else slot 1.
        let slot = b.iter().position(|e| e.key == key && e.depth >= 0).unwrap_or(
            if b[0].age != age || depth >= b[0].depth as i32 { 0 } else { 1 },
        );
        let old = b[slot];
        let same = old.key == key && old.depth >= 0;
        // Keep a deeper entry of the same position and search unless the new one is exact.
        if same && old.age == age && depth < old.depth as i32 && bound != Bound::Exact {
            return;
        }
        b[slot] = TtEntry {
            key,
            // Keep the old best move when this search found none (fail-low).
            mv: if mv == Move::NULL && same { old.mv } else { mv },
            score: score_to_tt(score, ply) as i16,
            depth: depth.clamp(0, 127) as i8,
            bound,
            age,
        };
    }

    /// Per-mille of slots written by the current search (UCI `hashfull`).
    pub fn hashfull(&self) -> u64 {
        let n = self.buckets.len().min(500);
        let used = self.buckets[..n].iter().flat_map(|b| b.0.iter()).filter(|e| e.depth >= 0 && e.age == self.age).count();
        (used * 1000 / (n * BUCKET)) as u64
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

    #[test]
    fn test_bucket_keeps_deep_entries() {
        assert_eq!(std::mem::size_of::<Bucket>(), 32);
        let mut tt = TranspositionTable::new(1);
        // Small keys all map to bucket 0 (index is the high half of key * len).
        let key = |k: u64| k + 1;
        tt.store(key(0), Move::NULL, 0, 20, Bound::Exact, 0);
        for k in 1..6 {
            tt.store(key(k), Move::NULL, 0, k as i32, Bound::Lower, 0);
        }
        // The deep entry survives the shallow stream in slot 0; slot 1 holds the latest.
        assert_eq!(tt.probe(key(0), 0).unwrap().depth, 20);
        assert!(tt.probe(key(5), 0).is_some() && tt.probe(key(4), 0).is_none());
        // A later search may replace the old deep entry.
        tt.new_search();
        tt.store(key(6), Move::NULL, 0, 1, Bound::Lower, 0);
        assert!(tt.probe(key(6), 0).is_some() && tt.probe(key(0), 0).is_none());
    }
}
