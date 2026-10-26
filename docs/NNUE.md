# NNUE — Phase 3

Status (2026-10-21): inference, search integration, data generation, a trainer and the
visualizer are written and tested; no real net is trained yet. The handcrafted eval (HCE)
stays the default build. Everything here is Quant's own code; outside sources were only read
for concepts and for bullet's documented file layout (see "Sources").

## Architecture: (768 → 256)x2 → 1, SCReLU

```
 White's view: 768 inputs ─┐                        ┌─ side to move's accumulator (256)  ─┐
                           ├─ shared feature        │                                     ├─ SCReLU ─ 512 → 1 ─ × 400 = cp
 Black's view: 768 inputs ─┘  transformer 768 → 256 ┴─ other side's accumulator (256)     ─┘
```

- **Inputs:** one feature per (own/enemy, piece type, square) = 2 × 6 × 64 = 768, for each of
  the two perspectives. Black's view is mirrored vertically, so both sides see their own
  pieces moving "up" the board and the net is exactly colour-symmetric.
- **Feature transformer:** 768 → 256, shared by both perspectives (one accumulator each).
- **Output:** the side to move's accumulator first, the other side's second, SCReLU
  (clamp to [0, 1], then square), one linear layer 512 → 1. Output × 400 = centipawns.

Why this net first:

- It is the standard first step that reliably beats a handcrafted eval by a wide margin and
  trains on tens of millions of positions, which the machine can generate overnight.
- The perspective split knows whose move it is (tempo) and halves the parameters to learn.
- SCReLU gains over plain clipped ReLU at the same size for almost no cost.
- 256 neurons keep an incremental update to 2 × 512 bytes, cheap enough that the NNUE search
  runs at ~85–95 % of the HCE's nps (cloud Xeon, bench: ~2.0 M vs ~2.2 M nps).
- Later steps (each its own SPRT): hidden 512–1024, output buckets by piece count, king-input
  buckets with horizontal mirroring, more data from NNUE self-play.

### Feature index

For a piece of colour `c` and type `t` (P N B R Q K = 0..5) on square `s` (A1 = 0, H8 = 63):

```
White's view:  (c == White ? 0 : 1) * 384 + t * 64 + s
Black's view:  (c == Black ? 0 : 1) * 384 + t * 64 + (s ^ 56)
```

This is the "Chess768" layout bullet uses with side-to-move-relative data, so nets from
bullet and from `scripts/nnue/train.py` are interchangeable.

### Quantised inference

| Tensor | Shape | Stored as |
|---|---|---|
| feature weights | 768 × 256 | i16, × QA = 255 |
| feature bias | 256 | i16, × QA |
| output weights | 2 × 256 (side to move, then other side) | i16, × QB = 64 |
| output bias | 1 | i16, × QA · QB |

```
acc[p]  = bias + Σ weights[feature]                    (i16, per perspective p)
c       = clamp(acc, 0, QA)
sum     = Σ c_us² · w_us + Σ c_them² · w_them            (i64; units QA² · QB)
cp      = ((sum / QA) + output_bias) · 400 / (QA · QB)   (Rust integer division, truncating)
cp      = clamp(cp, -20000, 20000)                       (clear of mate scores)
```

The AVX2 path computes `(c · w) · c` with the first product in i16, which needs every
|output weight| ≤ 128 (float ≤ 2.0). Train with weight clipping at ±1.98 (both trainers do);
a net outside that range still loads and uses the scalar path.

AVX2 is chosen at run time (`is_x86_feature_detected`), because the release build targets
plain x86-64; the reference machine has AVX2. No AVX-512 path (the target has none).

## Net file format

No header. Little-endian i16, in this order, then zero padding to a multiple of 64 bytes:

```
feature weights  768 × 256   feature-major: weight[f * 256 + h]
feature bias     256
output weights   512         side to move's 256, then the other side's 256
output bias      1
```

Size: 2 × (768·256 + 256 + 512 + 1) = 394,754 bytes, padded 394,816. The loader accepts both
sizes and rejects anything else.

This is bullet's `quantised.bin` for this net when saved with (bullet stores each `M × N`
weight column-major, so the `256 × 768` feature weights come out feature-major):

| bullet weight | SavedFormat |
|---|---|
| `l0w` | round, quantise i16 × 255 |
| `l0b` | round, quantise i16 × 255 |
| `l1w` | round, quantise i16 × 64 (stm half first, as the net concatenates stm then ntm) |
| `l1b` | round, quantise i16 × 16320 |

Check for the first bullet net: `eval` in the engine must equal
`python3 scripts/nnue/train.py eval --net <net> --fen <fen>` (both read the same bytes; this
checks the engine), the start position must be a small positive number, and a colour-flipped
position must give the same eval. If the start position is far off, the stm/ntm halves of
`l1w` are swapped or a weight was saved transposed.

## Engine integration

- `src/nnue/`: network, loader, lazy accumulator stack (`NnueState`), SIMD kernels. Always
  compiled and unit-tested (`cargo test`), but only used by the search in `nnue` builds.
  `nnue` is a default feature since 2026-10-26 (gen-0 net); AVX2 is picked at run time and
  every kernel has a scalar version (`QUANT_NO_AVX2=1` forces it, `make test-scalar`).
- `src/search/evaluator.rs`: every make/unmake and static eval in the search goes through
  `Evaluator`. In `--no-default-features` builds it is the HCE and plain make/unmake. With the
  `nnue` feature (default) and an active net it pushes the move's feature changes
  before `make_move`, pops after `unmake_move`, and updates accumulators only when a
  position is evaluated (nodes pruned before their static eval pay nothing). Null moves need
  no update. **Rule for new search code: use `ctx.eval.make_move/unmake_move/evaluate`,
  never `board.make_move` or `evaluate` directly.**
- Build: `cargo build --release` embeds the net named in `build.rs` (`DEFAULT_NET`);
  `EVALFILE=<path> cargo build --release` embeds another one, `make release-hce` builds
  without NNUE. At run time `setoption name EvalFile value <path>` loads another net.
- Nets live in `nets/` as `quant-gen<N>-<first 8 hex of sha256>.nnue`; CHANGELOG.md says
  which net each build embeds. At ~400 KB a net is a normal commit; once nets grow past a
  few MB they go to GitHub Releases or Git LFS instead.
- UCI (nnue builds): `EvalFile` (default `<embedded>`), `UseNNUE` (default true).
  `eval` prints the HCE eval and the NNUE eval of the current position.
- Tests: `cargo test` runs everything with the feature on, `make test-hce` without it; the NNUE unit tests check the
  file round trip, the forward pass against an independent reference, colour symmetry,
  incremental updates against full refreshes over random games (castling, en passant,
  promotions), and AVX2 against scalar.

## Data generation

```
quant datagen --out data/hce1 --threads 10 --games 600000 --nodes 5000 --seed 1
```

- 8–9 random plies from the start position; openings whose first search is beyond ±1000 cp
  are dropped. Each move is searched to 5000 nodes (soft limit: no new iteration after it;
  hard limit 10×).
- Adjudication: win when |score| ≥ 2000 for the same side over 6 plies; draw after ply 80
  when |score| ≤ 10 for 12 plies; draws by 50 moves, repetition, insufficient material,
  400-ply cap.
- Kept positions: not in check, best move not a capture or promotion, no mate score.
- One file per thread, `data_s<seed>_t<id>.txt`, lines `<FEN> | <cp> | <result>`, White's
  view (bullet's text format). Use a different `--seed` for every run.
- Throughput: ~105 kept positions per game; ~195 positions/s per thread at 5000 nodes on the
  cloud Xeon (2.1 GHz). Expect ~2000/s on 10 threads, so ~7 M positions per hour.
- With an `nnue` build and a net (`--evalfile`, or embedded), the games use the net: that is
  the reinforcement loop below.

Volume: first net from 50–100 M HCE positions (7–14 h with 10 threads). Later nets: 200 M+
from the previous net.

## Training

### A. Quant's trainer (`scripts/nnue/train.py`, CPU or CUDA)

Needs numpy and torch. Without an NVIDIA GPU install the CPU build in a venv
(`pip install torch --index-url https://download.pytorch.org/whl/cpu`, ~200 MB); the plain
`pip install torch` pulls ~3 GB of CUDA libraries. Steps:

```
python3 scripts/nnue/train.py prepare data/hce1/*.txt --out data/hce1-packed
python3 scripts/nnue/train.py train --data data/hce1-packed --out nets/hce1 --epochs 40 --threads 10 --in-memory
python3 scripts/nnue/train.py eval --net nets/hce1/epoch040.nnue --fen "<fen>"
```

- Packed data: 70 bytes per position (100 M positions = 7 GB; `--in-memory` needs that much
  RAM, the machine has 32 GB).
- Loss: mean squared error between sigmoid(output) and
  `wdl · result + (1 − wdl) · sigmoid(cp / 400)`, side to move's view, `wdl` 0.25.
- AdamW, lr 1e-3 with cosine decay to 1e-5 over the epochs, batch 16384, weights clipped
  to ±1.98 after every step. 0.5 % (max 200k positions) held out for a validation loss.
- Every epoch writes `epochNNN.pt` (resume with `--resume`) and `epochNNN.nnue`.
- Speed: ~80k positions/s on 4 cloud threads (2.1 GHz Xeon), so expect ~150k/s on 10
  threads: one epoch over 60 M positions ≈ 7 min, 40 epochs ≈ 4–5 h.

Cloud end-to-end test (2026-10-21), only to prove the pipeline: 5000 HCE games at 2000 nodes
→ 482k positions → 15 epochs (90 s) → net loaded by the engine. Results with that net
(side to move's view, HCE in brackets): start position +14 (0), White a queen up +1186
(+948), same with Black to move −1166, a colour-flipped Italian pair +30 / +30, K+P vs K
+141 (+110). Engine `eval` and `train.py eval` agreed on every position. NNUE bench signature
with that net 1099589. Far too little data for real strength: the first real net needs the
50–100 M positions above.

### B. bullet (GPU only)

bullet needs CUDA, ROCm or Metal. If the machine has a supported GPU, it trains much faster:

1. Convert: `bullet-utils convert` from the text files to bulletformat, then shuffle and
   interleave with `bullet-utils`.
2. Network: Chess768 inputs, perspective, 256 hidden, SCReLU, 1 output; eval scale 400,
   wdl 0.25, weight clipping 1.98, AdamW, lr 1e-3 cosine to 1e-5, batch 16384, the
   SavedFormat in the table above. Write this trainer config ourselves (no copied example).
3. Check the first net as described under "Net file format".

## Testing a net (after the SPRT queue)

1. `make test`, `make test-nnue`, perft, `make bench` (HCE build: 888666).
2. Sanity: `eval` on a few positions vs `train.py eval`; start position small positive;
   a queen up is clearly positive; NNUE build `bench` runs.
3. SPRT NNUE build vs current main (HCE), bounds [0, 10]. Expect a large gain.
4. Then retune search margins that depend on the eval scale (RFP, futility, SEE pruning,
   delta) with SPRT, since NNUE scores spread differently from the HCE.

## Reinforcement loop

1. Train net N on the data so far.
2. SPRT net N vs net N−1 (or the HCE for N = 1); keep it only if it passes.
3. Generate new data with net N (`quant datagen --evalfile nets/N.nnue`), add it to the
   pool (drop the oldest HCE data once there is enough net data), retrain.

## Sources

Concepts and file layout only (no code taken):

- bullet documentation: "NNUE Basics", "Training Data" and "Saved Networks" pages in
  github.com/jw1912/bullet/tree/main/docs (perspective net, Chess768 layout, text data
  format `<FEN> | <score> | <result>`, quantised.bin layout and padding).
- General NNUE ideas (efficiently updatable accumulators, SCReLU) as described in public
  chess-programming write-ups.
