//! OpenIDEA Collab plugin: real-time file sync, presence, and cursor decorations.

pub mod client;
pub mod presence;
pub mod room;
pub mod sync;

use openidea_plugin::{
    host, register_plugin, Context, Error, Event, EventKind, Invocation,
    NotificationLevel, Plugin,
    views::{Body, Button, View, ViewEvent, Viewport, Views},
};
use client::CollabClient;
use sync::SyncState;
use room::RoomState;

const DEFAULT_SERVER_URL: &str = "http://localhost:4444";
const STORAGE_KEY_SERVER: &str = "server_url";
const STORAGE_KEY_USER_NAME: &str = "user_name";
const STORAGE_KEY_USER_GITHUB: &str = "user_github";
const VIEW_COLLAB_PANEL: &str = "collab-panel";

const IS_DEV: bool = cfg!(collab_dev);

// Placeholders replaced at render time: __SERVER_URL__ __IS_DEV__ __INIT_NAME__ __INIT_GITHUB__
const PANEL_HTML: &str = r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>OpenIDEA Collab</title>
<style>
:root {
  --bg:     var(--vscode-sideBar-background,      #1e1e1e);
  --fg:     var(--vscode-sideBar-foreground,      #cccccc);
  --accent: var(--vscode-focusBorder,             #007fd4);
  --card:   var(--vscode-editorWidget-background, #252526);
  --border: var(--vscode-editorWidget-border,     #3c3c3c);
  --input:  var(--vscode-input-background,        #3c3c3c);
  --muted:  var(--vscode-descriptionForeground,   #888);
  --green:  #4caf50;
  --danger: #e05252;
  --radius: 6px;
  font-family: var(--vscode-font-family, system-ui, sans-serif);
  font-size: 13px;
}
*{box-sizing:border-box;margin:0;padding:0}
body{background:var(--bg);color:var(--fg);padding:12px 10px 20px}

/* ── header ── */
.hdr{display:flex;align-items:center;gap:6px;margin-bottom:14px}
.hdr-logo{font-size:14px;font-weight:700;white-space:nowrap}
.hdr-spacer{flex:1}
.badge{font-size:10px;font-weight:700;padding:1px 6px;border-radius:3px;white-space:nowrap}
.badge-dev{background:#c4860018;color:#c48600;border:1px solid #c4860038}
.badge-pub{background:#007fd418;color:#007fd4;border:1px solid #007fd438}
.gear-btn{cursor:pointer;background:none;border:none;color:var(--muted);
          font-size:14px;padding:2px 4px;border-radius:4px;line-height:1;transition:color .15s}
.gear-btn:hover{color:var(--fg)}

/* ── server drawer ── */
#server-drawer{background:var(--card);border:1px solid var(--border);
               border-radius:var(--radius);padding:10px;margin-bottom:12px}
.drawer-row{display:flex;gap:6px;align-items:center}
.drawer-lbl{font-size:10px;text-transform:uppercase;letter-spacing:.5px;
            color:var(--muted);margin-bottom:6px}

/* ── inputs ── */
input[type=text]{
  flex:1;background:var(--input);border:1px solid var(--border);
  color:var(--fg);border-radius:4px;padding:5px 8px;font-size:12px;
  outline:none;min-width:0;width:100%
}
input[type=text]:focus{border-color:var(--accent)}
input[type=text]::placeholder{color:var(--muted)}

/* ── profile card ── */
#profile-card{
  background:var(--card);border:1px solid var(--border);
  border-radius:var(--radius);padding:12px;
  display:flex;align-items:center;gap:12px;margin-bottom:14px
}
#profile-av{
  width:52px;height:52px;border-radius:50%;flex-shrink:0;
  display:flex;align-items:center;justify-content:center;
  font-size:18px;font-weight:700;color:#fff;user-select:none
}
.profile-fields{flex:1;min-width:0;display:flex;flex-direction:column;gap:5px}
.profile-fields input{font-size:12px}
.profile-fields input:first-child{font-weight:600}

/* ── divider ── */
.divider{display:flex;align-items:center;gap:8px;margin:10px 0;color:var(--muted);font-size:11px}
.divider::before,.divider::after{content:'';flex:1;height:1px;background:var(--border)}

/* ── buttons ── */
.btn{
  width:100%;padding:8px 12px;border-radius:var(--radius);
  border:none;cursor:pointer;font-size:13px;font-weight:600;
  text-align:center;transition:filter .15s,opacity .15s;letter-spacing:.01em
}
.btn:hover:not(:disabled){filter:brightness(1.12)}
.btn:active:not(:disabled){filter:brightness(.9)}
.btn:disabled{opacity:.4;cursor:default}
.btn-primary{background:var(--accent);color:#fff}
.btn-outline{background:transparent;color:var(--fg);border:1px solid var(--border)}
.btn-outline:hover:not(:disabled){background:var(--card)}
.btn-danger{background:#e0525212;color:var(--danger);border:1px solid #e0525230}

/* ── error / status ── */
#err{
  background:#e0525210;border:1px solid #e0525230;border-radius:var(--radius);
  color:#f08080;font-size:12px;padding:7px 10px;margin-top:8px;line-height:1.4
}
#loading{
  display:flex;align-items:center;justify-content:center;gap:8px;
  color:var(--muted);font-size:12px;padding:6px 0;margin-top:4px
}
.spinner{
  width:14px;height:14px;border:2px solid var(--border);
  border-top-color:var(--accent);border-radius:50%;
  animation:spin .7s linear infinite;flex-shrink:0
}
@keyframes spin{to{transform:rotate(360deg)}}

/* ── room join row ── */
.join-row{display:flex;flex-direction:column;gap:6px}

/* ══ CONNECTED ════════════════════════════════════════════════════ */
/* room card */
.room-card{
  background:var(--card);border:1px solid var(--border);
  border-radius:var(--radius);padding:12px;margin-bottom:12px
}
.room-status-row{display:flex;align-items:center;gap:6px;margin-bottom:8px}
#sync-dot{
  width:8px;height:8px;border-radius:50%;
  background:var(--muted);flex-shrink:0
}
#sync-dot.on{background:var(--green);animation:pulse 1.6s ease-in-out infinite}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:.3}}
.room-status-label{font-size:11px;color:var(--muted);flex:1}
#conn-badge{display:none}

.room-id-row{
  display:flex;align-items:center;gap:6px;
  background:var(--bg);border:1px solid var(--border);
  border-radius:4px;padding:6px 10px;margin-bottom:6px
}
.room-id-val{
  flex:1;font-family:monospace;font-size:13px;font-weight:700;
  overflow:hidden;text-overflow:ellipsis;white-space:nowrap;letter-spacing:.04em
}
.copy-btn{
  flex-shrink:0;cursor:pointer;background:none;border:none;
  color:var(--muted);font-size:16px;padding:0 2px;line-height:1;transition:color .15s
}
.copy-btn:hover{color:var(--fg)}
.copy-btn.ok{color:var(--green)!important}

.room-hint{font-size:11px;color:var(--muted);line-height:1.5}
.room-server{font-size:11px;color:var(--muted);margin-top:6px;
             overflow:hidden;text-overflow:ellipsis;white-space:nowrap}

/* peers */
.section-hdr{
  display:flex;align-items:center;gap:6px;margin-bottom:6px;
  font-size:10px;text-transform:uppercase;letter-spacing:.5px;color:var(--muted)
}
#peer-count{background:var(--card);border:1px solid var(--border);
            padding:1px 5px;border-radius:10px;font-size:10px;color:var(--muted)}
.peer{
  display:flex;align-items:center;gap:10px;
  padding:7px 6px;border-radius:4px;transition:background .15s
}
.peer:hover{background:var(--card)}
.peer-av{
  width:30px;height:30px;border-radius:50%;flex-shrink:0;
  display:flex;align-items:center;justify-content:center;
  font-size:11px;font-weight:700;color:#fff
}
.peer-info{flex:1;min-width:0}
.peer-name{font-weight:600;font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.peer-detail{color:var(--muted);font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;margin-top:1px}
#empty{color:var(--muted);text-align:center;padding:16px 0;font-size:12px}
</style>
</head>
<body>

<!-- ══ NOT CONNECTED ══════════════════════════════════════════════ -->
<div id="nc">

  <div class="hdr">
    <span class="hdr-logo">⟡ OpenIDEA Collab</span>
    <span class="hdr-spacer"></span>
    <span id="nc-badge" class="badge"></span>
    <button class="gear-btn" title="Configurações do servidor" onclick="toggleSettings()">⚙</button>
  </div>

  <!-- server drawer (collapsed by default) -->
  <div id="server-drawer" hidden>
    <div class="drawer-lbl">Servidor</div>
    <div class="drawer-row">
      <input type="text" id="inp-server" value="__SERVER_URL__" placeholder="http://localhost:4444">
      <button class="btn btn-outline" style="width:auto;padding:5px 10px;font-size:12px"
              onclick="doSetServer()">Salvar</button>
    </div>
  </div>

  <!-- profile card -->
  <div id="profile-card">
    <div id="profile-av">?</div>
    <div class="profile-fields">
      <input type="text" id="inp-name"   placeholder="Seu nome"            value="__INIT_NAME__">
      <input type="text" id="inp-github" placeholder="@github (opcional)"  value="__INIT_GITHUB__">
    </div>
  </div>

  <button class="btn btn-primary" id="btn-create" onclick="doCreate()">＋  Criar Nova Sala</button>

  <div class="divider">ou entrar numa sala</div>

  <div class="join-row">
    <input type="text" id="inp-room" placeholder="Room ID">
    <button class="btn btn-outline" id="btn-join" onclick="doJoin()">→  Entrar na Sala</button>
  </div>

  <div id="err" hidden></div>
  <div id="loading" hidden><div class="spinner"></div><span id="loading-text"></span></div>

</div><!-- /nc -->

<!-- ══ CONNECTED ════════════════════════════════════════════════════ -->
<div id="app" hidden>

  <div class="hdr">
    <span class="hdr-logo">⟡ OpenIDEA Collab</span>
    <span class="hdr-spacer"></span>
    <span id="conn-badge" class="badge"></span>
  </div>

  <!-- room card -->
  <div class="room-card">
    <div class="room-status-row">
      <span id="sync-dot"></span>
      <span class="room-status-label">Sala ativa — sincronizando arquivos</span>
    </div>
    <div class="room-id-row">
      <span class="room-id-val" id="room-id">—</span>
      <button class="copy-btn" title="Copiar Room ID" onclick="cp('room-id',this)">⎘</button>
    </div>
    <div class="room-hint">
      Compartilhe o Room ID acima com seu colaborador.<br>
      Ele abre o painel OpenIDEA Collab → "Entrar na Sala" → cola o ID.
    </div>
    <div class="room-server" id="conn-server"></div>
  </div>

  <!-- peer list -->
  <div class="section-hdr">
    Colaboradores
    <span id="peer-count"></span>
  </div>
  <div id="peers"></div>
  <div id="empty" hidden>Aguardando colaboradores entrar…</div>

  <div style="margin-top:14px">
    <button class="btn btn-danger" onclick="doLeave()">↩  Sair da Sala</button>
  </div>

</div><!-- /app -->

<script>
var IS_DEV = __IS_DEV__;
var initServerUrl = '__SERVER_URL__';
var $nc  = document.getElementById('nc');
var $app = document.getElementById('app');
var settingsOpen = false;

function toggleSettings() {
  settingsOpen = !settingsOpen;
  document.getElementById('server-drawer').hidden = !settingsOpen;
}

/* ── messaging ───────────────────────────────────────────────────── */
function sendMsg(obj) {
  try {
    if (typeof openidea !== 'undefined') {
      openidea.postMessage(JSON.stringify(obj));
    } else {
      showErr('API do OpenIDEA não disponível — reabra o painel e tente novamente.');
    }
  } catch(e) { showErr('Erro: ' + e.message); }
}

/* ── error / loading ─────────────────────────────────────────────── */
function showErr(msg) {
  var e = document.getElementById('err');
  e.textContent = msg; e.hidden = false;
}
function hideErr() { document.getElementById('err').hidden = true; }

function setLoading(on, text) {
  var el = document.getElementById('loading');
  el.hidden = !on;
  if (on) document.getElementById('loading-text').textContent = text || '';
  document.getElementById('btn-create').disabled = on;
  document.getElementById('btn-join').disabled = on;
}

/* ── avatar ──────────────────────────────────────────────────────── */
function hashColor(s) {
  if (!s) return '#607d8b';
  var h = 0;
  for (var i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) & 0xffff;
  return 'hsl(' + (h % 360) + ',40%,40%)';
}
function initials(name) {
  if (!name) return '?';
  return name.trim().split(/\s+/).map(function(w){return w[0]||'';}).join('').toUpperCase().slice(0,2) || name[0].toUpperCase();
}
function updateAvatar() {
  var name = document.getElementById('inp-name').value;
  var gh   = document.getElementById('inp-github').value.trim();
  var av   = document.getElementById('profile-av');
  var seed = gh || name;
  av.style.background = hashColor(seed);
  av.textContent = initials(name);
}
document.getElementById('inp-name').addEventListener('input', updateAvatar);
document.getElementById('inp-github').addEventListener('input', updateAvatar);

/* ── clipboard ───────────────────────────────────────────────────── */
function cp(id, btn) {
  var val = document.getElementById(id).textContent;
  function ok() {
    btn.classList.add('ok'); btn.textContent = '✓';
    setTimeout(function(){ btn.classList.remove('ok'); btn.textContent = '⎘'; }, 1600);
  }
  function fb() {
    var t = document.createElement('input');
    t.value = val; document.body.appendChild(t); t.select();
    try { document.execCommand('copy'); ok(); } catch(_) {}
    document.body.removeChild(t);
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(val).then(ok, fb);
  } else { fb(); }
}

/* ── badge ───────────────────────────────────────────────────────── */
function setBadge(el, dev) {
  if (!el) return;
  el.textContent = dev ? 'DEV' : '';
  el.className = 'badge ' + (dev ? 'badge-dev' : 'badge-pub');
  el.style.display = dev ? '' : 'none';
}

/* ── actions ─────────────────────────────────────────────────────── */
function doSetServer() {
  var url = document.getElementById('inp-server').value.trim().replace(/\/+$/, '');
  if (!url) { showErr('URL não pode ser vazia'); return; }
  hideErr();
  sendMsg({ type: 'set-server', url: url });
}

function doCreate() {
  var name   = document.getElementById('inp-name').value.trim();
  var github = document.getElementById('inp-github').value.trim().replace(/^@/, '');
  if (!name) { showErr('Informe seu nome antes de criar a sala.'); return; }
  hideErr(); setLoading(true, 'Criando sala…');
  sendMsg({ type: 'create-room', name: name, github: github || null });
}

function doJoin() {
  var room_id = document.getElementById('inp-room').value.trim();
  var name    = document.getElementById('inp-name').value.trim();
  var github  = document.getElementById('inp-github').value.trim().replace(/^@/, '');
  if (!room_id) { showErr('Cole o Room ID que seu colaborador compartilhou.'); return; }
  if (!name)    { showErr('Informe seu nome antes de entrar.'); return; }
  hideErr(); setLoading(true, 'Entrando na sala…');
  sendMsg({ type: 'join-room', room_id: room_id, name: name, github: github || null });
}

function doLeave() {
  sendMsg({ type: 'leave-room' });
}

/* ── render ──────────────────────────────────────────────────────── */
function render(state) {
  if (!state || !state.room_id) {
    $nc.hidden = false; $app.hidden = true;
    setLoading(false); return;
  }
  $nc.hidden = true; $app.hidden = false;

  document.getElementById('room-id').textContent = state.room_id;
  document.getElementById('sync-dot').className  = state.sync_active ? 'on' : '';
  document.getElementById('conn-server').textContent = state.server_url ? '🌐 ' + state.server_url : '';
  setBadge(document.getElementById('conn-badge'), state.is_dev);

  var peers = state.peers || [];
  var pc = document.getElementById('peer-count');
  pc.textContent = peers.length ? peers.length : '';
  pc.style.display = peers.length ? '' : 'none';

  var pl = document.getElementById('peers');
  pl.innerHTML = '';
  peers.forEach(function(p) {
    var row = document.createElement('div');
    row.className = 'peer';
    var av = document.createElement('div');
    av.className = 'peer-av';
    av.style.background = hashColor(p.github || p.name || '');
    av.textContent = initials(p.name);
    var inf = document.createElement('div');
    inf.className = 'peer-info';
    var nm = document.createElement('div');
    nm.className = 'peer-name';
    nm.textContent = p.name + (p.github ? ' @' + p.github : '');
    inf.appendChild(nm);
    if (p.file) {
      var dt = document.createElement('div');
      dt.className = 'peer-detail';
      dt.textContent = (p.line != null) ? p.file + ':' + (p.line + 1) : p.file;
      inf.appendChild(dt);
    }
    row.appendChild(av); row.appendChild(inf);
    pl.appendChild(row);
  });
  document.getElementById('empty').hidden = peers.length > 0;
}

/* ── boot ────────────────────────────────────────────────────────── */
setBadge(document.getElementById('nc-badge'), IS_DEV);
updateAvatar();

function setupListener() {
  try {
    if (typeof openidea !== 'undefined') {
      openidea.onMessage(function(raw) {
        try {
          var msg = typeof raw === 'string' ? JSON.parse(raw) : raw;
          if (msg.type === 'state')        { render(msg.state); setLoading(false); }
          else if (msg.type === 'error')   { showErr(msg.message); setLoading(false); }
          else if (msg.type === 'server-saved') {
            document.getElementById('inp-server').value = msg.url;
          }
        } catch(_) {}
      });
      return true;
    }
  } catch(_) {}
  return false;
}
if (!setupListener()) {
  setTimeout(function(){ if (!setupListener()) setTimeout(setupListener, 800); }, 150);
}
</script>
</body>
</html>"#;

// ── git identity helpers ──────────────────────────────────────────────────────

fn git_config_value(text: &str, section: &str, subsection: Option<&str>, key: &str) -> Option<String> {
    let header = match subsection {
        Some(sub) => format!("[{} \"{}\"]", section, sub),
        None => format!("[{}]", section),
    };
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed.to_lowercase() == header.to_lowercase();
            continue;
        }
        if in_section {
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            if parts.len() == 2 && parts[0].trim().eq_ignore_ascii_case(key) {
                return Some(parts[1].trim().to_owned());
            }
        }
    }
    None
}

fn git_config_github(text: &str) -> Option<String> {
    let url = git_config_value(text, "remote", Some("origin"), "url")?;
    let rest = if let Some(r) = url.strip_prefix("https://github.com/") {
        r
    } else if let Some(r) = url.strip_prefix("git@github.com:") {
        r
    } else {
        return None;
    };
    let username = rest.split('/').next()?;
    if username.is_empty() { return None; }
    Some(username.to_owned())
}

fn detect_git_identity(ctx: &Invocation) -> (String, String) {
    let project = match ctx.project() {
        Some(p) => p,
        None => return (String::new(), String::new()),
    };
    let cfg = match project.read_file(".git/config") {
        Ok(c) => c,
        Err(_) => return (String::new(), String::new()),
    };
    let name = git_config_value(&cfg, "user", None, "name").unwrap_or_default();
    let github = git_config_github(&cfg).unwrap_or_default();
    (name, github)
}

// ── plugin ────────────────────────────────────────────────────────────────────

pub struct CollabPlugin {
    sync: SyncState,
    client: CollabClient,
}

impl Default for CollabPlugin {
    fn default() -> Self {
        Self {
            sync: SyncState::default(),
            client: CollabClient::new(DEFAULT_SERVER_URL.to_owned()),
        }
    }
}

impl CollabPlugin {
    fn send_panel_error(&self, msg: &str) {
        let payload = serde_json::json!({ "type": "error", "message": msg });
        if let Ok(s) = serde_json::to_string(&payload) {
            let _ = host::post_view_message(VIEW_COLLAB_PANEL, &s);
        }
    }

    fn panel_send_state(&self) {
        let room_id = self.sync.room.as_ref().map(|r| r.room_id.as_str()).unwrap_or("");
        let server_url = self.sync.room.as_ref()
            .map(|r| r.server_url.as_str())
            .unwrap_or(DEFAULT_SERVER_URL);
        let payload = self.sync.presence.build_panel_json(
            room_id,
            self.sync.sync_active,
            server_url,
            IS_DEV,
        );
        if let Ok(msg) = serde_json::to_string(&payload) {
            let _ = host::post_view_message(VIEW_COLLAB_PANEL, &msg);
        }
        host::refresh_view(VIEW_COLLAB_PANEL);
    }

    fn panel_create_room(&mut self, name: String, github: Option<String>) {
        // Persist profile for future sessions
        let _ = host::storage::set(STORAGE_KEY_USER_NAME, &name);
        if let Some(ref gh) = github {
            let _ = host::storage::set(STORAGE_KEY_USER_GITHUB, gh);
        }

        let room_id = match self.client.create_room() {
            Ok(id) => id,
            Err(e) => { self.send_panel_error(&format!("Erro ao criar sala: {e}")); return; }
        };
        let user_id = match self.client.join_room(&room_id, &name) {
            Ok(id) => id,
            Err(e) => { self.send_panel_error(&format!("Erro ao entrar na sala: {e}")); return; }
        };
        let server_url = self.client.server_url.clone();
        let room = RoomState {
            room_id: room_id.clone(),
            user_id: user_id.clone(),
            name: name.clone(),
            server_url,
            github: github.clone(),
        };
        let mut pstate = presence::PresenceState::new(user_id, name);
        pstate.self_github = github;
        self.sync.room = Some(room);
        self.sync.presence = pstate;
        self.sync.save_to_storage();
        self.sync.update_status_bar(&room_id);
        host::notify(NotificationLevel::Info, "Collab", &format!("Sala criada: {room_id}"));
        self.panel_send_state();
    }

    fn panel_join_room(&mut self, room_id: String, name: String, github: Option<String>) {
        let _ = host::storage::set(STORAGE_KEY_USER_NAME, &name);
        if let Some(ref gh) = github {
            let _ = host::storage::set(STORAGE_KEY_USER_GITHUB, gh);
        }

        let user_id = match self.client.join_room(&room_id, &name) {
            Ok(id) => id,
            Err(e) => { self.send_panel_error(&format!("Erro ao entrar na sala: {e}")); return; }
        };
        let server_url = self.client.server_url.clone();
        let room = RoomState {
            room_id: room_id.clone(),
            user_id: user_id.clone(),
            name: name.clone(),
            server_url,
            github: github.clone(),
        };
        let mut pstate = presence::PresenceState::new(user_id.clone(), name.clone());
        pstate.self_github = github;
        self.sync.room = Some(room);
        self.sync.presence = pstate;
        self.sync.poll_cursor = 0;
        self.sync.save_to_storage();
        if let Ok(members) = self.client.get_members(&room_id) {
            for m in members {
                if m.user_id != user_id {
                    self.sync.presence.update_peer(
                        m.user_id, m.name, m.current_file, m.github, m.line, m.col,
                    );
                }
            }
        }
        self.sync.update_status_bar(&room_id);
        self.sync.apply_file_badges();
        host::notify(NotificationLevel::Info, "Collab", &format!("Entrou na sala {room_id}"));
        self.panel_send_state();
    }

    fn panel_leave_room(&mut self) {
        if let Some(ref room) = self.sync.room.clone() {
            let _ = self.client.leave_room(&room.room_id, &room.user_id);
        }
        self.sync.room = None;
        self.sync.presence = presence::PresenceState::new(String::new(), String::new());
        self.sync.file_versions.clear();
        self.sync.poll_cursor = 0;
        self.sync.sync_active = false;
        self.sync.save_to_storage();
        let _ = host::clear_decorations("collab-cursors");
        let _ = host::clear_file_decorations("collab-open");
        host::remove_status_item("collab-status");
        host::notify(NotificationLevel::Info, "Collab", "Saiu da sala");
        self.panel_send_state();
    }
}

impl Plugin for CollabPlugin {
    fn activate(&mut self, _ctx: &Context) -> Result<(), Error> {
        self.sync = SyncState::load_from_storage();
        let server_url = host::storage::get(STORAGE_KEY_SERVER)
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_SERVER_URL.to_owned());
        self.client = CollabClient::new(server_url);

        host::subscribe(EventKind::FileSaved)?;
        host::subscribe(EventKind::EditorFocused)?;
        host::subscribe(EventKind::Scheduled)?;

        if let Some(ref room) = self.sync.room.clone() {
            self.sync.update_status_bar(&room.room_id);
        }
        Ok(())
    }

    fn run_command(
        &mut self,
        id: &str,
        _ctx: &Invocation,
        _arg: Option<&str>,
    ) -> Result<Option<String>, Error> {
        match id {
            "set-server" => {
                let current = self.client.server_url.clone();
                let url = host::input("Server URL", Some(&current))?
                    .unwrap_or(current);
                let url = url.trim().trim_end_matches('/').to_owned();
                let _ = host::storage::set(STORAGE_KEY_SERVER, &url);
                self.client = CollabClient::new(url.clone());
                host::notify(NotificationLevel::Info, "Collab", &format!("Servidor: {url}"));
                host::refresh_view(VIEW_COLLAB_PANEL);
                Ok(None)
            }
            "create-room" => {
                let room_id =
                    self.client.create_room().map_err(|e| Error::Failed(e.to_string()))?;
                let name = host::input("Seu nome", None)?
                    .unwrap_or_else(|| "Anonymous".to_owned());
                let github = host::input("GitHub username (opcional)", None)?
                    .filter(|s| !s.trim().is_empty());
                let user_id = self
                    .client
                    .join_room(&room_id, &name)
                    .map_err(|e| Error::Failed(e.to_string()))?;
                let server_url = self.client.server_url.clone();
                let room = RoomState {
                    room_id: room_id.clone(),
                    user_id: user_id.clone(),
                    name: name.clone(),
                    server_url,
                    github: github.clone(),
                };
                let mut pstate = presence::PresenceState::new(user_id, name);
                pstate.self_github = github;
                self.sync.room = Some(room);
                self.sync.presence = pstate;
                self.sync.save_to_storage();
                self.sync.update_status_bar(&room_id);
                host::notify(NotificationLevel::Info, "Collab", &format!("Sala criada: {room_id}"));
                self.sync.push_panel_state(&room_id);
                Ok(Some(room_id))
            }
            "join-room" => {
                let room_id = match host::input("Room ID", None)? {
                    Some(r) if !r.trim().is_empty() => r.trim().to_owned(),
                    _ => return Err(Error::Cancelled),
                };
                let name = host::input("Seu nome", None)?
                    .unwrap_or_else(|| "Anonymous".to_owned());
                let github = host::input("GitHub username (opcional)", None)?
                    .filter(|s| !s.trim().is_empty());
                let user_id = self
                    .client
                    .join_room(&room_id, &name)
                    .map_err(|e| Error::Failed(e.to_string()))?;
                let server_url = self.client.server_url.clone();
                let room = RoomState {
                    room_id: room_id.clone(),
                    user_id: user_id.clone(),
                    name: name.clone(),
                    server_url,
                    github: github.clone(),
                };
                let mut pstate = presence::PresenceState::new(user_id.clone(), name.clone());
                pstate.self_github = github;
                self.sync.room = Some(room);
                self.sync.presence = pstate;
                self.sync.poll_cursor = 0;
                self.sync.save_to_storage();
                if let Ok(members) = self.client.get_members(&room_id) {
                    for m in members {
                        if m.user_id != user_id {
                            self.sync.presence.update_peer(
                                m.user_id, m.name, m.current_file, m.github, m.line, m.col,
                            );
                        }
                    }
                }
                self.sync.update_status_bar(&room_id);
                self.sync.apply_file_badges();
                host::notify(NotificationLevel::Info, "Collab", &format!("Entrou na sala {room_id}"));
                self.sync.push_panel_state(&room_id);
                Ok(None)
            }
            "leave-room" => {
                self.panel_leave_room();
                Ok(None)
            }
            _ => Err(Error::NotFound(format!("unknown command {id}"))),
        }
    }

    fn handle_events(&mut self, events: &[Event], ctx: &Invocation) {
        for event in events {
            match event {
                Event::FileSaved(path) => {
                    let sync = &mut self.sync;
                    let client = &self.client;
                    sync.on_file_saved(client, ctx, path);
                }
                Event::EditorFocused(maybe_path) => {
                    if let Some(ref room) = self.sync.room.clone() {
                        let (line, col) = ctx.editor()
                            .and_then(|e| e.selections().ok())
                            .and_then(|sels| sels.into_iter().next())
                            .map(|r| (r.start.line, r.start.character))
                            .unwrap_or((0, 0));
                        let github = self.sync.room.as_ref().and_then(|r| r.github.clone());
                        let _ = self.client.update_presence(
                            &room.room_id,
                            &room.user_id,
                            maybe_path.as_deref(),
                            github.as_deref(),
                            Some(line),
                            Some(col),
                        );
                    }
                }
                Event::Scheduled => {
                    let sync = &mut self.sync;
                    let client = &self.client;
                    sync.on_poll_tick(client, ctx);
                }
                _ => {}
            }
        }
    }
}

impl Views for CollabPlugin {
    fn render(
        &mut self,
        view: &str,
        _viewport: Option<Viewport>,
        ctx: &Invocation,
    ) -> Result<View, Error> {
        if view != VIEW_COLLAB_PANEL {
            return Err(Error::NotFound(format!("unknown view {view}")));
        }

        let server_url = host::storage::get(STORAGE_KEY_SERVER)
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_SERVER_URL.to_owned());

        // Prefer stored profile, fall back to git identity
        let stored_name = host::storage::get(STORAGE_KEY_USER_NAME).ok().flatten().unwrap_or_default();
        let stored_github = host::storage::get(STORAGE_KEY_USER_GITHUB).ok().flatten().unwrap_or_default();
        let (git_name, git_github) = if stored_name.is_empty() || stored_github.is_empty() {
            detect_git_identity(ctx)
        } else {
            (String::new(), String::new())
        };
        let init_name   = if stored_name.is_empty()   { git_name }   else { stored_name };
        let init_github = if stored_github.is_empty() { git_github } else { stored_github };

        let html = PANEL_HTML
            .replace("__SERVER_URL__", &server_url)
            .replace("__IS_DEV__", if IS_DEV { "true" } else { "false" })
            .replace("__INIT_NAME__", &init_name.replace('"', "&quot;"))
            .replace("__INIT_GITHUB__", &init_github.replace('"', "&quot;"));

        let view = View::new(Body::Html(html));
        let view = if self.sync.room.is_some() {
            view.button(Button::new("leave-room", "Leave Room").icon("close"))
        } else {
            view
                .button(Button::new("create-room", "Create Room").icon("add"))
                .button(Button::new("join-room", "Join Room").icon("arrow-right"))
        };
        Ok(view)
    }

    fn handle(&mut self, view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        if view != VIEW_COLLAB_PANEL {
            return Ok(());
        }
        match event {
            ViewEvent::Button(ref id) => {
                match id.as_str() {
                    "leave-room" => self.panel_leave_room(),
                    "create-room" => {
                        // Toolbar button: use stored name or prompt via run_command
                        let name = host::storage::get(STORAGE_KEY_USER_NAME)
                            .ok().flatten().unwrap_or_default();
                        if name.is_empty() {
                            let _ = self.run_command("create-room", ctx, None);
                        } else {
                            let github = host::storage::get(STORAGE_KEY_USER_GITHUB)
                                .ok().flatten().filter(|s| !s.is_empty());
                            self.panel_create_room(name, github);
                        }
                    }
                    "join-room" => {
                        let _ = self.run_command("join-room", ctx, None);
                    }
                    _ => {}
                }
            }
            ViewEvent::Message(ref raw) => {
                let msg: serde_json::Value = match serde_json::from_str(raw) {
                    Ok(v) => v,
                    Err(_) => return Ok(()),
                };
                match msg.get("type").and_then(|t| t.as_str()) {
                    Some("create-room") => {
                        let name = msg["name"].as_str().unwrap_or("Anonymous").to_owned();
                        let github = msg["github"].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
                        self.panel_create_room(name, github);
                    }
                    Some("join-room") => {
                        let room_id = match msg["room_id"].as_str() {
                            Some(r) if !r.trim().is_empty() => r.trim().to_owned(),
                            _ => { self.send_panel_error("Room ID inválido"); return Ok(()); }
                        };
                        let name = msg["name"].as_str().unwrap_or("Anonymous").to_owned();
                        let github = msg["github"].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
                        self.panel_join_room(room_id, name, github);
                    }
                    Some("leave-room") => {
                        self.panel_leave_room();
                    }
                    Some("set-server") => {
                        let url = match msg["url"].as_str() {
                            Some(u) if !u.trim().is_empty() => u.trim().trim_end_matches('/').to_owned(),
                            _ => { self.send_panel_error("URL inválida"); return Ok(()); }
                        };
                        let _ = host::storage::set(STORAGE_KEY_SERVER, &url);
                        self.client = CollabClient::new(url.clone());
                        host::notify(NotificationLevel::Info, "Collab", &format!("Servidor: {url}"));
                        // Notify panel the URL was saved
                        let ack = serde_json::json!({ "type": "server-saved", "url": url });
                        if let Ok(s) = serde_json::to_string(&ack) {
                            let _ = host::post_view_message(VIEW_COLLAB_PANEL, &s);
                        }
                    }
                    Some("command") => {
                        // Legacy path kept for compatibility
                        if let Some(id) = msg["id"].as_str() {
                            let _ = self.run_command(id, ctx, None);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}

register_plugin!(CollabPlugin, views);
