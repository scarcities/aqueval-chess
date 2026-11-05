# TASKS.md — aqueval chess

Labels: `[LEAD]` = correctness/performance-critical (board, movegen, make/unmake, search, TT, NNUE, SIMD).
`[DELEGATE]` = scripts, tests, CI, docs, data generation, refactors; each has a self-contained spec.
Status: `todo`, `in progress`, `review`. Finished tasks are removed; CHANGELOG.md and git history keep them.

## Phase 2 — Search

| ID | Task | Label | Status |
|---|---|---|---|
| P2-01 | Only set the EP square/hash key when an enemy pawn can actually capture | [LEAD] | todo |
| P2-02 | `unmake_move` pops its `UndoState` from history instead of taking it as a parameter | [DELEGATE] | todo |
| P2-03 | Real ponder: on `ponderhit` switch to normal time control instead of moving at once | [LEAD] | todo |
| P2-04 | After v1.0: when the best move comes from an aborted iteration, print a matching `info ... pv` line before `bestmove` (fastchess warned 151 times in the v1.0 gauntlet that `bestmove` did not start the last PV) | [LEAD] | todo |

## Phase 3 — NNUE (see docs/NNUE.md)

| ID | Task | Label | Status |
|---|---|---|---|
| P3-08 | Remove the handcrafted eval (`--no-default-features` build, `UseNNUE` option, HCE code and tests) once the second net (gen-1) passes its SPRT against gen-0; keeping two evals costs maintenance on every search change | [LEAD] | todo |
| P3-07 | Retune eval-scale-dependent search margins for NNUE | [LEAD] | todo |

### P2-02 spec (delegate)

- **Goal:** one source of truth for undo data. `Board::make_move` already pushes the `UndoState` onto `self.history`; `unmake_move(m, &undo)` pops it again but uses the parameter.
- **Change:** make `unmake_move(&mut self, m: Move)` pop the `UndoState` from `self.history`; drop the returned value from `make_move` if no caller needs it. Update every caller (`src/perft`, `src/search`, `src/uci`, all of `tests/`).
- **Must hold:** `make test` green, bench signature unchanged, perft 6 bulk nps not lower than before (`make perft`).
- **Files:** `src/board/make_move.rs` and callers only. Branch `task/P2-02`.
