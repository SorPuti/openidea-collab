# OpenIDEA Collab

Open-source collaboration for [OpenIDEA](https://github.com/l0ng-ai/openidea), using a small authenticated relay at **collab.synthosia.com**. MIT licensed.

**Status:** the relay and HTTPS session preparation are implemented. Native realtime editing is **not available in the current OpenIDEA API**. The plugin explicitly reports this limitation and never uploads project files or applies remote writes. This source is not an approved marketplace release, and this repository does not establish that the domain is deployed.

## Install as a collaborator

The intended distribution is one signed `.oiplugin` installed through **Settings → Plugins**. No Rust, server, tunnel, VPN or native helper is required on the collaborator's machine. The package currently prepares sessions and invitations only; live editing remains gated on the public host APIs described in [the integration proposal](docs/host-api.md).

The plugin uses `https://collab.synthosia.com` for session control. Invitations contain a temporary, single-use secret in the URL fragment. Realtime transport is **WSS**, terminated by the operator's TLS reverse proxy; the Rust relay binds to loopback. HTTPS and WSS serve different parts of the same protocol.

## Repository layout

All implementation and tooling source lives under `src/`:

```text
src/
  protocol/   shared control schemas and bounded binary frames
  core/       UTF-16 CRDT, chunk reassembly, checkpoints and sharing policy
  relay/      authenticated HTTP control plane and WebSocket forwarding
  plugin/     OpenIDEA WASM session preparation and native UI
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

Defaults: `127.0.0.1:4444`, public origin `https://collab.synthosia.com`, private authorization metadata in `./collab-state/metadata.json`. Documents, updates and credentials in plaintext are not written to that file. Use a dedicated state directory. The public service still needs operator deployment, TLS and abuse controls; see [operations](docs/operations.md).

To build a development plugin package with the official CLI v0.1.40 available:

```sh
./src/tooling/package-dev.sh
openidea-plugin link target/plugin-package
```

A development link bypasses publisher verification. It does not imply marketplace approval. See [installation](docs/installation.md) and [releases](docs/releasing.md).

## Limits and data handling

The relay forwards opaque binary updates without constructing CRDT documents. It bounds sessions, participants, frame size, input rates and output queues. A slow recipient is disconnected for resynchronization. Only small authorization metadata is persisted, through atomic replacement and fsync. End-to-end encryption is not implemented: the operator can inspect transient bytes.

A relay receipt is not a durability ACK. Clients must retain state and pending updates; joining requires an authorized peer with a document copy. A URL is not a cloud backup. The standalone core supplies bounded checkpoints, but integration with actual IDE persistence/undo remains unimplemented.

Read [architecture](docs/architecture.md), [protocol](docs/protocol.md), [security](docs/security.md), [implementation status](docs/implementation-status.md), [contributing](CONTRIBUTING.md), and [license notices](NOTICE).
