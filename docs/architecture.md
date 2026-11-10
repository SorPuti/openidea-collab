# Architecture

All source is in `src/`; root files describe the workspace and repository ownership. Product modules are independent of documentation, deployment configuration and GitHub metadata.

| Component | Responsibility |
| --- | --- |
| `src/protocol` | Typed HTTPS schemas, exact-origin invitation parsing, bounded binary framing |
| `src/core` | Yrs UTF-16 documents, incremental updates, bounded pending queue, integrity/project-bound checkpoints, chunk reassembly and lexical sharing policy |
| `src/relay` | Authentication/authorization, invitation lifecycle, opaque byte fanout and small persisted authorization metadata |
| `src/plugin` | Native OpenIDEA UI and explicit HTTPS session preparation on the released SDK |
| `src/tooling` | Checks, packaging, release validation and automation tests |

## Data plane

The intended path is native editor → plugin CRDT → host-owned background WSS → relay → peer host/plugin/editor. No local helper, tunnel or system service is needed on collaborator machines. The relay does not depend on the CRDT crate and never reconstructs a document.

Each received update is validated by the client core. Application fanout shares one immutable byte allocation across recipients. Per-recipient queues, rates and connection permits are bounded. Metadata commits run on a blocking task; the networking runtime uses one async worker. No database, broker or compression context per connection is required.

The standalone core currently stages a CRDT document before accepting changes to enforce limits without corrupting the previous state. This prioritizes correctness and is **not a demonstrated low-latency native-editor integration**. Measure and reduce snapshot/staging overhead before connecting it to the keystroke path; no end-to-end performance claim is made.

## Safety and lifecycle

The pinned host lacks native document subscriptions and background WebSockets. Therefore the plugin has no file-save subscription and no project-write capability; prepared sessions exchange metadata only. The unsafe legacy REST file store and automatic global-room restoration are removed from the active implementation.

The local project identity is hashed for storage, never sent to the relay. All session operations that can invite or prepare require the same invoking project; a project switch requires leaving/reassociating explicitly. A future file adapter must enforce the lexical allowlist **and** no-follow handles; lexical validation alone does not prove filesystem safety.

Native undo/redo, versioned remote changes, nonfocused documents and persistent host event delivery remain the host API boundary described in [host-api.md](host-api.md).

## Durability

Relay metadata contains token digests, UUIDs, roles, expiry and registered document IDs only. It is committed through private temp files, fsync and atomic replacement; no success response is sent before the metadata commit succeeds. Document traffic does not write disk.

The client core checkpoint includes its CRDT state and pending update IDs, a generation, the local project identity and an integrity digest. A checkpoint in memory is not a durability ACK: the IDE persistence adapter is still required. The relay never acknowledges content as durable. The plugin's current session preparation does not enable the core editing engine or imply offline editing recovery.
