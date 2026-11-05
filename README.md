# Quant

Quant is a UCI chess engine written from scratch in Rust. It has no dependencies, and its NNUE evaluation was trained only on Quant's own self-play games.

- **Strength:** about <!-- ELO: from the v1.0 gauntlet --> Elo against Stockfish 17 with `UCI_LimitStrength`. This is an estimate, not a CCRL rating.
- **Platforms:** Linux and Windows (x86-64), macOS (Apple Silicon). AVX2 is detected at run time; CPUs without it use scalar kernels, which give the same results at about half the speed.

## Download

Prebuilt binaries are on the [releases page](https://github.com/scarcities/quant/releases). Quant speaks UCI, so you need a chess GUI such as Cute Chess, Arena, En Croissant or BanksiaGUI. Add the binary there as a UCI engine.

## Build from source

Install Rust from https://rustup.rs, then:

```
git clone https://github.com/scarcities/quant
cd quant
cargo build --release
```

The binary is `target/release/quant`. The net in `nets/` is embedded at build time. To embed a different net, build with `EVALFILE=<path> cargo build --release`.

## UCI options

| Option | Default | Meaning |
|---|---|---|
| Hash | 16 | Transposition table size in MB (1–1024) |
| Threads | 1 | Single-threaded for now |
| Move Overhead | 30 | Milliseconds held back per move for GUI and network lag |
| EvalFile | `<embedded>` | Path to another `.nnue` net |
| UseNNUE | true | `false` switches to the handcrafted evaluation |

## Command line

```
quant              # UCI mode
quant bench        # fixed search over standard positions; the node count is the bench signature
quant perft <n>    # perft from the start position
```

`go perft <n>` inside UCI prints a divide.

## How it works

- **Board:** bitboards with fancy magic slider attacks and fully legal move generation via check masks and pin rays. Move generation and the search allocate nothing on the heap.
- **Search:** iterative deepening and principal variation search with aspiration windows. The transposition table, quiescence search and SEE are standard. Pruning and reductions include null move pruning, reverse futility pruning, futility pruning, late move pruning, SEE pruning and late move reductions. Search extends checks and singular moves, with multicut. Moves are ordered by killers, counter moves, butterfly history and continuation history. A pawn correction history adjusts the static eval.
- **Evaluation:** a (768 → 256)x2 → 1 NNUE with SCReLU, quantised and updated incrementally, with AVX2 kernels. The search uses no floating point, so `bench` gives the same node count on every platform.
- **Testing:** every search change was tested by SPRT with fastchess before it was merged, and failed tests were logged too. The log is in [AGENTS.md](AGENTS.md), and [CHANGELOG.md](CHANGELOG.md) lists every merged change with its bench signature.

## Training data

The net was trained on 87.3 million positions from Quant's own self-play with its handcrafted evaluation, using Quant's own trainer (`scripts/nnue/train.py`). No Stockfish, Leela or other external data, evaluations, nets or tablebases were used. Details are in [docs/NNUE.md](docs/NNUE.md).

## License

<!-- LICENSE: to be decided -->
