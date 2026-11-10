use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Local identity, never transmitted to the relay. The host must resolve the root.
pub fn project_identity(canonical_root: &str) -> [u8; 32] {
    Sha256::digest(canonical_root.as_bytes()).into()
}

/// This lexical filter does not prove no-follow filesystem access. Applying
/// remote files still requires a host adapter with no-follow guarantees.
pub fn safe_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.len() > 1024
        || path.contains(['\\', ':', '\0'])
        || path.starts_with('/')
    {
        return false;
    }
    path.split('/').all(|part| {
        let lower = part.to_ascii_lowercase();
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.chars().any(char::is_control)
            && !part.ends_with(['.', ' '])
            && !lower.starts_with(".env")
            && !matches!(
                lower.as_str(),
                ".git"
                    | ".hg"
                    | ".svn"
                    | ".openidea"
                    | ".idea"
                    | ".vscode"
                    | ".github"
                    | ".collabignore"
                    | ".gitignore"
                    | "node_modules"
                    | ".venv"
                    | "target"
                    | "dist"
                    | "build"
                    | "id_rsa"
                    | "id_ed25519"
                    | "credentials"
            )
            && ![".pem", ".key", ".p12", ".pfx"]
                .iter()
                .any(|extension| lower.ends_with(extension))
            && !matches!(
                lower.split('.').next().unwrap_or(""),
                "con"
                    | "prn"
                    | "aux"
                    | "nul"
                    | "com1"
                    | "com2"
                    | "com3"
                    | "com4"
                    | "com5"
                    | "com6"
                    | "com7"
                    | "com8"
                    | "com9"
                    | "lpt1"
                    | "lpt2"
                    | "lpt3"
                    | "lpt4"
                    | "lpt5"
                    | "lpt6"
                    | "lpt7"
                    | "lpt8"
                    | "lpt9"
            )
    })
}

#[derive(Clone)]
pub struct SharePolicy {
    project: [u8; 32],
    files: BTreeSet<String>,
}
impl SharePolicy {
    pub fn new(
        canonical_root: &str,
        files: impl IntoIterator<Item = String>,
    ) -> Result<Self, &'static str> {
        let mut accepted = BTreeSet::new();
        for file in files {
            if !safe_relative_path(&file) || accepted.len() >= 64 {
                return Err("unsafe or excessive sharing scope");
            }
            if accepted
                .iter()
                .any(|p: &String| p.eq_ignore_ascii_case(&file))
            {
                return Err("ambiguous file names");
            }
            accepted.insert(file);
        }
        if accepted.is_empty() {
            return Err("explicit sharing scope required");
        }
        Ok(Self {
            project: project_identity(canonical_root),
            files: accepted,
        })
    }
    pub fn permits(&self, current_canonical_root: &str, path: &str) -> bool {
        self.project == project_identity(current_canonical_root)
            && safe_relative_path(path)
            && self.files.contains(path)
    }
}

#[derive(Default)]
pub struct HostCapabilities {
    pub websocket_events: bool,
    pub document_events: bool,
    pub versioned_remote_edits: bool,
    pub no_follow_files: bool,
}
impl HostCapabilities {
    pub fn realtime_supported(&self) -> bool {
        self.websocket_events && self.document_events && self.versioned_remote_edits
    }
    pub fn project_transfer_supported(&self) -> bool {
        self.realtime_supported() && self.no_follow_files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consent_does_not_follow_a_different_project() {
        let scope = SharePolicy::new("/project/a", ["src/main.rs".into()]).unwrap();
        assert!(scope.permits("/project/a", "src/main.rs"));
        assert!(!scope.permits("/project/b", "src/main.rs"));
        assert!(!scope.permits("/project/a", "src/other.rs"));
    }
    #[test]
    fn control_files_secrets_and_cross_platform_paths_are_denied() {
        for path in [
            "../x",
            "/x",
            "src/../../x",
            "src/.git/config",
            ".git/hooks/pre-commit",
            ".openidea/a",
            ".env",
            "secret.KEY",
            "C:/x",
            "a\\b",
            "src/CON.txt",
            "src/x.",
            "node_modules/pkg/a",
        ] {
            assert!(!safe_relative_path(path), "{path}");
        }
        assert!(safe_relative_path("src/hello.rs"));
        assert!(!HostCapabilities::default().realtime_supported());
    }
}
