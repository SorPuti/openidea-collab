use std::collections::HashMap;
use std::time::Instant;

use crate::events::EventStore;

pub struct Member {
    pub user_id: String,
    pub name: String,
    pub current_file: Option<String>,
    pub last_seen: Instant,
}

impl Member {
    /// Returns true when this member has not been seen for longer than timeout_secs.
    pub fn is_expired(&self, timeout_secs: u64) -> bool {
        self.last_seen.elapsed().as_secs() > timeout_secs
    }
}

pub struct FileState {
    pub version: u64,
    pub text: String,
    pub last_user: String,
    pub last_updated: Instant,
}

pub struct Room {
    pub id: String,
    pub members: HashMap<String, Member>,
    pub files: HashMap<String, FileState>,
    pub events: EventStore,
}

impl Room {
    pub fn new(id: String) -> Self {
        Room {
            id,
            members: HashMap::new(),
            files: HashMap::new(),
            events: EventStore::new(),
        }
    }

    pub fn add_member(&mut self, member: Member) {
        self.members.insert(member.user_id.clone(), member);
    }

    pub fn remove_member(&mut self, user_id: &str) {
        self.members.remove(user_id);
    }

    pub fn update_presence(&mut self, user_id: &str, file: Option<String>) {
        if let Some(member) = self.members.get_mut(user_id) {
            member.current_file = file;
            member.last_seen = Instant::now();
        }
    }

    pub fn expire_members(&mut self, timeout_secs: u64) {
        self.members.retain(|_, m| !m.is_expired(timeout_secs));
    }
}
