Depends on #2544. Relates to #2532.

## What and why

This adds explicit document subscriptions on top of the host-owned WebSocket API in #2544. A plugin subscribes one project-bound connection to selected open documents, receives committed versioned UTF-16 changes through the bounded socket queue, and can apply version-checked edits back to an open document.

The subscription boundary is deliberately narrow: project-relative path filters are explicit; VCS/IDE control directories, common secret files, non-regular files, and every symlink component are rejected. Subscriptions are removed on unsubscribe, disconnect, plugin stop, or transport failure. Stale edits fail without changing the editor.

The change includes the editor-to-plugin event bridge, WIT and Rust SDK APIs, mock-host behavior, generated TypeScript bindings, lifecycle cleanup, and documentation. It does not add a collaboration-specific protocol or relay policy.

## Validation

- `cargo test -p ide-plugin --lib` — 27 passed
- `cargo test -p ide-host --lib plugins::websocket::tests` — 6 passed
- `cargo test --manifest-path sdk/rust/openidea-plugin/Cargo.toml --lib background_` — 2 passed
- `pnpm exec vitest run src/editor/core/model/document.test.ts` — 27 passed
- `pnpm test` — 798 files and 9,741 tests passed; 6 skipped
- `cargo fmt --all --check`

This is draft while #2544 and its `tokio-tungstenite` dependency are under review.
