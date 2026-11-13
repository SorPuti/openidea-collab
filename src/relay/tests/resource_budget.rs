//! Opt-in Linux process benchmark with current room limits; not an IDE or internet latency benchmark.
use futures_util::{SinkExt, StreamExt};
use openidea_collab_protocol::{
    CreateInvite, Endpoint, Frame, Invite, Kind, Role, SUBPROTOCOL, SessionCredential,
};
use reqwest::Client;
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use uuid::Uuid;
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn rss(pid: u32) -> usize {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap()
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap()
}
#[tokio::test]
#[ignore = "opt-in process/RSS benchmark on Linux"]
async fn twelve_connections_with_bounded_relay_memory() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let reserve = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserve.local_addr().unwrap();
    drop(reserve);
    let origin = format!("http://{address}");
    let mut process = Process(
        Command::new(env!("CARGO_BIN_EXE_collab-server"))
            .env("COLLAB_BIND", address.to_string())
            .env("COLLAB_PUBLIC_ORIGIN", &origin)
            .env(
                "COLLAB_STATE_FILE",
                directory.path().join("private/metadata.json"),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let http = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let mut ready = false;
    for _ in 0..100 {
        if http.get(format!("{origin}/healthz")).send().await.is_ok() {
            ready = true;
            break;
        }
        assert!(
            process.0.try_wait().unwrap().is_none(),
            "relay exited before startup"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(ready);
    let mut rooms = Vec::new();
    for _ in 0..4 {
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
        let doc = Uuid::new_v4();
        http.post(format!(
            "{origin}/v1/sessions/{}/documents",
            owner.session_id
        ))
        .bearer_auth(&owner.credential)
        .json(&serde_json::json!({"document_id":doc}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
        let mut members = vec![owner];
        for _ in 0..2 {
            let invitation: Invite = http
                .post(format!(
                    "{origin}/v1/sessions/{}/invitations",
                    members[0].session_id
                ))
                .bearer_auth(&members[0].credential)
                .json(&CreateInvite {
                    role: Role::Editor,
                    ttl_seconds: 300,
                })
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            let redemption = Endpoint::parse(&origin)
                .unwrap()
                .parse_invitation(&invitation.url)
                .unwrap();
            members.push(
                http.post(format!("{origin}/v1/invitations/redeem"))
                    .json(&redemption)
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap()
                    .json::<SessionCredential>()
                    .await
                    .unwrap(),
            );
        }
        let mut sockets = Vec::new();
        for member in members {
            let mut request = format!("ws://{address}/v1/socket")
                .into_client_request()
                .unwrap();
            request.headers_mut().insert(
                "authorization",
                format!("Bearer {}", member.credential).parse().unwrap(),
            );
            request
                .headers_mut()
                .insert("sec-websocket-protocol", SUBPROTOCOL.parse().unwrap());
            sockets.push(connect_async(request).await.unwrap().0);
        }
        rooms.push((doc, sockets));
    }
    let mut peak_rss = rss(process.0.id());
    let mut samples = Vec::new();
    let payload = vec![42; 1024];
    for _ in 0..100 {
        for (doc, sockets) in &mut rooms {
            let frame = Frame {
                kind: Kind::Update,
                document: *doc,
                target: Uuid::nil(),
                message: Uuid::new_v4(),
                part: 0,
                parts: 1,
                total: 1024,
                payload: &payload,
            }
            .encode()
            .unwrap();
            let start = Instant::now();
            sockets[0]
                .send(Message::Binary(frame.into()))
                .await
                .unwrap();
            for socket in &mut sockets[1..] {
                tokio::time::timeout(Duration::from_secs(3), async {
                    loop {
                        match socket.next().await.unwrap().unwrap() {
                            Message::Binary(data) => {
                                assert_eq!(Frame::decode(&data[16..]).unwrap().payload, payload);
                                break;
                            }
                            Message::Ping(data) => {
                                socket.send(Message::Pong(data)).await.unwrap();
                            }
                            _ => {}
                        }
                    }
                })
                .await
                .unwrap();
            }
            samples.push(start.elapsed().as_micros());
        }
        peak_rss = peak_rss.max(rss(process.0.id()));
    }
    samples.sort_unstable();
    println!(
        "relay_process connections=12 rooms=4 messages=400 deliveries=800 payload_bytes=1024 sampled_peak_rss_kib={peak_rss} p50_fanout_us={} p95_fanout_us={} max_fanout_us={}",
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples[samples.len() - 1]
    );
    assert!(
        peak_rss < 64 * 1024,
        "relay exceeded the proposed 64 MiB process budget"
    );
}
