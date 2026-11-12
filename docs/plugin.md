# OpenIDEA Collab

Experimental realtime collaboration for the authenticated relay at **collab.synthosia.com**. This package is not approved for marketplace listing and requires OpenIDEA v0.1.43 or newer (API 1.6).

## What this package does

- Prepare a relay session after explicit consent.
- Accept a single-use invitation for the configured relay.
- Create editor invitations as the session owner.
- Leave/revoke participation, closing the room when its owner leaves.
- Bind prepared session metadata to the current project identity.
- Store the member credential in the IDE's declared OS-keychain secret, never project files or plugin key-value storage.
- Open one project-bound WSS connection through the IDE host.
- Share only documents selected explicitly by each participant.
- Forward committed UTF-16 document changes through a bounded CRDT protocol and apply remote text with version checks.
- Persist project-bound checkpoints and pending updates in private plugin storage for recovery.

The plugin does not subscribe to file-save events, scan the project, restore the old global room, launch a helper, or expose arbitrary filesystem writes. A document event subscription is installed only for the exact project-relative paths the user shared. The host refuses symlinks, VCS/IDE control paths and common secret files on every document read and remote apply.

## Installation

Install a reviewed, signed `.oiplugin` using Settings → Plugins. A collaborator needs no compiler or extra executable. For development, link the staged package explicitly in OpenIDEA v0.1.43 or newer. Marketplace distribution still requires a clean tag, a signed package and maintainer re-review.

## Permissions

- `net:collab.synthosia.com`: HTTPS session creation, invitation redemption, invitation generation and revocation. No wildcard host or remote plaintext HTTP.
- `project.read` and `editor.read`: bind consent to one project and read only explicitly shared open documents, including unsaved editor text.
- `editor.write`: apply a remote merged result through a versioned editor transaction; stale versions change nothing.
- `events`: receive changes only for the exact shared path filters and host WebSocket readiness.
- `storage`: nonsecret session metadata, project identity, shared path list and CRDT recovery checkpoints.
- `secrets`: one declared member credential in the IDE keychain.

The session-control requests contain credentials/invitation secrets where required but no source text or absolute project paths. Source text is sent only as bounded binary CRDT updates after **Share Current Document** confirmation. Installation/activation performs no network requests. Invitation links must be treated as secrets, are single-use and expire after five minutes by default. Each room is capped server-side at three members total (owner plus at most two collaborators). The current package supports one prepared project session and at most 32 explicitly shared documents at a time.

The relay operator can inspect transient traffic after TLS termination; no E2EE is claimed. The relay stores authorization/document IDs but does not persist content. A slow client is disconnected rather than allowed to grow an unbounded queue; reconnect performs state-vector synchronization with an online peer.

When leaving, a failed server revocation retains local session information and reports an error. The host removes plugin private storage on uninstall; it is not a backup. MIT license and third-party notices accompany the package.
