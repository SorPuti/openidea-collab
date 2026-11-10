# Release process

Keep marketplace publication gated while native host integration, security, real IDE tests and operator requirements in [implementation-status.md](implementation-status.md) remain unresolved. The current session-preparation build must not be advertised as a working realtime editor.

Rust 1.96.0 and the workspace Cargo.lock are pinned. The current component uses SDK/CLI v0.1.40, API 1.4. New host-dependent components need an exact compatible released SDK/CLI and matching minimum API; do not claim that API 1.5 automatically includes the proposed WSS/document functions.

```sh
./src/tooling/check.sh
./src/tooling/package-dev.sh
```

The development staging directory is `target/plugin-package`: WASM, manifest, translations, LICENSE, NOTICE and the package README. Source/legal documentation stays outside implementation directories and is copied during packaging.

The tagged release workflow validates a clean exact tag and matching plugin versions, tests the workspace, then builds candidates only when the repository variable `COLLAB_RELEASE_READY` is explicitly enabled. Signing uses the `release` environment and its secret; configure reviewer protection before enabling it. The workflow creates a draft only. It does not update the marketplace or publish a final release.

`src/tooling/release.sh` builds from a fresh `git archive`, invokes package validation, signs/indexes the package and records source metadata/checksums. It refuses an existing output directory. Required variables are `RELEASE_TAG`, `OPENIDEA_KEY_FILE`, `OPENIDEA_PUBLISHER=sorputi` and `OPENIDEA_DOWNLOAD_BASE` for that tag's download URL. Never put signing keys in the repository or chat. No signing key or real tag was generated for this implementation.

Before any release: complete dependency/license notices for the actual graph (including Yrs and the Apache-2.0 SDK), run signed installation and both-IDE acceptance, test platform binaries and deployment, and record evidence for every PR #102 request. Add a new version table with its exact package URL/SHA-256 only after that process. Preserve historical tags.
