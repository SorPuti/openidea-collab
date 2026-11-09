/// Error returned when two versions of a file cannot be automatically merged.
#[derive(Debug)]
pub enum MergeError {
    Conflict,
}

/// Merge two diverged versions of a file against their common base.
#[allow(dead_code)]
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Result<String, MergeError> {
    diffy::merge(base, ours, theirs).map_err(|_| MergeError::Conflict)
}
