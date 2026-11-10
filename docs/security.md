# Security and data handling

## Implemented

The active server is a bounded authenticated relay, replacing the historical unrestricted file store. Protected operations authenticate a high-entropy member credential, derive identity on the server, enforce room/role/document authorization and recheck revocation. Invite consumption and metadata changes are serialized and committed before success. Viewer writes, unknown documents, foreign recipients and unauthenticated handshakes are refused.

The relay only binds to loopback, expects a TLS reverse proxy at collab.synthosia.com, rejects credentials in socket queries and refuses browser-origin sockets. Metadata stores credential digests rather than bearer values. Content is never written to metadata or logs. End-to-end encryption is not implemented; a TLS-terminating operator can inspect transient traffic.

The plugin asks only for the configured HTTPS origin, project read (root identity), storage and one declared keychain secret. It never uploads a file or writes remote content. Installation/activation and save/scheduled events do not send requests. Legacy global room state is removed on activation and never resumed.

## Remaining release blockers

- Host-owned WSS events, native document transactions/undo and a verified local persistence adapter.
- Filesystem no-follow protection, including ancestor races and cross-platform links, before any remote file application or project transfer. The standalone lexical policy does not solve this.
- Explicit file scope preview, ignore handling, secret exclusions, safe bootstrap and conflicts before sending project content.
- Real IDE installation, project-switch, refusal, revocation, offline/restart, undo and two-client tests.
- Client core resource/fuzz/performance testing, independent security review and complete third-party notices.
- Operator authentication/admission and abuse policy for public session creation. The global cap protects memory but does not prevent an attacker occupying the small instance's room quota. Do not expose a public anonymous service as production-ready on this basis.
- Real TLS/proxy deployment, certificate/timeout tests, platform matrix, signed release and registry approval.

## Operational boundaries

Use a dedicated owner-only metadata directory and a nonprivileged service account. State paths containing symlinks are refused. If metadata cannot be committed, the mutation fails and prior in-memory authorization is retained. Filesystem/host power-loss guarantees remain platform-dependent and need target deployment tests.

A slow peer is disconnected rather than buffered indefinitely. No document cache means a new peer needs another authorized peer with a copy; a session URL is not backup storage. Authentication does not make remote source code trustworthy. This product neither executes it nor shares terminals or build permissions.

Report issues through [SECURITY.md](../SECURITY.md). Historical tag v0.1.0 remains unchanged and is not a verified candidate.
