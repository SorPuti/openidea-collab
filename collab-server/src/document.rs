/// Error returned when two versions of a file cannot be automatically merged.
#[derive(Debug)]
pub enum MergeError {
    Conflict,
}

/// Attempt a 3-way text merge using the `diffy` crate.
///
/// Returns the merged text on success, or `Err(MergeError::Conflict)` when
/// the two sides make incompatible changes to the same region.
///
/// `diffy::merge` is used as the fast path.  It is conservative: changes to
/// adjacent lines fall inside the same unified-diff context window and are
/// reported as a conflict even when the edits do not actually overlap.
/// For that case a line-level fallback is applied, which only conflicts when
/// the *same* base line is changed by both sides to *different* values.
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Result<String, MergeError> {
    // Fast path — handles most real-world cases cleanly.
    if let Ok(merged) = diffy::merge(base, ours, theirs) {
        return Ok(merged);
    }

    // Fall back to line-level merge that is not affected by context windows.
    line_merge(base, ours, theirs)
}

/// Line-level 3-way merge.
///
/// A conflict is raised only when the same base line is modified by both
/// `ours` and `theirs` to different values.
///
/// Limitation: when either modified text has a different number of lines than
/// `base` (i.e. lines were inserted or deleted) the function returns
/// `Err(Conflict)` conservatively, because proper alignment requires a
/// full LCS computation not performed here.
fn line_merge(base: &str, ours: &str, theirs: &str) -> Result<String, MergeError> {
    let base_lines: Vec<&str> = base.split('\n').collect();
    let ours_lines: Vec<&str> = ours.split('\n').collect();
    let theirs_lines: Vec<&str> = theirs.split('\n').collect();

    if base_lines.len() != ours_lines.len() || base_lines.len() != theirs_lines.len() {
        return Err(MergeError::Conflict);
    }

    let mut result = Vec::with_capacity(base_lines.len());
    for i in 0..base_lines.len() {
        let b = base_lines[i];
        let o = ours_lines[i];
        let t = theirs_lines[i];

        if o == t {
            result.push(o); // both agree (unchanged, or both made the same edit)
        } else if o == b {
            result.push(t); // only theirs changed this line
        } else if t == b {
            result.push(o); // only ours changed this line
        } else {
            return Err(MergeError::Conflict); // both changed it to different values
        }
    }

    Ok(result.join("\n"))
}
