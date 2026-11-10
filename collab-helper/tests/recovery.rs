#[cfg(unix)]
#[test]
fn backup_survives_reopen_and_refuses_insecure_directory() {
    use openidea_collab_helper::recovery::Recovery;
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("collab-recovery-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let backup = Recovery::new(&root, "original").unwrap();
    let path = backup.path().to_owned();
    backup.save("alteração offline 🌍").unwrap();
    drop(backup);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "alteração offline 🌍"
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Recovery::new(&root, "previous").unwrap();
    let filename = backup.path().file_name().unwrap().to_owned();
    let moved = root.with_extension("moved");
    std::fs::rename(&root, &moved).unwrap();
    assert!(backup.save("unsaved").is_err());
    assert_eq!(std::fs::read_to_string(moved.join(filename)).unwrap(), "previous");
    std::fs::rename(&moved, &root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Recovery::new(&root, "data").is_err());
    std::fs::remove_dir_all(root).unwrap();
}
