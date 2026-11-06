# aqueval chess

A UCI-compatible chess engine written in Rust (edition 2021). aqueval chess combines legal bitboard move generation, modern game-tree search, and NNUE evaluation. Built for local play, benchmarking, and engine development.

- **Strength:** ~2980 Elo, an estimate against Stockfish 17 with UCI_Elo 2900 at 60+0.6 over 1000 games, not an official rating.
- **Platforms:** Linux and Windows (x86-64), macOS (Apple Silicon). AVX2 is detected at run time; CPUs without it use scalar inference with identical results at about half the speed.

## Download

Prebuilt binaries for Linux, Windows and macOS are on the [releases page](https://github.com/scarcities/aqueval-chess/releases). Add the binary to a UCI chess GUI such as Cute Chess, Arena, En Croissant or BanksiaGUI.

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
./target/release/aqueval
```

The executable starts in **UCI mode**. In its terminal session, try:

```text
uci
isready
position startpos
go depth 8
quit
```

You can also connect `target/release/aqueval` to a UCI-compatible chess GUI.

## UCI options

| Option | Default | Meaning |
|---|---|---|
| Hash | 16 | Transposition table size in MB (1–1024) |
| Threads | 1 | Single search thread for now |
| Move Overhead | 30 | Milliseconds held back per move for GUI and network lag |
| EvalFile | `<embedded>` | Path to another `.nnue` network (NNUE builds) |
| UseNNUE | true | `false` switches to the handcrafted evaluation (NNUE builds) |

## Commands

```bash
./target/release/aqueval bench            # deterministic search benchmark
./target/release/aqueval perft 6          # perft from the initial position
./target/release/aqueval divide 4         # per-move perft breakdown
./target/release/aqueval perft 4 "<FEN>"  # perft from a custom position
```

The node count printed by `bench` is the bench signature: it is the same on every platform, because the search and evaluation use no floating point.

## Engine

- **Move generation:** bitboards, magic sliding-piece attacks, pins, checks, castling, promotions, and en passant.
- **Search:** principal variation search with iterative deepening, aspiration windows, a transposition table and quiescence search with SEE. Pruning and reductions: null move, reverse futility, futility, late move and SEE pruning, late move reductions. Check and singular extensions with multicut. Move ordering by killers, counter moves, butterfly and continuation history; pawn correction history for the static eval.
- **Evaluation:** embedded NNUE network by default; handcrafted evaluation remains available.
- **UCI:** supports standard engine communication, position setup, search commands, and runtime options.
- **Validation:** Rust regression tests, perft suites, and an independent UCI test harness. Every search and evaluation feature after the first working search was tested by SPRT with fastchess before merging; failed tests are logged too ([AGENTS.md](AGENTS.md)).

## NNUE

The default release build embeds the network from `nets/`. To build with another network, set `EVALFILE` at build time:

```bash
EVALFILE=/path/to/network.nnue cargo build --release
```

To build with handcrafted evaluation only:

```bash
cargo build --release --no-default-features
```

On CPUs without AVX2, NNUE uses scalar inference. Set `AQUEVAL_NO_AVX2=1` to force that path when testing. UCI exposes `EvalFile` and `UseNNUE` in NNUE-enabled builds; `Threads` is currently limited to one search thread.

### Training data

The bundled network, a (768 → 256)x2 → 1 SCReLU net, was trained on 87.3 million positions from aqueval chess's own self-play, scored by aqueval chess's handcrafted-evaluation search, with aqueval chess's own trainer (`scripts/nnue/train.py`). No Stockfish, Leela or other external data, evaluations, networks or tablebases were used. Details are in [docs/NNUE.md](docs/NNUE.md).

## Testing

```bash
cargo test --release
cargo test --release --no-default-features
AQUEVAL_NO_AVX2=1 cargo test --release
cargo clippy --all-targets -- -D warnings
python3 harness/perft.py --engine target/release/aqueval
```

The perft harness uses Python's standard library and tests the compiled engine through UCI. The NNUE training tools in `scripts/nnue/` have separate Python dependencies; see the NNUE documentation.

## How aqueval chess is developed

aqueval chess is developed with AI coding agents under the direction of Deniz. Deniz sets the goals, the test protocol and the release decisions; the agents write the code and run the tests. No change is merged without passing the test suite, and new search and evaluation features also have to pass an SPRT.

## Documentation

```text
AGENTS.md              architecture, process rules, SPRT log
TASKS.md               open tasks
CHANGELOG.md           changes and benchmark history
docs/NNUE.md           network format, inference, training
docs/MACOS.md          macOS / Apple Silicon notes
docs/ROADMAP.md        development roadmap
harness/README.md      perft harness options and usage
```

## License

aqueval chess, including the network in `nets/`, is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. See [LICENSE](LICENSE).
