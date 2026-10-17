# CHANGELOG — Quant

Every merged change, with bench signature and nps where relevant. Measured on the reference machine, release build.

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
