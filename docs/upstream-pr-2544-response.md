Thanks — split completed.

- #2544 is now only the generic WebSocket transport: WIT connect/send/drain/close, capability checks, project/plugin binding, lifecycle cleanup, bounded inboxes, `websocket-ready`, SDK wrapper, and mock. Its head is `2533040`.
- Document subscriptions were moved to the stacked [`SorPuti:feat/plugin-document-subscriptions`](https://github.com/SorPuti/openidea/tree/feat/plugin-document-subscriptions) branch at `288b7b4`; it contains one commit on top of #2544 and will remain draft/dependent until the transport and `tokio-tungstenite` are approved.
- The #2544 description has been rewritten around the socket API and records the commands and runner limitations precisely.

The transport suites, SDK suite, scoped clippy, formatting, and frontend suite pass. The combined package-test command compiled the packages but exceeded this runner's 4.9 GiB `/dev/shm` while linking all `ide-host` integration binaries concurrently, so I ran the library suites sequentially: 25 `ide-plugin` and 347 `ide-host` tests passed. `pnpm test` passed 9,741 tests in 798 files. `pnpm gen:bindings` is blocked on this runner because its Tauri export test needs GTK/WebKit development packages; the checked-in split bindings are included for CI verification.
