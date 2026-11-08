export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: all build release release-hce test test-hce test-scalar bench perft clippy clean viz

all: release

build:
	cargo build

# Default build: NNUE with the net in nets/ (see build.rs); another net with EVALFILE=<path>.
# EXE=<path> also copies the binary there (OpenBench builds with `make -j EXE=... EVALFILE=...`).
release:
	cargo build --release
ifdef EXE
	cp target/release/aqueval$(if $(filter Windows_NT,$(OS)),.exe) $(EXE)
endif

# Handcrafted eval only (no net).
release-hce:
	cargo build --release --no-default-features

test:
	cargo test --release

test-hce:
	cargo test --release --no-default-features

# NNUE with the scalar kernels, as on CPUs without AVX2.
test-scalar:
	AQUEVAL_NO_AVX2=1 cargo test --release

bench: release
	./target/release/aqueval bench

perft: release
	./target/release/aqueval perft 6

clippy:
	cargo clippy --all-targets -- -D warnings
	cargo clippy --all-targets --no-default-features -- -D warnings

viz:
	xdg-open tools/netviz/netviz.html

clean:
	cargo clean
