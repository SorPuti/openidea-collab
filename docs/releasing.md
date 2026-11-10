# Release process

## Current gate

Do not publish a new marketplace candidate until the requirements in [security.md](security.md) and the [manual IDE exercise](validation.md) are complete. Historical `v0.1.0` must remain unchanged. The `latest` rolling workflow has been removed; pushing to `master` now runs CI only.

The tag workflow always validates source and tests. Signing and draft creation are disabled unless the repository variable `COLLAB_RELEASE_READY` is `true`. Configure the `release` GitHub environment with required reviewers and the `OPENIDEA_KEY` secret before enabling it. Only enable the variable after the outstanding review requirements are resolved. Do not paste private keys into issues, logs or documentation.

## Versions and tools

- Rust is pinned by `rust-toolchain.toml` to 1.93.0.
- The CLI requires Rust 1.96.0 to compile; install it with the explicit toolchain shown below. The plugin/server checks use the repository toolchain.
- SDK and CLI use OpenIDEA `v0.1.40`; Cargo lockfiles pin resolved dependencies.
- Plugin manifest ID is `sorputi.openidea-collab`; publisher is `sorputi`.
- Plugin Cargo and manifest versions must match the new `v<version>` tag. The server has its own version.
- A release contains the repository license, plugin license, README, exact source commit and SHA-256 checksums.

## Preparation

1. Finish security remediation and record manual validation results. Review third-party license obligations for the actual dependency graph, including the Apache-2.0 OpenIDEA SDK/CLI. Include applicable license text, attribution and upstream NOTICE material as needed before distribution; a project MIT license does not replace dependency notices.
2. Run `./scripts/check.sh`, `python3 scripts/verify-release.py` and package validation using the pinned CLI.
3. Update the changelog and matching plugin versions in a reviewed commit. Create a new version tag on that commit; never move an old tag to repair provenance.
4. Run packaging from a clean checkout of that exact tag with no untracked source files. `scripts/release.sh` verifies the tag and builds from a fresh `git archive`, so ignored local files cannot enter the package.

```sh
rustup toolchain install 1.96.0 --profile minimal
cargo +1.96.0 install --locked --git https://github.com/l0ng-ai/openidea --tag v0.1.40 ide-host --bin openidea-plugin
RELEASE_TAG=v0.2.1 \
OPENIDEA_PUBLISHER=sorputi \
OPENIDEA_KEY_FILE=/secure/location/publisher.pem \
OPENIDEA_DOWNLOAD_BASE=https://github.com/SorPuti/openidea-collab/releases/download/v0.2.1/ \
./scripts/release.sh
```

The version above is illustrative; choose a new unused version and set both manifests first. Output goes to `dist-release/<tag>/`: signed `.oiplugin`, signed `index.json`, development `plugin.tar.gz`, `source.json` and `SHA256SUMS`. Existing output is refused. The script performs `openidea-plugin check` on the staged plugin and resulting package; it does not publish anything.

## Review and distribution

The workflow creates a **draft** release only after validation and the explicit readiness gate. Review the source commit, packaged files, dependency notices, signature and checksums. Verify the package using the registered publisher key and perform an installation in an isolated IDE profile. A passing package check does not establish safe collaboration behavior.

The workflow also builds the existing Linux/macOS x86_64/ARM64 server binaries from the same tag and includes them in the draft and checksums. Linux ARM builds pin `cross` to 0.2.5. These platform builds still need CI validation; the local packaging script prepares the plugin only. The legacy installer resolves the latest published release and must be exercised against the chosen candidate before it is advertised as supported. There is no automatic rolling `latest` publication.

Publish the reviewed draft only after all release criteria pass. Then update the marketplace PR's version, package URL and SHA-256 to this exact artifact, and explain which tests cover each review finding. Do not change the historical package entry or claim approval before the maintainer reviews it.
