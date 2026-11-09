//! Sync loop: file version tracking, event polling and presence management.

use std::collections::HashMap;
use openidea_plugin::{host, Invocation, NotificationLevel};
use crate::client::CollabClient;
use crate::presence::PresenceState;
use crate::room::RoomState;

const STORAGE_KEY: &str = "room_state";
const VIEW_COLLAB_PANEL: &str = "collab-panel";

pub struct SyncState {
    pub room: Option<RoomState>,
    pub presence: PresenceState,
    pub file_versions: HashMap<String, u64>,
    pub poll_cursor: u64,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            room: None,
            presence: PresenceState::new(String::new(), String::new()),
            file_versions: HashMap::new(),
            poll_cursor: 0,
        }
    }
}

impl SyncState {
    /// Restore state from the plugin's key-value store.
    pub fn load_from_storage() -> Self {
        match host::storage::get(STORAGE_KEY) {
            Ok(Some(json)) => match serde_json::from_str::<RoomState>(&json) {
                Ok(room) => Self {
                    presence: PresenceState::new(
                        room.user_id.clone(),
                        room.name.clone(),
                    ),
                    room: Some(room),
                    file_versions: HashMap::new(),
                    poll_cursor: 0,
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
    pub fn on_file_saved(
        &mut self,
        client: &CollabClient,
        ctx: &Invocation,
        path: &str,
    ) {
        let room = match self.room.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };
        let project = match ctx.project() {
            Some(p) => p,
            None => return,
        };
        let text = match project.read_file(path) {
            Ok(t) => t,
            Err(_) => return,
        };
        let version = self.file_versions.get(path).copied().unwrap_or(0);
        match client.post_file(&room.room_id, &room.user_id, path, version, &text) {
            Ok(event_id) => {
                self.file_versions.insert(path.to_owned(), event_id);
            }
            Err(e) => {
                host::notify(
                    NotificationLevel::Warning,
                    "Collab Sync",
                    &format!("Failed to sync {path}: {e}"),
                );
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
            Err(_) => return,
        };
        let mut presence_changed = false;
        for envelope in &result.events {
            let inner = &envelope["event"];
            let event_type = inner["type"].as_str().unwrap_or("");
            match event_type {
                "file_updated" => {
                    self.apply_file_updated(client, ctx, inner, &room);
                }
                "member_joined" => {
                    if let (Some(uid), Some(name)) = (
                        inner["user_id"].as_str(),
                        inner["name"].as_str(),
                    ) {
                        self.presence.update_peer(
                            uid.to_owned(),
                            name.to_owned(),
                            inner["current_file"]
                                .as_str()
                                .map(str::to_owned),
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
                    if let (Some(uid), Some(name)) = (
                        inner["user_id"].as_str(),
                        inner["name"].as_str(),
                    ) {
                        self.presence.update_peer(
                            uid.to_owned(),
                            name.to_owned(),
                            inner["current_file"]
                                .as_str()
                                .map(str::to_owned),
                        );
                        presence_changed = true;
                    }
                }
                "conflict_warn" => {
                    let file = inner["path"]
                        .as_str()
                        .unwrap_or("(unknown)");
                    let user = inner["user_id"]
                        .as_str()
                        .unwrap_or("another user");
                    host::notify(
                        NotificationLevel::Warning,
                        "Collab Conflict",
                        &format!("Conflict in {file} with {user}"),
                    );
                }
                _ => {}
            }
        }
        self.poll_cursor = result.next_cursor;
        if presence_changed {
            host::refresh_view(VIEW_COLLAB_PANEL);
        }
    }

    fn apply_file_updated(
        &mut self,
        client: &CollabClient,
        ctx: &Invocation,
        event: &serde_json::Value,
        room: &RoomState,
    ) {
        let path = match event["path"].as_str() {
            Some(p) => p,
            None => return,
        };
        let remote_version = event["version"].as_u64().unwrap_or(0);
        let local_version = self.file_versions.get(path).copied().unwrap_or(0);
        if remote_version <= local_version {
            return;
        }
        // Fetch the latest text from the server and write it to the project.
        match client.get_file(&room.room_id, path) {
            Ok((version, text)) => {
                if let Some(project) = ctx.project() {
                    if project.write_file(path, &text).is_ok() {
                        self.file_versions.insert(path.to_owned(), version);
                    }
                }
            }
            Err(_) => {}
        }
    }
}
