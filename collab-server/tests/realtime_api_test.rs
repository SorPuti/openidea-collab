use collab_server::realtime;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task =
        tokio::spawn(async move { axum::serve(listener, realtime::router()).await.unwrap() });
    (format!("http://{addr}"), task)
}

#[tokio::test]
async fn authenticated_members_merge_concurrent_edits_and_viewer_cannot_write() {
    let (base, server) = server().await;
    let client = reqwest::Client::new();
    let create: serde_json::Value = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room = create["room_id"].as_str().unwrap();
    let owner = create["owner_token"].as_str().unwrap();
    let docs = format!("{base}/v1/rooms/{room}/documents/main");
    assert_eq!(client.get(&docs).send().await.unwrap().status(), 401);

    let invite: serde_json::Value = client
        .post(format!("{base}/v1/rooms/{room}/invites"))
        .bearer_auth(owner)
        .json(&serde_json::json!({"role":"viewer","ttl_seconds":60}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let invite_token = invite["token"].as_str().unwrap();
    let join = || {
        client
            .post(format!("{base}/v1/rooms/{room}/join"))
            .json(&serde_json::json!({"token":invite_token}))
    };
    let viewer: serde_json::Value = join().send().await.unwrap().json().await.unwrap();
    let token = viewer["member_token"].as_str().unwrap();
    assert_eq!(
        join().send().await.unwrap().status(),
        401,
        "invite is one-use"
    );

    let a = Doc::new();
    let b = Doc::new();
    let ta = a.get_or_insert_text("content");
    let tb = b.get_or_insert_text("content");
    ta.push(&mut a.transact_mut(), "Olá ");
    tb.push(&mut b.transact_mut(), "🌍");
    let ua = a
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let ub = b
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    assert_eq!(
        client
            .post(&docs)
            .bearer_auth(token)
            .body(ub.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .put(&docs)
            .bearer_auth(owner)
            .body(ua)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .post(&docs)
            .bearer_auth(owner)
            .body(ub)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let response = client.get(&docs).bearer_auth(token).send().await.unwrap();
    assert_eq!(response.headers()["x-collab-revision"], "2");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let snapshot = response.bytes().await.unwrap();
    a.transact_mut()
        .apply_update(Update::decode_v1(&snapshot).unwrap())
        .unwrap();
    b.transact_mut()
        .apply_update(Update::decode_v1(&snapshot).unwrap())
        .unwrap();
    assert_eq!(ta.get_string(&a.transact()), tb.get_string(&b.transact()));
    let id = viewer["member_id"].as_str().unwrap();
    assert_eq!(
        client
            .delete(format!("{base}/v1/rooms/{room}/members/{id}"))
            .bearer_auth(owner)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .get(&docs)
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.abort();
}

#[tokio::test]
async fn waiting_client_is_notified_when_a_document_changes() {
    let (base, server) = server().await;
    let client = reqwest::Client::new();
    let create: serde_json::Value = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room = create["room_id"].as_str().unwrap();
    let owner = create["owner_token"].as_str().unwrap().to_owned();
    let docs = format!("{base}/v1/rooms/{room}/documents/main");
    let doc = Doc::new();
    let text = doc.get_or_insert_text("content");
    text.push(&mut doc.transact_mut(), "one");
    let first = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    assert_eq!(
        client
            .put(&docs)
            .bearer_auth(&owner)
            .body(first)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let waiting = tokio::spawn({
        let client = client.clone();
        let owner = owner.clone();
        let docs = docs.clone();
        async move {
            client
                .get(format!("{docs}/changes?after=1"))
                .bearer_auth(owner)
                .send()
                .await
                .unwrap()
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    text.push(&mut doc.transact_mut(), " two");
    let second = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    assert_eq!(
        client
            .post(&docs)
            .bearer_auth(&owner)
            .body(second)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let response = tokio::time::timeout(std::time::Duration::from_secs(2), waiting)
        .await
        .expect("must wake on update")
        .unwrap();
    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["revision"], 2);
    server.abort();
}

#[tokio::test]
async fn revoked_member_cannot_receive_a_pending_notification() {
    let (base, server) = server().await;
    let client = reqwest::Client::new();
    let created: serde_json::Value = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room = created["room_id"].as_str().unwrap();
    let owner = created["owner_token"].as_str().unwrap();
    let inv: serde_json::Value = client
        .post(format!("{base}/v1/rooms/{room}/invites"))
        .bearer_auth(owner)
        .json(&serde_json::json!({"role":"viewer","ttl_seconds":60}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let member: serde_json::Value = client
        .post(format!("{base}/v1/rooms/{room}/join"))
        .json(&serde_json::json!({"token":inv["token"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let member_token = member["member_token"].as_str().unwrap().to_owned();
    let docs = format!("{base}/v1/rooms/{room}/documents/main");
    let doc = Doc::new();
    let text = doc.get_or_insert_text("content");
    text.push(&mut doc.transact_mut(), "one");
    let initial = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    client
        .put(&docs)
        .bearer_auth(owner)
        .body(initial.clone())
        .send()
        .await
        .unwrap();
    let waiting = tokio::spawn({
        let client = client.clone();
        let docs = docs.clone();
        async move {
            client
                .get(format!("{docs}/changes?after=1"))
                .bearer_auth(member_token)
                .send()
                .await
                .unwrap()
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(
        client
            .delete(format!(
                "{base}/v1/rooms/{room}/members/{}",
                member["member_id"].as_str().unwrap()
            ))
            .bearer_auth(owner)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    client
        .post(&docs)
        .bearer_auth(owner)
        .body(initial)
        .send()
        .await
        .unwrap();
    let response = tokio::time::timeout(std::time::Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), 401);
    server.abort();
}

#[tokio::test]
async fn owner_only_can_issue_invites_and_oversize_updates_are_rejected() {
    let (base, server) = server().await;
    let client = reqwest::Client::new();
    let create_response = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap();
    assert_eq!(create_response.headers()["cache-control"], "no-store");
    let created: serde_json::Value = create_response.json().await.unwrap();
    let room = created["room_id"].as_str().unwrap();
    let owner = created["owner_token"].as_str().unwrap();
    let invites = format!("{base}/v1/rooms/{room}/invites");
    let invalid = "0".repeat(64);
    assert_eq!(
        client
            .post(&invites)
            .bearer_auth(invalid)
            .json(&serde_json::json!({"role":"editor","ttl_seconds":60}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .post(&invites)
            .bearer_auth(owner)
            .json(&serde_json::json!({"role":"editor","ttl_seconds":0}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let docs = format!("{base}/v1/rooms/{room}/documents/main");
    assert_eq!(
        client
            .post(&docs)
            .bearer_auth(owner)
            .body(vec![0; 1024 * 1024 + 1])
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    assert_eq!(
        client
            .get(&docs)
            .bearer_auth(owner)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.abort();
}

#[tokio::test]
async fn initialization_is_atomic_and_does_not_merge_two_separate_initial_files() {
    let (base, server) = server().await;
    let client = reqwest::Client::new();
    let created: serde_json::Value = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let owner = created["owner_token"].as_str().unwrap();
    let url = format!(
        "{base}/v1/rooms/{}/documents/main",
        created["room_id"].as_str().unwrap()
    );
    let a = Doc::new();
    a.get_or_insert_text("content")
        .push(&mut a.transact_mut(), "file A");
    let b = Doc::new();
    b.get_or_insert_text("content")
        .push(&mut b.transact_mut(), "file B");
    let ua = a
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let ub = b
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let (ra, rb) = tokio::join!(
        client.put(&url).bearer_auth(owner).body(ua).send(),
        client.put(&url).bearer_auth(owner).body(ub).send()
    );
    let statuses = [ra.unwrap().status().as_u16(), rb.unwrap().status().as_u16()];
    assert!(statuses.contains(&204) && statuses.contains(&409));
    server.abort();
}

#[tokio::test]
async fn browser_origin_cannot_create_local_rooms() {
    let (base, server) = server().await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{base}/v1/rooms"))
        .header("Origin", "https://untrusted.example")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert_eq!(
        client
            .post(format!("{base}/v1/rooms"))
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    server.abort();
}

#[tokio::test]
async fn too_large_result_is_rejected_without_changing_readable_document() {
    let (base, server) = server().await;
    let client = reqwest::Client::new();
    let room: serde_json::Value = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = room["owner_token"].as_str().unwrap();
    let url = format!(
        "{base}/v1/rooms/{}/documents/main",
        room["room_id"].as_str().unwrap()
    );
    let doc = Doc::new();
    let text = doc.get_or_insert_text("content");
    text.push(&mut doc.transact_mut(), &"a".repeat(600_000));
    let first = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    assert_eq!(
        client
            .put(&url)
            .bearer_auth(token)
            .body(first)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let previous = doc.transact().state_vector();
    text.push(&mut doc.transact_mut(), &"b".repeat(600_000));
    let second = doc.transact().encode_state_as_update_v1(&previous);
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(token)
            .body(second)
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    let response = client.get(&url).bearer_auth(token).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["x-collab-revision"], "1");
    let result = Doc::new();
    result
        .transact_mut()
        .apply_update(Update::decode_v1(&response.bytes().await.unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        result
            .get_or_insert_text("content")
            .get_string(&result.transact())
            .len(),
        600_000
    );
    server.abort();
}

#[tokio::test]
async fn storage_quota_rejects_edits_without_losing_state_and_allows_revocation() {
    use collab_server::journal::JournalLimits;
    let root = std::env::temp_dir().join(format!("collab-quota-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let router = realtime::persistent_router_with_limits(
        &root.join("journal.sqlite"),
        JournalLimits {
            normal_bytes: 5000,
            revoke_reserve_bytes: 1024,
        },
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let created: serde_json::Value = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room = created["room_id"].as_str().unwrap();
    let owner = created["owner_token"].as_str().unwrap();
    let invite: serde_json::Value = client
        .post(format!("{base}/v1/rooms/{room}/invites"))
        .bearer_auth(owner)
        .json(&serde_json::json!({"role":"editor","ttl_seconds":60}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let member: serde_json::Value = client
        .post(format!("{base}/v1/rooms/{room}/join"))
        .json(&serde_json::json!({"token":invite["token"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let url = format!("{base}/v1/rooms/{room}/documents/main");
    let doc = Doc::new();
    doc.get_or_insert_text("content")
        .push(&mut doc.transact_mut(), "original");
    let update = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    assert_eq!(
        client
            .put(&url)
            .bearer_auth(owner)
            .body(update.clone())
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let mut rejected = false;
    for _ in 0..100 {
        let status = client
            .post(&url)
            .bearer_auth(owner)
            .body(update.clone())
            .send()
            .await
            .unwrap()
            .status();
        if status == 507 {
            rejected = true;
            break;
        }
        assert_eq!(status, 204);
    }
    assert!(rejected, "quota was not enforced");
    let response = client.get(&url).bearer_auth(owner).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let recovered = Doc::new();
    recovered
        .transact_mut()
        .apply_update(Update::decode_v1(&response.bytes().await.unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        recovered
            .get_or_insert_text("content")
            .get_string(&recovered.transact()),
        "original"
    );
    assert_eq!(
        client
            .delete(format!(
                "{base}/v1/rooms/{room}/members/{}",
                member["member_id"].as_str().unwrap()
            ))
            .bearer_auth(owner)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(member["member_token"].as_str().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.abort();
    let _ = server.await;
    std::fs::remove_dir_all(root).unwrap();
}
