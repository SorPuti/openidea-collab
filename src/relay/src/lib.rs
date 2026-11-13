//! Bounded, authenticated relay. Only authorization metadata is persisted.
mod control;
mod observability;
mod persistence;
mod socket;
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Path as RoutePath, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bytes::{Bytes, BytesMut};
use control::*;
use futures_util::{SinkExt, StreamExt};
use openidea_collab_protocol::{
    CreateInvite, CreateSession, Endpoint, Frame, Invite, Kind, MAX_FRAME, MAX_TRANSFER,
    RedeemInvite, RegisterDocument, Role, SUBPROTOCOL, ServiceInfo, SessionCredential, VERSION,
    credential_valid,
};
use persistence::{load_metadata, save_metadata};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use socket::upgrade;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use tokio::sync::{Semaphore, mpsc, watch};
use uuid::Uuid;

pub const MAX_SESSIONS: usize = 4;
pub const MAX_MEMBERS: usize = 3;
pub const MAX_CONNECTIONS: usize = 16;
pub const MAX_DOCUMENTS: usize = 64;
pub const QUEUE_FRAMES: usize = 4;
const MAX_INVITES: usize = 16;
const SESSION_TTL: u64 = 4 * 60 * 60;

#[derive(Clone)]
pub struct Relay {
    inner: Arc<Inner>,
}
struct Inner {
    registry: Mutex<Registry>,
    endpoint: Endpoint,
    state_file: Option<PathBuf>,
    permits: Arc<Semaphore>,
    failed: AtomicBool,
    requests: Arc<Semaphore>,
    metadata_writer: Arc<Semaphore>,
    /// SHA-256 of the operator-issued admission token.  The bearer token itself
    /// is never retained after startup or written to the state file.
    admission: Option<[u8; 32]>,
}
struct Registry {
    metadata: Metadata,
    live: HashMap<Uuid, Live>,
    creates: Bucket,
}
struct Mutation {
    metadata: Metadata,
    creates: Bucket,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    version: u8,
    sessions: HashMap<Uuid, Session>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    expires_at: u64,
    members: HashMap<Uuid, Member>,
    invitations: HashMap<Uuid, Invitation>,
    documents: HashSet<Uuid>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    digest: [u8; 32],
    role: Role,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invitation {
    digest: [u8; 32],
    role: Role,
    expires_at: u64,
}
#[derive(Clone)]
struct Live {
    connection: Uuid,
    connected_at: u64,
    session: Uuid,
    sender: mpsc::Sender<Delivery>,
    kick: watch::Sender<bool>,
}
struct Delivery {
    author: Uuid,
    document: Uuid,
    bytes: Bytes,
}
#[derive(Clone, Copy)]
struct Principal {
    session: Uuid,
    member: Uuid,
    role: Role,
}
struct ApiError(StatusCode, &'static str);
type ApiResult<T> = Result<T, ApiError>;
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({"error":self.1}))).into_response()
    }
}
fn error(code: StatusCode, message: &'static str) -> ApiError {
    ApiError(code, message)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn secret() -> String {
    let mut bytes = [0; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}
fn bearer(headers: &HeaderMap) -> ApiResult<[u8; 32]> {
    let value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| credential_valid(v))
        .ok_or(error(StatusCode::UNAUTHORIZED, "authentication_required"))?;
    Ok(hash(value))
}
impl Relay {
    /// Emits a bounded snapshot of unexpired sessions and currently registered sockets.
    pub fn log_presence_snapshot(&self) {
        if let Ok(registry) = self.inner.registry.lock() {
            observability::snapshot(&registry);
        }
    }

    fn require_admission(&self, headers: &HeaderMap) -> ApiResult<()> {
        let Some(expected) = self.inner.admission else {
            return Ok(());
        };
        let supplied = headers
            .get("x-collab-admission")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty() && value.len() <= 256)
            .ok_or(error(StatusCode::UNAUTHORIZED, "admission_required"))?;
        if bool::from(hash(supplied).ct_eq(&expected)) {
            Ok(())
        } else {
            Err(error(StatusCode::UNAUTHORIZED, "admission_required"))
        }
    }
}
impl Metadata {
    fn authenticate(&self, digest: &[u8; 32]) -> ApiResult<Principal> {
        for (session_id, session) in &self.sessions {
            if session.expires_at <= now() {
                continue;
            }
            for (member_id, member) in &session.members {
                if bool::from(member.digest.ct_eq(digest)) {
                    return Ok(Principal {
                        session: *session_id,
                        member: *member_id,
                        role: member.role,
                    });
                }
            }
        }
        Err(error(StatusCode::UNAUTHORIZED, "invalid_credential"))
    }
    fn authorize(&self, principal: Principal, document: Option<Uuid>, write: bool) -> bool {
        self.sessions
            .get(&principal.session)
            .filter(|s| s.expires_at > now())
            .is_some_and(|s| {
                s.members
                    .get(&principal.member)
                    .is_some_and(|m| !write || m.role.can_write())
                    && document.is_none_or(|id| s.documents.contains(&id))
            })
    }
    fn prune(&mut self) {
        let time = now();
        self.sessions.retain(|_, s| s.expires_at > time);
        for session in self.sessions.values_mut() {
            session.invitations.retain(|_, i| i.expires_at > time);
        }
    }
}
impl Relay {
    /// `None` means ephemeral authorization metadata. Documents are never persisted.
    pub fn new(origin: &str, state_file: Option<PathBuf>) -> Result<Self, String> {
        Self::with_admission(origin, state_file, None)
    }

    /// Creates a relay with an optional operator admission token for new rooms.
    /// Public deployments should always supply one; loopback development may
    /// deliberately omit it.
    pub fn with_admission(
        origin: &str,
        state_file: Option<PathBuf>,
        admission_token: Option<&str>,
    ) -> Result<Self, String> {
        let endpoint = Endpoint::parse(origin).map_err(str::to_owned)?;
        if !endpoint.is_loopback() && admission_token.is_none() {
            return Err("public relay requires COLLAB_ADMISSION_TOKEN".into());
        }
        let admission = admission_token
            .map(str::trim)
            .filter(|token| !token.is_empty());
        if admission_token.is_some() && admission.is_none() {
            return Err("COLLAB_ADMISSION_TOKEN must not be empty".into());
        }
        let mut metadata = match &state_file {
            Some(path) => load_metadata(path)?,
            None => Metadata {
                version: VERSION,
                ..Default::default()
            },
        };
        metadata.prune();
        Ok(Self {
            inner: Arc::new(Inner {
                registry: Mutex::new(Registry {
                    metadata,
                    live: HashMap::new(),
                    creates: Bucket::new(4.0, 1.0 / 30.0),
                }),
                endpoint,
                state_file,
                permits: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
                failed: AtomicBool::new(false),
                requests: Arc::new(Semaphore::new(32)),
                metadata_writer: Arc::new(Semaphore::new(1)),
                admission: admission.map(hash),
            }),
        })
    }
    pub fn shutdown(&self) {
        self.inner.failed.store(true, Ordering::Release);
        if let Ok(registry) = self.inner.registry.lock() {
            for live in registry.live.values() {
                let _ = live.kick.send(true);
            }
        }
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/healthz", get(health))
            .route("/v1/info", get(info))
            .route("/v1/sessions", post(create_session))
            .route(
                "/v1/sessions/{session}/invitations",
                post(create_invitation),
            )
            .route("/v1/invitations/redeem", post(redeem_invitation))
            .route("/v1/sessions/{session}/documents", post(register_document))
            .route(
                "/v1/sessions/{session}/members/{member}",
                delete(remove_member),
            )
            .route("/v1/socket", get(upgrade))
            .route("/join/{invitation}", get(invitation_page))
            .route("/assets/invitation.js", get(invitation_script))
            .layer(DefaultBodyLimit::max(2048))
            .layer(axum::middleware::from_fn_with_state(
                self.clone(),
                bound_request,
            ))
            .with_state(self.clone())
    }
    async fn transaction<T, F>(&self, operation: F) -> ApiResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Mutation) -> ApiResult<T> + Send + 'static,
    {
        let inner = self.inner.clone();
        let _writer = inner
            .metadata_writer
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "state_unavailable"))?;
        // Metadata fsync is not performed on the async I/O thread.
        tokio::task::spawn_blocking(move || {
            // A cancelled HTTP future must not release admission while fsync is running.
            let _writer = _writer;
            if inner.failed.load(Ordering::Acquire) {
                return Err(error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "restart_required_after_storage_failure",
                ));
            }
            let mut candidate = {
                let registry = inner
                    .registry
                    .lock()
                    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "state_unavailable"))?;
                Mutation {
                    metadata: registry.metadata.clone(),
                    creates: registry.creates.clone(),
                }
            };
            candidate.metadata.prune();
            let value = operation(&mut candidate);
            {
                let mut registry = inner
                    .registry
                    .lock()
                    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "state_unavailable"))?;
                registry.creates = candidate.creates;
            }
            let value = value?;
            // Disk I/O never holds the data-plane registry lock. Readers continue
            // using the previous authorization snapshot until commit completes.
            if let Some(path) = &inner.state_file
                && save_metadata(path, &candidate.metadata).is_err()
            {
                inner.failed.store(true, Ordering::Release);
                if let Ok(registry) = inner.registry.lock() {
                    for live in registry.live.values() {
                        let _ = live.kick.send(true);
                    }
                }
                return Err(error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "metadata_not_committed",
                ));
            }
            let mut registry = inner
                .registry
                .lock()
                .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "state_unavailable"))?;
            registry.metadata = candidate.metadata;
            for (member, live) in &registry.live {
                let allowed = !inner.failed.load(Ordering::Acquire)
                    && registry
                        .metadata
                        .sessions
                        .get(&live.session)
                        .is_some_and(|s| s.expires_at > now() && s.members.contains_key(member));
                if !allowed {
                    let _ = live.kick.send(true);
                }
            }
            observability::snapshot(&registry);
            Ok(value)
        })
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "state_unavailable"))?
    }
}
async fn bound_request(
    State(relay): State<Relay>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let Ok(_permit) = relay.inner.requests.clone().try_acquire_owned() else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "request_limit").into_response();
    };
    match tokio::time::timeout(Duration::from_secs(5), next.run(request)).await {
        Ok(response) => response,
        Err(_) => error(StatusCode::GATEWAY_TIMEOUT, "request_timeout").into_response(),
    }
}
#[derive(Clone)]
struct Bucket {
    capacity: f64,
    remaining: f64,
    per_second: f64,
    updated: Instant,
}
impl Bucket {
    fn new(capacity: f64, per_second: f64) -> Self {
        Self {
            capacity,
            remaining: capacity,
            per_second,
            updated: Instant::now(),
        }
    }
    fn take(&mut self, cost: f64) -> bool {
        let time = Instant::now();
        self.remaining = (self.remaining
            + time.duration_since(self.updated).as_secs_f64() * self.per_second)
            .min(self.capacity);
        self.updated = time;
        if cost > self.remaining {
            false
        } else {
            self.remaining -= cost;
            true
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn failed_metadata_commit_rolls_back_membership() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("private/metadata.json");
        let relay = Relay::new("http://127.0.0.1:4444", Some(path.clone())).unwrap();
        // A file in place of the configured parent makes the commit fail.
        std::fs::write(path.parent().unwrap(), b"not a directory").unwrap();
        let result = relay
            .transaction(|registry| {
                registry.metadata.sessions.insert(
                    Uuid::new_v4(),
                    Session {
                        expires_at: now() + 100,
                        members: HashMap::new(),
                        invitations: HashMap::new(),
                        documents: HashSet::new(),
                    },
                );
                Ok(())
            })
            .await;
        assert!(matches!(
            result,
            Err(ApiError(StatusCode::INSUFFICIENT_STORAGE, _))
        ));
        assert!(
            relay
                .inner
                .registry
                .lock()
                .unwrap()
                .metadata
                .sessions
                .is_empty()
        );
    }
    #[tokio::test]
    async fn control_mutations_do_not_hold_the_data_plane_lock() {
        let relay = Relay::new("http://127.0.0.1:4444", None).unwrap();
        let inner = relay.inner.clone();
        relay
            .transaction(move |_candidate| {
                assert!(inner.registry.try_lock().is_ok());
                Ok(())
            })
            .await
            .unwrap_or_else(|_| panic!("control operation failed"));
    }
    #[test]
    fn full_recipient_queue_disconnects_instead_of_growing() {
        let relay = Relay::new("http://127.0.0.1:4444", None).unwrap();
        let session_id = Uuid::new_v4();
        let author = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        let document = Uuid::new_v4();
        let (sender, _receiver) = mpsc::channel(1);
        let (kick, kicked) = watch::channel(false);
        sender
            .try_send(Delivery {
                author,
                document,
                bytes: Bytes::from_static(b"queued"),
            })
            .ok()
            .unwrap();
        {
            let mut registry = relay.inner.registry.lock().unwrap();
            registry.metadata.sessions.insert(
                session_id,
                Session {
                    expires_at: now() + 100,
                    members: HashMap::from([
                        (
                            author,
                            Member {
                                digest: [1; 32],
                                role: Role::Editor,
                            },
                        ),
                        (
                            recipient,
                            Member {
                                digest: [2; 32],
                                role: Role::Viewer,
                            },
                        ),
                    ]),
                    invitations: HashMap::new(),
                    documents: HashSet::from([document]),
                },
            );
            registry.live.insert(
                recipient,
                Live {
                    connected_at: now(),
                    connection: Uuid::new_v4(),
                    session: session_id,
                    sender,
                    kick,
                },
            );
        }
        let data = Frame {
            kind: Kind::Update,
            document,
            target: recipient,
            message: Uuid::new_v4(),
            part: 0,
            parts: 1,
            total: 1,
            payload: b"x",
        }
        .encode()
        .unwrap();
        assert!(socket::forward(
            &relay,
            Principal {
                session: session_id,
                member: author,
                role: Role::Editor
            },
            &Frame::decode(&data).unwrap(),
            &Bytes::copy_from_slice(&data)
        ));
        assert!(*kicked.borrow());
    }
    #[test]
    fn expired_credentials_are_not_authorized() {
        let mut metadata = Metadata::default();
        let session = Uuid::new_v4();
        let member = Uuid::new_v4();
        metadata.sessions.insert(
            session,
            Session {
                expires_at: now() - 1,
                members: HashMap::from([(
                    member,
                    Member {
                        digest: [3; 32],
                        role: Role::Owner,
                    },
                )]),
                invitations: HashMap::new(),
                documents: HashSet::new(),
            },
        );
        assert!(metadata.authenticate(&[3; 32]).is_err());
        assert!(!metadata.authorize(
            Principal {
                session,
                member,
                role: Role::Owner
            },
            None,
            false
        ));
    }
}
