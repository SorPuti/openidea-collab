# OpenIDEA Collab

Open-source collaboration for [OpenIDEA](https://github.com/l0ng-ai/openidea), using a small authenticated relay at **collab.synthosia.com**. MIT licensed.

**Status:** the development source `0.4.7-dev.1` enables inbound edits for explicitly shared documents and requests `editor.write`. This is an unreviewed development build, not the signed Marketplace release. OpenIDEA v0.1.44 predates the upstream no-follow fix; using this build there permits remote edits without that protection. Real IDE acceptance and the save path are not yet verified. The relay at `collab.synthosia.com` must be operational for sessions to be created.

## Install as a collaborator

The intended distribution is one signed `.oiplugin` installed through **Settings → Plugins**. No Rust, server, tunnel, VPN or native helper is required on the collaborator's machine. In OpenIDEA v0.1.43 or newer, install the package, prepare or join a session, connect realtime, focus a document and choose **Share Current Document**. Every participant explicitly selects the same project-relative document; no project scan or automatic whole-project sharing occurs. The development build can apply incoming updates to shared open documents after requesting editor write permission; signed Marketplace versions may behave differently. This does not provide whole-project synchronization.

The default plugin connects to `https://collab.synthosia.com`, operated by Synthosia. If that relay is unavailable, collaboration stops until it is reachable again; it is not a backup or offline sync service. Invitations contain a temporary, single-use secret in the URL fragment. Realtime transport is **WSS**, terminated by the operator's TLS reverse proxy; the Rust relay binds to loopback. HTTPS and WSS serve different parts of the same protocol.

## Repository layout

All implementation and tooling source lives under `src/`:

```text
src/
  protocol/   shared control schemas and bounded binary frames
  core/       UTF-16 CRDT, chunk reassembly, checkpoints and sharing policy
  relay/      authenticated HTTP control plane and WebSocket forwarding
  plugin/     OpenIDEA WASM UI and realtime document integration
  tooling/    build, packaging, validation and release scripts/tests
```

`docs/` contains documentation, `deploy/` operator configuration, `.github/` repository automation and ownership, and root Cargo files workspace metadata. Tests live with their components. There is one workspace lockfile and one pinned Rust toolchain.

## Develop

Developers need Rust 1.96.0 (selected by `rust-toolchain.toml`); users installing the plugin do not.

```sh
cargo test --locked --workspace
./src/tooling/check.sh
```

To run the relay locally:

```sh
cargo run --locked -p collab-server
```

Defaults: `127.0.0.1:4444`, public origin `https://collab.synthosia.com`, private authorization metadata in `./collab-state/metadata.json`. Documents, updates and credentials in plaintext are not written to that file. Use a dedicated state directory. A non-loopback public origin requires `COLLAB_ADMISSION_TOKEN`; the creator enters the corresponding token into the IDE keychain when preparing a session. See [operations](docs/operations.md).

To build a development plugin package with the official v0.1.43 CLI:

```sh
./src/tooling/package-dev.sh
openidea-plugin link target/plugin-package
```

A development link bypasses publisher verification. It does not imply marketplace approval. See [installation](docs/installation.md) and [releases](docs/releasing.md).

## Limits and data handling

The relay forwards opaque binary updates without constructing CRDT documents. It bounds sessions, participants, frame size, input rates and output queues. A slow recipient is disconnected for resynchronization. Only small authorization metadata is persisted, through atomic replacement and fsync. End-to-end encryption is not implemented: the operator can inspect transient bytes.

A relay receipt is not a durability ACK. Clients persist project-bound CRDT checkpoints and pending updates in private plugin storage, ACK only after applying and checkpointing an update, and use state vectors after reconnecting. The relay retains no document backlog, so joining still requires an authorized connected peer with a document copy. A room is not a cloud backup. Conflicting initial local content and stale versioned writes are preserved and surfaced instead of overwritten.

Read [architecture](docs/architecture.md), [protocol](docs/protocol.md), [security](docs/security.md), [implementation status](docs/implementation-status.md), [contributing](CONTRIBUTING.md), and [license notices](NOTICE).
