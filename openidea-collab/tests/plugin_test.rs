use openidea_collab::{CollabPlugin, presence::PresenceState, room::RoomState};
use openidea_plugin::{Event, EventKind, HttpResponse, Plugin, host, mock::MockHost};
use serde_json::json;

#[test]
fn presence_panel_reflects_peer_updates_and_removal() {
    let mut state = PresenceState::new("u1".into(), "Alice".into());
    state.update_peer(
        "u2".into(),
        "Bob".into(),
        Some("src/main.rs".into()),
        Some("bob".into()),
        Some(3),
        Some(2),
    );
    let panel = state.build_panel_json("room", true, "http://localhost:4444", false);
    assert_eq!(panel["state"]["peer_count"], 1);
    assert_eq!(panel["state"]["peers"][0]["name"], "Bob");
    assert_eq!(panel["state"]["peers"][0]["file"], "main.rs");
    assert_eq!(panel["state"]["peers"][0]["line"], 3);
    state.remove_peer("u2");
    assert_eq!(state.peer_count(), 0);
}

#[test]
fn room_state_reads_legacy_profile_and_roundtrips() {
    let legacy = json!({"room_id":"room", "user_id":"u1", "name":"Alice", "server_url":"http://localhost:4444"});
    let mut room: RoomState = serde_json::from_value(legacy).unwrap();
    assert!(room.github.is_none());
    room.github = Some("alice".into());
    let restored: RoomState = serde_json::from_str(&serde_json::to_string(&room).unwrap()).unwrap();
    assert_eq!(restored.room_id, "room");
    assert_eq!(restored.github.as_deref(), Some("alice"));
}

#[test]
fn scheduled_event_polls_and_applies_remote_file() {
    let mock = MockHost::install().with_capabilities(&[
        "storage",
        "events",
        "project.read",
        "project.write",
        "editor.read",
        "net:localhost",
    ]);
    mock.add_file("src/example.txt", "old\n");
    host::storage::set(
        "room_state",
        &json!({
            "room_id":"room", "user_id":"u1", "name":"Alice", "server_url":"http://localhost:4444"
        })
        .to_string(),
    )
    .unwrap();
    mock.on_fetch(|request| {
        let body = if request.url.ends_with("/events?cursor=0") {
            json!({"events":[{"id":0,"event":{"type":"file_updated","path":"src/example.txt","version":1,"user_id":"u2"}}],"next_cursor":1})
        } else if request.url.ends_with("/files/src%2Fexample.txt") {
            json!({"version":1,"text":"remote\n","user_id":"u2"})
        } else if request.url.ends_with("/events?cursor=1") {
            json!({"events":[],"next_cursor":1})
        } else { panic!("unexpected request: {}", request.url) };
        Ok(HttpResponse { status:200, headers:vec![], body:serde_json::to_vec(&body).unwrap() })
    });
    let mut plugin = CollabPlugin::default();
    plugin
        .activate(&mock.context("sorputi.openidea-collab"))
        .unwrap();
    assert!(mock.subscriptions().contains(&EventKind::Scheduled));
    plugin.handle_events(&[Event::Scheduled], &mock.invocation());
    assert_eq!(
        mock.invocation()
            .project()
            .unwrap()
            .read_file("src/example.txt")
            .unwrap(),
        "remote\n"
    );
    plugin.handle_events(&[Event::Scheduled], &mock.invocation());
    assert_eq!(mock.requests().len(), 3);
    // This verifies dispatch, not the IDE's wall-clock scheduler.
}
