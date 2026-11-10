# Changelog

## Unreleased — plugin 0.3.0 / relay 0.2.0

- Move implementation, tests and tooling into `src/`; use one Rust workspace/lockfile and Rust 1.96.0.
- Configure collab.synthosia.com for HTTPS session preparation and future WSS transport.
- Replace the legacy unrestricted file store with an authenticated bounded WebSocket relay and metadata-only recovery.
- Add per-member credentials, single-use invitations, roles, revocation, document IDs, queues/rates and protocol version checks.
- Add a standalone UTF-16 Yrs core, bounded chunk assembly, integrity/project-bound checkpoints and sharing policy.
- Replace legacy auto-upload/file writes with native session-preparation UI and keychain credentials; show the missing host realtime support explicitly.
- Remove the system installer from the collaborator path; stage one plugin package with legal documentation and translations.
- Document public host API dependencies, operator deployment, limits and remaining release blockers.

## Historical v0.1.0

Submitted but not approved. The tag lacks the license file and its source ID differs from the published package. Preserve it for audit; it is not a verified release. The later helper/custom-editor experiment remains in its archive tag and is not the active architecture.
