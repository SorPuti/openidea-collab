use collab_server::routes::router;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[tokio::test]
async fn health_identifies_the_collab_server() {
    let state = Arc::new(Mutex::new(HashMap::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
    let response = reqwest::get(format!("http://{addr}/health")).await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let value: serde_json::Value = response.json().await.unwrap();
    assert_eq!(value["service"], "openidea-collab");
    assert_eq!(value["status"], "ok");
    server.abort();
}
