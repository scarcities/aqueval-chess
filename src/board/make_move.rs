use crate::board::Board;
use crate::board::fen::{BLACK_OO, BLACK_OOO, WHITE_OO, WHITE_OOO};
use crate::board::zobrist::{castling_key, ep_key, piece_key, side_key};
use crate::movegen::types::Move;
use crate::types::{Color, Piece, PieceType};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct UndoState {
    pub captured_piece: Option<Piece>,
    pub castling_rights: u8,
    pub ep_square: Option<u8>,
    pub halfmove_clock: u16,
    pub zobrist_hash: u64,
}

pub const CASTLING_RIGHTS_MASK: [u8; 64] = {
    let mut mask = [0x0F; 64];
    mask[0] &= !WHITE_OOO; // A1 rook moved / captured -> clear White Queenside
    mask[4] &= !(WHITE_OO | WHITE_OOO); // E1 king moved -> clear White both
    mask[7] &= !WHITE_OO; // H1 rook moved / captured -> clear White Kingside
    mask[56] &= !BLACK_OOO; // A8 rook moved / captured -> clear Black Queenside
    mask[60] &= !(BLACK_OO | BLACK_OOO); // E8 king moved -> clear Black both
    mask[63] &= !BLACK_OO; // H8 rook moved / captured -> clear Black Kingside
    mask
};

impl Board {
    pub fn make_move(&mut self, m: Move) -> UndoState {
        let us = self.side_to_move;
        let them = !us;
        let from = m.from_sq();
        let to = m.to_sq();
        let piece = self.piece_at[from as usize].expect("moving piece must exist on board");

        let undo = UndoState {
            captured_piece: self.piece_at[to as usize],
            castling_rights: self.castling_rights,
            ep_square: self.ep_square,
            halfmove_clock: self.halfmove_clock,
            zobrist_hash: self.zobrist_hash,
        };

        let mut hash = self.zobrist_hash;

        // 1. Remove moving piece from `from`
        self.pieces[piece.piece_type.index()].clear(from);
        self.colors[us.index()].clear(from);
        self.piece_at[from as usize] = None;
        hash ^= piece_key(us, piece.piece_type, from);

        // 2. Handle captured piece
        if m.is_en_passant() {
            let victim_sq = if us == Color::White { to - 8 } else { to + 8 };
            self.pieces[PieceType::Pawn.index()].clear(victim_sq);
            self.colors[them.index()].clear(victim_sq);
            self.piece_at[victim_sq as usize] = None;
            hash ^= piece_key(them, PieceType::Pawn, victim_sq);
        } else if let Some(captured) = undo.captured_piece {
            self.pieces[captured.piece_type.index()].clear(to);
            self.colors[them.index()].clear(to);
            hash ^= piece_key(them, captured.piece_type, to);
        }

        // 3. Place piece at `to`
        let final_piece_type = if let Some(promo) = m.promotion_piece() {
            promo
        } else {
            piece.piece_type
        };
        let final_piece = Piece::new(us, final_piece_type);
        self.pieces[final_piece_type.index()].set(to);
        self.colors[us.index()].set(to);
        self.piece_at[to as usize] = Some(final_piece);
        hash ^= piece_key(us, final_piece_type, to);

        // 4. Handle Castling rook move
        if m.is_castle() {
            let (rook_from, rook_to) = match (us, m.is_king_castle()) {
                (Color::White, true) => (7, 5),   // H1 -> F1
                (Color::White, false) => (0, 3),  // A1 -> D1
                (Color::Black, true) => (63, 61), // H8 -> F8
                (Color::Black, false) => (56, 59),// A8 -> D8
            };
            self.pieces[PieceType::Rook.index()].clear(rook_from);
            self.colors[us.index()].clear(rook_from);
            self.piece_at[rook_from as usize] = None;
            hash ^= piece_key(us, PieceType::Rook, rook_from);

            self.pieces[PieceType::Rook.index()].set(rook_to);
            self.colors[us.index()].set(rook_to);
            self.piece_at[rook_to as usize] = Some(Piece::new(us, PieceType::Rook));
            hash ^= piece_key(us, PieceType::Rook, rook_to);
        }

        // 5. Update castling rights
        let new_rights = self.castling_rights
            & CASTLING_RIGHTS_MASK[from as usize]
            & CASTLING_RIGHTS_MASK[to as usize];
        if new_rights != self.castling_rights {
            hash ^= castling_key(self.castling_rights) ^ castling_key(new_rights);
            self.castling_rights = new_rights;
        }

        // 6. Update EP square
        if let Some(old_ep) = self.ep_square {
            hash ^= ep_key(old_ep & 7);
            self.ep_square = None;
        }
        if m.is_double_push() {
            let ep_sq = if us == Color::White { from + 8 } else { from - 8 };
            self.ep_square = Some(ep_sq);
            hash ^= ep_key(ep_sq & 7);
        }

        // 7. Update clocks
        if piece.piece_type == PieceType::Pawn || m.is_capture() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }

        if us == Color::Black {
            self.fullmove_number += 1;
        }

        // 8. Toggle side to move
        self.side_to_move = them;
        hash ^= side_key();
        self.zobrist_hash = hash;

        self.history.push(undo);
        undo
    }

    pub fn unmake_move(&mut self, m: Move, undo: &UndoState) {
        let us = !self.side_to_move; // Player who made move `m`
        let them = self.side_to_move;
        let from = m.from_sq();
        let to = m.to_sq();

        // 1. Restore history stack
        self.history.pop();

        // 2. Remove piece from `to`
        let moved_piece_type = if m.is_promotion() {
            m.promotion_piece().unwrap()
        } else {
            self.piece_at[to as usize].expect("piece must exist at destination on unmake").piece_type
        };
        self.pieces[moved_piece_type.index()].clear(to);
        self.colors[us.index()].clear(to);
        self.piece_at[to as usize] = None;

        // 3. Put original piece back at `from`
        let orig_piece_type = if m.is_promotion() {
            PieceType::Pawn
        } else {
            moved_piece_type
        };
        self.pieces[orig_piece_type.index()].set(from);
        self.colors[us.index()].set(from);
        self.piece_at[from as usize] = Some(Piece::new(us, orig_piece_type));

        // 4. Restore captured piece
        if m.is_en_passant() {
            let victim_sq = if us == Color::White { to - 8 } else { to + 8 };
            self.pieces[PieceType::Pawn.index()].set(victim_sq);
            self.colors[them.index()].set(victim_sq);
            self.piece_at[victim_sq as usize] = Some(Piece::new(them, PieceType::Pawn));
        } else if let Some(captured) = undo.captured_piece {
            self.pieces[captured.piece_type.index()].set(to);
            self.colors[them.index()].set(to);
            self.piece_at[to as usize] = Some(captured);
        }

        // 5. Restore castling rook
        if m.is_castle() {
            let (rook_from, rook_to) = match (us, m.is_king_castle()) {
                (Color::White, true) => (7, 5),   // H1 -> F1
                (Color::White, false) => (0, 3),  // A1 -> D1
                (Color::Black, true) => (63, 61), // H8 -> F8
                (Color::Black, false) => (56, 59),// A8 -> D8
            };
            self.pieces[PieceType::Rook.index()].clear(rook_to);
            self.colors[us.index()].clear(rook_to);
            self.piece_at[rook_to as usize] = None;

            self.pieces[PieceType::Rook.index()].set(rook_from);
            self.colors[us.index()].set(rook_from);
            self.piece_at[rook_from as usize] = Some(Piece::new(us, PieceType::Rook));
        }

        // 6. Restore scalar state from undo
        self.castling_rights = undo.castling_rights;
        self.ep_square = undo.ep_square;
        self.halfmove_clock = undo.halfmove_clock;
        self.zobrist_hash = undo.zobrist_hash;

        if us == Color::Black {
            self.fullmove_number -= 1;
        }

        // 7. Restore side to move
        self.side_to_move = us;
    }
}
