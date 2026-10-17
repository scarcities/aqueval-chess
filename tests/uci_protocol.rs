use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use quant::uci::run_uci;

/// Helper to run synchronous UCI commands in-process with a mock reader and writer.
fn run_uci_in_process(input: &str) -> String {
    let output = Arc::new(Mutex::new(Vec::new()));
    let reader = std::io::Cursor::new(input.as_bytes().to_vec());
    run_uci(reader, output.clone());
    let bytes = output.lock().unwrap().clone();
    String::from_utf8(bytes).expect("Valid UTF-8 output")
}

/// Helper managing an interactive UCI subprocess session.
struct UciSession {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl UciSession {
    fn start() -> Self {
        let bin_path = env!("CARGO_BIN_EXE_quant");
        let mut child = Command::new(bin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start quant binary");

        let stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let reader = BufReader::new(stdout);

        Self {
            child,
            stdin,
            reader,
        }
    }

    fn send(&mut self, cmd: &str) {
        writeln!(self.stdin, "{}", cmd).unwrap();
        self.stdin.flush().unwrap();
    }

    fn read_line(&mut self) -> String {
        let mut line = String::new();
        self.reader.read_line(&mut line).unwrap();
        line.trim().to_string()
    }

    fn read_until_match<F: Fn(&str) -> bool>(&mut self, predicate: F) -> String {
        loop {
            let line = self.read_line();
            if predicate(&line) {
                return line;
            }
        }
    }

    fn read_until_bestmove(&mut self) -> (Vec<String>, String) {
        let mut lines = Vec::new();
        loop {
            let line = self.read_line();
            if let Some(stripped) = line.strip_prefix("bestmove ") {
                let bm = stripped.trim().to_string();
                return (lines, bm);
            }
            lines.push(line);
        }
    }

    fn quit(mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.child.wait();
    }
}

#[test]
fn test_uci_handshake() {
    let out = run_uci_in_process("uci\nquit\n");
    assert!(out.contains("id name Quant 0.1.0"), "Missing engine name: {out}");
    assert!(out.contains("id author Quant Engine Team"), "Missing author: {out}");
    assert!(out.contains("option name Hash"), "Missing Hash option: {out}");
    assert!(out.contains("option name Threads"), "Missing Threads option: {out}");
    assert!(out.contains("uciok"), "Missing uciok: {out}");
}

#[test]
fn test_isready() {
    let out = run_uci_in_process("isready\nquit\n");
    assert!(out.contains("readyok"), "Missing readyok: {out}");
}

#[test]
fn test_setoption_case_insensitive() {
    let script = "uci\nsetoption name Hash value 64\nsetoption name hash value 32\nsetoption name Threads value 1\nisready\nquit\n";
    let out = run_uci_in_process(script);
    assert!(out.contains("readyok"), "Must answer readyok after setoption: {out}");
}

#[test]
fn test_position_and_go_depth() {
    let mut session = UciSession::start();
    session.send("uci");
    session.read_until_match(|l| l == "uciok");

    session.send("position startpos moves e2e4 e7e5");
    session.send("go depth 2");

    let (lines, bestmove) = session.read_until_bestmove();
    session.quit();

    assert!(lines.iter().any(|l| l.contains("info depth 1")), "Missing depth 1 info: {lines:?}");
    assert!(lines.iter().any(|l| l.contains("info depth 2")), "Missing depth 2 info: {lines:?}");
    assert!(!bestmove.is_empty(), "Bestmove must not be empty");
}

#[test]
fn test_position_fen_tactical_capture() {
    let mut session = UciSession::start();
    session.send("position fen 4k3/8/8/4q3/8/8/4Q3/3K4 w - - 0 1");
    session.send("go depth 2");

    let (_, bestmove) = session.read_until_bestmove();
    session.quit();

    assert_eq!(bestmove, "e2e5", "Engine must capture hanging queen with e2e5");
}

#[test]
fn test_go_perft_divide() {
    let script = "position startpos\ngo perft 2\nquit\n";
    let out = run_uci_in_process(script);
    assert!(out.contains("a2a3: 20"), "Missing divide move a2a3: {out}");
    assert!(out.contains("Nodes searched: 400"), "Missing perft total 400: {out}");
    assert!(!out.contains("bestmove"), "perft command must not output bestmove: {out}");
}

#[test]
fn test_ucinewgame_resets_board() {
    let mut session = UciSession::start();
    session.send("position startpos moves e2e4");
    session.send("ucinewgame");
    session.send("isready");
    session.read_until_match(|l| l == "readyok");

    session.send("go depth 1");
    let (_, bestmove) = session.read_until_bestmove();
    session.quit();

    assert!(!bestmove.is_empty());
}

#[test]
fn test_whitespace_and_unknown_token_tolerance() {
    let script = "   \t  unknown_command_prefix   isready   \t \nquit\n";
    let out = run_uci_in_process(script);
    assert!(out.contains("readyok"), "Must tolerate leading whitespace/unknown tokens: {out}");
}

#[test]
fn test_stop_when_idle_is_silent_noop() {
    let script = "stop\nisready\nquit\n";
    let out = run_uci_in_process(script);
    assert!(out.contains("readyok"), "Must answer readyok: {out}");
    assert!(!out.contains("bestmove"), "Stop when idle must not emit bestmove: {out}");
}

#[test]
fn test_bench_via_uci() {
    let script = "bench 2\nquit\n";
    let out = run_uci_in_process(script);
    assert!(out.contains("Nodes searched  :"), "Missing bench nodes searched: {out}");
    assert!(out.contains("Nodes/second    :"), "Missing bench nps: {out}");
}

#[test]
fn test_subprocess_go_infinite_and_stop() {
    let mut session = UciSession::start();
    session.send("uci");
    session.read_until_match(|l| l == "uciok");

    session.send("position startpos");
    session.send("go infinite");

    // Wait 150ms while search is running
    thread::sleep(Duration::from_millis(150));

    // Non-blocking isready test
    session.send("isready");
    let ready_res = session.read_until_match(|l| l == "readyok");
    assert_eq!(ready_res, "readyok");

    // Send stop
    session.send("stop");
    let (_, bestmove) = session.read_until_bestmove();
    session.quit();

    assert!(!bestmove.is_empty(), "Must emit bestmove after stop");
}

#[test]
fn test_subprocess_go_ponder_and_stop() {
    let mut session = UciSession::start();
    session.send("uci");
    session.read_until_match(|l| l == "uciok");

    session.send("position startpos");
    session.send("go ponder");

    thread::sleep(Duration::from_millis(150));

    session.send("isready");
    let ready_res = session.read_until_match(|l| l == "readyok");
    assert_eq!(ready_res, "readyok");

    session.send("stop");
    let (_, bestmove) = session.read_until_bestmove();
    session.quit();

    assert!(!bestmove.is_empty(), "Must emit bestmove after stop on ponder");
}

#[test]
fn test_subprocess_ponderhit_emits_bestmove() {
    let mut session = UciSession::start();
    session.send("position startpos");
    // Time limits make the search finish early and then wait; ponderhit must still move.
    session.send("go ponder wtime 2000 btime 2000");
    thread::sleep(Duration::from_millis(300));
    session.send("ponderhit");
    session.send("isready");

    let mut saw_bestmove = false;
    loop {
        let line = session.read_line();
        saw_bestmove |= line.starts_with("bestmove ");
        if line == "readyok" {
            break;
        }
    }
    session.quit();
    assert!(saw_bestmove, "ponderhit must produce bestmove before readyok");
}

#[test]
fn test_subprocess_low_time_bestmove_in_time() {
    let mut session = UciSession::start();
    session.send("isready");
    session.read_until_match(|l| l == "readyok");

    // Kiwipete: wide tree, so the search would overrun any limit it does not enforce.
    let kiwipete = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R";
    for (side, time) in [("w", 500u64), ("w", 100), ("w", 20), ("b", 20)] {
        session.send(&format!("position fen {kiwipete} {side} KQkq - 0 1"));
        let start = std::time::Instant::now();
        session.send(&format!("go wtime {time} btime {time}"));
        let (_, bestmove) = session.read_until_bestmove();
        let elapsed = start.elapsed().as_millis() as u64;
        assert!(elapsed < time, "{side} wtime/btime {time}: bestmove after {elapsed} ms");
        assert_ne!(bestmove, "0000");
    }

    // Large increment and movestogo 1 must not push the search past the clock either.
    session.send(&format!("position fen {kiwipete} w KQkq - 0 1"));
    let start = std::time::Instant::now();
    session.send("go wtime 300 btime 300 winc 5000 binc 5000 movestogo 1");
    session.read_until_bestmove();
    assert!(start.elapsed().as_millis() < 300);
    session.quit();
}
