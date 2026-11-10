# OpenIDEA Collab

Experimental file collaboration plugin for [OpenIDEA](https://github.com/l0ng-ai/openidea), with a Rust/WASM plugin and a standalone HTTP server.

**Status: not approved for marketplace listing.** The historical `v0.1.0` package has unresolved safety and provenance issues. The current source still needs sharing boundaries, project-bound sessions, participant authentication and reliable conflict recovery. See [security and data handling](docs/security.md) before running it.

## What works today

- Create, join and leave rooms.
- Upload the full text of a saved file and retrieve remote file updates.
- Display participant names, optional GitHub usernames, current files and cursor snapshots.
- Reject some concurrent updates with HTTP 409. Automatic three-way merging is not integrated.

Remote changes are polled on scheduled events. The manifest requests a 60-second schedule; the server waits five seconds before rechecking an empty event stream. This is file synchronization on save, not character-by-character collaborative editing. No subsecond latency guarantee is made.

## Development quick start

Use only disposable projects and synthetic data while the safety work remains open. The server currently binds to **all IPv4 interfaces** and has no participant authentication; isolate it from untrusted networks.

```sh
git clone https://github.com/SorPuti/openidea-collab.git
cd openidea-collab
cargo run --locked --manifest-path collab-server/Cargo.toml
```

The default port is `4444`; set `COLLAB_PORT` to change it. `RUST_LOG` controls logging. Rooms, file contents and events are held in memory and disappear when the server stops.

In a second terminal, install the CLI matching the SDK and build the plugin:

```sh
rustup toolchain install 1.96.0 --profile minimal
cargo +1.96.0 install --locked --git https://github.com/l0ng-ai/openidea --tag v0.1.40 ide-host --bin openidea-plugin
openidea-plugin build openidea-collab
openidea-plugin link openidea-collab
```

Restart OpenIDEA. In a disposable project, use **Collab: Create Room** or **Collab: Join Room** and share the room ID with the second participant. **Collab: Leave Room** ends participation. **Collab: Set Server URL** changes the endpoint; the default is `http://localhost:4444`. Localhost and IPv4 loopback network permissions are declared. Other hosts need explicit manifest permissions and a development rebuild; arbitrary remote URLs are not supported by the default permissions.

Development linking bypasses publisher verification. It does not establish marketplace approval or resolve the safety issues. The legacy `install.sh` also installs services and can consume historical release assets; prefer the explicit development steps above during remediation.

## Repository

| Path | Purpose |
| --- | --- |
| `openidea-collab/` | WASM plugin, client, sync and presence panel |
| `collab-server/` | Axum server, rooms and in-memory event/file stores |
| `scripts/` | Development, validation and packaging helpers |
| `docs/` | Architecture, data handling, manual validation and releases |

Read [CONTRIBUTING.md](CONTRIBUTING.md) for checks and contribution conventions, [architecture](docs/architecture.md) for the protocol, and [release instructions](docs/releasing.md) for packaging. [CHANGELOG.md](CHANGELOG.md) distinguishes source changes from published releases.

## License

MIT; see [LICENSE](LICENSE). OpenIDEA SDK and CLI dependencies retain their own licenses and notices. See the [release instructions](docs/releasing.md) for the dependency-notice review required before distribution.
