use crate::types::{Color, Piece, Square};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseFenError {
    InvalidFormat,
    InvalidPiecePlacement,
    InvalidActiveColor,
    InvalidCastlingRights,
    InvalidEnPassant,
    InvalidHalfmoveClock,
    InvalidFullmoveNumber,
}

impl std::fmt::Display for ParseFenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseFenError::InvalidFormat => write!(f, "Invalid FEN format: expected 6 fields"),
            ParseFenError::InvalidPiecePlacement => write!(f, "Invalid piece placement field in FEN"),
            ParseFenError::InvalidActiveColor => write!(f, "Invalid active color in FEN"),
            ParseFenError::InvalidCastlingRights => write!(f, "Invalid castling rights in FEN"),
            ParseFenError::InvalidEnPassant => write!(f, "Invalid en passant square in FEN"),
            ParseFenError::InvalidHalfmoveClock => write!(f, "Invalid halfmove clock in FEN"),
            ParseFenError::InvalidFullmoveNumber => write!(f, "Invalid fullmove number in FEN"),
        }
    }
}

impl std::error::Error for ParseFenError {}

pub struct ParsedFen {
    pub piece_at: [Option<Piece>; 64],
    pub side_to_move: Color,
    pub castling_rights: u8,
    pub ep_square: Option<u8>,
    pub halfmove_clock: u16,
    pub fullmove_number: u16,
}

pub const WHITE_OO: u8 = 1 << 0;
pub const WHITE_OOO: u8 = 1 << 1;
pub const BLACK_OO: u8 = 1 << 2;
pub const BLACK_OOO: u8 = 1 << 3;

pub fn parse_fen(fen: &str) -> Result<ParsedFen, ParseFenError> {
    let fields: Vec<&str> = fen.split_whitespace().collect();
    if fields.len() < 4 {
        return Err(ParseFenError::InvalidFormat);
    }

    let mut piece_at = [None; 64];
    let rows: Vec<&str> = fields[0].split('/').collect();
    if rows.len() != 8 {
        return Err(ParseFenError::InvalidPiecePlacement);
    }

    for (rank_idx, row) in rows.iter().enumerate() {
        let rank = 7 - (rank_idx as u8);
        let mut file = 0u8;

        for ch in row.chars() {
            if ch.is_ascii_digit() {
                let empty_count = ch.to_digit(10).unwrap() as u8;
                file += empty_count;
                if file > 8 {
                    return Err(ParseFenError::InvalidPiecePlacement);
                }
            } else if let Some(piece) = Piece::from_char(ch) {
                if file >= 8 {
                    return Err(ParseFenError::InvalidPiecePlacement);
                }
                let sq = (rank << 3) | file;
                piece_at[sq as usize] = Some(piece);
                file += 1;
            } else {
                return Err(ParseFenError::InvalidPiecePlacement);
            }
        }

        if file != 8 {
            return Err(ParseFenError::InvalidPiecePlacement);
        }
    }

    let side_to_move = match fields[1] {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err(ParseFenError::InvalidActiveColor),
    };

    let mut castling_rights = 0u8;
    if fields[2] != "-" {
        for ch in fields[2].chars() {
            match ch {
                'K' => castling_rights |= WHITE_OO,
                'Q' => castling_rights |= WHITE_OOO,
                'k' => castling_rights |= BLACK_OO,
                'q' => castling_rights |= BLACK_OOO,
                _ => return Err(ParseFenError::InvalidCastlingRights),
            }
        }
    }

    let ep_square = if fields[3] == "-" {
        None
    } else {
        match Square::parse(fields[3]) {
            Some(sq) => Some(sq.to_u8()),
            None => return Err(ParseFenError::InvalidEnPassant),
        }
    };

    // Positions the movegen cannot handle: exactly one king per side, no pawns on rank 1/8.
    let is = |sq: u8, c: char| piece_at[sq as usize] == Piece::from_char(c);
    if (0..64).filter(|&sq| is(sq, 'K')).count() != 1
        || (0..64).filter(|&sq| is(sq, 'k')).count() != 1
        || (0..8).chain(56..64).any(|sq| is(sq, 'P') || is(sq, 'p'))
    {
        return Err(ParseFenError::InvalidPiecePlacement);
    }

    // Drop castling rights whose king or rook is not on its home square.
    for (right, king, rook, k, r) in [
        (WHITE_OO, 4, 7, 'K', 'R'),
        (WHITE_OOO, 4, 0, 'K', 'R'),
        (BLACK_OO, 60, 63, 'k', 'r'),
        (BLACK_OOO, 60, 56, 'k', 'r'),
    ] {
        if !is(king, k) || !is(rook, r) {
            castling_rights &= !right;
        }
    }

    // EP target must sit behind an enemy pawn that just double-pushed: target and origin empty.
    if let Some(ep) = ep_square {
        let (target_rank, pawn_sq, origin_sq, pawn) = match side_to_move {
            Color::White => (5, ep.wrapping_sub(8), ep.wrapping_add(8), 'p'),
            Color::Black => (2, ep.wrapping_add(8), ep.wrapping_sub(8), 'P'),
        };
        if ep >> 3 != target_rank
            || !is(pawn_sq, pawn)
            || piece_at[ep as usize].is_some()
            || piece_at[origin_sq as usize].is_some()
        {
            return Err(ParseFenError::InvalidEnPassant);
        }
    }

    let halfmove_clock = if fields.len() > 4 {
        fields[4].parse::<u16>().map_err(|_| ParseFenError::InvalidHalfmoveClock)?
    } else {
        0
    };

    let fullmove_number = if fields.len() > 5 {
        fields[5].parse::<u16>().map_err(|_| ParseFenError::InvalidFullmoveNumber)?
    } else {
        1
    };

    Ok(ParsedFen {
        piece_at,
        side_to_move,
        castling_rights,
        ep_square,
        halfmove_clock,
        fullmove_number,
    })
}

pub fn format_fen(
    piece_at: &[Option<Piece>; 64],
    side_to_move: Color,
    castling_rights: u8,
    ep_square: Option<u8>,
    halfmove_clock: u16,
    fullmove_number: u16,
) -> String {
    let mut s = String::new();

    for rank in (0..8).rev() {
        let mut empty_count = 0;
        for file in 0..8 {
            let sq = (rank << 3) | file;
            if let Some(piece) = piece_at[sq as usize] {
                if empty_count > 0 {
                    s.push_str(&empty_count.to_string());
                    empty_count = 0;
                }
                s.push(piece.to_char());
            } else {
                empty_count += 1;
            }
        }
        if empty_count > 0 {
            s.push_str(&empty_count.to_string());
        }
        if rank > 0 {
            s.push('/');
        }
    }

    s.push(' ');
    s.push(if side_to_move == Color::White { 'w' } else { 'b' });

    s.push(' ');
    if castling_rights == 0 {
        s.push('-');
    } else {
        if castling_rights & WHITE_OO != 0 {
            s.push('K');
        }
        if castling_rights & WHITE_OOO != 0 {
            s.push('Q');
        }
        if castling_rights & BLACK_OO != 0 {
            s.push('k');
        }
        if castling_rights & BLACK_OOO != 0 {
            s.push('q');
        }
    }

    s.push(' ');
    if let Some(ep) = ep_square {
        s.push_str(&Square::from_u8(ep).to_str());
    } else {
        s.push('-');
    }

    s.push(' ');
    s.push_str(&halfmove_clock.to_string());

    s.push(' ');
    s.push_str(&fullmove_number.to_string());

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_not_to_move_in_check_rejected() {
        use crate::board::Board;
        // Black to move while White's king on f1 is attacked by the queen on h1.
        assert!(Board::from_fen("8/8/8/8/8/5k2/8/5K1q b - - 0 1").is_err());
        // Same position with White to move is legal (checkmate).
        assert!(Board::from_fen("8/8/8/8/8/5k2/8/5K1q w - - 0 1").is_ok());
    }

    #[test]
    fn test_fen_roundtrip_startpos() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let parsed = parse_fen(fen).unwrap();
        let formatted = format_fen(
            &parsed.piece_at,
            parsed.side_to_move,
            parsed.castling_rights,
            parsed.ep_square,
            parsed.halfmove_clock,
            parsed.fullmove_number,
        );
        assert_eq!(fen, formatted);
    }

    #[test]
    fn test_fen_roundtrip_kiwipete() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let parsed = parse_fen(fen).unwrap();
        let formatted = format_fen(
            &parsed.piece_at,
            parsed.side_to_move,
            parsed.castling_rights,
            parsed.ep_square,
            parsed.halfmove_clock,
            parsed.fullmove_number,
        );
        assert_eq!(fen, formatted);
    }

    #[test]
    fn test_fen_invalid_formats() {
        assert!(parse_fen("invalid").is_err());
        assert!(parse_fen("8/8/8/8/8/8/8/8 x - - 0 1").is_err());
        assert!(parse_fen("8/8/8/8/8/8/8/8 w INVALID - 0 1").is_err());
        assert!(parse_fen("8/8/8/8/8/8/8/8 w - e9 0 1").is_err());
    }

    #[test]
    fn test_fen_rejects_impossible_positions() {
        let err = |fen: &str| parse_fen(fen).err();
        assert_eq!(err("8/8/8/8/8/8/8/8 w - - 0 1"), Some(ParseFenError::InvalidPiecePlacement));
        assert_eq!(err("4k3/8/8/8/8/8/8/4KK2 w - - 0 1"), Some(ParseFenError::InvalidPiecePlacement));
        assert_eq!(err("P3k3/8/8/8/8/8/8/4K3 w - - 0 1"), Some(ParseFenError::InvalidPiecePlacement));
        let start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR";
        assert_eq!(err(&format!("{start} w KQkq e3 0 1")), Some(ParseFenError::InvalidEnPassant));
        assert_eq!(err(&format!("{start} w KQkq e6 0 1")), Some(ParseFenError::InvalidEnPassant));
        // Legal EP after 1.e4 is still accepted.
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        assert_eq!(parse_fen(after_e4).unwrap().ep_square, Some(20));
    }

    #[test]
    fn test_fen_drops_castling_rights_without_pieces() {
        assert_eq!(parse_fen("4k3/8/8/8/8/8/8/4K3 w KQkq - 0 1").unwrap().castling_rights, 0);
        let only_h_rooks = parse_fen("4k2r/8/8/8/8/8/8/4K2R w KQkq - 0 1").unwrap();
        assert_eq!(only_h_rooks.castling_rights, WHITE_OO | BLACK_OO);
    }
}
