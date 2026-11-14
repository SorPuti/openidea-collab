# OpenIDEA Collab

Experimental realtime collaboration for **collab.synthosia.com**, operated by Synthosia. The relay is required for collaboration: if it is unavailable, realtime sharing stops. This development build requires OpenIDEA v0.1.45 or newer (host API 1.8).

## What this package does

- Prepare a relay session after explicit consent.
- Accept a single-use invitation for the configured relay.
- Create editor invitations as the session owner.
- Leave/revoke participation, closing the room when its owner leaves.
- Bind prepared session metadata to the current project identity.
- Store the member credential in the IDE's declared OS-keychain secret, never project files or plugin key-value storage.
- Open one project-bound WSS connection through the IDE host.
- After a one-time project scope confirmation in **Connect Realtime** or **Share All Open Documents**, share the eligible open editors and automatically include eligible files opened later during that session. The plugin does not scan closed files or upload the entire project.
- Forward committed UTF-16 document changes through a bounded CRDT protocol. Incoming changes to shared open documents are applied through the host's versioned background editor API; unsynchronized local content and different nonempty initial documents are preserved and the incoming update is refused.
- Persist project-bound checkpoints and pending updates in private plugin storage for recovery.

The plugin does not subscribe to file-save events, scan the project, restore the old global room, launch a helper, or expose arbitrary filesystem writes. The plugin subscribes to `FileOpened` events to discover eligible open editors and subscribes to document changes for the exact project-relative paths currently shared. The plugin's lexical policy refuses VCS/IDE control paths and common secret names. OpenIDEA 0.1.45 checks background edit paths for links and control directories. Filesystem replacement races and the subsequent save path are not independently verified.

## Installation

Install a reviewed, signed `.oiplugin` using Settings → Plugins. A collaborator needs no compiler or extra executable. For development, link the staged package explicitly in OpenIDEA v0.1.45 or newer. Marketplace distribution still requires a clean tag, a signed package and maintainer re-review.

## Permissions

- `net:collab.synthosia.com`: HTTPS session creation, invitation redemption, invitation generation and revocation. No wildcard host or remote plaintext HTTP.
- `project.read` and `editor.read`: bind consent to one project and read eligible open documents covered by the approved sharing scope, including unsaved editor text.
- `editor.write`: apply incoming changes to shared open editor buffers using version checks.
- `events`: observe `FileOpened` events, document changes for the exact shared path filters, and host WebSocket readiness.
- `storage`: nonsecret session metadata, project identity, shared path list and CRDT recovery checkpoints.
- `secrets`: a member credential and, when required by the relay operator, a room-creation admission token in the IDE keychain.

The session-control requests contain credentials/invitation secrets where required but no source text or absolute project paths. A relay may require an operator-issued admission token to create a room; it is sent only to `POST /v1/sessions` and does not authorize sockets, documents or invitations. Source text is sent as bounded binary CRDT snapshots and updates after explicit sharing confirmation. The project scope confirmation permits eligible open files, including their unsaved text and subsequent edits, to be sent automatically when opened later without another per-file prompt. Installation/activation performs no network requests. Invitation links must be treated as secrets, are single-use and expire after five minutes by default. Each room is capped server-side at three members total (owner plus at most two collaborators). The current package supports one prepared project session and at most 32 shared documents at a time.

## Controlling the automatic scope

A participant approves the automatic scope for the current project and prepared session when confirming **Connect Realtime** or **Share All Open Documents**. It persists in private plugin storage through disconnects, reconnects and IDE restarts, until the participant leaves the session. Disconnecting stops realtime transmission while disconnected but does not erase this approval; leaving the session revokes participation and removes the stored scope. Preparing or joining another session requires a new approval. Eligible future open files can be shared without an additional per-file prompt while connected. Closed files are not scanned, and common secret and control paths are excluded by policy.

Use **Stop Sharing Current Document** and confirm to remove that document from sharing. The decision is stored for this session and prevents automatic re-sharing, including after reconnecting or opening other editors. To share it again, use **Share Current Document** or **Share All Open Documents** and confirm explicitly. Stopping sharing cannot retract data already sent. Closing a tab alone is not a revocation action. To end all sharing and remove the automatic scope, use **Leave Session**; if server revocation fails, the plugin retains the session and reports the failure.

The relay operator can inspect transient traffic after TLS termination; no E2EE is claimed. The relay stores authorization/document IDs but does not persist content. A slow client is disconnected rather than allowed to grow an unbounded queue. An incoming update receives an ACK only after the editor change and checkpoint succeed. Refused conflicts are not acknowledged. Two-IDE acceptance and full-project synchronization remain unverified and are not implemented in this build.

When leaving, a failed server revocation retains local session information and reports an error. The host removes plugin private storage on uninstall; it is not a backup. MIT license and third-party notices accompany the package.
