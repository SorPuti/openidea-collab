# Contributing

The project is under active security remediation. Keep fixes small and identify which marketplace review requirement they address. Do not use real credentials or private project files in examples or tests.

## Setup and checks

Install the Rust version in `rust-toolchain.toml`. The plugin pins the OpenIDEA SDK to `v0.1.40`; use the CLI from that same tag (its build requires Rust 1.96.0; see the README). Keep both Cargo lockfiles committed.

```sh
./scripts/check.sh
```

This checks formatting, Clippy, server HTTP tests, plugin mock tests and the WASM build. For CLI/package validation, follow [docs/releasing.md](docs/releasing.md). For the IDE lifecycle and two-client exercise, follow [docs/validation.md](docs/validation.md); mock tests do not establish actual IDE delivery.

## Changes

- Use branches such as `fix/server-routing` or `docs/security-model`.
- Use commit subjects such as `fix: preserve refused updates` or `docs: explain uploaded data`.
- Describe the problem, resulting behavior, checks run and remaining limitations in each PR.
- Add regression coverage for protocol, security and data preservation changes. Avoid tests that only repeat implementation details.
- Run `cargo fmt` with each component's manifest when changing Rust.
- Update documentation and `CHANGELOG.md` when behavior changes. Version the plugin and server independently; plugin Cargo and manifest versions must agree.

Security reports: see [SECURITY.md](SECURITY.md). Release policy: see [docs/releasing.md](docs/releasing.md).
