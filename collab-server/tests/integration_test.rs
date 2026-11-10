use collab_server::routes::router;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let state = Arc::new(Mutex::new(HashMap::new()));
    let task = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    (format!("http://{address}"), task)
}

#[tokio::test]
async fn two_clients_can_exchange_saved_file_and_stale_updates_fail() {
    let (base, task) = server().await;
    let client = reqwest::Client::new();
    let room: serde_json::Value = client
        .post(format!("{base}/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = room["room_id"].as_str().unwrap();
    let mut users = Vec::new();
    for name in ["Ana", "Bob"] {
        let member: serde_json::Value = client
            .post(format!("{base}/rooms/{id}/join"))
            .json(&serde_json::json!({"name":name}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        users.push(member["user_id"].as_str().unwrap().to_owned());
    }
    let url = format!("{base}/rooms/{id}/files/src/main.rs");
    let update = |user: &str, version, text: &str| serde_json::json!({"user_id":user,"version":version,"text":text});
    assert_eq!(
        client
            .post(&url)
            .json(&update(&users[0], 0, "one"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let received: serde_json::Value = client.get(&url).send().await.unwrap().json().await.unwrap();
    assert_eq!(received["text"], "one");
    assert_eq!(received["version"], 1);
    assert_eq!(
        client
            .post(&url)
            .json(&update(&users[1], 0, "stale"))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        client
            .post(&url)
            .json(&update(&users[0], 0, "also stale"))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let current: serde_json::Value = client.get(&url).send().await.unwrap().json().await.unwrap();
    assert_eq!(current["text"], "one");
    task.abort();
}

#[tokio::test]
async fn pending_event_poll_is_woken_by_new_event() {
    let (base, task) = server().await;
    let client = reqwest::Client::new();
    let room: serde_json::Value = client
        .post(format!("{base}/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = room["room_id"].as_str().unwrap();
    let event_url = format!("{base}/rooms/{id}/events?cursor=0");
    let pending = tokio::spawn({
        let client = client.clone();
        async move { client.get(event_url).send().await.unwrap() }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    client
        .post(format!("{base}/rooms/{id}/join"))
        .json(&serde_json::json!({"name":"Ana"}))
        .send()
        .await
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), pending)
        .await
        .expect("poll must wake when an event arrives")
        .unwrap();
    let payload: serde_json::Value = response.json().await.unwrap();
    assert_eq!(payload["events"][0]["event"]["type"], "member_joined");
    task.abort();
}
