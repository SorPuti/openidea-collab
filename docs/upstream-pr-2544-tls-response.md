Implemented the TLS review note in `85b0639`.

- WebSockets now receive the same `rustls::ClientConfig` used by the backend HTTP client: ring plus `rustls-platform-verifier`.
- `tokio-tungstenite` no longer enables its native-roots feature and does not select another crypto provider.
- The shared builder remains in `ide-http`, so HTTP and WebSocket trust behavior cannot drift independently.
- #2610 was rebased on this change and remains draft/dependent at `8d56f2c`.

Validation after the change:

- `cargo test -p ide-host --lib plugins::websocket::tests` — 4 passed on #2544
- `cargo test -p ide-http --lib` — 24 passed
- `cargo clippy -p ide-host -p ide-http --lib --no-deps -- -D warnings -A clippy::nonminimal-bool` — passed
- `cargo fmt --all --check` — passed
- On the rebased #2610 branch: 6 WebSocket/subscription tests and 27 `ide-plugin` tests passed

