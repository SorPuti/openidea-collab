use super::*;

fn check_state_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("metadata path must be absolute".into());
    }
    for ancestor in path.ancestors() {
        if let Ok(meta) = std::fs::symlink_metadata(ancestor)
            && meta.is_symlink()
        {
            return Err("metadata path cannot contain symlinks".into());
        }
    }
    Ok(())
}
pub(super) fn load_metadata(path: &Path) -> Result<Metadata, String> {
    check_state_path(path)?;
    if let Ok(meta) = std::fs::metadata(path)
        && (!meta.is_file() || meta.len() > 1024 * 1024)
    {
        return Err("authorization metadata is not a bounded file".into());
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Metadata {
                version: VERSION,
                ..Default::default()
            });
        }
        Err(_) => return Err("cannot read authorization metadata".into()),
    };
    if bytes.len() > 1024 * 1024 {
        return Err("authorization metadata too large".into());
    }
    let metadata: Metadata =
        serde_json::from_slice(&bytes).map_err(|_| "invalid authorization metadata")?;
    if metadata.version != VERSION || metadata.sessions.len() > MAX_SESSIONS {
        return Err("unsupported authorization metadata".into());
    }
    for session in metadata.sessions.values() {
        if session.members.len() > MAX_MEMBERS
            || session.invitations.len() > MAX_INVITES
            || session.documents.len() > MAX_DOCUMENTS
        {
            return Err("metadata exceeds limits".into());
        }
    }
    Ok(metadata)
}
pub(super) fn save_metadata(path: &Path, metadata: &Metadata) -> Result<(), String> {
    use std::io::Write;
    check_state_path(path)?;
    let parent = path.parent().ok_or("metadata parent missing")?;
    // Configure a dedicated state directory; do not chmod arbitrary ancestors.
    if !parent.exists() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(parent)
            .map_err(|_| "cannot create private state directory")?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(parent)
            .map_err(|_| "state directory unavailable")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("state directory must be owner-only".into());
        }
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "cannot stage metadata")?;
    let bytes = serde_json::to_vec(metadata).map_err(|_| "cannot encode metadata")?;
    temporary
        .write_all(&bytes)
        .map_err(|_| "cannot write metadata")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "cannot sync metadata")?;
    temporary
        .persist(path)
        .map_err(|_| "cannot commit metadata")?;
    #[cfg(unix)]
    {
        std::fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| "cannot sync metadata directory")?;
    }
    Ok(())
}
