#!/usr/bin/env python3
"""Build and install the Linux client bundle in an isolated directory."""
import os
from pathlib import Path
import socket
import subprocess
import tarfile
import tempfile
import time
import urllib.request
import zipfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='collab-client-bundle-') as tmp:
    tmp = Path(tmp)
    archive = tmp/'bundle.tar.gz'
    subprocess.run(['bash', str(root/'scripts/package-client.sh'), str(archive)], check=True)
    with tarfile.open(archive) as tar:
        tar.extractall(tmp, filter='data')
    bundle = tmp/'collab-client'
    with zipfile.ZipFile(bundle/'plugin.oiplugin') as pkg:
        manifest = pkg.read('plugin.toml').decode()
        assert 'collab-panel' not in manifest and 'priority = "default"' in manifest
        assert 'web/collab-editor/editor.js' in pkg.namelist()
    env = dict(os.environ, OPENIDEA_COLLAB_CLIENT_ROOT=str(tmp/'installation with spaces'), PATH='/usr/bin:/bin')
    run = lambda *args: subprocess.run(['bash', str(bundle/'install-client.sh'), *args], env=env, capture_output=True, text=True)
    assert run('--dry-run').returncode == 0
    assert not (tmp/'installation with spaces').exists()
    installed = run()
    assert installed.returncode == 0, installed.stderr
    client = next((tmp/'installation with spaces/releases').iterdir())
    assert (client/'plugin.oiplugin').is_file() and os.access(client/'openidea-collab-helper', os.X_OK)
    assert not (client/'collab-server').exists()
    host_env = dict(env, OPENIDEA_COLLAB_CLIENT_ROOT=str(tmp/'host install'))
    host = subprocess.run(['bash', str(bundle/'install-client.sh'), '--server'], env=host_env, capture_output=True, text=True)
    assert host.returncode == 0, host.stderr
    host_release = next((tmp/'host install/releases').iterdir())
    assert os.access(host_release/'collab-server', os.X_OK) and os.access(host_release/'start-server.sh', os.X_OK)
    assert (host_release/'data').stat().st_mode & 0o777 == 0o700
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    process = subprocess.Popen([str(host_release/'start-server.sh')], env=dict(host_env, COLLAB_PORT=str(port)), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            if process.poll() is not None: raise AssertionError('server exited')
            try:
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/health', timeout=.2) as response:
                    if response.status == 200: break
            except OSError: time.sleep(.03)
        else: raise AssertionError('server did not start')
    finally:
        process.terminate(); process.wait(timeout=3)
    with (bundle/'plugin.oiplugin').open('ab') as f: f.write(b'tampered')
    assert run('--dry-run').returncode != 0
print('PASS: binary bundle, isolated client installation, tamper detection and no Rust/CLI in PATH')
