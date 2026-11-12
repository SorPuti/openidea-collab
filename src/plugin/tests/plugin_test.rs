use openidea_collab::{CollabPlugin, state};
use openidea_collab_core::{document::Document, transfer::split};
use openidea_collab_protocol::{DEFAULT_ORIGIN, Frame, Kind};
use openidea_plugin::{
    DocumentChanged, DocumentTextChange, Event, HttpResponse, Plugin, Position, Range, TextEdit,
    WebSocketReady, host, mock::MockHost,
};
use serde_json::json;
use uuid::Uuid;
fn mock() -> MockHost {
    let mock = MockHost::install().with_capabilities(&[
        "storage",
        "secrets",
        "project.read",
        "editor.read",
        "editor.write",
        "events",
        "net:collab.synthosia.com",
    ]);
    mock.declare_secret("member-credential");
    mock
}
fn response(value: serde_json::Value) -> HttpResponse {
    HttpResponse {
        status: 201,
        headers: vec![],
        body: serde_json::to_vec(&value).unwrap(),
    }
}
#[test]
fn installation_and_file_events_do_not_upload_or_write_anything() {
    let mock = mock();
    mock.add_file(".env", "PRIVATE");
    host::storage::set("room_state", "legacy room").unwrap();
    let mut plugin = CollabPlugin::default();
    plugin
        .activate(&mock.context("sorputi.openidea-collab"))
        .unwrap();
    plugin.handle_events(
        &[Event::FileSaved(".env".into()), Event::Scheduled],
        &mock.invocation(),
    );
    assert!(mock.requests().is_empty());
    assert_eq!(
        mock.subscriptions(),
        [openidea_plugin::EventKind::WebsocketReady].into()
    );
    assert!(mock.path_subscriptions().is_empty());
    assert!(host::storage::get("room_state").unwrap().is_none());
    assert_eq!(
        mock.invocation()
            .project()
            .unwrap()
            .read_file(".env")
            .unwrap(),
        "PRIVATE"
    );
}
#[test]
fn credentials_are_kept_in_keychain_and_project_switch_does_not_reuse_session() {
    let mock = mock();
    let token = "A".repeat(43);
    mock.on_fetch(move |request| {
        assert_eq!(request.url, format!("{DEFAULT_ORIGIN}/v1/sessions"));
        Ok(response(json!({"session_id":"11111111-1111-4111-8111-111111111111", "member_id":"22222222-2222-4222-8222-222222222222", "credential":token,"role":"owner","expires_at":9999999999u64,"websocket_url":"wss://collab.synthosia.com/v1/socket"})))
    });
    mock.answer_next_prompt(Some(&["yes"]));
    let mut plugin = CollabPlugin::default();
    plugin
        .run_command("create-session", &mock.invocation(), None)
        .unwrap();
    assert!(host::secret_is_set("member-credential"));
    assert!(
        !host::storage::get("session_v1")
            .unwrap()
            .unwrap()
            .contains(&"A".repeat(43))
    );
    assert!(state::for_project(&mock.invocation()).unwrap().is_some());
    mock.set_project_root(Some("/other-project"));
    assert!(state::for_project(&mock.invocation()).is_err());
    assert!(
        plugin
            .run_command("create-invite", &mock.invocation(), None)
            .is_err()
    );
    assert_eq!(mock.requests().len(), 1);
}
#[test]
fn declined_consent_and_unexpected_invitation_hosts_do_not_send_requests() {
    let mock = mock();
    let mut plugin = CollabPlugin::default();
    plugin
        .run_command("create-session", &mock.invocation(), None)
        .unwrap();
    assert!(mock.requests().is_empty());
    mock.answer_next_prompt(Some(&["yes"]));
    mock.answer_next_prompt(Some(&["https://evil.example/join/11111111-1111-4111-8111-111111111111#AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"]));
    assert!(
        plugin
            .run_command("join-session", &mock.invocation(), None)
            .is_err()
    );
    assert!(mock.requests().is_empty());
}

#[test]
fn already_revoked_membership_can_be_forgotten_without_retaining_credentials() {
    let mock = mock();
    state::save(&mock.invocation(), "https://collab.synthosia.com".into(), serde_json::from_value(json!({
        "session_id":"11111111-1111-4111-8111-111111111111", "member_id":"22222222-2222-4222-8222-222222222222",
        "credential":"A".repeat(43), "role":"owner", "expires_at":9999999999u64,
        "websocket_url":"wss://collab.synthosia.com/v1/socket"
    })).unwrap()).unwrap();
    mock.on_fetch(|request| {
        assert_eq!(request.method, "DELETE");
        Ok(HttpResponse {
            status: 401,
            headers: vec![],
            body: vec![],
        })
    });
    mock.answer_next_prompt(Some(&["yes"]));
    CollabPlugin::default()
        .run_command("leave-session", &mock.invocation(), None)
        .unwrap();
    assert!(!host::secret_is_set("member-credential"));
    assert!(state::load().unwrap().is_none());
}

#[test]
fn document_change_flows_through_crdt_websocket_and_remote_edit_returns_to_editor() {
    let mock = mock();
    mock.open_editor("/project/src/main.rs", "rust", "hello", &[]);
    mock.on_fetch(|request| {
        assert!(request.url.ends_with("/documents"));
        Ok(HttpResponse {
            status: 204,
            headers: vec![],
            body: vec![],
        })
    });
    let credential = serde_json::from_value(json!({
        "session_id":"11111111-1111-4111-8111-111111111111",
        "member_id":"22222222-2222-4222-8222-222222222222",
        "credential":"A".repeat(43),
        "role":"owner",
        "expires_at":9999999999u64,
        "websocket_url":"wss://collab.synthosia.com/v1/socket"
    }))
    .unwrap();
    state::save(
        &mock.invocation(),
        "https://collab.synthosia.com".into(),
        credential,
    )
    .unwrap();

    let mut plugin = CollabPlugin::default();
    plugin
        .activate(&mock.context("sorputi.openidea-collab"))
        .unwrap();
    plugin
        .run_command("connect-realtime", &mock.invocation(), None)
        .unwrap();
    mock.answer_next_prompt(Some(&["yes"]));
    plugin
        .run_command("share-open-documents", &mock.invocation(), None)
        .unwrap();

    let socket = mock.websockets().remove(0);
    assert_eq!(socket.request.url, "wss://collab.synthosia.com/v1/socket");
    assert_eq!(
        socket.request.headers[0].1,
        format!("Bearer {}", "A".repeat(43))
    );
    assert_eq!(
        mock.path_subscriptions()["document-changed"],
        ["src/main.rs"]
    );
    let session = state::load().unwrap().unwrap();
    let document_id = session.documents[0].document_id;
    let snapshot = socket
        .sent
        .iter()
        .map(|bytes| Frame::decode(bytes).unwrap())
        .find(|frame| frame.kind == Kind::Snapshot)
        .unwrap();
    assert_eq!(snapshot.document, document_id);
    let mut peer = Document::default();
    peer.receive(snapshot.payload).unwrap();
    assert_eq!(peer.text(), "hello");

    let project = mock.invocation().project().unwrap();
    let local = project.document("src/main.rs").unwrap();
    let edit = TextEdit {
        range: Range::new(Position::new(0, 5), Position::new(0, 5)),
        text: "!".into(),
    };
    let version = project
        .apply_document_edits("src/main.rs", local.version, &[edit])
        .unwrap();
    plugin.handle_events(
        &[Event::DocumentChanged(DocumentChanged {
            path: "src/main.rs".into(),
            base_version: local.version,
            version,
            changes: vec![DocumentTextChange {
                from: 5,
                to: 5,
                insert: "!".into(),
            }],
        })],
        &mock.invocation(),
    );
    let socket = mock.websockets().remove(0);
    let update = socket
        .sent
        .iter()
        .map(|bytes| Frame::decode(bytes).unwrap())
        .find(|frame| frame.kind == Kind::Update)
        .unwrap();
    peer.receive(update.payload).unwrap();
    assert_eq!(peer.text(), "hello!");

    plugin
        .run_command("disconnect-realtime", &mock.invocation(), None)
        .unwrap();
    assert!(mock.websockets().iter().all(|candidate| candidate.closed));
    assert!(
        mock.path_subscriptions()
            .get("document-changed")
            .is_none_or(Vec::is_empty)
    );
    plugin
        .run_command("connect-realtime", &mock.invocation(), None)
        .unwrap();
    let socket = mock
        .websockets()
        .into_iter()
        .find(|candidate| !candidate.closed)
        .unwrap();
    assert!(
        socket
            .sent
            .iter()
            .map(|bytes| Frame::decode(bytes).unwrap())
            .any(|frame| frame.kind == Kind::Update)
    );

    let operation = peer.edit(6, 6, " remote").unwrap();
    let update = peer.operation(operation).unwrap().update.clone();
    let sender = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    for frame in split(Kind::Update, document_id, Uuid::nil(), operation, &update).unwrap() {
        let mut delivery = sender.as_bytes().to_vec();
        delivery.extend_from_slice(&frame);
        mock.push_websocket_message(socket.id, &delivery).unwrap();
    }
    plugin.handle_events(
        &[Event::WebSocketReady(WebSocketReady {
            plugin_id: "sorputi.openidea-collab".into(),
            connection: socket.id,
        })],
        &mock.invocation(),
    );

    assert_eq!(mock.editor_text(), "hello! remote");
    assert!(
        mock.websockets()
            .iter()
            .flat_map(|socket| &socket.sent)
            .map(|bytes| Frame::decode(bytes).unwrap())
            .any(|frame| frame.kind == Kind::Ack)
    );
}

#[test]
fn differing_initial_content_is_preserved_without_acknowledging_remote_snapshot() {
    let mock = mock();
    mock.open_editor("/project/src/main.rs", "rust", "local work", &[]);
    let credential = serde_json::from_value(json!({
        "session_id":"11111111-1111-4111-8111-111111111111",
        "member_id":"22222222-2222-4222-8222-222222222222",
        "credential":"A".repeat(43),
        "role":"editor",
        "expires_at":9999999999u64,
        "websocket_url":"wss://collab.synthosia.com/v1/socket"
    }))
    .unwrap();
    state::save(
        &mock.invocation(),
        "https://collab.synthosia.com".into(),
        credential,
    )
    .unwrap();
    let mut plugin = CollabPlugin::default();
    plugin
        .activate(&mock.context("sorputi.openidea-collab"))
        .unwrap();
    plugin
        .run_command("connect-realtime", &mock.invocation(), None)
        .unwrap();
    mock.answer_next_prompt(Some(&["yes"]));
    plugin
        .run_command("share-current-document", &mock.invocation(), None)
        .unwrap();
    assert!(host::storage::keys("checkpoint/").unwrap().is_empty());

    let session = state::load().unwrap().unwrap();
    let socket = mock.websockets().remove(0);
    let remote = Document::from_text("remote work").unwrap();
    let sender = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    for frame in split(
        Kind::Snapshot,
        session.documents[0].document_id,
        Uuid::nil(),
        Uuid::new_v4(),
        &remote.snapshot(),
    )
    .unwrap()
    {
        let mut delivery = sender.as_bytes().to_vec();
        delivery.extend_from_slice(&frame);
        mock.push_websocket_message(socket.id, &delivery).unwrap();
    }
    plugin.handle_events(
        &[Event::WebSocketReady(WebSocketReady {
            plugin_id: "sorputi.openidea-collab".into(),
            connection: socket.id,
        })],
        &mock.invocation(),
    );

    assert_eq!(mock.editor_text(), "local work");
    assert!(host::storage::keys("checkpoint/").unwrap().is_empty());
    assert_eq!(
        mock.notifications().last().unwrap().title,
        "Collaboration update refused"
    );
    assert!(
        !mock.websockets()[0]
            .sent
            .iter()
            .map(|bytes| Frame::decode(bytes).unwrap())
            .any(|frame| frame.kind == Kind::Ack)
    );
}
