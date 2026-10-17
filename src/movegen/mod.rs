pub mod attacks;
pub mod magic_constants;
pub mod magics;
pub mod types;

use crate::board::Board;
use crate::board::fen::{BLACK_OO, BLACK_OOO, WHITE_OO, WHITE_OOO};
use crate::movegen::attacks::{between, get_king_attacks, get_knight_attacks, get_pawn_attacks, line};
use crate::movegen::magics::{get_bishop_attacks, get_queen_attacks, get_rook_attacks};
use crate::movegen::types::{Move, MoveList};
use crate::types::{Bitboard, Color, PieceType};

impl Board {
    pub fn is_square_attacked_by(&self, sq: u8, by: Color, occ: Bitboard) -> bool {
        let them_occ = self.colors[by.index()];

        // Knight attacks
        if (get_knight_attacks(sq) & self.pieces[PieceType::Knight.index()] & them_occ).is_not_empty() {
            return true;
        }

        // Pawn attacks
        let pawn_attacker_sqs = get_pawn_attacks(!by, sq);
        if (pawn_attacker_sqs & self.pieces[PieceType::Pawn.index()] & them_occ).is_not_empty() {
            return true;
        }

        // King attacks
        if (get_king_attacks(sq) & self.pieces[PieceType::King.index()] & them_occ).is_not_empty() {
            return true;
        }

        // Bishop / Queen diagonal attacks
        let diags = self.pieces[PieceType::Bishop.index()] | self.pieces[PieceType::Queen.index()];
        if (get_bishop_attacks(sq, occ) & diags & them_occ).is_not_empty() {
            return true;
        }

        // Rook / Queen straight attacks
        let straights = self.pieces[PieceType::Rook.index()] | self.pieces[PieceType::Queen.index()];
        if (get_rook_attacks(sq, occ) & straights & them_occ).is_not_empty() {
            return true;
        }

        false
    }

    #[inline(always)]
    pub fn is_in_check(&self) -> bool {
        let us = self.side_to_move;
        let ksq = self.king_square(us);
        self.is_square_attacked_by(ksq, !us, self.occupied())
    }

    pub fn generate_legal_moves(&self, moves: &mut MoveList) {
        let us = self.side_to_move;
        let them = !us;
        let ksq = self.king_square(us);
        let occ = self.occupied();
        let us_occ = self.colors[us.index()];
        let them_occ = self.colors[them.index()];

        // 1. Calculate checkers
        let knight_checkers = get_knight_attacks(ksq) & self.pieces[PieceType::Knight.index()] & them_occ;
        let pawn_checkers = get_pawn_attacks(us, ksq) & self.pieces[PieceType::Pawn.index()] & them_occ;
        let diag_checkers = get_bishop_attacks(ksq, occ)
            & (self.pieces[PieceType::Bishop.index()] | self.pieces[PieceType::Queen.index()])
            & them_occ;
        let straight_checkers = get_rook_attacks(ksq, occ)
            & (self.pieces[PieceType::Rook.index()] | self.pieces[PieceType::Queen.index()])
            & them_occ;

        let checkers = knight_checkers | pawn_checkers | diag_checkers | straight_checkers;
        let num_checkers = checkers.count();

        // 2. Generate King moves
        let king_vacated_occ = occ ^ Bitboard::from_sq(ksq);
        let king_targets = get_king_attacks(ksq) & !us_occ;
        for to in king_targets {
            if !self.is_square_attacked_by(to, them, king_vacated_occ) {
                if them_occ.contains(to) {
                    moves.push(Move::new(ksq, to, Move::CAPTURE));
                } else {
                    moves.push(Move::new(ksq, to, Move::QUIET));
                }
            }
        }

        // If double check, ONLY king moves are legal!
        if num_checkers >= 2 {
            return;
        }

        // 3. Determine check evasion mask
        let check_mask = if num_checkers == 1 {
            let checker_sq = checkers.lsb();
            Bitboard::from_sq(checker_sq) | between(ksq, checker_sq)
        } else {
            Bitboard::ALL
        };

        // 4. Castling moves (only when not in check)
        if num_checkers == 0 {
            if us == Color::White {
                // White Kingside: E1(4) -> G1(6), Rook H1(7) -> F1(5)
                if self.castling_rights & WHITE_OO != 0 {
                    let path_mask = (1u64 << 5) | (1u64 << 6);
                    if (occ.0 & path_mask) == 0
                        && !self.is_square_attacked_by(5, Color::Black, occ)
                        && !self.is_square_attacked_by(6, Color::Black, occ)
                    {
                        moves.push(Move::new(4, 6, Move::KING_CASTLE));
                    }
                }
                // White Queenside: E1(4) -> C1(2), Rook A1(0) -> D1(3)
                if self.castling_rights & WHITE_OOO != 0 {
                    let path_mask = (1u64 << 1) | (1u64 << 2) | (1u64 << 3);
                    if (occ.0 & path_mask) == 0
                        && !self.is_square_attacked_by(3, Color::Black, occ)
                        && !self.is_square_attacked_by(2, Color::Black, occ)
                    {
                        moves.push(Move::new(4, 2, Move::QUEEN_CASTLE));
                    }
                }
            } else {
                // Black Kingside: E8(60) -> G8(62), Rook H8(63) -> F8(61)
                if self.castling_rights & BLACK_OO != 0 {
                    let path_mask = (1u64 << 61) | (1u64 << 62);
                    if (occ.0 & path_mask) == 0
                        && !self.is_square_attacked_by(61, Color::White, occ)
                        && !self.is_square_attacked_by(62, Color::White, occ)
                    {
                        moves.push(Move::new(60, 62, Move::KING_CASTLE));
                    }
                }
                // Black Queenside: E8(60) -> C8(58), Rook A8(56) -> D8(59)
                if self.castling_rights & BLACK_OOO != 0 {
                    let path_mask = (1u64 << 57) | (1u64 << 58) | (1u64 << 59);
                    if (occ.0 & path_mask) == 0
                        && !self.is_square_attacked_by(59, Color::White, occ)
                        && !self.is_square_attacked_by(58, Color::White, occ)
                    {
                        moves.push(Move::new(60, 58, Move::QUEEN_CASTLE));
                    }
                }
            }
        }

        // 5. Pin calculations
        let mut pinned_bb = Bitboard::EMPTY;
        let mut pin_ray = [Bitboard::ALL; 64];

        // Diagonal pinners
        let candidate_diag_pinners = get_bishop_attacks(ksq, Bitboard::EMPTY)
            & (self.pieces[PieceType::Bishop.index()] | self.pieces[PieceType::Queen.index()])
            & them_occ;
        for pinner_sq in candidate_diag_pinners {
            let ray = between(ksq, pinner_sq);
            let blockers = ray & occ;
            if blockers.count() == 1 {
                let blocker_sq = blockers.lsb();
                if us_occ.contains(blocker_sq) {
                    pinned_bb.set(blocker_sq);
                    pin_ray[blocker_sq as usize] = line(ksq, pinner_sq);
                }
            }
        }

        // Straight pinners
        let candidate_straight_pinners = get_rook_attacks(ksq, Bitboard::EMPTY)
            & (self.pieces[PieceType::Rook.index()] | self.pieces[PieceType::Queen.index()])
            & them_occ;
        for pinner_sq in candidate_straight_pinners {
            let ray = between(ksq, pinner_sq);
            let blockers = ray & occ;
            if blockers.count() == 1 {
                let blocker_sq = blockers.lsb();
                if us_occ.contains(blocker_sq) {
                    pinned_bb.set(blocker_sq);
                    pin_ray[blocker_sq as usize] = line(ksq, pinner_sq);
                }
            }
        }

        // 6. Generate Pawn moves
        let pawns = self.pieces[PieceType::Pawn.index()] & us_occ;
        for from in pawns {
            let ray_mask = if pinned_bb.contains(from) {
                pin_ray[from as usize]
            } else {
                Bitboard::ALL
            };
            let valid_targets = check_mask & ray_mask;

            if us == Color::White {
                let to = from + 8;
                if !occ.contains(to) {
                    if valid_targets.contains(to) {
                        if to >= 56 {
                            moves.push(Move::new(from, to, Move::PROMO_QUEEN));
                            moves.push(Move::new(from, to, Move::PROMO_ROOK));
                            moves.push(Move::new(from, to, Move::PROMO_BISHOP));
                            moves.push(Move::new(from, to, Move::PROMO_KNIGHT));
                        } else {
                            moves.push(Move::new(from, to, Move::QUIET));
                        }
                    }
                    // Double push
                    if from / 8 == 1 && !occ.contains(from + 16) && valid_targets.contains(from + 16) {
                        moves.push(Move::new(from, from + 16, Move::DOUBLE_PAWN_PUSH));
                    }
                }
                // Captures
                let caps = get_pawn_attacks(Color::White, from) & them_occ & valid_targets;
                for to in caps {
                    if to >= 56 {
                        moves.push(Move::new(from, to, Move::PROMO_CAP_QUEEN));
                        moves.push(Move::new(from, to, Move::PROMO_CAP_ROOK));
                        moves.push(Move::new(from, to, Move::PROMO_CAP_BISHOP));
                        moves.push(Move::new(from, to, Move::PROMO_CAP_KNIGHT));
                    } else {
                        moves.push(Move::new(from, to, Move::CAPTURE));
                    }
                }
            } else {
                let to = from - 8;
                if !occ.contains(to) {
                    if valid_targets.contains(to) {
                        if to <= 7 {
                            moves.push(Move::new(from, to, Move::PROMO_QUEEN));
                            moves.push(Move::new(from, to, Move::PROMO_ROOK));
                            moves.push(Move::new(from, to, Move::PROMO_BISHOP));
                            moves.push(Move::new(from, to, Move::PROMO_KNIGHT));
                        } else {
                            moves.push(Move::new(from, to, Move::QUIET));
                        }
                    }
                    // Double push
                    if from / 8 == 6 && !occ.contains(from - 16) && valid_targets.contains(from - 16) {
                        moves.push(Move::new(from, from - 16, Move::DOUBLE_PAWN_PUSH));
                    }
                }
                // Captures
                let caps = get_pawn_attacks(Color::Black, from) & them_occ & valid_targets;
                for to in caps {
                    if to <= 7 {
                        moves.push(Move::new(from, to, Move::PROMO_CAP_QUEEN));
                        moves.push(Move::new(from, to, Move::PROMO_CAP_ROOK));
                        moves.push(Move::new(from, to, Move::PROMO_CAP_BISHOP));
                        moves.push(Move::new(from, to, Move::PROMO_CAP_KNIGHT));
                    } else {
                        moves.push(Move::new(from, to, Move::CAPTURE));
                    }
                }
            }
        }

        // 7. En Passant
        if let Some(ep_sq) = self.ep_square {
            let victim_sq = if us == Color::White { ep_sq - 8 } else { ep_sq + 8 };
            let candidate_pawns = get_pawn_attacks(!us, ep_sq) & self.pieces[PieceType::Pawn.index()] & us_occ;

            for from in candidate_pawns {
                // If in check, en passant must either capture the checker (victim_sq) or block the check
                if num_checkers == 1 {
                    let checker_sq = checkers.lsb();
                    if checker_sq != victim_sq && !between(ksq, checker_sq).contains(ep_sq) {
                        continue;
                    }
                }

                // If pawn is pinned, moving to ep_sq must follow the pin ray
                if pinned_bb.contains(from) && !pin_ray[from as usize].contains(ep_sq) {
                    continue;
                }

                // Horizontal and diagonal discovered check check
                let occ_sim = (occ.0 ^ (1u64 << from) ^ (1u64 << victim_sq)) | (1u64 << ep_sq);
                let enemy_straights = (self.pieces[PieceType::Rook.index()] | self.pieces[PieceType::Queen.index()]) & them_occ;
                if (get_rook_attacks(ksq, Bitboard(occ_sim)) & enemy_straights).is_not_empty() {
                    continue;
                }
                let enemy_diags = (self.pieces[PieceType::Bishop.index()] | self.pieces[PieceType::Queen.index()]) & them_occ;
                if (get_bishop_attacks(ksq, Bitboard(occ_sim)) & enemy_diags).is_not_empty() {
                    continue;
                }

                moves.push(Move::new(from, ep_sq, Move::EN_PASSANT));
            }
        }

        // 8. Knights
        let knights = self.pieces[PieceType::Knight.index()] & us_occ;
        for from in knights {
            if pinned_bb.contains(from) {
                continue; // Pinned knight can never move legally
            }
            let targets = get_knight_attacks(from) & !us_occ & check_mask;
            for to in targets {
                if them_occ.contains(to) {
                    moves.push(Move::new(from, to, Move::CAPTURE));
                } else {
                    moves.push(Move::new(from, to, Move::QUIET));
                }
            }
        }

        // 9. Bishops
        let bishops = self.pieces[PieceType::Bishop.index()] & us_occ;
        for from in bishops {
            let ray_mask = if pinned_bb.contains(from) {
                pin_ray[from as usize]
            } else {
                Bitboard::ALL
            };
            let targets = get_bishop_attacks(from, occ) & !us_occ & check_mask & ray_mask;
            for to in targets {
                if them_occ.contains(to) {
                    moves.push(Move::new(from, to, Move::CAPTURE));
                } else {
                    moves.push(Move::new(from, to, Move::QUIET));
                }
            }
        }

        // 10. Rooks
        let rooks = self.pieces[PieceType::Rook.index()] & us_occ;
        for from in rooks {
            let ray_mask = if pinned_bb.contains(from) {
                pin_ray[from as usize]
            } else {
                Bitboard::ALL
            };
            let targets = get_rook_attacks(from, occ) & !us_occ & check_mask & ray_mask;
            for to in targets {
                if them_occ.contains(to) {
                    moves.push(Move::new(from, to, Move::CAPTURE));
                } else {
                    moves.push(Move::new(from, to, Move::QUIET));
                }
            }
        }

        // 11. Queens
        let queens = self.pieces[PieceType::Queen.index()] & us_occ;
        for from in queens {
            let ray_mask = if pinned_bb.contains(from) {
                pin_ray[from as usize]
            } else {
                Bitboard::ALL
            };
            let targets = get_queen_attacks(from, occ) & !us_occ & check_mask & ray_mask;
            for to in targets {
                if them_occ.contains(to) {
                    moves.push(Move::new(from, to, Move::CAPTURE));
                } else {
                    moves.push(Move::new(from, to, Move::QUIET));
                }
            }
        }
    }
}
