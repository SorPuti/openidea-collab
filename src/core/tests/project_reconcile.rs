use openidea_collab_core::project_sync::{Baseline, Decision, FileVersion, reconcile};

fn file(bytes: &[u8]) -> FileVersion {
    FileVersion::from_bytes(bytes)
}

#[test]
fn first_sync_copies_unique_files_and_refuses_divergent_pairs() {
    let a = file(b"one");
    let b = file(b"two");
    assert_eq!(reconcile(Baseline::Unknown, Some(a), None), Decision::Send);
    assert_eq!(
        reconcile(Baseline::Unknown, None, Some(a)),
        Decision::Receive
    );
    assert_eq!(
        reconcile(Baseline::Unknown, Some(a), Some(a)),
        Decision::Unchanged
    );
    assert_eq!(
        reconcile(Baseline::Unknown, Some(a), Some(b)),
        Decision::Conflict
    );
}

#[test]
fn known_baseline_detects_changes_deletions_and_conflicts() {
    let base = file(b"before");
    let a = file(b"edit-a");
    let b = file(b"edit-b");
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), Some(a), Some(base)),
        Decision::Send
    );
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), Some(base), Some(b)),
        Decision::Receive
    );
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), None, Some(base)),
        Decision::DeleteRemote
    );
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), Some(base), None),
        Decision::DeleteLocal
    );
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), None, Some(b)),
        Decision::Conflict
    );
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), Some(a), None),
        Decision::Conflict
    );
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), Some(a), Some(b)),
        Decision::Conflict
    );
    assert_eq!(
        reconcile(Baseline::Known(Some(base)), None, None),
        Decision::Unchanged
    );
}

#[test]
fn binary_versions_hash_the_exact_bytes() {
    let one = file(&[0, 255, 1, 0]);
    let two = file(&[0, 255, 1, 1]);
    assert_ne!(one, two);
    assert_eq!(
        reconcile(Baseline::Known(None), Some(one), Some(two)),
        Decision::Conflict
    );
}

#[test]
fn inventory_refuses_sensitive_ambiguous_and_overlapping_paths() {
    use openidea_collab_core::project_sync::Manifest;
    let a = file(b"x");
    assert!(Manifest::new([("src/a.rs".into(), a), ("src/b.png".into(), a)]).is_ok());
    for paths in [
        vec![".env"],
        vec!["src/A.rs", "src/a.rs"],
        vec!["src/file", "src/file/child"],
        vec!["a", "a-b", "a/child"],
        vec!["../outside"],
    ] {
        assert!(Manifest::new(paths.into_iter().map(|p| (p.to_string(), a))).is_err());
    }
}

#[test]
fn full_inventory_planning_refuses_incomplete_scans_and_cross_peer_collisions() {
    use openidea_collab_core::project_sync::{Manifest, plan};
    let a = file(b"a");
    let b = file(b"b");
    let base = Manifest::new([("old.txt".into(), a), ("gone.txt".into(), a)]).unwrap();
    let local = Manifest::new([("old.txt".into(), b), ("new.png".into(), a)]).unwrap();
    let remote = Manifest::new([("old.txt".into(), a), ("gone.txt".into(), a)]).unwrap();
    let result = plan(Some(&base), &local, &remote, true, true).unwrap();
    assert_eq!(result["old.txt"], Decision::Send);
    assert_eq!(result["new.png"], Decision::Send);
    assert_eq!(result["gone.txt"], Decision::DeleteRemote);
    assert!(plan(Some(&base), &local, &remote, false, true).is_err());
    assert!(plan(Some(&base), &local, &remote, true, false).is_err());
    let upper = Manifest::new([("src/A".into(), a)]).unwrap();
    let lower = Manifest::new([("src/a".into(), a)]).unwrap();
    assert!(plan(None, &upper, &lower, true, true).is_err());
    let nested = Manifest::new([("src/A/child".into(), a)]).unwrap();
    assert!(plan(None, &upper, &nested, true, true).is_err());
}

#[test]
fn large_binary_transfer_streams_in_bounded_chunks_and_detects_corruption() {
    use openidea_collab_core::project_sync::ChunkVerifier;
    let data: Vec<u8> = (0..(3 * 1024 * 1024 + 17))
        .map(|i| (i % 251) as u8)
        .collect();
    let expected = file(&data);
    let mut verifier = ChunkVerifier::new(expected, 4 * 1024 * 1024).unwrap();
    for (index, part) in data.chunks(32768).enumerate() {
        verifier.push((index * 32768) as u64, part).unwrap();
    }
    verifier.finish().unwrap();
    let mut corrupt = ChunkVerifier::new(expected, 4 * 1024 * 1024).unwrap();
    let mut first = data[..32768].to_vec();
    first[1] ^= 1;
    corrupt.push(0, &first).unwrap();
    for (index, part) in data.chunks(32768).enumerate().skip(1) {
        corrupt.push((index * 32768) as u64, part).unwrap();
    }
    assert!(corrupt.finish().is_err());
    let mut out_of_order = ChunkVerifier::new(expected, 4 * 1024 * 1024).unwrap();
    assert!(out_of_order.push(1, &[1]).is_err());
    assert!(out_of_order.push(0, &vec![0; 65537]).is_err());
    assert!(ChunkVerifier::new(expected, 1024).is_err());
}

#[test]
fn real_files_detect_changes_deletion_and_stream_binary_staging() {
    use openidea_collab_core::project_sync::{ChunkVerifier, Manifest, plan};
    use std::io::{Read, Write};
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("left");
    let right = root.path().join("right");
    std::fs::create_dir(&left).unwrap();
    std::fs::create_dir(&right).unwrap();
    let baseline = file(b"original");
    for dir in [&left, &right] {
        std::fs::write(dir.join("old.txt"), b"original").unwrap();
    }
    std::fs::write(left.join("old.txt"), b"local changed").unwrap();
    std::fs::remove_file(right.join("old.txt")).unwrap();
    let local_file = file(&std::fs::read(left.join("old.txt")).unwrap());
    let base = Manifest::new([("old.txt".into(), baseline)]).unwrap();
    let local = Manifest::new([("old.txt".into(), local_file)]).unwrap();
    let remote = Manifest::new([]).unwrap();
    assert_eq!(
        plan(Some(&base), &local, &remote, true, true).unwrap()["old.txt"],
        Decision::Conflict
    );
    assert_eq!(
        std::fs::read(left.join("old.txt")).unwrap(),
        b"local changed"
    );
    assert!(!right.join("old.txt").exists());
    let data: Vec<u8> = (0..(3 * 1024 * 1024 + 19))
        .map(|i| (i % 251) as u8)
        .collect();
    std::fs::write(left.join("asset.bin"), &data).unwrap();
    let mut source = std::fs::File::open(left.join("asset.bin")).unwrap();
    let mut staged = tempfile::tempfile_in(&right).unwrap();
    let mut check = ChunkVerifier::new(file(&data), data.len() as u64).unwrap();
    let mut buf = vec![0; 32768];
    let mut offset = 0;
    loop {
        let count = source.read(&mut buf).unwrap();
        if count == 0 {
            break;
        }
        check.push(offset, &buf[..count]).unwrap();
        staged.write_all(&buf[..count]).unwrap();
        offset += count as u64;
    }
    check.finish().unwrap();
    staged.flush().unwrap();
    assert_eq!(offset, data.len() as u64);
}
