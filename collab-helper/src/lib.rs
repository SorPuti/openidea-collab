//! Experimental native client for the versioned CRDT API.
//! Used by the experimental custom editor's PTY helper; the legacy panel is separate.
use openidea_collab_core::SharedDocument;
use reqwest::{redirect::Policy, StatusCode, Url};
use std::{collections::VecDeque, net::IpAddr, time::Duration};
use zeroize::Zeroizing;

pub mod project;

#[cfg(unix)]
pub mod recovery;

pub struct Client {
    http: reqwest::Client,
    endpoint: String,
    base: String,
    room_id: String,
    token: Zeroizing<String>,
    document: SharedDocument,
    revision: u64,
    pending: VecDeque<Vec<u8>>,
}

impl Client {
    /// Remote endpoints must use HTTPS. Plain HTTP is restricted to loopback.
    pub fn validate_endpoint(endpoint: &str) -> Result<Url, String> {
        let url = Url::parse(endpoint).map_err(|_| "invalid endpoint".to_owned())?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err("endpoint must not contain credentials, path, query or fragment".into());
        }
        let host = url.host_str().ok_or("missing endpoint host")?;
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
            return Err("remote endpoints require HTTPS".into());
        }
        Ok(url)
    }

    pub async fn attach(
        endpoint: &str,
        room: &str,
        document: &str,
        token: &str,
        local_text: &str,
    ) -> Result<Self, String> {
        let base = Self::validate_endpoint(endpoint)?;
        uuid::Uuid::parse_str(room).map_err(|_| "invalid room id")?;
        if document.is_empty()
            || document.len() > 128
            || !document
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            || document == "."
            || document == ".."
        {
            return Err("invalid document id".into());
        }
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid credential".into());
        }
        let mut builder = reqwest::Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(10));
        // Never send a loopback session credential through an environment-configured proxy.
        if base.scheme() == "http" {
            builder = builder.no_proxy();
        }
        let http = builder
            .build()
            .map_err(|_| "could not create HTTP client")?;
        let endpoint = base
            .join(&format!("v1/rooms/{room}/documents/{document}"))
            .map_err(|_| "invalid document endpoint")?
            .to_string();
        let mut client = Self {
            http,
            endpoint,
            base: base.to_string(),
            room_id: room.to_owned(),
            token: Zeroizing::new(token.to_owned()),
            document: SharedDocument::new(),
            revision: 0,
            pending: VecDeque::new(),
        };
        let response = client.get_snapshot().await?;
        if response.status() == StatusCode::NOT_FOUND {
            let initial = client.document.edit(0, 0, local_text)?;
            let result = client
                .http
                .put(&client.endpoint)
                .bearer_auth(client.token.as_str())
                .body(initial)
                .send()
                .await
                .map_err(|_| "network error while initializing")?;
            if result.status() != StatusCode::NO_CONTENT && result.status() != StatusCode::CONFLICT
            {
                return Err(format!(
                    "could not initialize document: HTTP {}",
                    result.status()
                ));
            }
            // On conflict another client initialized first; never merge two independent initial files.
            client.document = SharedDocument::new();
            let response = client.get_snapshot().await?;
            client.apply_snapshot(response).await?;
        } else {
            client.apply_snapshot(response).await?;
        }
        if client.document.text() != local_text {
            return Err(
                "local file differs from shared document; manual resolution required".into(),
            );
        }
        Ok(client)
    }

    async fn get_snapshot(&self) -> Result<reqwest::Response, String> {
        self.http
            .get(&self.endpoint)
            .bearer_auth(self.token.as_str())
            .send()
            .await
            .map_err(|_| "network error while reading document".into())
    }

    async fn apply_snapshot(&mut self, mut response: reqwest::Response) -> Result<(), String> {
        if response.status() != StatusCode::OK {
            return Err(format!(
                "could not read document: HTTP {}",
                response.status()
            ));
        }
        let revision = response
            .headers()
            .get("x-collab-revision")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or("missing document revision")?;
        if response
            .content_length()
            .is_some_and(|n| n > openidea_collab_core::MAX_UPDATE_BYTES as u64)
        {
            return Err("snapshot exceeds size limit".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "could not read snapshot")?
        {
            if bytes.len().saturating_add(chunk.len()) > openidea_collab_core::MAX_UPDATE_BYTES {
                return Err("snapshot exceeds size limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        self.document.apply(&bytes)?;
        self.revision = revision;
        Ok(())
    }

    /// Queue locally before sending, so an uncertain network result is retryable.
    pub fn edit(&mut self, from: u32, to: u32, insert: &str) -> Result<(), String> {
        if self.pending.len() >= 1024 {
            return Err("too many pending updates".into());
        }
        let update = self.document.edit(from, to, insert)?;
        self.pending.push_back(update);
        Ok(())
    }

    pub fn text(&self) -> String {
        self.document.text()
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub async fn create_invite(
        &self,
        role: &str,
        ttl_seconds: u64,
    ) -> Result<InviteCredential, String> {
        SessionApi::new(&self.base)?
            .create_invite(&self.room_id, self.token.as_str(), role, ttl_seconds)
            .await
    }

    /// Merge the server snapshot, retry pending idempotent updates, then refresh revision.
    pub async fn sync(&mut self) -> Result<(), String> {
        let response = self.get_snapshot().await?;
        self.apply_snapshot(response).await?;
        while let Some(update) = self.pending.front() {
            let response = self
                .http
                .post(&self.endpoint)
                .bearer_auth(self.token.as_str())
                .body(update.clone())
                .send()
                .await
                .map_err(|_| "network error while sending update")?;
            if response.status() != StatusCode::NO_CONTENT {
                return Err(format!("update rejected: HTTP {}", response.status()));
            }
            self.pending.pop_front();
        }
        let response = self.get_snapshot().await?;
        self.apply_snapshot(response).await
    }

    /// Wait for a server change. Caller controls cancellation and reconnection.
    pub async fn wait_for_change(&mut self) -> Result<bool, String> {
        let url = format!("{}/changes?after={}", self.endpoint, self.revision);
        let response = self
            .http
            .get(url)
            .bearer_auth(self.token.as_str())
            .send()
            .await
            .map_err(|_| "network error while waiting")?;
        match response.status() {
            StatusCode::NO_CONTENT => Ok(false),
            StatusCode::OK => {
                self.sync().await?;
                Ok(true)
            }
            status => Err(format!("change notification rejected: HTTP {status}")),
        }
    }
}

/// Room setup API. Secrets are not printable via Debug and are wiped on drop.
pub struct SessionApi {
    http: reqwest::Client,
    base: Url,
}
pub struct RoomCredential {
    pub room_id: String,
    token: Zeroizing<String>,
}
pub struct InviteCredential {
    token: Zeroizing<String>,
}
pub struct MemberCredential {
    pub member_id: String,
    token: Zeroizing<String>,
}
impl RoomCredential {
    pub fn token(&self) -> &str {
        self.token.as_str()
    }
}
impl InviteCredential {
    pub fn token(&self) -> &str {
        self.token.as_str()
    }
}
impl MemberCredential {
    pub fn token(&self) -> &str {
        self.token.as_str()
    }
}

#[derive(serde::Deserialize)]
struct RoomResponse {
    room_id: String,
    owner_token: String,
}
#[derive(serde::Deserialize)]
struct InviteResponse {
    token: String,
}
#[derive(serde::Deserialize)]
struct MemberResponse {
    member_id: String,
    member_token: String,
}

async fn read_json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, String> {
    if !response.status().is_success() {
        return Err(format!("request rejected: HTTP {}", response.status()));
    }
    let mut data = Zeroizing::new(Vec::new());
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "could not read response")?
    {
        if data.len().saturating_add(chunk.len()) > 16 * 1024 {
            return Err("response exceeds size limit".into());
        }
        data.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&data).map_err(|_| "invalid server response".into())
}

fn valid_room(room: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(room)
        .map(|_| ())
        .map_err(|_| "invalid room id".into())
}
fn valid_token(token: &str) -> Result<(), String> {
    if token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("invalid credential".into())
    }
}

impl SessionApi {
    pub fn new(endpoint: &str) -> Result<Self, String> {
        let base = Client::validate_endpoint(endpoint)?;
        let mut builder = reqwest::Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(10));
        if base.scheme() == "http" {
            builder = builder.no_proxy();
        }
        let http = builder
            .build()
            .map_err(|_| "could not create HTTP client")?;
        Ok(Self { http, base })
    }
    pub async fn create_room(&self) -> Result<RoomCredential, String> {
        let url = self.base.join("v1/rooms").map_err(|_| "invalid endpoint")?;
        let response = self
            .http
            .post(url)
            .send()
            .await
            .map_err(|_| "network error creating room")?;
        let response: RoomResponse = read_json(response).await?;
        valid_room(&response.room_id)?;
        valid_token(&response.owner_token)?;
        Ok(RoomCredential {
            room_id: response.room_id,
            token: Zeroizing::new(response.owner_token),
        })
    }
    pub async fn create_invite(
        &self,
        room: &str,
        owner_token: &str,
        role: &str,
        ttl_seconds: u64,
    ) -> Result<InviteCredential, String> {
        valid_room(room)?;
        valid_token(owner_token)?;
        if !matches!(role, "editor" | "viewer") || !(1..=3600).contains(&ttl_seconds) {
            return Err("invalid invite parameters".into());
        }
        let url = self
            .base
            .join(&format!("v1/rooms/{room}/invites"))
            .map_err(|_| "invalid endpoint")?;
        let response = self
            .http
            .post(url)
            .bearer_auth(owner_token)
            .json(&serde_json::json!({"role":role,"ttl_seconds":ttl_seconds}))
            .send()
            .await
            .map_err(|_| "network error creating invite")?;
        let response: InviteResponse = read_json(response).await?;
        valid_token(&response.token)?;
        Ok(InviteCredential {
            token: Zeroizing::new(response.token),
        })
    }
    pub async fn join_room(
        &self,
        room: &str,
        invite_token: &str,
    ) -> Result<MemberCredential, String> {
        valid_room(room)?;
        valid_token(invite_token)?;
        let url = self
            .base
            .join(&format!("v1/rooms/{room}/join"))
            .map_err(|_| "invalid endpoint")?;
        let response = self
            .http
            .post(url)
            .json(&serde_json::json!({"token":invite_token}))
            .send()
            .await
            .map_err(|_| "network error joining room")?;
        let response: MemberResponse = read_json(response).await?;
        valid_room(&response.member_id)?;
        valid_token(&response.member_token)?;
        Ok(MemberCredential {
            member_id: response.member_id,
            token: Zeroizing::new(response.member_token),
        })
    }
}
