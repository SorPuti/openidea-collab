//! Shared control and binary transport contract. No document contents are stored by the relay.
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

pub const DEFAULT_ORIGIN: &str = "https://collab.synthosia.com";
pub const SUBPROTOCOL: &str = "openidea-collab.v1";
pub const VERSION: u8 = 1;
pub const MAX_FRAME: usize = 64 * 1024;
pub const MAX_TRANSFER: usize = 2 * 1024 * 1024;
pub const MAX_PARTS: u16 = 64;
pub const HEADER: usize = 61;
pub const MAX_PAYLOAD: usize = MAX_FRAME - HEADER;
pub const MAX_PRESENCE: usize = 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Editor,
    Viewer,
}
impl Role {
    pub fn can_write(self) -> bool {
        self != Self::Viewer
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSession {
    pub protocol: u8,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionCredential {
    pub session_id: Uuid,
    pub member_id: Uuid,
    pub credential: String,
    pub role: Role,
    pub expires_at: u64,
    pub websocket_url: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateInvite {
    pub role: Role,
    pub ttl_seconds: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    pub url: String,
    pub expires_at: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedeemInvite {
    pub invite_id: Uuid,
    pub secret: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterDocument {
    pub document_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub protocol: u8,
    pub max_members_per_session: usize,
    pub websocket_url: String,
    pub max_frame_bytes: usize,
    pub max_transfer_bytes: usize,
    pub content_retention: String,
}

/// URLs carry no credentials; fragments are only used for one-time invitations.
#[derive(Clone, Debug)]
pub struct Endpoint(Url);
impl Endpoint {
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        let url = Url::parse(input).map_err(|_| "invalid relay URL")?;
        let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !(url.scheme() == "https" || (url.scheme() == "http" && loopback)) {
            return Err("relay requires HTTPS outside loopback");
        }
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err("relay URL must contain only an origin");
        }
        Ok(Self(url))
    }
    pub fn origin(&self) -> String {
        self.0.origin().ascii_serialization()
    }
    pub fn api(&self, path: &str) -> String {
        format!("{}{path}", self.origin())
    }
    pub fn websocket(&self) -> String {
        let mut url = self.0.clone();
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .expect("HTTP and WS are special schemes");
        url.set_path("/v1/socket");
        url.to_string()
    }
    pub fn invitation(&self, id: Uuid, secret: &str) -> String {
        format!("{}/join/{id}#{secret}", self.origin())
    }
    pub fn parse_invitation(&self, input: &str) -> Result<RedeemInvite, &'static str> {
        let url = Url::parse(input).map_err(|_| "invalid invitation")?;
        if url.origin() != self.0.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
        {
            return Err("invitation belongs to another relay");
        }
        let id = url
            .path()
            .strip_prefix("/join/")
            .ok_or("invalid invitation path")?;
        let invite_id = Uuid::parse_str(id).map_err(|_| "invalid invitation ID")?;
        let secret = url.fragment().ok_or("invitation secret missing")?;
        if !credential_valid(secret) {
            return Err("invalid invitation secret");
        }
        Ok(RedeemInvite {
            invite_id,
            secret: secret.to_owned(),
        })
    }
}

pub fn credential_valid(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    StateVector = 1,
    Update = 2,
    Ack = 3,
    Presence = 4,
    Snapshot = 5,
}
impl Kind {
    fn decode(value: u8) -> Result<Self, &'static str> {
        match value {
            1 => Ok(Self::StateVector),
            2 => Ok(Self::Update),
            3 => Ok(Self::Ack),
            4 => Ok(Self::Presence),
            5 => Ok(Self::Snapshot),
            _ => Err("unknown frame kind"),
        }
    }
    pub fn writes(self) -> bool {
        matches!(self, Self::Update | Self::Snapshot)
    }
}

/// Inbound: OIC1, kind, document UUID, target UUID (nil=broadcast), message UUID,
/// part index, part count, total length (big-endian), payload. Outbound adds the
/// authenticated sender UUID before these bytes. Identity is never accepted from payload.
pub struct Frame<'a> {
    pub kind: Kind,
    pub document: Uuid,
    pub target: Uuid,
    pub message: Uuid,
    pub part: u16,
    pub parts: u16,
    pub total: u32,
    pub payload: &'a [u8],
}
impl<'a> Frame<'a> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < HEADER || bytes.len() > MAX_FRAME || &bytes[..4] != b"OIC1" {
            return Err("invalid frame size or version");
        }
        let uuid =
            |offset| Uuid::from_slice(&bytes[offset..offset + 16]).map_err(|_| "invalid UUID");
        let frame = Self {
            kind: Kind::decode(bytes[4])?,
            document: uuid(5)?,
            target: uuid(21)?,
            message: uuid(37)?,
            part: u16::from_be_bytes([bytes[53], bytes[54]]),
            parts: u16::from_be_bytes([bytes[55], bytes[56]]),
            total: u32::from_be_bytes(bytes[57..61].try_into().unwrap()),
            payload: &bytes[HEADER..],
        };
        frame.validate()?;
        Ok(frame)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.document.is_nil()
            || self.message.is_nil()
            || self.parts == 0
            || self.parts > MAX_PARTS
            || self.part >= self.parts
            || self.total == 0
            || self.total as usize > MAX_TRANSFER
            || self.payload.is_empty()
            || self.payload.len() > MAX_PAYLOAD
            || self.payload.len() > self.total as usize
            || (self.parts == 1 && self.payload.len() != self.total as usize)
            || self.parts as u32 > self.total
        {
            return Err("invalid transfer bounds");
        }
        if matches!(self.kind, Kind::Presence | Kind::Ack | Kind::StateVector) && self.parts != 1 {
            return Err("control frames cannot be chunked");
        }
        if self.kind == Kind::Presence && self.payload.len() > MAX_PRESENCE {
            return Err("presence too large");
        }
        if self.kind == Kind::Ack && (self.target.is_nil() || self.payload.len() != 16) {
            return Err("ACK requires a target and update UUID");
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        self.validate()?;
        let mut out = Vec::with_capacity(HEADER + self.payload.len());
        out.extend_from_slice(b"OIC1");
        out.push(self.kind as u8);
        for id in [self.document, self.target, self.message] {
            out.extend_from_slice(id.as_bytes());
        }
        out.extend_from_slice(&self.part.to_be_bytes());
        out.extend_from_slice(&self.parts.to_be_bytes());
        out.extend_from_slice(&self.total.to_be_bytes());
        out.extend_from_slice(self.payload);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invitations_are_bound_to_exact_origin_and_secret_is_not_in_request_path() {
        let endpoint = Endpoint::parse(DEFAULT_ORIGIN).unwrap();
        let id = Uuid::new_v4();
        let secret = "A".repeat(43);
        let url = endpoint.invitation(id, &secret);
        assert_eq!(endpoint.parse_invitation(&url).unwrap().invite_id, id);
        assert!(!Url::parse(&url).unwrap().path().contains(&secret));
        for hostile in [
            url.replace("collab.synthosia.com", "evil.example"),
            format!("{url}?token=x"),
            url.replace("https:", "http:"),
        ] {
            assert!(endpoint.parse_invitation(&hostile).is_err());
        }
    }
    #[test]
    fn endpoints_refuse_remote_http_credentials_and_paths() {
        for url in [
            "http://example.com",
            "https://u:p@example.com",
            "https://example.com/path",
            "https://example.com/?token=x",
        ] {
            assert!(Endpoint::parse(url).is_err());
        }
        assert_eq!(
            Endpoint::parse(DEFAULT_ORIGIN).unwrap().websocket(),
            "wss://collab.synthosia.com/v1/socket"
        );
        assert!(Endpoint::parse("http://127.0.0.1:4444").is_ok());
    }
    #[test]
    fn malformed_and_oversized_frames_are_refused() {
        let data = Frame {
            kind: Kind::Update,
            document: Uuid::new_v4(),
            target: Uuid::nil(),
            message: Uuid::new_v4(),
            part: 0,
            parts: 1,
            total: 3,
            payload: b"abc",
        }
        .encode()
        .unwrap();
        assert_eq!(Frame::decode(&data).unwrap().payload, b"abc");
        for length in 0..HEADER {
            assert!(Frame::decode(&data[..length]).is_err());
        }
        let mut wrong = data.clone();
        wrong[4] = 99;
        assert!(Frame::decode(&wrong).is_err());
        let mut wrong = data;
        wrong[55..57].copy_from_slice(&0u16.to_be_bytes());
        assert!(Frame::decode(&wrong).is_err());
        assert!(Frame::decode(&vec![0; MAX_FRAME + 1]).is_err());
    }
}
