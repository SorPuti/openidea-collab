#!/usr/bin/env python3
"""Roda testes no SDK com cópia isolada do plugin; sem instalar nem alterar o produto."""
from pathlib import Path
import os, shutil, subprocess, tempfile
root = Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix='collab-sdk-proof-') as tmp:
    copy = Path(tmp) / 'plugin'
    shutil.copytree(root / 'openidea-collab', copy, ignore=shutil.ignore_patterns('target', 'dist', '*.pem', '*.oiplugin'))
    lib = copy / 'src/lib.rs'
    lib.write_text(lib.read_text() + (root / 'tasks/provas/collab-real/sdk-test.rs').read_text())
    env = {**os.environ, 'CARGO_TARGET_DIR': '/tmp/openidea-collab-sdk-proof-target'}
    subprocess.run(['cargo', 'test', '--manifest-path', str(copy / 'Cargo.toml'), '--lib', '--offline', 'proof_collab_panel'], check=True, env=env)
