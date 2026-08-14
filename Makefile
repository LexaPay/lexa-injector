.PHONY: all build test fmt lint clean

all: build test

build:
	cargo build --target wasm32-unknown-unknown --release

test:
	cargo test --release

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --release

clean:
	cargo clean

# Makefile runtime config
DEBUG ?= false
