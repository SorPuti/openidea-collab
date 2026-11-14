//! Three-way decisions for project files. No filesystem or network side effects.
//!
//! A persisted baseline distinguishes a new peer (unknown) from a file known
//! to have been absent. Callers must only update it after durable application
//! and acknowledgement on both peers.
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileVersion {
    pub size: u64,
    pub sha256: [u8; 32],
}
impl FileVersion {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            size: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Baseline {
    Unknown,
    Known(Option<FileVersion>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Unchanged,
    Send,
    Receive,
    DeleteLocal,
    DeleteRemote,
    Conflict,
}

/// Compare current versions relative to the last mutually acknowledged state.
/// A conflict must preserve both sides and never advance the baseline.
pub fn reconcile(
    base: Baseline,
    local: Option<FileVersion>,
    remote: Option<FileVersion>,
) -> Decision {
    if local == remote {
        return Decision::Unchanged;
    }
    match base {
        Baseline::Unknown => match (local, remote) {
            (Some(_), None) => Decision::Send,
            (None, Some(_)) => Decision::Receive,
            _ => Decision::Conflict,
        },
        Baseline::Known(base) if local == base => match remote {
            Some(_) => Decision::Receive,
            None => Decision::DeleteLocal,
        },
        Baseline::Known(base) if remote == base => match local {
            Some(_) => Decision::Send,
            None => Decision::DeleteRemote,
        },
        Baseline::Known(_) => Decision::Conflict,
    }
}

/// An inventory of regular files; the host adapter must reject links and
/// prove the inventory is complete before treating a missing path as deleted.
#[derive(Debug, PartialEq, Eq)]
pub struct Manifest {
    files: std::collections::BTreeMap<String, FileVersion>,
}
impl Manifest {
    pub const MAX_FILES: usize = 100_000;

    pub fn files(&self) -> &std::collections::BTreeMap<String, FileVersion> {
        &self.files
    }

    pub fn new(
        entries: impl IntoIterator<Item = (String, FileVersion)>,
    ) -> Result<Self, &'static str> {
        let mut files = std::collections::BTreeMap::new();
        let mut folded = std::collections::BTreeSet::new();
        for (path, version) in entries {
            if files.len() >= Self::MAX_FILES || !crate::policy::safe_relative_path(&path) {
                return Err("unsafe or excessive inventory");
            }
            if !folded.insert(path.to_ascii_lowercase()) || files.insert(path, version).is_some() {
                return Err("duplicate or ambiguous file path");
            }
        }
        for path in &folded {
            for (index, byte) in path.bytes().enumerate() {
                if byte == b'/' && folded.contains(&path[..index]) {
                    return Err("a file path cannot contain another file");
                }
            }
        }
        Ok(Self { files })
    }
}

/// Plan only after both peers have complete inventories. An incomplete scan
/// cannot establish absence and must never produce delete operations.
pub fn plan(
    base: Option<&Manifest>,
    local: &Manifest,
    remote: &Manifest,
    local_complete: bool,
    remote_complete: bool,
) -> Result<std::collections::BTreeMap<String, Decision>, &'static str> {
    if !local_complete || !remote_complete {
        return Err("incomplete inventory");
    }
    let mut paths = std::collections::BTreeSet::new();
    for manifest in [base, Some(local), Some(remote)].into_iter().flatten() {
        paths.extend(manifest.files.keys().cloned());
    }
    if paths.len() > Manifest::MAX_FILES {
        return Err("inventory union exceeds limit");
    }
    let placeholder = FileVersion {
        size: 0,
        sha256: [0; 32],
    };
    Manifest::new(paths.iter().cloned().map(|path| (path, placeholder)))?;
    Ok(paths
        .into_iter()
        .map(|path| {
            let baseline = base.map_or(Baseline::Unknown, |m| {
                Baseline::Known(m.files.get(&path).copied())
            });
            let decision = reconcile(
                baseline,
                local.files.get(&path).copied(),
                remote.files.get(&path).copied(),
            );
            (path, decision)
        })
        .collect())
}

/// Validates a sequential stream while the caller stages bytes outside the
/// destination. The caller must only commit the staged file after `finish`
/// and a fresh conflict/safe-path check succeed.
pub struct ChunkVerifier {
    expected: FileVersion,
    received: u64,
    hash: Sha256,
}
impl ChunkVerifier {
    pub fn new(expected: FileVersion, max_file_bytes: u64) -> Result<Self, &'static str> {
        if expected.size > max_file_bytes {
            return Err("file exceeds configured limit");
        }
        Ok(Self {
            expected,
            received: 0,
            hash: Sha256::new(),
        })
    }

    pub fn push(&mut self, offset: u64, bytes: &[u8]) -> Result<(), &'static str> {
        if offset != self.received
            || bytes.is_empty()
            || bytes.len() > openidea_collab_protocol::MAX_PAYLOAD
        {
            return Err("invalid file chunk or offset");
        }
        let next = self
            .received
            .checked_add(bytes.len() as u64)
            .ok_or("file size overflow")?;
        if next > self.expected.size {
            return Err("chunk exceeds declared file size");
        }
        self.hash.update(bytes);
        self.received = next;
        Ok(())
    }

    pub fn finish(self) -> Result<(), &'static str> {
        let hash: [u8; 32] = self.hash.finalize().into();
        if self.received != self.expected.size || hash != self.expected.sha256 {
            return Err("incomplete or corrupt file");
        }
        Ok(())
    }
}

/// A complete, project-scoped inventory and version-conditional file access.
/// Implementations must reject links/reparse points and paths outside the
/// approved root without check/use races. A failed or truncated scan is an
/// error, never a complete inventory. Reads must return exactly the named
/// version. These are host obligations, not supplied by the reconciliation code.
pub trait ProjectStore {
    fn inventory(&mut self) -> std::io::Result<Manifest>;
    fn open_version(
        &mut self,
        path: &str,
        version: FileVersion,
    ) -> std::io::Result<Box<dyn std::io::Read>>;
    fn stage(
        &mut self,
        path: &str,
        expected: Option<FileVersion>,
        incoming: FileVersion,
    ) -> std::io::Result<Box<dyn StagedFile>>;
    fn remove_if_version(&mut self, path: &str, expected: FileVersion) -> std::io::Result<()>;
}

/// A private, bounded staging destination. Commit must atomically compare
/// the current destination with the expected version supplied at creation,
/// reject dirty open editors, and atomically replace only on a match. Dropping
/// an uncommitted stage must discard it. Adapters must enforce these rules.
pub trait StagedFile: std::io::Write {
    fn commit(self: Box<Self>) -> std::io::Result<()>;
}

pub struct SyncResult {
    /// Only returned when both post-sync inventories match. The caller must
    /// persist and acknowledge it with the peer before using it as a baseline.
    pub baseline: Option<Manifest>,
    /// Conflicting paths are not modified. No baseline is returned while any
    /// conflict remains, even if independent paths were synchronized.
    pub conflicts: Vec<String>,
}

/// Reconcile two complete stores. A failed transfer or conditional write stops
/// the round; retaining the previous baseline makes a retry safe. This engine
/// does not provide transport, host adapters or distributed acknowledgement.
pub fn synchronize(
    local: &mut dyn ProjectStore,
    remote: &mut dyn ProjectStore,
    base: Option<&Manifest>,
    max_file_bytes: u64,
) -> std::io::Result<SyncResult> {
    let local_manifest = local.inventory()?;
    let remote_manifest = remote.inventory()?;
    if local_manifest
        .files
        .values()
        .chain(remote_manifest.files.values())
        .any(|v| v.size > max_file_bytes)
    {
        return Err(std::io::Error::other("file exceeds configured limit"));
    }
    let decisions =
        plan(base, &local_manifest, &remote_manifest, true, true).map_err(std::io::Error::other)?;
    let mut conflicts = Vec::new();
    for (path, decision) in decisions {
        let left = local_manifest.files.get(&path).copied();
        let right = remote_manifest.files.get(&path).copied();
        match decision {
            Decision::Unchanged => {}
            Decision::Conflict => conflicts.push(path),
            Decision::Send => transfer_file(
                local,
                remote,
                &path,
                left.expect("send requires local"),
                right,
                max_file_bytes,
            )?,
            Decision::Receive => transfer_file(
                remote,
                local,
                &path,
                right.expect("receive requires remote"),
                left,
                max_file_bytes,
            )?,
            Decision::DeleteLocal => {
                local.remove_if_version(&path, left.expect("delete requires local"))?
            }
            Decision::DeleteRemote => {
                remote.remove_if_version(&path, right.expect("delete requires remote"))?
            }
        }
    }
    if !conflicts.is_empty() {
        return Ok(SyncResult {
            baseline: None,
            conflicts,
        });
    }
    let left_after = local.inventory()?;
    let right_after = remote.inventory()?;
    if left_after != right_after {
        return Err(std::io::Error::other(
            "project changed during synchronization",
        ));
    }
    Ok(SyncResult {
        baseline: Some(left_after),
        conflicts,
    })
}

fn transfer_file(
    source: &mut dyn ProjectStore,
    destination: &mut dyn ProjectStore,
    path: &str,
    incoming: FileVersion,
    expected: Option<FileVersion>,
    max_file_bytes: u64,
) -> std::io::Result<()> {
    use std::io::Write;
    let mut reader = source.open_version(path, incoming)?;
    let mut staged = destination.stage(path, expected, incoming)?;
    let mut verifier =
        ChunkVerifier::new(incoming, max_file_bytes).map_err(std::io::Error::other)?;
    let mut buffer = [0u8; 32 * 1024];
    let mut offset = 0u64;
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        verifier
            .push(offset, &buffer[..n])
            .map_err(std::io::Error::other)?;
        staged.write_all(&buffer[..n])?;
        offset += n as u64;
    }
    verifier.finish().map_err(std::io::Error::other)?;
    staged.flush()?;
    staged.commit()
}
