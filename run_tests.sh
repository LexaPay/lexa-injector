#!/bin/bash
set -e
cargo fmt --check
cargo clippy --all-targets --release
cargo test --release
