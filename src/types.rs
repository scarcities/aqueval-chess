use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, Shr};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    #[inline(always)]
    pub fn opposite(self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    #[inline(always)]
    pub fn index(self) -> usize {
        self as usize
    }
}

impl Not for Color {
    type Output = Self;
    #[inline(always)]
    fn not(self) -> Self::Output {
        self.opposite()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceType {
    pub const ALL: [PieceType; 6] = [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ];

    #[inline(always)]
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'p' => Some(PieceType::Pawn),
            'n' => Some(PieceType::Knight),
            'b' => Some(PieceType::Bishop),
            'r' => Some(PieceType::Rook),
            'q' => Some(PieceType::Queen),
            'k' => Some(PieceType::King),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            PieceType::Pawn => 'p',
            PieceType::Knight => 'n',
            PieceType::Bishop => 'b',
            PieceType::Rook => 'r',
            PieceType::Queen => 'q',
            PieceType::King => 'k',
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Piece {
    pub color: Color,
    pub piece_type: PieceType,
}

impl Piece {
    #[inline(always)]
    pub fn new(color: Color, piece_type: PieceType) -> Self {
        Self { color, piece_type }
    }

    pub fn from_char(c: char) -> Option<Self> {
        let color = if c.is_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        let piece_type = PieceType::from_char(c)?;
        Some(Self { color, piece_type })
    }

    pub fn to_char(self) -> char {
        let c = self.piece_type.to_char();
        if self.color == Color::White {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(u8)]
pub enum Square {
    A1 = 0, B1, C1, D1, E1, F1, G1, H1,
    A2, B2, C2, D2, E2, F2, G2, H2,
    A3, B3, C3, D3, E3, F3, G3, H3,
    A4, B4, C4, D4, E4, F4, G4, H4,
    A5, B5, C5, D5, E5, F5, G5, H5,
    A6, B6, C6, D6, E6, F6, G6, H6,
    A7, B7, C7, D7, E7, F7, G7, H7,
    A8, B8, C8, D8, E8, F8, G8, H8,
}

impl Square {
    #[inline(always)]
    pub fn from_u8(sq: u8) -> Self {
        debug_assert!(sq < 64);
        // SAFETY: Square is #[repr(u8)] with variants 0..64; callers pass board squares (FEN parsing rejects anything else).
        unsafe { std::mem::transmute(sq) }
    }

    #[inline(always)]
    pub fn from_coords(file: u8, rank: u8) -> Self {
        debug_assert!(file < 8 && rank < 8);
        Self::from_u8((rank << 3) | file)
    }

    #[inline(always)]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    #[inline(always)]
    pub fn file(self) -> u8 {
        (self as u8) & 7
    }

    #[inline(always)]
    pub fn rank(self) -> u8 {
        (self as u8) >> 3
    }

    pub fn parse(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let file = bytes[0].to_ascii_lowercase().wrapping_sub(b'a');
        let rank = bytes[1].wrapping_sub(b'1');
        if file < 8 && rank < 8 {
            Some(Self::from_coords(file, rank))
        } else {
            None
        }
    }

    pub fn to_str(self) -> String {
        let file = (b'a' + self.file()) as char;
        let rank = (b'1' + self.rank()) as char;
        format!("{}{}", file, rank)
    }
}

impl std::str::FromStr for Square {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, Hash)]
#[repr(transparent)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(!0);

    pub const FILE_A: Bitboard = Bitboard(0x0101010101010101);
    pub const FILE_B: Bitboard = Bitboard(0x0202020202020202);
    pub const FILE_C: Bitboard = Bitboard(0x0404040404040404);
    pub const FILE_D: Bitboard = Bitboard(0x0808080808080808);
    pub const FILE_E: Bitboard = Bitboard(0x1010101010101010);
    pub const FILE_F: Bitboard = Bitboard(0x2020202020202020);
    pub const FILE_G: Bitboard = Bitboard(0x4040404040404040);
    pub const FILE_H: Bitboard = Bitboard(0x8080808080808080);

    pub const RANK_1: Bitboard = Bitboard(0x00000000000000FF);
    pub const RANK_2: Bitboard = Bitboard(0x000000000000FF00);
    pub const RANK_3: Bitboard = Bitboard(0x0000000000FF0000);
    pub const RANK_4: Bitboard = Bitboard(0x00000000FF000000);
    pub const RANK_5: Bitboard = Bitboard(0x000000FF00000000);
    pub const RANK_6: Bitboard = Bitboard(0x0000FF0000000000);
    pub const RANK_7: Bitboard = Bitboard(0x00FF000000000000);
    pub const RANK_8: Bitboard = Bitboard(0xFF00000000000000);

    #[inline(always)]
    pub fn from_sq(sq: u8) -> Self {
        Bitboard(1u64 << sq)
    }

    #[inline(always)]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub fn is_not_empty(self) -> bool {
        self.0 != 0
    }

    #[inline(always)]
    pub fn contains(self, sq: u8) -> bool {
        (self.0 & (1u64 << sq)) != 0
    }

    #[inline(always)]
    pub fn set(&mut self, sq: u8) {
        self.0 |= 1u64 << sq;
    }

    #[inline(always)]
    pub fn clear(&mut self, sq: u8) {
        self.0 &= !(1u64 << sq);
    }

    #[inline(always)]
    pub fn toggle(&mut self, sq: u8) {
        self.0 ^= 1u64 << sq;
    }

    #[inline(always)]
    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    #[inline(always)]
    pub fn more_than_one(self) -> bool {
        (self.0 & self.0.wrapping_sub(1)) != 0
    }

    #[inline(always)]
    pub fn lsb(self) -> u8 {
        debug_assert!(self.0 != 0);
        self.0.trailing_zeros() as u8
    }

    #[inline(always)]
    pub fn pop_lsb(&mut self) -> u8 {
        debug_assert!(self.0 != 0);
        let sq = self.0.trailing_zeros() as u8;
        self.0 &= self.0.wrapping_sub(1);
        sq
    }

    #[inline(always)]
    pub fn shift_north(self) -> Bitboard {
        Bitboard(self.0 << 8)
    }

    #[inline(always)]
    pub fn shift_south(self) -> Bitboard {
        Bitboard(self.0 >> 8)
    }

    #[inline(always)]
    pub fn shift_east(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_H.0) << 1)
    }

    #[inline(always)]
    pub fn shift_west(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_A.0) >> 1)
    }

    #[inline(always)]
    pub fn shift_north_east(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_H.0) << 9)
    }

    #[inline(always)]
    pub fn shift_north_west(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_A.0) << 7)
    }

    #[inline(always)]
    pub fn shift_south_east(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_H.0) >> 7)
    }

    #[inline(always)]
    pub fn shift_south_west(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_A.0) >> 9)
    }
}

impl Iterator for Bitboard {
    type Item = u8;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            None
        } else {
            Some(self.pop_lsb())
        }
    }
}

impl BitAnd for Bitboard {
    type Output = Self;
    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitAndAssign for Bitboard {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitOr for Bitboard {
    type Output = Self;
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 | rhs.0)
    }
}

impl BitOrAssign for Bitboard {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitXor for Bitboard {
    type Output = Self;
    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for Bitboard {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for Bitboard {
    type Output = Self;
    #[inline(always)]
    fn not(self) -> Self::Output {
        Bitboard(!self.0)
    }
}

impl Shl<usize> for Bitboard {
    type Output = Self;
    #[inline(always)]
    fn shl(self, rhs: usize) -> Self::Output {
        Bitboard(self.0 << rhs)
    }
}

impl Shr<usize> for Bitboard {
    type Output = Self;
    #[inline(always)]
    fn shr(self, rhs: usize) -> Self::Output {
        Bitboard(self.0 >> rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_coords() {
        let sq = Square::from_coords(4, 3); // E4
        assert_eq!(sq, Square::E4);
        assert_eq!(sq.file(), 4);
        assert_eq!(sq.rank(), 3);
        assert_eq!(sq.to_str(), "e4");
        assert_eq!(Square::parse("e4"), Some(Square::E4));
        assert_eq!(Square::parse("a1"), Some(Square::A1));
        assert_eq!(Square::parse("h8"), Some(Square::H8));
        assert_eq!(Square::parse("z9"), None);
    }

    #[test]
    fn test_bitboard_ops() {
        let mut bb = Bitboard::EMPTY;
        assert!(bb.is_empty());
        assert_eq!(bb.count(), 0);

        bb.set(Square::E4 as u8);
        assert!(!bb.is_empty());
        assert_eq!(bb.count(), 1);
        assert!(bb.contains(Square::E4 as u8));
        assert_eq!(bb.lsb(), Square::E4 as u8);

        bb.set(Square::A1 as u8);
        assert_eq!(bb.count(), 2);
        assert!(bb.more_than_one());
        assert_eq!(bb.lsb(), Square::A1 as u8);

        let popped = bb.pop_lsb();
        assert_eq!(popped, Square::A1 as u8);
        assert_eq!(bb.count(), 1);
        assert_eq!(bb.lsb(), Square::E4 as u8);
    }

    #[test]
    fn test_piece_char() {
        let wp = Piece::new(Color::White, PieceType::Knight);
        assert_eq!(wp.to_char(), 'N');
        assert_eq!(Piece::from_char('N'), Some(wp));

        let bp = Piece::new(Color::Black, PieceType::Queen);
        assert_eq!(bp.to_char(), 'q');
        assert_eq!(Piece::from_char('q'), Some(bp));
    }
}

