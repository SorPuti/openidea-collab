# Native runtime integration: work in progress

The IDE currently has no public background WebSocket contract suitable for this
plugin. The draft WIT is a proposal, not an available capability. The plugin
therefore continues to prepare sessions without claiming native realtime.

## First implementation slice

`src/host-api/prototype/network_inbox.rs` is the transport-independent inbox
prototype for the IDE host. It is not linked into the shipped plugin or relay.
A matching local patch exists on OpenIDEA branch `feat/plugin-background-network`.
Run `./src/tooling/check-host-inbox.sh` to compile and test it without building
Tauri/Wasmtime. This does not validate the full IDE build.

The inbox bounds both message count and aggregate payload bytes, enforces a
frame limit, coalesces wake-ups and preserves accepted message order. Overflow
marks the connection closed instead of silently dropping a CRDT update. The
consumer must continue draining while `more` is true; after draining, a new
producer rearms notification under the same lock. Callbacks run outside the lock.
The producer must enforce frame limits before allocating network frames: the
inbox alone cannot bound an allocation already made by its caller.

Four tests cover notification rearming, each overflow bound, idempotent close,
and concurrent producers with per-producer order preservation.

## Remaining integration order

1. Add a native asynchronous WSS worker with bounded outgoing and incoming
   queues. Enforce approved origins, loopback exception, TLS verification,
   forbidden browser-style headers, secret substitution, connect timeout and
   frame limits before allocation. Never log credentials or invitation secrets.
2. Route coalesced readiness directly to the plugin actor. Existing upkeep runs
   every few seconds and scheduled activation is not a realtime delivery path.
   Specify rejection/retry behavior if the actor queue cannot accept a wake-up.
3. Own handles by plugin and project. Define idle-instance reactivation,
   project-close cleanup, grant revocation, unload, cancellation and task joining.
   Test closure while connect/receive/send is pending.
4. Extend only the draft API version and mirror its WIT/SDK definitions. Preserve
   released API signatures. Add native mocks and integration tests, then compile
   the actual host; standalone inbox tests cannot replace these checks.
5. Add native document subscriptions and versioned UTF-16 transactions, including
   nonfocused documents. Mark remote transactions outside local undo history;
   local undo must generate an ordinary local change. Resolve upstream #466.
6. Connect the Yrs engine, persist recovery/outbox through a durable host adapter,
   handle reconnection/state-vector exchange and test simultaneous edits,
   project switches, revoked peers, crashes and conflicts in the real IDE.
7. Profile CRDT staging and production TLS/proxy behavior before enabling the
   collaboration UI. Keep publishing gated until all release blockers are met.

No upstream PR, host release, deployed WSS endpoint or native editor integration
is implied by this prototype. Billing/account access is being handled separately
by the repository owner; local work does not depend on remote CI availability.

## Native companion proof (experimental)

`src/companion` is a native Rust WSS transport prototype. It pins the relay URL
and subprotocol, validates system TLS roots, accepts bounded JSON-line commands
on stdin and emits bounded events on stdout. The host/plugin can supervise it
with the OpenIDEA `process.start` child handle. It deliberately has no filesystem
API and never scans the project. Build with `cargo build --locked --release -p
openidea-collab-companion`.

This is not yet packaged or launched by the plugin. The manifest must declare a
pinned `process:<binary>` capability and the user must approve it. The plugin
must pass the member credential over the child's private stdin pipe, not argv or
environment, poll child output without blocking its actor, and terminate it on
leave, project switch, revoke, unload and shutdown. These lifecycle changes and
cross-platform signed binaries remain prerequisites to distributing the helper.

The helper supplies transport only; it does not observe documents or perform
edits. No filesystem watcher or save polling is used. The current editor resource
is obtained in an invocation/view context and provides versioned snapshot/edit
operations, but the released API does not provide a continuous subscription for
all documents/windows. The first reliable editor integration therefore requires
the host document subscription API requested in issue #2532. Until then, do not
claim end-to-end collaboration and do not enable remote writes.

### Companion packaging status

Build the native binary for the current or configured Rust target with
`./src/tooling/build-companion.sh <target-triple>`. The script emits a target
specific binary and SHA256 manifest under `target/companion/`. Example target:
`x86_64-unknown-linux-gnu`. Cross builds require the Rust target and platform
linker; macOS/Windows artifacts must be built and tested on supported runners.
These hashes are integrity records, not signatures. Release signing and a
trusted update/install path are still required before distribution. Do not add
the process capability to the production manifest until the signed executable
can be installed and the marketplace accepts the external binary model.
