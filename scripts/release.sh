#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$SCRIPT_DIR/.."
PLUGIN_DIR="$ROOT/openidea-collab"
DIST_DIR="$ROOT/dist-release"

# --- args ---
KEY_FILE="${OPENIDEA_KEY_FILE:-}"
PUBLISHER="${OPENIDEA_PUBLISHER:-}"
DOWNLOAD_BASE="${OPENIDEA_DOWNLOAD_BASE:-}"

usage() {
  echo "Usage: OPENIDEA_KEY_FILE=<path> OPENIDEA_PUBLISHER=<id> OPENIDEA_DOWNLOAD_BASE=<url> $0"
  echo ""
  echo "  OPENIDEA_KEY_FILE      path to your .pem signing key (from: openidea-plugin keygen)"
  echo "  OPENIDEA_PUBLISHER     your publisher ID (e.g. SorPuti)"
  echo "  OPENIDEA_DOWNLOAD_BASE base URL where .oiplugin will be hosted"
  echo "                         e.g. https://github.com/SorPuti/openidea-collab/releases/download/v0.1.0/"
  exit 1
}

[[ -z "$KEY_FILE" || -z "$PUBLISHER" || -z "$DOWNLOAD_BASE" ]] && usage

VERSION=$(grep '^version' "$PLUGIN_DIR/plugin.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')
PACKAGE_FILE="ai.l0ng.openidea-collab-${VERSION}.oiplugin"

echo "▶ Versão: $VERSION"
echo "▶ Publisher: $PUBLISHER"

# build WASM
echo "▶ Buildando plugin WASM..."
cd "$PLUGIN_DIR"
openidea-plugin build

# pack
echo "▶ Empacotando..."
mkdir -p "$DIST_DIR"
openidea-plugin pack "$PLUGIN_DIR" --key "$KEY_FILE" --publisher "$PUBLISHER" --out "$DIST_DIR/$PACKAGE_FILE"

# generate index
echo "▶ Gerando index.json..."
openidea-plugin index "$DIST_DIR" \
  --publisher "$PUBLISHER" \
  --key "$KEY_FILE" \
  --download-base "$DOWNLOAD_BASE" \
  --out "$DIST_DIR/index.json"

echo ""
echo "✓ Pronto! Arquivos em $DIST_DIR:"
ls -lh "$DIST_DIR"
echo ""
echo "Próximo passo: fazer upload de '$PACKAGE_FILE' e 'index.json' para:"
echo "  $DOWNLOAD_BASE"
