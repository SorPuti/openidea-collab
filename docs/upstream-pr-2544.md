Relates to #2532.

## What and why

Wasm plugins that consume realtime services currently have no bounded background transport. This adds a generic API 1.5 host surface for project-bound WebSocket connections, without document or collaboration semantics.

The host owns the socket and separate bounded incoming/outgoing queues. It requires the endpoint's `net:<host>` capability, expands declared keychain secrets only in the `Authorization` header, requires WSS outside loopback and a negotiated subprotocol, and emits `websocket-ready` instead of requiring polling. Connection handles cannot cross plugin or project boundaries and are cleaned up on close, transport failure, plugin stop, or after an abandoned final drain.

Queue overflow explicitly rejects the new frame and closes the connection. It never reports a refused frame as accepted and never grows retained memory without a bound. The Rust SDK and `MockHost` expose the same connect/send/drain/close lifecycle for downstream tests.

Document subscriptions are intentionally excluded from this PR and live on the stacked [`SorPuti:feat/plugin-document-subscriptions`](https://github.com/SorPuti/openidea/tree/feat/plugin-document-subscriptions) branch.

## How it was verified

Passed:

- `cargo test -p ide-plugin --lib` — 25 passed
- `cargo test -p ide-host --lib` — 347 passed
- `cargo test -p ide-host --lib plugins::network_inbox::tests` — 8 passed
- `cargo test -p ide-host --lib plugins::websocket::tests` — 4 passed
- `cargo test --manifest-path sdk/rust/openidea-plugin/Cargo.toml` — 59 passed
- `cargo clippy -p ide-host -p ide-plugin --all-targets --no-deps -- -D warnings -A clippy::nonminimal-bool` — passed; the single allowed lint is already present in untouched `main` files under Rust 1.96
- `cargo fmt --all --check`
- `pnpm test` — 798 files and 9,741 tests passed; 6 skipped

Also executed:

- `cargo test -p ide-host -p ide-plugin` — both packages compiled; this 4.9 GiB runner exhausted `/dev/shm` while linking all `ide-host` integration binaries concurrently, so the package library suites above were run sequentially
- `cargo clippy -p ide-host -p ide-plugin --all-targets -- -D warnings` — reached only pre-existing `clippy::nonminimal-bool` diagnostics in untouched `main` files; the scoped command above passed
- `pnpm gen:bindings` — this runner lacks the GTK/WebKit system development packages required to compile the Tauri export test; the checked-in bindings match the split Rust types and the normal CI environment should regenerate/verify them

There are no user-visible UI changes. Relay authentication models, collaboration protocols, document ownership, and CRDT behavior remain downstream concerns.

## Review focus

Please review the capability and project boundaries, bounded queue/refusal contract, readiness and close lifecycle, WIT compatibility, and SDK/host parity. The new `tokio-tungstenite` runtime dependency remains subject to maintainer approval.
