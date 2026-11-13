#!/usr/bin/env python3
"""Low-RAM integration with real editor.js, upstream PTY bridge and server."""
from pathlib import Path
import argparse, os, socket, subprocess, tempfile, time, urllib.request

root = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser()
parser.add_argument('--release', type=Path, help='Use the staged installer release instead of debug builds')
args = parser.parse_args()
server_path = args.release/'bin/collab-server' if args.release else root/'collab-server/target/debug/collab-server'
helper_path = args.release/'bin/openidea-collab-helper' if args.release else root/'collab-helper/target/debug/openidea-collab-helper'
editor_path = args.release/'plugin/web/collab-editor/editor.js' if args.release else root/'openidea-collab/web/collab-editor/editor.js'
sources = list((Path.home()/'.cargo/git/checkouts').glob('openidea-*/37c2a60/src/features/plugins/views/ptyBridge.ts'))
if len(sources) != 1: raise SystemExit('Expected cached OpenIDEA v0.1.40 source')
source = sources[0].read_text()
imports = 'import type { TerminalId, TerminalInfo, WebPtyCommand } from "@/bindings";\nimport { tx } from "@/platform/i18n";'
assert imports in source
source = source.replace(imports, 'type TerminalId = number; type TerminalInfo = {id:number}; type WebPtyCommand = any;\nconst tx = (text:string) => text;', 1)
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
endpoint = f'http://127.0.0.1:{port}'
env = {**os.environ, 'COLLAB_API':'realtime', 'COLLAB_BIND':'127.0.0.1', 'COLLAB_PORT':str(port)}
env.pop('COLLAB_JOURNAL', None)
server = subprocess.Popen([str(server_path)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(100):
        if server.poll() is not None: raise RuntimeError('server exited')
        try:
            with urllib.request.urlopen(endpoint+'/health', timeout=.2) as response:
                if response.status == 200: break
        except OSError: time.sleep(.03)
    else: raise RuntimeError('server did not start')
    with tempfile.TemporaryDirectory(prefix='collab-real-bridge-') as temp:
        temp = Path(temp)
        (temp/'bridge.ts').write_text(source)
        (temp/'test.mjs').write_bytes((Path(__file__).with_suffix('.mjs')).read_bytes())
        child_env = {**os.environ, 'COLLAB_ENDPOINT':endpoint, 'COLLAB_HELPER':str(helper_path), 'COLLAB_EDITOR':str(editor_path)}
        subprocess.run(['node','--experimental-strip-types',str(temp/'test.mjs')], env=child_env, check=True, timeout=12)
finally:
    server.terminate()
    try: server.wait(timeout=3)
    except subprocess.TimeoutExpired: server.kill(); server.wait()
