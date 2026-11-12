Thanks — split completed.

- #2544 is now only the generic WebSocket transport: WIT connect/send/drain/close, capability checks, project/plugin binding, lifecycle cleanup, bounded inboxes, `websocket-ready`, SDK wrapper, and mock. Its head is `85b0639`.
- Document subscriptions are in draft [PR #2610](https://github.com/l0ng-ai/openidea/pull/2610) at `8dcfdf6`; it contains one commit on top of the merged #2544 transport.
- The approved `tokio-tungstenite` transport now reuses OpenIDEA's existing rustls configuration with ring and the platform verifier; it does not select another crypto provider.
- The #2544 description has been rewritten around the socket API and records the commands and runner limitations precisely.

The transport suites, SDK suite, scoped clippy, formatting, and frontend suite pass. The combined package-test command compiled the packages but exceeded this runner's 4.9 GiB `/dev/shm` while linking all `ide-host` integration binaries concurrently, so I ran the library suites sequentially: 25 `ide-plugin` and 347 `ide-host` tests passed. `pnpm test` passed 9,741 tests in 798 files. `pnpm gen:bindings` is blocked on this runner because its Tauri export test needs GTK/WebKit development packages; the checked-in split bindings are included for CI verification.
