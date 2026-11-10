use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn disconnect_waits_for_pending_changes_and_session_can_recover() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let reject = Arc::new(AtomicBool::new(true));
    let gate = reject.clone();
    let router = collab_server::realtime::router().layer(axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let gate = gate.clone();
            async move {
                if request.method() == axum::http::Method::POST
                    && request.uri().path().contains("/documents/")
                    && gate.load(Ordering::SeqCst)
                {
                    return axum::response::IntoResponse::into_response(
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    );
                }
                next.run(request).await
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_openidea-collab-helper"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    output.next_line().await.unwrap().unwrap();
    let attach = serde_json::json!({"type":"attach","id":"attach","endpoint":base,"room":room["room_id"],"document":"main","token":room["owner_token"],"text":"a"});
    input
        .write_all(format!("{attach}\n").as_bytes())
        .await
        .unwrap();
    let attached: serde_json::Value =
        serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(attached["type"], "attached");
    let edit = serde_json::json!({"type":"edit","id":"edit","from":1,"to":1,"insert":"b"});
    input
        .write_all(format!("{edit}\n").as_bytes())
        .await
        .unwrap();
    let edited: serde_json::Value =
        serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(edited["type"], "edited");
    input
        .write_all(b"{\"type\":\"disconnect\",\"id\":\"leave\"}\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let line = output.next_line().await.unwrap().unwrap();
            let event: serde_json::Value = serde_json::from_str(&line).unwrap();
            if event["type"] == "disconnect_blocked" {
                assert_eq!(event["pending"], 1);
                break;
            }
            assert_ne!(event["type"], "disconnected");
        }
    })
    .await
    .unwrap();
    reject.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let line = output.next_line().await.unwrap().unwrap();
            let event: serde_json::Value = serde_json::from_str(&line).unwrap();
            if event["type"] == "synced" {
                assert_eq!(event["text"], "ab");
                break;
            }
        }
    })
    .await
    .unwrap();
    input
        .write_all(b"{\"type\":\"disconnect\",\"id\":\"leave2\"}\n")
        .await
        .unwrap();
    let disconnected: serde_json::Value =
        serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(disconnected["type"], "disconnected");
    child.kill().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn helper_process_attaches_edits_and_syncs_without_echoing_credentials() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, collab_server::realtime::router())
            .await
            .unwrap()
    });
    let base = format!("http://{addr}");
    let created: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = created["owner_token"].as_str().unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_openidea-collab-helper"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready = tokio::time::timeout(Duration::from_secs(3), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&ready).unwrap()["type"],
        "ready"
    );
    let connect = serde_json::json!({"type":"attach","id":"1","endpoint":base,"room":created["room_id"],"document":"main","token":token,"text":"A😀B"});
    stdin
        .write_all(format!("{connect}\n").as_bytes())
        .await
        .unwrap();
    let attached = tokio::time::timeout(Duration::from_secs(3), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!attached.contains(token));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&attached).unwrap()["type"],
        "attached"
    );
    let edit = serde_json::json!({"type":"edit","id":"2","from":1,"to":1,"insert":"X"});
    stdin
        .write_all(format!("{edit}\n").as_bytes())
        .await
        .unwrap();
    let edited = tokio::time::timeout(Duration::from_secs(3), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let message: serde_json::Value = serde_json::from_str(&edited).unwrap();
    assert_eq!(message["type"], "edited");
    assert_eq!(message["text"], "AX😀B");
    let synced = tokio::time::timeout(Duration::from_secs(3), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&synced).unwrap()["type"],
        "synced"
    );
    drop(stdin);
    let _ = tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .unwrap()
        .unwrap();
    server.abort();
}

#[tokio::test]
async fn two_helper_processes_receive_and_merge_live_edits() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, collab_server::realtime::router())
            .await
            .unwrap()
    });
    let base = format!("http://{addr}");
    let created: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut children = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_openidea-collab-helper"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output.next_line().await.unwrap().unwrap())
                .unwrap()["type"],
            "ready"
        );
        let attach = serde_json::json!({"type":"attach","id":"1","endpoint":base,"room":created["room_id"],"document":"main","token":created["owner_token"],"text":"ab"});
        input
            .write_all(format!("{attach}\n").as_bytes())
            .await
            .unwrap();
        let attached = tokio::time::timeout(Duration::from_secs(3), output.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&attached).unwrap()["type"],
            "attached"
        );
        children.push(child);
        inputs.push(input);
        outputs.push(output);
    }
    let edit_a = serde_json::json!({"type":"edit","id":"a","from":1,"to":1,"insert":"X"});
    let edit_b = serde_json::json!({"type":"edit","id":"b","from":1,"to":1,"insert":"Y"});
    inputs[0]
        .write_all(format!("{edit_a}\n").as_bytes())
        .await
        .unwrap();
    inputs[1]
        .write_all(format!("{edit_b}\n").as_bytes())
        .await
        .unwrap();
    for mut output in outputs {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let line = output.next_line().await.unwrap().expect("helper exited");
                let value: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_ne!(
                    value["type"], "connection_error",
                    "unexpected helper failure"
                );
                if value["pending"] == 0
                    && value["text"]
                        .as_str()
                        .is_some_and(|s| s.contains('X') && s.contains('Y'))
                {
                    break;
                }
            }
        })
        .await
        .expect("helpers did not converge");
    }
    for mut child in children {
        child.kill().await.unwrap();
    }
    server.abort();
}

#[tokio::test]
async fn helpers_create_and_join_with_one_use_invite() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, collab_server::realtime::router())
            .await
            .unwrap()
    });
    let mut processes = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_openidea-collab-helper"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output.next_line().await.unwrap().unwrap())
                .unwrap()["type"],
            "ready"
        );
        processes.push(child);
        inputs.push(input);
        outputs.push(output);
    }
    let endpoint = format!("http://{addr}");
    let create = serde_json::json!({"type":"create","id":"c","endpoint":endpoint,"document":"main","text":"ab"});
    inputs[0]
        .write_all(format!("{create}\n").as_bytes())
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(3), outputs[0].next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(created["type"], "created");
    assert!(created.get("token").is_none());
    let invite = serde_json::json!({"type":"invite","id":"i","role":"editor","ttl_seconds":60});
    inputs[0]
        .write_all(format!("{invite}\n").as_bytes())
        .await
        .unwrap();
    let issued: serde_json::Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(3), outputs[0].next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(issued["type"], "invite");
    let join = serde_json::json!({"type":"join","id":"j","endpoint":endpoint,"room":created["room_id"],"document":"main","invite":issued["token"],"text":"ab"});
    inputs[1]
        .write_all(format!("{join}\n").as_bytes())
        .await
        .unwrap();
    let joined: serde_json::Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(3), outputs[1].next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(joined["type"], "joined");
    assert!(joined.get("token").is_none());
    for mut child in processes {
        child.kill().await.unwrap();
    }
    server.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn opted_in_recovery_copy_survives_helper_termination_without_credentials() {
    let root =
        std::env::temp_dir().join(format!("collab-helper-recovery-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, collab_server::realtime::router())
            .await
            .unwrap()
    });
    let base = format!("http://{addr}");
    let created: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/rooms"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = created["owner_token"].as_str().unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_openidea-collab-helper"))
        .env("XDG_STATE_HOME", &root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    output.next_line().await.unwrap().unwrap();
    let attach = serde_json::json!({"type":"attach","id":"1","endpoint":base,"room":created["room_id"],"document":"main","token":token,"text":"a","recovery":true});
    input
        .write_all(format!("{attach}\n").as_bytes())
        .await
        .unwrap();
    let attached: serde_json::Value =
        serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(attached["type"], "attached");
    let path = std::path::PathBuf::from(attached["recovery_path"].as_str().unwrap());
    assert!(path.starts_with(&root));
    let edit = serde_json::json!({"type":"edit","id":"2","from":1,"to":1,"insert":" offline"});
    input
        .write_all(format!("{edit}\n").as_bytes())
        .await
        .unwrap();
    let edited: serde_json::Value =
        serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
    assert_eq!(edited["type"], "edited");
    child.kill().await.unwrap();
    let content = std::fs::read_to_string(path).unwrap();
    assert_eq!(content, "a offline");
    assert!(!content.contains(token));
    server.abort();
    std::fs::remove_dir_all(root).unwrap();
}
