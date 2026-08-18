#!/bin/bash
set -e
cargo clean
cargo fmt --check
cargo clippy --all-targets --release
cargo test --release
