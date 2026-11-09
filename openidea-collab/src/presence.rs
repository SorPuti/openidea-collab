//! Presence state and tree-view rendering for the Collaborators panel.

use std::collections::HashMap;
use openidea_plugin::views::TreeItem;

/// One peer's presence snapshot.
#[derive(Debug, Clone)]
pub struct PeerPresence {
    pub user_id: String,
    pub name: String,
    pub current_file: Option<String>,
}

/// Tracks the local user and all remote peers.
pub struct PresenceState {
    pub self_id: String,
    pub self_name: String,
    peers: HashMap<String, PeerPresence>,
}

impl PresenceState {
    pub fn new(self_id: String, self_name: String) -> Self {
        Self { self_id, self_name, peers: HashMap::new() }
    }

    /// Insert or update a peer's snapshot.
    pub fn update_peer(&mut self, uid: String, name: String, file: Option<String>) {
        self.peers.insert(uid.clone(), PeerPresence { user_id: uid, name, current_file: file });
    }

    /// Remove a peer (e.g. they left the room).
    pub fn remove_peer(&mut self, uid: &str) {
        self.peers.remove(uid);
    }

    /// Return the display name for an existing peer, if known.
    pub fn peer_name(&self, uid: &str) -> Option<&str> {
        self.peers.get(uid).map(|p| p.name.as_str())
    }

    /// Build the tree items for the Collaborators side-panel.
    /// Each peer becomes one item: label = name, detail = current file.
    pub fn to_tree_items(&self) -> Vec<TreeItem> {
        self.peers
            .values()
            .map(|p| {
                let item = TreeItem::new(&p.user_id, &p.name);
                match &p.current_file {
                    Some(f) => item.detail(f),
                    None => item,
                }
            })
            .collect()
    }
}
