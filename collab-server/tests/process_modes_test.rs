use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn realtime_mode_starts_without_exposing_legacy_routes() {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let _process = Process(
        Command::new(env!("CARGO_BIN_EXE_collab-server"))
            .env("COLLAB_PORT", port.to_string())
            .env("COLLAB_API", "realtime")
            .env_remove("COLLAB_BIND")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{port}");
    for _ in 0..50 {
        if client.get(format!("{base}/health")).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let health = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(health.status(), 200);
    assert_eq!(
        client
            .post(format!("{base}/v1/rooms"))
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    assert_eq!(
        client
            .post(format!("{base}/rooms"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[test]
fn realtime_mode_refuses_plaintext_non_loopback_binding() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_collab-server"))
        .env("COLLAB_BIND", "0.0.0.0")
        .env("COLLAB_PORT", "0")
        .env("COLLAB_API", "realtime")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(!status.success());
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    panic!("realtime API must reject public plaintext bind");
}
