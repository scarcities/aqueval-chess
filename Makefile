export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: all build release test bench perft clippy clean

all: release

build:
	cargo build

release:
	cargo build --release

test:
	cargo test --release

bench: release
	./target/release/quant bench

perft: release
	./target/release/quant perft 6

clippy:
	cargo clippy --all-targets -- -D warnings

clean:
	cargo clean
