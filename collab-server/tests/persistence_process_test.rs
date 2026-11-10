use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn start(path: &Path) -> (String, Server) {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let server = Server(
        Command::new(env!("CARGO_BIN_EXE_collab-server"))
            .env("COLLAB_API", "realtime")
            .env("COLLAB_BIND", "127.0.0.1")
            .env("COLLAB_PORT", port.to_string())
            .env("COLLAB_JOURNAL", path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();
    for _ in 0..100 {
        if client.get(format!("{base}/health")).send().await.is_ok() {
            return (base, server);
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("server failed to start");
}

#[tokio::test]
async fn acknowledged_room_invite_document_and_revocation_survive_abrupt_restart() {
    let root = std::env::temp_dir().join(format!("collab-persist-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("journal.sqlite");
    let client = reqwest::Client::new();
    let (base, server) = start(&path).await;
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
    let invites = format!("{base}/v1/rooms/{room}/invites");
    let invite: serde_json::Value = client
        .post(&invites)
        .bearer_auth(owner)
        .json(&serde_json::json!({"role":"editor","ttl_seconds":600}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = invite["token"].as_str().unwrap();
    let doc = Doc::new();
    doc.get_or_insert_text("content")
        .push(&mut doc.transact_mut(), "texto persistido 🌍");
    let update = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let document = format!("{base}/v1/rooms/{room}/documents/main");
    assert_eq!(
        client
            .put(&document)
            .bearer_auth(owner)
            .body(update)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    drop(server);

    let (base, server) = start(&path).await;
    let join_url = format!("{base}/v1/rooms/{room}/join");
    let joined: serde_json::Value = client
        .post(&join_url)
        .json(&serde_json::json!({"token":token}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let member = joined["member_id"].as_str().unwrap();
    let member_token = joined["member_token"].as_str().unwrap();
    assert_eq!(
        client
            .post(&join_url)
            .json(&serde_json::json!({"token":token}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let response = client
        .get(format!("{base}/v1/rooms/{room}/documents/main"))
        .bearer_auth(member_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["x-collab-revision"], "1");
    let recovered = Doc::new();
    recovered
        .transact_mut()
        .apply_update(Update::decode_v1(&response.bytes().await.unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        recovered
            .get_or_insert_text("content")
            .get_string(&recovered.transact()),
        "texto persistido 🌍"
    );
    let before = recovered.transact().state_vector();
    recovered
        .get_or_insert_text("content")
        .push(&mut recovered.transact_mut(), " atualizado");
    let update = recovered.transact().encode_state_as_update_v1(&before);
    assert_eq!(
        client
            .post(format!("{base}/v1/rooms/{room}/documents/main"))
            .bearer_auth(member_token)
            .body(update)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .delete(format!("{base}/v1/rooms/{room}/members/{member}"))
            .bearer_auth(owner)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    drop(server);

    let (base, server) = start(&path).await;
    assert_eq!(
        client
            .get(format!("{base}/v1/rooms/{room}/documents/main"))
            .bearer_auth(member_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .post(format!("{base}/v1/rooms/{room}/join"))
            .json(&serde_json::json!({"token":token}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let response = client
        .get(format!("{base}/v1/rooms/{room}/documents/main"))
        .bearer_auth(owner)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["x-collab-revision"], "2");
    let recovered = Doc::new();
    recovered
        .transact_mut()
        .apply_update(Update::decode_v1(&response.bytes().await.unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        recovered
            .get_or_insert_text("content")
            .get_string(&recovered.transact()),
        "texto persistido 🌍 atualizado"
    );
    drop(server);
    let journal = std::fs::read(&path).unwrap();
    for secret in [owner, token, member_token] {
        assert!(!journal
            .windows(secret.len())
            .any(|w| w == secret.as_bytes()));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn pending_causal_update_is_preserved_across_restart() {
    let root = std::env::temp_dir().join(format!("collab-persist-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("journal.sqlite");
    let client = reqwest::Client::new();
    let (base, server) = start(&path).await;
    let room: serde_json::Value = client
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = room["owner_token"].as_str().unwrap();
    let room_id = room["room_id"].as_str().unwrap();
    let url = format!("{base}/v1/rooms/{room_id}/documents/main");
    let empty = Doc::new()
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    assert_eq!(
        client
            .put(&url)
            .bearer_auth(token)
            .body(empty)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let author = Doc::new();
    let text = author.get_or_insert_text("content");
    text.push(&mut author.transact_mut(), "A");
    let first = author
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let before_second = author.transact().state_vector();
    text.push(&mut author.transact_mut(), "B");
    let second = author.transact().encode_state_as_update_v1(&before_second);
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(token)
            .body(second)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    drop(server);
    let (base, server) = start(&path).await;
    let url = format!("{base}/v1/rooms/{room_id}/documents/main");
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(token)
            .body(first)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let response = client.get(&url).bearer_auth(token).send().await.unwrap();
    let recovered = Doc::new();
    recovered
        .transact_mut()
        .apply_update(Update::decode_v1(&response.bytes().await.unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        recovered
            .get_or_insert_text("content")
            .get_string(&recovered.transact()),
        "AB"
    );
    drop(server);
    std::fs::remove_dir_all(root).unwrap();
}
