# OpenIDEA host integration

[OpenIDEA v0.1.43](https://github.com/l0ng-ai/openidea/releases/tag/v0.1.43) releases the generic WSS transport from [#2544](https://github.com/l0ng-ai/openidea/pull/2544) and the document events/versioned editor access from [#2610](https://github.com/l0ng-ai/openidea/pull/2610); both follow [issue #2532](https://github.com/l0ng-ai/openidea/issues/2532). This plugin pins that official SDK release.

## Public surface

API 1.6 adds project-bound `WebSocket::{connect, send, drain, close}`, `Project::document`, versioned document edit methods, exact path event subscriptions, and typed `document-changed`/`websocket-ready` events. The plugin owns the collaboration wire protocol: it receives project-relative editor changes, encodes its own frames and sends them through the generic socket.

The host owns transport tasks and queues. It requires WSS outside loopback, one declared secret-backed Authorization header, a granted exact network host and a negotiated subprotocol. Handles cannot be used from another project. Incoming and outgoing queues are bounded by frame count and bytes; overflow rejects the new frame, reports it as refused and closes the connection for resynchronization. Readiness notifications are coalesced, so no scheduled polling is required.

Document access is limited to explicitly named open regular files. Reads and remote applications repeat the shared-document resolver checks, reject symlinks, VCS/IDE control directories, hooks and common secret files, and require project/editor/event grants. Edits use expected versions and UTF-16 ranges; stale writes are atomic refusals, do not enter Undo history and are not re-emitted as local changes.

## Open-source boundary

The upstream implementation is not tailored to this repository. It contains no relay domain, room code, role, CRDT, invitation or three-member assumption. Its mock can test any plugin that uses background WebSockets and document subscriptions. OpenIDEA Collab owns its URL, authenticated protocol, explicit-share confirmation, Yrs state and relay behavior.

## Evidence and remaining acceptance

The upstream branch includes queue race/overflow tests, real loopback WebSocket transport tests, subscription isolation/cleanup, versioned editor tests, project-crossing refusals, SDK WIT coverage and a functional WebSocket mock. The plugin test uses those public wrappers to prove local document event → CRDT → WebSocket and inbound frame → versioned editor → ACK.

Before an official plugin release, perform a real IDE acceptance pass with two/three profiles covering idle runtime reclamation, Unicode/IME, Undo/Redo, project close, grant revocation, reconnect, conflicts and bounded slow-client behavior.

The independent proposal file under `src/host-api/` and the companion are retained as historical design artifacts. They are not used by the active plugin.
