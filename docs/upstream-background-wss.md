> Historical issue draft. Issue [#2532](https://github.com/l0ng-ai/openidea/issues/2532) was opened and implementation [PR #2544](https://github.com/l0ng-ai/openidea/pull/2544) now covers this request plus scoped document events. Keep this file as design history; do not submit it as a duplicate.

### Before you start

- [x] I searched existing issues. #466 covers document/provider extension points and #922 concerns a web client architecture; I did not find a scoped background WebSocket API request for WASM plugins.

### Problem

A plugin cannot maintain a bidirectional relay connection through the public host APIs without installing a native helper. In v0.1.40/API 1.4 and main inspected at `c25459317bbc44a317fa203b9dc2f6074b7ab625`, `net.fetch` returns a bounded complete response, the webview is intentionally isolated from external networking, and scheduled activation has a 60-second minimum. Main's API 1.5 `net:ask` is an HTTP/HTTPS consent flow, not a WebSocket transport.

The concrete use case at the time was OpenIDEA Collab's bounded authenticated relay and WASM session-preparation package. The later implementation keeps the host primitive generic and places the Collab protocol in this repository.

### Proposed solution

A generic host-owned WSS connection with nonblocking bounded enqueue/drain and coalesced readiness events scoped to the invoking plugin/project. The network worker should not occupy a synchronous WASM call or UI thread; idle instance reclamation, reactivation and project lifecycle need an explicit contract.

Reuse the host's exact-destination permission and keychain-header model where appropriate. Keep TLS mandatory off loopback; no credential URLs, unrestricted networking or direct iframe socket access. API names, capability spelling and release version should be decided upstream. A [nonbinding WIT sketch](https://github.com/SorPuti/openidea-collab/blob/40db6596afc773130202e1376fce266b638774a5/src/host-api/proposal.wit) separates networking from documents/persistence; it is not imported by our current plugin or presented as an existing SDK.

### Acceptance criteria

- [ ] A small installed WASM fixture sends and receives messages while idle, without a process grant or external helper.
- [ ] Connect/send/drain return within normal call budgets; plugin execution does not wait indefinitely for network input.
- [ ] Host queues, message/frame sizes, connections and wakeups are bounded; slow consumers get an explicit overflow/close/resync outcome.
- [ ] Closing the project, disabling/uninstalling the plugin or revoking its destination grant closes the associated connections, including pending upgrades.
- [ ] Events/handles cannot cross plugin/project boundaries; idle reclamation has a documented reconstruction and delivery policy.
- [ ] TLS, endpoint validation, keychain headers, cancellation, malformed frames, credential redaction and rejected grants have regression coverage.
- [ ] WIT, SDK/mock, host/runtime, compatibility snapshots and authoring docs remain consistent with the frozen-API rules.

### Alternatives and workarounds

PTY plus an approved native helper is available but adds an executable and high-risk process permission on every collaborator's machine. HTTP polling cannot provide the requested independent bidirectional delivery through the current activation model. Relaxing the webview sandbox or using private APIs is not an acceptable workaround.

### Additional context

The linked draft provides local real-WebSocket tests with two Yrs clients, role/room authorization, revocation, queue limits and a separate-process 16-connection memory check. Those tests do **not** establish actual IDE delivery, production TLS, a signed marketplace installation or a completed host implementation. We are requesting a maintainer decision on this host API before preparing its runtime implementation, following CONTRIBUTING.md's `planned` workflow.
