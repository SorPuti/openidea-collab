// Run with: cargo test -p collab-server -- --ignored
#[tokio::test]
#[ignore = "requires running server on :4444"]
async fn create_and_join_room() {
    let client = reqwest::Client::new();

    let resp = client
        .post("http://localhost:4444/rooms")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let room_id = body["room_id"].as_str().unwrap();
    assert_eq!(room_id.len(), 8);

    let resp = client
        .post(format!("http://localhost:4444/rooms/{room_id}/join"))
        .json(&serde_json::json!({"name": "TestUser"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["user_id"].as_str().is_some());
}
