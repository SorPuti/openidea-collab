//! Explicitly allowlisted operational metadata; never log payloads or credentials.
use super::*;

pub(super) fn connection_event(event: &'static str, principal: Principal, connection: Uuid) {
    eprintln!(
        "{}",
        serde_json::json!({
            "event": event, "timestamp": now(), "session_id": principal.session,
            "member_id": principal.member, "connection_id": connection, "role": principal.role
        })
    );
}

pub(super) fn snapshot(registry: &Registry) {
    let timestamp = now();
    let sessions: Vec<_> = registry
        .metadata
        .sessions
        .iter()
        .filter(|(_, session)| session.expires_at > timestamp)
        .map(|(id, session)| {
            let connected: Vec<_> = registry
                .live
                .iter()
                .filter_map(|(member_id, live)| {
                    if live.session != *id {
                        return None;
                    }
                    session.members.get(member_id).map(|member| serde_json::json!({
                    "member_id": member_id, "role": member.role, "connection_id": live.connection,
                    "connected_at": live.connected_at
                }))
                })
                .collect();
            serde_json::json!({"session_id": id, "expires_at": session.expires_at,
                "registered_members": session.members.len(), "connected": connected})
        })
        .collect();
    eprintln!(
        "{}",
        serde_json::json!({"event": "snapshot", "timestamp": timestamp, "sessions": sessions})
    );
}
