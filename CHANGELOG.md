# CHANGELOG — Quant

Every merged change, with bench signature and nps where relevant. Measured on the reference machine, release build.

## Unreleased

### Phase 2 step 1: tapered evaluation with piece-square tables (bench 3251662 → 3701041)
- Middlegame/endgame material and piece-square values blended by game phase (N=B=1, R=2, Q=4, max 24). Tables are built from simple shapes: centrality for minor pieces/queen/endgame king, pawn advancement, rook on 7th, castled king shelter.
- Tests: colour-flip symmetry, central knight > rim knight, advanced pawn bonus.

### Quiescence search: score no longer flips with depth (bench 782700 → 3251662)
- Cause: not a negamax sign error (eval is already side-to-move relative), but the horizon effect of a material-only search without quiescence: at odd depths the side to move made the last capture (+1.00), at even depths the opponent did (−1.00).
- Fix: captures-only quiescence search with stand-pat and MVV-LVA ordering at depth 0.
- Bench changes because leaf nodes now run quiescence (more nodes per depth). Bench time ~1.2 s.
- Tests: startpos scores `cp 0` at depths 1–5; a queen-up position scores positive for White to move and negative for Black to move.

### Time management fix (bench unchanged: 782700)
- New UCI option `Move Overhead` (default 30 ms, 0–5000), subtracted from wtime/btime/movetime before allocating.
- Hard limit is capped at 80% of the remaining time (after overhead) whatever increment or movestogo say.
- No new iteration once half the soft limit is used, so moves no longer overrun their budget by a whole iteration.
- Tests: hard limit vs remaining time; UCI subprocess sends bestmove in time with wtime 500/100/20.

## 0.1.0 — Phase 1: Foundation (2026-10-17)

**Bench signature: 782700** (six standard positions, depth 5; first signature)

**Perft 6 startpos:** 119,060,324 nodes
- bulk counting: 851 ms, ~140 Mnps
- without bulk counting: 4.20 s, ~28 Mnps

Contents:
- Bitboard board (LERF), FEN parsing/serialisation, Zobrist hashing, hash history and halfmove clock.
- Fancy magic slider attacks with embedded constants; legal move generation with pins, check evasions, EP discovered-check handling; make/unmake with incremental hash.
- Perft with and without bulk counting, divide, six standard validation positions.
- UCI: `uci`, `isready`, `setoption` (Hash, Threads), `ucinewgame`, `position`, `go` (wtime/btime/winc/binc/movestogo/movetime/depth/nodes/infinite/ponder/perft), `stop`, `ponderhit`, `quit`, `bench`.
- Basic search: iterative-deepening alpha-beta, material eval, soft/hard time limits, `info` output.
- `bench` command; tests: perft suites, movegen edge cases, 10k random positions against shakmaty (move lists and perft 2), UCI subprocess tests, bench determinism.
- Tooling: Makefile, `scripts/run_sprt.sh` (fastchess), GitHub Actions CI.

Fixes during code review (no bench change):
- `ponderhit` no longer leaves the engine waiting for `stop` without moving.
- `position fen` rejects positions without exactly one king per side, pawns on rank 1/8 or an impossible EP square, and drops castling rights whose king or rook is not at home (previously a crash or invented moves).
