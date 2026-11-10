# Architecture

The repository contains two independently built Rust components. `openidea-collab` runs as a WASM component inside OpenIDEA and accesses the project through the host SDK. `collab-server` is an Axum 0.8/Tokio HTTP service with an in-memory room map.

## Current flow

1. A client creates a room, then joins with a display name and receives a user ID.
2. File-save events read the whole file and POST its path, text, user ID and known version.
3. The server stores the latest text and emits a versioned file-update event.
4. Scheduled plugin events poll the event stream. For a newer file version, the client fetches the current text and writes it through the project API.
5. Editor focus sends file/cursor presence. The UI renders received presence snapshots.

The room ID is an invitation locator, but currently also acts as the only access gate. User IDs are supplied by the client and are not authenticated. Room state is stored in global plugin storage. See [security.md](security.md).

## HTTP routes

| Method | Route | Result |
| --- | --- | --- |
| POST | `/rooms` | Create a room |
| GET | `/rooms/{id}` | Room and members |
| POST | `/rooms/{id}/join` | Join; returns `user_id` |
| DELETE | `/rooms/{id}/members/{uid}` | Remove a member |
| GET / POST | `/rooms/{id}/files/{*path}` | Read / update full file text |
| GET | `/rooms/{id}/members` | Presence snapshots |
| POST | `/rooms/{id}/members/{uid}/presence` | Update presence |
| GET | `/rooms/{id}/events?cursor=N` | Events and next cursor |

Axum 0.8 uses brace-based parameters. An empty event poll sleeps five seconds and rechecks; it is not immediately awakened by new events. Events and file history are not durable.

## Concurrency limitations

Updates with a mismatching version from a different user receive HTTP 409 and the current server version/text. The client currently discards that structured conflict response. The server allows stale updates attributed to the same user. `document.rs` provides a merge helper, but the update route does not call it. Pending local edits and failed remote writes are not reliably retained. These are open correctness requirements, not supported merge behavior.
