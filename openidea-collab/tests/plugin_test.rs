use openidea_collab::presence::{PresenceState, PeerPresence};
use openidea_collab::room::RoomState;

#[test]
fn presence_to_tree_items() {
    let mut state = PresenceState::new("u1".into(), "Ana".into());
    state.update_peer("u2".into(), "Bob".into(), Some("src/main.rs".into()));
    let items = state.to_tree_items();
    assert!(items.iter().any(|i| i.label.contains("Bob")));
    assert!(items.iter().any(|i| i.detail.as_deref() == Some("src/main.rs")));
}

#[test]
fn presence_remove_peer() {
    let mut state = PresenceState::new("u1".into(), "Ana".into());
    state.update_peer("u2".into(), "Bob".into(), None);
    state.remove_peer("u2");
    let items = state.to_tree_items();
    assert!(!items.iter().any(|i| i.label.contains("Bob")));
}

#[test]
fn room_state_roundtrip() {
    let rs = RoomState {
        room_id: "ABCD1234".into(),
        user_id: "u1".into(),
        name: "Ana".into(),
        server_url: "http://localhost:4444".into(),
    };
    let json = serde_json::to_string(&rs).unwrap();
    let back: RoomState = serde_json::from_str(&json).unwrap();
    assert_eq!(back.room_id, "ABCD1234");
}
