//! OpenIDEA Collab plugin: real-time file sync, presence, and cursor decorations.

pub mod client;
pub mod presence;
pub mod room;
pub mod sync;

use openidea_plugin::{
    host, register_plugin, Context, Error, Event, Invocation,
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
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TIME: &str = env!("BUILD_TIME");

// Placeholders replaced at render time: __SERVER_URL__ __IS_DEV__ __VERSION__ __BUILD_TIME__ __INIT_NAME__ __INIT_GITHUB__ __BUILD_MODE__
const PANEL_HTML: &str = include_str!("page.html");

fn escape_html_attribute(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;").replace('\'', "&#39;")
        .replace('<', "&lt;").replace('>', "&gt;")
}

fn javascript_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serializes")
        .replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026")
}

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
    fn dbg(&self, step: &str) {
        if IS_DEV {
            host::notify(NotificationLevel::Info, "Collab Debug", step);
        }
    }

    fn send_panel_error(&self, msg: &str) {
        self.dbg(&format!("send_panel_error: {msg}"));
        let payload = serde_json::json!({ "type": "error", "message": msg });
        if let Ok(s) = serde_json::to_string(&payload) {
            let res = host::post_view_message(VIEW_COLLAB_PANEL, &s);
            self.dbg(&format!("post_view_message(error) -> {:?}", res.is_ok()));
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
            let res = host::post_view_message(VIEW_COLLAB_PANEL, &msg);
            self.dbg(&format!("post_view_message(state) ok={}", res.is_ok()));
        }
    }

    fn panel_create_room(&mut self, name: String, github: Option<String>) {
        let _ = host::storage::set(STORAGE_KEY_USER_NAME, &name);
        if let Some(ref gh) = github {
            let _ = host::storage::set(STORAGE_KEY_USER_GITHUB, gh);
        }

        self.dbg("create_room: calling HTTP POST /rooms ...");
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
        self.sync.last_synced_texts.clear();
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
        // The web custom editor and its approved helper own collaboration.
        // Do not start the legacy save-based sidebar or subscribe to events.
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
            .replace("__SERVER_URL_JSON__", &javascript_string(&server_url))
            .replace("__SERVER_URL__", &escape_html_attribute(&server_url))
            .replace("__IS_DEV__", if IS_DEV { "true" } else { "false" })
            .replace("__BUILD_MODE__", if IS_DEV { "DEV" } else { "RELEASE" })
            .replace("__VERSION__", VERSION)
            .replace("__BUILD_TIME__", BUILD_TIME)
            .replace("__INIT_NAME__", &escape_html_attribute(&init_name))
            .replace("__INIT_GITHUB__", &escape_html_attribute(&init_github));

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
                    Err(e) => {
                        self.dbg(&format!("handle: invalid JSON: {e}"));
                        return Ok(());
                    }
                };
                let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                self.dbg(&format!("handle: ViewEvent::Message type={msg_type}"));
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
                    Some("request-state") => {
                        self.panel_send_state();
                    }
                    Some("ping") => {
                        let url = msg["url"].as_str().unwrap_or(&self.client.server_url).trim_end_matches('/').to_owned();
                        let t0 = std::time::SystemTime::now();
                        let result = self.client.ping(&url);
                        let ms = t0.elapsed().map(|d| d.as_millis()).unwrap_or(0);
                        let reply = match result {
                            Ok(_) => serde_json::json!({ "type": "ping-result", "ok": true, "ms": ms, "url": url }),
                            Err(e) => serde_json::json!({ "type": "ping-result", "ok": false, "error": e.to_string(), "url": url }),
                        };
                        self.dbg(&format!("ping result ok={} ms={ms}", reply["ok"]));
                        if let Ok(s) = serde_json::to_string(&reply) {
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
