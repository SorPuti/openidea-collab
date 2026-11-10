# Public host integration proposal

## Status and evidence

The implementation is pinned to OpenIDEA SDK v0.1.40 (API 1.4). The upstream main inspected at `c25459317bbc44a317fa203b9dc2f6074b7ab625` has API 1.5 in development, but no public background WebSocket interface or general native document-change subscription. Its `net:ask` applies to HTTP/HTTPS, not WSS. The webview CSP blocks external network access. WIT editor access is focused-window based; custom web editors expose a separate versioned document bridge.

The current control-plane package deliberately does not call private APIs, download a helper, weaken CSP, run an infinite fetch or use the sixty-second scheduler as realtime transport. The feature is visibly gated. The relay is ready to exercise with native test clients; this is not equivalent to two IDEs editing.

The machine-readable contract sketch is [src/host-api/proposal.wit](../src/host-api/proposal.wit). It is an independent **proposal namespace**, not a supported OpenIDEA SDK or a patched runtime. It is not imported into the current component. Upstream must choose the real version/capabilities and approve the implementation before an official compatible release can exist.

## Proposed upstream PRs

1. **Native document subscription and transactions.** Add opaque document handles scoped to plugin/project, incremental UTF-16 changes, previous/new versions, origin and selection. Support nonfocused documents and closed-project rejection. Remote changes must map local undo history without adding another user's edits as local undo steps. Stale application is atomic and must reconcile before retry; IME, surrogate pairs, CRLF and concurrent Undo/Redo are required tests.
2. **Host-owned background WSS.** HTTPS/WSS origin validation, exact-host grants, keychain header substitution and a required subprotocol. Worker lifetime is separate from a five-second WASM call and must survive/rebuild across idle reclamation. Coalesce ready events, bound incoming/outgoing queues, provide nonblocking enqueue/drain, and close immediately on grant revocation/project closure/disable. Host messages must carry correct invoking-project context; a plugin must not drain another project's queue. No direct iframe socket or unrestricted native process.
3. **Recovery contract.** First test existing private-directory APIs. If their semantics do not satisfy the required crash/disk/power-loss guarantee, add a small compare-generation checkpoint primitive with a returned durability level. Do not silently treat unsupported fsync or successful rename as full persistence. Data limits and rollback/upgrade/uninstall behavior must be explicit.
4. **No-follow shared-file access.** Before project transfer, introduce scoped host primitives that reject symlinks/reparse points through every path component and avoid replacement races. Existing canonicalization follows internal symlinks. Reads and writes need the same sharing boundary; hardlinks, dirty buffers, special files, reserved names and filesystem collisions need platform tests.

Each PR belongs in `l0ng-ai/openidea` with WIT, SDK/mock, host/runtime, grants, documentation, compatibility tests and real IDE evidence. Do not mutate frozen released records/variants; add new interfaces/types or existing extensible event kinds according to upstream rules. This repository has not created or merged any upstream API PR or claimed a release version containing these features.

## Reproduction and acceptance

- Compile the current package against v0.1.40 and inspect `openidea-plugin check`: it is API 1.4, has no project-write/file-save activation and declares only its real HTTPS/control permissions.
- Inventory `wit/io.wit` (`net.fetch` returns one bounded body), `wit/events.wit`, `wit/workspace.wit`, `crates/ide-plugin/src/actor.rs` and the webview CSP in `src-tauri/src/plugin_web.rs`.
- An installed prototype must demonstrate remote packets while idle, native edits while unfocused, reclamation/restart, permission refusal/revocation, project changes, bounded queue overflow and concurrent local edits.
- Complete two-IDE tests before enabling realtime in this package or asking the marketplace reviewer to approve it. The prepared session UI and standalone tests are explicitly partial evidence.

## Protocol integration after host support

The plugin core translates native changes to bounded Yrs updates, stores a checkpoint before advertising local durability, sends binary frames through the host, and applies authenticated peer updates to the correct versioned document. A peer ACK is emitted only after the receiving checkpoint is committed at the advertised level. The relay only routes; it cannot issue that ACK on the peer's behalf.

The current CRDT staging implementation must be profiled before it enters the keystroke path. Chunk assembly and queues are bounded, but full-state staging is not an optimized native adapter. Optimizations must preserve refusal/no-loss behavior with regression coverage.
