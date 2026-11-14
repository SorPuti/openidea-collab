# Implementation status

Target domain: **collab.synthosia.com**. Source version: plugin 0.4.11-dev.1; signed package version: 0.4.10; relay 0.2.0. The relay must return `/healthz` 204 before a session can be prepared; a CDN 522 means its origin needs operator attention. This is a short functional check, not a soak test or security audit.

| Workstream | Status |
| --- | --- |
| Source/tooling under `src/`, one workspace and lockfile | Implemented |
| Bounded binary protocol and exact-origin invitations | Implemented and tested |
| HTTPS session/invitation/member/document authorization | Implemented and tested locally and against the live service |
| Opaque WSS relay, three-member rooms, revocation and bounded queues | Implemented; local integration/resource tests pass; live authenticated forwarding observed |
| Metadata-only relay recovery; no bearer/content persistence | Implemented and tested |
| UTF-16 Yrs core, bounded chunks, project-bound checkpoints/outbox | Implemented and tested |
| Open-document collaboration | Project-scope confirmation permits eligible open files and future open events; exact change filters, no closed-file scan |
| Local editor change → CRDT → host queue → WSS | Implemented; SDK integration test proves the plugin flow and upstream host tests prove bounded queue delivery |
| WSS → local editor transaction | Enabled in the development source on OpenIDEA 0.1.45 or newer; mock-tested, not validated with two real IDEs. Signed 0.4.10 remains outbound-only |
| Initial mismatch/stale write protection | Implemented: local content is preserved, conflict surfaced, refused work retained for recovery |
| Project/session isolation and keychain credential | Implemented and tested |
| Host API | API 1.6 is released in [OpenIDEA v0.1.43](https://github.com/l0ng-ai/openidea/releases/tag/v0.1.43), including [#2544](https://github.com/l0ng-ai/openidea/pull/2544) project-bound WSS and [#2610](https://github.com/l0ng-ai/openidea/pull/2610) project-relative document events/versioned editor access. The plugin pins the official tag. |
| Native helper/companion | No longer required by the selected architecture; prototype remains for research only |
| Marketplace package | Signed 0.4.10 registered; the experimental development source is not a signed release |
| Public admission/antiabuse and independent security audit | Operator/review work remains |
| Long-running internet/TLS soak and real multi-IDE UI evidence | Pending |

The active implementation keeps the OpenIDEA contribution generic. Its WSS transport, document subscriptions, queue and mock are usable by any plugin. Relay authentication, CRDT envelopes, session limits and collaboration UI remain in this repository.

The original historical v0.1.0 tag remains unchanged and must not be submitted again. Release evidence must be produced from a new clean tag after the required OpenIDEA API is officially available.
