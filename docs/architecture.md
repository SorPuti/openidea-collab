# Architecture

All source is in `src/`; root files describe the workspace and repository ownership. Product modules are independent of documentation, deployment configuration and GitHub metadata.

| Component | Responsibility |
| --- | --- |
| `src/protocol` | Typed HTTPS schemas, exact-origin invitation parsing, bounded binary framing |
| `src/core` | Yrs UTF-16 documents, incremental updates, bounded pending queue, integrity/project-bound checkpoints, chunk reassembly and lexical sharing policy |
| `src/relay` | Authentication/authorization, invitation lifecycle, opaque byte fanout and small persisted authorization metadata |
| `src/plugin` | Native UI, project-bound WSS runtime, exact document subscriptions and versioned editor adapter |
| `src/tooling` | Checks, packaging, release validation and automation tests |

## Data plane

The intended path is native editor → plugin CRDT → host-owned background WSS → relay → peer host/plugin/editor. No local helper, tunnel or system service is needed on collaborator machines. The relay does not depend on the CRDT crate and never reconstructs a document.

Each received update is validated by the client core. Application fanout shares one immutable byte allocation across recipients. Per-recipient queues, rates and connection permits are bounded. Metadata commits run on a blocking task; the networking runtime uses one async worker. No database, broker or compression context per connection is required.

The core stages a CRDT document before accepting remote changes so a malformed or oversized update cannot corrupt the previous state. Local committed editor changes use one batched CRDT transaction. This prioritizes refusal safety; production keystroke latency still needs profiling in real IDE sessions.

## Safety and lifecycle

The plugin consumes the generic host implementation released in OpenIDEA v0.1.43. It has no file-save subscription or arbitrary project-write capability. It subscribes only to exact explicitly shared document paths. The development build applies incoming updates; OpenIDEA v0.1.44 lacks the later no-follow resolver, and linked paths remain a material risk. The unsafe legacy REST file store and automatic global-room restoration remain removed.

The local project identity is hashed for storage and never sent to the relay. All session and connection operations require that project; a project switch requires leaving explicitly. The plugin's lexical policy limits what can be proposed, while the host's repeated shared-document resolver enforces the filesystem boundary.

Versioned local document events and nonfocused open documents are implemented. Remote application is experimental; see [experimental-remote-edits.md](experimental-remote-edits.md).

## Durability

Relay metadata contains token digests, UUIDs, roles, expiry and registered document IDs only. It is committed through private temp files, fsync and atomic replacement; no success response is sent before the metadata commit succeeds. Document traffic does not write disk.

The client checkpoint includes CRDT state, pending update IDs, a generation, local project identity and integrity digest. The plugin stores it in private host storage before sending a peer ACK and resends pending updates after reconnect. The relay never acknowledges content as durable and retains no offline document backlog; durability is limited to the semantics of host private storage.
