#!/usr/bin/env python3
"""Instalação real em /tmp com links OpenIDEA isolados; limpa somente recursos de teste."""
import json, os, signal, socket, subprocess, tempfile, time, urllib.request
from pathlib import Path
root = Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix='collab-install-proof-') as temp:
    base = Path(temp)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    env = {**os.environ, 'OPENIDEA_PLUGINS_DIR': str(base / 'openidea-links')}
    target = base / 'local install'
    result = subprocess.run(['bash', str(root/'install.sh'), '--experimental', '--mode','local','--root',str(target),'--port',str(port),'--link','--start-server','--persist'], env=env, text=True, capture_output=True)
    if result.returncode:
        print(result.stdout); print(result.stderr); raise SystemExit(result.returncode)
    releases = list((target / 'releases').iterdir())
    assert len(releases) == 1
    release = releases[0]
    pid = int((release / 'server.pid').read_text())
    restarted = None
    try:
        with urllib.request.urlopen(f'http://127.0.0.1:{port}/health', timeout=3) as response:
            assert json.load(response)['api'] == 'realtime-experimental'
        data = target/'data'
        assert data.stat().st_mode & 0o777 == 0o700
        assert (data/'journal.sqlite').stat().st_mode & 0o777 == 0o600
        request = urllib.request.Request(f'http://127.0.0.1:{port}/v1/rooms', method='POST')
        with urllib.request.urlopen(request, timeout=3) as response: room = json.load(response)
        os.kill(pid, signal.SIGTERM)
        for _ in range(60):
            try:
                with socket.create_connection(('127.0.0.1', port),timeout=.1): pass
            except OSError: break
            time.sleep(.1)
        else: raise RuntimeError('original server did not stop')
        pid = None
        restarted = subprocess.Popen([str(release/'start-server.sh')], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        for _ in range(100):
            if restarted.poll() is not None: raise RuntimeError('restart failed: '+restarted.stderr.read().decode())
            try:
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/health', timeout=.2): break
            except OSError: time.sleep(.05)
        else: raise RuntimeError('restarted server did not become healthy')
        request = urllib.request.Request(f'http://127.0.0.1:{port}/v1/rooms/{room["room_id"]}/invites',
            data=json.dumps({'role':'editor','ttl_seconds':60}).encode(),
            headers={'Authorization':f'Bearer {room["owner_token"]}', 'Content-Type':'application/json'}, method='POST')
        with urllib.request.urlopen(request, timeout=3) as response: assert response.status == 200
        assert (release/'bin/openidea-collab-helper').is_file()
        assert (release/'plugin/web/collab-editor/editor.js').is_file()
        assert f'http://127.0.0.1:{port}' in (release/'plugin/web/collab-editor/config.js').read_text()
        links = subprocess.check_output(['openidea-plugin','links'], env=env, text=True)
        assert str(release/'plugin') in links
        duplicate = subprocess.run(['bash', str(root/'scripts/install-realtime.sh'), '--mode','local','--root',str(base/'other'),'--link'], env=env, text=True, capture_output=True)
        assert duplicate.returncode != 0 and 'already exists' in duplicate.stderr
        unsafe_root = base/'unsafe root'
        unsafe_root.mkdir()
        outside = base/'outside'
        outside.mkdir()
        (outside/'sentinel').write_text('preserve')
        (unsafe_root/'data').symlink_to(outside, target_is_directory=True)
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0)); other_port = sock.getsockname()[1]
        rejected = subprocess.run(['bash', str(root/'scripts/install-realtime.sh'), '--mode','local','--root',str(unsafe_root), '--port', str(other_port), '--start-server', '--persist'], env=env, text=True, capture_output=True)
        assert rejected.returncode != 0 and 'not a symlink' in rejected.stderr
        assert (outside/'sentinel').read_text() == 'preserve' and not (outside/'journal.sqlite').exists()
        for mode in ('remote', 'managed'):
            dest = base / (mode + ' install')
            remote = subprocess.run(['bash', str(root/'scripts/install-realtime.sh'), '--mode',mode,'--endpoint','https://collab.example.test','--root',str(dest)], env=env, text=True, capture_output=True)
            if remote.returncode:
                print(remote.stdout); print(remote.stderr); raise RuntimeError(f'{mode} install failed')
            item = next((dest/'releases').iterdir())
            assert (item/'bin/openidea-collab-helper').is_file()
            assert not (item/'bin/collab-server').exists()
            assert 'https://collab.example.test' in (item/'plugin/web/collab-editor/config.js').read_text()
        print('PASS: instalação local persistente, reinício, remote/managed, link isolado e proteção de link existente')
    finally:
        if restarted is not None:
            restarted.terminate()
            restarted.wait(timeout=3)
        if pid is not None:
            try: os.kill(pid, signal.SIGTERM)
            except ProcessLookupError: pass
        for _ in range(30):
            try:
                with socket.create_connection(('127.0.0.1', port),timeout=.1): pass
            except OSError: break
            time.sleep(.1)
