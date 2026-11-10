# Relay operations

This repository provides code/configuration, not a deployed service. The target hostname is **collab.synthosia.com**. The operator must configure DNS, TLS, a reachable host, admission/antiabuse and monitoring before a public deployment. No credentials or host access were supplied in this task.

## Small-host setup

Build `cargo build --locked --release -p collab-server` and install that binary on the operator's Linux server. `deploy/openidea-collab.service` is an example hardened systemd unit; create the dedicated `openidea-collab` service account first and review paths, platform support and budgets before installing it. The state directory must be a real owner-only directory, not a symlink (including a DynamicUser private-state symlink). `deploy/Caddyfile` terminates HTTPS/WSS and forwards to loopback. Do not add access logs containing invitation bodies, Authorization or socket payloads.

Configuration:

| Variable | Default |
| --- | --- |
| `COLLAB_BIND` | `127.0.0.1:4444`; non-loopback addresses are refused |
| `COLLAB_PUBLIC_ORIGIN` | `https://collab.synthosia.com`; HTTPS outside loopback |
| `COLLAB_STATE_FILE` | `./collab-state/metadata.json`; use a dedicated owner-only parent |

The program uses one async I/O worker. Authorization metadata fsync runs outside that worker without holding the forwarding registry lock, with one write in flight; the executable caps blocking threads at two. Document data requires no disk writes, database or CRDT work. Queue/rate/session limits are defined in the protocol and source; changing them requires measurement, not just increasing defaults after overload. TLS proxy memory must be budgeted separately from the relay process.

`GET /healthz` returns 204 without creating a room. `/v1/info` reports protocol and bounds. A passing health request does not verify authentication, proxy WebSocket upgrade or two-client editing.

## Persistence and restart

Only membership digests, invitations, expiry and document IDs are persisted. Protect this metadata and backups as authorization material. A restart reloads valid membership, but no document content. Clients reconnect to authorized peers and resynchronize; there is no relay replay buffer. Restore tests must verify both that revoked users stay revoked and that source content is absent from metadata.

Atomic metadata replacement includes file and directory fsync on Unix. A metadata failure closes sockets and fails the service closed (health becomes 503) until restart reloads actual disk state. This also covers a successful rename followed by a directory-fsync error; no content durability ACK is issued. Review filesystem durability on the real host. Windows owner-only ACLs and deployment behavior have not been validated; the systemd example targets Linux.

## Public exposure gate

The default public creation route is bounded globally, not an antiabuse identity system. Add an operator-approved admission/rate policy before exposing it to untrusted users. Test TLS certificates, WSS upgrade, idle timeouts, revocation, blocked/slow clients, restart and quotas. No automatic room scaling, cluster or permanent free hosting is implied.

A candidate memory budget is 64 MiB RSS for the relay process at its sixteen-connection cap. The example systemd hard limit is 96 MiB to allow headroom; both are targets until measured. Measure Caddy and total host usage separately. A local forwarding benchmark is not an IDE latency benchmark.

## GitHub Actions deployment

`.github/workflows/deploy-relay.yml` tests and builds each push to `main` in a pinned Rust Alpine container and deploys the resulting Linux binary only after the build passes. Pull requests touching relay or deployment files run the tests without access to the deployment key. The deploy job rejects an already outdated `main` commit and sends the executable and SHA-256 over host-key-verified SSH using a dedicated forced-command account. The host serializes activations, verifies the checksum and size, switches the binary atomically, checks local health and restores the previous binary if startup fails. Old binaries are retained under `/opt/openidea-collab/releases/`.

The repository variable `COLLAB_DEPLOY_HOST` names the fixed public host. The repository secrets `COLLAB_DEPLOY_SSH_KEY` and `COLLAB_DEPLOY_KNOWN_HOSTS` contain the restricted deployment identity and the verified host key. On the VM, `deploy/activate-relay.sh` and `deploy/ssh-relay-dispatch.sh` must be installed root-owned at `/usr/local/sbin/openidea-collab-activate` and `/usr/local/sbin/openidea-collab-dispatch` respectively. The forced SSH key must have `restrict` and invoke the dispatcher; only the activation helper may run via sudo. Changes to those root-owned scripts require a separate reviewed operator installation: a repository change alone does not update the server helper.

A health check is not an end-to-end collaboration test. Restarting disconnects active clients. Rollback restores the executable only: it does not reverse metadata schema migrations or restore document contents. Review state compatibility before merging a version that changes persistent metadata; use a tested backup and migration procedure for incompatible changes. The relay does not store document contents and cannot recover them by itself. A static IP should be assigned to the Lightsail instance before relying on unattended deployments; update the variable and verified SSH host key if the endpoint changes.
