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
SOROBAN_SDK_VERSION = "22.0.11"

# Run audit checks
audit:
	cargo audit 2>/dev/null || echo "Install cargo-audit to scan dependencies"

# Clean build documentation files
clean-docs:
	rm -rf docs/*.html
