Closes #2532.

## What and why

WASM plugins that consume realtime services currently have no bounded background transport, and document integrations cannot observe or apply explicit versioned changes outside a synchronous command. This adds a generic API 1.5 host surface for project-bound WSS connections and explicitly scoped open-document streams.

The host owns the socket and bounded queues, expands declared keychain secrets in the Authorization header, requires WSS outside loopback and a negotiated subprotocol, coalesces readiness events, and closes on overflow instead of reporting a refused frame as delivered. Connection handles cannot cross project boundaries and are cleaned up with the plugin/transport lifecycle.

Plugins can subscribe to exact project-relative document paths, receive committed UTF-16 changes, read a versioned open document and apply expected-version edits. Shared-document resolution rejects symlinks, IDE/VCS control paths, hooks and common secret files on access and apply. No filesystem polling or arbitrary project write API is added.

The Rust SDK and `MockHost` expose the same behavior. The mock records connections/outbound frames, queues inbound frames, models close/drain and enforces network, secret and project grants so downstream plugins can test the real public wrappers.

## How it was verified

- `cargo test -p ide-host document_stream --lib` — 6 passed
- bounded inbox suite — 8 passed, including count/byte overflow, explicit refusal, partial drain, close and budget recovery
- `cargo test --manifest-path sdk/rust/openidea-plugin/Cargo.toml` — 59 passed
- `cargo clippy --manifest-path sdk/rust/openidea-plugin/Cargo.toml --all-targets -- -D warnings`
- `cargo check --manifest-path sdk/rust/openidea-plugin/Cargo.toml --target wasm32-wasip2`
- a downstream WASM plugin using this API built successfully and passed `openidea-plugin check` as API 1.5

There are no user-visible UI changes. The API is generic; relay authentication models, collaboration protocols and CRDT behavior remain downstream concerns.

## Review focus

Please review the project/permission boundaries, refusal and cleanup semantics, WIT compatibility, exact document scope and the SDK/host parity.
