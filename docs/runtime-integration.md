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
