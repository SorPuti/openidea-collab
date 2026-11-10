use super::*;

fn invitation_response(content: &'static str, content_type: &'static str) -> Response {
    ([
        ("content-type", content_type),
        ("cache-control", "no-store"),
        ("referrer-policy", "no-referrer"),
        ("x-content-type-options", "nosniff"),
        ("content-security-policy", "default-src 'none'; script-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"),
    ], content).into_response()
}
pub(super) async fn invitation_page(RoutePath(_id): RoutePath<Uuid>) -> Response {
    invitation_response(
        include_str!("../assets/invitation.html"),
        "text/html; charset=utf-8",
    )
}
pub(super) async fn invitation_script() -> Response {
    invitation_response(
        include_str!("../assets/invitation.js"),
        "text/javascript; charset=utf-8",
    )
}
pub(super) async fn health(State(relay): State<Relay>) -> StatusCode {
    if relay.inner.failed.load(Ordering::Acquire) {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::NO_CONTENT
    }
}
pub(super) async fn info(State(relay): State<Relay>) -> Json<ServiceInfo> {
    Json(ServiceInfo {
        protocol: VERSION,
        websocket_url: relay.inner.endpoint.websocket(),
        max_frame_bytes: MAX_FRAME,
        max_transfer_bytes: MAX_TRANSFER,
        content_retention: "transient_forwarding_only".to_owned(),
    })
}
pub(super) async fn create_session(
    State(relay): State<Relay>,
    Json(body): Json<CreateSession>,
) -> ApiResult<(StatusCode, Json<SessionCredential>)> {
    if body.protocol != VERSION {
        return Err(error(StatusCode::CONFLICT, "unsupported_protocol"));
    }
    let credential = secret();
    let digest = hash(&credential);
    let session_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();
    let expires_at = now() + SESSION_TTL;
    relay
        .transaction(move |registry| {
            if !registry.creates.take(1.0) {
                return Err(error(StatusCode::TOO_MANY_REQUESTS, "creation_rate_limit"));
            }
            if registry.metadata.sessions.len() >= MAX_SESSIONS {
                return Err(error(StatusCode::TOO_MANY_REQUESTS, "session_limit"));
            }
            registry.metadata.sessions.insert(
                session_id,
                Session {
                    expires_at,
                    members: HashMap::from([(
                        member_id,
                        Member {
                            digest,
                            role: Role::Owner,
                        },
                    )]),
                    invitations: HashMap::new(),
                    documents: HashSet::new(),
                },
            );
            Ok(())
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(SessionCredential {
            session_id,
            member_id,
            credential,
            role: Role::Owner,
            expires_at,
            websocket_url: relay.inner.endpoint.websocket(),
        }),
    ))
}
pub(super) async fn create_invitation(
    State(relay): State<Relay>,
    RoutePath(session_id): RoutePath<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CreateInvite>,
) -> ApiResult<Json<Invite>> {
    let digest = bearer(&headers)?;
    if body.role == Role::Owner || !(30..=600).contains(&body.ttl_seconds) {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_invitation_policy"));
    }
    let id = Uuid::new_v4();
    let credential = secret();
    let invitation_digest = hash(&credential);
    let expires_at = now() + body.ttl_seconds;
    let expires_at = relay
        .transaction(move |registry| {
            let principal = registry.metadata.authenticate(&digest)?;
            if principal.session != session_id || principal.role != Role::Owner {
                return Err(error(StatusCode::FORBIDDEN, "owner_required"));
            }
            let session = registry.metadata.sessions.get_mut(&session_id).unwrap();
            if session.invitations.len() >= MAX_INVITES {
                return Err(error(StatusCode::TOO_MANY_REQUESTS, "invitation_limit"));
            }
            session.invitations.insert(
                id,
                Invitation {
                    digest: invitation_digest,
                    role: body.role,
                    expires_at: expires_at.min(session.expires_at),
                },
            );
            Ok(expires_at.min(session.expires_at))
        })
        .await?;
    Ok(Json(Invite {
        url: relay.inner.endpoint.invitation(id, &credential),
        expires_at,
    }))
}
pub(super) async fn redeem_invitation(
    State(relay): State<Relay>,
    Json(body): Json<RedeemInvite>,
) -> ApiResult<Json<SessionCredential>> {
    if !credential_valid(&body.secret) {
        return Err(error(StatusCode::UNAUTHORIZED, "invalid_invitation"));
    }
    let invitation_digest = hash(&body.secret);
    let credential = secret();
    let digest = hash(&credential);
    let member_id = Uuid::new_v4();
    let (session_id, role, expires_at) = relay
        .transaction(move |registry| {
            for (session_id, session) in &mut registry.metadata.sessions {
                let Some(invitation) = session.invitations.get(&body.invite_id) else {
                    continue;
                };
                if !bool::from(invitation.digest.ct_eq(&invitation_digest))
                    || invitation.expires_at <= now()
                {
                    break;
                }
                if session.members.len() >= MAX_MEMBERS {
                    return Err(error(StatusCode::TOO_MANY_REQUESTS, "member_limit"));
                }
                let role = invitation.role;
                session.members.insert(member_id, Member { digest, role });
                session.invitations.remove(&body.invite_id);
                return Ok((*session_id, role, session.expires_at));
            }
            Err(error(
                StatusCode::UNAUTHORIZED,
                "invalid_or_used_invitation",
            ))
        })
        .await?;
    Ok(Json(SessionCredential {
        session_id,
        member_id,
        credential,
        role,
        expires_at,
        websocket_url: relay.inner.endpoint.websocket(),
    }))
}
pub(super) async fn register_document(
    State(relay): State<Relay>,
    RoutePath(session_id): RoutePath<Uuid>,
    headers: HeaderMap,
    Json(body): Json<RegisterDocument>,
) -> ApiResult<StatusCode> {
    let digest = bearer(&headers)?;
    if body.document_id.is_nil() {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_document"));
    }
    relay
        .transaction(move |registry| {
            let principal = registry.metadata.authenticate(&digest)?;
            if principal.session != session_id || principal.role != Role::Owner {
                return Err(error(StatusCode::FORBIDDEN, "owner_required"));
            }
            let session = registry.metadata.sessions.get_mut(&session_id).unwrap();
            if session.documents.len() >= MAX_DOCUMENTS
                && !session.documents.contains(&body.document_id)
            {
                return Err(error(StatusCode::TOO_MANY_REQUESTS, "document_limit"));
            }
            session.documents.insert(body.document_id);
            Ok(StatusCode::NO_CONTENT)
        })
        .await
}
pub(super) async fn remove_member(
    State(relay): State<Relay>,
    RoutePath((session_id, member_id)): RoutePath<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    let digest = bearer(&headers)?;
    relay
        .transaction(move |registry| {
            let principal = registry.metadata.authenticate(&digest)?;
            if principal.session != session_id
                || (principal.role != Role::Owner && principal.member != member_id)
            {
                return Err(error(StatusCode::FORBIDDEN, "not_authorized"));
            }
            if principal.role == Role::Owner && principal.member == member_id {
                registry.metadata.sessions.remove(&session_id);
            } else {
                registry
                    .metadata
                    .sessions
                    .get_mut(&session_id)
                    .unwrap()
                    .members
                    .remove(&member_id);
            }
            Ok(StatusCode::NO_CONTENT)
        })
        .await
}
