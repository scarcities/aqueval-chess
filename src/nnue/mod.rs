//! NNUE evaluation: a (768 → HIDDEN)x2 → 1 perspective network with SCReLU, run on
//! quantised integers. See docs/NNUE.md for the architecture and the net file layout.
//!
//! This module is always compiled and tested; the search only uses it when the crate is
//! built with the `nnue` feature (see `search::evaluator`).

mod simd;
mod state;

pub use state::NnueState;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crate::types::{Color, Piece};

/// Input features: 2 colours (own / enemy) × 6 piece types × 64 squares.
pub const INPUTS: usize = 768;
/// Hidden neurons per perspective.
pub const HIDDEN: usize = 256;
/// Quantisation of the feature transformer (weights and bias) and of the clipped activation.
pub const QA: i32 = 255;
/// Quantisation of the output weights.
pub const QB: i32 = 64;
/// Network output × SCALE = centipawns (the trainer fits sigmoid(cp / SCALE)).
pub const SCALE: i32 = 400;
/// NNUE scores are clamped to stay clear of mate scores.
pub const MAX_EVAL: i32 = 20_000;

// The AVX2 output layer sums HIDDEN / 8 products of at most 2·255²·128 per i32 lane.
const _: () = assert!(HIDDEN & 15 == 0 && HIDDEN <= 1024);

/// One perspective's hidden layer (or a row of weights with the same shape).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct Accumulator {
    pub vals: [i16; HIDDEN],
}

impl Accumulator {
    pub const ZERO: Accumulator = Accumulator { vals: [0; HIDDEN] };
}

/// Quantised network. Field order and types follow the net file exactly.
#[repr(C)]
pub struct Network {
    /// Feature transformer, one row of HIDDEN weights per input feature (× QA).
    pub feature_weights: [Accumulator; INPUTS],
    /// Feature transformer bias (× QA).
    pub feature_bias: Accumulator,
    /// Output weights: [0] for the side to move's accumulator, [1] for the other side's (× QB).
    pub output_weights: [Accumulator; 2],
    /// Output bias (× QA·QB).
    pub output_bias: i16,
    /// Every output weight is within ±128, so the AVX2 path may multiply in i16.
    small_output_weights: bool,
}

/// Bytes of the net data before padding: all i16, little-endian.
pub const NET_BYTES: usize = 2 * (INPUTS * HIDDEN + HIDDEN + 2 * HIDDEN + 1);
/// bullet pads `quantised.bin` to a multiple of 64 bytes.
pub const NET_BYTES_PADDED: usize = NET_BYTES.div_ceil(64) * 64;

/// Index of the feature for `piece` on `sq`, seen from `perspective`: own pieces first, then
/// enemy pieces, each P N B R Q K × 64 squares; Black's view is mirrored vertically so that
/// both sides see their own pieces moving "up" the board.
#[inline(always)]
pub fn feature_index(perspective: Color, piece: Piece, sq: u8) -> usize {
    let enemy = (piece.color != perspective) as usize;
    let rel_sq = (if perspective == Color::White { sq } else { sq ^ 56 }) as usize;
    enemy * 384 + piece.piece_type.index() * 64 + rel_sq
}

impl Network {
    /// Parses a net file. Accepts the exact size or bullet's 64-byte padded size.
    pub fn from_bytes(bytes: &[u8]) -> Result<Box<Network>, String> {
        if bytes.len() != NET_BYTES && bytes.len() != NET_BYTES_PADDED {
            return Err(format!(
                "net file has {} bytes, expected {} or {} for a (768 -> {})x2 -> 1 net",
                bytes.len(),
                NET_BYTES,
                NET_BYTES_PADDED,
                HIDDEN
            ));
        }
        // SAFETY: Network is integers and bools only; all-zero is a valid value (false).
        let mut net: Box<Network> = unsafe { Box::new_zeroed().assume_init() };
        let mut words = bytes[..NET_BYTES]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]));
        let mut fill = |acc: &mut Accumulator| {
            for v in acc.vals.iter_mut() {
                *v = words.next().expect("size checked above");
            }
        };
        for row in net.feature_weights.iter_mut() {
            fill(row);
        }
        fill(&mut net.feature_bias);
        for row in net.output_weights.iter_mut() {
            fill(row);
        }
        net.output_bias = words.next().expect("size checked above");
        net.small_output_weights = net
            .output_weights
            .iter()
            .all(|row| row.vals.iter().all(|w| (-128..=128).contains(w)));
        Ok(net)
    }

    pub fn from_file(path: &str) -> Result<Box<Network>, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        Self::from_bytes(&bytes)
    }

    /// Serialises the net in the file layout (unpadded). Used by tests and tools.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(NET_BYTES);
        let rows = self
            .feature_weights
            .iter()
            .chain([&self.feature_bias])
            .chain(self.output_weights.iter());
        for row in rows {
            for v in row.vals {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&self.output_bias.to_le_bytes());
        out
    }

    /// Accumulator for one perspective computed from scratch.
    pub fn refresh(&self, board: &crate::board::Board, perspective: Color) -> Accumulator {
        let mut acc = self.feature_bias;
        let mut occ = board.occupied();
        while occ.is_not_empty() {
            let sq = occ.pop_lsb();
            let piece = board.piece_at(sq).expect("occupied square has a piece");
            let row = &self.feature_weights[feature_index(perspective, piece, sq)];
            for (a, w) in acc.vals.iter_mut().zip(row.vals) {
                *a = a.wrapping_add(w);
            }
        }
        acc
    }

    /// Output in centipawns from the side to move's view, given its accumulator (`us`) and
    /// the other side's (`them`).
    #[inline]
    pub fn output(&self, us: &Accumulator, them: &Accumulator, avx2: bool) -> i32 {
        let dot = |acc: &Accumulator, w: &Accumulator| -> i64 {
            #[cfg(target_arch = "x86_64")]
            if avx2 && self.small_output_weights {
                // SAFETY: `avx2` is only true when the CPU supports AVX2 (see `simd::has_avx2`).
                return unsafe { simd::screlu_dot_avx2(acc, w) };
            }
            let _ = avx2;
            simd::screlu_dot_scalar(acc, w)
        };
        // Sum is in units of QA²·QB; one QA goes away here, the other with the final division.
        let sum = dot(us, &self.output_weights[0]) + dot(them, &self.output_weights[1]);
        let out = sum / QA as i64 + self.output_bias as i64;
        let cp = out * SCALE as i64 / (QA * QB) as i64;
        cp.clamp(-MAX_EVAL as i64, MAX_EVAL as i64) as i32
    }

    /// Full evaluation from scratch (no incremental state). Reference for tests and `eval`.
    pub fn evaluate_from_scratch(&self, board: &crate::board::Board) -> i32 {
        let stm = board.side_to_move;
        self.output(&self.refresh(board, stm), &self.refresh(board, !stm), false)
    }
}

// ---- active net, shared by every search ----

static ACTIVE: RwLock<Option<Arc<Network>>> = RwLock::new(None);
static ENABLED: AtomicBool = AtomicBool::new(true);
static EMBEDDED_LOADED: std::sync::Once = std::sync::Once::new();

/// Net compiled into the binary (`EVALFILE=<path>` at build time with the `nnue` feature);
/// empty when none was given.
#[cfg(feature = "nnue")]
static EMBEDDED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/embedded.nnue"));
#[cfg(not(feature = "nnue"))]
static EMBEDDED: &[u8] = &[];

pub fn has_embedded_net() -> bool {
    !EMBEDDED.is_empty()
}

/// The net the search should use: None when NNUE is switched off or no net is loaded.
pub fn active() -> Option<Arc<Network>> {
    EMBEDDED_LOADED.call_once(|| {
        if has_embedded_net() {
            match Network::from_bytes(EMBEDDED) {
                Ok(net) => *ACTIVE.write().unwrap() = Some(Arc::from(net)),
                Err(e) => eprintln!("embedded net rejected: {e}"),
            }
        }
    });
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    ACTIVE.read().unwrap().clone()
}

/// Loads `path` as the active net (`<embedded>` or an empty path selects the embedded net).
pub fn load_eval_file(path: &str) -> Result<(), String> {
    active(); // make sure the embedded net cannot overwrite this one later
    let net = if path.is_empty() || path == "<embedded>" {
        if !has_embedded_net() {
            return Err("no net is embedded in this binary".into());
        }
        Network::from_bytes(EMBEDDED)?
    } else {
        Network::from_file(path)?
    };
    *ACTIVE.write().unwrap() = Some(Arc::from(net));
    Ok(())
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::board::zobrist::SplitMix64;
    use crate::board::Board;
    use crate::types::PieceType;

    /// Random net with weights in the ranges a trained net has.
    pub(crate) fn random_net(seed: u64) -> Box<Network> {
        let mut rng = SplitMix64::new(seed);
        let mut r = |m: i64| (rng.next_u64() % (2 * m as u64 + 1)) as i64 - m;
        let mut bytes = Vec::with_capacity(NET_BYTES);
        let mut push = |v: i64| bytes.extend_from_slice(&(v as i16).to_le_bytes());
        for _ in 0..INPUTS * HIDDEN {
            push(r(120));
        }
        for _ in 0..HIDDEN {
            push(r(200));
        }
        for _ in 0..2 * HIDDEN {
            push(r(127));
        }
        push(r(5000));
        Network::from_bytes(&bytes).unwrap()
    }

    /// Straightforward float-free reference of the forward pass, independent of `output`.
    fn reference_eval(net: &Network, board: &Board) -> i32 {
        let stm = board.side_to_move;
        let mut sum: i64 = 0;
        for (k, persp) in [stm, !stm].into_iter().enumerate() {
            for h in 0..HIDDEN {
                let mut a = net.feature_bias.vals[h] as i64;
                for sq in 0..64u8 {
                    if let Some(p) = board.piece_at(sq) {
                        a += net.feature_weights[feature_index(persp, p, sq)].vals[h] as i64;
                    }
                }
                let c = a.clamp(0, QA as i64);
                sum += c * c * net.output_weights[k].vals[h] as i64;
            }
        }
        let out = sum / QA as i64 + net.output_bias as i64;
        (out * SCALE as i64 / (QA * QB) as i64).clamp(-MAX_EVAL as i64, MAX_EVAL as i64) as i32
    }

    #[test]
    fn feature_index_layout() {
        let wp = Piece::new(Color::White, PieceType::Pawn);
        let bk = Piece::new(Color::Black, PieceType::King);
        // e2 pawn: own pawn on e2 for White, enemy pawn on e7 for Black.
        assert_eq!(feature_index(Color::White, wp, 12), 12);
        assert_eq!(feature_index(Color::Black, wp, 12), 384 + 52);
        // e8 king: enemy king on e8 for White, own king on e1 for Black.
        assert_eq!(feature_index(Color::White, bk, 60), 384 + 5 * 64 + 60);
        assert_eq!(feature_index(Color::Black, bk, 60), 5 * 64 + 4);
    }

    #[test]
    fn file_roundtrip_and_size_checks() {
        let net = random_net(1);
        let bytes = net.to_bytes();
        assert_eq!(bytes.len(), NET_BYTES);
        assert_eq!(Network::from_bytes(&bytes).unwrap().to_bytes(), bytes);
        let mut padded = bytes.clone();
        padded.resize(NET_BYTES_PADDED, 0);
        assert_eq!(Network::from_bytes(&padded).unwrap().to_bytes(), bytes);
        assert!(Network::from_bytes(&bytes[..bytes.len() - 2]).is_err());
    }

    #[test]
    fn output_matches_reference() {
        let net = random_net(7);
        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        ] {
            let board = Board::from_fen(fen).unwrap();
            let expected = reference_eval(&net, &board);
            assert_eq!(net.evaluate_from_scratch(&board), expected, "{fen}");
            let stm = board.side_to_move;
            let (us, them) = (net.refresh(&board, stm), net.refresh(&board, !stm));
            assert_eq!(net.output(&us, &them, simd::has_avx2()), expected, "{fen} (simd)");
        }
    }

    #[test]
    fn colour_flipped_position_evaluates_the_same() {
        // Perspective features make the eval exactly symmetric under a colour flip.
        let net = random_net(3);
        let a = Board::from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4").unwrap();
        let b = Board::from_fen("rnbqk2r/pppp1ppp/5n2/2b1p3/4P3/2N2N2/PPPP1PPP/R1BQKB1R b KQkq - 4 4").unwrap();
        assert_eq!(net.evaluate_from_scratch(&a), net.evaluate_from_scratch(&b));
    }
}
