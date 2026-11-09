# openidea-collab — Design Spec (v2)

## Goal

Plugin para OpenIDEA que permite 2+ desenvolvedores (e agentes de IA) trabalharem no mesmo projeto com sincronização de arquivos em tempo (quase) real.

## Plugin API Constraints (discovered from wit/)

- **Network**: only `net.fetch` (HTTP). No WebSocket, no SSE.
- **HTML view**: sandboxed, no network. Use tree/list views instead.
- **Editor events**: `file-opened`, `file-saved`, `files-changed`. No keystroke-level events.
- **Editor read/write**: `project.read-file(path)` / `project.write-file(path, text)`.
- **Text edits**: `text-editor.apply-edits(version, edits)` — requires editor open.
- **Background tasks**: long-running via `tasks` interface (60s budget per report).

## Architecture

```
OpenIDEA Plugin (WASM/Rust)         collab-server (Rust/axum)
├── Commands panel                   ├── POST /rooms
├── Presence panel (tree view)       ├── GET  /rooms/{id}
├── On file-saved → POST full text   ├── POST /rooms/{id}/join
├── Background task: long-poll       ├── DELETE /rooms/{id}/members/{uid}
│   GET /rooms/{id}/events?cur=N     ├── POST /rooms/{id}/files/{path}
│   → write-file if remote newer     ├── GET  /rooms/{id}/files/{path}
└── Conflict notification            ├── GET  /rooms/{id}/events?cursor=N (long-poll)
                                     └── GET  /rooms/{id}/members
```

## HTTP REST API

```
POST   /rooms                           → 201 { room_id: "ABCD1234" }
GET    /rooms/{id}                      → 200 { room_id, members:[...] } | 404
POST   /rooms/{id}/join                 → 200 { user_id } | 404
  body: { name: "Ana" }
DELETE /rooms/{id}/members/{user_id}    → 204 | 404
POST   /rooms/{id}/files/{path}         → 200 { event_id } | 409 ConflictWarn
  body: { user_id, version: N, text: "..." }
GET    /rooms/{id}/files/{path}         → 200 { version: N, text: "...", user_id } | 404
GET    /rooms/{id}/members              → 200 [{ user_id, name, current_file? }]
GET    /rooms/{id}/events?cursor=N      → 200 { events:[...], next_cursor: M }
  events: { id, type, payload }
  types: file_updated | member_joined | member_left | presence_changed | conflict_warn
  long-poll: holds up to 5s when no events; cursor=0 for all history
```

## Sync Protocol

1. User opens file → plugin notes current version, subscribes to `file-saved`
2. User saves file → plugin `read-file` → `POST /rooms/{id}/files/{path}` with version+text
3. Background task polls `GET /rooms/{id}/events?cursor=N` (5s long-poll timeout)
4. On `file_updated` event: compare remote version with local → if remote newer, `write-file`
5. On `conflict_warn`: show notification "Conflict in {file} with {user}"

## Conflict Detection

Server detects conflict when:
- File version in POST doesn't match the server's last-known-remote-version for that (room, path, user)
- Two different users POST the same file within the same event window

On conflict: server applies diamond-types 3-way merge (base=last shared text, theirs=remote, mine=local). If merge clean → returns merged text in response. If merge fails → returns 409 with both versions; plugin shows notification.

## Plugin Structure

```
openidea-collab/
├── plugin.toml
├── Cargo.toml
└── src/
    ├── lib.rs          # Plugin trait impl: commands, events, view-provider
    ├── client.rs       # HTTP client wrappers (net.fetch)
    ├── sync.rs         # Sync loop (background task), file version tracking
    ├── presence.rs     # Presence state + tree view rendering
    └── room.rs         # Room state: room_id, user_id, members
```

## Server Structure

```
collab-server/
├── Cargo.toml
└── src/
    ├── main.rs         # axum router + AppState
    ├── routes.rs       # All route handlers
    ├── room.rs         # Room, Member, FileState
    ├── document.rs     # DocState (diamond-types merge)
    └── events.rs       # EventStore (append-only, cursor-indexed)
```

## plugin.toml capabilities

```toml
[capabilities]
required = ["project.read", "project.write", "events", "storage"]
optional = ["net:localhost", "net.http:*"]
```

## Success Criteria

- Two OpenIDEA instances can co-edit a file: save on A → B sees update within 10s
- Conflict on same file → notification with both versions
- Presence panel shows current file per member
- `cargo test` passes on both crates
- Plugin loads via `openidea-plugin link` without errors
