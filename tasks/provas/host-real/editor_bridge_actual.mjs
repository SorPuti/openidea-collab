// Actual Collab editor.js + upstream ptyBridge.ts, with simulated DOM/document
// and a pipe backend that runs actual helpers. Not a real OpenIDEA UI or PTY.
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { ptyBridge } from './bridge.ts';

const code = await readFile(process.env.COLLAB_EDITOR, 'utf8');
const children = [];
function makeEditor() {
  const elements = new Map();
  function el(name) {
    if (!elements.has(name)) {
      const handlers = new Map();
      elements.set(name, { value:'', textContent:'', checked:false, hidden:false, readOnly:false, disabled:false,
        addEventListener(type, callback) { handlers.set(type, callback); },
        dispatch(type) { const cb = handlers.get(type); assert.ok(cb, `${name}.${type}`); cb({preventDefault(){}}); } });
    }
    return elements.get(name);
  }
  el('endpoint').value = process.env.COLLAB_ENDPOINT;
  el('document').value = 'main'; el('mode').value = 'create';
  let text = 'a😀b', version = 1, handler = () => {}, exitListener = () => {}, child;
  const backend = {
    async spawn(command, output) {
      assert.equal(command.program, 'openidea-collab-helper');
      child = spawn(process.env.COLLAB_HELPER, [], {stdio:['pipe','pipe','ignore']});
      children.push(child); child.stdout.setEncoding('utf8'); child.stdout.on('data', output);
      child.on('exit', code => exitListener(1, code));
      return {id:1};
    },
    async write(id, data) { assert.equal(id,1); child.stdin.write(data); },
    async resize() {}, async kill() { if (child && !child.killed) child.kill(); },
    onExit(fn) { exitListener=fn; return () => { exitListener=()=>{}; }; }
  };
  const bridge = ptyBridge('sorputi.openidea-collab', backend, message => handler(JSON.stringify(message)));
  const openidea = { postMessage(raw) { assert.ok(bridge.handle(raw)); }, onMessage(fn) { handler=fn; },
    document: {
      async get() { return {version,text,readOnly:false}; }, onChange() {},
      async edit(at, edits) { if (at !== version) throw Error('stale');
        for (const e of edits) text=text.slice(0,e.from)+e.insert+text.slice(e.to);
        return {version:++version}; }
    }
  };
  const context = vm.createContext({document:{getElementById:el}, openidea});
  vm.runInContext(code, context, {filename:'editor.js'});
  return {el, bridge, doc:()=>text};
}
async function until(predicate, description) {
  const limit=Date.now()+7000;
  while (!predicate()) { if (Date.now()>limit) throw Error('timeout: '+description); await new Promise(r=>setTimeout(r,20)); }
}
const a=makeEditor(), b=makeEditor();
try {
  await until(()=>a.el('status').textContent.includes('Informe') && b.el('status').textContent.includes('Informe'),'initial documents');
  a.el('connect').dispatch('submit');
  await until(()=>!a.el('editor').readOnly && a.el('connect').hidden,'create room');
  a.el('create-invite').dispatch('click');
  await until(()=>a.el('invite-output').value.length>0,'invite');
  b.el('mode').value='join'; b.el('invite-input').value=a.el('invite-output').value;
  b.el('connect').dispatch('submit');
  await until(()=>!b.el('editor').readOnly && b.el('connect').hidden,'join room');
  a.el('editor').value='aX😀b'; b.el('editor').value='aY😀b';
  a.el('editor').dispatch('input'); b.el('editor').dispatch('input');
  await until(()=>a.doc()===b.doc() && a.doc().includes('X') && a.doc().includes('Y'),'concurrent convergence');
  assert.equal(a.el('editor').value,a.doc()); assert.equal(b.el('editor').value,b.doc());
  await until(()=>a.el('status').textContent === 'Conectado','first client sync');
  a.el('leave-session').dispatch('click');
  await until(()=>!a.el('connect').hidden && a.el('editor').readOnly,'clean disconnect');
  assert.equal(a.doc(), b.doc());
  console.log('PASS: 2 editor.js clients + upstream PTY bridge + real helpers and server converged');
} finally {
  a.bridge.close(); b.bridge.close();
  for (const child of children) if (!child.killed) child.kill();
}
