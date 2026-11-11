Relates to #2532.

## What and why

Wasm plugins that consume realtime services currently have no bounded background transport. This adds a generic API 1.6 host surface for project-bound WebSocket connections, without document or collaboration semantics.

The host owns the socket and separate bounded incoming/outgoing queues. It requires the endpoint's `net:<host>` capability, expands declared keychain secrets only in the `Authorization` header, requires WSS outside loopback and a negotiated subprotocol, and emits `websocket-ready` instead of requiring polling. Connection handles cannot cross plugin or project boundaries and are cleaned up on close, transport failure, plugin stop, or after an abandoned final drain. TLS reuses the backend's existing rustls client configuration with ring and the platform verifier; the WebSocket dependency does not select another crypto provider or trust-store implementation.

Queue overflow explicitly rejects the new frame and closes the connection. It never reports a refused frame as accepted and never grows retained memory without a bound. The Rust SDK and `MockHost` expose the same connect/send/drain/close lifecycle for downstream tests.

Document subscriptions are intentionally excluded from this PR and live on the stacked [`SorPuti:feat/plugin-document-subscriptions`](https://github.com/SorPuti/openidea/tree/feat/plugin-document-subscriptions) branch.

## How it was verified

Current API 1.6 update (`80150e0`):

- `cargo test --manifest-path sdk/rust/openidea-plugin/Cargo.toml` — 61 passed; the API freeze test accepts the 1.6 additions and preserves the released 1.4 surface
- `cargo test -p ide-extension -p ide-plugin --lib --tests` — 74 `ide-extension` and 25 `ide-plugin` unit tests passed; the compatibility test then identified the expected stale checked-in 1.5 components
- rebuilt all 43 checked-in official/fixture components for API 1.6
- `plugins/build.sh --check` — all 43 components and the Graphviz web page passed
- `cargo fmt --all --check`
- `pnpm format:check` — 2,629 files passed
- `src/bindings.ts` was regenerated from the Linux CI export output that exposed the stale file

Transport/TLS validation before the version-only update:

- `cargo test -p ide-host --lib` — 347 passed
- `cargo test -p ide-host --lib plugins::network_inbox::tests` — 8 passed
- `cargo test -p ide-host --lib plugins::websocket::tests` — 4 passed
- `cargo test -p ide-http --lib` — 24 passed
- `cargo clippy -p ide-host -p ide-plugin --all-targets --no-deps -- -D warnings -A clippy::nonminimal-bool` — passed; the allowed lint is pre-existing in untouched `main` files under Rust 1.96
- `pnpm test` — 798 files and 9,741 tests passed; 6 skipped

The new GitHub Actions runs are waiting for the repository's fork-workflow approval.

There are no user-visible UI changes. Relay authentication models, collaboration protocols, document ownership, and CRDT behavior remain downstream concerns.

## Review focus

Please review the capability and project boundaries, bounded queue/refusal contract, readiness and close lifecycle, WIT compatibility, SDK/host parity, and reuse of the existing ring/platform-verifier TLS configuration. The maintainer approved the `tokio-tungstenite` runtime dependency in the PR conversation.
