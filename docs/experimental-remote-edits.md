# Experimental remote document edits

This development branch enables the `experimental-remote-edits` Cargo feature by default, requests `editor.write`, and requires host API 1.8 (OpenIDEA 0.1.45 or newer). The signed v0.4.10 package remains outbound-only. Build with `--no-default-features` to retain the refusal behavior.

The candidate stages CRDT updates, refuses a mismatch with unprocessed local content, applies the smallest single versioned editor edit, checkpoints the CRDT and sends the peer ACK only after the host write and checkpoint succeed. Different nonempty initial content is preserved. These properties currently have mock-host tests, not real multi-IDE acceptance. Background edits in the host API are non-undoable.

The released OpenIDEA v0.1.45 host checks background document edit paths for links and control directories. These checks are performed before the editor operation; path replacement races and the complete save path remain unverified here.

Before distributing this build: review `editor.write`; test link/control-path and ancestor-race refusal through the actual host and save path; test failed checkpoint/write, concurrent changes, reconnect, undo expectations and UTF-16/IME; run an isolated two-machine IDE session and security review. Do not interpret SDK mock tests or a compiled WASM as proof of those items.

Focused checks:

```sh
CARGO_BUILD_JOBS=1 cargo test --locked -p openidea-plugin-openidea-collab
CARGO_BUILD_JOBS=1 cargo test --locked -p openidea-plugin-openidea-collab --no-default-features
```
