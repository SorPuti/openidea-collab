# Implementation status

Target domain: **collab.synthosia.com**. Source version: plugin 0.4.6 (unreleased); signed package version: 0.4.4; relay 0.2.0. The relay must return `/healthz` 204 before a session can be prepared; a CDN 522 means its origin needs operator attention. This is a short functional check, not a soak test or security audit.

| Workstream | Status |
| --- | --- |
| Source/tooling under `src/`, one workspace and lockfile | Implemented |
| Bounded binary protocol and exact-origin invitations | Implemented and tested |
| HTTPS session/invitation/member/document authorization | Implemented and tested locally and against the live service |
| Opaque WSS relay, three-member rooms, revocation and bounded queues | Implemented; local integration/resource tests pass; live authenticated forwarding observed |
| Metadata-only relay recovery; no bearer/content persistence | Implemented and tested |
| UTF-16 Yrs core, bounded chunks, project-bound checkpoints/outbox | Implemented and tested |
| Explicit selected-document collaboration | Implemented in the plugin; exact path filters, no scan/watcher/polling |
| Local editor change → CRDT → host queue → WSS | Implemented; SDK integration test proves the plugin flow and upstream host tests prove bounded queue delivery |
| WSS → local editor transaction | Intentionally disabled: released host lacks tested no-follow protection for remote document writes; inbound updates are refused without ACK |
| Initial mismatch/stale write protection | Implemented: local content is preserved, conflict surfaced, refused work retained for recovery |
| Project/session isolation and keychain credential | Implemented and tested |
| Host API | API 1.6 is released in [OpenIDEA v0.1.43](https://github.com/l0ng-ai/openidea/releases/tag/v0.1.43), including [#2544](https://github.com/l0ng-ai/openidea/pull/2544) project-bound WSS and [#2610](https://github.com/l0ng-ai/openidea/pull/2610) project-relative document events/versioned editor access. The plugin pins the official tag. |
| Native helper/companion | No longer required by the selected architecture; prototype remains for research only |
| Marketplace package | Pending clean tag, official build/check/signing, real IDE acceptance evidence and registry re-review |
| Public admission/antiabuse and independent security audit | Operator/review work remains |
| Long-running internet/TLS soak and real multi-IDE UI evidence | Pending |

The active implementation keeps the OpenIDEA contribution generic. Its WSS transport, document subscriptions, queue and mock are usable by any plugin. Relay authentication, CRDT envelopes, session limits and collaboration UI remain in this repository.

The original historical v0.1.0 tag remains unchanged and must not be submitted again. Release evidence must be produced from a new clean tag after the required OpenIDEA API is officially available.
