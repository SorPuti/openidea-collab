//! HTTP client wrappers over `host::fetch` for the collab-server REST API.

use crate::room::MemberInfo;
use openidea_plugin::{HttpRequest, host};

/// Long-poll response from GET /rooms/{id}/events?cursor=N.
pub struct PollResult {
    pub events: Vec<serde_json::Value>,
    pub next_cursor: u64,
}

/// Thin HTTP client; all calls go through `openidea_plugin::host::fetch`.
pub struct CollabClient {
    pub server_url: String,
}

/// Convert `openidea_plugin::Error` to `anyhow::Error` (the former is not
/// `Send + Sync`, so the `?` / `From` path is not available).
fn oe(e: openidea_plugin::Error) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

impl CollabClient {
    pub fn new(server_url: String) -> Self {
        Self { server_url }
    }

    /// POST /rooms → { room_id }
    pub fn create_room(&self) -> Result<String, anyhow::Error> {
        let body = b"{}".to_vec();
        let mut req = HttpRequest::post(&format!("{}/rooms", self.server_url), body)
            .header("content-type", "application/json")
            .header("accept", "application/json");
        req.timeout_ms = Some(5000);
        let resp = host::fetch(&req).map_err(oe)?;
        if resp.status == 200 || resp.status == 201 {
            let json: serde_json::Value = serde_json::from_slice(&resp.body)?;
            let room_id = json["room_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing room_id in response"))?
                .to_owned();
            Ok(room_id)
        } else {
            Err(anyhow::anyhow!(
                "create_room: server returned {}",
                resp.status
            ))
        }
    }

    /// POST /rooms/{id}/join  body: { name }  → { user_id }
    pub fn join_room(&self, room_id: &str, name: &str) -> Result<String, anyhow::Error> {
        let body = serde_json::to_vec(&serde_json::json!({ "name": name }))?;
        let mut req = HttpRequest::post(&format!("{}/rooms/{room_id}/join", self.server_url), body)
            .header("content-type", "application/json")
            .header("accept", "application/json");
        req.timeout_ms = Some(5000);
        let resp = host::fetch(&req).map_err(oe)?;
        if resp.status == 200 {
            let json: serde_json::Value = serde_json::from_slice(&resp.body)?;
            let user_id = json["user_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing user_id in response"))?
                .to_owned();
            Ok(user_id)
        } else if resp.status == 404 {
            Err(anyhow::anyhow!("room {room_id} not found"))
        } else {
            Err(anyhow::anyhow!(
                "join_room: server returned {}",
                resp.status
            ))
        }
    }

    /// DELETE /rooms/{id}/members/{user_id} → 204
    pub fn leave_room(&self, room_id: &str, user_id: &str) -> Result<(), anyhow::Error> {
        let url = format!("{}/rooms/{room_id}/members/{user_id}", self.server_url);
        let mut req = HttpRequest {
            method: "DELETE".into(),
            url,
            ..HttpRequest::default()
        };
        req.timeout_ms = Some(5000);
        let resp = host::fetch(&req).map_err(oe)?;
        if resp.status == 204 || resp.status == 200 || resp.status == 404 {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "leave_room: server returned {}",
                resp.status
            ))
        }
    }

    /// POST /rooms/{id}/files/{path}  body: { user_id, version, text }  → { version }
    pub fn post_file(
        &self,
        room_id: &str,
        user_id: &str,
        path: &str,
        version: u64,
        text: &str,
    ) -> Result<u64, anyhow::Error> {
        let encoded_path = path.replace('/', "%2F");
        let url = format!("{}/rooms/{room_id}/files/{encoded_path}", self.server_url);
        let payload = serde_json::to_vec(
            &serde_json::json!({ "user_id": user_id, "version": version, "text": text }),
        )?;
        let mut req = HttpRequest::post(&url, payload)
            .header("content-type", "application/json")
            .header("accept", "application/json");
        req.timeout_ms = Some(5000);
        let resp = host::fetch(&req).map_err(oe)?;
        if resp.status == 200 || resp.status == 201 {
            let json: serde_json::Value = serde_json::from_slice(&resp.body)?;
            let new_version = json["version"].as_u64().unwrap_or(0);
            Ok(new_version)
        } else if resp.status == 409 {
            // Server rejected the update due to an unresolvable conflict.
            Err(anyhow::anyhow!(
                "conflict: server rejected save, check for conflicts"
            ))
        } else {
            Err(anyhow::anyhow!(
                "post_file: server returned {}",
                resp.status
            ))
        }
    }

    /// GET /rooms/{id}/events?cursor=N  (long-poll, up to 8 s)
    pub fn poll_events(&self, room_id: &str, cursor: u64) -> Result<PollResult, anyhow::Error> {
        let url = format!("{}/rooms/{room_id}/events?cursor={cursor}", self.server_url);
        let mut req = HttpRequest::get(&url).header("accept", "application/json");
        req.timeout_ms = Some(8000);
        let resp = host::fetch(&req).map_err(oe)?;
        if resp.status == 200 {
            let json: serde_json::Value = serde_json::from_slice(&resp.body)?;
            let events = json["events"].as_array().cloned().unwrap_or_default();
            let next_cursor = json["next_cursor"].as_u64().unwrap_or(cursor);
            Ok(PollResult {
                events,
                next_cursor,
            })
        } else {
            Err(anyhow::anyhow!(
                "poll_events: server returned {}",
                resp.status
            ))
        }
    }

    /// GET /rooms/{id}/members → [{ user_id, name, current_file? }]
    pub fn get_members(&self, room_id: &str) -> Result<Vec<MemberInfo>, anyhow::Error> {
        let url = format!("{}/rooms/{room_id}/members", self.server_url);
        let mut req = HttpRequest::get(&url).header("accept", "application/json");
        req.timeout_ms = Some(5000);
        let resp = host::fetch(&req).map_err(oe)?;
        if resp.status == 200 {
            let members: Vec<MemberInfo> = serde_json::from_slice(&resp.body)?;
            Ok(members)
        } else {
            Err(anyhow::anyhow!(
                "get_members: server returned {}",
                resp.status
            ))
        }
    }

    /// GET /rooms/{id}/files/{path} → { version, text, user_id }
    pub fn get_file(&self, room_id: &str, path: &str) -> Result<(u64, String), anyhow::Error> {
        let encoded_path = path.replace('/', "%2F");
        let url = format!("{}/rooms/{room_id}/files/{encoded_path}", self.server_url);
        let mut req = HttpRequest::get(&url).header("accept", "application/json");
        req.timeout_ms = Some(5000);
        let resp = host::fetch(&req).map_err(oe)?;
        if resp.status == 200 {
            let json: serde_json::Value = serde_json::from_slice(&resp.body)?;
            let version = json["version"].as_u64().unwrap_or(0);
            let text = json["text"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing text in get_file response"))?
                .to_owned();
            Ok((version, text))
        } else if resp.status == 404 {
            Err(anyhow::anyhow!("file {path} not found in room {room_id}"))
        } else {
            Err(anyhow::anyhow!("get_file: server returned {}", resp.status))
        }
    }

    /// POST /rooms/{id}/members/{user_id}/presence  body: { current_file }
    pub fn update_presence(
        &self,
        room_id: &str,
        user_id: &str,
        current_file: Option<&str>,
    ) -> Result<(), anyhow::Error> {
        let url = format!(
            "{}/rooms/{room_id}/members/{user_id}/presence",
            self.server_url
        );
        let payload = serde_json::to_vec(&serde_json::json!({ "current_file": current_file }))?;
        let mut req = HttpRequest::post(&url, payload).header("content-type", "application/json");
        req.timeout_ms = Some(3000);
        // Best-effort: ignore errors (endpoint may not be implemented yet)
        let _ = host::fetch(&req);
        Ok(())
    }
}
