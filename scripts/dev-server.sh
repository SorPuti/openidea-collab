#!/usr/bin/env bash
set -e
cd "$(dirname "$0")/.."
echo "▶ Iniciando collab-server em :${COLLAB_PORT:-4444}..."
RUST_LOG=info COLLAB_PORT="${COLLAB_PORT:-4444}" cargo run --manifest-path collab-server/Cargo.toml
