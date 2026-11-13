use super::*;

pub(super) async fn upgrade(
    State(relay): State<Relay>,
    headers: HeaderMap,
    uri: Uri,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    if uri.query().is_some() || headers.contains_key("origin") {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "native_authenticated_client_required",
        ));
    }
    let protocols = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !protocols.split(',').any(|v| v.trim() == SUBPROTOCOL) {
        return Err(error(StatusCode::BAD_REQUEST, "subprotocol_required"));
    }
    if relay.inner.failed.load(Ordering::Acquire) {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE, "state_unavailable"));
    }
    let digest = bearer(&headers)?;
    let principal = relay
        .inner
        .registry
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "state_unavailable"))?
        .metadata
        .authenticate(&digest)?;
    let permit = relay
        .inner
        .permits
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::TOO_MANY_REQUESTS, "connection_limit"))?;
    Ok(ws
        .protocols([SUBPROTOCOL])
        .max_message_size(MAX_FRAME)
        .max_frame_size(MAX_FRAME)
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_FRAME * 2)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            run_socket(relay, principal, socket).await;
        }))
}
async fn run_socket(relay: Relay, principal: Principal, socket: WebSocket) {
    let connection = Uuid::new_v4();
    let (sender, mut receive) = mpsc::channel::<Delivery>(QUEUE_FRAMES);
    let (kick, mut kicked) = watch::channel(false);
    {
        let Ok(mut registry) = relay.inner.registry.lock() else {
            return;
        };
        if relay.inner.failed.load(Ordering::Acquire)
            || !registry.metadata.authorize(principal, None, false)
        {
            return;
        }
        if let Some(previous) = registry.live.insert(
            principal.member,
            Live {
                connection,
                connected_at: now(),
                session: principal.session,
                sender,
                kick,
            },
        ) {
            let _ = previous.kick.send(true);
        }
        observability::connection_event("connected", principal, connection);
        observability::snapshot(&registry);
    }
    let (mut output, mut input) = socket.split();
    let mut input_kick = kicked.clone();
    let reader = async {
        let mut frames = Bucket::new(120.0, 60.0);
        let mut bytes = Bucket::new(512.0 * 1024.0, 256.0 * 1024.0);
        let mut presence = Bucket::new(5.0, 5.0);
        loop {
            let message = tokio::select! { _ = input_kick.changed() => break, message = input.next() => message };
            let Some(Ok(message)) = message else { break };
            match message {
                Message::Binary(data) => {
                    if !frames.take(1.0) || !bytes.take(data.len() as f64) {
                        break;
                    }
                    let Ok(frame) = Frame::decode(&data) else {
                        break;
                    };
                    if frame.kind == Kind::Presence && !presence.take(1.0) {
                        continue;
                    }
                    if !forward(&relay, principal, &frame, &data) {
                        break;
                    }
                }
                Message::Ping(_) | Message::Pong(_) => {
                    if !frames.take(1.0) {
                        break;
                    }
                }
                Message::Close(_) | Message::Text(_) => break,
            }
        }
    };
    let writer = async {
        let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                _ = kicked.changed() => break,
                _ = heartbeat.tick() => {
                    let active = relay.inner.registry.lock().is_ok_and(|r| r.metadata.authorize(principal, None, false));
                    if !active { break; }
                    if !send_bounded(&mut output, &mut kicked, Message::Ping(Bytes::new())).await { break; }
                }
                delivery = receive.recv() => {
                    let Some(delivery) = delivery else { break };
                    let allowed = relay.inner.registry.lock().is_ok_and(|r| {
                        r.metadata.authorize(principal, Some(delivery.document), false)
                            && r.metadata.sessions.get(&principal.session).is_some_and(|s| s.members.contains_key(&delivery.author))
                    });
                    if allowed && !send_bounded(&mut output, &mut kicked, Message::Binary(delivery.bytes)).await { break; }
                }
            }
        }
    };
    tokio::pin!(reader, writer);
    tokio::select! { _ = &mut reader => {}, _ = &mut writer => {} }
    if let Ok(mut registry) = relay.inner.registry.lock() {
        if registry
            .live
            .get(&principal.member)
            .is_some_and(|live| live.connection == connection)
        {
            registry.live.remove(&principal.member);
        }
        observability::connection_event("disconnected", principal, connection);
        observability::snapshot(&registry);
    }
}
async fn send_bounded<S>(
    output: &mut S,
    kicked: &mut watch::Receiver<bool>,
    message: Message,
) -> bool
where
    S: futures_util::Sink<Message> + Unpin,
{
    tokio::select! { _ = kicked.changed() => false, result = tokio::time::timeout(Duration::from_secs(5), output.send(message)) => matches!(result, Ok(Ok(()))) }
}
pub(super) fn forward(
    relay: &Relay,
    principal: Principal,
    frame: &Frame<'_>,
    bytes: &Bytes,
) -> bool {
    if relay.inner.failed.load(Ordering::Acquire) {
        return false;
    }
    let Ok(registry) = relay.inner.registry.lock() else {
        return false;
    };
    if !registry
        .metadata
        .authorize(principal, Some(frame.document), frame.kind.writes())
        || frame.target == principal.member
    {
        return false;
    }
    let Some(session) = registry.metadata.sessions.get(&principal.session) else {
        return false;
    };
    if !frame.target.is_nil() && !session.members.contains_key(&frame.target) {
        return false;
    }
    let mut encoded = BytesMut::with_capacity(16 + bytes.len());
    encoded.extend_from_slice(principal.member.as_bytes());
    encoded.extend_from_slice(bytes);
    let encoded = encoded.freeze(); // One allocation shared by every recipient.
    for (member, live) in &registry.live {
        if *member == principal.member
            || live.session != principal.session
            || (!frame.target.is_nil() && *member != frame.target)
        {
            continue;
        }
        if !session.members.contains_key(member) {
            continue;
        }
        if live
            .sender
            .try_send(Delivery {
                author: principal.member,
                document: frame.document,
                bytes: encoded.clone(),
            })
            .is_err()
        {
            let _ = live.kick.send(true);
        }
    }
    true
}
