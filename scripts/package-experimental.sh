#!/usr/bin/env bash
# Create a validated, unsigned development bundle without publishing anything.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# == 1 && -n "$1" ]] || { echo "Usage: $0 OUTPUT.tar.gz" >&2; exit 2; }
output="$1"
[[ "$output" == /* ]] || output="$PWD/$output"
command -v openidea-plugin >/dev/null || { echo 'openidea-plugin is required.' >&2; exit 1; }
plugin="$repo_root/openidea-collab"
for file in plugin.toml dist/plugin.wasm web/collab-editor/index.html web/collab-editor/editor.js web/collab-editor/config.js; do
  [[ -f "$plugin/$file" ]] || { echo "Missing plugin asset: $file" >&2; exit 1; }
done
stage="$(mktemp -d)"
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$stage/plugin/dist" "$(dirname "$output")"
cp "$plugin/plugin.toml" "$stage/plugin/"
cp "$plugin/dist/plugin.wasm" "$stage/plugin/dist/"
cp -R "$plugin/web" "$stage/plugin/web"
openidea-plugin check "$stage/plugin"
tar -czf "$output" -C "$stage/plugin" .
printf 'Unsigned development bundle: %s\n' "$output"
