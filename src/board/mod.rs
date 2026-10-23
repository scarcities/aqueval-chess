pub mod fen;
pub mod make_move;
pub mod zobrist;

use crate::board::fen::{format_fen, parse_fen, ParseFenError};
use crate::board::make_move::UndoState;
use crate::board::zobrist::{castling_key, ep_key, piece_key, side_key};
use crate::types::{Bitboard, Color, Piece, PieceType};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub pieces: [Bitboard; 6],
    pub colors: [Bitboard; 2],
    pub piece_at: [Option<Piece>; 64],
    pub side_to_move: Color,
    pub castling_rights: u8,
    pub ep_square: Option<u8>,
    pub halfmove_clock: u16,
    pub fullmove_number: u16,
    pub zobrist_hash: u64,
    pub history: Vec<UndoState>,
}

impl Board {
    pub fn empty() -> Self {
        crate::movegen::magics::init();
        Self {
            pieces: [Bitboard::EMPTY; 6],
            colors: [Bitboard::EMPTY; 2],
            piece_at: [None; 64],
            side_to_move: Color::White,
            castling_rights: 0,
            ep_square: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            zobrist_hash: 0,
            history: Vec::with_capacity(512),
        }
    }

    pub fn startpos() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("startpos FEN is valid")
    }

    pub fn from_fen(fen: &str) -> Result<Self, ParseFenError> {
        crate::movegen::magics::init();
        let parsed = parse_fen(fen)?;
        let mut pieces = [Bitboard::EMPTY; 6];
        let mut colors = [Bitboard::EMPTY; 2];

        for sq in 0..64 {
            if let Some(piece) = parsed.piece_at[sq] {
                pieces[piece.piece_type.index()].set(sq as u8);
                colors[piece.color.index()].set(sq as u8);
            }
        }

        let mut board = Self {
            pieces,
            colors,
            piece_at: parsed.piece_at,
            side_to_move: parsed.side_to_move,
            castling_rights: parsed.castling_rights,
            ep_square: parsed.ep_square,
            halfmove_clock: parsed.halfmove_clock,
            fullmove_number: parsed.fullmove_number,
            zobrist_hash: 0,
            history: Vec::with_capacity(512),
        };
        // The side not to move must not be in check: the movegen would capture its king.
        let them = !board.side_to_move;
        if board.is_square_attacked_by(board.king_square(them), board.side_to_move, board.occupied()) {
            return Err(ParseFenError::InvalidPiecePlacement);
        }
        board.zobrist_hash = board.calculate_zobrist();
        Ok(board)
    }

    pub fn to_fen(&self) -> String {
        format_fen(
            &self.piece_at,
            self.side_to_move,
            self.castling_rights,
            self.ep_square,
            self.halfmove_clock,
            self.fullmove_number,
        )
    }

    #[inline(always)]
    pub fn occupied(&self) -> Bitboard {
        self.colors[0] | self.colors[1]
    }

    #[inline(always)]
    pub fn piece_at(&self, sq: u8) -> Option<Piece> {
        debug_assert!(sq < 64);
        self.piece_at[sq as usize]
    }

    #[inline(always)]
    pub fn king_square(&self, color: Color) -> u8 {
        let king_bb = self.pieces[PieceType::King.index()] & self.colors[color.index()];
        debug_assert!(king_bb.is_not_empty(), "King must exist on board");
        king_bb.lsb()
    }

    #[inline(always)]
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    #[inline(always)]
    pub fn castling_rights(&self) -> u8 {
        self.castling_rights
    }

    #[inline(always)]
    pub fn ep_square(&self) -> Option<u8> {
        self.ep_square
    }

    #[inline(always)]
    pub fn halfmove_clock(&self) -> u8 {
        self.halfmove_clock.min(255) as u8
    }

    #[inline(always)]
    pub fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    #[inline(always)]
    pub fn zobrist_hash(&self) -> u64 {
        self.zobrist_hash
    }

    pub fn calculate_zobrist(&self) -> u64 {
        let mut hash = 0u64;
        for sq in 0..64 {
            if let Some(piece) = self.piece_at[sq] {
                hash ^= piece_key(piece.color, piece.piece_type, sq as u8);
            }
        }
        hash ^= castling_key(self.castling_rights);
        if let Some(ep) = self.ep_square {
            hash ^= ep_key(ep & 7);
        }
        if self.side_to_move == Color::Black {
            hash ^= side_key();
        }
        hash
    }

    pub fn is_repetition(&self) -> bool {
        let current_hash = self.zobrist_hash;
        let mut count = 1;
        let total_states = self.history.len();
        let limit = total_states.saturating_sub(self.halfmove_clock as usize);

        if total_states >= 2 {
            let mut idx = total_states - 2;
            loop {
                if idx < limit {
                    break;
                }
                if self.history[idx].zobrist_hash == current_hash {
                    count += 1;
                    if count >= 3 {
                        return true;
                    }
                }
                if idx < 2 {
                    break;
                }
                idx -= 2;
            }
        }
        false
    }
}
