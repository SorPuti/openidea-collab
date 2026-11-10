use openidea_collab::{CollabPlugin, state};
use openidea_collab_protocol::DEFAULT_ORIGIN;
use openidea_plugin::{Event, HttpResponse, Plugin, host, mock::MockHost};
use serde_json::json;
fn mock() -> MockHost {
    let mock = MockHost::install().with_capabilities(&[
        "storage",
        "secrets",
        "project.read",
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
    assert!(mock.subscriptions().is_empty());
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
    state::save(&mock.invocation(), serde_json::from_value(json!({
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
