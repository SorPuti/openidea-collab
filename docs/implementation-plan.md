# OpenIDEA Collab delivery plan

## Completed implementation

- Relay control plane: per-member credentials, role/document authorization, single-use invitations, three-member cap, revocation, bounded WSS forwarding and metadata-only persistence.
- Client core: UTF-16 Yrs updates, bounded transfers/reassembly, project-bound integrity checkpoints and recoverable pending operations.
- Generic OpenIDEA API 1.6 in [v0.1.43](https://github.com/l0ng-ai/openidea/releases/tag/v0.1.43): host-owned project-bound WSS, explicit document subscriptions, versioned editor access, bounded queues and SDK/mock/documentation.
- Plugin 0.4.0 source: project-bound sessions, keychain credential, automatic connection after prepare/join, explicit per-document confirmation, exact event filters, CRDT propagation, remote versioned apply, conflict preservation, checkpoint-before-ACK and reconnect state exchange.
- Evidence: workspace/unit/integration suites, SDK suite, live HTTPS info and authenticated two-client WSS forwarding at `collab.synthosia.com`.

## Plugin acceptance after upstream release

1. Build against the official OpenIDEA v0.1.43 SDK/CLI and preserve the lockfile provenance.
2. Install the staged plugin into three isolated profiles and exercise owner/editor/viewer on disposable matching projects.
3. Validate simultaneous Unicode/IME edits, exact-path isolation, unshared secrets/control paths, stale conflicts, disconnect/reconnect, revocation, project switch/close, idle reclaim, restart and slow-client overflow.
4. Record CPU/RSS/latency and run a longer TLS/proxy soak. Preserve refused/pending updates and verify no silent overwrite or false delivery status.

## Security and operations before marketplace

- Define public session-creation admission/rate policy, alerting, credential rotation, backup/restore and incident response without logging payloads or bearer values.
- Run dependency/license/SBOM checks and an independent review of relay authorization, parser bounds, symlink/control/secret exclusions and checkpoint recovery.
- Confirm the trust statement: TLS protects transit, the operator can inspect transient content, no E2EE/cloud backup/offline backlog is claimed.
- Keep the collaborator installation to one IDE package; the companion stays outside the manifest and no process grant is requested.

## Release and re-review

After the official SDK migration and acceptance evidence, create a clean `v0.4.0` commit/tag containing the MIT LICENSE, NOTICE, accurate README, manifest ID `sorputi.openidea-collab` and matching Cargo/manifest versions. Build once from that exact tag with the official CLI, run `openidea-plugin check`, sign the package, publish the immutable URL and SHA256, install/upgrade/uninstall it, and update the marketplace PR. Never reuse the historical v0.1.0 artifact or claim checks from a different commit.

External completion conditions are limited to upstream merge/release, operator policy/audit and marketplace review. The implementation work that can be performed in these repositories is present; remaining claims require those external results.
