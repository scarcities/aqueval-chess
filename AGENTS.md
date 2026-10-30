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
| `src/bench/` | Searches the six standard positions to depth 11; total nodes = bench signature |
| `src/nnue/` | Phase 3 NNUE: (768 → 256)x2 → 1 SCReLU net, loader (bullet quantised.bin layout), lazy accumulator stack, AVX2 kernels picked at run time. On by default (`nnue` feature, net from `nets/` embedded by build.rs); scalar fallback without AVX2; see docs/NNUE.md |
| `src/search/evaluator.rs` | All make/unmake and static evals in the search go through `Evaluator` (HCE in the default build, NNUE accumulators in nnue builds) |
| `src/datagen/` | `quant datagen`: self-play training data, `<FEN> \| <cp> \| <result>` per line |
| `scripts/nnue/train.py` | Quant's own NNUE trainer (PyTorch, CPU or CUDA), data packing, exact integer eval for cross-checks |
| `tests/` | Perft suites, movegen edge cases, 10k-position shakmaty cross-check, UCI subprocess tests, bench regression |
| `harness/` | External perft harness that tests the shipped binary over UCI (see `harness/README.md`) |

Invariants: no floating point in the search or eval (bench = cross-platform checksum; LMR uses the integer table in `src/search/lmr.rs`); no heap allocation in movegen, make/unmake or search recursion; `shakmaty` is a dev-dependency only (R1). Search code makes and unmakes moves and evaluates only through `ctx.eval` (`Evaluator`), so NNUE accumulators stay in step.

## Current status

- **Elo estimate (vs limited Stockfish):** ~2560 (2026-10-23, gauntlet "HCE final" after docs/ROADMAP.md Phase 1: draw hygiene, pawn correction history, fail-soft, aspiration windows, time management on top of the 2026-10-21 search). Approximation against Stockfish 17 with `UCI_LimitStrength`, not a CCRL rating; SF's Elo scale is calibrated at longer time controls, so levels disagree somewhat (here by ~70).

  | Date | Opponent | TC | Games | Score | Elo diff | Implied Quant Elo |
  |---|---|---|---|---|---|---|
  | 2026-10-26 | SF 17, UCI_Elo 2600 (NNUE gen-0 build) | 10+0.1 | 200 | 163.0 (81.5 %) | +258 ± 57 | ~2857 |
  | 2026-10-26 | SF 17, UCI_Elo 2900 (NNUE gen-0 build) | 10+0.1 | 200 | 97.5 (48.8 %) | −9 ± 35 | ~2891 |
  | 2026-10-23 | SF 17, UCI_Elo 2300 | 10+0.1 | 200 | 157.0 (78.5 %) | +225 ± 56 | ~2524 |
  | 2026-10-23 | SF 17, UCI_Elo 2600 | 10+0.1 | 200 | 99.0 (49.5 %) | −3 ± 45 | ~2596 |
  | 2026-10-19 | SF 17, UCI_Elo 2300 | 10+0.1 | 200 | 130.5 (65.3 %) | +109 ± 47 | ~2409 |
  | 2026-10-19 | SF 17, UCI_Elo 2600 | 10+0.1 | 200 | 59.0 (29.5 %) | −151 ± 50 | ~2448 |
  | 2026-10-17 | SF 17, UCI_Elo 2000 | 10+0.1 | 200 | 110.5 (55.3 %) | +37 ± 46 | ~2036 |
  | 2026-10-17 | SF 17, UCI_Elo 2300 | 10+0.1 | 200 | 86.5 (43.3 %) | −47 ± 44 | ~2252 |

  NNUE gen-0 rows (`--features nnue` build with the gen-0 net, not yet the default build): ~2875; probes 2600: 82.5 %, 2900: 42.5 %, 3190: 17.5 %. 2026-10-23 levels picked from 20-game probes (2600: 47.5 %, 2900: 10 %, 3190: 10 %). 2026-10-19 rows: ~2430. 2026-10-17 rows: after steps 1–4 only (~2150). Rerun: `scripts/elo_gauntlet.sh` with `LEVELS="2300 2600"`.
- **Bench signature:** 953967 (default build, NNUE gen-0); 912599 with `--no-default-features` (handcrafted eval)
- **Perft 6 startpos:** see CHANGELOG.md for nps (bulk and non-bulk).
- **Roadmap:** docs/ROADMAP.md (integration plan). Phase 1 (search, steps 1.0–1.7) done 2026-10-23: all merged except TT buckets (1.2, H0 twice). Phase 2 NNUE gen-0: datagen 2026-10-23 07:48 → 2026-10-24 02:08 (87.3 M positions), training 40 epochs (paused 2026-10-24 09:02 → 2026-10-26 14:53, resumed from the epoch-22 checkpoint after a reboot), SPRT vs HCE H1 2026-10-26. NNUE gen-0 became the default build 2026-10-26 (2.4); next: Phase 3 of docs/ROADMAP.md.

## Ideas tried

| Idea | Result |
|---|---|
| Fancy magics vs PEXT for sliders | Magics chosen: PEXT is microcoded (~18 cycles) on Zen+, magics use one `imul`. Bulk perft reaches ~140 Mnps. |
| Stack `MoveList` vs `Vec` | Stack array kept: zero heap allocations in movegen. |
| Strict legal movegen vs pseudo-legal + check | Legal generation kept: lets perft bulk-count at the last ply. |
| Aspiration windows (fail-hard search, ±25 from depth 4 and ±50 from depth 6) | Fail-hard versions ~+4, undecided at 4000 games (branches `p2/aspiration`, `p2/aspiration-v2`). On fail-soft with a widening window and shallower fail-high re-search: +44 (2026-10-23, merged). |
| Late move pruning (move-count based) | Without "improving" −21 / −4 (not merged); with improving-scaled move counts +26 (2026-10-21, merged). |
| Futility pruning (static eval margin, fail-hard) | Undecided twice under a 4000-game cap (+10, +13); passed the 2026-10-19 retest (+10.2 ± 6.5, 6652 games) and merged. |
| Internal iterative reductions | Inconclusive at 15,000 games (+2.8 ± 4.4). Per docs/ROADMAP.md not rerun as-is; a depth ≥ 6 variant is on branch `q10c-iir` for a later, improved hypothesis. |
| TT buckets (4-entry depth/age replacement; 2-entry depth-preferred + always-replace) | Not merged: H0 both (−2.9, +1.9) at 8+0.08 with 16 MB Hash, where the single-slot table rarely fills. Code on `r12-tt`, `r12b-tt`. Retry with longer TC or smaller Hash, or once singular extensions need deep entries to survive. |
| Double / negative singular extensions | Not merged: bundle −13.4 (H0), double only −14.74 (H0), both on top of singular extensions + multicut with NNUE gen-0. Code on `r33-ext`, `r33b-ext`. Retry with a larger margin or after the history package changes move ordering. |
| Capture history (gravity table, bonus/malus on capture cutoffs, added to MVV-LVA) | Not merged: −5.7 (/32) and −5.1 (/8), H0 both times on top of SEE ordering. Code on `q5-caphist`, `q5b-caphist`. Retry using it in the SEE threshold / capture pruning rather than only ordering. |

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

**Protocol from 2026-10-21 (after the IIR test; integration plan "Epsylons Elo-Fahrplan", docs/ROADMAP.md):** bounds are in **normalized Elo (nElo)**, fastchess `model=normalized`, pentanomial game pairs, cap 25,000 games (`ROUNDS=12500`). Gainers: [0, 10] nElo (≈ [0, 5.8] logistic Elo). Enablers, simplifications and speed-ups: [−8, 0] nElo. Time management: 20+0.2. Stop rule unchanged: three features in a row that fail or are inconclusive stop the queue.

**Queue 2026-10-19:** inconclusive results (no decision at the game cap) are logged as "inconclusive" with the Elo estimate. Caps: 15,000 games for [0, 5], 20,000 for futility and the aspiration retest. **Change 2026-10-21:** the time-management test runs at 20+0.2 instead of 40+0.4. A short-lived 10,000-game cap was replaced the same day: [0, 5] tests use pentanomial statistics (game pairs, `-report penta=true`, which fastchess also defaults to) and cap at **25,000 games**; inconclusive at the cap is logged as before. The IIR test already running then keeps its 15,000-game cap because fastchess cannot change it mid-run.

| Change | Bounds | Result | Elo | Games |
|---|---|---|---|---|
| Futility pruning retest, depth ≤ 4, margin 80 + 80·depth (vs main with bench depth 11) | [0, 5] | H1 accepted | +10.2 ± 6.5 | 6652 |
| SEE: good/bad capture ordering, SEE pruning in qsearch and main search (vs futility) | [0, 10] | H1 accepted | +122 ± 35 | 290 |
| Capture history, ordering weight /32 (vs SEE) | [0, 5] | H0 accepted | −5.7 ± 6.2 | 7346 |
| Capture history, ordering weight /8, the one variation (vs SEE) | [0, 5] | H0 accepted | −5.1 ± 6.2 | 8192 |
| Counter move heuristic (vs SEE; capture history not merged) | [0, 5] | H1 accepted | +8.6 ± 5.8 | 8720 |
| Continuation history 1+2 ply, ordering weight 1 (vs counter move) | [0, 5] | inconclusive at 15,000-game cap (LLR 0.06) | +2.1 ± 4.5 | 15000 |
| Continuation history 1+2 ply, ordering weight ×2, the one variation (vs counter move) | [0, 5] | H1 accepted | +8.2 ± 5.6 | 9278 |
| "Improving" heuristic for RFP, LMR and futility margins (vs continuation history) | [0, 5] | H1 accepted | +22.4 ± 10.3 | 2796 |
| Late move pruning retest, (3 + depth²)/(2 − improving), depth ≤ 6 (vs improving) | [0, 5] | H1 accepted | +26.1 ± 11.3 | 2424 |
| Internal iterative reductions, depth ≥ 4 without TT move (vs LMP) | [0, 5] nElo, cap 15,000 (started before the 25k rule) | inconclusive (LLR 0.62) | +2.8 ± 4.4 | 15000 |
| NNUE infrastructure (Evaluator wrapper, `nnue` feature off; non-regression, vs LMP) | [−8, 0] nElo | H1 accepted | −0.45 ± 4.55 (nElo −0.6 ± 6.0) | 13106 |
| Draw hygiene: twofold repetition inside the search, no TT cutoffs at halfmove clock ≥ 90 (vs NNUE infrastructure) | [0, 10] nElo | H1 accepted | +25.2 ± 14.6 (nElo +32.7 ± 18.8) | 1310 |
| TT buckets: 4 entries per 64-byte line, replace shallowest with age counting 8 plies (vs draw hygiene) | [0, 10] nElo | H0 accepted | −2.9 ± 8.4 (nElo −3.6 ± 10.5) | 4184 |
| TT buckets: 2 entries, depth-preferred + always-replace, the one variation (vs draw hygiene) | [0, 10] nElo | H0 accepted | +1.9 ± 4.7 (nElo +2.4 ± 5.8) | 13692 |
| Pawn-structure correction history for the static eval (vs draw hygiene; TT buckets not merged) | [0, 10] nElo | H1 accepted | +20.6 ± 13.0 (nElo +26.7 ± 16.7) | 1654 |
| Fail-soft search: cutoffs, fail-lows, NMP and qsearch return the best score (enabler, vs pawn correction history) | [−8, 0] nElo | H1 accepted | +26.2 ± 15.5 (nElo +33.0 ± 19.5) | 1222 |
| Aspiration windows on fail-soft: ±20 from depth 4, widening by half, fail-high re-searched up to two plies shallower (vs fail-soft) | [0, 10] nElo | H1 accepted | +44.4 ± 20.9 (nElo +54.2 ± 25.2) | 732 |
| Time management: start iterations until the soft limit (was half of it), keep the best move of an aborted iteration, at 20+0.2 (vs aspiration) | [0, 10] nElo | H1 accepted, 0 time losses | +60.6 ± 23.3 (nElo +80.6 ± 30.3) | 504 |
| NNUE gen-0 (768→256)x2→1 SCReLU, trained on 87.3 M HCE self-play positions (950k games, 5000 nodes), 40 epochs, val loss 0.00652; `--features nnue` build with the net embedded (vs HCE main, time management) | [0, 10] nElo | H1 accepted, 0 time losses | +711 (nElo +1604 ± 55); 149.5/152, self-play exaggerates | 152 |
| Singular extensions: TT move searched one ply deeper when the rest fail below tt_score − 2·depth at half depth (depth ≥ 8, vs NNUE gen-0 main) | [0, 10] nElo | H1 accepted | +25.6 ± 14.3 (nElo +35.8 ± 19.9) | 1168 |
| Multicut: return singular_beta when the singular search without the TT move still reaches it and singular_beta ≥ beta (vs singular extensions) | [0, 10] nElo | H1 accepted | +7.3 ± 5.9 (nElo +10.6 ± 8.5) | 6398 |
| Double (≤ 6 per path, margin 20, non-PV) and negative (TT score ≥ beta) singular extensions, bundled (vs multicut) | [0, 10] nElo | H0 accepted | −13.4 ± 12.4 (nElo −19.1 ± 17.6) | 1500 |
| Double singular extensions only (≤ 6 per path, margin 20, non-PV), the one variation (vs multicut) | [0, 10] nElo | H0 accepted | −14.74 ± 12.85 (nElo −21.16 ± 18.41) | 1368 |
| Butterfly history with gravity (±16384) and malus for tried quiets, bonus min(16·depth², 1200) (vs multicut) | [0, 10] nElo | H1 accepted | +37.2 ± 17.1 (nElo +55.2 ± 25.2) | 732 |
| History tables kept across `go` within a game (butterfly and continuation halved, killers cleared; vs gravity + malus) | [0, 10] nElo | H1 accepted | +38.4 ± 17.8 (nElo +54.7 ± 25.1) | 736 |
| History pruning, depth ≤ 4, butterfly + 2·continuation < −2000·depth (vs history kept across go) | [0, 10] nElo | H0 accepted | −3.1 ± 7.5 (nElo −4.6 ± 11.2) | 3708 |

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

Merged 2026-10-22 (SPRT [-8, 0] nElo H1, no-op in the default build): inference, search integration behind the `nnue` feature, `quant datagen`, own trainer, netviz loader. Plan, architecture, file format and training steps: docs/NNUE.md.

**Gen-0 net (default since 2026-10-26): `nets/quant-gen0-74884169.nnue`.**

- **Training data provenance:** trained only on positions from Quant's own self-play, scored by Quant's own handcrafted-eval search and labelled with the results of those self-play games. No Stockfish, Leela or other external data, evaluations, nets or tablebases were used anywhere in data generation or training.
- **Data:** `quant datagen --out ~/tools/nnue-data/hce1 --threads 10 --games 950000 --nodes 5000 --seed 1`, run with the HCE build of `728d90f` (main after Phase 1). 950,000 games, 87,300,646 kept positions (~92 per game), filters and adjudication as in docs/NNUE.md. 2026-10-23 07:48 → 2026-10-24 02:08.
- **Architecture:** (768 → 256)x2 → 1, SCReLU, quantised as in docs/NNUE.md (QA/QB, bullet quantised.bin layout).
- **Training:** `scripts/nnue/train.py prepare` then `train --epochs 40 --threads 10 --in-memory` with the defaults of `9f4ee92`: batch 16384, AdamW lr 1e-3 with cosine decay to 1e-5, weight decay 0, wdl 0.25, sigmoid scale as in the script, seed 1, 200,000 positions held out for validation. Final validation loss 0.006517 (epoch 1: 0.007002). CPU only (PyTorch 2.14.1+cpu). Paused after epoch 22 and resumed from `epoch022.pt` after a reboot, so a bit-identical rerun needs the same resume point (the shuffle RNG restarts on resume).
- **Results:** SPRT vs HCE main [0, 10] nElo H1 (149.5/152, self-play exaggerates); gauntlet ~2875 vs SF 17 limited (table above).

- Decided 2026-10-22: install PyTorch (CPU build, ~200 MB, in a venv) for training. Install when NNUE gen-0 starts (ROADMAP Phase 2), not while an SPRT is running.
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
