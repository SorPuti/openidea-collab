# CI on the public repository

`SorPuti/openidea-collab` was confirmed **PUBLIC** through the repository API on 2026-11-10. The normal CI job uses the standard GitHub-hosted **ubuntu-24.04** label, read-only repository permissions, a 30-minute timeout, one Cargo build job and cancellation of superseded runs. It uploads no Actions artifacts and uses no Actions cache. Jobs are guarded against running if the repository becomes private.

GitHub documents standard hosted compute for public repositories as free: [GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions). Larger runners are a different paid category; none is selected here. Tagged release candidates remain separately gated; their optional artifact storage is not the normal CI and must be checked against account storage policy before enabling releases.

## Account-level failure observed

Run `42812019087` reports `repository.private=false`, runner label `ubuntu-24.04`, no assigned runner and an empty step list. Its annotation says:

> The job was not started because your account is locked due to a billing issue.

This happens before any repository command executes. Changing source, making an already-public repository public again, or changing between standard Ubuntu labels does not clear an account billing restriction. The connected integration also refuses repository Actions administration with 403; this task did not change paid plans, payment methods, spending limits or account settings.

The account owner should inspect the account's **Settings → Billing & licensing** alert. If the restriction incorrectly blocks public standard-runner usage, contact [GitHub Support](https://support.github.com/contact) with the repository and failed run URL. No paid plan is required for this public standard-runner CI, and adding a card or purchasing minutes is not a prescribed repository fix. Any outstanding charge for other account services is a separate account matter.

Once GitHub clears the account restriction, rerun the workflow. Local verification remains available without GitHub Actions:

```sh
./src/tooling/check.sh
```
