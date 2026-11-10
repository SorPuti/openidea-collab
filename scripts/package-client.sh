#!/usr/bin/env bash
# Build a transferable client bundle from already-built artifacts.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# == 1 && -n "$1" ]] || { echo "Usage: $0 OUTPUT.tar.gz" >&2; exit 2; }
output="$1"; [[ "$output" == /* ]] || output="$PWD/$output"
[[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || { echo 'Package on Linux x86_64.' >&2; exit 1; }
helper="$root/collab-helper/target/release/openidea-collab-helper"
server="$root/collab-server/target/release/collab-server"
[[ -x "$helper" ]] || { echo 'Build the release helper first.' >&2; exit 1; }
[[ -x "$server" ]] || { echo 'Build the release server first.' >&2; exit 1; }
stage="$(mktemp -d)"
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$stage/collab-client" "$(dirname "$output")"
openidea-plugin pack "$root/openidea-collab" --out "$stage/collab-client/plugin.oiplugin"
openidea-plugin check "$stage/collab-client/plugin.oiplugin"
install -m 755 "$helper" "$stage/collab-client/openidea-collab-helper"
install -m 755 "$server" "$stage/collab-client/collab-server"
install -m 755 "$root/scripts/install-client.sh" "$stage/collab-client/install.sh"
install -m 755 "$root/scripts/register-dev-plugin.py" "$stage/collab-client/register-dev-plugin.py"
(cd "$stage/collab-client" && sha256sum plugin.oiplugin openidea-collab-helper collab-server register-dev-plugin.py > SHA256SUMS)
tar -czf "$output" -C "$stage" collab-client
printf 'Unsigned client bundle: %s\n' "$output"
