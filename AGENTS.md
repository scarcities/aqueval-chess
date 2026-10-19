# AGENTS.md — Quant

Living document for anyone (human or agent) working on Quant, an original UCI chess engine in Rust.
Target machine: the reference machine (Zen+, 6c/12t, AVX2, no AVX-512, slow PEXT), 32 GB RAM.

## Architecture (Phase 1)

| Module | What it does |
|---|---|
| `src/types.rs` | `Square` (LERF, A1=0), `Color`, `PieceType`, `Piece`, `Bitboard` (`u64` wrapper) |
| `src/board/` | `Board` with piece/colour bitboards, FEN parse/serialise with validation, Zobrist (SplitMix64 keys), make/unmake with `UndoState`, hash history for repetition, halfmove clock |
| `src/movegen/` | Fancy magic bitboards (embedded constants), step attacks, 16-bit `Move`, stack `MoveList` (`[Move; 256]`), strictly legal generation via check masks and pin rays |
| `src/perft/` | Perft with and without bulk counting, divide, the six standard positions |
| `src/search/` | Phase 1 placeholder: iterative-deepening alpha-beta, material eval, soft/hard time limits |
| `src/uci/` | UCI loop; search runs on its own thread so `stop`/`isready` stay responsive; `go perft N` prints divide |
| `src/bench/` | Searches the six standard positions to depth 5; total nodes = bench signature |
| `tests/` | Perft suites, movegen edge cases, 10k-position shakmaty cross-check, UCI subprocess tests, bench regression |
| `harness/` | External perft harness that tests the shipped binary over UCI (see `harness/README.md`) |

Invariants: no heap allocation in movegen, make/unmake or search recursion; `shakmaty` is a dev-dependency only (R1).

## Current status

- **Elo estimate (vs limited Stockfish):** ~2430 (2026-10-19, after Phase 2 features: PST eval, PVS, TT, TT/MVV-LVA ordering, killers + history, null move pruning, LMR, reverse futility pruning, check extensions). Approximation against Stockfish 17 with `UCI_LimitStrength`, not a CCRL rating; SF's Elo scale is calibrated at longer time controls, so levels disagree somewhat.

  | Date | Opponent | TC | Games | Score | Elo diff | Implied Quant Elo |
  |---|---|---|---|---|---|---|
  | 2026-10-19 | SF 17, UCI_Elo 2300 | 10+0.1 | 200 | 130.5 (65.3 %) | +109 ± 47 | ~2409 |
  | 2026-10-19 | SF 17, UCI_Elo 2600 | 10+0.1 | 200 | 59.0 (29.5 %) | −151 ± 50 | ~2448 |
  | 2026-10-17 | SF 17, UCI_Elo 2000 | 10+0.1 | 200 | 110.5 (55.3 %) | +37 ± 46 | ~2036 |
  | 2026-10-17 | SF 17, UCI_Elo 2300 | 10+0.1 | 200 | 86.5 (43.3 %) | −47 ± 44 | ~2252 |

  2026-10-17 rows: after steps 1–4 only (~2150). Levels picked from 20-game probes. Rerun: `scripts/elo_gauntlet.sh` with `LEVELS="2300 2600"`.
- **Bench signature:** 125351
- **Perft 6 startpos:** see CHANGELOG.md for nps (bulk and non-bulk).
- **Roadmap:** Phase 1 (Foundation) complete. Phase 2 (Search) in progress: core search, ordering and basic pruning merged; aspiration windows, late move pruning and futility pruning not yet (see SPRT log).

## Ideas tried

| Idea | Result |
|---|---|
| Fancy magics vs PEXT for sliders | Magics chosen: PEXT is microcoded (~18 cycles) on Zen+, magics use one `imul`. Bulk perft reaches ~140 Mnps. |
| Stack `MoveList` vs `Vec` | Stack array kept: zero heap allocations in movegen. |
| Strict legal movegen vs pseudo-legal + check | Legal generation kept: lets perft bulk-count at the last ply. |
| Aspiration windows (fail-hard search, ±25 from depth 4 and ±50 from depth 6) | Not merged: both ~+4 Elo, SPRT [0, 5] undecided after 4000 games each. Code on branches `p2/aspiration`, `p2/aspiration-v2`. Worth retrying after a fail-soft search. |
| Late move pruning (move-count based, no history) | Not merged: −21 Elo, conservative variant ~−4. Code on `p2/lmp-main`, `p2/lmp-v2`. Retry with history-aware move counts / an "improving" flag. |
| Futility pruning (static eval margin, fail-hard) | Not merged under the 4000-game cap, but both tries were positive (+10 and +13 Elo). Code on `p2/futility-nolmp`, `p2/futility-v2`. Best candidate to retest with a longer SPRT. |

## SPRT log

Run with `scripts/run_sprt.sh`, one test at a time, fastchess concurrency 10 (2 of 12 threads free), 8+0.08, UHO_Lichess_4852_v1. Log every result here, including failures.

**Bounds:** from 2026-10-17, early Phase 2 features with large expected gains (PST, TT, move ordering, basic pruning) use `big-gainer` [0, 10]. Switch back to `gainer` [0, 5] once single changes are expected to gain less than ~20 Elo, and note the switch here.

**Switch (2026-10-18):** back to [0, 5] from aspiration windows on (aspiration windows, check extensions, late move pruning, futility pruning are each expected to gain less than ~20 Elo).

| Change | Bounds | Result | Elo | Games |
|---|---|---|---|---|
| Tapered eval + PST (vs Phase 1 search with quiescence) | [0, 5] | H1 accepted | +501 ± 89 | 312 |
| PVS (vs tapered eval) | [0, 5] | H1 accepted | +80 ± 20 | 780 |
| Transposition table (vs PVS) | [0, 10] | H1 accepted | +73 ± 27 | 452 |
| Move ordering: TT move + MVV-LVA (vs TT) | [0, 10] | H1 accepted | +102 ± 33 | 362 |
| Killer moves + history heuristic (vs TT/MVV-LVA ordering) | [0, 10] | H1 accepted | +33 ± 17 | 1028 (rerun; first run cut off at 1658 games by sleep) |
| Null move pruning (vs killers/history) | [0, 10] | H1 accepted | +111 ± 36 | 336 |
| Late move reductions (vs null move pruning) | [0, 10] | H1 accepted | +96 ± 32 | 356 |
| Reverse futility pruning (vs LMR) | [0, 10] | H1 accepted | +130 ± 38 | 282 |
| Aspiration windows ±25 from depth 4 (vs RFP) | [0, 5] | failed: no decision at 4000-game cap | +4.5 ± 8.8 | 4000 |
| Aspiration windows ±50 from depth 6, the one variation (vs RFP) | [0, 5] | failed: no decision at 4000-game cap | +4.1 ± 8.5 | 4000 |
| Check extensions (vs RFP; aspiration not merged) | [0, 5] | H1 accepted | +38 ± 14 | 1666 |
| Late move pruning, depth ≤ 4, after 3 + depth² quiets (vs check extensions) | [0, 5] | H0 accepted | −21 ± 11 | 2576 |
| Late move pruning, depth ≤ 3, after 5 + 2·depth², the one variation (vs check extensions) | [0, 5] | failed: −4 ± 10, LLR −0.99 when the run died at 3284 games (machine offline); H1 out of reach before the 4000 cap, not rerun | −3.9 ± 9.8 | 3284 |
| Futility pruning, depth ≤ 3, margin 150·depth (vs check extensions) | [0, 5] | failed: no decision at 4000-game cap (LLR 1.69) | +9.6 ± 8.2 | 4000 |
| Futility pruning, depth ≤ 4, margin 80 + 80·depth, the one variation (vs check extensions) | [0, 5] | failed: no decision at 4000-game cap (LLR 2.25) | +12.8 ± 8.6 | 4000 |

## Future roadmap

### Phase 2: Search

- Core: PVS, iterative deepening, aspiration windows, transposition table with aging and buckets, quiescence search with SEE
- Move ordering: TT move, good captures by SEE plus capture history, killers, counter moves, butterfly history, continuation history (1 and 2 ply), bad captures last
- Pruning and reductions: reverse futility pruning, null move pruning with verification, razoring, futility pruning, late move pruning, SEE pruning, LMR with tuned log formula and history-based adjustments, ProbCut, internal iterative reductions
- Extensions: singular extensions with double and negative extensions, check extensions
- "Improving" heuristic, static eval correction history (pawn, material, non-pawn)
- Mate distance pruning, repetition and 50-move handling, upcoming repetition detection
- Handcrafted eval as temporary baseline: tapered material plus piece-square tables, Texel-tuned
- Every feature tested with SPRT before merging

### Phase 3: NNUE

- Data generation: self-play at fixed nodes with randomized openings, adjudication, filtering
- Architecture: 768→256→1, then scale to HalfKA-style king input buckets, output buckets, hidden layer 1024+, SCReLU
- Quantized inference with SIMD (AVX2 only on target hardware)
- Efficiently updated accumulators with lazy updates
- Reinforcement loop: generate data → retrain → SPRT new net vs old → repeat

### Phase 4: Scaling

- SPSA tuning of search parameters
- Lazy SMP multithreading
- Syzygy tablebase probing
- Time management refinement: node-based scaling, best-move stability, eval instability
- Submit to CCRL once stable

## Process rules

1. Changes not meant to alter behaviour must keep the bench signature; every signature change goes in CHANGELOG.md with a reason.
2. `cargo test --release` (includes perft) must pass after every change.
3. No heap allocations in movegen, make/unmake and search.
4. Tasks in TASKS.md are `[LEAD]` (correctness/performance-critical) or `[DELEGATE]` (scripts, tests, CI, docs, refactors, with self-contained specs).
5. SPRT: gainers [0, 5], simplifications [-5, 0], normalized; log all results above.
6. One commit per completed milestone; delegate tasks on `task/<TASK-ID>` branches, reviewed by the lead before merging.
7. Ask the user before any large architectural change.
