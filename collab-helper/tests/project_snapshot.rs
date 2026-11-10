use openidea_collab_helper::project::{create_snapshot, Limits};
use std::{fs, io::Read};

fn temp_root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("collab-project-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    root
}

#[test]
fn snapshot_excludes_dependencies_secrets_symlinks_and_gitignored_files() {
    let root = temp_root();
    let project = root.join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(project.join("node_modules/package")).unwrap();
    fs::create_dir_all(project.join(".venv/bin")).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(project.join("node_modules/package/x"), "large dependency").unwrap();
    fs::write(project.join(".venv/bin/x"), "virtual environment").unwrap();
    fs::write(project.join(".git/config"), "private history").unwrap();
    fs::write(project.join(".env"), "SECRET=private").unwrap();
    fs::write(project.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(project.join("ignored.txt"), "ignored").unwrap();
    fs::write(project.join("src/run.sh"), "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            project.join("src/run.sh"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(project.join("src/main.rs"), project.join("link")).unwrap();
    let archive = root.join("snapshot.tar.gz");
    let report = create_snapshot(&project, &archive, Limits::default()).unwrap();
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(
        fs::File::open(&archive).unwrap(),
    ));
    let mut entries = Vec::new();
    for item in tar.entries().unwrap() {
        let mut item = item.unwrap();
        let name = item.path().unwrap().to_string_lossy().to_string();
        if name == "src/run.sh" && cfg!(unix) {
            assert_eq!(item.header().mode().unwrap() & 0o111, 0o111);
        }
        let mut text = String::new();
        item.read_to_string(&mut text).unwrap();
        entries.push((name, text));
    }
    assert!(entries
        .iter()
        .any(|(name, text)| name == "src/main.rs" && text == "fn main() {}\n"));
    for (name, _) in &entries {
        assert!(
            !name.contains("node_modules")
                && !name.contains(".venv")
                && !name.contains(".git/")
                && name != ".env"
                && name != "ignored.txt"
                && name != "link",
            "{name}"
        );
    }
    assert_eq!(report.included, entries.len());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn snapshot_rejects_total_limit_without_leaving_partial_archive() {
    let root = temp_root();
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("one.txt"), "123456").unwrap();
    let archive = root.join("snapshot.tar.gz");
    let limits = Limits {
        max_file_bytes: 10,
        max_total_bytes: 5,
        max_files: 10,
    };
    assert!(create_snapshot(&project, &archive, limits).is_err());
    assert!(!archive.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn large_file_is_skipped_and_existing_archive_is_never_overwritten() {
    let root = temp_root();
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("small.txt"), "123").unwrap();
    fs::write(project.join("large.bin"), "123456").unwrap();
    let archive = root.join("snapshot.tar.gz");
    let limits = Limits {
        max_file_bytes: 3,
        max_total_bytes: 10,
        max_files: 5,
    };
    let report = create_snapshot(&project, &archive, limits).unwrap();
    assert_eq!(report.included, 1);
    assert_eq!(report.skipped_large, 1);
    let before = fs::read(&archive).unwrap();
    assert!(create_snapshot(&project, &archive, limits).is_err());
    assert_eq!(fs::read(&archive).unwrap(), before);
    assert!(create_snapshot(&project, &project.join("nested.tar.gz"), limits).is_err());
    fs::remove_dir_all(root).unwrap();
}
