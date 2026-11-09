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
const VIEW_COLLAB_PANEL: &str = "collab-panel";

const IS_DEV: bool = cfg!(collab_dev);

// Placeholders replaced at render time: __SERVER_URL__  __IS_DEV__
const PANEL_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Collaborators</title>
<style>
  :root {
    --bg:     var(--vscode-sideBar-background,      #1e1e1e);
    --fg:     var(--vscode-sideBar-foreground,      #cccccc);
    --accent: var(--vscode-focusBorder,             #007fd4);
    --card:   var(--vscode-editorWidget-background, #252526);
    --border: var(--vscode-editorWidget-border,     #3c3c3c);
    --muted:  var(--vscode-descriptionForeground,   #888);
    --green:  #4caf50;
    font-family: var(--vscode-font-family, system-ui, sans-serif);
    font-size: var(--vscode-font-size, 13px);
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { background: var(--bg); color: var(--fg); padding: 10px; }

  /* ── copy row ── */
  .copy-row {
    display: flex; align-items: center; gap: 6px;
    background: var(--card); border: 1px solid var(--border);
    border-radius: 4px; padding: 5px 8px;
  }
  .copy-val {
    flex: 1; font-family: monospace; font-size: 12px;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .copy-btn {
    flex-shrink: 0; cursor: pointer; background: none; border: none;
    color: var(--muted); font-size: 15px; padding: 0 2px; line-height: 1;
    transition: color .15s;
  }
  .copy-btn:hover { color: var(--fg); }
  .copy-btn.ok    { color: var(--green) !important; }

  /* ── label ── */
  .lbl { font-size: 10px; text-transform: uppercase; letter-spacing: .6px;
         color: var(--muted); margin: 10px 0 4px; }

  /* ── badge ── */
  .badge {
    display: inline-block; font-size: 10px; font-weight: 700;
    padding: 1px 5px; border-radius: 3px; vertical-align: middle;
    margin-left: 6px;
  }
  .badge-dev  { background: #c4860020; color: #c48600; border: 1px solid #c4860040; }
  .badge-pub  { background: #007fd420; color: #007fd4; border: 1px solid #007fd440; }

  /* ── not connected ── */
  #setup-title {
    display: flex; align-items: center; gap: 0;
    font-weight: 600; margin-bottom: 2px;
  }

  /* ── action buttons ── */
  .actions { display: flex; flex-direction: column; gap: 6px; margin-top: 14px; }
  .btn {
    width: 100%; padding: 7px 10px; border-radius: 4px;
    border: none; cursor: pointer; font-size: 12px; font-weight: 500;
    text-align: center; transition: opacity .15s;
  }
  .btn:hover { opacity: .82; }
  .btn:active { opacity: .65; }
  .btn-primary   { background: var(--accent); color: #fff; }
  .btn-secondary { background: var(--card); color: var(--fg); border: 1px solid var(--border); }
  .btn-danger    { background: #c0392b22; color: #c0392b; border: 1px solid #c0392b44; }

  /* ── connected ── */
  #sync-dot { width: 8px; height: 8px; border-radius: 50%;
              background: var(--muted); display: inline-block; margin-right: 6px; }
  #sync-dot.active { background: var(--green); animation: pulse 1.5s ease-in-out infinite; }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:.35} }
  #peer-count { color: var(--muted); font-size: 11px; margin-left: auto; }

  .peer { display: flex; align-items: center; gap: 10px;
          padding: 6px 4px; border-radius: 4px; transition: background .15s; }
  .peer:hover { background: var(--card); }
  .avatar { width: 32px; height: 32px; border-radius: 50%; flex-shrink: 0;
            display: flex; align-items: center; justify-content: center;
            font-size: 11px; font-weight: 700; overflow: hidden; }
  .avatar img { width: 100%; height: 100%; object-fit: cover; }
  .peer-info { flex: 1; min-width: 0; }
  .peer-name   { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .peer-detail { color: var(--muted); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  #empty { color: var(--muted); text-align: center; padding: 20px 0; font-style: italic; font-size: 12px; }
  hr { border: none; border-top: 1px solid var(--border); margin: 8px 0; }
</style>
</head>
<body>

<!-- ── NOT CONNECTED ── -->
<div id="not-connected">
  <div id="setup-title">
    ⟡ OpenIDEA Collab
    <span id="nc-badge" class="badge"></span>
  </div>

  <div class="lbl">Servidor</div>
  <div class="copy-row">
    <span class="copy-val" id="nc-server"></span>
    <button class="copy-btn" title="Copiar URL" onclick="cp('nc-server',this)">⎘</button>
  </div>

  <div class="actions">
    <button class="btn btn-primary"   onclick="sendMsg('create-room')">＋  Criar Sala</button>
    <button class="btn btn-secondary" onclick="sendMsg('join-room')">→  Entrar na Sala</button>
  </div>
</div>

<!-- ── CONNECTED ── -->
<div id="app" hidden>
  <div style="display:flex;align-items:center;margin-bottom:8px">
    <span id="sync-dot"></span>
    <strong id="room-id" style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">—</strong>
    <button class="copy-btn" title="Copiar Room ID" onclick="cp('room-id',this)">⎘</button>
    <span id="peer-count"></span>
  </div>

  <div class="copy-row" style="margin-bottom:8px">
    <span class="copy-val" id="conn-server"></span>
    <button class="copy-btn" title="Copiar URL" onclick="cp('conn-server',this)">⎘</button>
  </div>
  <span id="conn-badge" class="badge" style="display:none"></span>

  <hr>
  <div id="peers"></div>
  <div id="empty" hidden>Nenhum colaborador ainda</div>

  <div class="actions" style="margin-top:16px">
    <button class="btn btn-danger" onclick="sendMsg('leave-room')">↩  Sair da Sala</button>
  </div>
</div>

<script>
  const IS_DEV = __IS_DEV__;
  const initServerUrl = '__SERVER_URL__';

  const $nc   = document.getElementById('not-connected');
  const $app  = document.getElementById('app');
  const $ncBadge   = document.getElementById('nc-badge');
  const $connBadge = document.getElementById('conn-badge');
  const $ncServer  = document.getElementById('nc-server');
  const $connServer= document.getElementById('conn-server');
  const $dot  = document.getElementById('sync-dot');
  const $rid  = document.getElementById('room-id');
  const $pc   = document.getElementById('peer-count');
  const $pl   = document.getElementById('peers');
  const $em   = document.getElementById('empty');

  function setBadge(el, dev) {
    el.textContent = dev ? 'DEV' : 'Published';
    el.className   = 'badge ' + (dev ? 'badge-dev' : 'badge-pub');
    el.style.display = '';
  }

  // Boot: show not-connected with server URL and dev flag
  $ncServer.textContent = initServerUrl;
  setBadge($ncBadge, IS_DEV);

  function cp(id, btn) {
    const val = document.getElementById(id).textContent;
    navigator.clipboard.writeText(val).then(() => {
      btn.classList.add('ok');
      btn.textContent = '✓';
      setTimeout(() => { btn.classList.remove('ok'); btn.textContent = '⎘'; }, 1500);
    }).catch(() => {});
  }

  function render(state) {
    if (!state.room_id) {
      $nc.hidden  = false;
      $app.hidden = true;
      $ncServer.textContent = state.server_url || initServerUrl;
      return;
    }
    $nc.hidden  = true;
    $app.hidden = false;
    $rid.textContent = state.room_id;
    $dot.className   = state.sync_active ? 'active' : '';
    $connServer.textContent = state.server_url || initServerUrl;
    setBadge($connBadge, state.is_dev);

    const peers = state.peers || [];
    $pc.textContent = peers.length ? `${peers.length} peer${peers.length>1?'s':''}` : '';
    $pl.innerHTML = '';
    peers.forEach(p => {
      const row  = document.createElement('div');
      row.className = 'peer';
      const av = document.createElement('div');
      av.className = 'avatar';
      if (p.github) {
        const img = document.createElement('img');
        img.src = `https://github.com/${p.github}.png?size=40`;
        img.onerror = () => { img.remove(); av.textContent = p.initials||'?'; av.style.background = p.color||'#607d8b'; };
        av.appendChild(img);
      } else {
        av.textContent = p.initials || '?';
        av.style.background = p.color || '#607d8b';
      }
      const info = document.createElement('div');
      info.className = 'peer-info';
      const nm = document.createElement('div');
      nm.className = 'peer-name';
      nm.textContent = p.github ? `${p.name} @${p.github}` : p.name;
      info.appendChild(nm);
      if (p.file) {
        const dt = document.createElement('div');
        dt.className = 'peer-detail';
        dt.textContent = p.line != null ? `${p.file} :${p.line+1}` : p.file;
        info.appendChild(dt);
      }
      row.appendChild(av); row.appendChild(info);
      $pl.appendChild(row);
    });
    $em.hidden = peers.length > 0;
  }

  function sendMsg(cmd) {
    if (typeof openidea !== 'undefined') {
      openidea.postMessage(JSON.stringify({ type: 'command', id: cmd }));
    }
  }

  if (typeof openidea !== 'undefined') {
    openidea.onMessage(function(raw) {
      try {
        const msg = typeof raw === 'string' ? JSON.parse(raw) : raw;
        if (msg?.type === 'state') render(msg.state);
      } catch {}
    });
  }
</script>
</body>
</html>"#;

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

        // Restore status bar if already in a room.
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
                host::notify(NotificationLevel::Info, "Collab", &format!("Server set to {url}"));
                Ok(None)
            }
            "create-room" => {
                let room_id =
                    self.client.create_room().map_err(|e| Error::Failed(e.to_string()))?;
                let name = host::input("Your name", None)?
                    .unwrap_or_else(|| "Anonymous".to_owned());
                let github = host::input("GitHub username (optional)", None)?
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
                host::notify(
                    NotificationLevel::Info,
                    "Collab",
                    &format!("Room created: {room_id}"),
                );
                self.sync.push_panel_state(&room_id);
                Ok(Some(room_id))
            }
            "join-room" => {
                let room_id = match host::input("Room ID", None)? {
                    Some(r) if !r.trim().is_empty() => r.trim().to_owned(),
                    _ => return Err(Error::Cancelled),
                };
                let name = host::input("Your name", None)?
                    .unwrap_or_else(|| "Anonymous".to_owned());
                let github = host::input("GitHub username (optional)", None)?
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
                pstate.self_github = github.clone();
                self.sync.room = Some(room);
                self.sync.presence = pstate;
                self.sync.poll_cursor = 0;
                self.sync.save_to_storage();
                // Seed presence from current member list.
                if let Ok(members) = self.client.get_members(&room_id) {
                    for m in members {
                        if m.user_id != user_id {
                            self.sync.presence.update_peer(
                                m.user_id,
                                m.name,
                                m.current_file,
                                m.github,
                                m.line,
                                m.col,
                            );
                        }
                    }
                }
                self.sync.update_status_bar(&room_id);
                self.sync.apply_file_badges();
                host::notify(
                    NotificationLevel::Info,
                    "Collab",
                    &format!("Joined room {room_id}"),
                );
                self.sync.push_panel_state(&room_id);
                Ok(None)
            }
            "leave-room" => {
                if let Some(ref room) = self.sync.room.clone() {
                    let _ = self.client.leave_room(&room.room_id, &room.user_id);
                }
                self.sync.room = None;
                self.sync.presence =
                    presence::PresenceState::new(String::new(), String::new());
                self.sync.file_versions.clear();
                self.sync.poll_cursor = 0;
                self.sync.sync_active = false;
                self.sync.save_to_storage();
                let _ = host::clear_decorations("collab-cursors");
                let _ = host::clear_file_decorations("collab-open");
                host::remove_status_item("collab-status");
                host::notify(NotificationLevel::Info, "Collab", "Left room");
                host::refresh_view(VIEW_COLLAB_PANEL);
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
                        // Capture cursor position from the active editor.
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
        _ctx: &Invocation,
    ) -> Result<View, Error> {
        if view != VIEW_COLLAB_PANEL {
            return Err(Error::NotFound(format!("unknown view {view}")));
        }
        let server_url = host::storage::get(STORAGE_KEY_SERVER)
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_SERVER_URL.to_owned());
        let html = PANEL_HTML
            .replace("__SERVER_URL__", &server_url)
            .replace("__IS_DEV__", if IS_DEV { "true" } else { "false" });
        let view = View::new(Body::Html(html));
        // Toolbar buttons change based on room state
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
        let cmd = match &event {
            // Toolbar button click
            ViewEvent::Button(id) => id.as_str(),
            // HTML panel button via openidea.postMessage(...)
            ViewEvent::Message(raw) => {
                if let Ok(msg) = serde_json::from_str::<serde_json::Value>(raw) {
                    if msg.get("type").and_then(|t| t.as_str()) == Some("command") {
                        if let Some(id) = msg.get("id").and_then(|id| id.as_str()) {
                            let _ = self.run_command(id, ctx, None);
                        }
                    }
                }
                return Ok(());
            }
            _ => return Ok(()),
        };
        let _ = self.run_command(cmd, ctx, None);
        Ok(())
    }
}

register_plugin!(CollabPlugin, views);
