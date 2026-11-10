# OpenIDEA Collab

Open-source collaboration for [OpenIDEA](https://github.com/l0ng-ai/openidea), using a small authenticated relay at **collab.synthosia.com**. MIT licensed.

**Status:** the relay, authenticated WSS forwarding and an experimental native realtime plugin are implemented. The plugin uses the generic project-bound WebSocket and versioned document APIs proposed in [OpenIDEA PR #2544](https://github.com/l0ng-ai/openidea/pull/2544), so development builds currently pin that reviewed fork commit. It cannot be published as a normal marketplace package until the API is merged and released by OpenIDEA. The source is not an approved marketplace release.

## Install as a collaborator

The intended distribution is one signed `.oiplugin` installed through **Settings → Plugins**. No Rust, server, tunnel, VPN or native helper is required on the collaborator's machine. In a development build of OpenIDEA containing PR #2544, install the package, prepare or join a session, connect realtime, focus a document and choose **Share Current Document**. Every participant explicitly selects the same project-relative document; no project scan or automatic whole-project sharing occurs.

The plugin uses `https://collab.synthosia.com` for session control. Invitations contain a temporary, single-use secret in the URL fragment. Realtime transport is **WSS**, terminated by the operator's TLS reverse proxy; the Rust relay binds to loopback. HTTPS and WSS serve different parts of the same protocol.

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

Defaults: `127.0.0.1:4444`, public origin `https://collab.synthosia.com`, private authorization metadata in `./collab-state/metadata.json`. Documents, updates and credentials in plaintext are not written to that file. Use a dedicated state directory. The public domain serves HTTPS/WSS; production monitoring, admission policy and an independent security review remain operator work. See [operations](docs/operations.md).

To build a development plugin package with a CLI built from the PR #2544 branch:

```sh
./src/tooling/package-dev.sh
openidea-plugin link target/plugin-package
```

A development link bypasses publisher verification. It does not imply marketplace approval. See [installation](docs/installation.md) and [releases](docs/releasing.md).

## Limits and data handling

The relay forwards opaque binary updates without constructing CRDT documents. It bounds sessions, participants, frame size, input rates and output queues. A slow recipient is disconnected for resynchronization. Only small authorization metadata is persisted, through atomic replacement and fsync. End-to-end encryption is not implemented: the operator can inspect transient bytes.

A relay receipt is not a durability ACK. Clients persist project-bound CRDT checkpoints and pending updates in private plugin storage, ACK only after applying and checkpointing an update, and use state vectors after reconnecting. The relay retains no document backlog, so joining still requires an authorized connected peer with a document copy. A room is not a cloud backup. Conflicting initial local content and stale versioned writes are preserved and surfaced instead of overwritten.

Read [architecture](docs/architecture.md), [protocol](docs/protocol.md), [security](docs/security.md), [implementation status](docs/implementation-status.md), [contributing](CONTRIBUTING.md), and [license notices](NOTICE).
