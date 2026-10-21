export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: all build release release-nnue test test-nnue bench perft clippy clean viz

all: release

build:
	cargo build

release:
	cargo build --release

# NNUE build; embeds a net with `make release-nnue EVALFILE=nets/<net>.nnue`.
release-nnue:
	EVALFILE=$(EVALFILE) cargo build --release --features nnue

test:
	cargo test --release

test-nnue:
	cargo test --release --features nnue

bench: release
	./target/release/quant bench

perft: release
	./target/release/quant perft 6

clippy:
	cargo clippy --all-targets -- -D warnings
	cargo clippy --all-targets --features nnue -- -D warnings

viz:
	xdg-open tools/netviz/netviz.html

clean:
	cargo clean
