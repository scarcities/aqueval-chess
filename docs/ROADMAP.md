# Roadmap for aqueval chess: integrating researched ideas

As of 2026-10-21, checked against `main` 32084f3. Based on a research report on Elo gains measured in other engines (not part of this repository) and a read-only audit of the code. All ideas are described in our own words; no code from other engines is used.

**Summary.** First finish the running queue and fix three problems in the foundation that the audit found: the TT overwrites on every collision, repetitions only count as a draw inside the search on the third occurrence, and the search is fail-hard throughout. Then comes our own NNUE (generation 0), because it brings by far the most Elo per machine hour: roughly +150 to +300 Elo for about a day and a half of machine time, against about +5 Elo per search test. The large eval-independent search techniques (singular extensions, history pruning, the history package, a new capture history, ProbCut, the correction-history ladder) follow on top of the NNUE, so their margins sit on the right eval scale from the start. SPSA tuning comes after that. The test protocol switches to **[0, 10] nElo with a 25,000-game cap**; that is about [0, 5.8] in logistic Elo and almost twice as many tests per day as [0, 5] nElo, with hardly less Elo collected.

**Reading guide.** Elo expectations are marked: **M** = measured in another engine (source in the report), **S** = our own estimate for aqueval chess. "nElo" is normalized Elo, the unit in which fastchess and `scripts/run_sprt.sh` (`model=normalized`) compute the bounds. For aqueval chess at STC with the UHO book, 1 Elo ≈ 1.73 nElo. Line references refer to `main` 32084f3.

## Test protocol from the test after IIR

| Kind of change | Bounds | Cap | Time control | Mean duration (S) |
|---|---|---|---|---|
| Gainer (default for all features) | [0, 10] nElo ≈ [0, 5.8] Elo | 25,000 games (`ROUNDS=12500`) | 8+0.08 | ~3.7 h |
| Enabler, simplification, pure speed-up | [−8, 0] nElo ≈ [−4.6, 0] Elo | 25,000 | 8+0.08 | ~5.8 h |
| Small refinement (expected +1 to +3) | bundle, or put into the SPSA block; alone [0, 8] nElo | 25,000 | 8+0.08 | ~4.8 h |
| Time management | [0, 10] nElo | 25,000 | 20+0.2 | ~9 h |
| NNUE build vs HCE main | [0, 10] nElo | 25,000 | 8+0.08 | < 1 h for a large gain |
| Net vs net (from gen-1) | [0, 10] nElo | 25,000 | 8+0.08 | ~3.7 h |
| LTC check (no SPRT) | fixed match, 2,000 games | – | 40+0.4 | ~5 h, after each phase |

**Why [0, 10] nElo instead of [0, 5] nElo.** Our own simulation (GSPRT, α = β = 0.05, pentanomial, draw rate ~50 %, 1,700 games/h) over 30 roadmap ideas: [0, 5] nElo with a 25k cap collects 92 % of the available Elo at 7.1 h per test, [0, 10] nElo with a 25k cap 88 % at 3.7 h. At true +3 / +4 / +5 / +6 Elo, [0, 10] nElo accepts 50 / 71 / 88 / 96 % of the time; a patch worth a true 0 Elo passes 4–5 % of the time, one worth −3 Elo under 0.5 %. The earlier "[0, 5]" was really about [0, 2.9] Elo because of the nElo unit, which is why it was so slow. [−3, 1] is unsuitable as a non-regression test: in nElo a true 0-Elo simplification passes only 21 % of the time.

**Still valid:** one SPRT at a time; every test against the current `main`; `cargo test --release`, perft and bench before every SPRT; stop after three failures or inconclusive results in a row (then pause and report). Concurrency stays at 10; both sides share the SMT cores equally. For the time-management tests, count time losses in the PGNs before and after: the target is zero. In AGENTS.md, write every bound with its unit ("[0, 10] nElo").

**Running IIR test:** stays as started ([0, 5] nElo, 15,000-game cap) and is logged that way. If it ends at the cap with positive Elo, test an improved variant later as a new hypothesis instead of restarting the same patch.

## Audit: the eight checks on the actual code

| Check | Result | Location | Consequence |
|---|---|---|---|
| Re-search order after LMR | fine: first full depth with a null window, then the full window only for alpha < score < beta | `src/search/mod.rs:402-411` | none |
| Return value on TT cutoffs | fine: the TT value is returned, cutoffs only in null windows. The rest of the search is fail-hard, though (`return beta` in NMP, beta cutoff and QS; the TT stores beta instead of the real value) | `mod.rs:234-247`; fail-hard: `mod.rs:286`, `:372-373`, `:441`, `:462-463` | step 1.4 fail-soft |
| 50-move rule on TT cutoffs | missing: no guard at a high halfmove clock | `mod.rs:228`, `:234` | include in step 1.1 |
| Move Overhead, movestogo, hard limit | fine: overhead is subtracted, movestogo parsed, hard limit at most 80 % of the remaining time. But a new iteration only starts while less than half of the soft limit is used, so aqueval chess uses much less time than it is given | `src/uci/mod.rs:184-192`, `src/search/time.rs:50-60`, `mod.rs:632-635` | step 1.6 time management |
| LMP counter | fine: counts by list index, so pruned moves and captures are included | `mod.rs:321` | none |
| Quiet checks in QS | none. But QS also stands pat on the static eval when in check and skips quiet promotions | `mod.rs:439-443`, `:450` | QS package in Phase 3 |
| Stack sentinels | fine: all look-backs use `checked_sub`/`Option`, ply bound checked before the tables | `mod.rs:143-152`, `:223`, `:255-260` | keep the pattern for conthist plies 4/6 |
| Bench signature and hardware | bench 888666 (AGENTS.md); the reference machine has 6 cores with SMT, so concurrency 10 shares cores | AGENTS.md, `scripts/run_sprt.sh` | concurrency stays; check time losses |

### Further audit findings

| Finding | Location | Consequence |
|---|---|---|
| **TT overwrites on every collision.** One slot per index; a different key always replaces it, even a much deeper entry | `src/search/tt.rs:30`, `:74` | step 1.2 TT buckets, prerequisite for singular extensions |
| **Repetition only counts threefold inside the search.** A single repetition inside the search tree is not scored as a draw | `src/board/mod.rs:151-172` (count ≥ 3), called at `mod.rs:228` | step 1.1 draw hygiene |
| **Butterfly history without malus and without gravity.** Only a depth² bonus, capped at 700,000; frequent moves saturate and lose the distinction. All heuristic tables are created anew on every `go` | `mod.rs:116-124`, `:509` | history package in Phase 3 |
| QS generates all legal moves and then filters captures; no TT in QS | `mod.rs:446-450` | QS package in Phase 3 |
| Ordering computes SEE for every capture and sorts the whole list up front | `mod.rs:180`, `:304-311` | later a staged move picker (speed-up) |
| The EP square is hashed after every double push, even when no EP capture is possible (TASKS P2-01 open) | `src/board/make_move.rs:105-112` | hygiene bundle in Phase 3 |
| The root discards an aborted iteration entirely; info shows only one PV move | `mod.rs:598-604`, `:618` | check in step 1.6; PV output is cosmetic |
| Threads at most 1 | `src/uci/options.rs:59` | only before a CCRL submission |

## Order

### Phase 0: right away, no machine time (while IIR runs)

- `scripts/run_sprt.sh`: gainer default to [0, 10] nElo, simplification to [−8, 0] nElo, `ROUNDS=12500` as default (25,000 cap). `model=normalized` stays explicit. AGENTS.md: adopt the test protocol above, nElo as the unit everywhere.
- Count time losses in the existing SPRT PGNs.
- Prepare branches for 1.1 to 1.4 in the worktree (own implementation, no outside code).

### Phase 1: finish the queue, repair the foundation (~2 days of machine time)

| Step | Content | Test | Expectation | Why here |
|---|---|---|---|---|
| 1.0 | Merge the NNUE infrastructure (patches 0001–0005 from `nnue-prep`, `git am -3`) | no SPRT if the default build keeps the bench signature and NPS (8 runs per side, machine otherwise idle) is within 1 %; otherwise [−8, 0] nElo | 0 Elo, no behaviour change | Otherwise every later search patch has to be adapted to the Evaluator interface again; merging once, early, avoids conflicts |
| 1.1 | Draw hygiene: one repetition after the root counts as a draw, threefold only through the game history; no TT cutoffs from about 90 halfmoves | [0, 10] nElo | S: +3 to +10 | Foundation bug; changes every tree, so before all further tests |
| 1.2 | TT buckets: several entries per cache line, replacement by depth and age, deep entries survive | [0, 10] nElo | M: Weiss (HCE) +17.0 / +6.6 for a new replacement scheme. S: +10 to +25, because today every collision overwrites | Prerequisite for singular extensions; helps all following tests |
| 1.3 | Pawn correction history (queue step 11) | [0, 10] nElo | M: Weiss (HCE) +4.9 / +11.7 | as planned |
| 1.4 | Fail-soft: beta cutoffs, NMP and QS return the real value, and the TT stores it | [−8, 0] nElo (enabler) | S: 0 to +5 | AGENTS.md: aspiration "worth retrying after a fail-soft search" |
| 1.5 | Aspiration retest on fail-soft (queue step 13): the window widens on a fail, re-search shallower after a fail-high | [0, 10] nElo | M: Weiss (HCE) +14.1 / +8.1, Ethereal (HCE) +10.6 / +4.8 for refinements of an existing window. S: +4 to +10 | needs 1.4 |
| 1.6 | Time management (queue step 12) at 20+0.2: really use the soft limit instead of stopping after half of it; use an aborted iteration once its first move is done | [0, 10] nElo, 20+0.2 | S: +10 to +30, because a lot of time is left unused today | basis for the TM additions in Phase 3 |
| 1.7 | Gauntlet "HCE final" | as before (`scripts/elo_gauntlet.sh`) | – | milestone before the NNUE |

### Phase 2: NNUE generation 0 (~1.5 days of machine time, no SPRT in parallel)

| Step | Content | Duration (S) |
|---|---|---|
| 2.1 | Datagen with `main` after Phase 1: 100 M positions, 5,000 nodes per move, random opening moves, filters as in `docs/NNUE.md` | ~14 h with 10 threads (~7 M/h) |
| 2.2 | Training with aqueval chess's own trainer (`scripts/nnue/train.py`, CPU PyTorch in a venv, ~200 MB download) or, if a CUDA/ROCm GPU is available, with bullet as a library in our own training program | ~6–8 h CPU for 40 epochs |
| 2.3 | Sanity checks from `docs/NNUE.md`, normalise the eval scale to centipawns, then SPRT the NNUE build against HCE `main` | [0, 10] nElo; M: akimbo +388, Stormphrax +358 (against the author's HCE engine), Leorik ~+305. S: +150 to +300 |
| 2.4 | On success: embed the net, NNUE as default, gauntlet "NNUE gen-0" | ~1 h |

Only self-generated data, no outside net and no outside datasets. This keeps aqueval chess independent and meets the TCEC NNUE guideline.

### Phase 3: search structure on top of the NNUE (~3–4 days)

All steps [0, 10] nElo unless stated otherwise. Ordered by expectation, dependencies and effort.

| Step | Content | Expectation |
|---|---|---|
| 3.1 | Singular extensions (basic, excluded-move search, per-path limit) | M: Weiss (HCE) +11.5 / +24.2 |
| 3.2 | Multicut from the singular search | M: Weiss +4.8 / +3.2 |
| 3.3 | Double and negative extensions (bundled) | M: Weiss double +11.5 / +11.2, negative +2.7 / +3.1 |
| 3.4 | History package: gravity and malus for the butterfly history, keep tables across moves (weaken instead of clearing on a new `go`) | S: +5 to +15 (audit finding) |
| 3.5 | History pruning for quiet moves with very bad history at low depth | M: Weiss (HCE) +9.1 / +10.5 |
| 3.6 | Capture history again, as a package: SEE only for good/bad, within the groups victim value plus capture history, malus for all captures tried, capture history shifts the SEE threshold | S: +3 to +8; the two failures were probably due to the design |
| 3.7 | LMR package: reduction by history, less in PV/ttPv nodes, more at cut nodes (cut-node flag), deeper/shallower re-search | M: Weiss +2.3 / +5.9, Viridithas +5.9. S: +5 to +10 in total |
| 3.8 | QS package: read and write the TT in QS, answer checks in QS with evasions; generate captures only (speed-up, [−8, 0] nElo) | M: Weiss +5.4 / +6.9 for TT in QS |
| 3.9 | ProbCut with a QS pre-check | M: Weiss (HCE) +6.4 / +6.7 |
| 3.10 | Correction-history ladder: non-pawn, minor/major, continuation (one test each) | M: Weiss non-pawn +2.8 / +6.8; StockDory continuation +3.6 / +5.5 |
| 3.11 | Cuckoo detection of upcoming repetitions | M: Weiss +3.6 / +5.4 |
| 3.12 | Hindsight reductions (Stockfish 2025, bundled) | M: SF ≈ +1 to +2 each |
| 3.13 | TM additions at 20+0.2: best-move stability, then node fraction | M: StockDory +6.9 / +9.1; rusty-rival +4.5 |
| 3.14 | Hygiene bundle: EP hash only when an EP capture is possible, mate distance pruning, NMP verification at high depth | [−8, 0] nElo |

### Phase 4: tuning (~2 days)

- SPSA over all search constants on top of the NNUE, 30,000–60,000 games. fastchess has no SPSA; for that, a small driver script of our own following the standard method (OpenBench defaults as starting values: α 0.602, γ 0.101, A = 10 % of the iterations). Then test the result against the pre-tune version ([0, 10] nElo) and confirm it with an LTC check.
- Only here, because the margins of RFP, futility, ProbCut and razoring depend on the eval scale and have to be reset after the NNUE anyway.

### Phase 5: NNUE generations (ongoing, overnight)

Gen-1 data with the NNUE engine (200 M+), then a wider first layer (512, later 1024), king buckets with an accumulator cache, output buckets by material, AVX2 lazy updates. Each step net vs net, [0, 10] nElo. Threat inputs only for large nets.

### Phase 6: our own research line (on the side)

Logging infrastructure for node data, then margins and LMR fitted from our own data, uncertainty-scaled margins using the magnitude of the correction history, a Gini signal for time management, and a comparison of two data lines (started from HCE vs started from a random net). All with the same test protocol.

### Deliberately not

MCTS or hybrid search, policy nets for move ordering, search-free transformers, Texel tuning of the HCE (replaced by the NNUE), Lazy SMP before a CCRL submission, outside nets or training data.

## Why this order

1. **Elo per machine hour.** The machine is the bottleneck. A search test brings about +4 Elo on average in ~3.7 h; NNUE gen-0 brings +150 to +300 Elo in ~1.5 days, going by other engines' measurements. That is why the NNUE comes before the big search wave and not at the end.
2. **Foundation first.** Repetitions, TT replacement and fail-soft change every search tree. While they are wrong, every further test measures against a distorted main. The TT also has to be right before singular extensions, because they need deep lower-bound entries.
3. **Dependencies.** Fail-soft before aspiration (note in AGENTS.md); TT before singular extensions; cut-node flag before the LMR package; time-management basis before the TM additions.
4. **Eval scale.** Everything that depends on margins, and the SPSA tuning, comes after the NNUE. Eval-independent structure (extensions, TT, history, repetitions) carries over unchanged.
5. **Merge the NNUE infrastructure early.** The default build does not change (same bench signature). Merging it only after ten more search patches would mean adapting each of them to the Evaluator interface.
6. **Datagen only after Phase 1.** Better draw and TT handling improve the labels and game results of the training data; the delay is only about two days.

## Checking the report

Two independent passes checked about 200 claims in the report against their sources: about 165 confirmed, about 30 corrected, about 25 not checkable (marked), none false. No correction changes the order of the top 10.

- **Confirmed** are all numbers the ranking rests on: Weiss singular extensions +11.52 / +24.21, TT replacement +16.98 / +6.63, TT in QS +5.41 / +6.89, history pruning +9.05 / +10.50, ProbCut +6.36 / +6.65, pawn corrhist +4.87 / +11.70; StockDory time management +6.85 / +9.10; Viridithas +29.59 and +14.41; akimbo +387.57; Stormphrax +357.65; the TCEC NNUE guideline; bullet without a CPU backend.
- **Corrected:** six Ethereal pairs with STC/LTC swapped or numbers from neighbouring commits (among them the ProbCut pre-check +2.61 / +6.10 and the LMP counter +4.79 / +6.31); Stockfish optimism (STC was rejected, not "undecided"); Tetrarch's +63.4 applies to a whole release; threat inputs came with v19 in Viridithas; Leorik's +355 includes ~50 Elo of non-NNUE changes; Stormphrax's +358 was measured against the author's earlier HCE engine.
- **SPRT unit:** the probabilities in the report apply to logistic bounds. aqueval chess tests in nElo, which led to the new protocol above. The mean test lengths in the report were ~11 % too high (Wald's formula instead of B/μ).

The corrected report and both review logs are not part of this repository.
