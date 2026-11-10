#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
python3 src/tooling/verify-release.py
python3 -m unittest discover -s src/tooling/tests
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release -p openidea-plugin-openidea-collab --target wasm32-wasip2
