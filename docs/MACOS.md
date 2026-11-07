# aqueval chess on macOS (Apple Silicon)

Checks that the default build (NNUE gen-1, scalar kernels: Apple Silicon has no AVX2) builds,
passes the tests and searches exactly like on Linux.

## 1. Install Rust

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
```

Xcode command line tools are needed for the linker; if `cargo` complains, run
`xcode-select --install`.

## 2. Clone the repository

```
git clone https://github.com/scarcities/aqueval-chess
cd aqueval-chess
```

The net `nets/aqueval-gen1-7ae6f8c0.nnue` is part of the repository and is embedded at build time.

## 3. Run the tests

```
cargo test --release
cargo test --release --no-default-features
```

Both must end without failures (136 tests each on Linux). The first run takes a few minutes
(perft and the 10k-position cross-check).

## 4. Run the bench

```
cargo build --release
./target/release/aqueval bench
```

The line `Nodes searched  : …` must show exactly the bench signature in AGENTS.md (953967 for v1.0, 680520 since the gen-1 net). Nodes/second will differ
from the Linux machine; the node count must not. The handcrafted-eval build gives the second signature there (912599):

```
cargo build --release --no-default-features
./target/release/aqueval bench
```

## If the bench differs

- Check that you are on the commit you expect (`git log -1`) and that CHANGELOG.md names
  `aqueval-gen1-7ae6f8c0.nnue` as the current net.
- If the NNUE bench differs but the handcrafted bench matches, the NNUE scalar path differs
  from the AVX2 path: report both numbers.
- If both differ, something platform-dependent reached the search. It should not: the search
  uses no floating point (late move reductions come from a fixed integer table,
  `src/search/lmr.rs`, and the net is integer-only). Report both numbers and `rustc --version`.
