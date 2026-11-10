(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const editor = $('editor');
  if (typeof globalThis.COLLAB_DEFAULTS?.endpoint === 'string') $('endpoint').value = globalThis.COLLAB_DEFAULTS.endpoint;
  const sessionId = 'collab-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 10);
  const outstanding = new Set();
  let version = 0, model = '', attached = false, seq = 0, queuedInputs = 0;
  let buffer = '', latestHelperText = null, chain = Promise.resolve(), stopped = false;
  let requestedMode = 'create', pendingInvite = null, activeRoom = '';
  let leaving = false, externalBeforeLeave = null;
  const send = (op, data = {}) => openidea.postMessage(JSON.stringify({ op, sessionId, ...data }));
  const status = (text) => { $('status').textContent = text; };
  const fail = (text) => { stopped = true; editor.readOnly = true; $('error').textContent = text; $('error').hidden = false; status('Ação necessária'); };
  // Use code points for boundaries, then count JavaScript UTF-16 units.
  function diff(before, after) {
    const a = Array.from(before), b = Array.from(after);
    let prefix = 0, suffix = 0;
    while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) prefix++;
    while (suffix < a.length - prefix && suffix < b.length - prefix && a[a.length - 1 - suffix] === b[b.length - 1 - suffix]) suffix++;
    const from = a.slice(0, prefix).join('').length;
    const to = before.length - a.slice(a.length - suffix).join('').length;
    return { from, to, insert: b.slice(prefix, b.length - suffix).join('') };
  }
  function enqueue(action) {
    chain = chain.then(async () => { if (!stopped) await action(); }).catch(() => fail('Não foi possível aplicar a alteração. Seu texto continua no editor; copie-o antes de fechar.'));
  }
  function helperEdit(before, after) {
    if (before === after) return;
    const id = String(++seq); outstanding.add(id);
    send('pty-input', { data: JSON.stringify({ type: 'edit', id, ...diff(before, after) }) + '\n' });
  }
  function reconcile() {
    if (stopped || !attached || queuedInputs || outstanding.size || latestHelperText === null) return;
    const target = latestHelperText; latestHelperText = null;
    enqueue(async () => {
      if (queuedInputs || outstanding.size || editor.value !== model) { latestHelperText = target; return; }
      if (target === model) return;
      const result = await openidea.document.edit(version, [diff(model, target)]);
      version = result.version; model = target; editor.value = target;
    });
  }
  editor.addEventListener('input', () => {
    const target = editor.value;
    queuedInputs++;
    enqueue(async () => {
      if (!attached) return;
      const before = model;
      if (before !== target) {
        const result = await openidea.document.edit(version, [diff(before, target)]);
        version = result.version; model = target;
        helperEdit(before, target);
      }
    });
    chain = chain.finally(() => { queuedInputs--; reconcile(); });
  });
  openidea.document.onChange((state) => {
    if (stopped || state.version <= version) return;
    if (leaving) {
      if (externalBeforeLeave === null) externalBeforeLeave = model;
      version = state.version; model = state.text; editor.value = state.text;
      return;
    }
    if (queuedInputs || outstanding.size || editor.value !== model) {
      fail('O documento mudou em outra janela durante uma edição. Preserve seu texto e resolva o conflito.'); return;
    }
    const before = model; version = state.version; model = state.text; editor.value = state.text;
    if (attached) helperEdit(before, state.text);
  });
  function onFrame(value) {
    if (value.type === 'ready') {
      const cmd = { type: requestedMode, id: 'attach', endpoint: $('endpoint').value.trim(), document: $('document').value.trim(), text: model, recovery: $('recovery').checked };
      if (requestedMode === 'join') { cmd.room = pendingInvite.room; cmd.invite = pendingInvite.token; pendingInvite = null; }
      send('pty-input', { data: JSON.stringify(cmd) + '\n' });
    } else if (value.type === 'created' || value.type === 'joined') {
      if ($('recovery').checked && typeof value.recovery_path !== 'string') { fail('A cópia de recuperação não foi confirmada.'); return; }
      if (typeof value.recovery_path === 'string') { $('recovery-location').textContent = 'Cópia local de recuperação: ' + value.recovery_path; $('recovery-location').hidden = false; }
      attached = true; activeRoom = value.room_id; editor.readOnly = false; $('connect').hidden = true;
      $('session-controls').hidden = false;
      $('create-invite').hidden = value.type !== 'created'; $('sharing').hidden = true; status('Conectado — experimental');
    } else if (value.type === 'disconnect_blocked' && leaving) {
      leaving = false; $('leave-session').disabled = false; editor.readOnly = false;
      if (externalBeforeLeave !== null) { const before = externalBeforeLeave; externalBeforeLeave = null; helperEdit(before, model); }
      status('Há alterações pendentes. Aguarde a sincronização antes de sair.');
    } else if (value.type === 'disconnected' && leaving) {
      send('pty-kill');
      attached = false; leaving = false; editor.readOnly = true; activeRoom = ''; latestHelperText = null;
      $('connect').hidden = false; $('start').disabled = false; $('leave-session').disabled = false;
      $('session-controls').hidden = true; $('sharing').hidden = true; $('invite-output').value = '';
      status(externalBeforeLeave === null ? 'Sessão encerrada.' : 'Sessão encerrada — alterações externas não foram enviadas.');
      externalBeforeLeave = null;
    } else if (value.type === 'invite') {
      $('invite-output').value = JSON.stringify({ version: 1, room: activeRoom, token: value.token });
      $('sharing').hidden = false;
      status('Convite criado. Ele pode ser usado uma vez.');
    } else if (value.type === 'edited') {
      outstanding.delete(value.id); latestHelperText = value.text; status(value.pending ? 'Alterações pendentes' : 'Conectado'); reconcile();
    } else if (value.type === 'remote' || value.type === 'synced') {
      latestHelperText = value.text; status(value.pending ? 'Alterações pendentes' : 'Conectado'); reconcile();
    } else if (value.type === 'connection_error') {
      if (typeof value.message === 'string' && value.message.includes('HTTP 507')) {
        status('Servidor sem espaço — alterações pendentes');
      } else {
        status('Sem conexão ou sincronização — alterações podem estar pendentes');
      }
    } else if (value.type === 'error') {
      fail(value.message || 'O auxiliar recusou a operação.');
    }
  }
  openidea.onMessage((raw) => {
    let message; try { message = JSON.parse(raw); } catch { return; }
    if (message.sessionId !== sessionId) return;
    if (message.op === 'pty-error' || message.op === 'pty-exit') {
      fail('O auxiliar foi encerrado ou não recebeu permissão. Preserve as alterações locais.'); return;
    }
    if (message.op !== 'pty-data') return;
    buffer += message.data;
    if (buffer.length > 65536) { send('pty-kill'); fail('Resposta excessiva do auxiliar.'); return; }
    let newline;
    while ((newline = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, newline).replace(/\r$/, ''); buffer = buffer.slice(newline + 1);
      try { onFrame(JSON.parse(line)); } catch { send('pty-kill'); fail('Resposta inválida do auxiliar.'); }
    }
    send('pty-ack', { sequence: message.sequence });
  });
  $('connect').addEventListener('submit', (event) => {
    event.preventDefault();
    if (attached || stopped) return;
    requestedMode = $('mode').value;
    if (requestedMode === 'join') {
      try {
        const value = JSON.parse($('invite-input').value);
        if (value.version !== 1 || typeof value.room !== 'string' || typeof value.token !== 'string') throw new Error('invalid');
        pendingInvite = { room: value.room, token: value.token };
        $('invite-input').value = '';
      } catch { $('error').textContent = 'Convite inválido.'; $('error').hidden = false; return; }
    }
    $('start').disabled = true; status('Iniciando auxiliar…');
    send('pty-spawn', { program: 'openidea-collab-helper', args: [], cols: 100, rows: 28 });
  });
  $('mode').addEventListener('change', () => { $('invite-field').hidden = $('mode').value !== 'join'; });
  $('leave-session').addEventListener('click', () => {
    if (!attached || stopped || leaving) return;
    if (queuedInputs || outstanding.size || editor.value !== model) { status('Aguarde o processamento das alterações antes de sair.'); return; }
    leaving = true; editor.readOnly = true; $('leave-session').disabled = true;
    status('Verificando alterações pendentes…');
    send('pty-input', { data: JSON.stringify({ type: 'disconnect', id: 'leave' }) + '\n' });
  });
  $('create-invite').addEventListener('click', () => {
    send('pty-input', { data: JSON.stringify({ type: 'invite', id: 'invite', role: 'editor', ttl_seconds: 600 }) + '\n' });
  });
  $('clear-invite').addEventListener('click', () => { $('invite-output').value = ''; $('sharing').hidden = true; });
  openidea.document.get().then((state) => {
    version = state.version; model = state.text; editor.value = state.text;
    if (state.readOnly) { fail('Este documento está somente para leitura.'); return; }
    status('Informe o servidor e a credencial para conectar.');
  }).catch(() => fail('Não foi possível abrir o documento.'));
})();
