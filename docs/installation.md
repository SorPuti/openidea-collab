# Installation and URLs

## Collaborators

The goal is one reviewed, signed package in OpenIDEA Settings → Plugins, followed by Prepare/Create Session or Accept Invitation. No external installer, Rust, CLI, native helper, server, VPN or tunnel is needed on a collaborator's computer. Installing the plugin does not transmit files.

The current package prepares relay sessions only. It explicitly reports that native realtime needs public host APIs that the pinned release does not provide. A marketplace package must not advertise live editing before those APIs and the two-IDE acceptance tests pass. This domain configuration is not evidence of an active hosted service.

The owner prepares a session, generates an invitation, and sends the link. The recipient pastes it into the native plugin dialog and confirms the relay. The link is exact-origin validated and redeemed once. Opening it in a browser displays a minimal copy page without tracking. A deep link that launches an IDE plugin command is not implemented or claimed.

A production package is installed by the IDE using the registered publisher key. The host handles versioned installation and package rollback. The old root `install.sh` was removed: a source script that compiles code, edits shell startup files and installs a login service is not the collaborator installation experience.

## Developers

Use the repository Rust toolchain and the official OpenIDEA CLI v0.1.40:

```sh
./src/tooling/package-dev.sh
openidea-plugin link target/plugin-package
```

This stages the WASM, manifest, translations, root LICENSE/NOTICE and package README outside `src/`. Linking is development-only and bypasses publisher verification.

## Operators

Only the relay operator installs a server and TLS proxy. See [operations.md](operations.md). DNS, hosting and public deployment have not been performed by this code change.

## Updates and removal

Reuse OpenIDEA's update and rollback flow. A future data migration needs its own reversible schema; package rollback does not automatically roll back session data. Leaving a session revokes membership remotely; if the relay is unavailable the current plugin retains credentials and reports the failure rather than pretending revocation succeeded.

The IDE deletes plugin private storage on uninstall. Do not promise that pending editing state survives uninstall. Export/leave UX and any cancellable uninstall requirement need host integration and acceptance tests before editing is enabled.
