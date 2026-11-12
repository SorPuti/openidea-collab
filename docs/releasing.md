# Release process

Keep marketplace publication gated while the upstream host release, real IDE tests, security review and operator requirements in [implementation-status.md](implementation-status.md) remain unresolved. Version 0.4.0 is a development realtime build, not an approved marketplace release.

Rust 1.96.0 and the workspace Cargo.lock are pinned. The component currently pins fork commit `5af2168`, which stacks document events and editor access in PR #2610 on the merged WSS transport in PR #2544. Replace it with an exact official SDK/CLI release before tagging; a fork revision must not be presented as an upstream release.

```sh
./src/tooling/check.sh
./src/tooling/package-dev.sh
```

The development staging directory is `target/plugin-package`: WASM, manifest, translations, LICENSE, NOTICE and the package README. Source/legal documentation stays outside implementation directories and is copied during packaging.

The tagged release workflow validates a clean exact tag and matching plugin versions, tests the workspace, then builds candidates only when the repository variable `COLLAB_RELEASE_READY` is explicitly enabled. Signing uses the `release` environment and its secret; configure reviewer protection before enabling it. The workflow creates a draft only. It does not update the marketplace or publish a final release.

`src/tooling/release.sh` builds from a fresh `git archive`, invokes package validation, signs/indexes the package and records source metadata/checksums. It refuses an existing output directory. Required variables are `RELEASE_TAG`, `OPENIDEA_KEY_FILE`, `OPENIDEA_PUBLISHER=sorputi` and `OPENIDEA_DOWNLOAD_BASE` for that tag's download URL. Never put signing keys in the repository or chat. No signing key or real tag was generated for this implementation.

Before any release: complete dependency/license notices for the actual graph (including Yrs and the Apache-2.0 SDK), run signed installation and both-IDE acceptance, test platform binaries and deployment, and record evidence for every PR #102 request. Add a new version table with its exact package URL/SHA-256 only after that process. Preserve historical tags.
