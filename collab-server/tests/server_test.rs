use collab_server::events::EventStore;
use collab_server::room::{Member, Room};
use std::time::Instant;

#[test]
fn room_add_remove_member() {
    let mut room = Room::new("ABCD1234".into());
    room.add_member(Member {
        user_id: "u1".into(),
        name: "Ana".into(),
        current_file: None,
        last_seen: Instant::now(),
    });
    assert_eq!(room.members.len(), 1);
    room.remove_member("u1");
    assert_eq!(room.members.len(), 0);
}

#[test]
fn event_store_cursor() {
    let mut store = EventStore::new();
    store.push(collab_server::events::RoomEvent::MemberJoined {
        user_id: "u1".into(),
        name: "Ana".into(),
    });
    let events = store.since(0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 0);
    let empty = store.since(1);
    assert!(empty.is_empty());
}

#[test]
fn event_store_cursor_overflow() {
    let store = EventStore::new();
    let events = store.since(999999);
    assert!(events.is_empty());
}

#[test]
fn merge3_non_conflicting() {
    use collab_server::document::merge3;
    // Edits at the first and last lines of an 8-line file are clearly
    // non-overlapping: their diff-context windows cover opposite ends of the
    // file and diffy resolves them cleanly.
    let base = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\n";
    let ours = "LINE1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\n"; // changed line1
    let theirs = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nLINE8\n"; // changed line8
    let merged = merge3(base, ours, theirs).expect("should merge cleanly");
    assert!(merged.contains("LINE1"));
    assert!(merged.contains("LINE8"));
}

#[test]
fn merge3_conflict() {
    use collab_server::document::{merge3, MergeError};
    let base = "line1\nline2\n";
    let ours = "line1\nOURS\n";
    let theirs = "line1\nTHEIRS\n";
    assert!(matches!(
        merge3(base, ours, theirs),
        Err(MergeError::Conflict)
    ));
}

// Review Focus: member heartbeat expiry
#[test]
fn member_expired() {
    use std::time::Duration;
    let member = Member {
        user_id: "u1".into(),
        name: "Ana".into(),
        current_file: None,
        last_seen: Instant::now() - Duration::from_secs(31),
    };
    assert!(member.is_expired(30));
}
