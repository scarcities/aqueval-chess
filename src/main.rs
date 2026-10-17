use std::env;
use std::process;
use quant::board::Board;
use quant::perft::{format_perft_divide, perft_divide, run_perft_timed};

fn print_usage() {
    println!("Quant Chess Engine v0.1.0");
    println!("Usage:");
    println!("  quant                                 # Starts interactive UCI loop");
    println!("  quant bench [depth]                   # Runs deterministic benchmark suite");
    println!("  quant perft <depth> [fen] [--divide]  # Runs perft leaf node counting");
    println!("  quant divide <depth> [fen]            # Runs perft divide breakdown");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Default with no arguments: enter UCI protocol loop
    if args.len() < 2 {
        quant::uci::run_uci_loop_stdio();
        return;
    }

    match args[1].as_str() {
        "uci" => {
            quant::uci::run_uci_loop_stdio();
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
            quant::bench::run_bench(depth);
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
            println!("Quant Perft — Depth {}", depth);
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
        other => {
            eprintln!("Unknown command: {}", other);
            print_usage();
            process::exit(1);
        }
    }
}
