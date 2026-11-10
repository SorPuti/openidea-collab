use collab_server::journal::{Event, Journal, JournalRole};

#[test]
fn committed_session_operations_survive_reopening_in_order() {
    let root = std::env::temp_dir().join(format!("collab-journal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let events = vec![
        Event::RoomCreated {
            room: "room".into(),
            owner_hash: [1; 32],
        },
        Event::InviteCreated {
            room: "room".into(),
            invite_hash: [2; 32],
            role: JournalRole::Editor,
            expires_unix: 2000000000,
        },
        Event::MemberJoined {
            room: "room".into(),
            invite_hash: [2; 32],
            member: "member".into(),
            member_hash: [3; 32],
            role: JournalRole::Editor,
        },
        Event::DocumentUpdated {
            room: "room".into(),
            document: "main".into(),
            revision: 1,
            update: vec![1, 2, 3],
        },
        Event::MemberRevoked {
            room: "room".into(),
            member: "member".into(),
        },
    ];
    {
        let mut journal = Journal::open(&db).unwrap();
        for (index, event) in events.iter().enumerate() {
            assert_eq!(
                journal.append(index as u64, event).unwrap(),
                index as u64 + 1
            );
        }
    }
    let journal = Journal::open(&db).unwrap();
    assert_eq!(journal.read_all().unwrap(), events);
    drop(journal);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_writer_does_not_append_or_partially_commit() {
    let root = std::env::temp_dir().join(format!("collab-journal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let mut first = Journal::open(&db).unwrap();
    let mut second = Journal::open(&db).unwrap();
    let event = Event::RoomCreated {
        room: "room".into(),
        owner_hash: [4; 32],
    };
    assert_eq!(first.append(0, &event).unwrap(), 1);
    assert!(second.append(0, &event).is_err());
    assert_eq!(second.read_all().unwrap(), vec![event]);
    drop((first, second));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn journal_file_is_private_and_rejects_symlink() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = std::env::temp_dir().join(format!("collab-journal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let journal = Journal::open(&db).unwrap();
    assert_eq!(
        std::fs::metadata(&db).unwrap().permissions().mode() & 0o777,
        0o600
    );
    drop(journal);
    let link = root.join("link.sqlite");
    symlink(&db, &link).unwrap();
    assert!(Journal::open(&link).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_or_unknown_event_fails_closed_on_replay() {
    let root = std::env::temp_dir().join(format!("collab-journal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let mut journal = Journal::open(&db).unwrap();
    journal
        .append(
            0,
            &Event::RoomCreated {
                room: "room".into(),
                owner_hash: [0; 32],
            },
        )
        .unwrap();
    drop(journal);
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute(
            "UPDATE entries SET payload = ?1 WHERE sequence = 1",
            [b"{\"type\":\"FutureEvent\"}".as_slice()],
        )
        .unwrap();
    drop(connection);
    match Journal::open(&db) {
        Ok(journal) => assert!(journal.read_all().is_err()),
        Err(_) => {} // A usage mismatch may detect the corruption first.
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn exclusive_owner_prevents_another_server_from_opening_the_log() {
    let root = std::env::temp_dir().join(format!("collab-journal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let owner = Journal::open(&db).unwrap();
    owner.claim_exclusive().unwrap();
    assert!(Journal::open(&db).is_err());
    drop(owner);
    assert!(Journal::open(&db).is_ok());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn quota_rejects_new_events_but_reserves_space_for_revocation_across_restart() {
    use collab_server::journal::{JournalLimits, QuotaExceeded};
    let root = std::env::temp_dir().join(format!("collab-journal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let created = Event::RoomCreated {
        room: "room".into(),
        owner_hash: [3; 32],
    };
    let revoke = Event::MemberRevoked {
        room: "room".into(),
        member: "member".into(),
    };
    let limits = JournalLimits {
        normal_bytes: serde_json::to_vec(&created).unwrap().len() as u64,
        revoke_reserve_bytes: serde_json::to_vec(&revoke).unwrap().len() as u64,
    };
    let mut journal = Journal::open_with_limits(&db, limits).unwrap();
    assert_eq!(journal.append(0, &created).unwrap(), 1);
    assert!(journal
        .append(1, &created)
        .unwrap_err()
        .downcast_ref::<QuotaExceeded>()
        .is_some());
    assert_eq!(journal.append(1, &revoke).unwrap(), 2);
    drop(journal);
    let mut journal = Journal::open_with_limits(&db, limits).unwrap();
    assert_eq!(
        journal.read_all().unwrap(),
        vec![created.clone(), revoke.clone()]
    );
    assert!(journal
        .append(2, &revoke)
        .unwrap_err()
        .downcast_ref::<QuotaExceeded>()
        .is_some());
    assert!(journal
        .append(2, &created)
        .unwrap_err()
        .downcast_ref::<QuotaExceeded>()
        .is_some());
    drop(journal);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn opens_and_migrates_a_version_one_journal_without_losing_records() {
    let root = std::env::temp_dir().join(format!("collab-journal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("state.sqlite");
    let event = Event::RoomCreated {
        room: "room".into(),
        owner_hash: [5; 32],
    };
    let journal = Journal::open(&db).unwrap();
    drop(journal);
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch("DROP TABLE IF EXISTS journal_usage; PRAGMA user_version=1;")
        .unwrap();
    connection
        .execute(
            "INSERT INTO entries(sequence,payload) VALUES(1,?1)",
            [serde_json::to_vec(&event).unwrap()],
        )
        .unwrap();
    drop(connection);
    let mut journal = Journal::open(&db).unwrap();
    assert_eq!(journal.read_all().unwrap(), vec![event.clone()]);
    assert_eq!(journal.append(1, &event).unwrap(), 2);
    drop(journal);
    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}
