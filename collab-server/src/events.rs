#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoomEvent {
    FileUpdated {
        path: String,
        version: u64,
        user_id: String,
    },
    MemberJoined {
        user_id: String,
        name: String,
    },
    MemberLeft {
        user_id: String,
    },
    PresenceChanged {
        user_id: String,
        file: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        github: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        col: Option<u32>,
    },
    ConflictWarn {
        path: String,
        user_a: String,
        user_b: String,
    },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventEnvelope {
    pub id: u64,
    pub event: RoomEvent,
}

pub struct EventStore {
    events: Vec<EventEnvelope>,
    next_id: u64,
}

impl EventStore {
    pub fn new() -> Self {
        EventStore {
            events: Vec::new(),
            next_id: 0,
        }
    }

    /// Push an event and return its assigned id.
    pub fn push(&mut self, event: RoomEvent) -> u64 {
        let id = self.next_id;
        self.events.push(EventEnvelope { id, event });
        self.next_id += 1;
        id
    }

    /// Return all events with id >= cursor.
    pub fn since(&self, cursor: u64) -> Vec<EventEnvelope> {
        self.events
            .iter()
            .filter(|e| e.id >= cursor)
            .cloned()
            .collect()
    }

    /// Next id that will be assigned (useful as next_cursor for callers).
    pub fn next_id(&self) -> u64 {
        self.next_id
    }
}

impl Default for EventStore {
    fn default() -> Self {
        Self::new()
    }
}
