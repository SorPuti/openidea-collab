//! OpenIDEA Collab plugin: real-time file sync, presence, and cursor decorations.

pub mod client;
pub mod presence;
pub mod room;
pub mod sync;

use openidea_plugin::{
    host, register_plugin, Context, Error, Event, EventKind, Invocation,
    NotificationLevel, Plugin,
    views::{Body, View, Viewport, Views},
};
use client::CollabClient;
use sync::SyncState;
use room::RoomState;

const DEFAULT_SERVER_URL: &str = "http://localhost:4444";
const STORAGE_KEY_SERVER: &str = "server_url";
const VIEW_COLLAB_PANEL: &str = "collab-panel";

// HTML panel served in the right sidebar.
const PANEL_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Collaborators</title>
<style>
  :root {
    --bg: var(--vscode-sideBar-background, #1e1e1e);
    --fg: var(--vscode-sideBar-foreground, #cccccc);
    --accent: var(--vscode-focusBorder, #007fd4);
    --card: var(--vscode-editorWidget-background, #252526);
    --border: var(--vscode-editorWidget-border, #454545);
    --muted: var(--vscode-descriptionForeground, #888);
    font-family: var(--vscode-font-family, system-ui, sans-serif);
    font-size: var(--vscode-font-size, 13px);
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { background: var(--bg); color: var(--fg); padding: 8px; }
  #header { display: flex; align-items: center; gap: 8px; padding: 6px 0 10px; border-bottom: 1px solid var(--border); margin-bottom: 10px; }
  #room-id { font-weight: 600; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  #sync-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--muted); flex-shrink: 0; }
  #sync-dot.active { background: #4caf50; animation: pulse 1.5s ease-in-out infinite; }
  @keyframes pulse { 0%,100% { opacity:1 } 50% { opacity:.4 } }
  #empty { color: var(--muted); text-align: center; padding: 24px 0; font-style: italic; }
  .peer { display: flex; align-items: center; gap: 10px; padding: 6px 4px; border-radius: 4px; transition: background .15s; }
  .peer:hover { background: var(--card); }
  .avatar { width: 32px; height: 32px; border-radius: 50%; flex-shrink: 0; display: flex; align-items: center; justify-content: center; font-size: 11px; font-weight: 700; overflow: hidden; }
  .avatar img { width: 100%; height: 100%; object-fit: cover; }
  .peer-info { flex: 1; min-width: 0; }
  .peer-name { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .peer-detail { color: var(--muted); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  #not-connected { color: var(--muted); text-align: center; padding: 32px 0; }
  #not-connected b { display: block; margin-bottom: 4px; }
</style>
</head>
<body>
<div id="not-connected">
  <b>Not connected</b>
  Use <em>Collab: Join Room</em> to start
</div>
<div id="app" hidden>
  <div id="header">
    <span id="sync-dot"></span>
    <span id="room-id">—</span>
    <span id="peer-count" style="color:var(--muted);font-size:11px"></span>
  </div>
  <div id="peers"></div>
  <div id="empty" hidden>No other collaborators yet</div>
</div>
<script>
  const app = document.getElementById('app');
  const nc  = document.getElementById('not-connected');
  const dot = document.getElementById('sync-dot');
  const rid = document.getElementById('room-id');
  const pc  = document.getElementById('peer-count');
  const pl  = document.getElementById('peers');
  const em  = document.getElementById('empty');

  function render(state) {
    nc.hidden = true;
    app.hidden = false;
    rid.textContent = state.room_id || '—';
    dot.className = state.sync_active ? 'active' : '';
    const peers = state.peers || [];
    pc.textContent = peers.length ? `${peers.length} peer${peers.length>1?'s':''}` : '';
    pl.innerHTML = '';
    peers.forEach(p => {
      const div = document.createElement('div');
      div.className = 'peer';
      const av = document.createElement('div');
      av.className = 'avatar';
      if (p.github) {
        const img = document.createElement('img');
        img.src = `https://github.com/${p.github}.png?size=40`;
        img.onerror = () => { img.remove(); av.textContent = p.initials || '?'; av.style.background = p.color || '#607d8b'; };
        av.appendChild(img);
      } else {
        av.textContent = p.initials || '?';
        av.style.background = p.color || '#607d8b';
      }
      const info = document.createElement('div');
      info.className = 'peer-info';
      const name = document.createElement('div');
      name.className = 'peer-name';
      name.textContent = p.github ? `${p.name} @${p.github}` : p.name;
      info.appendChild(name);
      if (p.file) {
        const detail = document.createElement('div');
        detail.className = 'peer-detail';
        detail.textContent = p.line != null ? `${p.file} :${p.line+1}` : p.file;
        info.appendChild(detail);
      }
      div.appendChild(av);
      div.appendChild(info);
      pl.appendChild(div);
    });
    em.hidden = peers.length > 0;
  }

  window.addEventListener('message', e => {
    try {
      const msg = typeof e.data === 'string' ? JSON.parse(e.data) : e.data;
      if (msg && msg.type === 'state') render(msg.state);
    } catch {}
  });
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
        Ok(View::new(Body::Html(PANEL_HTML.to_owned())))
    }
}

register_plugin!(CollabPlugin, views);
