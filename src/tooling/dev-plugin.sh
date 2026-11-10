#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
exec ./src/tooling/package-dev.sh
