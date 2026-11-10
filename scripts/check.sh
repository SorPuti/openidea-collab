#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/verify-release.py
python3 -m unittest discover -s scripts/tests
for component in collab-server openidea-collab; do
  cargo fmt --manifest-path "$component/Cargo.toml" -- --check
  cargo clippy --locked --manifest-path "$component/Cargo.toml" --all-targets -- -D warnings
  cargo test --locked --manifest-path "$component/Cargo.toml"
done
cargo build --locked --release --manifest-path openidea-collab/Cargo.toml --target wasm32-wasip2
