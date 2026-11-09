/// Error returned when two versions of a file cannot be automatically merged.
#[derive(Debug)]
pub enum MergeError {
    Conflict,
}

/// Attempt a 3-way text merge (diamond-types semantic).
///
/// Trivial cases are resolved immediately:
/// - both sides identical → return either one unchanged
/// - one side unchanged from base → accept the other
///
/// When both sides diverge from base the function returns `Err(MergeError::Conflict)`.
/// A future implementation will use diamond-types CRDT ops for a richer merge.
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Result<String, MergeError> {
    if ours == theirs {
        return Ok(ours.to_string());
    }
    if ours == base {
        return Ok(theirs.to_string());
    }
    if theirs == base {
        return Ok(ours.to_string());
    }
    // Both sides changed from base — cannot safely auto-merge without edit history.
    Err(MergeError::Conflict)
}
