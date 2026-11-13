//! Uses a real loopback HTTP server and the official SDK mock. Not an IDE UI test.
use openidea_collab::{client::CollabClient, room::RoomState, sync::SyncState, CollabPlugin};
use openidea_plugin::{host, mock::MockHost, views::{ViewEvent, Views}, Error, Event, HttpResponse, Plugin};
use std::{collections::HashMap, sync::{Arc, Mutex}};

fn connect_mock(host: &MockHost) {
    host.on_fetch(|request| {
        let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|e| Error::Failed(e.to_string()))?;
        let mut call = reqwest::blocking::Client::new().request(method, &request.url);
        for (name, value) in &request.headers { call = call.header(name, value); }
        if let Some(body) = &request.body { call = call.body(body.clone()); }
        let response = call.send().map_err(|e| Error::Failed(e.to_string()))?;
        let status = response.status().as_u16();
        let body = response.bytes().map_err(|e| Error::Failed(e.to_string()))?.to_vec();
        Ok(HttpResponse { status, headers: vec![], body })
    });
}

#[test]
fn panel_and_two_mock_clients_exchange_a_saved_file_via_real_server() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let state = Arc::new(Mutex::new(HashMap::new()));
            axum_server(listener, state, shutdown_rx).await;
        });
    });
    let base = format!("http://localhost:{port}");
    let room_a: RoomState;
    {
        let h = MockHost::install().with_capabilities(&["events", "storage", "project.read", "project.write", "net:localhost"]);
        connect_mock(&h);
        host::storage::set("server_url", &base).unwrap();
        h.add_file("src/main.rs", "texto de teste");
        let mut plugin = CollabPlugin::default();
        plugin.activate(&h.context("sorputi.openidea-collab")).unwrap();
        plugin.handle("collab-panel", ViewEvent::Message(r#"{"type":"create-room","name":"Ana"}"#.into()), &h.invocation()).unwrap();
        room_a = serde_json::from_str(&host::storage::get("room_state").unwrap().unwrap()).unwrap();
        assert!(h.view_messages().iter().any(|(view, message)| view == "collab-panel" && message.contains(&room_a.room_id)));
        plugin.handle_events(&[Event::FileSaved("/project/src/main.rs".into())], &h.invocation());
    }
    {
        let h = MockHost::install().with_capabilities(&["events", "storage", "project.read", "project.write", "net:localhost"]);
        connect_mock(&h);
        let client = CollabClient::new(base.clone());
        let user_id = client.join_room(&room_a.room_id, "Bob").unwrap();
        let mut sync = SyncState::default();
        sync.room = Some(RoomState { room_id: room_a.room_id, user_id, name: "Bob".into(), server_url: base, github: None });
        let context = h.invocation();
        sync.on_poll_tick(&client, &context);
        assert_eq!(context.project().unwrap().read_file("src/main.rs").unwrap(), "texto de teste");
        assert!(sync.sync_active);
    }
    shutdown_tx.send(()).unwrap();
    server.join().unwrap();
}

async fn axum_server(listener: tokio::net::TcpListener, state: collab_server::routes::AppState, shutdown: tokio::sync::oneshot::Receiver<()>) {
    axum::serve(listener, collab_server::routes::router(state))
        .with_graceful_shutdown(async { let _ = shutdown.await; })
        .await.unwrap();
}
