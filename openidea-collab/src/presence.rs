//! Presence state and HTML panel rendering for the Collaborators panel.

use std::collections::HashMap;

/// One peer's presence snapshot.
#[derive(Debug, Clone)]
pub struct PeerPresence {
    pub user_id: String,
    pub name: String,
    pub current_file: Option<String>,
    pub github: Option<String>,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

impl PeerPresence {
    fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }

    fn avatar_color(&self) -> &'static str {
        let colors = [
            "#E57373", "#F06292", "#BA68C8", "#64B5F6", "#4DB6AC", "#81C784", "#FFD54F", "#FF8A65",
        ];
        let idx = self
            .user_id
            .bytes()
            .fold(0usize, |a, b| a.wrapping_add(b as usize))
            % colors.len();
        colors[idx]
    }

    pub fn to_json(&self) -> serde_json::Value {
        let file_display = self
            .current_file
            .as_deref()
            .map(|f| f.rsplit('/').next().unwrap_or(f).to_owned());
        serde_json::json!({
            "user_id": self.user_id,
            "name": self.name,
            "github": self.github,
            "initials": self.initials(),
            "color": self.avatar_color(),
            "file": file_display,
            "line": self.line,
            "col": self.col,
        })
    }
}

/// Tracks the local user and all remote peers.
pub struct PresenceState {
    pub self_id: String,
    pub self_name: String,
    pub self_github: Option<String>,
    peers: HashMap<String, PeerPresence>,
}

impl PresenceState {
    pub fn new(self_id: String, self_name: String) -> Self {
        Self {
            self_id,
            self_name,
            self_github: None,
            peers: HashMap::new(),
        }
    }

    /// Insert or update a peer's snapshot.
    pub fn update_peer(
        &mut self,
        uid: String,
        name: String,
        file: Option<String>,
        github: Option<String>,
        line: Option<u32>,
        col: Option<u32>,
    ) {
        let entry = self
            .peers
            .entry(uid.clone())
            .or_insert_with(|| PeerPresence {
                user_id: uid,
                name: name.clone(),
                current_file: None,
                github: None,
                line: None,
                col: None,
            });
        entry.name = name;
        entry.current_file = file;
        if github.is_some() {
            entry.github = github;
        }
        entry.line = line;
        entry.col = col;
    }

    /// Remove a peer (e.g. they left the room).
    pub fn remove_peer(&mut self, uid: &str) {
        self.peers.remove(uid);
    }

    /// Return the display name for an existing peer, if known.
    pub fn peer_name(&self, uid: &str) -> Option<&str> {
        self.peers.get(uid).map(|p| p.name.as_str())
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn all_peers(&self) -> impl Iterator<Item = &PeerPresence> {
        self.peers.values()
    }

    /// Build JSON state payload for the HTML panel.
    pub fn build_panel_json(
        &self,
        room_id: &str,
        sync_active: bool,
        server_url: &str,
        is_dev: bool,
    ) -> serde_json::Value {
        let peers: Vec<serde_json::Value> = self.peers.values().map(|p| p.to_json()).collect();
        serde_json::json!({
            "type": "state",
            "state": {
                "room_id": room_id,
                "sync_active": sync_active,
                "server_url": server_url,
                "is_dev": is_dev,
                "peer_count": peers.len(),
                "peers": peers,
            }
        })
    }
}
