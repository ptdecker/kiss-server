#!/usr/bin/env bash
set -euo pipefail

cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
cargo test --locked
