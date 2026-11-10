#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --locked --release -p openidea-plugin-openidea-collab --target wasm32-wasip2
package_dir="target/plugin-package"
mkdir -p "$package_dir/dist"
cp src/plugin/plugin.toml "$package_dir/plugin.toml"
cp target/wasm32-wasip2/release/openidea_collab.wasm "$package_dir/dist/plugin.wasm"
cp LICENSE NOTICE "$package_dir/"
cp docs/plugin.md "$package_dir/README.md"
if [[ -d src/plugin/locales ]]; then
  mkdir -p "$package_dir/locales"
  cp src/plugin/locales/*.json "$package_dir/locales/"
fi
"${OPENIDEA_PLUGIN_CLI:-openidea-plugin}" check "$package_dir"
printf 'Development package: %s\n' "$package_dir"
