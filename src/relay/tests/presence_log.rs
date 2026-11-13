use openidea_collab_protocol::{SUBPROTOCOL, SessionCredential};
use std::{
    fs,
    net::TcpListener,
    process::{Command, Stdio},
    time::Duration,
};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

#[tokio::test]
async fn presence_events_and_snapshot_exclude_secrets_and_document_content() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("stderr");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let origin = format!("http://{addr}");
    let mut child = Command::new(env!("CARGO_BIN_EXE_collab-server"))
        .env("COLLAB_BIND", addr.to_string())
        .env("COLLAB_PUBLIC_ORIGIN", &origin)
        .env("COLLAB_STATE_FILE", dir.path().join("state/metadata.json"))
        .stderr(Stdio::from(fs::File::create(&log_path).unwrap()))
        .spawn()
        .unwrap();
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    for _ in 0..100 {
        if http.get(format!("{origin}/healthz")).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let owner: SessionCredential = http
        .post(format!("{origin}/v1/sessions"))
        .json(&serde_json::json!({"protocol":1}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut request = format!("ws://{addr}/v1/socket")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", owner.credential).parse().unwrap(),
    );
    request
        .headers_mut()
        .insert("sec-websocket-protocol", SUBPROTOCOL.parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "private-document-content".into(),
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = socket.close(None).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    child.kill().unwrap();
    child.wait().unwrap();
    let log = fs::read_to_string(log_path).unwrap();
    let entries: Vec<serde_json::Value> = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let event = |name: &str| {
        entries.iter().any(|v| {
            v["event"] == name
                && v["session_id"] == owner.session_id.to_string()
                && v["member_id"] == owner.member_id.to_string()
        })
    };
    assert!(event("connected"), "missing connect event");
    assert!(event("disconnected"), "missing disconnect event");
    assert!(
        entries.iter().any(|entry| entry["event"] == "snapshot"
            && entry["sessions"]
                .as_array()
                .is_some_and(|sessions| sessions.iter().any(|s| s["session_id"]
                    == owner.session_id.to_string()
                    && s["connected"].as_array().is_some_and(|members| members
                        .iter()
                        .any(|m| m["member_id"] == owner.member_id.to_string()
                            && m["connected_at"].is_number()))))),
        "missing connected participant in snapshot"
    );
    assert!(
        entries.iter().any(|v| v["event"] == "snapshot"
            && v["sessions"].as_array().is_some_and(|s| s
                .iter()
                .any(|s| s["session_id"] == owner.session_id.to_string()))),
        "missing active session snapshot"
    );
    assert!(!log.contains(&owner.credential));
    assert!(!log.contains("private-document-content"));
}
use futures_util::SinkExt;
