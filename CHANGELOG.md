# CHANGELOG — Quant

Every merged change, with bench signature and nps where relevant. Measured on the reference machine, release build.

## Unreleased

### Capture history, variant 2 (bench 2458792 → 2405295)
- Table [moving piece][to][captured piece] with gravity (saturates at ±16384). A capture that fails high gets +min(16·depth², 1200); captures tried before it at that node get the same as a malus.
- Capture ordering key: SEE class (good/bad), then MVV-LVA + capture history / 8 (variant 2; / 32 was the first try).

### SEE: capture ordering and SEE pruning (bench 4533123 → 2458792)
- New `search::see::see_ge` (swap algorithm with x-rays; castling, en passant and promotions count as 0).
- Ordering: captures with SEE ≥ 0 first (MVV-LVA among them), losing captures after all quiet moves. Keys are computed once per move.
- Quiescence skips captures with SEE < 0. Main search (null-window, not in check, depth ≤ 6, after the first move) skips captures with SEE < −100·depth and quiets with SEE < −50·depth.

### Futility pruning (bench 6398189 → 4533123)
- At null-window nodes not in check with depth ≤ 4: once one move is searched, quiet non-killer moves that do not give check are skipped when static eval + 80 + 80·depth ≤ alpha. (The better of two earlier tries, +12.8 ± 8.6 but undecided at 4000 games; retested with a 20,000-game cap.)

### Bench depth 5 → 11 (bench 125351 → 6398189, no search change)
- Depth 5 was only ~125k nodes and never reached depth-6+ code paths (e.g. aspiration windows from depth 6). Depth 11 covers them: ~6.4M nodes, ~2 s on the reference machine.

### Check extensions (bench 116403 → 125351)
- A node in check searches one ply deeper (so quiescence is never entered in check). Ply is capped at 127 because extensions can lengthen lines.

### Reverse futility pruning (bench 216428 → 116403)
- At null-window nodes not in check with depth ≤ 6: return the static eval if it beats beta by 80·depth (not near mate scores). Static eval is now computed once per such node and shared with null move pruning.

### Late move reductions (bench 372854 → 216428)
- From the 4th move on, at depth ≥ 3, quiet non-killer moves that do not give check (node not in check) are searched with a null window reduced by ⌊0.75 + ln(depth)·ln(index)/2.25⌋ plies (one less at PV nodes); a reduced move that beats alpha is re-searched at full depth.

### Null move pruning (bench 481473 → 372854)
- At null-window nodes with depth ≥ 3, not in check, static eval ≥ beta, a non-pawn piece on the board and no null move just before: pass the turn and search with R = 3 + depth/4; fail high if that already beats beta.
- `Board::make_null_move` / `unmake_null_move` (clears EP, resets the halfmove clock so repetition detection never looks across it).

### Killer moves and history heuristic (bench 602730 → 481473)
- Quiet moves that cause a beta cutoff become killers for their ply (two slots) and get depth² butterfly history ([side][from][to], capped below killer scores). Order: TT move, captures (MVV-LVA), killer 1, killer 2, quiets by history.
- Startpos depth 8: 16.7M → 2.0M nodes.

### Phase 2 step 4: move ordering with TT move and MVV-LVA (bench 1400124 → 602730)
- In the tree: TT move first, then captures by MVV-LVA, then quiet moves; at the root the previous best move first, then MVV-LVA.
- Uses `sort_unstable_by_key` in the tree, which does not allocate (the stable sort used before could).

### Phase 2 step 3: transposition table (bench 1613766 → 1400124)
- Single-slot table sized by `Hash` (now clamped to the advertised 1–1024 MB), aging per `go`, mate scores stored relative to the node, cleared on `ucinewgame`, kept across moves.
- Cutoffs only at null-window nodes; stores the best move for move ordering (next step). Bench uses a fresh table per position, so the signature stays deterministic.

### Phase 2 step 2: principal variation search (bench 3701041 → 1613766)
- Iterative deepening (already present) now searches every move after the first with a null window and re-searches only when it beats alpha, at the root and in the tree. Same root scores at fixed depth, fewer nodes.

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
