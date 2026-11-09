# OpenIDEA Collab

Real-time collaborative editing for [OpenIDEA](https://github.com/l0ng-ai/openidea) — sync files between developers (and AI agents) over HTTP long-poll without WebSockets.

## Features

- **Room-based sessions** — create or join a room with a simple ID
- **File sync** — changes are pushed to all room members within ~500 ms
- **Presence panel** — see who is in the room and which file they have open
- **3-way merge** — automatic conflict resolution using LCS diff; 409 returned when unresolvable
- **Cross-network** — route through any HTTP reverse proxy (ngrok, Cloudflare Tunnel, etc.)
- **AI-agent ready** — the collab-server exposes a plain REST API any agent can call

## Architecture

```
┌────────────────┐    HTTP long-poll     ┌──────────────────┐
│  OpenIDEA IDE  │◄──────────────────────│  collab-server   │
│  (plugin WASM) │──────────────────────►│  (axum / Tokio)  │
└────────────────┘   POST /rooms/:id/…   └──────────────────┘
```

| Component | Description |
|-----------|-------------|
| `openidea-collab/` | Plugin (Rust → `wasm32-wasip2`) — commands, sync loop, presence panel |
| `collab-server/` | Standalone HTTP server — rooms, events, presence, file store |
| `scripts/` | Dev helpers (`dev-server.sh`, `dev-plugin.sh`) |
| `.github/workflows/release.yml` | CI: build → pack → create GitHub Release |

## Quick start

### 1. Run the server

```bash
# Install Rust if needed: https://rustup.rs
cargo run --manifest-path collab-server/Cargo.toml
# Server listens on http://localhost:4444
```

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `COLLAB_PORT` | `4444` | Listening port |
| `RUST_LOG` | `info` | Log level |

### 2. Build and install the plugin

```bash
# Install the OpenIDEA plugin CLI
cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin

# Build
cd openidea-collab
openidea-plugin build        # output: dist/plugin.wasm

# Install into OpenIDEA (dev link)
openidea-plugin link .
```

### 3. Start collaborating

1. Open the **Collaborators** panel in the right sidebar.
2. Run **Collab: Create Room** — note the Room ID shown in the notification.
3. Share the Room ID and your server URL with your collaborator.
4. On the other machine: run **Collab: Join Room** and enter the Room ID and server URL.

> **Cross-network**: expose the server with `ngrok http 4444` and share the HTTPS URL as the server address.

## REST API (collab-server)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/rooms` | Create room → `{"room_id": "…"}` |
| `POST` | `/rooms/:id/join` | Join room, get `member_id` |
| `GET` | `/rooms/:id/members` | List present members |
| `POST` | `/rooms/:id/members/:uid/presence` | Update current file |
| `GET` | `/rooms/:id/files/:path` | Get latest file text + version |
| `POST` | `/rooms/:id/files/:path` | Push file update (conflict → 409) |
| `GET` | `/rooms/:id/events?cursor=N` | Long-poll for new events (holds 5 s) |

All endpoints return JSON. The `events` endpoint holds the connection open until an event arrives or the timeout expires.

## Install from marketplace

In **OpenIDEA → Settings → Plugins → Available**, search for `openidea-collab`.

Or install manually: download the `.oiplugin` from the [latest release](../../releases/latest) and drag it onto the OpenIDEA window.

## Development

```bash
# Run server with auto-reload (requires cargo-watch)
cargo watch -x 'run --manifest-path collab-server/Cargo.toml'

# Run server tests
cargo test --manifest-path collab-server/Cargo.toml

# Rebuild plugin after code changes
cd openidea-collab && openidea-plugin build
```

## Release

Tag a version to trigger the GitHub Actions release workflow:

```bash
git tag v0.2.0
git push origin v0.2.0
```

The workflow builds the plugin, packs it as `.oiplugin`, and creates a GitHub Release. Required secrets: `OPENIDEA_KEY` (PEM signing key content) and `OPENIDEA_PUBLISHER_ID` (`sorputi`).

## License

MIT — see [LICENSE](LICENSE).
