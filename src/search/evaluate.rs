use crate::board::Board;
use crate::types::{Color, PieceType};

/// Material piece values in centipawns.
pub const PAWN_VALUE: i32 = 100;
pub const KNIGHT_VALUE: i32 = 320;
pub const BISHOP_VALUE: i32 = 330;
pub const ROOK_VALUE: i32 = 500;
pub const QUEEN_VALUE: i32 = 900;

/// Returns the material value of a piece type in centipawns.
#[inline(always)]
pub fn piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => PAWN_VALUE,
        PieceType::Knight => KNIGHT_VALUE,
        PieceType::Bishop => BISHOP_VALUE,
        PieceType::Rook => ROOK_VALUE,
        PieceType::Queen => QUEEN_VALUE,
        PieceType::King => 0,
    }
}

// Tapered evaluation: every term has a middlegame and an endgame value, blended by game phase.
// Values are Quant's own, built from a few simple shapes (centrality, pawn advancement,
// king shelter) rather than tuned tables. Texel tuning comes later in Phase 2.

/// Middlegame / endgame material: P, N, B, R, Q, K.
const MATERIAL_MG: [i32; 6] = [90, 320, 335, 480, 950, 0];
const MATERIAL_EG: [i32; 6] = [110, 300, 320, 530, 980, 0];
/// Phase weight per piece type; 24 = all minor and major pieces on the board.
const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// Distance-from-edge ring: 0 on the rim, 3 on d4/e4/d5/e5.
const fn ring(sq: usize) -> i32 {
    let (f, r) = ((sq % 8) as i32, (sq / 8) as i32);
    let fd = if f < 7 - f { f } else { 7 - f };
    let rd = if r < 7 - r { r } else { 7 - r };
    if fd < rd { fd } else { rd }
}

/// Piece-square bonus from White's point of view (square index LERF, A1 = 0).
const fn pst_value(pt: usize, sq: usize, endgame: bool) -> i32 {
    let (f, r) = ((sq % 8) as i32, (sq / 8) as i32);
    let c = ring(sq);
    let central_file = f == 3 || f == 4;
    match (pt, endgame) {
        // Pawns: advance; in the middlegame mostly the centre pawns.
        (0, false) => {
            let adv = [0, 0, 4, 8, 16, 28, 50, 0][r as usize];
            adv + if central_file && (r == 3 || r == 4) { 12 } else { 0 }
                - if central_file && r == 1 { 6 } else { 0 }
        }
        (0, true) => [0, 0, 8, 18, 32, 55, 90, 0][r as usize],
        (1, false) => -30 + 14 * c,
        (1, true) => -24 + 10 * c,
        (2, false) => -10 + 6 * c,
        (2, true) => -8 + 5 * c,
        // Rooks: seventh rank and central files in the middlegame.
        (3, false) => (if r == 6 { 15 } else { 0 }) + (if central_file { 5 } else { 0 }),
        (3, true) => if r == 6 { 10 } else { 0 },
        (4, false) => -6 + 3 * c,
        (4, true) => -12 + 7 * c,
        // King: castled corners in the middlegame, centre in the endgame.
        (5, false) => {
            if r == 0 {
                [10, 25, 10, -10, -15, -10, 30, 15][f as usize]
            } else {
                -20 * (if r < 3 { r } else { 3 })
            }
        }
        _ => -30 + 15 * c,
    }
}

const fn build_pst(endgame: bool) -> [[i32; 64]; 6] {
    let mut t = [[0; 64]; 6];
    let mut pt = 0;
    while pt < 6 {
        let mut sq = 0;
        while sq < 64 {
            t[pt][sq] = pst_value(pt, sq, endgame);
            sq += 1;
        }
        pt += 1;
    }
    t
}

const PST_MG: [[i32; 64]; 6] = build_pst(false);
const PST_EG: [[i32; 64]; 6] = build_pst(true);

/// Static evaluation in centipawns, relative to the side to move (positive = side to move is better).
pub fn evaluate(board: &Board) -> i32 {
    let (mut mg, mut eg, mut phase) = (0, 0, 0);
    for (color, sign) in [(Color::White, 1), (Color::Black, -1)] {
        let ours = board.colors[color.index()];
        for pt in 0..6 {
            let mut bb = board.pieces[pt] & ours;
            while bb.is_not_empty() {
                let sq = bb.pop_lsb() as usize;
                // Black uses the same tables mirrored vertically.
                let rel = if color == Color::White { sq } else { sq ^ 56 };
                mg += sign * (MATERIAL_MG[pt] + PST_MG[pt][rel]);
                eg += sign * (MATERIAL_EG[pt] + PST_EG[pt][rel]);
                phase += PHASE_WEIGHT[pt];
            }
        }
    }
    let phase = phase.min(MAX_PHASE);
    let white_score = (mg * phase + eg * (MAX_PHASE - phase)) / MAX_PHASE;
    if board.side_to_move == Color::White { white_score } else { -white_score }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_startpos_is_zero() {
        let board = Board::startpos();
        assert_eq!(evaluate(&board), 0);
    }

    #[test]
    fn test_eval_material_advantage() {
        // White to move with an extra queen
        let board = Board::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        assert!(evaluate(&board) > 800);
        // Black to move with an extra knight
        let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/R1BQKBNR b KQkq - 0 1").unwrap();
        assert!(evaluate(&board) > 250);
    }

    /// Swaps colours and mirrors ranks; the side to move flips too.
    fn flip_fen(fen: &str) -> String {
        let f: Vec<&str> = fen.split(' ').collect();
        let swap = |c: char| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() };
        let ranks: Vec<String> = f[0].split('/').rev().map(|r| r.chars().map(swap).collect()).collect();
        let stm = if f[1] == "w" { "b" } else { "w" };
        let castling: String = if f[2] == "-" { "-".into() } else { f[2].chars().map(swap).collect() };
        format!("{} {} {} - 0 1", ranks.join("/"), stm, castling)
    }

    #[test]
    fn test_eval_is_colour_symmetric() {
        for fen in [
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        ] {
            let a = evaluate(&Board::from_fen(fen).unwrap());
            let b = evaluate(&Board::from_fen(&flip_fen(fen)).unwrap());
            assert_eq!(a, b, "{fen}");
        }
    }

    #[test]
    fn test_eval_prefers_central_knight_and_advanced_pawn() {
        let rim = Board::from_fen("4k3/8/8/8/8/8/8/N3K3 w - - 0 1").unwrap();
        let centre = Board::from_fen("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1").unwrap();
        assert!(evaluate(&centre) > evaluate(&rim));
        let back = Board::from_fen("4k3/8/8/8/8/8/P7/4K3 w - - 0 1").unwrap();
        let far = Board::from_fen("4k3/P7/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(evaluate(&far) > evaluate(&back) + 50);
    }
}
