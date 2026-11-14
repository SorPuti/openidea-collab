#![cfg(feature = "experimental-remote-edits")]

use openidea_collab::{CollabPlugin, state};
use openidea_collab_protocol::{Frame, Kind};
use openidea_plugin::{
    DocumentChanged, DocumentTextChange, Event, HttpResponse, Plugin, Position, Range, TextEdit,
    WebSocketReady, mock::MockHost,
};
use serde_json::json;
use std::{sync::mpsc, thread};
use uuid::Uuid;

enum Action {
    Deliver(Uuid, Vec<Vec<u8>>),
    Append(&'static str),
    Quit,
}
struct ResultState {
    text: String,
    sent: Vec<Vec<u8>>,
}
fn peer(
    role: &'static str,
    member: Uuid,
) -> (
    mpsc::Sender<Action>,
    mpsc::Receiver<ResultState>,
    thread::JoinHandle<()>,
) {
    let (send, actions) = mpsc::channel();
    let (results, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
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
        mock.open_editor(
            "/project/src/main.rs",
            "rust",
            if role == "owner" { "hello" } else { "" },
            &[],
        );
        mock.on_fetch(|_| {
            Ok(HttpResponse {
                status: 204,
                headers: vec![],
                body: vec![],
            })
        });
        let credential = serde_json::from_value(json!({
            "session_id":"11111111-1111-4111-8111-111111111111", "member_id": member.to_string(),
            "credential":"A".repeat(43), "role":role, "expires_at":9999999999u64,
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
        mock.answer_next_prompt(Some(&["yes"]));
        plugin
            .run_command("connect-realtime", &mock.invocation(), None)
            .unwrap();
        let socket_id = mock.websockets()[0].id;
        let initial = mock.websockets()[0].sent.clone();
        let mut cursor = initial.len();
        results
            .send(ResultState {
                text: mock.editor_text(),
                sent: initial,
            })
            .unwrap();
        while let Ok(action) = actions.recv() {
            match action {
                Action::Deliver(sender, frames) => {
                    for frame in frames {
                        let mut bytes = sender.as_bytes().to_vec();
                        bytes.extend_from_slice(&frame);
                        mock.push_websocket_message(socket_id, &bytes).unwrap();
                    }
                    plugin.handle_events(
                        &[Event::WebSocketReady(WebSocketReady {
                            plugin_id: "sorputi.openidea-collab".into(),
                            connection: socket_id,
                        })],
                        &mock.invocation(),
                    );
                }
                Action::Append(text) => {
                    let project = mock.invocation().project().unwrap();
                    let current = project.document("src/main.rs").unwrap();
                    let offset = current.text.encode_utf16().count() as u32;
                    let version = project
                        .apply_document_edits(
                            "src/main.rs",
                            current.version,
                            &[TextEdit {
                                range: Range::new(
                                    Position::new(0, offset),
                                    Position::new(0, offset),
                                ),
                                text: text.into(),
                            }],
                        )
                        .unwrap();
                    plugin.handle_events(
                        &[Event::DocumentChanged(DocumentChanged {
                            path: "src/main.rs".into(),
                            base_version: current.version,
                            version,
                            changes: vec![DocumentTextChange {
                                from: offset,
                                to: offset,
                                insert: text.into(),
                            }],
                        })],
                        &mock.invocation(),
                    );
                }
                Action::Quit => break,
            }
            let socket = mock
                .websockets()
                .into_iter()
                .find(|s| s.id == socket_id)
                .unwrap();
            let sent = socket.sent[cursor..].to_vec();
            cursor = socket.sent.len();
            results
                .send(ResultState {
                    text: mock.editor_text(),
                    sent,
                })
                .unwrap();
        }
    });
    (send, receive, worker)
}
fn frames_of(sent: Vec<Vec<u8>>, kind: Kind) -> Vec<Vec<u8>> {
    sent.into_iter()
        .filter(|bytes| Frame::decode(bytes).unwrap().kind == kind)
        .collect()
}
#[test]
fn two_plugin_instances_exchange_edits_in_both_directions() {
    let owner_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let guest_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    let (owner, owner_results, owner_thread) = peer("owner", owner_id);
    let (guest, guest_results, guest_thread) = peer("editor", guest_id);
    let initial_owner = owner_results.recv().unwrap();
    guest_results.recv().unwrap();
    let snapshot = frames_of(initial_owner.sent, Kind::Snapshot);
    assert!(!snapshot.is_empty());
    guest.send(Action::Deliver(owner_id, snapshot)).unwrap();
    assert_eq!(guest_results.recv().unwrap().text, "hello");
    guest.send(Action::Append(" guest")).unwrap();
    let guest_update = frames_of(guest_results.recv().unwrap().sent, Kind::Update);
    assert!(!guest_update.is_empty());
    owner.send(Action::Deliver(guest_id, guest_update)).unwrap();
    assert_eq!(owner_results.recv().unwrap().text, "hello guest");
    owner.send(Action::Append(" owner")).unwrap();
    let owner_update = frames_of(owner_results.recv().unwrap().sent, Kind::Update);
    assert!(!owner_update.is_empty());
    guest.send(Action::Deliver(owner_id, owner_update)).unwrap();
    assert_eq!(guest_results.recv().unwrap().text, "hello guest owner");
    owner.send(Action::Quit).unwrap();
    guest.send(Action::Quit).unwrap();
    owner_thread.join().unwrap();
    guest_thread.join().unwrap();
}
