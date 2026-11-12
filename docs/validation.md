# Validation

Run `./src/tooling/check.sh`. It validates metadata/provenance tests, formatting, Clippy with warnings denied, workspace tests and the WASM build. The relay integration suite starts its own loopback listener on an OS-selected port and tests HTTP/WebSocket access without a running external service.

Package check: with the official OpenIDEA v0.1.43 `openidea-plugin` CLI available, run `./src/tooling/package-dev.sh`. This verifies the staged development package, not a publisher signature or marketplace installation.

The suites cover single-use invitations under concurrent redemption, viewer/owner roles, authenticated sender stamping, registered-document/room boundaries, revoked open sockets, oversized frames, metadata restart without credentials/content, failed metadata commits, queue overflow, malformed protocol, exact-origin invites, UTF-16/convergence, checkpoint corruption/project mismatch, chunk reordering and unsafe path policy. Plugin/SDK tests also prove activation sends no files, project changes cannot reuse a session, exact document filters are installed, local committed text reaches an outbound CRDT frame, and an inbound peer update reaches the versioned editor and produces an ACK.

## Required actual IDE acceptance

Install the candidate in three isolated OpenIDEA v0.1.43-or-newer profiles without Rust/CLI/helper. Confirm native UI, owner/editor/viewer grants, exact path isolation, project switching, safe credential lifecycle, concurrent native edits, nonfocused documents, Undo/Redo, Unicode/IME, reconnect/reclamation, checkpoint failure handling, revocation and bounded queues. Use disposable synthetic content only.

Public deployment also requires actual HTTPS/WSS certificate/proxy tests, operator admission controls, resource measurements, restart and slow-peer behavior. A localhost forwarding measurement is not a measure of internet or IDE editing latency. Platform and full-IDE tests not executed must remain marked pending.

## Measured local evidence — 2026-11-10

Environment: Linux execution workspace, Rust 1.96.0, unoptimized test-profile relay binary without debug symbols, one async worker, metadata persistence enabled. Synthetic payloads only. This measurement excludes internet, TLS proxy and real OpenIDEA editing.

```sh
cargo test --locked -p collab-server --test resource_budget -- --ignored --nocapture
```

At 16 real WebSocket connections across 4 rooms, 400 messages of 1 KiB were fanned out into 1,200 deliveries. The separate relay process had a sampled peak RSS of **7,744 KiB (about 7.6 MiB)**; fanout completion p50 **194 µs**, p95 **511 µs**, maximum **33,218 µs**. The run lasted about 0.33 s. This is a short local functional/budget check, not a soak test, a CPU benchmark, a worst-case allocation bound or a latency guarantee. Kernel socket buffers, proxy TLS, clients and IDEs are outside this process RSS.

The optional browser check was executed with Playwright and system Chromium:

```sh
CHROMIUM_EXECUTABLE_PATH=/usr/bin/chromium node src/tooling/tests/invitation-ui.mjs
```

It verified copying the complete synthetic invitation, clearing its fragment from the current history entry, disabling copying when no secret is present, and the absence of third-party requests. This tests the relay invitation page, not the native IDE view. Its screenshot contains no invitation secret:

![Invitation page after copying](assets/invitation-page.png)

On 2026-11-10 a separate live smoke check created an owner and editor through HTTPS, registered a synthetic document, negotiated `openidea-collab.v1` over two authenticated WSS connections at `collab.synthosia.com`, forwarded one bounded binary frame and revoked the owner session. It printed no credentials or content. This proves the deployed authentication/upgrade/forwarding path at that moment, not sustained reliability.

Final local validation on 2026-11-10: source/release guards, formatting, Clippy with warnings denied, 37 Rust tests (plus one opt-in resource test ignored), five Python release-guard tests and the optimized `wasm32-wasip2` build passed. The SDK itself passed 59 native tests and a direct WASM target check. A CLI built from OpenIDEA PR #2544 accepted both the staged folder and unsigned development package as API 1.5. The WASM is 814.8 KiB with SHA-256 `0bde26360d937009ac41cf80da4159ee598f2f85a95622a8690151e249528c1a`; `openidea-collab-0.4.0-dev.oiplugin` is 293.3 KiB with SHA-256 `4dfc80e26552294ba57a69167949a115bfb493567688b95fbd48439eadc7577f`. Signing, official-host packaging after upstream release, cross-platform execution and real multi-IDE editing remain separate evidence.

Remote GitHub Actions did not execute the PR job: the check annotation reports “The job was not started because your account is locked due to a billing issue.” Local results above are independent of that account-level blocker; no remote CI success is claimed.
