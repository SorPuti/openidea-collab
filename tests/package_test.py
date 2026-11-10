#!/usr/bin/env python3
"""Check both experimental and signed packages using a disposable signing key."""
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
with (ROOT / 'openidea-collab/plugin.toml').open('rb') as manifest_file:
    manifest = tomllib.load(manifest_file)
plugin_id, version = manifest['id'], manifest['version']
publisher = plugin_id.split('.', 1)[0]
REQUIRED = {
    'plugin.toml', 'dist/plugin.wasm', 'web/collab-editor/index.html',
    'web/collab-editor/editor.js', 'web/collab-editor/config.js',
}

with tempfile.TemporaryDirectory(prefix='collab-package-test-') as tmp:
    tmp = Path(tmp)
    archive = tmp / 'bundle.tar.gz'
    subprocess.run(['bash', str(ROOT / 'scripts/package-experimental.sh'), str(archive)], check=True, cwd=ROOT)
    with tarfile.open(archive) as package:
        names = {name.removeprefix('./') for name in package.getnames()}
        assert REQUIRED <= names, REQUIRED - names

    key = tmp / 'publisher.pem'
    subprocess.run(['openidea-plugin', 'keygen', str(key)], check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
    env = dict(os.environ, OPENIDEA_KEY_FILE=str(key), OPENIDEA_PUBLISHER=publisher,
               OPENIDEA_DOWNLOAD_BASE='https://example.org/releases/', OPENIDEA_RELEASE_DIR=str(tmp / 'release'))
    subprocess.run(['bash', str(ROOT / 'scripts/release.sh')], check=True, cwd=ROOT, env=env)
    bundle = tmp / 'release' / f'{plugin_id}-{version}.oiplugin'
    with zipfile.ZipFile(bundle) as package:
        names = set(package.namelist())
        assert REQUIRED <= names, REQUIRED - names
    assert (tmp / 'release/index.json').is_file()
print('PASS: development and signed packages contain the editor, manifest and WASM')
