#!/usr/bin/env bash
set -e
echo "▶ Compilando collab-server..."
cd "$(dirname "$0")/.."
cargo build --manifest-path collab-server/Cargo.toml
echo "▶ Iniciando collab-server em :${COLLAB_PORT:-4444}..."
RUST_LOG=info COLLAB_PORT="${COLLAB_PORT:-4444}" ./collab-server/target/debug/collab-server
