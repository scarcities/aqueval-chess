# Quant

A UCI-compatible chess engine written in Rust (edition 2021). Quant combines legal bitboard move generation, modern game-tree search, and NNUE evaluation. Built for local play, benchmarking, and engine development.

## Layout

```text
src/board/             board state, FEN, make/unmake, Zobrist hashing
src/movegen/           legal move generation and magic bitboard attacks
src/search/            search, evaluation, move ordering, time management
src/nnue/              network loading, accumulators, AVX2/scalar inference
src/uci/               UCI protocol and engine options
src/perft/             move-generation validation and node counting
src/bench/             reproducible engine benchmarks
src/datagen/           self-play training data generation
nets/                  bundled NNUE network
scripts/nnue/          Python NNUE training tools
harness/               black-box UCI/perft validation harness
tests/                 integration and regression tests
docs/                  platform notes, NNUE details, roadmap
```

## Quickstart

Requirements: a current stable Rust toolchain with Cargo.

```bash
cargo build --release
cargo test --release
./target/release/quant
```

The executable starts in **UCI mode**. In its terminal session, try:

```text
uci
isready
position startpos
go depth 8
quit
```

You can also connect `target/release/quant` to a UCI-compatible chess GUI.

## Commands

```bash
./target/release/quant bench            # deterministic search benchmark
./target/release/quant perft 6          # perft from the initial position
./target/release/quant divide 4         # per-move perft breakdown
./target/release/quant perft 4 "<FEN>"  # perft from a custom position
```

## Engine

- **Move generation:** bitboards, magic sliding-piece attacks, pins, checks, castling, promotions, and en passant.
- **Search:** alpha-beta/PVS-based search with a transposition table, move ordering, pruning, and time management.
- **Evaluation:** embedded NNUE network by default; handcrafted evaluation remains available.
- **UCI:** supports standard engine communication, position setup, search commands, and runtime options.
- **Validation:** Rust regression tests, perft suites, and an independent UCI test harness.

## NNUE

The default release build embeds the network from `nets/`. To build with another network, set `EVALFILE` at build time:

```bash
EVALFILE=/path/to/network.nnue cargo build --release
```

To build with handcrafted evaluation only:

```bash
cargo build --release --no-default-features
```

On CPUs without AVX2, NNUE uses scalar inference. Set `QUANT_NO_AVX2=1` to force that path when testing. UCI exposes `EvalFile` and `UseNNUE` in NNUE-enabled builds; `Threads` is currently limited to one search thread.

## Testing

```bash
cargo test --release
cargo test --release --no-default-features
QUANT_NO_AVX2=1 cargo test --release
cargo clippy --all-targets -- -D warnings
python3 harness/perft.py --engine target/release/quant
```

The perft harness uses Python's standard library and tests the compiled engine through UCI. The NNUE training tools in `scripts/nnue/` have separate Python dependencies; see the NNUE documentation.

## Documentation

```text
PROJECT.md             architecture and feature inventory
TASKS.md               implementation status and open tasks
CHANGELOG.md           changes and benchmark history
docs/NNUE.md            network format, inference, training
docs/MACOS.md           macOS / Apple Silicon notes
docs/ROADMAP.md         development roadmap
harness/README.md      perft harness options and usage
```
