#!/usr/bin/env bash
# Source installer entry point. For the prebuilt bundle use its install-client.sh.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ "${1:-}" == "--experimental" ]]; then shift; fi
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" || $# == 0 ]]; then
  cat <<'HELP'
OpenIDEA Collab — source installer (experimental)

  bash install.sh --mode local [--link] [--start-server] [--persist]
  bash install.sh --mode remote --endpoint https://example.org [--link]
  bash install.sh --mode managed --endpoint https://example.org [--link]
  Add --dry-run to inspect the plan without changing files.

For a prebuilt client without Rust, extract the release client bundle and run
its install-client.sh. Remote and managed modes require an existing server.
Run this script from a complete source checkout; it does not download code.
HELP
  exit 0
fi
[[ -f "$repo_root/scripts/install-realtime.sh" ]] || { echo 'Missing scripts/install-realtime.sh; use a full source checkout.' >&2; exit 1; }
exec bash "$repo_root/scripts/install-realtime.sh" "$@"
