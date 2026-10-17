use std::fmt;
use std::ops::Deref;
use crate::types::{PieceType, Square};

#[derive(Copy, Clone, PartialEq, Eq, Default, Hash)]
#[repr(transparent)]
pub struct Move(pub u16);

impl Move {
    pub const NULL: Move = Move(0);

    pub const QUIET: u16 = 0x0;
    pub const DOUBLE_PAWN_PUSH: u16 = 0x1;
    pub const KING_CASTLE: u16 = 0x2;
    pub const QUEEN_CASTLE: u16 = 0x3;
    pub const CAPTURE: u16 = 0x4;
    pub const EN_PASSANT: u16 = 0x5;

    pub const PROMO_KNIGHT: u16 = 0x8;
    pub const PROMO_BISHOP: u16 = 0x9;
    pub const PROMO_ROOK: u16 = 0xA;
    pub const PROMO_QUEEN: u16 = 0xB;

    pub const PROMO_CAP_KNIGHT: u16 = 0xC;
    pub const PROMO_CAP_BISHOP: u16 = 0xD;
    pub const PROMO_CAP_ROOK: u16 = 0xE;
    pub const PROMO_CAP_QUEEN: u16 = 0xF;

    #[inline(always)]
    pub fn new(from: u8, to: u8, flags: u16) -> Self {
        debug_assert!(from < 64);
        debug_assert!(to < 64);
        debug_assert!(flags <= 0xF);
        Move((from as u16) | ((to as u16) << 6) | (flags << 12))
    }

    #[inline(always)]
    pub fn from_sq(self) -> u8 {
        (self.0 & 0x3F) as u8
    }

    #[inline(always)]
    pub fn to_sq(self) -> u8 {
        ((self.0 >> 6) & 0x3F) as u8
    }

    #[inline(always)]
    pub fn flags(self) -> u16 {
        self.0 >> 12
    }

    #[inline(always)]
    pub fn is_capture(self) -> bool {
        (self.0 & 0x4000) != 0
    }

    #[inline(always)]
    pub fn is_promotion(self) -> bool {
        (self.0 & 0x8000) != 0
    }

    #[inline(always)]
    pub fn is_castle(self) -> bool {
        (self.flags() & 0xE) == 0x2
    }

    #[inline(always)]
    pub fn is_king_castle(self) -> bool {
        self.flags() == Self::KING_CASTLE
    }

    #[inline(always)]
    pub fn is_queen_castle(self) -> bool {
        self.flags() == Self::QUEEN_CASTLE
    }

    #[inline(always)]
    pub fn is_en_passant(self) -> bool {
        self.flags() == Self::EN_PASSANT
    }

    #[inline(always)]
    pub fn is_double_push(self) -> bool {
        self.flags() == Self::DOUBLE_PAWN_PUSH
    }

    #[inline(always)]
    pub fn promotion_piece(self) -> Option<PieceType> {
        if !self.is_promotion() {
            return None;
        }
        match self.flags() & 0x3 {
            0 => Some(PieceType::Knight),
            1 => Some(PieceType::Bishop),
            2 => Some(PieceType::Rook),
            _ => Some(PieceType::Queen),
        }
    }

    pub fn to_uci(self) -> String {
        let from = Square::from_u8(self.from_sq()).to_str();
        let to = Square::from_u8(self.to_sq()).to_str();
        if let Some(promo) = self.promotion_piece() {
            format!("{}{}{}", from, to, promo.to_char())
        } else {
            format!("{}{}", from, to)
        }
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move({})", self.to_uci())
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uci())
    }
}

#[derive(Copy, Clone)]
pub struct MoveList {
    pub moves: [Move; 256],
    pub count: usize,
}

impl MoveList {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            moves: [Move::NULL; 256],
            count: 0,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, m: Move) {
        debug_assert!(self.count < 256, "MoveList capacity overflow");
        // SAFETY: a legal chess position has at most 218 moves, below the 256 capacity.
        unsafe {
            *self.moves.get_unchecked_mut(self.count) = m;
        }
        self.count += 1;
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.count
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.count = 0;
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.count]
    }
}

impl Default for MoveList {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for MoveList {
    type Target = [Move];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.moves[..self.count]
    }
}

impl fmt::Debug for MoveList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_encoding() {
        let m = Move::new(Square::E2 as u8, Square::E4 as u8, Move::DOUBLE_PAWN_PUSH);
        assert_eq!(m.from_sq(), Square::E2 as u8);
        assert_eq!(m.to_sq(), Square::E4 as u8);
        assert_eq!(m.flags(), Move::DOUBLE_PAWN_PUSH);
        assert!(m.is_double_push());
        assert!(!m.is_capture());
        assert!(!m.is_promotion());
        assert_eq!(m.to_uci(), "e2e4");

        let promo = Move::new(Square::E7 as u8, Square::E8 as u8, Move::PROMO_QUEEN);
        assert!(promo.is_promotion());
        assert_eq!(promo.promotion_piece(), Some(PieceType::Queen));
        assert_eq!(promo.to_uci(), "e7e8q");

        let promo_cap = Move::new(Square::E7 as u8, Square::F8 as u8, Move::PROMO_CAP_KNIGHT);
        assert!(promo_cap.is_promotion());
        assert!(promo_cap.is_capture());
        assert_eq!(promo_cap.promotion_piece(), Some(PieceType::Knight));
        assert_eq!(promo_cap.to_uci(), "e7f8n");
    }

    #[test]
    fn test_movelist_stack_operations() {
        let mut list = MoveList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);

        let m1 = Move::new(0, 1, Move::QUIET);
        let m2 = Move::new(2, 3, Move::CAPTURE);
        list.push(m1);
        list.push(m2);

        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
        assert_eq!(list[0], m1);
        assert_eq!(list[1], m2);

        list.clear();
        assert!(list.is_empty());
    }
}

