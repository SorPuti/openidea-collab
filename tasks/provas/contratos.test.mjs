import { test } from 'node:test';
import assert from 'node:assert/strict';
import { makeSession, createPtyProbe, applyEditorEdit } from './contratos.mjs';

for (const connection of ['managed', 'self_hosted']) {
  for (const editor of ['native', 'custom']) {
    test(`mesmo núcleo seleciona adaptadores ${connection}/${editor}`, async () => {
      const sent = [];
      const transport = { async connect(endpoint) { sent.push(endpoint); }, async send(update) { sent.push(update); } };
      const writes = [];
      const editorAdapter = { async apply(edit) { writes.push(edit); } };
      const session = makeSession({ connection, editor, endpoint: 'https://relay.example.test' }, {
        connections: { [connection]: transport }, editors: { [editor]: editorAdapter },
      });
      await session.connect();
      await session.sendLocal({ id: 'u1', data: 'abc' });
      await session.applyRemote({ id: 'u2', data: 'xyz' });
      assert.deepEqual(sent, ['https://relay.example.test', { id: 'u1', data: 'abc' }]);
      assert.deepEqual(writes, [{ id: 'u2', data: 'xyz' }]);
    });
  }
}

test('modo não suportado falha sem fallback silencioso', () => {
  assert.throws(() => makeSession({ connection: 'other', editor: 'custom' }, { connections: {}, editors: {} }), /unsupported/);
});

test('edição stale é propagada sem substituir ou tentar novamente', async () => {
  let calls = 0;
  const bridge = { edit: async () => { calls++; throw new Error('stale'); } };
  await assert.rejects(applyEditorEdit(bridge, 1, 0, 0, 'x'), /stale/);
  assert.equal(calls, 1);
});

test('PTY confirma somente após consumir e ignora outras sessões', async () => {
  const messages = [];
  let consume;
  const waiting = new Promise(resolve => { consume = resolve; });
  const pty = createPtyProbe('one', value => messages.push(JSON.parse(value)), async () => waiting);
  await pty.onMessage(JSON.stringify({ op: 'pty-data', sessionId: 'two', data: 'x', sequence: 1 }));
  const processing = pty.onMessage(JSON.stringify({ op: 'pty-data', sessionId: 'one', data: 'x', sequence: 2 }));
  assert.deepEqual(messages, []);
  consume();
  await processing;
  assert.deepEqual(messages, [{ op: 'pty-ack', sessionId: 'one', sequence: 2 }]);
});

test('PTY não confirma em erro de processamento', async () => {
  const messages = [];
  const pty = createPtyProbe('one', value => messages.push(value), async () => { throw Error('invalid'); });
  await assert.rejects(pty.onMessage(JSON.stringify({ op: 'pty-data', sessionId: 'one', data: 'x', sequence: 3 })), /invalid/);
  assert.deepEqual(messages, []);
});
