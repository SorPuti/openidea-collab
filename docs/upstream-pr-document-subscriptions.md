Depends on #2544. Relates to #2532.

## What and why

This adds explicit document subscriptions on top of the host-owned WebSocket API in #2544. A plugin subscribes one project-bound connection to selected open documents, receives committed versioned UTF-16 changes through the bounded socket queue, and can apply version-checked edits back to an open document.

The subscription boundary is deliberately narrow: project-relative path filters are explicit; VCS/IDE control directories, common secret files, non-regular files, and every symlink component are rejected. Subscriptions are removed on unsubscribe, disconnect, plugin stop, or transport failure. Stale edits fail without changing the editor.

The change includes the editor-to-plugin event bridge, WIT and Rust SDK APIs, mock-host behavior, generated TypeScript bindings, lifecycle cleanup, and documentation. It does not add a collaboration-specific protocol or relay policy.

## How it was verified

After rebasing onto merged API 1.6 (`180fed4`):

- `cargo test --manifest-path sdk/rust/openidea-plugin/Cargo.toml` — 62 passed, including document/WebSocket mock and API freeze/WIT coverage tests
- `pnpm test --run src/editor/core/model/document.test.ts` — 27 passed
- rebuilt all 43 checked-in official/fixture components; `plugins/build.sh --check` passed
- downstream Collab's `src/tooling/check.sh` — 5 release-guard tests and 37 Rust tests passed; Clippy, formatting and the optimized `wasm32-wasip2` plugin build passed
- `cargo fmt --all --check`
- `pnpm format:check` — 2,629 files passed

Before the version-only rebase, `cargo test -p ide-host --lib plugins::websocket::tests` passed all 6 transport/subscription tests and the complete frontend suite passed 9,741 tests in 798 files (6 skipped).

The new GitHub Actions runs are waiting for the repository's fork-workflow approval.

This is draft while #2544 and its `tokio-tungstenite` dependency are under review.
