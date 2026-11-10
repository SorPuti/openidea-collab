//! Experimental write-ahead session journal for the opt-in realtime API.
//!
//! Entries contain token hashes, never bearer tokens. Callers must validate and
//! replay every entry before enabling writes; an append is acknowledged only
//! after SQLite commits. It requires a private, trusted containing directory.
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{fs::OpenOptions, path::Path};

const MAX_ENTRY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
pub struct JournalLimits {
    pub normal_bytes: u64,
    pub revoke_reserve_bytes: u64,
}
impl Default for JournalLimits {
    fn default() -> Self {
        Self {
            normal_bytes: 64 * 1024 * 1024,
            revoke_reserve_bytes: 4 * 1024 * 1024,
        }
    }
}
#[derive(Debug)]
pub struct QuotaExceeded;
impl std::fmt::Display for QuotaExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("journal storage quota exceeded")
    }
}
impl std::error::Error for QuotaExceeded {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalRole {
    Viewer,
    Editor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Event {
    RoomCreated {
        room: String,
        owner_hash: [u8; 32],
    },
    InviteCreated {
        room: String,
        invite_hash: [u8; 32],
        role: JournalRole,
        expires_unix: u64,
    },
    MemberJoined {
        room: String,
        invite_hash: [u8; 32],
        member: String,
        member_hash: [u8; 32],
        role: JournalRole,
    },
    DocumentUpdated {
        room: String,
        document: String,
        revision: u64,
        update: Vec<u8>,
    },
    MemberRevoked {
        room: String,
        member: String,
    },
}

pub struct Journal {
    connection: Connection,
    limits: JournalLimits,
}

impl Journal {
    /// Open a private database. The containing directory must be trusted and
    /// inaccessible to other users; this is not a hostile-directory sandbox.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Self::open_with_limits(path, JournalLimits::default())
    }

    /// Limits count serialized journal payloads, not SQLite file overhead.
    pub fn open_with_limits(path: &Path, limits: JournalLimits) -> anyhow::Result<Self> {
        let maximum = limits
            .normal_bytes
            .checked_add(limits.revoke_reserve_bytes)
            .ok_or_else(|| anyhow::anyhow!("invalid journal limits"))?;
        anyhow::ensure!(maximum <= i64::MAX as u64, "invalid journal limits");
        use std::io::ErrorKind;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(file) => drop(file),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                let metadata = std::fs::symlink_metadata(path)?;
                anyhow::ensure!(metadata.is_file(), "journal must be a regular file");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    anyhow::ensure!(
                        metadata.permissions().mode() & 0o077 == 0,
                        "journal must not be accessible to group or others"
                    );
                }
            }
            Err(err) => return Err(err.into()),
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        anyhow::ensure!((0..=2).contains(&version), "unsupported journal version");
        if version == 0 {
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE entries (sequence INTEGER PRIMARY KEY, payload BLOB NOT NULL); PRAGMA user_version=1; COMMIT;")?;
        }
        if version <= 1 {
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE journal_usage (id INTEGER PRIMARY KEY CHECK(id=1), bytes INTEGER NOT NULL CHECK(bytes>=0)); INSERT INTO journal_usage(id, bytes) SELECT 1, COALESCE(SUM(length(payload)), 0) FROM entries; PRAGMA user_version=2; COMMIT;")?;
        }
        let (declared, actual): (i64, i64) = connection.query_row("SELECT (SELECT bytes FROM journal_usage WHERE id=1), (SELECT COALESCE(SUM(length(payload)), 0) FROM entries)", [], |row| Ok((row.get(0)?, row.get(1)?)))?;
        anyhow::ensure!(
            declared == actual && actual >= 0,
            "journal usage metadata mismatch"
        );
        Ok(Self { connection, limits })
    }

    /// Compare the expected last sequence and append within one transaction.
    pub fn append(&mut self, expected: u64, event: &Event) -> anyhow::Result<u64> {
        let payload = serde_json::to_vec(event)?;
        anyhow::ensure!(
            payload.len() <= MAX_ENTRY_BYTES,
            "journal entry exceeds limit"
        );
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let last: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM entries",
            [],
            |row| row.get(0),
        )?;
        anyhow::ensure!(u64::try_from(last)? == expected, "stale journal sequence");
        let next = last
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("journal sequence exhausted"))?;
        let usage: i64 = tx.query_row("SELECT bytes FROM journal_usage WHERE id=1", [], |row| {
            row.get(0)
        })?;
        let usage = u64::try_from(usage)?;
        let updated = usage
            .checked_add(payload.len() as u64)
            .ok_or(QuotaExceeded)?;
        let limit = if matches!(event, Event::MemberRevoked { .. }) {
            self.limits.normal_bytes + self.limits.revoke_reserve_bytes
        } else {
            self.limits.normal_bytes
        };
        if updated > limit {
            return Err(QuotaExceeded.into());
        }
        tx.execute(
            "INSERT INTO entries(sequence, payload) VALUES (?1, ?2)",
            params![next, payload],
        )?;
        tx.execute(
            "UPDATE journal_usage SET bytes=?1 WHERE id=1",
            [i64::try_from(updated)?],
        )?;
        tx.commit()?;
        Ok(u64::try_from(next)?)
    }

    /// Keep SQLite's exclusive lock for this connection's lifetime. A second
    /// server must never recover a stale snapshot and continue serving reads.
    pub fn claim_exclusive(&self) -> anyhow::Result<()> {
        self.connection
            .execute_batch("PRAGMA locking_mode=EXCLUSIVE; BEGIN EXCLUSIVE; COMMIT;")?;
        Ok(())
    }

    /// Reject malformed, unknown, oversized or discontinuous records on replay.
    pub fn read_all(&self) -> anyhow::Result<Vec<Event>> {
        let mut stmt = self
            .connection
            .prepare("SELECT sequence, length(payload), payload FROM entries ORDER BY sequence")?;
        let mut rows = stmt.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            let sequence: i64 = row.get(0)?;
            anyhow::ensure!(
                sequence == i64::try_from(result.len())? + 1,
                "journal sequence gap"
            );
            let length: i64 = row.get(1)?;
            anyhow::ensure!(
                length >= 0 && length <= MAX_ENTRY_BYTES as i64,
                "journal entry exceeds limit"
            );
            let payload: Vec<u8> = row.get(2)?;
            result.push(serde_json::from_slice(&payload)?);
        }
        Ok(result)
    }
}
