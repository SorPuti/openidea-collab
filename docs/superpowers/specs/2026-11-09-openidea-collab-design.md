# openidea-collab — Design Spec

## Goal

Plugin para OpenIDEA que permite 2+ desenvolvedores (e agentes de IA) editarem o mesmo projeto simultaneamente em tempo real.

## Architecture

```
OpenIDEA Plugin (Rust/WASM)         collab-server (Rust)
├── Commands panel                   ├── axum WebSocket server
├── Presence sidebar                 ├── Room registry (HashMap)
├── Editor hooks (capture/apply)     ├── diamond-types CRDT per file
└── WebSocket client                 └── Presence broadcast
        ↕ ws://host:4444/room/{id}
```

## Components

### collab-server

Standalone Rust binary. Manages rooms, documents, and presence.

- **Room**: `id: String`, `members: Vec<Member>`, `docs: HashMap<String, DocState>`
- **DocState**: wraps `diamond_types::list_fuzzer::SimpleOpLog` for each file
- **WebSocket endpoint**: `ws://host:4444/ws?room={id}&user={name}`
- **HTTP endpoint**: `POST /room` → creates room, returns `room_id`

### openidea-collab (plugin)

WASM plugin for OpenIDEA. Connects to collab-server via WebSocket.

- **Commands**: Create Room, Join Room, Leave Room
- **Panel**: "Collaborators" sidebar — list users + their current file
- **Editor hooks**: on_text_change → send `EditMsg`; on_receive_edit → apply to buffer
- **Cursors**: on_cursor_move → send `CursorMsg`; render remote cursors as colored overlays

### collab-proto (shared crate)

Serde message types used by both server and plugin.

## Protocol (JSON over WebSocket)

```
Client → Server:
  { "t": "edit",    "file": "src/main.rs", "op": <OpData> }
  { "t": "cursor",  "file": "src/main.rs", "pos": 42 }
  { "t": "presence","file": "src/main.rs" }
  { "t": "leave" }

Server → Client:
  { "t": "welcome",         "user_id": "u1", "members": [...] }
  { "t": "member_joined",   "user_id": "u2", "name": "Ana" }
  { "t": "member_left",     "user_id": "u2" }
  { "t": "edit",            "user_id": "u2", "file": "...", "op": <OpData> }
  { "t": "cursor",          "user_id": "u2", "file": "...", "pos": 42 }
  { "t": "presence_update", "user_id": "u2", "file": "..." }
  { "t": "conflict_warn",   "file": "...", "users": ["u1","u2"] }
```

## OpData (diamond-types)

Serialized as the base64-encoded bytes of `diamond_types` merge data. Server holds the authoritative doc; clients send their local ops and receive the server-merged result.

## MVP Scope

1. Create room (server generates ID) + copy invite link
2. Join room by ID
3. Presence panel: who is connected, which file they're viewing
4. File edit sync: capture changes via editor hook, apply remote ops
5. Conflict warning when two users edit same line simultaneously

## Success Criteria

- Two OpenIDEA windows on same machine can co-edit a file
- Edits converge (no data loss) after concurrent changes
- Panel updates within 200ms of a member joining/leaving
- Build passes `cargo test` on both crates
- Plugin loads via `openidea-plugin link` without errors
