use collab_server::Relay;
use futures_util::{SinkExt, StreamExt};
use openidea_collab_protocol::{
    CreateInvite, Endpoint, Frame, Invite, Kind, Role, SUBPROTOCOL, SessionCredential,
};
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::{path::PathBuf, time::Duration};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use uuid::Uuid;
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Server {
    origin: String,
    http: Client,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn start(state: Option<PathBuf>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let relay = Relay::new(&origin, state).unwrap();
        let task =
            tokio::spawn(async move { axum::serve(listener, relay.router()).await.unwrap() });
        let http = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        Self { origin, http, task }
    }
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }
    async fn create(&self) -> SessionCredential {
        let response = self
            .http
            .post(self.url("/v1/sessions"))
            .json(&json!({"protocol":1}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        response.json().await.unwrap()
    }
    async fn invite(&self, owner: &SessionCredential, role: Role) -> Invite {
        let response = self
            .http
            .post(self.url(&format!("/v1/sessions/{}/invitations", owner.session_id)))
            .bearer_auth(&owner.credential)
            .json(&CreateInvite {
                role,
                ttl_seconds: 300,
            })
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.json().await.unwrap()
    }
    async fn join(&self, owner: &SessionCredential, role: Role) -> SessionCredential {
        let invitation = self.invite(owner, role).await;
        let redeem = Endpoint::parse(&self.origin)
            .unwrap()
            .parse_invitation(&invitation.url)
            .unwrap();
        let response = self
            .http
            .post(self.url("/v1/invitations/redeem"))
            .json(&redeem)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.json().await.unwrap()
    }
    async fn document(&self, owner: &SessionCredential) -> Uuid {
        let id = Uuid::new_v4();
        let response = self
            .http
            .post(self.url(&format!("/v1/sessions/{}/documents", owner.session_id)))
            .bearer_auth(&owner.credential)
            .json(&json!({"document_id":id}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        id
    }
    async fn socket(&self, member: &SessionCredential) -> Socket {
        let mut request = format!("{}/v1/socket", self.origin.replace("http:", "ws:"))
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {}", member.credential).parse().unwrap(),
        );
        request
            .headers_mut()
            .insert("sec-websocket-protocol", SUBPROTOCOL.parse().unwrap());
        connect_async(request).await.unwrap().0
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn frame(kind: Kind, doc: Uuid, target: Uuid, payload: &[u8]) -> Vec<u8> {
    Frame {
        kind,
        document: doc,
        target,
        message: Uuid::new_v4(),
        part: 0,
        parts: 1,
        total: payload.len() as u32,
        payload,
    }
    .encode()
    .unwrap()
}
async fn binary(socket: &mut Socket) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Message::Binary(bytes) => return bytes.to_vec(),
                Message::Ping(payload) => {
                    socket.send(Message::Pong(payload)).await.unwrap();
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap()
}
async fn closed(socket: &mut Socket) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Ping(payload))) => {
                    let _ = socket.send(Message::Pong(payload)).await;
                }
                _ => {}
            }
        }
    })
    .await
    .expect("socket must close promptly");
}

#[tokio::test]
async fn health_and_protocol_do_not_expose_content_or_legacy_file_routes() {
    let server = Server::start(None).await;
    assert_eq!(
        server
            .http
            .get(server.url("/healthz"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let info: serde_json::Value = server
        .http
        .get(server.url("/v1/info"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info["protocol"], 1);
    assert_eq!(info["content_retention"], "transient_forwarding_only");
    for path in ["/rooms", "/rooms/old/files/.git/config"] {
        assert_eq!(
            server
                .http
                .post(server.url(path))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        server
            .http
            .post(server.url("/v1/sessions"))
            .json(&json!({"protocol":99}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}
#[tokio::test]
async fn invitations_are_individual_single_use_and_roles_are_enforced() {
    let server = Server::start(None).await;
    let owner = server.create().await;
    let invitation = server.invite(&owner, Role::Viewer).await;
    let payload = Endpoint::parse(&server.origin)
        .unwrap()
        .parse_invitation(&invitation.url)
        .unwrap();
    let request = || {
        server
            .http
            .post(server.url("/v1/invitations/redeem"))
            .json(&payload)
            .send()
    };
    let (a, b) = tokio::join!(request(), request());
    let statuses = [a.unwrap().status(), b.unwrap().status()];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::UNAUTHORIZED));
    let member = server.join(&owner, Role::Editor).await;
    assert_ne!(member.member_id, owner.member_id);
    assert_ne!(member.credential, owner.credential);
    let response = server
        .http
        .post(server.url(&format!("/v1/sessions/{}/invitations", owner.session_id)))
        .bearer_auth(&member.credential)
        .json(&json!({"role":"editor","ttl_seconds":300}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = server
        .http
        .post(server.url(&format!("/v1/sessions/{}/documents", owner.session_id)))
        .json(&json!({"document_id":Uuid::new_v4()}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn relay_forwards_bytes_with_authenticated_sender_and_revokes_open_socket() {
    let server = Server::start(None).await;
    let owner = server.create().await;
    let member = server.join(&owner, Role::Editor).await;
    let doc = server.document(&owner).await;
    let mut alice = server.socket(&owner).await;
    let mut bob = server.socket(&member).await;
    // Allow the upgrade task to register its bounded receiver before sending.
    alice
        .send(Message::Binary(
            frame(Kind::Update, doc, member.member_id, b"opaque update").into(),
        ))
        .await
        .unwrap();
    let received = binary(&mut bob).await;
    assert_eq!(&received[..16], owner.member_id.as_bytes());
    assert_eq!(
        Frame::decode(&received[16..]).unwrap().payload,
        b"opaque update"
    );
    let response = server
        .http
        .delete(server.url(&format!(
            "/v1/sessions/{}/members/{}",
            owner.session_id, member.member_id
        )))
        .bearer_auth(&owner.credential)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    closed(&mut bob).await;
    let response = server
        .http
        .post(server.url(&format!("/v1/sessions/{}/invitations", owner.session_id)))
        .bearer_auth(&member.credential)
        .json(&json!({"role":"editor","ttl_seconds":300}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn viewers_cannot_write_and_members_cannot_target_other_rooms() {
    let server = Server::start(None).await;
    let owner = server.create().await;
    let viewer = server.join(&owner, Role::Viewer).await;
    let doc = server.document(&owner).await;
    let mut socket = server.socket(&viewer).await;
    socket
        .send(Message::Binary(
            frame(Kind::Update, doc, Uuid::nil(), b"forged update").into(),
        ))
        .await
        .unwrap();
    closed(&mut socket).await;
    let other = server.create().await;
    let mut socket = server.socket(&owner).await;
    socket
        .send(Message::Binary(
            frame(Kind::StateVector, doc, other.member_id, b"x").into(),
        ))
        .await
        .unwrap();
    closed(&mut socket).await;
}
#[tokio::test]
async fn unknown_documents_and_oversized_messages_are_refused() {
    let server = Server::start(None).await;
    let owner = server.create().await;
    let mut socket = server.socket(&owner).await;
    socket
        .send(Message::Binary(
            frame(Kind::Update, Uuid::new_v4(), Uuid::nil(), b"x").into(),
        ))
        .await
        .unwrap();
    closed(&mut socket).await;
    let mut socket = server.socket(&owner).await;
    socket
        .send(Message::Binary(vec![0; 65537].into()))
        .await
        .unwrap();
    closed(&mut socket).await;
}
#[tokio::test]
async fn authorization_survives_restart_without_storing_credentials_or_document_contents() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("private/metadata.json");
    let server = Server::start(Some(file.clone())).await;
    let owner = server.create().await;
    let member = server.join(&owner, Role::Editor).await;
    let doc = server.document(&owner).await;
    let mut a = server.socket(&owner).await;
    let mut b = server.socket(&member).await;
    a.send(Message::Binary(
        frame(
            Kind::Update,
            doc,
            member.member_id,
            b"PRIVATE-DOCUMENT-CONTENT",
        )
        .into(),
    ))
    .await
    .unwrap();
    binary(&mut b).await;
    drop(a);
    drop(b);
    drop(server);
    let metadata = std::fs::read_to_string(&file).unwrap();
    assert!(!metadata.contains(&owner.credential));
    assert!(!metadata.contains(&member.credential));
    assert!(!metadata.contains("PRIVATE-DOCUMENT-CONTENT"));
    let server = Server::start(Some(file)).await;
    let mut a = server.socket(&owner).await;
    let mut b = server.socket(&member).await;
    a.send(Message::Binary(
        frame(Kind::Update, doc, member.member_id, b"after restart").into(),
    ))
    .await
    .unwrap();
    assert_eq!(
        Frame::decode(&binary(&mut b).await[16..]).unwrap().payload,
        b"after restart"
    );
}
#[tokio::test]
async fn quota_is_bounded_and_invalid_origins_are_rejected() {
    assert!(Relay::new("http://public.example", None).is_err());
    let server = Server::start(None).await;
    for _ in 0..4 {
        server.create().await;
    }
    let response = server
        .http
        .post(server.url("/v1/sessions"))
        .json(&json!({"protocol":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn websocket_handshake_requires_header_auth_and_rejects_browser_origins_and_query_tokens() {
    let server = Server::start(None).await;
    let owner = server.create().await;
    for case in 0..4 {
        let mut request = format!(
            "{}/v1/socket{}",
            server.origin.replace("http:", "ws:"),
            if case == 2 { "?token=forbidden" } else { "" }
        )
        .into_client_request()
        .unwrap();
        if case != 0 {
            request.headers_mut().insert(
                "authorization",
                format!("Bearer {}", owner.credential).parse().unwrap(),
            );
        }
        if case != 3 {
            request
                .headers_mut()
                .insert("sec-websocket-protocol", SUBPROTOCOL.parse().unwrap());
        }
        if case == 1 {
            request
                .headers_mut()
                .insert("origin", "https://evil.example".parse().unwrap());
        }
        assert!(connect_async(request).await.is_err());
    }
}

#[tokio::test]
async fn invitation_page_has_no_network_trackers_or_secret_in_server_response() {
    let server = Server::start(None).await;
    let owner = server.create().await;
    let invitation = server.invite(&owner, Role::Viewer).await;
    let url = url_without_fragment(&invitation.url);
    let response = server.http.get(url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    let html = response.text().await.unwrap();
    assert!(!html.contains(&invitation.url));
    assert!(!html.contains("https://"));
}
fn url_without_fragment(url: &str) -> &str {
    url.split('#').next().unwrap()
}

#[tokio::test]
async fn two_crdt_clients_converge_through_the_real_relay_without_implicit_durability_ack() {
    use openidea_collab_core::{document::Document, transfer::split};
    let server = Server::start(None).await;
    let owner = server.create().await;
    let member = server.join(&owner, Role::Editor).await;
    let doc_id = server.document(&owner).await;
    let mut alice = server.socket(&owner).await;
    let mut bob = server.socket(&member).await;
    let mut a = Document::default();
    let mut b = Document::default();
    a.edit(0, 0, "A😀B").unwrap();
    let initial = a.pending().back().unwrap();
    for part in split(
        Kind::Snapshot,
        doc_id,
        member.member_id,
        initial.id,
        &initial.update,
    )
    .unwrap()
    {
        alice.send(Message::Binary(part.into())).await.unwrap();
    }
    let received = binary(&mut bob).await;
    b.receive(Frame::decode(&received[16..]).unwrap().payload)
        .unwrap();
    a.edit(1, 1, "α").unwrap();
    b.edit(4, 4, "β").unwrap();
    let au = a.pending().back().unwrap();
    let bu = b.pending().back().unwrap();
    for part in split(Kind::Update, doc_id, member.member_id, au.id, &au.update).unwrap() {
        alice.send(Message::Binary(part.into())).await.unwrap();
    }
    for part in split(Kind::Update, doc_id, owner.member_id, bu.id, &bu.update).unwrap() {
        bob.send(Message::Binary(part.into())).await.unwrap();
    }
    let ar = binary(&mut alice).await;
    let br = binary(&mut bob).await;
    a.receive(Frame::decode(&ar[16..]).unwrap().payload)
        .unwrap();
    b.receive(Frame::decode(&br[16..]).unwrap().payload)
        .unwrap();
    assert_eq!(a.text(), "Aα😀Bβ");
    assert_eq!(a.text(), b.text());
    // Receipt alone never removes pending work; no ACK is synthesized by the relay.
    assert_eq!(a.pending().len(), 2);
    assert_eq!(b.pending().len(), 1);
    let checkpoint = b.checkpoint([9; 32]);
    let recovered = Document::restore(&checkpoint, [9; 32]).unwrap();
    assert_eq!(recovered.text(), a.text());
    assert_eq!(recovered.pending().len(), 1);
}

#[tokio::test]
async fn shutdown_closes_idle_authenticated_sockets() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let relay = Relay::new(&origin, None).unwrap();
    let app = relay.router();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let server = Server {
        origin,
        http: Client::builder().no_proxy().build().unwrap(),
        task,
    };
    let owner = server.create().await;
    let mut socket = server.socket(&owner).await;
    relay.shutdown();
    closed(&mut socket).await;
}

#[tokio::test]
async fn client_supplied_identity_fields_are_refused() {
    let server = Server::start(None).await;
    let owner = server.create().await;
    let endpoint = server.url(&format!("/v1/sessions/{}/invitations", owner.session_id));
    let response = server
        .http
        .post(endpoint)
        .bearer_auth(&owner.credential)
        .json(&json!({"role":"editor","ttl_seconds":300,"user_id":owner.member_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
