//! HTTPS session, invitation and document-authorization control plane.
use openidea_collab_protocol::{
    CreateInvite, CreateSession, DEFAULT_ORIGIN, Endpoint, Invite, RedeemInvite, Role,
    SessionCredential, VERSION, credential_valid,
};
use openidea_plugin::{Error, HttpRequest, host};
use serde::{Serialize, de::DeserializeOwned};

pub const SECRET: &str = "member-credential";
pub const ADMISSION_SECRET: &str = "relay-admission";
pub fn relay_origin() -> Result<String, Error> {
    let origin = host::setting_string("relayOrigin").unwrap_or_else(|| DEFAULT_ORIGIN.into());
    Ok(Endpoint::parse(&origin)
        .map_err(|e| Error::Invalid(e.into()))?
        .origin())
}
fn request(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<Vec<u8>>,
    authenticated: bool,
    admission: bool,
) -> Result<Vec<u8>, Error> {
    let endpoint = Endpoint::parse(origin).map_err(|e| Error::Invalid(e.into()))?;
    let mut req = HttpRequest {
        method: method.to_owned(),
        url: endpoint.api(path),
        body,
        timeout_ms: Some(4000),
        ..Default::default()
    }
    .header("content-type", "application/json")
    .header("accept", "application/json");
    if authenticated {
        req = req.header("authorization", "Bearer {{secret:member-credential}}");
    }
    if admission && host::secret_is_set(ADMISSION_SECRET) {
        req = req.header("x-collab-admission", "{{secret:relay-admission}}");
    }
    let response = host::fetch(&req).map_err(|_| {
        Error::Failed(
            "Could not reach the collaboration relay. Check the relay URL and its availability, then try again."
                .into(),
        )
    })?;
    if method == "DELETE" && response.status == 401 {
        return Ok(Vec::new());
    }
    if !matches!(response.status, 200 | 201 | 204) {
        // Never surface untrusted response bodies, invite URLs or credentials.
        return Err(Error::Failed(format!(
            "Collaboration relay returned HTTP {}",
            response.status
        )));
    }
    if response.body.len() > 8192 {
        return Err(Error::Invalid("Relay response exceeds limit".into()));
    }
    Ok(response.body)
}
fn post<T: DeserializeOwned>(
    origin: &str,
    path: &str,
    body: &impl Serialize,
    auth: bool,
    admission: bool,
) -> Result<T, Error> {
    let body = serde_json::to_vec(body)
        .map_err(|_| Error::Invalid("Could not encode relay request".into()))?;
    serde_json::from_slice(&request(origin, "POST", path, Some(body), auth, admission)?)
        .map_err(|_| Error::Invalid("Invalid relay response".into()))
}
fn validate(origin: &str, credential: SessionCredential) -> Result<SessionCredential, Error> {
    let endpoint = Endpoint::parse(origin).map_err(|e| Error::Invalid(e.into()))?;
    if credential.session_id.is_nil()
        || credential.member_id.is_nil()
        || !credential_valid(&credential.credential)
        || credential.websocket_url != endpoint.websocket()
    {
        return Err(Error::Invalid(
            "Relay returned an invalid session or destination".into(),
        ));
    }
    Ok(credential)
}
pub fn create(origin: &str) -> Result<SessionCredential, Error> {
    validate(
        origin,
        post(
            origin,
            "/v1/sessions",
            &CreateSession { protocol: VERSION },
            false,
            true,
        )?,
    )
}
pub fn join(origin: &str, invitation: &str) -> Result<SessionCredential, Error> {
    let endpoint = Endpoint::parse(origin).map_err(|e| Error::Invalid(e.into()))?;
    let redeem: RedeemInvite = endpoint
        .parse_invitation(invitation)
        .map_err(|e| Error::Invalid(e.into()))?;
    validate(
        origin,
        post(origin, "/v1/invitations/redeem", &redeem, false, false)?,
    )
}
pub fn invite(origin: &str, session: &str) -> Result<Invite, Error> {
    let invitation: Invite = post(
        origin,
        &format!("/v1/sessions/{session}/invitations"),
        &CreateInvite {
            role: Role::Editor,
            ttl_seconds: 300,
        },
        true,
        false,
    )?;
    Endpoint::parse(origin)
        .map_err(|e| Error::Invalid(e.into()))?
        .parse_invitation(&invitation.url)
        .map_err(|_| Error::Invalid("Invalid invitation destination".into()))?;
    Ok(invitation)
}
pub fn register_document(
    origin: &str,
    session: &str,
    document_id: uuid::Uuid,
) -> Result<(), Error> {
    request(
        origin,
        "POST",
        &format!("/v1/sessions/{session}/documents"),
        Some(
            serde_json::to_vec(&openidea_collab_protocol::RegisterDocument { document_id })
                .map_err(|_| Error::Invalid("Could not encode document registration".into()))?,
        ),
        true,
        false,
    )
    .map(drop)
}
pub fn leave(origin: &str, session: &str, member: &str) -> Result<(), Error> {
    request(
        origin,
        "DELETE",
        &format!("/v1/sessions/{session}/members/{member}"),
        None,
        true,
        false,
    )
    .map(drop)
}
