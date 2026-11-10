use openidea_collab::{client::CollabClient, presence::PresenceState, room::RoomState};
use openidea_plugin::{mock::MockHost, HttpResponse};

#[test]
fn presence_panel_displays_and_removes_peer() {
    let mut state = PresenceState::new("u1".into(), "Ana".into());
    state.update_peer("u2".into(), "Bob".into(), Some("src/main.rs".into()), None, Some(1), None);
    let panel = state.build_panel_json("room", true, "http://localhost:4444", false);
    assert_eq!(panel["state"]["peers"][0]["name"], "Bob");
    assert_eq!(panel["state"]["peers"][0]["file"], "main.rs");
    state.remove_peer("u2");
    assert_eq!(state.peer_count(), 0);
}

#[test]
fn room_state_roundtrip() {
    let rs = RoomState {
        room_id: "ABCD1234".into(), user_id: "u1".into(), name: "Ana".into(),
        server_url: "http://localhost:4444".into(), github: None,
    };
    let back: RoomState = serde_json::from_str(&serde_json::to_string(&rs).unwrap()).unwrap();
    assert_eq!(back.room_id, "ABCD1234");
    assert_eq!(back.server_url, rs.server_url);
}

#[test]
fn health_rejects_unrelated_404_response() {
    let host = MockHost::install().with_capabilities(&["net:localhost"]);
    host.on_fetch(|_| Ok(HttpResponse { status: 404, headers: vec![], body: vec![] }));
    assert!(CollabClient::new("http://localhost:4444".into()).ping("http://localhost:4444").is_err());
}

#[test]
fn health_requires_expected_server_response() {
    let host = MockHost::install().with_capabilities(&["net:localhost"]);
    host.on_fetch(|_| Ok(HttpResponse { status: 200, headers: vec![], body: br#"{"service":"openidea-collab","status":"ok","api":"legacy"}"#.to_vec() }));
    assert!(CollabClient::new("http://localhost:4444".into()).ping("http://localhost:4444").is_ok());
}

#[test]
fn panel_escapes_stored_user_data_in_html_and_script() {
    use openidea_collab::CollabPlugin;
    use openidea_plugin::{host, views::{Body, Views}};
    let host_state = MockHost::install().with_capabilities(&["storage", "project.read"]);
    host::storage::set("server_url", "https://example.invalid/\"</script><script>alert(1)</script>").unwrap();
    host::storage::set("user_name", "\"><img src=x onerror=alert(1)>").unwrap();
    let view = CollabPlugin::default().render("collab-panel", None, &host_state.invocation()).unwrap();
    let Body::Html(html) = view.body else { panic!("expected html") };
    assert!(!html.contains("</script><script>alert(1)</script>"));
    assert!(!html.contains("\"><img src=x onerror=alert(1)>"));
}

#[test]
fn poll_preserves_cursor_when_remote_file_cannot_be_applied() {
    use openidea_collab::sync::SyncState;
    let host = MockHost::install().with_capabilities(&["net:localhost", "project.read"]);
    host.on_fetch(|request| {
        let body = if request.url.contains("/events?") {
            r#"{"events":[{"id":0,"event":{"type":"file_updated","path":"main.rs","version":1,"user_id":"u2"}}],"next_cursor":1}"#
        } else {
            r#"{"version":1,"text":"remote"}"#
        };
        Ok(HttpResponse { status: 200, headers: vec![], body: body.as_bytes().to_vec() })
    });
    let mut sync = SyncState::default();
    sync.room = Some(RoomState { room_id: "room".into(), user_id: "u1".into(), name: "Ana".into(), server_url: "http://localhost:4444".into(), github: None });
    sync.on_poll_tick(&CollabClient::new("http://localhost:4444".into()), &host.invocation());
    assert_eq!(sync.poll_cursor, 0, "failed updates must remain retryable");
    assert!(!sync.sync_active, "failed writes must not be shown as synchronized");
}

#[test]
fn remote_update_never_overwrites_unrelated_local_file() {
    use openidea_collab::sync::SyncState;
    let host = MockHost::install().with_capabilities(&["net:localhost", "project.read", "project.write"]);
    host.add_file("main.rs", "local work");
    host.on_fetch(|request| {
        let body = if request.url.contains("/events?") {
            r#"{"events":[{"id":0,"event":{"type":"file_updated","path":"main.rs","version":1,"user_id":"u2"}}],"next_cursor":1}"#
        } else {
            r#"{"version":1,"text":"remote work"}"#
        };
        Ok(HttpResponse { status: 200, headers: vec![], body: body.as_bytes().to_vec() })
    });
    let mut sync = SyncState::default();
    sync.room = Some(RoomState { room_id: "room".into(), user_id: "u1".into(), name: "Ana".into(), server_url: "http://localhost:4444".into(), github: None });
    let ctx = host.invocation();
    sync.on_poll_tick(&CollabClient::new("http://localhost:4444".into()), &ctx);
    assert_eq!(ctx.project().unwrap().read_file("main.rs").unwrap(), "local work");
    assert_eq!(sync.poll_cursor, 0);
    assert!(!sync.sync_active);
}

#[test]
fn remote_update_can_replace_unchanged_known_baseline() {
    use openidea_collab::sync::SyncState;
    let host = MockHost::install().with_capabilities(&["net:localhost", "project.read", "project.write"]);
    host.add_file("main.rs", "baseline");
    host.on_fetch(|request| {
        let body = if request.method == "POST" {
            r#"{"version":1}"#
        } else if request.url.contains("/events?") {
            r#"{"events":[{"id":0,"event":{"type":"file_updated","path":"main.rs","version":2,"user_id":"u2"}}],"next_cursor":1}"#
        } else {
            r#"{"version":2,"text":"remote change"}"#
        };
        Ok(HttpResponse { status: 200, headers: vec![], body: body.as_bytes().to_vec() })
    });
    let mut sync = SyncState::default();
    sync.room = Some(RoomState { room_id: "room".into(), user_id: "u1".into(), name: "Ana".into(), server_url: "http://localhost:4444".into(), github: None });
    let client = CollabClient::new("http://localhost:4444".into());
    let ctx = host.invocation();
    sync.on_file_saved(&client, &ctx, "main.rs");
    sync.on_poll_tick(&client, &ctx);
    assert_eq!(ctx.project().unwrap().read_file("main.rs").unwrap(), "remote change");
    assert_eq!(sync.poll_cursor, 1);
    assert!(sync.sync_active);
}

#[test]
fn saved_absolute_path_is_shared_relative_to_project_root() {
    use openidea_collab::sync::SyncState;
    let host = MockHost::install().with_capabilities(&["net:localhost", "project.read"]);
    host.add_file("src/main.rs", "hello");
    host.on_fetch(|_| Ok(HttpResponse {status: 200, headers: vec![], body: br#"{"version":1}"#.to_vec()}));
    let mut sync = SyncState::default();
    sync.room = Some(RoomState {room_id: "room".into(), user_id: "u1".into(), name: "Ana".into(), server_url: "http://localhost:4444".into(), github: None});
    sync.on_file_saved(&CollabClient::new("http://localhost:4444".into()), &host.invocation(), "/project/src/main.rs");
    let requests = host.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].url.ends_with("/files/src%2Fmain.rs"), "{}", requests[0].url);
}

#[test]
fn file_paths_are_encoded_as_url_data() {
    let host = MockHost::install().with_capabilities(&["net:localhost"]);
    host.on_fetch(|_| Ok(HttpResponse {status: 200, headers: vec![], body: br#"{"version":1}"#.to_vec()}));
    CollabClient::new("http://localhost:4444".into()).post_file("room", "u1", "src/ação #1?.rs", 0, "test").unwrap();
    assert!(host.requests()[0].url.ends_with("/files/src%2Fa%C3%A7%C3%A3o%20%231%3F.rs"));
}

#[test]
fn remote_parent_traversal_is_rejected() {
    use openidea_collab::sync::SyncState;
    let host = MockHost::install().with_capabilities(&["net:localhost", "project.read", "project.write"]);
    host.on_fetch(|_| Ok(HttpResponse { status: 200, headers: vec![], body: br#"{"events":[{"id":0,"event":{"type":"file_updated","path":"../outside.txt","version":1}}],"next_cursor":1}"#.to_vec() }));
    let mut sync = SyncState::default();
    sync.room = Some(RoomState {room_id: "room".into(), user_id: "u1".into(), name: "Ana".into(), server_url: "http://localhost:4444".into(), github: None});
    sync.on_poll_tick(&CollabClient::new("http://localhost:4444".into()), &host.invocation());
    assert_eq!(sync.poll_cursor, 0);
    assert_eq!(host.requests().len(), 1, "unsafe path must not be fetched");
}

#[test]
fn health_rejects_incompatible_realtime_server_mode() {
    let host = MockHost::install().with_capabilities(&["net:localhost"]);
    host.on_fetch(|_| Ok(HttpResponse { status: 200, headers: vec![], body: br#"{"service":"openidea-collab","status":"ok","api":"realtime-experimental"}"#.to_vec() }));
    assert!(CollabClient::new("http://localhost:4444".into()).ping("http://localhost:4444").is_err());
}
