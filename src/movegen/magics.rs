use std::sync::Once;
use crate::types::Bitboard;
use crate::movegen::attacks::{compute_bishop_attacks_classical, compute_rook_attacks_classical};
use crate::movegen::magic_constants::{BISHOP_MAGICS, ROOK_MAGICS, TOTAL_ATTACK_ENTRIES};

static mut ATTACK_TABLE: [Bitboard; TOTAL_ATTACK_ENTRIES] = [Bitboard(0); TOTAL_ATTACK_ENTRIES];
static INIT: Once = Once::new();

pub fn init() {
    INIT.call_once(|| {
        unsafe {
            let table_ptr = std::ptr::addr_of_mut!(ATTACK_TABLE) as *mut Bitboard;

            // Populate bishop attack tables
            for sq in 0..64 {
                let magic = &BISHOP_MAGICS[sq as usize];
                let mask = magic.mask;
                let mut blockers = 0u64;

                loop {
                    let attack = compute_bishop_attacks_classical(sq, Bitboard(blockers));
                    let idx = magic.offset + ((blockers.wrapping_mul(magic.magic) >> magic.shift) as usize);
                    *table_ptr.add(idx) = attack;

                    blockers = blockers.wrapping_sub(mask.0) & mask.0;
                    if blockers == 0 {
                        break;
                    }
                }
            }

            // Populate rook attack tables
            for sq in 0..64 {
                let magic = &ROOK_MAGICS[sq as usize];
                let mask = magic.mask;
                let mut blockers = 0u64;

                loop {
                    let attack = compute_rook_attacks_classical(sq, Bitboard(blockers));
                    let idx = magic.offset + ((blockers.wrapping_mul(magic.magic) >> magic.shift) as usize);
                    *table_ptr.add(idx) = attack;

                    blockers = blockers.wrapping_sub(mask.0) & mask.0;
                    if blockers == 0 {
                        break;
                    }
                }
            }
        }
    });
}

#[inline(always)]
pub fn get_bishop_attacks(sq: u8, occ: Bitboard) -> Bitboard {
    debug_assert!(sq < 64);
    let magic = &BISHOP_MAGICS[sq as usize];
    let blockers = occ.0 & magic.mask.0;
    let idx = magic.offset + ((blockers.wrapping_mul(magic.magic) >> magic.shift) as usize);
    unsafe {
        let ptr = std::ptr::addr_of!(ATTACK_TABLE) as *const Bitboard;
        *ptr.add(idx)
    }
}

#[inline(always)]
pub fn get_rook_attacks(sq: u8, occ: Bitboard) -> Bitboard {
    debug_assert!(sq < 64);
    let magic = &ROOK_MAGICS[sq as usize];
    let blockers = occ.0 & magic.mask.0;
    let idx = magic.offset + ((blockers.wrapping_mul(magic.magic) >> magic.shift) as usize);
    unsafe {
        let ptr = std::ptr::addr_of!(ATTACK_TABLE) as *const Bitboard;
        *ptr.add(idx)
    }
}

#[inline(always)]
pub fn get_queen_attacks(sq: u8, occ: Bitboard) -> Bitboard {
    get_bishop_attacks(sq, occ) | get_rook_attacks(sq, occ)
}
