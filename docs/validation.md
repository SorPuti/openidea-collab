# Validation

Run `./src/tooling/check.sh`. It validates metadata/provenance tests, formatting, Clippy with warnings denied, workspace tests and the WASM build. The relay integration suite starts its own loopback listener on an OS-selected port and tests HTTP/WebSocket access without a running external service.

Package check: with the official CLI v0.1.40 available, run `./src/tooling/package-dev.sh`. This verifies the staged **control-plane package**, not native editing or a signature. Signing and marketplace installation remain separate checks.

The suites cover single-use invitations under concurrent redemption, viewer/owner roles, authenticated sender stamping, registered-document/room boundaries, revoked open sockets, oversized frames, metadata restart without credentials/content, failed metadata commits, queue overflow, malformed protocol, exact-origin invites, UTF-16/convergence, checkpoint corruption/project mismatch, chunk reordering and unsafe path policy. SDK mock tests verify that activation/save events send no files and that project changes do not reuse session access.

## Required actual IDE acceptance

Install the candidate in two isolated IDE profiles without Rust/CLI/helper. Confirm native UI, grants/refusal, project switching, safe credential lifecycle and no file traffic. Once the proposed host APIs are released, test concurrent native edits, nonfocused documents, Undo/Redo, Unicode/IME, reconnect/reclamation, local journal crash/disk-full handling, revocation and bounded queues. Use disposable synthetic content only.

Public deployment also requires actual HTTPS/WSS certificate/proxy tests, operator admission controls, resource measurements, restart and slow-peer behavior. A localhost forwarding measurement is not a measure of internet or IDE editing latency. Platform and full-IDE tests not executed must remain marked pending.

## Measured local evidence — 2026-11-10

Environment: Linux execution workspace, Rust 1.96.0, unoptimized test-profile relay binary without debug symbols, one async worker, metadata persistence enabled. Synthetic payloads only. No internet, TLS proxy or real OpenIDEA editing in this measurement.

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

The official OpenIDEA CLI v0.1.40 is used by `package-dev.sh` to validate the current control-plane WASM component. It does not sign the development package or demonstrate a persistent native host socket. Browser/memory tests are opt-in and their extra tooling is not installed on collaborator machines.

Final local validation: 29 Rust tests and 5 Python release-guard tests passed, plus the opt-in process-budget test and browser scenario. Formatting, Clippy (`-D warnings`), WASM build, shell/workflow validation and the official CLI package-folder check passed. The checked WASM component is 425.8 KiB/API 1.4. Signing, production WSS/TLS deployment, cross-platform execution and real IDE editing remain unverified.

Remote GitHub Actions did not execute the PR job: the check annotation reports “The job was not started because your account is locked due to a billing issue.” Local results above are independent of that account-level blocker; no remote CI success is claimed.
