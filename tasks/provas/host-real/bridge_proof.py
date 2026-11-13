#!/usr/bin/env python3
"""Run the pinned upstream PTY bridge with the real Collab helper over pipes.

Only the upstream type import and translation import are replaced. The backend
is a small subprocess adapter: this does not test OpenIDEA's Rust host, PTY,
permissions, real document integration, or a running IDE.
"""
from pathlib import Path
import os
import subprocess
import tempfile

root = Path(__file__).resolve().parents[3]
sources = list((Path.home()/'.cargo/git/checkouts').glob('openidea-*/37c2a60/src/features/plugins/views/ptyBridge.ts'))
if len(sources) != 1:
    raise SystemExit('Expected one cached OpenIDEA v0.1.40 source checkout')
source = sources[0].read_text()
imports = 'import type { TerminalId, TerminalInfo, WebPtyCommand } from "@/bindings";\nimport { tx } from "@/platform/i18n";'
assert imports in source
source = source.replace(imports, 'type TerminalId = number; type TerminalInfo = {id: number}; type WebPtyCommand = any;\nconst tx = (text: string) => text;', 1)
helper = root/'collab-helper/target/debug/openidea-collab-helper'
if not helper.is_file():
    raise SystemExit('Build collab-helper first')

harness = r'''
import { spawn } from 'node:child_process';
import { ptyBridge } from './bridge.ts';

let child, exitListener = () => {}, seen = '', resolved = false;
const backend = {
  async spawn(command, output) {
    if (command.program !== 'openidea-collab-helper') throw Error('unexpected program');
    child = spawn(process.env.COLLAB_HELPER, [], {stdio:['pipe','pipe','ignore']});
    child.stdout.setEncoding('utf8'); child.stdout.on('data', output);
    child.on('exit', code => exitListener(1, code));
    return {id:1};
  },
  async write(id, data) { if (id !== 1) throw Error('bad id'); child.stdin.write(data); },
  async resize() {},
  async kill() { if (child && !child.killed) child.kill(); },
  onExit(fn) { exitListener = fn; return () => { exitListener = () => {}; }; }
};
const result = new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(Error('bridge timed out')), 5000);
  const bridge = ptyBridge('collab-proof', backend, message => {
    if (message.op === 'pty-error') { clearTimeout(timer); reject(Error(message.error)); return; }
    if (message.op !== 'pty-data') return;
    seen += message.data;
    bridge.handle(JSON.stringify({op:'pty-ack',sessionId:'collab-test',sequence:message.sequence}));
    if (seen.includes('"type":"ready"') && !seen.includes('"type":"error"') && !resolved) {
      resolved = true;
      bridge.handle(JSON.stringify({op:'pty-input',sessionId:'collab-test',data:'not-json\n'}));
    }
    if (seen.includes('"type":"error"')) { clearTimeout(timer); bridge.close(); resolve(); }
  });
  bridge.handle(JSON.stringify({op:'pty-spawn',sessionId:'collab-test',program:'openidea-collab-helper',args:[],cols:100,rows:28}));
});
try { await result; console.log('PASS: upstream PTY bridge exchanged frames with real Collab helper'); }
finally { if (child && !child.killed) child.kill(); }
'''
with tempfile.TemporaryDirectory(prefix='collab-upstream-bridge-') as temp:
    temp = Path(temp)
    (temp/'bridge.ts').write_text(source)
    (temp/'test.mjs').write_text(harness)
    subprocess.run(['node', '--experimental-strip-types', str(temp/'test.mjs')],
                   env={**os.environ, 'COLLAB_HELPER':str(helper)}, check=True, timeout=10)
