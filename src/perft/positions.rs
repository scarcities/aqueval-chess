/// Ground truth perft positions from the Chess Programming Wiki and standard engine test suites.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerftPosition {
    pub id: usize,
    pub name: &'static str,
    pub fen: &'static str,
    pub expected_nodes: &'static [u64],
}

impl PerftPosition {
    /// Returns the ground truth node count for the given depth (1-indexed).
    /// Depth 0 always returns Some(1).
    pub fn expected_nodes(&self, depth: usize) -> Option<u64> {
        if depth == 0 {
            Some(1)
        } else if depth <= self.expected_nodes.len() {
            Some(self.expected_nodes[depth - 1])
        } else {
            None
        }
    }

    /// Maximum depth available in the ground truth table.
    pub fn max_depth(&self) -> usize {
        self.expected_nodes.len()
    }
}

/// Position 1: Initial starting position
pub const POS_STARTPOS: PerftPosition = PerftPosition {
    id: 1,
    name: "Starting Position",
    fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    expected_nodes: &[
        20,
        400,
        8_902,
        197_281,
        4_865_609,
        119_060_324,
    ],
};

/// Position 2: "Kiwipete" by Peter McKenzie
pub const POS_KIWIPETE: PerftPosition = PerftPosition {
    id: 2,
    name: "Kiwipete",
    fen: "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    expected_nodes: &[
        48,
        2_039,
        97_862,
        4_085_603,
        193_690_690,
    ],
};

/// Position 3: Silver Endgame / Pawn Endgame
pub const POS_3: PerftPosition = PerftPosition {
    id: 3,
    name: "Position 3",
    fen: "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    expected_nodes: &[
        14,
        191,
        2_812,
        43_238,
        674_624,
        11_030_083,
    ],
};

/// Position 4: Tactical Promotion & Check Web
pub const POS_4: PerftPosition = PerftPosition {
    id: 4,
    name: "Position 4",
    fen: "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    expected_nodes: &[
        6,
        264,
        9_467,
        422_333,
        15_833_292,
        706_045_033,
    ],
};

/// Position 5: TalkChess Bug Trap
pub const POS_5: PerftPosition = PerftPosition {
    id: 5,
    name: "Position 5",
    fen: "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    expected_nodes: &[
        44,
        1_486,
        62_379,
        2_103_487,
        89_941_194,
    ],
};

/// Position 6: Steven Edwards Alternative Standard
pub const POS_6: PerftPosition = PerftPosition {
    id: 6,
    name: "Position 6",
    fen: "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    expected_nodes: &[
        46,
        2_079,
        89_890,
        3_894_594,
        164_075_551,
    ],
};

/// Slice of all 6 standard test positions.
pub const ALL_POSITIONS: [PerftPosition; 6] = [
    POS_STARTPOS,
    POS_KIWIPETE,
    POS_3,
    POS_4,
    POS_5,
    POS_6,
];
