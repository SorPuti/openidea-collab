# Development remote editing build

The `0.4.7-dev.1` source enables inbound edits by default and requests `editor.write`. It is a development build; it has not passed actual two-IDE acceptance and is not the signed Marketplace release. The old refusal behavior can be built with `--no-default-features`.

The user explicitly requested activation before a fixed official OpenIDEA release. OpenIDEA v0.1.44 does not include [upstream PR #3236](https://github.com/l0ng-ai/openidea/pull/3236). The old host does not enforce the later no-follow background-edit protection; linked paths or replacement races can cause edits to affect unintended content. Do not use real projects for validation. Remote edits are non-undoable in the host API.

The candidate stages CRDT updates, preserves mismatching local content, applies a minimal versioned editor edit and checkpoints before sending ACK. These properties are covered by mock-host tests, not a crash-durability or full IDE guarantee. There is no whole-project synchronization.

Before distributing a signed release: provide an enforceable host capability/version gate, use a released host containing the fix, validate symbolic link/control-path and ancestor races through the host and save path, test checkpoint/write failure and concurrent edits, and complete isolated multi-IDE acceptance.
