#!/usr/bin/env python3
"""Validate the actual custom-editor manifest in isolation; never install or link it."""
from pathlib import Path
import shutil, subprocess, tempfile
root = Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix='collab-editor-package-') as tmp:
    package = Path(tmp)
    (package / 'dist').mkdir()
    shutil.copy(root / 'openidea-collab/dist/plugin.wasm', package / 'dist/plugin.wasm')
    shutil.copytree(root / 'openidea-collab/web', package / 'web')
    shutil.copy(root / 'openidea-collab/plugin.toml', package / 'plugin.toml')
    subprocess.run(['openidea-plugin', 'check', str(package)], check=True)
