#!/usr/bin/env bash
# Prepare artifacts only. Publishing and marketplace updates are separate steps.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
: "${OPENIDEA_KEY_FILE:?Set the signing key file path}"
: "${OPENIDEA_PUBLISHER:?Set the publisher ID (sorputi)}"
: "${OPENIDEA_DOWNLOAD_BASE:?Set the versioned release download URL}"
: "${RELEASE_TAG:?Set the exact source tag, for example v0.3.0}"
[[ "$OPENIDEA_PUBLISHER" == sorputi ]] || { echo "Publisher must be sorputi" >&2; exit 1; }
python3 src/tooling/verify-release.py --tag "$RELEASE_TAG"
# An absolute key path continues to work after changing the working directory.
KEY_FILE="$(python3 -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$OPENIDEA_KEY_FILE")"
VERSION="$(python3 -c 'import tomllib; print(tomllib.load(open("src/plugin/plugin.toml", "rb"))["version"])')"
PACKAGE_FILE="sorputi.openidea-collab-${VERSION}.oiplugin"
DIST_DIR="$ROOT/dist-release/$RELEASE_TAG"
[[ ! -e "$DIST_DIR" ]] || { echo "Output exists: $DIST_DIR; use a fresh checkout" >&2; exit 1; }
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
# Build tracked source from the verified tag, excluding local ignored files.
git archive "$RELEASE_TAG" | tar -x -C "$STAGE"
cd "$STAGE"
./src/tooling/package-dev.sh
mkdir -p "$DIST_DIR"
openidea-plugin pack target/plugin-package --key "$KEY_FILE" --publisher "$OPENIDEA_PUBLISHER" --out "$DIST_DIR/$PACKAGE_FILE"
openidea-plugin check "$DIST_DIR/$PACKAGE_FILE"
openidea-plugin index "$DIST_DIR" --publisher "$OPENIDEA_PUBLISHER" --key "$KEY_FILE" \
  --download-base "$OPENIDEA_DOWNLOAD_BASE" --out "$DIST_DIR/index.json"
# Preserve the development installer bundle format, with license and README.
tar -czf "$DIST_DIR/plugin.tar.gz" -C target/plugin-package plugin.toml README.md LICENSE NOTICE dist locales
cd "$DIST_DIR"
python3 - "$ROOT" "$RELEASE_TAG" <<'PY'
import hashlib
import json
from pathlib import Path
import subprocess
import sys
root, tag = sys.argv[1:]
commit = subprocess.check_output(["git", "-C", root, "rev-parse", f"refs/tags/{tag}^{{commit}}"], text=True).strip()
Path("source.json").write_text(json.dumps({"tag": tag, "commit": commit, "sdk_tag": "v0.1.43", "rust": "1.96.0"}, indent=2) + "\n")
files = sorted(p for p in Path('.').iterdir() if p.is_file())
Path("SHA256SUMS").write_text(''.join(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in files))
PY
printf 'Prepared artifacts in %s\nNo release was published.\n' "$DIST_DIR"
