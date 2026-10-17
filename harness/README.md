# UCI perft harness

A black-box safety net for Quant's move generation. It talks to the engine
binary only through UCI (`position ...` and `go perft N`), so it does not
depend on internal Rust APIs and keeps working through refactors. It
complements the in-crate `cargo test` perft and shakmaty tests rather than
replacing them: this one tests the shipped binary end to end, including FEN
parsing, `position startpos` / `position ... moves` handling and the divide
output.

Python 3 standard library only; nothing to install. This folder is maintained
separately from the engine source.

## Usage

```bash
cargo build --release
python3 harness/perft.py --engine target/release/quant              # entries up to 5M nodes
python3 harness/perft.py --engine target/release/quant --max-nodes 0  # full suite (incl. 193M Kiwipete D5)
python3 harness/perft.py --engine target/release/quant --only kiwipete,cpw-pos3
python3 harness/perft.py --engine target/release/quant --json perft.json --log uci.log
```

Until the engine speaks UCI, its command-line perft can be tested through a
shim (moves are resolved to a FEN by `refgen.py`, so this does not test the
engine's own `moves` handling; if the engine crashes, the shim exits too, so the
harness reports the crash with its output instead of waiting for a timeout):

```bash
python3 harness/perft.py --engine "python3 harness/cli_shim.py target/release/quant" --max-nodes 0
```

Exit code: `0` all passed, `1` a wrong result, `2` engine or harness error
(crash, timeout, unrecognised output, bad arguments, nothing tested).

What a run does:

1. **Probes** (skip with `--no-probes`): `position startpos` + `go perft 1`,
   `position startpos moves ...` reaching an en passant position,
   `position fen ... moves ...` with castling and a capture-promotion, and one
   with an underpromotion to a knight. Expected counts come from `refgen.py`.
   If the engine does not answer the first probe in a recognisable format
   within `--probe-timeout` (30 s), the run stops at once with exit 2 and shows
   the engine's last output lines.
2. **Suite**: each position from depth 1 upward, stopping at its first failing
   depth. A depth passes only if the total matches, divide lines are present
   (unless there are no legal moves), they sum to the total, no move is listed
   twice, and, at the first depth run, the root move names equal the
   reference's (this catches castling printed as `e1h1`).
3. **Moves re-check**: at each position's first passing depth >= 2, up to three
   root moves (castle, en passant, promotion, double push when present) are
   replayed as `position fen ... moves <m>` + `go perft <d-1>` and must match
   the divide count.

A crash or timeout anywhere is an error (exit 2); the engine is restarted for
the next position, and after 3 errors in a row the run stops. Engines are
started in their own process group and killed with it, so wrapper commands
such as `cargo run --release --` leave no orphans, also on SIGTERM, SIGHUP and
Ctrl-C. `--json` is always written, including for aborted runs (`"aborted"`
says why). `--startup-timeout` (30 s) bounds the `uciok`/`readyok` handshake.

## Automatic bug localisation

On a wrong total the harness bisects with perft divide against a reference: it
compares per-move counts, follows the first differing move one ply deeper, and
repeats until the defect is pinned down. It reports one of:

- **wrong move list**: the position, how to reach it, and the missing,
  illegal or duplicated moves (with a hint if castling is printed as `e1h1`);
- **wrong count from the parent, correct when set up fresh**: the engine's
  count for a move differs between "counted below the parent" and "set up
  fresh with `position ... moves`", while the fresh count is correct. This
  points at make/unmake not restoring state, a stale perft cache, or a
  last-ply bulk count that disagrees with the move list;
- **inconsistent engine / nondeterministic result**: the same position gives
  different counts depending on how or when it is asked;
- **total line wrong**: the divide is right but the printed total is not.

Example (from the self-test, with an injected en passant pin bug):

```
illegal-ep-2                 D2  FAIL   total 103, expected 102 (+1)
    divide D2 after [root]
     d2d4: engine 8 vs reference 7
    divide D1 after [d2d4]
  >> culprit position: 8/8/4k3/8/2pP4/8/B5K1/8 b - d3 0 1
     reached by: position fen 8/8/4k3/8/2p5/8/B2P2K1/8 w - - 0 1 moves d2d4
     engine generates ILLEGAL move(s): c4d3
```

References:

- `--reference builtin` (default): `refgen.py`, a small independent Python
  move generator (roughly 70-200k nodes/s). It only bisects failures up to
  `--bisect-budget` nodes (default 5M, 0 = no limit); deeper failures print a
  hint instead. It refuses invalid FENs (bad syntax, no king, inconsistent
  castling or en passant) rather than guessing.
- `--reference /path/to/stockfish`: any UCI engine with `go perft`, fast at any
  depth. It is probed with `go perft 1` from the start position first.
- `--reference none`: report mismatches only.

The reference's root count is checked against the suite before bisecting, and
unparseable or inconsistent reference output aborts the bisection instead of
producing a diagnosis.

## What the engine must do

- Answer `uci` with `uciok` and `isready` with `readyok`.
- Accept `position startpos [moves ...]` and
  `position fen <FEN> [moves <uci moves>]`.
- On `go perft N`, print one line per root move as `<uci move>: <nodes>`
  (e.g. `e2e4: 20`; promotions lowercase like `e7e8q`; castling as `e1g1`),
  then a total line such as `Nodes searched: <n>` (also accepted: `Total: <n>`,
  `Nodes: <n>`, optionally followed by more text). Lines after the total
  (`Time:`, `NPS:`) and `info ...` lines are ignored.
- The harness sends `isready` only after it has seen the total line and reads
  up to `readyok` to discard trailing output, so an engine may run perft on a
  search thread.

## Files

| File | Purpose |
| --- | --- |
| `perft.py` | Runner, probes, bisection, `--verify-suite`, `--self-test` |
| `suites/standard.epd` | 21 positions, 116 counts: CPW 1-6 (+ mirrored 4), illegal en passant, castling into/through check, promotions, stalemate/mate |
| `refgen.py` | Reference move generator used for probes, bisection and suite verification |
| `mock_engine.py` | Tiny UCI engine on top of `refgen.py` with injectable bugs, for testing the harness |
| `cli_shim.py` | Temporary UCI front end for an engine that only has `quant divide <depth> <fen>` |
| `oracle/` | Standalone Rust tool (shakmaty 0.27.3) that recomputes every suite count |

## Checking the harness itself

```bash
python3 harness/perft.py --self-test      # 11 cases, about 15 s
python3 harness/perft.py --verify-suite   # re-derives suite counts up to --max-nodes (default 5M) with refgen; several minutes
python3 harness/perft.py --verify-suite --reference /path/to/stockfish --max-nodes 0   # all counts, via a UCI engine
cargo run --offline --release --manifest-path harness/oracle/Cargo.toml -- harness/suites/standard.epd
```

The self-test runs a clean mock engine and mocks with injected bugs, and checks
the exit code and the exact diagnosis for each: an illegal en passant capture
through a pin (also with a UCI engine as reference), missing underpromotions,
castling rights lost on unmake, an engine that ignores `moves`, a move listed
twice, castling printed as `e1h1`, an unrecognised total line, a crash on one
position, and an engine that cannot be restarted after a crash.

Suite format: `<FEN> ;D1 <nodes> ;D2 <nodes> ...`, with a `# name` comment
line before each position. Deep counts come from the Chess Programming Wiki
and the well-known tricky-position collection; shallower counts for the tricky
positions were filled in with `refgen.py`. All 116 counts were recomputed with
shakmaty on 2026-10-17 and match.
