#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# Rust retains dependency source locations in this Wasm component.  Canonicalize
# Cargo's otherwise user-specific prefix so release bytes can be reproduced on
# a different machine.  Callers may override the stable public prefix, but not
# disable the mapping accidentally.
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
remap_prefix="${OPENIDEA_REMAP_CARGO_PREFIX:-/usr/local/cargo}"
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--remap-path-prefix=${cargo_home}=${remap_prefix}"
cargo build --locked --release -p openidea-plugin-openidea-collab --target wasm32-wasip2
target_dir="${CARGO_TARGET_DIR:-target}"
package_dir="target/plugin-package"
mkdir -p "$package_dir/dist"
cp src/plugin/plugin.toml "$package_dir/plugin.toml"
cp src/plugin/settings.schema.json "$package_dir/settings.schema.json"
if [[ -d src/plugin/icons ]]; then
  mkdir -p "$package_dir/icons"
  cp src/plugin/icons/*.svg "$package_dir/icons/"
fi
cp "$target_dir/wasm32-wasip2/release/openidea_collab.wasm" "$package_dir/dist/plugin.wasm"
# A manifest contribution is only usable when the component exports the
# optional WIT interface. `openidea-plugin check` validates the manifest but
# intentionally permits components without views, so catch a future macro
# regression before a package reaches the Marketplace.
if ! grep -aq 'openidea:plugin/view-provider@1.6.0#render' "$package_dir/dist/plugin.wasm"; then
  printf '%s\n' 'plugin.wasm does not export the required view-provider interface' >&2
  exit 1
fi
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
