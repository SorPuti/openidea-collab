#!/usr/bin/env bash
set -e
echo "▶ Buildando plugin WASM..."
cd "$(dirname "$0")/../openidea-collab"
openidea-plugin build
echo "✓ Plugin buildado em dist/plugin.wasm"
echo "  Use 'openidea-plugin link' para linkar no OpenIDEA local."
