use crate::board::Board;
use crate::movegen::attacks::{get_king_attacks, get_knight_attacks, get_pawn_attacks};
use crate::movegen::magics::{get_bishop_attacks, get_rook_attacks};
use crate::movegen::types::Move;
use crate::search::evaluate::piece_value;
use crate::types::{Bitboard, Color, PieceType};

/// All pieces of both colours attacking `sq` with occupancy `occ`.
fn attackers_to(board: &Board, sq: u8, occ: Bitboard) -> Bitboard {
    let p = |pt: PieceType| board.pieces[pt.index()];
    let white = board.colors[Color::White.index()];
    let black = board.colors[Color::Black.index()];
    (get_pawn_attacks(Color::Black, sq) & p(PieceType::Pawn) & white)
        | (get_pawn_attacks(Color::White, sq) & p(PieceType::Pawn) & black)
        | (get_knight_attacks(sq) & p(PieceType::Knight))
        | (get_king_attacks(sq) & p(PieceType::King))
        | (get_bishop_attacks(sq, occ) & (p(PieceType::Bishop) | p(PieceType::Queen)))
        | (get_rook_attacks(sq, occ) & (p(PieceType::Rook) | p(PieceType::Queen)))
}

/// Static exchange evaluation: does `m` win at least `threshold` centipawns once all
/// captures on the target square are played out, least valuable attacker first?
/// shortcut: castling, en passant and promotions count as SEE 0 and pins are ignored; refine if SEE pruning needs it.
pub fn see_ge(board: &Board, m: Move, threshold: i32) -> bool {
    if m.is_castle() || m.is_en_passant() || m.is_promotion() {
        return 0 >= threshold;
    }
    let (from, to) = (m.from_sq(), m.to_sq());
    let value_on = |sq: u8| board.piece_at(sq).map_or(0, |p| piece_value(p.piece_type));

    let mut swap = value_on(to) - threshold;
    if swap < 0 {
        return false;
    }
    swap = value_on(from) - swap;
    if swap <= 0 {
        return true;
    }

    let diag = board.pieces[PieceType::Bishop.index()] | board.pieces[PieceType::Queen.index()];
    let straight = board.pieces[PieceType::Rook.index()] | board.pieces[PieceType::Queen.index()];
    let mut occ = board.occupied() ^ Bitboard::from_sq(from) ^ Bitboard::from_sq(to);
    let mut attackers = attackers_to(board, to, occ);
    let mut stm = board.side_to_move;
    let mut res = true;

    loop {
        stm = !stm;
        attackers &= occ;
        let ours = attackers & board.colors[stm.index()];
        if ours.is_empty() {
            break;
        }
        res = !res;

        let Some(pt) = [PieceType::Pawn, PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen]
            .into_iter()
            .find(|&pt| (ours & board.pieces[pt.index()]).is_not_empty())
        else {
            // Only the king can recapture: legal only if the opponent has no attacker left.
            return if (attackers & board.colors[(!stm).index()]).is_not_empty() { !res } else { res };
        };

        swap = piece_value(pt) - swap;
        if swap < res as i32 {
            break;
        }
        occ ^= Bitboard::from_sq((ours & board.pieces[pt.index()]).lsb());
        // Removing the capturer may uncover sliders behind it.
        if matches!(pt, PieceType::Pawn | PieceType::Bishop | PieceType::Queen) {
            attackers |= get_bishop_attacks(to, occ) & diag;
        }
        if matches!(pt, PieceType::Rook | PieceType::Queen) {
            attackers |= get_rook_attacks(to, occ) & straight;
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movegen::types::MoveList;

    fn see(fen: &str, uci: &str, threshold: i32) -> bool {
        let b = Board::from_fen(fen).unwrap();
        let mut list = MoveList::new();
        b.generate_legal_moves(&mut list);
        let m = *list.as_slice().iter().find(|m| m.to_uci() == uci).expect("legal move");
        see_ge(&b, m, threshold)
    }

    #[test]
    fn test_see() {
        // Rook takes an undefended pawn: +100.
        let free_pawn = "1k1r4/1pp4p/p7/4p3/8/P5P1/1PP4P/2K1R3 w - - 0 1";
        assert!(see(free_pawn, "e1e5", 100));
        assert!(!see(free_pawn, "e1e5", 101));
        // Knight takes a pawn defended by a pawn: 100 - 320 < 0.
        let defended = "4k3/8/3p4/4p3/8/5N2/8/4K3 w - - 0 1";
        assert!(!see(defended, "f3e5", 0));
        assert!(see(defended, "f3e5", -220));
        // Queen moves to a square attacked by a pawn: loses the queen.
        let hanging = "3k4/8/8/3p4/8/8/4Q3/4K3 w - - 0 1";
        assert!(!see(hanging, "e2c4", 0));
        assert!(see(hanging, "e2e3", 0));
        // X-ray: rook takes pawn, defended by rook, backed by our second rook: wins a pawn.
        let xray = "3r3k/3p4/8/8/8/8/3R4/3RK3 w - - 0 1";
        assert!(see(xray, "d2d7", 100));
    }
}
