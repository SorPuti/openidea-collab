#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -ne 1 ]]; then
  echo "usage: $0 <rust-target-triple>" >&2
  exit 2
fi
target="$1"
name="openidea-collab-companion"
cargo build --locked --release -p "$name" --target "$target"
case "$target" in
  *windows*) ext=".exe" ;;
  *) ext="" ;;
esac
out="target/companion/$target"
mkdir -p "$out"
cp "target/$target/release/$name$ext" "$out/$name$ext"
if command -v sha256sum >/dev/null; then
  (cd "$out" && sha256sum "$name$ext" > SHA256SUMS)
else
  shasum -a 256 "$out/$name$ext" | sed "s#${out}/##" > "$out/SHA256SUMS"
fi
printf 'Companion build: %s\n' "$out"
