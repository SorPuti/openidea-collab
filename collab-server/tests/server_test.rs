use collab_server::room::{Member, Room};
use collab_server::events::EventStore;
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
    let events = store.since(0, 0); // timeout_ms=0: return immediately
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 0);
    let empty = store.since(1, 0);
    assert!(empty.is_empty());
}

#[test]
fn event_store_cursor_overflow() {
    let store = EventStore::new();
    let events = store.since(999999, 0);
    assert!(events.is_empty());
}

#[test]
fn merge3_non_conflicting() {
    use collab_server::document::merge3;
    let base = "line1\nline2\nline3\n";
    let ours = "line1\nLINE2\nline3\n";   // changed line2
    let theirs = "line1\nline2\nLINE3\n"; // changed line3
    let merged = merge3(base, ours, theirs).expect("should merge cleanly");
    assert!(merged.contains("LINE2"));
    assert!(merged.contains("LINE3"));
}

#[test]
fn merge3_conflict() {
    use collab_server::document::{merge3, MergeError};
    let base = "line1\nline2\n";
    let ours = "line1\nOURS\n";
    let theirs = "line1\nTHEIRS\n";
    assert!(matches!(merge3(base, ours, theirs), Err(MergeError::Conflict)));
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
