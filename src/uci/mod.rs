pub mod options;

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::board::Board;
use crate::movegen::types::{Move, MoveList};
use crate::perft::{format_perft_divide, perft_divide};
use crate::search::time::TimeManager;
use crate::search::tt::TranspositionTable;
use crate::search::{search, PonderState, SearchInfo, SearchLimits, SearchResult};
use crate::uci::options::UciOptions;

struct ActiveSearch {
    stop_flag: Arc<AtomicBool>,
    is_pondering: Arc<AtomicBool>,
    _is_infinite: bool,
    best_move_emitted: Arc<AtomicBool>,
    last_result: Arc<Mutex<Option<SearchResult>>>,
    /// Set for `go ponder`; `ponderhit` starts its clock.
    ponder: Option<Arc<PonderState>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

/// `bestmove <move>`, plus `ponder <reply>` when the TT knows the expected reply.
fn bestmove_line(result: Option<SearchResult>) -> String {
    match result {
        Some(r) if r.ponder_move != Move::NULL => format!("bestmove {} ponder {}", r.best_move.to_uci(), r.ponder_move.to_uci()),
        Some(r) => format!("bestmove {}", r.best_move.to_uci()),
        None => format!("bestmove {}", Move::NULL.to_uci()),
    }
}

/// Tokenizes a line into whitespace-separated tokens, trimming leading/trailing whitespace.
fn tokenize(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

/// Finds a legal move matching the given UCI move string on the current board.
fn find_legal_move(board: &Board, move_str: &str) -> Option<Move> {
    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);
    moves.as_slice().iter().copied().find(|m| m.to_uci() == move_str)
}

/// Applies a sequence of moves to the board. Invalid moves are ignored/truncated.
fn apply_moves(board: &mut Board, move_strs: &[&str]) {
    for &m_str in move_strs {
        if let Some(m) = find_legal_move(board, m_str) {
            board.make_move(m);
        } else {
            break;
        }
    }
}

/// Parses the `position` command arguments:
/// `position startpos [moves ...]`
/// `position fen <fen> [moves ...]`
fn handle_position(board: &mut Board, tokens: &[&str]) {
    if tokens.is_empty() {
        return;
    }

    let moves_idx = tokens.iter().position(|&t| t == "moves");

    if tokens[0] == "startpos" {
        *board = Board::startpos();
        if let Some(idx) = moves_idx {
            apply_moves(board, &tokens[idx + 1..]);
        }
    } else if tokens[0] == "fen" {
        let fen_end = moves_idx.unwrap_or(tokens.len());
        let fen_tokens = &tokens[1..fen_end];
        let fen_str = fen_tokens.join(" ");
        if let Ok(new_board) = Board::from_fen(&fen_str) {
            *board = new_board;
            if let Some(idx) = moves_idx {
                apply_moves(board, &tokens[idx + 1..]);
            }
        }
    }
}

/// Parses the arguments of a `go` command into `SearchLimits` and optional `perft` depth.
struct ParsedGo {
    limits: SearchLimits,
    perft_depth: Option<usize>,
}

fn parse_go(board: &Board, tokens: &[&str], move_overhead_ms: u64) -> ParsedGo {
    let mut depth = None;
    let mut nodes = None;
    let mut wtime = None;
    let mut btime = None;
    let mut winc = None;
    let mut binc = None;
    let mut movestogo = None;
    let mut movetime = None;
    let mut infinite = false;
    let mut ponder = false;
    let mut perft_depth = None;
    let mut searchmoves = None;

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "depth" => {
                if i + 1 < tokens.len() {
                    depth = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "nodes" => {
                if i + 1 < tokens.len() {
                    nodes = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "wtime" => {
                if i + 1 < tokens.len() {
                    wtime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "btime" => {
                if i + 1 < tokens.len() {
                    btime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "winc" => {
                if i + 1 < tokens.len() {
                    winc = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "binc" => {
                if i + 1 < tokens.len() {
                    binc = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "movestogo" => {
                if i + 1 < tokens.len() {
                    movestogo = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < tokens.len() {
                    movetime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "infinite" => {
                infinite = true;
            }
            "ponder" => {
                ponder = true;
            }
            "perft" => {
                if i + 1 < tokens.len() {
                    perft_depth = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "searchmoves" => {
                let mut moves = Vec::new();
                i += 1;
                while i < tokens.len() {
                    match tokens[i] {
                        "depth" | "nodes" | "wtime" | "btime" | "winc" | "binc"
                        | "movestogo" | "movetime" | "infinite" | "ponder" | "perft" => {
                            i -= 1;
                            break;
                        }
                        m_str => {
                            if let Some(m) = find_legal_move(board, m_str) {
                                moves.push(m);
                            }
                        }
                    }
                    i += 1;
                }
                searchmoves = Some(moves);
            }
            _ => {}
        }
        i += 1;
    }

    // Reserve the GUI/OS latency up front so the limits only cover time we can really spend.
    let minus_overhead = |t: Option<u64>| t.map(|t: u64| t.saturating_sub(move_overhead_ms));
    let time_limits = TimeManager::calculate_limits(
        board.side_to_move,
        minus_overhead(wtime),
        minus_overhead(btime),
        winc,
        binc,
        movestogo,
        minus_overhead(movetime),
    );

    ParsedGo {
        limits: SearchLimits {
            depth,
            nodes,
            soft_nodes: None,
            time_limits,
            infinite,
            ponder: ponder.then(|| Arc::new(PonderState::default())),
            searchmoves,
        },
        perft_depth,
    }
}

/// Parses `setoption name <id> [value <val>]`. Returns the option's message, if any.
fn handle_setoption(options: &mut UciOptions, tokens: &[&str]) -> Option<String> {
    // Format: name <id...> [value <val...>]
    let mut name = String::new();
    let mut value: Option<String> = None;
    let mut in_name = false;
    let mut in_value = false;

    for &tok in tokens {
        if tok.eq_ignore_ascii_case("name") {
            in_name = true;
            in_value = false;
        } else if tok.eq_ignore_ascii_case("value") {
            in_name = false;
            in_value = true;
        } else if in_name {
            if !name.is_empty() {
                name.push(' ');
            }
            name.push_str(tok);
        } else if in_value {
            if let Some(ref mut v) = value {
                v.push(' ');
                v.push_str(tok);
            } else {
                value = Some(tok.to_string());
            }
        }
    }

    if name.is_empty() {
        return None;
    }
    options.set_option(&name, value.as_deref())
}

/// `eval`: static evaluation of the current position (side to move's view), for debugging.
fn eval_report(board: &Board) -> String {
    #[allow(unused_mut)]
    let mut out = format!("info string HCE eval: {} cp", crate::search::evaluate::evaluate(board));
    #[cfg(feature = "nnue")]
    match crate::nnue::active() {
        Some(net) => out += &format!("\ninfo string NNUE eval: {} cp", net.evaluate_from_scratch(board)),
        None => out += "\ninfo string NNUE eval: no net loaded (or UseNNUE is off)",
    }
    out
}

/// Runs the UCI protocol loop with the given reader and writer.
/// Non-blocking command processing: handles `stop`, `isready`, `ponderhit` while search is active.
pub fn run_uci<R: BufRead, W: Write + Send + 'static>(mut reader: R, writer: Arc<Mutex<W>>) {
    let mut board = Board::startpos();
    let mut options = UciOptions::new();
    // Shared with the search thread, which locks it for the whole search.
    let tt = Arc::new(Mutex::new(TranspositionTable::new(options.hash_mb)));
    let mut active_search: Option<ActiveSearch> = None;

    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        match reader.read_line(&mut line_buf) {
            Ok(0) => break, // EOF reached
            Ok(_) => {}
            Err(_) => break,
        }

        // Clean up completed non-infinite/non-ponder searches if finished
        if let Some(ref active) = active_search {
            if active.best_move_emitted.load(Ordering::SeqCst)
                && active.thread_handle.as_ref().is_none_or(|h| h.is_finished())
            {
                active_search = None;
            }
        }

        let trimmed = line_buf.trim();
        if trimmed.is_empty() {
            continue;
        }

        let all_tokens = tokenize(trimmed);
        if all_tokens.is_empty() {
            continue;
        }

        // Find the first known UCI command keyword (handles arbitrary leading unknown tokens)
        let cmd_pos = all_tokens.iter().position(|&tok| {
            matches!(
                tok,
                "uci"
                    | "debug"
                    | "isready"
                    | "setoption"
                    | "register"
                    | "ucinewgame"
                    | "position"
                    | "go"
                    | "stop"
                    | "ponderhit"
                    | "quit"
                    | "bench"
                    | "eval"
            )
        });

        let cmd_idx = match cmd_pos {
            Some(idx) => idx,
            None => continue, // Unknown command line, ignore per UCI spec
        };

        let cmd = all_tokens[cmd_idx];
        let args = &all_tokens[cmd_idx + 1..];

        match cmd {
            "uci" => {
                let mut w = writer.lock().unwrap();
                let _ = writeln!(w, "id name aqueval {}", env!("CARGO_PKG_VERSION"));
                let _ = writeln!(w, "id author Deniz");
                let _ = options.print_options(&mut *w);
                let _ = writeln!(w, "uciok");
                let _ = w.flush();
            }
            "debug" => {
                // Toggles debug mode (no-op in phase 1)
            }
            "isready" => {
                // Immediately answer readyok, even during active calculation
                let mut w = writer.lock().unwrap();
                let _ = writeln!(w, "readyok");
                let _ = w.flush();
            }
            "setoption" => {
                let old_hash = options.hash_mb;
                let message = handle_setoption(&mut options, args);
                if options.hash_mb != old_hash {
                    *tt.lock().unwrap() = TranspositionTable::new(options.hash_mb);
                }
                if let Some(msg) = message {
                    let mut w = writer.lock().unwrap();
                    let _ = writeln!(w, "info string {msg}");
                    let _ = w.flush();
                }
            }
            "register" => {
                // Unsupported / free engine
            }
            "ucinewgame" => {
                if let Some(mut active) = active_search.take() {
                    active.stop_flag.store(true, Ordering::SeqCst);
                    if let Some(h) = active.thread_handle.take() {
                        let _ = h.join();
                    }
                }
                board = Board::startpos();
                tt.lock().unwrap().clear();
            }
            "position" => {
                if let Some(mut active) = active_search.take() {
                    active.stop_flag.store(true, Ordering::SeqCst);
                    if let Some(h) = active.thread_handle.take() {
                        let _ = h.join();
                    }
                }
                handle_position(&mut board, args);
            }
            "go" => {
                if let Some(mut active) = active_search.take() {
                    active.stop_flag.store(true, Ordering::SeqCst);
                    if let Some(h) = active.thread_handle.take() {
                        let _ = h.join();
                    }
                }

                let parsed_go = parse_go(&board, args, options.move_overhead_ms);

                if let Some(perft_d) = parsed_go.perft_depth {
                    let mut b_clone = board.clone();
                    let divide_res = perft_divide(&mut b_clone, perft_d);
                    let formatted = format_perft_divide(&divide_res);
                    let mut w = writer.lock().unwrap();
                    let _ = write!(w, "{}", formatted);
                    let _ = w.flush();
                } else {
                    let stop_flag = Arc::new(AtomicBool::new(false));
                    let is_pondering = Arc::new(AtomicBool::new(parsed_go.limits.ponder.is_some()));
                    let ponder = parsed_go.limits.ponder.clone();
                    let is_infinite = parsed_go.limits.infinite;
                    let best_move_emitted = Arc::new(AtomicBool::new(false));
                    let last_result = Arc::new(Mutex::new(None));

                    let thread_stop = stop_flag.clone();
                    let thread_pondering = is_pondering.clone();
                    let thread_emitted = best_move_emitted.clone();
                    let thread_result = last_result.clone();
                    let thread_writer = writer.clone();
                    let mut thread_board = board.clone();
                    let thread_limits = parsed_go.limits;
                    let thread_tt = tt.clone();

                    let handle = std::thread::spawn(move || {
                        let info_cb = |info: &SearchInfo| {
                            let mut w = thread_writer.lock().unwrap();
                            let _ = writeln!(
                                w,
                                "info depth {} score {} nodes {} nps {} time {} pv {}",
                                info.depth,
                                info.score_str,
                                info.nodes,
                                info.nps,
                                info.time_ms,
                                info.pv_move.to_uci()
                            );
                            let _ = w.flush();
                        };

                        let result = search(
                            &mut thread_board,
                            &thread_limits,
                            &thread_stop,
                            Some(&info_cb),
                            &mut thread_tt.lock().unwrap(),
                        );

                        *thread_result.lock().unwrap() = Some(result);

                        // On normal searches (not infinite and not ponder), emit bestmove immediately
                        if !is_infinite
                            && !thread_pondering.load(Ordering::SeqCst)
                            && !thread_emitted.swap(true, Ordering::SeqCst)
                        {
                            let mut w = thread_writer.lock().unwrap();
                            let _ = writeln!(w, "{}", bestmove_line(Some(result)));
                            let _ = w.flush();
                        }
                    });

                    active_search = Some(ActiveSearch {
                        stop_flag,
                        is_pondering,
                        _is_infinite: is_infinite,
                        best_move_emitted,
                        last_result,
                        ponder,
                        thread_handle: Some(handle),
                    });
                }
            }
            // The opponent played the expected move: keep searching, now on the clock from `go ponder`.
            "ponderhit" => {
                if let Some(active) = &active_search {
                    if let Some(p) = &active.ponder {
                        active.is_pondering.store(false, Ordering::SeqCst);
                        p.hit();
                    }
                }
            }
            "stop" => {
                if let Some(mut active) = active_search.take() {
                    active.is_pondering.store(false, Ordering::SeqCst);
                    active.stop_flag.store(true, Ordering::SeqCst);
                    if let Some(h) = active.thread_handle.take() {
                        let _ = h.join();
                    }
                    if !active.best_move_emitted.swap(true, Ordering::SeqCst) {
                        let r = *active.last_result.lock().unwrap();
                        let mut w = writer.lock().unwrap();
                        let _ = writeln!(w, "{}", bestmove_line(r));
                        let _ = w.flush();
                    }
                }
            }
            "quit" => {
                if let Some(mut active) = active_search.take() {
                    active.stop_flag.store(true, Ordering::SeqCst);
                    if let Some(h) = active.thread_handle.take() {
                        let _ = h.join();
                    }
                }
                break;
            }
            "bench" => {
                if let Some(mut active) = active_search.take() {
                    active.stop_flag.store(true, Ordering::SeqCst);
                    if let Some(h) = active.thread_handle.take() {
                        let _ = h.join();
                    }
                }
                let depth = args.first().and_then(|s| s.parse::<usize>().ok());
                let mut w = writer.lock().unwrap();
                crate::bench::run_bench_with_writer(depth, &mut *w);
                let _ = w.flush();
            }
            "eval" => {
                let mut w = writer.lock().unwrap();
                let _ = writeln!(w, "{}", eval_report(&board));
                let _ = w.flush();
            }
            _ => {}
        }
    }
}

/// Convenience entry point running UCI on standard input and standard output.
pub fn run_uci_loop_stdio() {
    let stdin = std::io::stdin();
    let stdout = Arc::new(Mutex::new(std::io::stdout()));
    run_uci(stdin.lock(), stdout);
}
