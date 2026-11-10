use openidea_collab_core::policy::project_identity;
use openidea_collab_protocol::{DEFAULT_ORIGIN, Role, SessionCredential};
use openidea_plugin::{Error, Invocation, host};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const KEY: &str = "session_v1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub schema: u8,
    pub project: [u8; 32],
    pub relay: String,
    pub session_id: Uuid,
    pub member_id: Uuid,
    pub role: Role,
    pub expires_at: u64,
}
fn root(ctx: &Invocation) -> Result<String, Error> {
    ctx.project()
        .ok_or_else(|| Error::Invalid("Open a project first".into()))?
        .root()
}
pub fn load() -> Result<Option<Session>, Error> {
    let Some(value) = host::storage::get(KEY)? else {
        return Ok(None);
    };
    let session: Session = serde_json::from_str(&value)
        .map_err(|_| Error::Invalid("Invalid stored session; leave it before continuing".into()))?;
    if session.schema != 1
        || session.relay != DEFAULT_ORIGIN
        || session.session_id.is_nil()
        || session.member_id.is_nil()
    {
        return Err(Error::Invalid("Stored session is incompatible".into()));
    }
    Ok(Some(session))
}
pub fn for_project(ctx: &Invocation) -> Result<Option<Session>, Error> {
    let current = project_identity(&root(ctx)?);
    match load()? {
        Some(session) if session.project != current => Err(Error::Invalid("Another project has a prepared session. Leave it explicitly before preparing this project.".into())),
        session => Ok(session),
    }
}
pub fn save(ctx: &Invocation, credential: SessionCredential) -> Result<(), Error> {
    let session = Session {
        schema: 1,
        project: project_identity(&root(ctx)?),
        relay: DEFAULT_ORIGIN.into(),
        session_id: credential.session_id,
        member_id: credential.member_id,
        role: credential.role,
        expires_at: credential.expires_at,
    };
    let json = serde_json::to_string(&session)
        .map_err(|_| Error::Failed("Could not encode session".into()))?;
    host::secret_set(crate::client::SECRET, &credential.credential)?;
    if let Err(error) = host::storage::set(KEY, &json) {
        let _ = host::secret_delete(crate::client::SECRET);
        return Err(error);
    }
    Ok(())
}
pub fn forget() -> Result<(), Error> {
    host::secret_delete(crate::client::SECRET)?;
    host::storage::delete(KEY)
}
