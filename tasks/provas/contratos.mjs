// Protótipo de contrato para testes; não é cliente, transporte ou segurança de produção.
export function makeSession(config, adapters) {
  const transport = adapters.connections[config.connection];
  const editor = adapters.editors[config.editor];
  if (!transport || !editor) throw new Error('unsupported adapter');
  return {
    connect: () => transport.connect(config.endpoint),
    sendLocal: (update) => transport.send(update),
    applyRemote: (update) => editor.apply(update),
  };
}

export function applyEditorEdit(bridge, version, from, to, insert) {
  return bridge.edit(version, [{ from, to, insert }]);
}

export function createPtyProbe(sessionId, postMessage, consume) {
  return {
    async onMessage(raw) {
      let msg;
      try { msg = JSON.parse(raw); } catch { return; }
      if (msg.sessionId !== sessionId || msg.op !== 'pty-data') return;
      await consume(msg.data);
      postMessage(JSON.stringify({ op: 'pty-ack', sessionId, sequence: msg.sequence }));
    },
  };
}
