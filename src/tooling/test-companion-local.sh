#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --locked --release -p collab-server -p openidea-collab-companion
python3 src/companion/tests/local_e2e.py
