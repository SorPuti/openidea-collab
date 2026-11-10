# OpenIDEA Collab

Experimental session preparation for the authenticated relay at **collab.synthosia.com**. This package is not approved for marketplace listing.

## What this package does

- Prepare a relay session after explicit consent.
- Accept a single-use invitation for the configured relay.
- Create editor invitations as the session owner.
- Leave/revoke participation, closing the room when its owner leaves.
- Bind prepared session metadata to the current project identity.
- Store the member credential in the IDE's declared OS-keychain secret, never project files or plugin key-value storage.

**Native realtime editing is not available on the pinned OpenIDEA API 1.4.** This package does not subscribe to file-save events, upload content, restore the old global room, write remote files, launch a process, or claim WSS support in a host that lacks it. The relay implements the WebSocket endpoint for future host integration.

## Installation

Install a reviewed, signed `.oiplugin` using Settings → Plugins. A collaborator needs no compiler or extra executable. Marketplace distribution requires the outstanding security and host integration work and a maintainer's approval. For development, build this repository and link its staged package explicitly.

## Permissions

- `net:collab.synthosia.com`: HTTPS session creation, invitation redemption, invitation generation and revocation. No wildcard host or remote plaintext HTTP.
- `project.read`: obtain the current project root to bind consent; this version does not read file contents.
- `storage`: nonsecret, versioned session metadata and local project identity hash.
- `secrets`: one declared member credential in the IDE keychain.

The session-control requests contain credentials/invitation secrets where required but no source files or absolute project paths. Installation/activation performs no network requests. Only explicit commands contact the relay. Invitation links must be treated as secrets, are single-use and expire after five minutes by default. The current package supports one prepared project session at a time.

The relay operator can inspect network traffic after TLS termination; no E2EE is claimed. The service domain being configured in the package does not imply that a server is already deployed.

When leaving, a failed server revocation retains local session information and reports an error. The host removes plugin private storage on uninstall; it is not a backup. MIT license and third-party notices accompany the package.
