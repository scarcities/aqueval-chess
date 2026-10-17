use crate::types::{Bitboard, Color};

pub const KNIGHT_ATTACKS: [Bitboard; 64] = compute_all_knight_attacks();
pub const KING_ATTACKS: [Bitboard; 64] = compute_all_king_attacks();
pub const PAWN_ATTACKS: [[Bitboard; 64]; 2] = compute_all_pawn_attacks();
pub static BETWEEN: [[Bitboard; 64]; 64] = compute_all_between();
pub static LINE: [[Bitboard; 64]; 64] = compute_all_line();

const fn compute_knight_attacks_sq(sq: u8) -> Bitboard {
    let mut bb = 0u64;
    let rank = (sq >> 3) as i32;
    let file = (sq & 7) as i32;

    let offsets: [(i32, i32); 8] = [
        (2, 1), (2, -1), (-2, 1), (-2, -1),
        (1, 2), (1, -2), (-1, 2), (-1, -2),
    ];

    let mut i = 0;
    while i < 8 {
        let (dr, df) = offsets[i];
        let r = rank + dr;
        let f = file + df;
        if r >= 0 && r < 8 && f >= 0 && f < 8 {
            let target = (r << 3) | f;
            bb |= 1u64 << target;
        }
        i += 1;
    }
    Bitboard(bb)
}

const fn compute_all_knight_attacks() -> [Bitboard; 64] {
    let mut table = [Bitboard::EMPTY; 64];
    let mut sq = 0;
    while sq < 64 {
        table[sq] = compute_knight_attacks_sq(sq as u8);
        sq += 1;
    }
    table
}

const fn compute_king_attacks_sq(sq: u8) -> Bitboard {
    let mut bb = 0u64;
    let rank = (sq >> 3) as i32;
    let file = (sq & 7) as i32;

    let offsets: [(i32, i32); 8] = [
        (1, 0), (-1, 0), (0, 1), (0, -1),
        (1, 1), (1, -1), (-1, 1), (-1, -1),
    ];

    let mut i = 0;
    while i < 8 {
        let (dr, df) = offsets[i];
        let r = rank + dr;
        let f = file + df;
        if r >= 0 && r < 8 && f >= 0 && f < 8 {
            let target = (r << 3) | f;
            bb |= 1u64 << target;
        }
        i += 1;
    }
    Bitboard(bb)
}

const fn compute_all_king_attacks() -> [Bitboard; 64] {
    let mut table = [Bitboard::EMPTY; 64];
    let mut sq = 0;
    while sq < 64 {
        table[sq] = compute_king_attacks_sq(sq as u8);
        sq += 1;
    }
    table
}

const fn compute_pawn_attacks_sq(color: usize, sq: u8) -> Bitboard {
    let mut bb = 0u64;
    let rank = (sq >> 3) as i32;
    let file = (sq & 7) as i32;

    let dr = if color == 0 { 1 } else { -1 };
    let r = rank + dr;
    if r >= 0 && r < 8 {
        if file > 0 {
            bb |= 1u64 << ((r << 3) | (file - 1));
        }
        if file < 7 {
            bb |= 1u64 << ((r << 3) | (file + 1));
        }
    }
    Bitboard(bb)
}

const fn compute_all_pawn_attacks() -> [[Bitboard; 64]; 2] {
    let mut table = [[Bitboard::EMPTY; 64]; 2];
    let mut sq = 0;
    while sq < 64 {
        table[0][sq] = compute_pawn_attacks_sq(0, sq as u8);
        table[1][sq] = compute_pawn_attacks_sq(1, sq as u8);
        sq += 1;
    }
    table
}

const fn compute_all_between() -> [[Bitboard; 64]; 64] {
    let mut table = [[Bitboard::EMPTY; 64]; 64];
    let mut sq1 = 0;
    while sq1 < 64 {
        let r1 = (sq1 >> 3) as i32;
        let f1 = (sq1 & 7) as i32;

        let mut sq2 = 0;
        while sq2 < 64 {
            let r2 = (sq2 >> 3) as i32;
            let f2 = (sq2 & 7) as i32;

            let dr = r2 - r1;
            let df = f2 - f1;

            if dr == 0 || df == 0 || dr.abs() == df.abs() {
                let step_r = if dr == 0 { 0 } else { dr / dr.abs() };
                let step_f = if df == 0 { 0 } else { df / df.abs() };

                let mut cur_r = r1 + step_r;
                let mut cur_f = f1 + step_f;
                let mut bb = 0u64;

                while cur_r != r2 || cur_f != f2 {
                    bb |= 1u64 << ((cur_r << 3) | cur_f);
                    cur_r += step_r;
                    cur_f += step_f;
                }
                table[sq1][sq2] = Bitboard(bb);
            }
            sq2 += 1;
        }
        sq1 += 1;
    }
    table
}

const fn compute_all_line() -> [[Bitboard; 64]; 64] {
    let mut table = [[Bitboard::EMPTY; 64]; 64];
    let mut sq1 = 0;
    while sq1 < 64 {
        let r1 = (sq1 >> 3) as i32;
        let f1 = (sq1 & 7) as i32;

        let mut sq2 = 0;
        while sq2 < 64 {
            if sq1 != sq2 {
                let r2 = (sq2 >> 3) as i32;
                let f2 = (sq2 & 7) as i32;

                let dr = r2 - r1;
                let df = f2 - f1;

                if dr == 0 || df == 0 || dr.abs() == df.abs() {
                    let step_r = if dr == 0 { 0 } else { dr / dr.abs() };
                    let step_f = if df == 0 { 0 } else { df / df.abs() };

                    let mut bb = 0u64;
                    // Go in positive direction from sq1
                    let mut cur_r = r1;
                    let mut cur_f = f1;
                    while cur_r >= 0 && cur_r < 8 && cur_f >= 0 && cur_f < 8 {
                        bb |= 1u64 << ((cur_r << 3) | cur_f);
                        cur_r += step_r;
                        cur_f += step_f;
                    }
                    // Go in negative direction from sq1
                    cur_r = r1 - step_r;
                    cur_f = f1 - step_f;
                    while cur_r >= 0 && cur_r < 8 && cur_f >= 0 && cur_f < 8 {
                        bb |= 1u64 << ((cur_r << 3) | cur_f);
                        cur_r -= step_r;
                        cur_f -= step_f;
                    }
                    table[sq1][sq2] = Bitboard(bb);
                }
            }
            sq2 += 1;
        }
        sq1 += 1;
    }
    table
}

#[inline(always)]
pub fn get_knight_attacks(sq: u8) -> Bitboard {
    debug_assert!(sq < 64);
    KNIGHT_ATTACKS[sq as usize]
}

#[inline(always)]
pub fn get_king_attacks(sq: u8) -> Bitboard {
    debug_assert!(sq < 64);
    KING_ATTACKS[sq as usize]
}

#[inline(always)]
pub fn get_pawn_attacks(color: Color, sq: u8) -> Bitboard {
    debug_assert!(sq < 64);
    PAWN_ATTACKS[color.index()][sq as usize]
}

#[inline(always)]
pub fn between(sq1: u8, sq2: u8) -> Bitboard {
    debug_assert!(sq1 < 64 && sq2 < 64);
    BETWEEN[sq1 as usize][sq2 as usize]
}

#[inline(always)]
pub fn between_inclusive(sq1: u8, sq2: u8) -> Bitboard {
    debug_assert!(sq1 < 64 && sq2 < 64);
    BETWEEN[sq1 as usize][sq2 as usize] | Bitboard::from_sq(sq1) | Bitboard::from_sq(sq2)
}

#[inline(always)]
pub fn line(sq1: u8, sq2: u8) -> Bitboard {
    debug_assert!(sq1 < 64 && sq2 < 64);
    LINE[sq1 as usize][sq2 as usize]
}

pub fn compute_rook_attacks_classical(sq: u8, occ: Bitboard) -> Bitboard {
    let mut attacks = 0u64;
    let rank = (sq >> 3) as i32;
    let file = (sq & 7) as i32;

    // North
    let mut r = rank + 1;
    while r < 8 {
        let s = (r << 3) | file;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        r += 1;
    }

    // South
    let mut r = rank - 1;
    while r >= 0 {
        let s = (r << 3) | file;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        r -= 1;
    }

    // East
    let mut f = file + 1;
    while f < 8 {
        let s = (rank << 3) | f;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        f += 1;
    }

    // West
    let mut f = file - 1;
    while f >= 0 {
        let s = (rank << 3) | f;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        f -= 1;
    }

    Bitboard(attacks)
}

pub fn compute_bishop_attacks_classical(sq: u8, occ: Bitboard) -> Bitboard {
    let mut attacks = 0u64;
    let rank = (sq >> 3) as i32;
    let file = (sq & 7) as i32;

    // North-East
    let mut r = rank + 1;
    let mut f = file + 1;
    while r < 8 && f < 8 {
        let s = (r << 3) | f;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        r += 1;
        f += 1;
    }

    // North-West
    let mut r = rank + 1;
    let mut f = file - 1;
    while r < 8 && f >= 0 {
        let s = (r << 3) | f;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        r += 1;
        f -= 1;
    }

    // South-East
    let mut r = rank - 1;
    let mut f = file + 1;
    while r >= 0 && f < 8 {
        let s = (r << 3) | f;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        r -= 1;
        f += 1;
    }

    // South-West
    let mut r = rank - 1;
    let mut f = file - 1;
    while r >= 0 && f >= 0 {
        let s = (r << 3) | f;
        attacks |= 1u64 << s;
        if occ.contains(s as u8) {
            break;
        }
        r -= 1;
        f -= 1;
    }

    Bitboard(attacks)
}

pub fn rook_mask(sq: u8) -> Bitboard {
    let mut mask = 0u64;
    let rank = (sq >> 3) as i32;
    let file = (sq & 7) as i32;

    let mut r = rank + 1;
    while r < 7 {
        mask |= 1u64 << ((r << 3) | file);
        r += 1;
    }

    let mut r = rank - 1;
    while r > 0 {
        mask |= 1u64 << ((r << 3) | file);
        r -= 1;
    }

    let mut f = file + 1;
    while f < 7 {
        mask |= 1u64 << ((rank << 3) | f);
        f += 1;
    }

    let mut f = file - 1;
    while f > 0 {
        mask |= 1u64 << ((rank << 3) | f);
        f -= 1;
    }

    Bitboard(mask)
}

pub fn bishop_mask(sq: u8) -> Bitboard {
    let mut mask = 0u64;
    let rank = (sq >> 3) as i32;
    let file = (sq & 7) as i32;

    let mut r = rank + 1;
    let mut f = file + 1;
    while r < 7 && f < 7 {
        mask |= 1u64 << ((r << 3) | f);
        r += 1;
        f += 1;
    }

    let mut r = rank + 1;
    let mut f = file - 1;
    while r < 7 && f > 0 {
        mask |= 1u64 << ((r << 3) | f);
        r += 1;
        f -= 1;
    }

    let mut r = rank - 1;
    let mut f = file + 1;
    while r > 0 && f < 7 {
        mask |= 1u64 << ((r << 3) | f);
        r -= 1;
        f += 1;
    }

    let mut r = rank - 1;
    let mut f = file - 1;
    while r > 0 && f > 0 {
        mask |= 1u64 << ((r << 3) | f);
        r -= 1;
        f -= 1;
    }

    Bitboard(mask)
}
