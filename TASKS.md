# TASKS.md — Quant

Labels: `[LEAD]` = correctness/performance-critical (board, movegen, make/unmake, search, TT, NNUE, SIMD).
`[DELEGATE]` = scripts, tests, CI, docs, data generation, refactors; each has a self-contained spec.
Status: `todo`, `in progress`, `review`, `done`.

## Phase 1 — Foundation

| ID | Task | Label | Status |
|---|---|---|---|
| P1-01 | Board, FEN, Zobrist, magics, legal movegen, make/unmake (M1) | [LEAD] | done |
| P1-02 | Perft bulk/non-bulk, divide, standard positions (M2) | [LEAD] | done |
| P1-03 | UCI loop, `go`/`stop`/`ponder`, time management, basic search (M3) | [LEAD] | done |
| P1-04 | Bench command and signature regression test (M4) | [LEAD] | done |
| P1-05 | FEN validation: kings, pawns on rank 1/8, EP square, castling rights | [LEAD] | done |
| P1-06 | 10k shakmaty cross-check, SPRT script, Makefile (M5) | [DELEGATE] | done |
| P1-07 | AGENTS.md, TASKS.md, CHANGELOG.md, GitHub Actions CI (M6) | [DELEGATE] | done |

## Phase 2 — Search (next)

| ID | Task | Label | Status |
|---|---|---|---|
| P2-01 | Only set the EP square/hash key when an enemy pawn can actually capture | [LEAD] | todo |
| P2-02 | `unmake_move` pops its `UndoState` from history instead of taking it as a parameter | [DELEGATE] | todo |
| P2-03 | Real ponder: on `ponderhit` switch to normal time control instead of moving at once | [LEAD] | todo |
| P2-04 | PVS + transposition table + quiescence (start of the Phase 2 roadmap in AGENTS.md) | [LEAD] | todo |
| P2-05 | FEN validation: reject positions where the side not to move is in check (today `go` panics at `src/movegen/attacks.rs:193`) | [LEAD] | done |
| P2-06 | UCI: print `bestmove 0000` instead of `a1a1` when the side to move has no legal move | [DELEGATE] | done |

## Phase 3 — NNUE (prepared, see docs/NNUE.md)

| ID | Task | Label | Status |
|---|---|---|---|
| P3-01 | NNUE inference: (768 → 256)x2 → 1 SCReLU, loader, lazy accumulators, AVX2 | [LEAD] | review |
| P3-02 | Search integration behind `--features nnue`, UCI `EvalFile`/`UseNNUE`, `eval` command | [LEAD] | review |
| P3-03 | `quant datagen` self-play data generation | [DELEGATE] | review |
| P3-04 | Own trainer `scripts/nnue/train.py`; netviz reads the net format | [DELEGATE] | review |
| P3-05 | Generate 50–100 M HCE positions (after the SPRT queue) | [DELEGATE] | todo |
| P3-06 | Train the first net, sanity checks, SPRT vs HCE main [0, 10] | [LEAD] | todo |
| P3-07 | Retune eval-scale-dependent search margins for NNUE | [LEAD] | todo |

### P2-02 spec (delegate)

- **Goal:** one source of truth for undo data. `Board::make_move` already pushes the `UndoState` onto `self.history`; `unmake_move(m, &undo)` pops it again but uses the parameter.
- **Change:** make `unmake_move(&mut self, m: Move)` pop the `UndoState` from `self.history`; drop the returned value from `make_move` if no caller needs it. Update every caller (`src/perft`, `src/search`, `src/uci`, all of `tests/`).
- **Must hold:** `make test` green, bench signature unchanged (782700), perft 6 bulk nps not lower than before (`make perft`).
- **Files:** `src/board/make_move.rs` and callers only. Branch `task/P2-02`.
