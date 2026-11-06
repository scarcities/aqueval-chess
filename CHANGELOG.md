# CHANGELOG — aqueval chess

Until 2026-11-06 the engine was called Quant. Entries before the rename keep the old name, paths and variables.

Every merged change, with bench signature and nps where relevant. Measured on the reference machine (6 cores / 12 threads, AVX2), release build.

## 1.0.0 (2026-11-06)

### Renamed to aqueval (bench unchanged, 953967 / 912599)
- The project is now aqueval; the engine is aqueval chess and reports `id name aqueval 1.0.0` and `id author Deniz`. Crate and binary are `aqueval`, release archives `aqueval-<tag>-<platform>`.
- The net file is now `nets/aqueval-gen0-74884169.nnue` (same net, same sha256). `QUANT_NO_AVX2` is now `AQUEVAL_NO_AVX2`.

### v1.0 stability gauntlet and Hash tests (no code change, bench 953967)
- Run on a cloud server with 16 dedicated vCPUs instead of the reference machine, with main `495df92` (`fa926fa` before the 2026-11-06 history cleanup) (NNUE gen-0), built and bench-checked there (953967). 3.7 (`r37-lmr`) failed its SPRT, so it is not in v1.0.
- Gauntlet vs SF 17, `UCI_LimitStrength` with UCI_Elo 2900, 60+0.6, Hash 16, concurrency 16, UHO_Lichess_4852_v1: 1000 games, 613.5 points (61.4 %), +80.3 ± 17.9 Elo, so about **2980**. Like the earlier gauntlets, this is an estimate against limited Stockfish, not a CCRL rating.
- Hash 1, 16, 256 and 1024 MB: 200 games each at 10+0.1, main against itself at the same Hash, concurrency 15.
- All 1800 games ended normally: no crashes, no time losses, no illegal moves. fastchess warned 151 times in the gauntlet that Quant's `bestmove` differed from the start of its last printed PV, a cosmetic message.

### History tables kept across moves (bench unchanged, 953967 / 912599)
- Killers, butterfly, continuation, counter-move and correction tables now live with the TT between `go` commands of a game instead of starting empty each move. At each new `go` butterfly and continuation history are halved and killers cleared. `ucinewgame`, a new Hash size and each datagen game start fresh. Bench searches every position with a fresh TT, so its signature does not change.

### Butterfly history with gravity and malus (bench 1027683 → 953967; handcrafted build 1116769 → 912599)
- On a quiet beta cutoff the move gets `min(16 · depth², 1200)` and every quiet tried before it the same as a malus, with gravity (`h += b − h·|b| / 16384`) instead of the old `+depth²` capped at 700,000. Tables are still reset every `go`; keeping them across moves is a separate test.

### Multicut from the singular search (bench 1004476 → 1027683; handcrafted build 1177704 → 1116769)
- When the singular search (TT move excluded) still reaches `singular_beta` and `singular_beta ≥ beta`, at least two moves fail high: the node returns `singular_beta` without searching the TT move.

### Singular extensions (bench 547732 → 1004476; handcrafted build 1032928 → 1177704)
- When the TT move has a lower-bound or exact entry at least depth − 3 deep (depth ≥ 8, non-mate score, ply < 2 · iteration depth), the other moves are searched at half depth against `tt_score − 2 · depth` with the TT move excluded. If they all fail low, the TT move is extended by one ply. Searches with an excluded move skip TT cutoffs, null move pruning, TT stores and correction-history updates.

### Late move reductions from a fixed table (bench unchanged, 547732 / 1032928)
- `src/search/lmr.rs` holds the reductions as integer thresholds generated once from the old `0.75 + ln d · ln i / 2.25` formula, so no platform's `f64::ln` can change the search and the bench works as a cross-platform checksum. A test checks the table against the formula on x86-64 Linux. That was the only floating point in the search; NPS output and datagen's result field (exactly 0, 0.5 or 1) do not affect it.

### NNUE gen-0 as the default build (bench 1032928 → 547732)
- **Net:** `nets/quant-gen0-74884169.nnue` (sha256 `748841698123d80a1cbca77191e64904a35c5529babbc3e0c92bb8c6072108e8`), embedded by `cargo build`. From this entry on, every build embeds this net until a later entry names another one.
- **Why the bench changes:** the search now evaluates with the net instead of the handcrafted eval, so it visits different nodes. 547732 is the default build; `--no-default-features` (handcrafted eval only) keeps 1032928.
- **Strength:** SPRT vs HCE main H1, 149.5/152 at 8+0.08; gauntlet vs SF 17 limited ~2875 (HCE ~2560).
- **Fallback:** AVX2 is detected at run time; without it the scalar kernels run (same results, about half the speed). `QUANT_NO_AVX2=1` forces them; CI runs the tests that way too. `UseNNUE false` switches to the handcrafted eval at run time.

### UCI: `bestmove 0000` without legal moves (bench unchanged)
- `Move::NULL` prints as `0000` (UCI's null move) instead of `a1a1`, e.g. when `go` is sent in a mate or stalemate position.

### FEN: reject positions where the side not to move is in check (bench unchanged, 1032928)
- `Board::from_fen` returns `InvalidPiecePlacement` for them; before, `go` on such a position panicked in the movegen. Three tests used positions of that kind (the opponent's king attacked along an open file) and now use a legal king square with the same idea.

### Time management: use the soft limit, keep aborted iterations (bench unchanged, 1032928)
- A new iteration starts while less than the soft limit has elapsed (was half of it); the hard limit still stops the search. When an iteration is aborted, its best move is kept if it already beat the aspiration window. Bench is depth-limited, so the signature does not change.

### Aspiration windows on fail-soft (bench 817175 → 1032928)
- From depth 4 the root searches a ±20 window around the last score; on a fail-low the window drops (beta to the middle, alpha below the result), on a fail-high it rises and the next try is one ply shallower (at most two). The window grows by half on every fail, and the move that failed high is searched first.

### Fail-soft search (bench 1029973 → 817175)
- Beta cutoffs, fail-lows, null move pruning (except unproven mates) and quiescence return the best score found instead of the window bound, and the TT stores it. Enabler for aspiration windows (docs/ROADMAP.md 1.4/1.5).

### Static eval correction history, pawn structure (bench 888437 → 1029973)
- Table [side][14-bit pawn-structure hash] of learned (search score − raw eval), added to the static eval used for pruning and "improving". Updated when a node's result is reliable relative to the eval (exact, or a fail-high above / fail-low below it, with a quiet or no best move), weighted by depth, clamped to ±128 cp. Reset every `go`.

### Draw hygiene (bench 888666 → 888437)
- Repetition inside the search: a single earlier occurrence on the search path (from the root on) is a draw; positions from the game before the root still need a threefold. Previously the search only scored threefold repetitions.
- No TT cutoffs at halfmove clock ≥ 90, where a stored score can miss an approaching 50-move draw.

### NNUE infrastructure (bench unchanged, 888666)
- `src/nnue/` ((768 → 256)x2 → 1 SCReLU, quantised, lazy incremental accumulators, AVX2 at run time), `search::evaluator::Evaluator`, `quant datagen`, `scripts/nnue/train.py`, netviz reads the net format. The search uses NNUE only in `--features nnue` builds with a net; the default build is unchanged. Details in docs/NNUE.md.

### Late move pruning with "improving" (bench 1387095 → 888666)
- At null-window nodes not in check with depth ≤ 6, quiet non-killer moves after the first 3 + depth² (improving) or (3 + depth²)/2 (not improving) are skipped. Earlier move-count LMP without improving lost ~21 Elo.

### "Improving" heuristic (bench 2309154 → 1387095)
- Static eval is now stored per ply; a node is improving when its eval beats the eval at our previous turn (2 plies back, or 4 if we were in check). Static eval is now computed at every node not in check.
- RFP margin 80·(depth − improving); LMR reduces one ply more when not improving; futility margin 80·depth + 80 when improving, + 40 when not.

### Continuation history, variant 2 (bench 2430886 → 2309154)
- Table [previous piece][previous to][piece][to] (flat, 2.4 MB per search, allocated once per `go`), looked up with the moves 1 and 2 plies back. A quiet cutoff gives +min(16·depth², 1200) to the cutoff move and the same malus to the quiets tried before it, with gravity at ±16384.
- Quiet ordering key: butterfly history + 2 × (both continuation scores) (variant 2; weight 1 was undecided).

### Counter move heuristic (bench 2458792 → 2430886)
- A quiet move that fails high is stored as the counter to the previous move ([previous piece][previous to-square]); it is ordered after the two killers and before history-ordered quiets.

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
