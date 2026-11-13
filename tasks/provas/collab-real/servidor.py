#!/usr/bin/env python3
"""Prova de API isolada: servidor temporário em loopback, nenhum dado do usuário."""
import json, os, shutil, socket, subprocess, tempfile, time, urllib.request
from pathlib import Path
root = Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix='collab-server-proof-') as tmp:
    path = Path(tmp) / 'server'
    shutil.copytree(root / 'collab-server', path, ignore=shutil.ignore_patterns('target'))
    shutil.copytree(root / 'collab-core', Path(tmp) / 'collab-core', ignore=shutil.ignore_patterns('target'))
    sock = socket.socket(); sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]; sock.close()
    env = {**os.environ, 'COLLAB_PORT': str(port), 'COLLAB_BIND': '127.0.0.1', 'COLLAB_API': 'legacy', 'CARGO_TARGET_DIR': '/tmp/openidea-collab-proof-target'}
    process = subprocess.Popen(['cargo', 'run', '--locked', '--offline', '--quiet', '--manifest-path', str(path / 'Cargo.toml')], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    base = f'http://127.0.0.1:{port}'
    def request(method, route, data=None):
        payload = None if data is None else json.dumps(data).encode()
        req = urllib.request.Request(base + route, data=payload, method=method, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=3) as response:
            return response.status, json.load(response)
    try:
        for _ in range(200):
            if process.poll() is not None:
                raise RuntimeError(process.stderr.read().decode())
            try:
                with socket.create_connection(('127.0.0.1', port), timeout=.1): break
            except OSError: time.sleep(.1)
        else: raise RuntimeError('servidor temporário não iniciou')
        status, room = request('POST', '/rooms', {})
        assert status == 201
        room_id = room['room_id']
        _, member = request('POST', f'/rooms/{room_id}/join', {'name': 'Pessoa de teste'})
        path_id = f'/rooms/{room_id}/files/arquivo.txt'
        _, result = request('POST', path_id, {'user_id': member['user_id'], 'version': 0, 'text': 'Texto de teste'})
        assert result['version'] == 1
        _, content = request('GET', path_id)
        assert content['text'] == 'Texto de teste'
        _, events = request('GET', f'/rooms/{room_id}/events?cursor=0')
        assert any(item['event']['type'] == 'file_updated' for item in events['events'])
        print('PASS: criar sala, entrar, gravar, ler e obter evento no servidor isolado')
    finally:
        process.terminate()
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired: process.kill(); process.wait()
