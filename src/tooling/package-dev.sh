#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --locked --release -p openidea-plugin-openidea-collab --target wasm32-wasip2
target_dir="${CARGO_TARGET_DIR:-target}"
package_dir="target/plugin-package"
mkdir -p "$package_dir/dist"
cp src/plugin/plugin.toml "$package_dir/plugin.toml"
cp src/plugin/settings.schema.json "$package_dir/settings.schema.json"
cp "$target_dir/wasm32-wasip2/release/openidea_collab.wasm" "$package_dir/dist/plugin.wasm"
cp LICENSE NOTICE "$package_dir/"
cp docs/plugin.md "$package_dir/README.md"
if [[ -n "${COLLAB_RELAY_HOST:-}" ]]; then
  if [[ ! "$COLLAB_RELAY_HOST" =~ ^[A-Za-z0-9.-]+$ ]]; then
    printf '%s\n' 'COLLAB_RELAY_HOST must be a DNS hostname, without scheme, port, or path' >&2
    exit 2
  fi
  sed -i "s/net:collab.synthosia.com/net:${COLLAB_RELAY_HOST}/" "$package_dir/plugin.toml"
  printf 'Development package is restricted to relay host: %s\n' "$COLLAB_RELAY_HOST"
fi
if [[ -d src/plugin/locales ]]; then
  mkdir -p "$package_dir/locales"
  cp src/plugin/locales/*.json "$package_dir/locales/"
fi
"${OPENIDEA_PLUGIN_CLI:-openidea-plugin}" check "$package_dir"
printf 'Development package: %s\n' "$package_dir"
