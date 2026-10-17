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

- **Elo estimate:** N/A (Phase 1 has no real search).
- **Bench signature:** 3251662
- **Perft 6 startpos:** see CHANGELOG.md for nps (bulk and non-bulk).
- **Roadmap:** Phase 1 (Foundation) complete. Next: Phase 2 (Search).

## Ideas tried

| Idea | Result |
|---|---|
| Fancy magics vs PEXT for sliders | Magics chosen: PEXT is microcoded (~18 cycles) on Zen+, magics use one `imul`. Bulk perft reaches ~140 Mnps. |
| Stack `MoveList` vs `Vec` | Stack array kept: zero heap allocations in movegen. |
| Strict legal movegen vs pseudo-legal + check | Legal generation kept: lets perft bulk-count at the last ply. |

## SPRT log

No SPRT runs yet. Use `scripts/run_sprt.sh` and log every result here, including failures.

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
