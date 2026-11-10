# Implementation status

Target domain: **collab.synthosia.com**. Source version: plugin 0.3.0 (unreleased), relay 0.2.0. The live domain responded on 2026-11-10: `/healthz` returned 204 and `/v1/info` advertised protocol 1 and the WSS endpoint. This proves public HTTP reachability only, not authenticated WSS or collaborative editing. No signed release or marketplace entry was deployed/updated by this implementation.

| Workstream | Status |
| --- | --- |
| All implementation/tooling under `src/`, workspace and one lockfile | Implemented |
| Binary bounded protocol and exact-origin invitations | Implemented |
| HTTPS session/invitation/credential lifecycle | Implemented and tested locally |
| Opaque WebSocket relay, roles, document IDs, revocation and bounds | Implemented and tested locally over loopback WS; live domain serves HTTPS; authenticated WebSocket upgrade still needs end-to-end validation |
| Metadata-only recovery, no bearer/content persistence | Implemented and tested locally |
| Yrs UTF-16 core, checkpoint integrity/project binding and chunk assembly | Implemented standalone; not connected to the native IDE editor |
| Native plugin session preparation and keychain storage | Implemented with SDK mock tests |
| No unsafe legacy upload/write activation | Implemented; old routes and automatic room restore removed |
| Simple signed package installation | Packaging implemented; local official CLI validation required and recorded in validation results |
| Native realtime transport | Experimental native companion crate builds with pinned WSS/TLS; not yet launched by plugin or validated against live authenticated service |
| Native document realtime | Blocked on host document subscriptions; no filesystem polling/watcher and no remote writes enabled |
| Host API integration | Contract/proposed PR cuts documented; native events relate to upstream #466 (`needs-decision`); WSS request prepared but publishing was refused with integration 403. Bounded inbox prototype implemented/tested separately; background worker, public API and official release pending |
| Relay process memory | Local 16-connection synthetic check measured about 7.6 MiB sampled RSS; TLS/IDE/internet/soak remain untested |
| Optimized native CRDT hot path | Pending profiling/rework of full-state staging before integration |
| Durable client recovery/ACK and actual IDE undo | Pending host persistence/document adapters and real IDE tests |
| Full project transfer, file operations, no-follow writes | Disabled/pending; lexical policy alone is insufficient |
| Public admission/antiabuse and independent security audit | Pending operator policy/review |
| Domain HTTPS and health endpoint | Observed live; authenticated WSS, abuse controls, monitoring and platform acceptance pending |
| Marketplace re-review | Pending all applicable blockers and a new signed tag/package |

The archived custom-editor/helper experiment remains preserved in its tag; it was not merged into this architecture. The original historical v0.1.0 tag remains unchanged. A new UI or relay test does not retroactively validate that package.

Native runtime progress and integration order: [runtime-integration.md](runtime-integration.md).

Detailed PR sequence, security evidence and license plan: [implementation-plan.md](implementation-plan.md).
