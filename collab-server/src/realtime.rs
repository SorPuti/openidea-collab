//! Experimental versioned CRDT API. Not mounted by the legacy server yet.
//! Tokens are sent once over the API, kept as hashes in memory, and scoped to one room.
use crate::collaboration::{merge_updates, SharedDocument, MAX_UPDATE_BYTES};
use crate::journal::{Event as JournalEvent, Journal, JournalLimits, JournalRole, QuotaExceeded};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{
        header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, ORIGIN, X_CONTENT_TYPE_OPTIONS},
        HeaderMap, HeaderValue, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::Path as FilePath,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

const MAX_ROOMS: usize = 100;
const MAX_MEMBERS: usize = 64;
const MAX_INVITES: usize = 64;
const MAX_DOCUMENTS: usize = 64;
type DigestBytes = [u8; 32];
type AppState = Arc<Mutex<Store>>;

#[derive(Default)]
struct Store {
    rooms: HashMap<String, Room>,
    journal: Option<Journal>,
    sequence: u64,
}
impl Deref for Store {
    type Target = HashMap<String, Room>;
    fn deref(&self) -> &Self::Target {
        &self.rooms
    }
}
impl DerefMut for Store {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rooms
    }
}
impl Store {
    fn record(&mut self, event: &JournalEvent) -> Result<(), StatusCode> {
        if let Some(journal) = &mut self.journal {
            self.sequence = journal.append(self.sequence, event).map_err(|err| {
                if err.downcast_ref::<QuotaExceeded>().is_some() {
                    return StatusCode::INSUFFICIENT_STORAGE;
                }
                tracing::error!(error = %err, "journal commit failed");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Role {
    Viewer,
    Editor,
}
struct Member {
    token_hash: DigestBytes,
    role: Role,
}
struct Invite {
    token_hash: DigestBytes,
    role: Role,
    expires: u64,
}
struct Room {
    owner_hash: DigestBytes,
    members: HashMap<String, Member>,
    invites: Vec<Invite>,
    documents: HashMap<String, Document>,
}
struct Document {
    state: SharedDocument,
    revision: u64,
    changes: tokio::sync::watch::Sender<u64>,
    merged_update: Option<Vec<u8>>,
}
impl Default for Document {
    fn default() -> Self {
        let (changes, _) = tokio::sync::watch::channel(0);
        Self {
            state: SharedDocument::new(),
            revision: 0,
            changes,
            merged_update: None,
        }
    }
}
#[derive(Serialize)]
struct CreatedRoom {
    room_id: String,
    owner_token: String,
}
#[derive(Serialize)]
struct CreatedInvite {
    token: String,
}
#[derive(Serialize)]
struct Joined {
    member_id: String,
    member_token: String,
}
#[derive(Deserialize)]
struct InviteRequest {
    role: Role,
    ttl_seconds: u64,
}
#[derive(Deserialize)]
struct JoinRequest {
    token: String,
}
#[derive(Deserialize)]
struct ChangeQuery {
    after: u64,
}
#[derive(Serialize)]
struct Revision {
    revision: u64,
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hash_token(value: &str) -> Option<DigestBytes> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(Sha256::digest(value.as_bytes()).into())
}
fn same_token(left: &DigestBytes, right: &DigestBytes) -> bool {
    bool::from(left.ct_eq(right))
}
fn bearer(headers: &HeaderMap) -> Result<DigestBytes, StatusCode> {
    let value = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .and_then(hash_token);
    value.ok_or(StatusCode::UNAUTHORIZED)
}
fn owner(room: &Room, token: &DigestBytes) -> bool {
    same_token(&room.owner_hash, token)
}
fn role(room: &Room, token: &DigestBytes) -> Option<Role> {
    if owner(room, token) {
        return Some(Role::Editor);
    }
    room.members
        .values()
        .find(|m| same_token(&m.token_hash, token))
        .map(|m| m.role)
}
fn document_id_valid(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        && id != "."
        && id != ".."
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn journal_role(role: Role) -> JournalRole {
    match role {
        Role::Viewer => JournalRole::Viewer,
        Role::Editor => JournalRole::Editor,
    }
}
fn from_journal(role: JournalRole) -> Role {
    match role {
        JournalRole::Viewer => Role::Viewer,
        JournalRole::Editor => Role::Editor,
    }
}

pub fn router() -> Router {
    router_with_store(Store::default())
}

/// Recover the entire log before exposing any route, with one server per database.
pub fn persistent_router(path: &FilePath) -> anyhow::Result<Router> {
    persistent_router_with_limits(path, JournalLimits::default())
}

pub fn persistent_router_with_limits(
    path: &FilePath,
    limits: JournalLimits,
) -> anyhow::Result<Router> {
    let journal = Journal::open_with_limits(path, limits)?;
    journal.claim_exclusive()?;
    let events = journal.read_all()?;
    let mut store = Store::default();
    for event in &events {
        replay(&mut store, event)?;
    }
    for room in store.rooms.values_mut() {
        room.invites.retain(|invite| invite.expires > now_unix());
    }
    store.sequence = events.len().try_into()?;
    store.journal = Some(journal);
    Ok(router_with_store(store))
}

fn replay(store: &mut Store, event: &JournalEvent) -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    match event {
        JournalEvent::RoomCreated { room, owner_hash } => {
            ensure!(
                !store.contains_key(room) && store.len() < MAX_ROOMS,
                "invalid room creation"
            );
            store.insert(
                room.clone(),
                Room {
                    owner_hash: *owner_hash,
                    members: HashMap::new(),
                    invites: Vec::new(),
                    documents: HashMap::new(),
                },
            );
        }
        JournalEvent::InviteCreated {
            room,
            invite_hash,
            role,
            expires_unix,
        } => {
            let room = store.get_mut(room).context("invite room missing")?;
            ensure!(
                !room
                    .invites
                    .iter()
                    .any(|i| same_token(&i.token_hash, invite_hash)),
                "duplicate invitation"
            );
            room.invites.push(Invite {
                token_hash: *invite_hash,
                role: from_journal(*role),
                expires: *expires_unix,
            });
        }
        JournalEvent::MemberJoined {
            room,
            invite_hash,
            member,
            member_hash,
            role,
        } => {
            let room = store.get_mut(room).context("join room missing")?;
            let index = room
                .invites
                .iter()
                .position(|i| same_token(&i.token_hash, invite_hash))
                .context("join invitation missing")?;
            ensure!(
                room.invites[index].role == from_journal(*role),
                "join role mismatch"
            );
            ensure!(
                !room.members.contains_key(member) && room.members.len() < MAX_MEMBERS,
                "duplicate member or room full"
            );
            room.invites.remove(index);
            room.members.insert(
                member.clone(),
                Member {
                    token_hash: *member_hash,
                    role: from_journal(*role),
                },
            );
        }
        JournalEvent::MemberRevoked { room, member } => {
            let room = store.get_mut(room).context("revoke room missing")?;
            ensure!(
                room.members.remove(member).is_some(),
                "revoked member missing"
            );
        }
        JournalEvent::DocumentUpdated {
            room,
            document,
            revision,
            update,
        } => {
            let room = store.get_mut(room).context("document room missing")?;
            ensure!(document_id_valid(document), "invalid document ID");
            if !room.documents.contains_key(document) {
                ensure!(
                    *revision == 1 && room.documents.len() < MAX_DOCUMENTS,
                    "invalid document initialization"
                );
                room.documents.insert(document.clone(), Document::default());
            }
            let doc = room.documents.get_mut(document).unwrap();
            ensure!(
                doc.revision.checked_add(1) == Some(*revision),
                "document revision gap"
            );
            doc.state.apply(update).map_err(anyhow::Error::msg)?;
            let merged = match &doc.merged_update {
                Some(previous) => merge_updates(previous, update).map_err(anyhow::Error::msg)?,
                None => update.clone(),
            };
            anyhow::ensure!(
                merged.len() <= MAX_UPDATE_BYTES,
                "recovered document exceeds current limit"
            );
            anyhow::ensure!(
                doc.state.diff(None).map_err(anyhow::Error::msg)?.len() <= MAX_UPDATE_BYTES,
                "recovered document snapshot exceeds current limit"
            );
            doc.merged_update = Some(merged);
            doc.revision = *revision;
            doc.changes.send_replace(*revision);
        }
    }
    Ok(())
}

fn router_with_store(store: Store) -> Router {
    let state: AppState = Arc::new(Mutex::new(store));
    Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"service":"openidea-collab","status":"ok","api":"realtime-experimental"})) }))
        .route("/v1/rooms", post(create_room))
        .route("/v1/rooms/{room}/invites", post(create_invite))
        .route("/v1/rooms/{room}/join", post(join_room))
        .route("/v1/rooms/{room}/members/{member}", delete(revoke_member))
        .route("/v1/rooms/{room}/documents/{document}", get(get_document).put(initialize_document).post(update_document))
        .route("/v1/rooms/{room}/documents/{document}/changes", get(document_changes))
        .layer(DefaultBodyLimit::max(MAX_UPDATE_BYTES))
        .layer(middleware::from_fn(reject_browser_origins))
        .layer(middleware::map_response(no_store))
        .with_state(state)
}

async fn reject_browser_origins(request: Request, next: Next) -> Response {
    // This API is for the native helper. A web origin cannot create rooms on localhost.
    if request.headers().contains_key(ORIGIN) || request.headers().contains_key("sec-fetch-site") {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    response
}

async fn create_room(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CreatedRoom>), StatusCode> {
    let mut rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if rooms.len() >= MAX_ROOMS {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let room_id = Uuid::new_v4().to_string();
    let token = new_token();
    let owner_hash = hash_token(&token).expect("generated token");
    rooms.record(&JournalEvent::RoomCreated {
        room: room_id.clone(),
        owner_hash,
    })?;
    rooms.insert(
        room_id.clone(),
        Room {
            owner_hash,
            members: HashMap::new(),
            invites: Vec::new(),
            documents: HashMap::new(),
        },
    );
    Ok((
        StatusCode::CREATED,
        Json(CreatedRoom {
            room_id,
            owner_token: token,
        }),
    ))
}

async fn create_invite(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<InviteRequest>,
) -> Result<Json<CreatedInvite>, StatusCode> {
    let token_hash = bearer(&headers)?;
    if !(1..=3600).contains(&req.ttl_seconds) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = rooms.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    if !owner(room, &token_hash) {
        return Err(StatusCode::FORBIDDEN);
    }
    room.invites.retain(|i| i.expires > now_unix());
    if room.invites.len() >= MAX_INVITES {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let token = new_token();
    let invite_hash = hash_token(&token).expect("generated token");
    let expires = now_unix().saturating_add(req.ttl_seconds);
    rooms.record(&JournalEvent::InviteCreated {
        room: id.clone(),
        invite_hash,
        role: journal_role(req.role),
        expires_unix: expires,
    })?;
    rooms
        .get_mut(&id)
        .expect("room exists")
        .invites
        .push(Invite {
            token_hash: invite_hash,
            role: req.role,
            expires,
        });
    Ok(Json(CreatedInvite { token }))
}

async fn join_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<JoinRequest>,
) -> Result<Json<Joined>, StatusCode> {
    let token_hash = hash_token(&req.token).ok_or(StatusCode::UNAUTHORIZED)?;
    let mut rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = rooms.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    let now = now_unix();
    room.invites.retain(|i| i.expires > now);
    if room.members.len() >= MAX_MEMBERS {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let index = room
        .invites
        .iter()
        .position(|i| same_token(&i.token_hash, &token_hash))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let invitation_role = room.invites[index].role;
    let member_id = Uuid::new_v4().to_string();
    let token = new_token();
    let member_hash = hash_token(&token).expect("generated token");
    rooms.record(&JournalEvent::MemberJoined {
        room: id.clone(),
        invite_hash: token_hash,
        member: member_id.clone(),
        member_hash,
        role: journal_role(invitation_role),
    })?;
    let room = rooms.get_mut(&id).expect("room exists");
    room.invites.remove(index);
    room.members.insert(
        member_id.clone(),
        Member {
            token_hash: member_hash,
            role: invitation_role,
        },
    );
    Ok(Json(Joined {
        member_id,
        member_token: token,
    }))
}

async fn revoke_member(
    State(state): State<AppState>,
    Path((id, member)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let token = bearer(&headers)?;
    let mut rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = rooms.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    if !owner(room, &token) {
        return Err(StatusCode::FORBIDDEN);
    }
    if !room.members.contains_key(&member) {
        return Err(StatusCode::NOT_FOUND);
    }
    rooms.record(&JournalEvent::MemberRevoked {
        room: id.clone(),
        member: member.clone(),
    })?;
    rooms
        .get_mut(&id)
        .expect("room exists")
        .members
        .remove(&member);
    Ok(StatusCode::NO_CONTENT)
}

async fn get_document(
    State(state): State<AppState>,
    Path((id, document)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let token = bearer(&headers)?;
    if !document_id_valid(&document) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = rooms.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if role(room, &token).is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let doc = room.documents.get(&document).ok_or(StatusCode::NOT_FOUND)?;
    let snapshot = doc
        .state
        .diff(None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if snapshot.len() > MAX_UPDATE_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let mut response = Bytes::from(snapshot).into_response();
    response.headers_mut().insert(
        "x-collab-revision",
        HeaderValue::from_str(&doc.revision.to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    Ok(response)
}

async fn update_document(
    State(state): State<AppState>,
    Path((id, document)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, StatusCode> {
    let token = bearer(&headers)?;
    if !document_id_valid(&document) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = rooms.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    match role(room, &token) {
        Some(Role::Editor) => {}
        Some(Role::Viewer) => return Err(StatusCode::FORBIDDEN),
        None => return Err(StatusCode::UNAUTHORIZED),
    }
    let doc = room
        .documents
        .get_mut(&document)
        .ok_or(StatusCode::NOT_FOUND)?;
    let merged = merge_updates(
        doc.merged_update
            .as_deref()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        &body,
    )
    .map_err(|_| StatusCode::BAD_REQUEST)?;
    if merged.len() > MAX_UPDATE_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let candidate = SharedDocument::new();
    candidate
        .apply(&merged)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if candidate
        .diff(None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .len()
        > MAX_UPDATE_BYTES
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let revision = doc
        .revision
        .checked_add(1)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    rooms.record(&JournalEvent::DocumentUpdated {
        room: id.clone(),
        document: document.clone(),
        revision,
        update: body.to_vec(),
    })?;
    let doc = rooms
        .get_mut(&id)
        .unwrap()
        .documents
        .get_mut(&document)
        .unwrap();
    doc.state = candidate;
    doc.merged_update = Some(merged);
    doc.revision = revision;
    doc.changes.send_replace(revision);
    Ok(StatusCode::NO_CONTENT)
}

async fn initialize_document(
    State(state): State<AppState>,
    Path((id, document)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, StatusCode> {
    let token = bearer(&headers)?;
    if !document_id_valid(&document) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = rooms.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    match role(room, &token) {
        Some(Role::Editor) => {}
        Some(Role::Viewer) => return Err(StatusCode::FORBIDDEN),
        None => return Err(StatusCode::UNAUTHORIZED),
    }
    if room.documents.contains_key(&document) {
        return Err(StatusCode::CONFLICT);
    }
    if room.documents.len() >= MAX_DOCUMENTS {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let mut doc = Document::default();
    doc.state
        .apply(&body)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if doc
        .state
        .diff(None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .len()
        > MAX_UPDATE_BYTES
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    doc.revision = 1;
    doc.changes.send_replace(1);
    doc.merged_update = Some(body.to_vec());
    rooms.record(&JournalEvent::DocumentUpdated {
        room: id.clone(),
        document: document.clone(),
        revision: 1,
        update: body.to_vec(),
    })?;
    rooms.get_mut(&id).unwrap().documents.insert(document, doc);
    Ok(StatusCode::NO_CONTENT)
}

async fn document_changes(
    State(state): State<AppState>,
    Path((id, document)): Path<(String, String)>,
    Query(query): Query<ChangeQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let token = bearer(&headers)?;
    if !document_id_valid(&document) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (revision, mut changes) = {
        let rooms = state
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let room = rooms.get(&id).ok_or(StatusCode::NOT_FOUND)?;
        if role(room, &token).is_none() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let doc = room.documents.get(&document).ok_or(StatusCode::NOT_FOUND)?;
        (doc.revision, doc.changes.subscribe())
    };
    if revision <= query.after {
        let _ = tokio::time::timeout(Duration::from_secs(5), changes.changed()).await;
    }
    // Re-check authorization after waiting: revocation must apply to pending polls.
    let rooms = state
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let room = rooms.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if role(room, &token).is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let doc = room.documents.get(&document).ok_or(StatusCode::NOT_FOUND)?;
    if doc.revision <= query.after {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    Ok(Json(Revision {
        revision: doc.revision,
    })
    .into_response())
}
