use std::sync::LazyLock;
use crate::types::{Color, PieceType};

pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

pub struct ZobristKeys {
    pub pieces: [[[u64; 64]; 6]; 2],
    pub castling: [u64; 16],
    pub ep_file: [u64; 8],
    pub side_to_move: u64,
}

impl ZobristKeys {
    pub fn init() -> Self {
        let mut rng = SplitMix64::new(0x18F3B927A65C04DE);
        let mut pieces = [[[0u64; 64]; 6]; 2];
        for piece_color in &mut pieces {
            for piece_type in piece_color {
                for sq_key in piece_type {
                    *sq_key = rng.next_u64();
                }
            }
        }

        let mut castling = [0u64; 16];
        for key in &mut castling {
            *key = rng.next_u64();
        }

        let mut ep_file = [0u64; 8];
        for key in &mut ep_file {
            *key = rng.next_u64();
        }

        let side_to_move = rng.next_u64();

        Self {
            pieces,
            castling,
            ep_file,
            side_to_move,
        }
    }
}

pub static ZOBRIST: LazyLock<ZobristKeys> = LazyLock::new(ZobristKeys::init);

#[inline(always)]
pub fn piece_key(color: Color, piece_type: PieceType, sq: u8) -> u64 {
    ZOBRIST.pieces[color.index()][piece_type.index()][sq as usize]
}

#[inline(always)]
pub fn castling_key(rights: u8) -> u64 {
    debug_assert!(rights < 16);
    ZOBRIST.castling[rights as usize]
}

#[inline(always)]
pub fn ep_key(file: u8) -> u64 {
    debug_assert!(file < 8);
    ZOBRIST.ep_file[file as usize]
}

#[inline(always)]
pub fn side_key() -> u64 {
    ZOBRIST.side_to_move
}
