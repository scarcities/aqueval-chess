use std::env;
use std::process;
use aqueval::board::Board;
use aqueval::perft::{format_perft_divide, perft_divide, run_perft_timed};

fn print_usage() {
    println!("aqueval chess v{}", env!("CARGO_PKG_VERSION"));
    println!("Usage:");
    println!("  aqueval                                 # Starts interactive UCI loop");
    println!("  aqueval bench [depth]                   # Runs deterministic benchmark suite");
    println!("  aqueval perft <depth> [fen] [--divide]  # Runs perft leaf node counting");
    println!("  aqueval divide <depth> [fen]            # Runs perft divide breakdown");
    println!("  aqueval datagen [--out DIR] [--threads N] [--games N] [--nodes N] [--seed N]");
    println!("                [--random-plies N] [--hash MB] [--evalfile NET]");
    println!("                                        # Self-play training data (docs/NNUE.md)");
}

/// `aqueval datagen ...`: parses `--name value` pairs into a config and runs data generation.
fn run_datagen(args: &[String]) {
    let mut cfg = aqueval::datagen::DatagenConfig::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let Some(value) = it.next() else {
            eprintln!("Error: {flag} needs a value");
            process::exit(1);
        };
        let num = || -> u64 {
            value.parse().unwrap_or_else(|_| {
                eprintln!("Error: invalid value '{value}' for {flag}");
                process::exit(1);
            })
        };
        match flag.as_str() {
            "--out" => cfg.out_dir = value.into(),
            "--threads" => cfg.threads = num() as usize,
            "--games" => cfg.games = num(),
            "--nodes" => cfg.soft_nodes = num(),
            "--seed" => cfg.seed = num(),
            "--random-plies" => cfg.random_plies = num() as usize,
            "--hash" => cfg.hash_mb = num() as usize,
            "--evalfile" => {
                if !cfg!(feature = "nnue") {
                    eprintln!("Error: --evalfile needs a build with --features nnue");
                    process::exit(1);
                }
                if let Err(e) = aqueval::nnue::load_eval_file(value) {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            }
            _ => {
                eprintln!("Error: unknown datagen option {flag}");
                print_usage();
                process::exit(1);
            }
        }
    }
    let eval = if aqueval::nnue::active().is_some() && cfg!(feature = "nnue") { "NNUE" } else { "HCE" };
    println!("datagen eval: {eval}");
    if let Err(e) = aqueval::datagen::run(&cfg) {
        eprintln!("Error: datagen failed: {e}");
        process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Default with no arguments: enter UCI protocol loop
    if args.len() < 2 {
        aqueval::uci::run_uci_loop_stdio();
        return;
    }

    match args[1].as_str() {
        "uci" => {
            aqueval::uci::run_uci_loop_stdio();
        }
        "bench" => {
            let depth: Option<usize> = if args.len() >= 3 {
                match args[2].parse() {
                    Ok(d) => Some(d),
                    Err(e) => {
                        eprintln!("Error: invalid bench depth '{}': {}", args[2], e);
                        process::exit(1);
                    }
                }
            } else {
                None
            };
            aqueval::bench::run_bench(depth);
        }
        "perft" => {
            if args.len() < 3 {
                eprintln!("Error: 'perft' requires a depth argument.");
                print_usage();
                process::exit(1);
            }

            let depth: usize = match args[2].parse() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: invalid depth '{}': {}", args[2], e);
                    process::exit(1);
                }
            };

            let mut board = if args.len() >= 4 && !args[3].starts_with("--") {
                Board::from_fen(&args[3]).unwrap_or_else(|e| {
                    eprintln!("Error: invalid FEN '{}': {:?}", args[3], e);
                    process::exit(1);
                })
            } else {
                Board::startpos()
            };

            let show_divide = args.iter().any(|arg| arg == "--divide" || arg == "divide");

            if show_divide {
                let divide_results = perft_divide(&mut board, depth);
                print!("{}", format_perft_divide(&divide_results));
                return;
            }

            println!("========================================");
            println!("aqueval perft — depth {}", depth);
            println!("FEN: {}", board.to_fen());
            println!("========================================");

            // 1. Bulk counting
            println!("Running Bulk Counting...");
            let bulk_stats = run_perft_timed(&mut board, depth, true);
            println!(
                "Bulk:     {:>12} nodes | {:>8.3?} | {:>12} NPS",
                bulk_stats.nodes, bulk_stats.elapsed, bulk_stats.nps
            );

            // 2. Non-bulk counting
            println!("Running Non-Bulk Counting...");
            let non_bulk_stats = run_perft_timed(&mut board, depth, false);
            println!(
                "Non-Bulk: {:>12} nodes | {:>8.3?} | {:>12} NPS",
                non_bulk_stats.nodes, non_bulk_stats.elapsed, non_bulk_stats.nps
            );

            println!("========================================");
            if bulk_stats.nodes == non_bulk_stats.nodes {
                println!("Node counts match: SUCCESS ({} nodes)", bulk_stats.nodes);
            } else {
                eprintln!(
                    "MISMATCH: Bulk {} != Non-bulk {}",
                    bulk_stats.nodes, non_bulk_stats.nodes
                );
                process::exit(1);
            }
        }
        "divide" => {
            if args.len() < 3 {
                eprintln!("Error: 'divide' requires a depth argument.");
                print_usage();
                process::exit(1);
            }
            let depth: usize = match args[2].parse() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: invalid depth '{}': {}", args[2], e);
                    process::exit(1);
                }
            };
            let mut board = if args.len() >= 4 {
                Board::from_fen(&args[3]).unwrap_or_else(|e| {
                    eprintln!("Error: invalid FEN '{}': {:?}", args[3], e);
                    process::exit(1);
                })
            } else {
                Board::startpos()
            };

            let divide_results = perft_divide(&mut board, depth);
            print!("{}", format_perft_divide(&divide_results));
        }
        "datagen" => run_datagen(&args[2..]),
        other => {
            eprintln!("Unknown command: {}", other);
            print_usage();
            process::exit(1);
        }
    }
}
