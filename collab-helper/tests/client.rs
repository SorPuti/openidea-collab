use openidea_collab_helper::Client;

async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, collab_server::realtime::router())
            .await
            .unwrap()
    });
    (format!("http://{addr}"), task)
}

#[tokio::test]
async fn clients_reconcile_concurrent_utf16_edits_through_real_server() {
    let (base, server) = server().await;
    let http = reqwest::Client::new();
    let created: serde_json::Value = http
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room = created["room_id"].as_str().unwrap();
    let token = created["owner_token"].as_str().unwrap();
    let mut first = Client::attach(&base, room, "main", token, "A😀B")
        .await
        .unwrap();
    let mut second = Client::attach(&base, room, "main", token, "A😀B")
        .await
        .unwrap();
    first.edit(1, 1, "X").unwrap();
    second.edit(3, 3, "Y").unwrap();
    first.sync().await.unwrap();
    second.sync().await.unwrap();
    first.sync().await.unwrap();
    assert_eq!(first.text(), second.text());
    assert!(first.text().contains('X') && first.text().contains('Y'));
    assert_eq!(first.pending_count(), 0);
    assert_eq!(second.pending_count(), 0);
    server.abort();
}

#[tokio::test]
async fn attach_refuses_to_overwrite_a_different_local_file() {
    let (base, server) = server().await;
    let http = reqwest::Client::new();
    let created: serde_json::Value = http
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room = created["room_id"].as_str().unwrap();
    let token = created["owner_token"].as_str().unwrap();
    let _first = Client::attach(&base, room, "main", token, "original")
        .await
        .unwrap();
    assert!(Client::attach(&base, room, "main", token, "local work")
        .await
        .is_err());
    server.abort();
}

#[tokio::test]
async fn transport_failure_keeps_local_edits_in_memory() {
    let (base, server) = server().await;
    let http = reqwest::Client::new();
    let created: serde_json::Value = http
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut client = Client::attach(
        &base,
        created["room_id"].as_str().unwrap(),
        "main",
        created["owner_token"].as_str().unwrap(),
        "a",
    )
    .await
    .unwrap();
    server.abort();
    client.edit(1, 1, "b").unwrap();
    assert!(client.sync().await.is_err());
    assert_eq!(client.text(), "ab");
    assert_eq!(client.pending_count(), 1);
}

#[test]
fn unencrypted_remote_endpoints_are_rejected() {
    assert!(Client::validate_endpoint("http://example.com").is_err());
    assert!(Client::validate_endpoint("https://user:password@example.com").is_err());
    assert!(Client::validate_endpoint("http://127.0.0.1:4444").is_ok());
}

#[tokio::test]
async fn waiting_client_receives_remote_update_without_polling_on_a_timer() {
    let (base, server) = server().await;
    let http = reqwest::Client::new();
    let created: serde_json::Value = http
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room = created["room_id"].as_str().unwrap();
    let token = created["owner_token"].as_str().unwrap();
    let mut author = Client::attach(&base, room, "main", token, "initial")
        .await
        .unwrap();
    let mut receiver = Client::attach(&base, room, "main", token, "initial")
        .await
        .unwrap();
    let waiting = tokio::spawn(async move {
        let changed = receiver.wait_for_change().await.unwrap();
        (receiver, changed)
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    author.edit(7, 7, "!").unwrap();
    author.sync().await.unwrap();
    let (receiver, changed) = tokio::time::timeout(std::time::Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(changed);
    assert_eq!(receiver.text(), "initial!");
    server.abort();
}

#[tokio::test]
async fn owner_can_create_a_one_use_editor_invite_and_collaborate() {
    use openidea_collab_helper::SessionApi;
    let (base, server) = server().await;
    let api = SessionApi::new(&base).unwrap();
    let owner = api.create_room().await.unwrap();
    let mut alice = Client::attach(&base, &owner.room_id, "main", owner.token(), "ab")
        .await
        .unwrap();
    let invite = api
        .create_invite(&owner.room_id, owner.token(), "editor", 60)
        .await
        .unwrap();
    let member = api.join_room(&owner.room_id, invite.token()).await.unwrap();
    assert!(api.join_room(&owner.room_id, invite.token()).await.is_err());
    let mut bob = Client::attach(&base, &owner.room_id, "main", member.token(), "ab")
        .await
        .unwrap();
    bob.edit(1, 1, "X").unwrap();
    bob.sync().await.unwrap();
    alice.sync().await.unwrap();
    assert_eq!(alice.text(), "aXb");
    server.abort();
}
