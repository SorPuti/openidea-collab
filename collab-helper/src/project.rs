//! Experimental local project snapshots. Not wired to network or UI.
//! The source directory must not be concurrently modified by untrusted users.
use flate2::{write::GzEncoder, Compression};
use ignore::{DirEntry, WalkBuilder};
use std::{
    fs::{self, OpenOptions},
    io,
    path::Path,
};

#[derive(Clone, Copy)]
pub struct Limits {
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_files: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 16 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
            max_files: 50_000,
        }
    }
}
#[derive(Default, Debug)]
pub struct Report {
    pub included: usize,
    pub skipped_large: usize,
    pub bytes: u64,
}

fn allowed(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    let lower = name.to_ascii_lowercase();
    if entry.file_type().is_some_and(|t| t.is_symlink()) {
        return false;
    }
    if entry.file_type().is_some_and(|t| t.is_dir()) {
        return !matches!(
            lower.as_str(),
            ".git"
                | ".hg"
                | ".svn"
                | "node_modules"
                | ".venv"
                | "venv"
                | "__pycache__"
                | ".tox"
                | ".mypy_cache"
                | ".pytest_cache"
                | "target"
                | "dist"
                | "build"
                | ".gradle"
                | ".idea"
                | ".terraform"
                | ".ssh"
                | ".aws"
                | ".gnupg"
                | ".kube"
        );
    }
    !(lower == ".env"
        || lower.starts_with(".env.")
        || lower == ".envrc"
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.ends_with(".jks")
        || matches!(
            lower.as_str(),
            "credentials.json" | ".npmrc" | ".pypirc" | ".netrc" | "id_rsa" | "id_ed25519"
        ))
}

/// Create a new archive outside the project, never overwriting an existing file.
/// Large individual files are skipped; aggregate limits abort the whole snapshot.
pub fn create_snapshot(project: &Path, output: &Path, limits: Limits) -> io::Result<Report> {
    let root = fs::canonicalize(project)?;
    if !root.is_dir() {
        return Err(io::Error::other("project must be a directory"));
    }
    let parent = fs::canonicalize(
        output
            .parent()
            .ok_or_else(|| io::Error::other("output requires parent"))?,
    )?;
    if parent.starts_with(&root) {
        return Err(io::Error::other("archive must be outside project"));
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(output)?;
    let result = (|| {
        let encoder = GzEncoder::new(file, Compression::fast());
        let mut tar = tar::Builder::new(encoder);
        let mut report = Report::default();
        let mut walker = WalkBuilder::new(&root);
        walker
            .hidden(false)
            .follow_links(false)
            .require_git(false)
            .git_global(false)
            .git_exclude(false)
            .filter_entry(allowed);
        for entry in walker.build() {
            let entry = entry.map_err(io::Error::other)?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let before = fs::symlink_metadata(path)?;
            if !before.is_file() {
                return Err(io::Error::other("source changed during snapshot"));
            }
            if before.len() > limits.max_file_bytes {
                report.skipped_large += 1;
                continue;
            }
            let total = report
                .bytes
                .checked_add(before.len())
                .ok_or_else(|| io::Error::other("project size overflow"))?;
            if total > limits.max_total_bytes || report.included >= limits.max_files {
                return Err(io::Error::other("project exceeds snapshot limits"));
            }
            let mut read = OpenOptions::new();
            read.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                read.custom_flags(libc::O_NOFOLLOW);
            }
            let mut input = read.open(path)?;
            let opened = input.metadata()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if before.dev() != opened.dev() || before.ino() != opened.ino() {
                    return Err(io::Error::other("source changed during snapshot"));
                }
            }
            if !opened.is_file()
                || opened.len() != before.len()
                || opened.modified()? != before.modified()?
            {
                return Err(io::Error::other("source changed during snapshot"));
            }
            let mut mode = 0o644;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if opened.permissions().mode() & 0o111 != 0 {
                    mode = 0o755;
                }
            }
            let mut header = tar::Header::new_gnu();
            header.set_size(opened.len());
            header.set_mode(mode);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            let rel = path.strip_prefix(&root).map_err(io::Error::other)?;
            tar.append_data(&mut header, rel, &mut input)?;
            let after = input.metadata()?;
            if after.len() != opened.len() || after.modified()? != opened.modified()? {
                return Err(io::Error::other("source changed during snapshot"));
            }
            report.bytes = total;
            report.included += 1;
        }
        let encoder = tar.into_inner()?;
        encoder.finish()?.sync_all()?;
        Ok(report)
    })();
    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}
