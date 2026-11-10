use collab_server::routes::router;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct TestServer {
    url: String,
    client: Client,
    task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        // Constructing this router catches incompatible Axum route syntax.
        let app = router(Arc::new(Mutex::new(HashMap::new())));
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        Self { url, client, task }
    }

    async fn room(&self) -> String {
        let response = self
            .client
            .post(format!("{}/rooms", self.url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        response.json::<Value>().await.unwrap()["room_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn join(&self, room: &str, name: &str) -> String {
        let response = self
            .client
            .post(format!("{}/rooms/{room}/join", self.url))
            .json(&json!({"name": name}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.json::<Value>().await.unwrap()["user_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn room_lifecycle_and_presence() {
    let server = TestServer::start().await;
    let room = server.room().await;
    let user = server.join(&room, "Alice").await;
    let response = server
        .client
        .post(format!(
            "{}/rooms/{room}/members/{user}/presence",
            server.url
        ))
        .json(&json!({"current_file":"src/a.txt", "line":3, "col":2}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    for suffix in ["", "/members"] {
        let response = server
            .client
            .get(format!("{}/rooms/{room}{suffix}", server.url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.unwrap();
        let members = if suffix.is_empty() {
            &body["members"]
        } else {
            &body
        };
        assert_eq!(members[0]["user_id"], user);
        assert_eq!(members[0]["current_file"], "src/a.txt");
        assert_eq!(members[0]["line"], 3);
    }
    let response = server
        .client
        .delete(format!("{}/rooms/{room}/members/{user}", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let members: Value = server
        .client
        .get(format!("{}/rooms/{room}/members", server.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(members, json!([]));
}

#[tokio::test]
async fn nested_file_update_events_and_conflict_preserve_server_text() {
    let server = TestServer::start().await;
    let room = server.room().await;
    let alice = server.join(&room, "Alice").await;
    let bob = server.join(&room, "Bob").await;
    // Match the plugin's current slash encoding as well as the nested route.
    let file_url = format!("{}/rooms/{room}/files/src%2Fnested%2Fhello.txt", server.url);
    let response = server
        .client
        .post(&file_url)
        .json(&json!({"user_id":alice,"version":0,"text":"first\n"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let update: Value = response.json().await.unwrap();
    assert_eq!(update["version"], 1);
    let events: Value = server
        .client
        .get(format!("{}/rooms/{room}/events?cursor=0", server.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(events["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["event"]["type"] == "file_updated"
            && e["event"]["path"] == "src/nested/hello.txt"));
    assert!(events["next_cursor"].as_u64().unwrap() > update["event_id"].as_u64().unwrap());
    let response = server
        .client
        .post(&file_url)
        .json(&json!({"user_id":bob,"version":0,"text":"stale\n"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let conflict: Value = response.json().await.unwrap();
    assert_eq!(conflict["server_version"], 1);
    assert_eq!(conflict["server_text"], "first\n");
    let file: Value = server
        .client
        .get(format!(
            "{}/rooms/{room}/files/src/nested/hello.txt",
            server.url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(file["text"], "first\n");
    assert_eq!(file["version"], 1);
    let response = server
        .client
        .post(&file_url)
        .json(&json!({"user_id":bob,"version":1,"text":"second\n"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.json::<Value>().await.unwrap()["version"], 2);
}

#[tokio::test]
async fn empty_poll_returns_cursor_and_missing_room_is_not_found() {
    let server = TestServer::start().await;
    let room = server.room().await;
    let response = server
        .client
        .get(format!("{}/rooms/{room}/events?cursor=0", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body, json!({"events":[], "next_cursor":0}));
    let response = server
        .client
        .get(format!("{}/rooms/missing/files/a.txt", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
