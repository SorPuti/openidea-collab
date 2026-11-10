# Contributing

Product implementation and tooling belong in `src/`. Documentation belongs in `docs/`, deployment configuration in `deploy/`, and repository policy/automation in root or `.github/`. Each Rust component has its tests beside its source and uses the root workspace/lockfile.

```sh
./src/tooling/check.sh
```

Rust 1.96.0 is pinned. Developers need the toolchain; package users do not. Use one build job on constrained machines (`CARGO_BUILD_JOBS=1`). The official CLI v0.1.40 is used to check the current API 1.4 control-plane package; future host-dependent builds need the exact matching released SDK and CLI.

Keep one problem per PR. Use Conventional Commit titles and describe the behavior, verification commands/results and residual limitations. Add meaningful regression tests for authorization, resource limits, project isolation and preservation of edits. No test or README should claim native realtime from a mocked session request.

Source changes to the OpenIDEA public API belong upstream with WIT, SDK, runtime, grants, compatibility tests and IDE verification. The integration proposal lives in [docs/host-api.md](docs/host-api.md); configuration does not create a missing API.

Do not expose unreviewed code as a public service or submit a new marketplace version until [implementation-status.md](docs/implementation-status.md) release blockers are resolved. Follow [SECURITY.md](SECURITY.md) for vulnerability reports and [releasing.md](docs/releasing.md) for provenance.
