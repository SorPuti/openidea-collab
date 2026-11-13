
#[cfg(test)]
mod proof_collab_panel {
    use super::*;
    use openidea_plugin::{mock::MockHost, views::{Views, ViewEvent}, HttpResponse};
    #[test]
    fn panel_create_room_invokes_server_and_updates_state() {
        let host = MockHost::install().with_capabilities(&["storage", "net:localhost"]);
        host.on_fetch(|req| {
            let body = if req.url.ends_with("/rooms") { r#"{"room_id":"room-test"}"# }
                       else if req.url.ends_with("/rooms/room-test/join") { r#"{"user_id":"user-test"}"# }
                       else { panic!("unexpected request {}", req.url) };
            Ok(HttpResponse {status: if req.url.ends_with("/rooms") {201} else {200}, headers:vec![],body:body.as_bytes().to_vec()})
        });
        let mut plugin = CollabPlugin::default();
        plugin.handle("collab-panel", ViewEvent::Message(r#"{"type":"create-room","name":"Pessoa teste"}"#.into()), &host.invocation()).unwrap();
        let req = host.requests();
        assert_eq!(req.len(), 2);
        assert!(req[0].url.ends_with("/rooms"));
        assert!(req[1].url.ends_with("/rooms/room-test/join"));
        assert_eq!(plugin.sync.room.as_ref().unwrap().room_id, "room-test");
        assert!(host.view_messages().iter().any(|(view, msg)| view == "collab-panel" && msg.contains("room-test")));
    }
    #[test]
    fn ping_atual_aceita_404_o_que_mascara_falha() {
        let host = MockHost::install().with_capabilities(&["net:localhost"]);
        host.on_fetch(|_| Ok(HttpResponse {status:404,headers:vec![],body:vec![]}));
        assert!(CollabClient::new("http://localhost:4444".into()).ping("http://localhost:4444").is_ok());
    }
}
