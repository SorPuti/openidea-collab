//! OpenIDEA Collab plugin: real-time file sync and presence for collaborative sessions.

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
        // Restore persisted room state and update the client's server URL.
        self.sync = SyncState::load_from_storage();
        let server_url = host::storage::get(STORAGE_KEY_SERVER)
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_SERVER_URL.to_owned());
        self.client = CollabClient::new(server_url);

        // Subscribe to the events we care about.
        host::subscribe(EventKind::FileSaved)?;
        host::subscribe(EventKind::EditorFocused)?;
        host::subscribe(EventKind::Scheduled)?;
        Ok(())
    }

    fn run_command(
        &mut self,
        id: &str,
        _ctx: &Invocation,
        _arg: Option<&str>,
    ) -> Result<Option<String>, Error> {
        match id {
            "create-room" => {
                let room_id =
                    self.client.create_room().map_err(|e| Error::Failed(e.to_string()))?;
                let name = host::input("Your name", None)?
                    .unwrap_or_else(|| "Anonymous".to_owned());
                let user_id = self
                    .client
                    .join_room(&room_id, &name)
                    .map_err(|e| Error::Failed(e.to_string()))?;
                let server_url = self.client.server_url.clone();
                let room = RoomState { room_id: room_id.clone(), user_id, name, server_url };
                self.sync.room = Some(room);
                self.sync.presence = presence::PresenceState::new(
                    self.sync.room.as_ref().map(|r| r.user_id.clone()).unwrap_or_default(),
                    self.sync.room.as_ref().map(|r| r.name.clone()).unwrap_or_default(),
                );
                self.sync.save_to_storage();
                host::notify(
                    NotificationLevel::Info,
                    "Collab",
                    &format!("Room created: {room_id}"),
                );
                host::refresh_view(VIEW_COLLAB_PANEL);
                Ok(Some(room_id))
            }
            "join-room" => {
                let room_id = match host::input("Room ID", None)? {
                    Some(r) if !r.trim().is_empty() => r.trim().to_owned(),
                    _ => return Err(Error::Cancelled),
                };
                let name = host::input("Your name", None)?
                    .unwrap_or_else(|| "Anonymous".to_owned());
                let user_id = self
                    .client
                    .join_room(&room_id, &name)
                    .map_err(|e| Error::Failed(e.to_string()))?;
                let server_url = self.client.server_url.clone();
                let room =
                    RoomState { room_id: room_id.clone(), user_id: user_id.clone(), name: name.clone(), server_url };
                self.sync.room = Some(room);
                self.sync.presence = presence::PresenceState::new(user_id, name);
                self.sync.poll_cursor = 0;
                self.sync.save_to_storage();
                // Seed presence from current member list (best-effort).
                if let Ok(members) = self.client.get_members(&room_id) {
                    let my_id = self
                        .sync
                        .room
                        .as_ref()
                        .map(|r| r.user_id.as_str())
                        .unwrap_or("");
                    for m in members {
                        if m.user_id != my_id {
                            self.sync.presence.update_peer(
                                m.user_id,
                                m.name,
                                m.current_file,
                            );
                        }
                    }
                }
                host::notify(
                    NotificationLevel::Info,
                    "Collab",
                    &format!("Joined room {room_id}"),
                );
                host::refresh_view(VIEW_COLLAB_PANEL);
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
                self.sync.save_to_storage();
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
                        let _ = self.client.update_presence(
                            &room.room_id,
                            &room.user_id,
                            maybe_path.as_deref(),
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
        match self.sync.room.as_ref() {
            None => Ok(View::message("Not connected — use Collab: Join Room")),
            Some(room) => {
                let items = self.sync.presence.to_tree_items();
                let status = format!("Room: {}", room.room_id);
                Ok(View::new(Body::Tree(items)).status(&status))
            }
        }
    }
}

register_plugin!(CollabPlugin, views);
