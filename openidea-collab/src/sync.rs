//! Sync loop: file version tracking, event polling, presence and decorations.

use crate::client::CollabClient;
use crate::presence::PresenceState;
use crate::room::RoomState;
use openidea_plugin::{
    Decoration, FileDecoration, Invocation, NotificationLevel, Range, StatusItem, host,
};
use std::collections::HashMap;

const STORAGE_KEY: &str = "room_state";
const VIEW_COLLAB_PANEL: &str = "collab-panel";
const DECORATION_SET: &str = "collab-cursors";
const FILE_BADGE_SET: &str = "collab-open";
const STATUS_ITEM_ID: &str = "collab-status";

/// Map local editor paths to portable project-relative paths; reject traversal.
fn relative_path(path: &str, root: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let root = root.replace('\\', "/");
    let root = root.trim_end_matches('/');
    let relative = if let Some(rest) = path.strip_prefix(root).and_then(|p| p.strip_prefix('/')) {
        rest
    } else if !path.starts_with('/') && !path.as_bytes().get(1).is_some_and(|b| *b == b':') {
        path.as_str()
    } else {
        return None;
    };
    if relative.is_empty() || relative.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return None;
    }
    Some(relative.to_owned())
}

pub struct SyncState {
    pub room: Option<RoomState>,
    pub presence: PresenceState,
    pub file_versions: HashMap<String, u64>,
    pub last_synced_texts: HashMap<String, String>,
    pub poll_cursor: u64,
    pub sync_active: bool,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            room: None,
            presence: PresenceState::new(String::new(), String::new()),
            file_versions: HashMap::new(),
            last_synced_texts: HashMap::new(),
            poll_cursor: 0,
            sync_active: false,
        }
    }
}

impl SyncState {
    /// Restore state from the plugin's key-value store.
    pub fn load_from_storage() -> Self {
        match host::storage::get(STORAGE_KEY) {
            Ok(Some(json)) => match serde_json::from_str::<RoomState>(&json) {
                Ok(room) => Self {
                    presence: PresenceState::new(room.user_id.clone(), room.name.clone()),
                    room: Some(room),
                    file_versions: HashMap::new(),
                    last_synced_texts: HashMap::new(),
                    poll_cursor: 0,
                    sync_active: false,
                },
                Err(_) => Self::default(),
            },
            _ => Self::default(),
        }
    }

    /// Persist the current room state.
    pub fn save_to_storage(&self) {
        if let Some(ref room) = self.room {
            if let Ok(json) = serde_json::to_string(room) {
                let _ = host::storage::set(STORAGE_KEY, &json);
            }
        } else {
            let _ = host::storage::delete(STORAGE_KEY);
        }
    }

    /// Called when the user saves a file: read its text and POST to the server.
    pub fn on_file_saved(&mut self, client: &CollabClient, ctx: &Invocation, path: &str) {
        let room = match self.room.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };
        let project = match ctx.project() {
            Some(p) => p,
            None => return,
        };
        let root = match project.root() { Ok(root) => root, Err(_) => return };
        let shared_path = match relative_path(path, &root) {
            Some(path) => path,
            None => {
                host::notify(NotificationLevel::Warning, "Collab Sync", "File is outside the shared project.");
                return;
            }
        };
        let text = match project.read_file(path) {
            Ok(t) => t,
            Err(_) => return,
        };
        let version = self.file_versions.get(&shared_path).copied().unwrap_or(0);
        match client.post_file(&room.room_id, &room.user_id, &shared_path, version, &text) {
            Ok(new_version) => {
                self.file_versions.insert(shared_path.clone(), new_version);
                self.last_synced_texts.insert(shared_path, text);
                self.sync_active = true;
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("conflict") {
                    host::notify(
                        NotificationLevel::Warning,
                        "Collab Conflict",
                        "Conflict detected. Your local file was not sent; compare it with the server before retrying.",
                    );
                } else {
                    host::notify(
                        NotificationLevel::Warning,
                        "Collab Sync",
                        &format!("Failed to sync {path}: {e}"),
                    );
                }
            }
        }
    }

    /// Called on each Scheduled tick: poll for new events and apply them.
    pub fn on_poll_tick(&mut self, client: &CollabClient, ctx: &Invocation) {
        let room = match self.room.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };
        let result = match client.poll_events(&room.room_id, self.poll_cursor) {
            Ok(r) => r,
            Err(_) => {
                self.sync_active = false;
                self.update_status_bar(&room.room_id);
                return;
            }
        };
        let mut presence_changed = false;
        for envelope in &result.events {
            let inner = &envelope["event"];
            let event_type = inner["type"].as_str().unwrap_or("");
            match event_type {
                "file_updated" => {
                    if let Err(error) = self.apply_file_updated(client, ctx, inner, &room) {
                        self.sync_active = false;
                        self.update_status_bar(&room.room_id);
                        self.push_panel_state(&room.room_id);
                        host::notify(NotificationLevel::Warning, "Collab Sync", &format!("Could not apply a remote update: {error}. It will be retried."));
                        return;
                    }
                }
                "member_joined" => {
                    if let (Some(uid), Some(name)) =
                        (inner["user_id"].as_str(), inner["name"].as_str())
                    {
                        self.presence.update_peer(
                            uid.to_owned(),
                            name.to_owned(),
                            None,
                            None,
                            None,
                            None,
                        );
                        presence_changed = true;
                    }
                }
                "member_left" => {
                    if let Some(uid) = inner["user_id"].as_str() {
                        self.presence.remove_peer(uid);
                        presence_changed = true;
                    }
                }
                "presence_changed" => {
                    if let Some(uid) = inner["user_id"].as_str() {
                        let name = self
                            .presence
                            .peer_name(uid)
                            .map(str::to_owned)
                            .unwrap_or_else(|| uid.to_owned());
                        self.presence.update_peer(
                            uid.to_owned(),
                            name,
                            inner["file"].as_str().map(str::to_owned),
                            inner["github"].as_str().map(str::to_owned),
                            inner["line"].as_u64().map(|v| v as u32),
                            inner["col"].as_u64().map(|v| v as u32),
                        );
                        presence_changed = true;
                    }
                }
                "conflict_warn" => {
                    let path = inner["path"].as_str().unwrap_or("(unknown)");
                    let user_a = inner["user_a"].as_str().unwrap_or("(unknown)");
                    let user_b = inner["user_b"].as_str().unwrap_or("(unknown)");
                    host::notify(
                        NotificationLevel::Warning,
                        "Collab Conflict",
                        &format!("Conflict on {path}: {user_a} vs {user_b}"),
                    );
                }
                _ => {}
            }
            if let Some(id) = envelope["id"].as_u64() {
                self.poll_cursor = id.saturating_add(1);
            }
        }
        self.poll_cursor = result.next_cursor;
        self.sync_active = true;
        if presence_changed {
            self.apply_decorations();
            self.apply_file_badges();
            self.update_status_bar(&room.room_id);
            self.push_panel_state(&room.room_id);
        }
    }

    fn apply_file_updated(
        &mut self,
        client: &CollabClient,
        ctx: &Invocation,
        event: &serde_json::Value,
        room: &RoomState,
    ) -> Result<(), String> {
        let path = match event["path"].as_str() {
            Some(p) => p,
            None => return Err("missing file path".into()),
        };
        if path.starts_with('/') || path.contains('\\') || path.as_bytes().get(1) == Some(&b':')
            || path.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err("invalid remote project path".into());
        }
        let remote_version = event["version"].as_u64().unwrap_or(0);
        let local_version = self.file_versions.get(path).copied().unwrap_or(0);
        if remote_version <= local_version {
            return Ok(());
        }
        let (version, text) = client.get_file(&room.room_id, path).map_err(|e| e.to_string())?;
        let project = ctx.project().ok_or("no project available")?;
        match project.read_file(path) {
            Ok(local) if local == text => {}
            Ok(local) if self.last_synced_texts.get(path) == Some(&local) => {
                project.write_file(path, &text).map_err(|e| e.to_string())?;
            }
            Ok(_) => return Err(format!("{path} has local changes; compare before applying the remote version")),
            Err(openidea_plugin::Error::NotFound(_)) if !self.last_synced_texts.contains_key(path) => {
                project.write_file(path, &text).map_err(|e| e.to_string())?;
            }
            Err(error) => return Err(format!("could not safely read {path}: {error}")),
        }
        self.file_versions.insert(path.to_owned(), version);
        self.last_synced_texts.insert(path.to_owned(), text);
        Ok(())
    }

    /// Set inline cursor decorations: show peer name and cursor position after their line.
    pub fn apply_decorations(&self) {
        let mut by_file: HashMap<String, Vec<Decoration>> = HashMap::new();
        for peer in self.presence.all_peers() {
            let file = match &peer.current_file {
                Some(f) => f.clone(),
                None => continue,
            };
            let line = peer.line.unwrap_or(0);
            let col = peer.col.unwrap_or(0);
            let label = match &peer.github {
                Some(gh) => format!("  @{gh} :{}", col + 1),
                None => format!("  {} :{}", peer.name, col + 1),
            };
            let pos = openidea_plugin::Position { line, character: col };
            let range = Range { start: pos, end: pos };
            let deco = Decoration {
                range,
                style: "collab-cursor".to_owned(),
                hover: Some(format!("{} (line {}:{})", peer.name, line + 1, col + 1)),
                after: Some(label),
            };
            by_file.entry(file).or_default().push(deco);
        }
        // Clear first, then re-apply per file.
        let _ = host::clear_decorations(DECORATION_SET);
        for (path, decos) in &by_file {
            let _ = host::set_decorations(path, DECORATION_SET, decos);
        }
    }

    /// Apply file-tree badges for files a peer currently has open.
    pub fn apply_file_badges(&self) {
        let _ = host::clear_file_decorations(FILE_BADGE_SET);
        // Count peers per file.
        let mut counts: HashMap<String, usize> = HashMap::new();
        for peer in self.presence.all_peers() {
            if let Some(ref f) = peer.current_file {
                *counts.entry(f.clone()).or_insert(0) += 1;
            }
        }
        for (path, count) in counts {
            let badge = if count == 1 { "●".to_owned() } else { format!("{count}●") };
            let deco = FileDecoration {
                badge: Some(badge),
                style: "collab-open".to_owned(),
                tooltip: Some(format!("{count} collaborator(s) have this file open")),
            };
            let _ = host::set_file_decoration(&path, FILE_BADGE_SET, &deco);
        }
    }

    /// Update the status bar item with current room and peer count.
    pub fn update_status_bar(&self, room_id: &str) {
        if self.room.is_none() {
            host::remove_status_item(STATUS_ITEM_ID);
            return;
        }
        let peer_count = self.presence.peer_count();
        let icon = if self.sync_active { "$(sync~spin)" } else { "$(plug)" };
        let text = if peer_count == 0 {
            format!("{icon} {room_id}")
        } else {
            format!("{icon} {room_id} ·{peer_count}")
        };
        let item = StatusItem {
            text,
            tooltip: Some(format!(
                "Collab room {room_id} — {} peer(s) online",
                peer_count
            )),
            command: None,
        };
        host::set_status_item(STATUS_ITEM_ID, &item);
    }

    /// Push current state as JSON to the HTML panel via post_view_message.
    pub fn push_panel_state(&self, room_id: &str) {
        let server_url = self
            .room
            .as_ref()
            .map(|r| r.server_url.as_str())
            .unwrap_or(crate::DEFAULT_SERVER_URL);
        let payload = self.presence.build_panel_json(
            room_id,
            self.sync_active,
            server_url,
            crate::IS_DEV,
        );
        if let Ok(msg) = serde_json::to_string(&payload) {
            let _ = host::post_view_message(VIEW_COLLAB_PANEL, &msg);
        }
    }
}
