# Experimental remote document edits

This branch prepares the incoming-edit path behind the opt-in Cargo feature `experimental-remote-edits`. The default build continues to refuse remote updates and does not request `editor.write`. The feature is for isolated tests only: **do not publish or install a feature-enabled build** while the host version cannot be checked and the release and acceptance work below remains incomplete. Enabling the feature by itself does not change the package manifest to request `editor.write`.

The candidate stages CRDT updates, refuses a mismatch with unprocessed local content, applies the smallest single versioned editor edit, checkpoints the CRDT and sends the peer ACK only after the host write and checkpoint succeed. Different nonempty initial content is preserved. These properties currently have mock-host tests, not real multi-IDE acceptance. Background edits in the host API are non-undoable.

The upstream no-follow work was merged in [OpenIDEA PR #3236](https://github.com/l0ng-ai/openidea/pull/3236), but it was not present in the latest official `v0.1.44` release when checked on 2026-11-13. Both the older and newer hosts report the same plugin API version, so the plugin cannot safely use that API version to distinguish them. The resolver checks existing path components for symlinks/reparse points; adversarial replacement races and the complete save path remain unverified here.

Before enabling this in a distributable build: obtain a released host with the fix and a trustworthy enforceable host capability/version gate; request and review `editor.write`; test link/control-path and ancestor-race refusal through the actual host and save path; test failed checkpoint/write, concurrent changes, reconnect, undo expectations and UTF-16/IME; run an isolated two-machine IDE session and security review. Do not interpret SDK mock tests or a compiled WASM as proof of those items.

Focused checks:

```sh
CARGO_BUILD_JOBS=1 cargo test --locked -p openidea-plugin-openidea-collab --features experimental-remote-edits
CARGO_BUILD_JOBS=1 cargo test --locked -p openidea-plugin-openidea-collab
```
