# Quant on macOS (Apple Silicon)

Checks that the default build (NNUE gen-0, scalar kernels: Apple Silicon has no AVX2) builds,
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
git clone https://github.com/scarcities/quant
cd quant
```

The net `nets/quant-gen0-74884169.nnue` is part of the repository and is embedded at build time.

## 3. Run the tests

```
cargo test --release
cargo test --release --no-default-features
```

Both must end without failures (134 tests each on Linux). The first run takes a few minutes
(perft and the 10k-position cross-check).

## 4. Run the bench

```
cargo build --release
./target/release/quant bench
```

The line `Nodes searched  : 547732` must show exactly that number. Nodes/second will differ
from the Linux machine; the node count must not. The handcrafted-eval build gives 1032928:

```
cargo build --release --no-default-features
./target/release/quant bench
```

## If the bench differs

- Check that you are on the commit you expect (`git log -1`) and that CHANGELOG.md names
  `quant-gen0-74884169.nnue` as the current net.
- If the NNUE bench differs but the handcrafted bench matches, the NNUE scalar path differs:
  report both numbers.
- If both differ, the most likely cause is the late move reduction formula, which uses
  `f64::ln` from the system math library; macOS and Linux may round the last bit differently.
  Report both numbers and the output of `rustc --version`.
