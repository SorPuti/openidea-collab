# Installation and URLs

## Collaborators

The goal is one reviewed, signed package in OpenIDEA Settings → Plugins, followed by Prepare/Create Session or Accept Invitation. No external installer, Rust, CLI, native helper, server, VPN or tunnel is needed on a collaborator's computer. Installing the plugin does not transmit files.

The 0.4.0 release candidate provides realtime in OpenIDEA v0.1.43 or newer. Marketplace installation must still wait for multi-IDE acceptance tests, a signed package and registry review. The hosted relay is active, but service availability does not imply marketplace approval.

The owner prepares a session, generates an invitation, and sends the link. The recipient pastes it into the native plugin dialog and confirms the relay. The link is exact-origin validated and redeemed once. Each participant then focuses the same project-relative document and confirms **Share Current Document**; unsaved text is included. Nothing else in the project is scanned or uploaded.

A production package is installed by the IDE using the registered publisher key. The host handles versioned installation and package rollback. The old root `install.sh` was removed: a source script that compiles code, edits shell startup files and installs a login service is not the collaborator installation experience.

## Developers

Use the repository Rust toolchain and the official OpenIDEA v0.1.43 CLI:

```sh
./src/tooling/package-dev.sh
openidea-plugin link target/plugin-package
```

This stages the WASM, manifest, translations, root LICENSE/NOTICE and package README outside `src/`. Linking is development-only and bypasses publisher verification.

### Self-hosted development package

The reviewed marketplace package is deliberately restricted by its `net:collab.synthosia.com` capability. A setting alone cannot safely grant a package permission to contact arbitrary domains. To link a development package for one self-hosted relay, build it with that exact DNS hostname:

```sh
COLLAB_RELAY_HOST=collab.example.org ./src/tooling/package-dev.sh
openidea-plugin link target/plugin-package
```

Then set **OpenIDEA Collab → Relay origin** to `https://collab.example.org`. The hostname in the setting must match the package capability. This does not run a local executable from the plugin; run `collab-server` as a separately managed loopback service behind TLS, as described in [operations](operations.md). A signed self-hosted distribution needs its own reviewed manifest and publisher process.

## Operators

Only the relay operator installs a server and TLS proxy. See [operations.md](operations.md). The public domain currently serves the protocol; operators still own monitoring, admission controls and incident response.

## Updates and removal

Reuse OpenIDEA's update and rollback flow. A future data migration needs its own reversible schema; package rollback does not automatically roll back session data. Leaving a session revokes membership remotely; if the relay is unavailable the current plugin retains credentials and reports the failure rather than pretending revocation succeeded.

The IDE deletes plugin private storage on uninstall. Do not promise that pending editing state survives uninstall. Export/leave UX and any cancellable uninstall requirement need host integration and acceptance tests before editing is enabled.
