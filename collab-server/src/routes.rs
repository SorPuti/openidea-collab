use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::events::{EventEnvelope, RoomEvent};
use crate::room::{FileState, Member, Room};

pub type AppState = Arc<Mutex<HashMap<String, Room>>>;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Serialize)]
struct CreateRoomResponse {
    room_id: String,
}

#[derive(Serialize)]
struct MemberInfo {
    user_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_file: Option<String>,
}

#[derive(Serialize)]
struct RoomInfoResponse {
    room_id: String,
    members: Vec<MemberInfo>,
}

#[derive(Deserialize)]
struct JoinRequest {
    name: String,
}

#[derive(Serialize)]
struct JoinResponse {
    user_id: String,
}

#[derive(Deserialize)]
struct FileUpdateRequest {
    user_id: String,
    version: u64,
    text: String,
}

#[derive(Serialize)]
struct FileUpdateResponse {
    event_id: u64,
    version: u64,
}

#[derive(Serialize)]
struct FileGetResponse {
    version: u64,
    text: String,
    user_id: String,
}

#[derive(Deserialize)]
struct EventsQuery {
    cursor: Option<u64>,
}

#[derive(Serialize)]
struct EventsResponse {
    events: Vec<EventEnvelope>,
    next_cursor: u64,
}

#[derive(Deserialize)]
struct PresenceRequest {
    current_file: Option<String>,
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/rooms", post(create_room))
        .route("/rooms/:id", get(get_room))
        .route("/rooms/:id/join", post(join_room))
        .route("/rooms/:id/members/:uid", delete(leave_room))
        .route("/rooms/:id/files/*path", post(update_file).get(get_file))
        .route("/rooms/:id/members", get(get_members))
        .route("/rooms/:id/events", get(get_events))
        .route("/rooms/:id/members/:uid/presence", post(post_presence))
        .with_state(state)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn create_room(State(state): State<AppState>) -> Response {
    let room_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();

    let result = state.lock().map(|mut guard| {
        guard.insert(room_id.clone(), Room::new(room_id.clone()));
    });

    match result {
        Ok(()) => (StatusCode::CREATED, Json(CreateRoomResponse { room_id })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_room(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<RoomInfoResponse>, StatusCode> {
    let guard = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = guard.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let members = room
        .members
        .values()
        .map(|m| MemberInfo {
            user_id: m.user_id.clone(),
            name: m.name.clone(),
            current_file: m.current_file.clone(),
        })
        .collect();
    Ok(Json(RoomInfoResponse {
        room_id: room.id.clone(),
        members,
    }))
}

async fn join_room(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<JoinRequest>,
) -> Result<Json<JoinResponse>, StatusCode> {
    let user_id = Uuid::new_v4().to_string();
    let mut guard = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = guard.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    room.add_member(Member {
        user_id: user_id.clone(),
        name: body.name.clone(),
        current_file: None,
        last_seen: Instant::now(),
    });
    room.events.push(RoomEvent::MemberJoined {
        user_id: user_id.clone(),
        name: body.name,
    });
    Ok(Json(JoinResponse { user_id }))
}

async fn leave_room(
    Path((id, uid)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let mut guard = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = guard.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    room.remove_member(&uid);
    room.events.push(RoomEvent::MemberLeft { user_id: uid });
    Ok(StatusCode::NO_CONTENT)
}

async fn update_file(
    Path((id, raw_path)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(body): Json<FileUpdateRequest>,
) -> Response {
    let path = raw_path.trim_start_matches('/').to_string();

    let result = state
        .lock()
        .map(|mut guard| -> Result<(u64, u64), Box<Response>> {
            let room = match guard.get_mut(&id) {
                Some(r) => r,
                None => return Err(Box::new(StatusCode::NOT_FOUND.into_response())),
            };

            // Update presence for the requesting user.
            room.update_presence(&body.user_id, Some(path.clone()));

            // Detect conflict: version mismatch from a different user.
            let conflict_info: Option<(String, String, u64)> =
                room.files.get(&path).and_then(|file| {
                    if file.version != body.version && file.last_user != body.user_id {
                        Some((file.last_user.clone(), file.text.clone(), file.version))
                    } else {
                        None
                    }
                });

            // On conflict emit a warning event and return 409 — no fake merge.
            let final_text = if let Some((last_user, server_text, server_version)) = conflict_info {
                room.events.push(RoomEvent::ConflictWarn {
                    path: path.clone(),
                    user_a: body.user_id.clone(),
                    user_b: last_user,
                });
                return Err(Box::new(
                    (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "error": "conflict",
                            "server_text": server_text,
                            "server_version": server_version,
                        })),
                    )
                        .into_response(),
                ));
            } else {
                body.text
            };

            let next_version = room.files.get(&path).map(|f| f.version + 1).unwrap_or(1);

            room.files.insert(
                path.clone(),
                FileState {
                    version: next_version,
                    text: final_text,
                    last_user: body.user_id.clone(),
                    last_updated: Instant::now(),
                },
            );

            let event_id = room.events.push(RoomEvent::FileUpdated {
                path,
                version: next_version,
                user_id: body.user_id,
            });

            Ok((event_id, next_version))
        });

    match result {
        Ok(Ok((event_id, version))) => (
            StatusCode::OK,
            Json(FileUpdateResponse { event_id, version }),
        )
            .into_response(),
        Ok(Err(boxed)) => *boxed,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn get_file(
    Path((id, raw_path)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<FileGetResponse>, StatusCode> {
    let path = raw_path.trim_start_matches('/').to_string();
    let guard = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = guard.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let file = room.files.get(&path).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(FileGetResponse {
        version: file.version,
        text: file.text.clone(),
        user_id: file.last_user.clone(),
    }))
}

async fn get_members(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<MemberInfo>>, StatusCode> {
    let guard = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = guard.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let members = room
        .members
        .values()
        .map(|m| MemberInfo {
            user_id: m.user_id.clone(),
            name: m.name.clone(),
            current_file: m.current_file.clone(),
        })
        .collect();
    Ok(Json(members))
}

async fn get_events(
    Path(id): Path<String>,
    Query(params): Query<EventsQuery>,
    State(state): State<AppState>,
) -> Result<Json<EventsResponse>, StatusCode> {
    let cursor = params.cursor.unwrap_or(0);

    // Snapshot events under mutex (no blocking inside the lock).
    let (initial_events, base_next_cursor) = {
        let guard = state
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let room = guard.get(&id).ok_or(StatusCode::NOT_FOUND)?;
        (room.events.since(cursor), room.events.next_id())
    };

    if !initial_events.is_empty() {
        let next_cursor = initial_events
            .iter()
            .map(|e| e.id + 1)
            .max()
            .unwrap_or(base_next_cursor);
        return Ok(Json(EventsResponse {
            events: initial_events,
            next_cursor,
        }));
    }

    // No events yet — long-poll: sleep 5 s outside the mutex.
    tokio::time::sleep(std::time::Duration::from_millis(5000)).await;

    let guard = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = guard.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let events = room.events.since(cursor);
    let next_cursor = events
        .iter()
        .map(|e| e.id + 1)
        .max()
        .unwrap_or_else(|| room.events.next_id());

    Ok(Json(EventsResponse {
        events,
        next_cursor,
    }))
}

async fn post_presence(
    Path((id, uid)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(body): Json<PresenceRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut guard = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = guard.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    if !room.members.contains_key(&uid) {
        return Err(StatusCode::NOT_FOUND);
    }
    room.update_presence(&uid, body.current_file.clone());
    room.events.push(RoomEvent::PresenceChanged {
        user_id: uid,
        file: body.current_file,
    });
    Ok(StatusCode::NO_CONTENT)
}
