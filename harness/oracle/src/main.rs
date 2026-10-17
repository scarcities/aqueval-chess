//! Independent perft oracle for the harness suite files, built on shakmaty.
//!
//! Usage: cargo run --release --manifest-path harness/oracle/Cargo.toml -- \
//!            harness/suites/standard.epd [max_nodes]
//!
//! Recomputes every `;D<depth> <nodes>` entry (in parallel over root moves) and
//! exits with status 1 if any count differs. Use it before adding positions.

use shakmaty::{fen::Fen, perft, CastlingMode, Chess, Position};
use std::io::BufRead;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: perft-oracle <suite.epd> [max_nodes]");
        return ExitCode::from(2);
    };
    let max_nodes: u64 = match args.get(2).map(|s| s.parse::<f64>()) {
        None => u64::MAX,
        Some(Ok(v)) if v > 0.0 => v as u64,
        Some(Ok(_)) => u64::MAX,
        Some(Err(e)) => {
            eprintln!("bad max_nodes: {e}");
            return ExitCode::from(2);
        }
    };
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot open {path}: {e}");
            return ExitCode::from(2);
        }
    };

    let (mut name, mut bad, mut checked, mut skipped) = (String::from("?"), 0, 0, 0);
    for (lineno, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line.expect("read error");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            name = comment.split_whitespace().next().unwrap_or("?").to_string();
            continue;
        }
        let fields: Vec<&str> = line.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
        let pos: Chess = match fields[0]
            .parse::<Fen>()
            .map_err(|e| e.to_string())
            .and_then(|f| f.into_position(CastlingMode::Standard).map_err(|e| e.to_string()))
        {
            Ok(p) => p,
            Err(e) => {
                println!("{name} (line {}): bad FEN: {e}", lineno + 1);
                bad += 1;
                continue;
            }
        };
        for field in &fields[1..] {
            let mut it = field.split_whitespace();
            let (Some(d), Some(n)) = (it.next(), it.next()) else {
                println!("{name} (line {}): cannot parse {field:?}", lineno + 1);
                bad += 1;
                continue;
            };
            let (Ok(depth), Ok(expected)) = (d[1..].parse::<u32>(), n.parse::<u64>()) else {
                println!("{name} (line {}): cannot parse {field:?}", lineno + 1);
                bad += 1;
                continue;
            };
            if expected > max_nodes {
                skipped += 1;
                continue;
            }
            let t = std::time::Instant::now();
            let moves = pos.legal_moves();
            let got: u64 = if depth <= 1 {
                if depth == 0 { 1 } else { moves.len() as u64 }
            } else {
                std::thread::scope(|s| {
                    let handles: Vec<_> = moves
                        .iter()
                        .map(|m| {
                            let mut child = pos.clone();
                            child.play_unchecked(m);
                            s.spawn(move || perft(&child, depth - 1))
                        })
                        .collect();
                    handles.into_iter().map(|h| h.join().unwrap()).sum()
                })
            };
            checked += 1;
            let ok = got == expected;
            if !ok {
                bad += 1;
            }
            println!(
                "{name:<28} D{depth:<2} {} {expected:>12}{} ({:.2}s)",
                if ok { "ok " } else { "BAD" },
                if ok { String::new() } else { format!(" shakmaty says {got}") },
                t.elapsed().as_secs_f64()
            );
        }
    }
    println!("\n{checked} checked, {bad} wrong, {skipped} skipped");
    if bad > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS }
}
