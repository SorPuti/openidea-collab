#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$root/target/host-api-prototype"
rustc --edition 2024 --test "$root/src/host-api/prototype/harness.rs" \
  -o "$root/target/host-api-prototype/network-inbox"
"$root/target/host-api-prototype/network-inbox"
