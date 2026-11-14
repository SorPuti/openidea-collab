# Native runtime integration

The native design is implemented across two repositories. [OpenIDEA v0.1.43](https://github.com/l0ng-ai/openidea/releases/tag/v0.1.43) releases API 1.6: [#2544](https://github.com/l0ng-ai/openidea/pull/2544) provides generic WSS transport and [#2610](https://github.com/l0ng-ai/openidea/pull/2610) adds project-relative document events and versioned editor access. This repository consumes that official release for collaboration.

## Generic OpenIDEA layer

The upstream change supplies:

- host-owned WSS with TLS/origin/capability/secret validation;
- project-bound connection handles and lifecycle cleanup;
- bounded incoming/outgoing queues with explicit overflow rejection;
- coalesced `websocket-ready` events instead of polling;
- exact project-relative `document-changed` subscriptions;
- versioned reads and atomic UTF-16 editor edits for explicit open documents;
- project-relative subscriptions and versioned editor APIs; OpenIDEA 0.1.45 checks background edit paths, though races and saving still need testing;
- an SDK mock that records frames, accepts inbound messages, models close/drain and enforces project/network grants.

This layer contains no Synthosia URL, room model, CRDT, member role or OpenIDEA Collab protocol.

## Collab layer

The plugin stores one session against a project identity and opens WSS only after an explicit prepare/join/connect action. Sharing requires confirmation and states that unsaved editor content is transmitted. A project scope can automatically share eligible open documents; each participant must have the same project-relative document open; its relay ID is UUIDv5(session, path). The owner registers that opaque ID with the relay.

The runtime then follows this flow:

```text
versioned document event
  -> Yrs update + private checkpoint/outbox
  -> bounded host WebSocket send
  -> authenticated relay forwarding
  -> bounded host receive queue + websocket-ready
  -> chunk validation/reassembly
  -> staged CRDT update + versioned editor edit + checkpoint
  -> peer ACK only after successful application
```

Local events carry sorted non-overlapping offsets in the original document's UTF-16 coordinates. The core applies a batch in descending order as one CRDT transaction. If event versions skip, the runtime reconciles from the current versioned editor snapshot instead of applying offsets to the wrong base.

The development build applies incoming updates after checking local content and the editor version, then checkpoints before acknowledging. Conflicts and denied writes are refused without ACK. Actual-host path races, saving and real multi-IDE behavior remain unverified. The signed 0.4.10 package still refuses incoming updates.

The relay stores no replay backlog. On reconnect, persisted pending operations are resent and state vectors request missing state from an online authorized peer. Disconnect clears the event path filter; leaving revokes membership and then removes the local credential/session.

## Validation boundary

Unit and integration tests cover the queue, SDK mock transport, project binding, subscriptions, CRDT convergence/checkpoints, relay authorization/backpressure, and the plugin's document-to-WSS-to-document path. Remaining release evidence is a real two/three-IDE session, long-running reconnect/conflict testing, official CLI packaging/signature/checksum, and marketplace re-review.

`src/companion` and `src/host-api/prototype` are retained as historical experiments. The plugin does not launch the companion and requests no process capability.
