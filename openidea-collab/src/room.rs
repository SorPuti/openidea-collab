//! Room state: persisted via storage, shared with client and sync.

/// Identifies the current collaborative session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomState {
    pub room_id: String,
    pub user_id: String,
    pub name: String,
    pub server_url: String,
    #[serde(default)]
    pub github: Option<String>,
}

/// One member as returned by the server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemberInfo {
    pub user_id: String,
    pub name: String,
    pub current_file: Option<String>,
    #[serde(default)]
    pub github: Option<String>,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub col: Option<u32>,
}
