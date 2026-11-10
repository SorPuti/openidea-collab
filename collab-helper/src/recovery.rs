//! Optional local plaintext recovery copy, containing document text only.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

pub struct Recovery {
    directory: PathBuf,
    path: PathBuf,
}

fn check_private(directory: &Path) -> io::Result<()> {
    let info = fs::symlink_metadata(directory)?;
    if !info.file_type().is_dir()
        || info.uid() != unsafe { libc::geteuid() }
        || info.mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recovery directory must be private and owned by the current user",
        ));
    }
    Ok(())
}

impl Recovery {
    /// The parent of `directory` must be trusted by the current user.
    pub fn new(directory: &Path, initial: &str) -> io::Result<Self> {
        check_private(directory)?;
        let recovery = Self {
            directory: directory.to_owned(),
            path: directory.join(format!("{}.txt", uuid::Uuid::new_v4())),
        };
        recovery.save(initial)?;
        Ok(recovery)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Sync a temporary file before atomically replacing the previous copy.
    pub fn save(&self, text: &str) -> io::Result<()> {
        check_private(&self.directory)?;
        let temp = self
            .directory
            .join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)?;
            fs::File::open(&self.directory)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

/// Create the application-specific leaf directory. Existing leaf directories
/// are never chmodded; a misconfigured or shared directory is rejected.
pub fn default_directory() -> io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(
            std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?,
        )
        .join(".local/state"),
    };
    if !base.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "state directory must be absolute",
        ));
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&base)?;
    let directory = base.join("openidea-collab-recovery");
    match fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err),
    }
    check_private(&directory)?;
    Ok(directory)
}
