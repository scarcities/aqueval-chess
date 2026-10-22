//! Self-play data generation for NNUE training.
//!
//! Each thread plays games from a few random opening moves, searching every move to a soft
//! node limit, and writes one line per kept position:
//!
//! ```text
//! <FEN> | <score> | <result>
//! ```
//!
//! with the score in centipawns and the result (1.0 / 0.5 / 0.0) both from White's view,
//! the text format bullet's converter reads (and `scripts/nnue/train.py`).
//! Kept positions: not in check, best move quiet (no capture, no promotion), no mate score.
//! The search uses whatever eval is active, so an `nnue` build with a net generates data
//! with that net (the reinforcement loop in docs/NNUE.md).

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::board::zobrist::SplitMix64;
use crate::board::Board;
use crate::movegen::types::MoveList;
use crate::search::tt::TranspositionTable;
use crate::search::{search, SearchLimits, MATE_THRESHOLD};
use crate::types::{Color, PieceType};

#[derive(Clone, Debug)]
pub struct DatagenConfig {
    pub out_dir: PathBuf,
    pub threads: usize,
    /// Total games over all threads.
    pub games: u64,
    /// Soft node limit per move; the hard limit is ten times this.
    pub soft_nodes: u64,
    pub seed: u64,
    /// Random plies at the start of each game (one more on odd seeds of the game RNG).
    pub random_plies: usize,
    pub hash_mb: usize,
}

impl Default for DatagenConfig {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("data"),
            threads: 1,
            games: 1000,
            soft_nodes: 5000,
            seed: 1,
            random_plies: 8,
            hash_mb: 16,
        }
    }
}

/// Openings whose first search is beyond this are discarded as unbalanced.
const MAX_OPENING_SCORE: i32 = 1000;
/// Adjudicate a win when the score stays beyond ±WIN_SCORE for the same side this many plies.
const WIN_SCORE: i32 = 2000;
const WIN_PLIES: u32 = 6;
/// Adjudicate a draw after DRAW_FROM_PLY when |score| ≤ DRAW_SCORE for DRAW_PLIES plies.
const DRAW_FROM_PLY: u32 = 80;
const DRAW_SCORE: i32 = 10;
const DRAW_PLIES: u32 = 12;
const MAX_GAME_PLIES: u32 = 400;

struct Game {
    /// (FEN, score from White's view)
    positions: Vec<(String, i32)>,
    /// 1.0 White won, 0.5 draw, 0.0 Black won.
    result: f32,
}

/// No side can mate: kings only, or kings plus a single minor piece.
fn insufficient_material(board: &Board) -> bool {
    let heavy = board.pieces[PieceType::Pawn.index()]
        | board.pieces[PieceType::Rook.index()]
        | board.pieces[PieceType::Queen.index()];
    let minors = board.pieces[PieceType::Knight.index()] | board.pieces[PieceType::Bishop.index()];
    heavy.is_empty() && minors.count() <= 1
}

/// Plays random opening moves; None if the game ended during them.
fn random_opening(rng: &mut SplitMix64, plies: usize) -> Option<Board> {
    let mut board = Board::startpos();
    for _ in 0..plies {
        let mut moves = MoveList::new();
        board.generate_legal_moves(&mut moves);
        if moves.is_empty() {
            return None;
        }
        let m = moves.as_slice()[(rng.next_u64() % moves.len() as u64) as usize];
        board.make_move(m);
    }
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);
    (!moves.is_empty()).then_some(board)
}

fn play_game(rng: &mut SplitMix64, tt: &mut TranspositionTable, cfg: &DatagenConfig) -> Option<Game> {
    let plies = cfg.random_plies + (rng.next_u64() & 1) as usize;
    let mut board = random_opening(rng, plies)?;
    tt.clear();
    let limits = SearchLimits {
        soft_nodes: Some(cfg.soft_nodes),
        nodes: Some(cfg.soft_nodes * 10),
        ..Default::default()
    };
    let stop = AtomicBool::new(false);
    let mut positions = Vec::new();
    let (mut win_streak, mut win_side, mut draw_streak) = (0, Color::White, 0);

    for ply in 0.. {
        let stm = board.side_to_move;
        let mut moves = MoveList::new();
        board.generate_legal_moves(&mut moves);
        if moves.is_empty() {
            let result = if !board.is_in_check() {
                0.5
            } else if stm == Color::White {
                0.0
            } else {
                1.0
            };
            return Some(Game { positions, result });
        }
        if board.halfmove_clock >= 100 || board.is_repetition() || insufficient_material(&board) || ply >= MAX_GAME_PLIES {
            return Some(Game { positions, result: 0.5 });
        }

        stop.store(false, Ordering::Relaxed);
        let res = search(&mut board, &limits, &stop, None, tt);
        let score = res.score;
        if ply == 0 && score.abs() > MAX_OPENING_SCORE {
            return None;
        }

        // Adjudication, with the score from the side to move's view.
        let leader = if score > 0 { stm } else { !stm };
        if score.abs() >= WIN_SCORE {
            win_streak = if leader == win_side { win_streak + 1 } else { 1 };
            win_side = leader;
        } else {
            win_streak = 0;
        }
        draw_streak = if ply >= DRAW_FROM_PLY && score.abs() <= DRAW_SCORE { draw_streak + 1 } else { 0 };
        if win_streak >= WIN_PLIES {
            let result = if win_side == Color::White { 1.0 } else { 0.0 };
            return Some(Game { positions, result });
        }
        if draw_streak >= DRAW_PLIES {
            return Some(Game { positions, result: 0.5 });
        }

        let best = res.best_move;
        if !board.is_in_check() && !best.is_capture() && !best.is_promotion() && score.abs() < MATE_THRESHOLD {
            let white_score = if stm == Color::White { score } else { -score };
            positions.push((board.to_fen(), white_score));
        }
        board.make_move(best);
    }
    unreachable!()
}

fn worker(id: usize, games: u64, cfg: &DatagenConfig, done_games: &AtomicU64, done_positions: &AtomicU64) -> std::io::Result<()> {
    let path = cfg.out_dir.join(format!("data_s{}_t{}.txt", cfg.seed, id));
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    let mut out = BufWriter::new(file);
    let mut rng = SplitMix64::new(cfg.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (id as u64 + 1));
    let mut tt = TranspositionTable::new(cfg.hash_mb);
    let mut played = 0;
    while played < games {
        let Some(game) = play_game(&mut rng, &mut tt, cfg) else {
            continue;
        };
        for (fen, score) in &game.positions {
            writeln!(out, "{fen} | {score} | {:.1}", game.result)?;
        }
        played += 1;
        done_games.fetch_add(1, Ordering::Relaxed);
        done_positions.fetch_add(game.positions.len() as u64, Ordering::Relaxed);
    }
    out.flush()
}

/// Runs data generation with `cfg.threads` threads, printing progress every 10 seconds.
pub fn run(cfg: &DatagenConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(&cfg.out_dir)?;
    let threads = cfg.threads.max(1);
    let done_games = Arc::new(AtomicU64::new(0));
    let done_positions = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    println!(
        "datagen: {} games, {} threads, {} soft nodes, seed {}, output {}",
        cfg.games,
        threads,
        cfg.soft_nodes,
        cfg.seed,
        cfg.out_dir.display()
    );

    let handles: Vec<_> = (0..threads)
        .map(|id| {
            let games = cfg.games / threads as u64 + u64::from((id as u64) < cfg.games % threads as u64);
            let (cfg, g, p) = (cfg.clone(), done_games.clone(), done_positions.clone());
            std::thread::spawn(move || worker(id, games, &cfg, &g, &p))
        })
        .collect();

    let report = |final_line: bool| {
        let secs = start.elapsed().as_secs_f64().max(1e-3);
        let (g, p) = (done_games.load(Ordering::Relaxed), done_positions.load(Ordering::Relaxed));
        println!(
            "{}games {g}/{} | positions {p} | {:.0} pos/s | {:.0} s",
            if final_line { "done: " } else { "" },
            cfg.games,
            p as f64 / secs,
            secs
        );
    };
    let mut last_report = Instant::now();
    while !handles.iter().all(|h| h.is_finished()) {
        std::thread::sleep(Duration::from_millis(200));
        if last_report.elapsed() >= Duration::from_secs(10) {
            report(false);
            last_report = Instant::now();
        }
    }
    for h in handles {
        h.join().expect("datagen thread panicked")?;
    }
    report(true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insufficient_material_cases() {
        assert!(insufficient_material(&Board::from_fen("8/8/4k3/8/8/3NK3/8/8 w - - 0 1").unwrap()));
        assert!(insufficient_material(&Board::from_fen("8/8/4k3/8/8/4K3/8/8 w - - 0 1").unwrap()));
        assert!(!insufficient_material(&Board::from_fen("8/8/4k3/8/8/2BNK3/8/8 w - - 0 1").unwrap()));
        assert!(!insufficient_material(&Board::from_fen("8/8/4k3/8/8/4K3/4P3/8 w - - 0 1").unwrap()));
    }

    #[test]
    fn short_run_writes_valid_lines() {
        let dir = std::env::temp_dir().join(format!("quant-datagen-test-{}", std::process::id()));
        let cfg = DatagenConfig { out_dir: dir.clone(), threads: 2, games: 2, soft_nodes: 300, seed: 3, ..Default::default() };
        run(&cfg).unwrap();
        let mut lines = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            for line in std::fs::read_to_string(entry.unwrap().path()).unwrap().lines() {
                let parts: Vec<&str> = line.split(" | ").collect();
                assert_eq!(parts.len(), 3, "{line}");
                Board::from_fen(parts[0]).unwrap();
                assert!(parts[1].parse::<i32>().unwrap().abs() < MATE_THRESHOLD);
                assert!(["1.0", "0.5", "0.0"].contains(&parts[2]), "{line}");
                lines += 1;
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(lines > 0);
    }
}
