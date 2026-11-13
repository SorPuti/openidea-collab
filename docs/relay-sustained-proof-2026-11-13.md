# Relay load and recovery evidence — 2026-11-13

## Scope

The tested production executable had SHA-256 `4750251218f62afa4b7ae4420a79f55c5bb1d7e6ecdcae7430ee3559e0afed44`, built from the relay changes merged in PR #11. The VPS was an Ohio Lightsail Ubuntu 24.04 machine with a 512 MB plan and two vCPUs. Results are observations from specific synthetic tests, not a service-level guarantee.

## Isolated 16-session API, restart, and sustained load

On the VPS, a separate relay process created 16 sessions, three members and one registered document per session through the actual HTTP APIs, using a private temporary state directory. Creation respected the existing rate limit. A seventeenth session was refused with `session_limit`. The isolated process was restarted and all 48 credentials reconnected. A 2 MiB synthetic transfer was split into 35 protocol frames, delivered to two recipients and reassembled byte-for-byte.

The sustained phase ran 180.06 seconds with 16 simultaneous sending clients and 32 receiving clients: 35,808 messages of 1 KiB and 71,616 byte-for-byte checked deliveries, with zero mismatches in that run. Fanout completion was p50 2.78 ms, p95 4.51 ms, p99 4.74 ms and maximum 51.63 ms. Sampled peak relay RSS was 10,096 KiB; mean relay CPU was 2.23% of one core; minimum sampled available host memory was 162,184 KiB. The separate test relay and Python clients shared a cgroup with a 60% CPU quota and 72 MiB memory limit. The workload had one message in flight per room and approximately 75 ms between completions, so it does not establish burst handling or maximum capacity.

A member was revoked and its credential was refused. The test checked that the credential strings and the synthetic payload did not appear in the isolated relay logs. The production relay's local health endpoint was sampled approximately every two seconds during the run and returned 204 when checked; intermittent failures between samples cannot be ruled out. Caddy, TLS, real network and IDE behavior were not part of this isolated run.

## Public HTTPS and WSS

A 90-second synthetic test from a separate client used the live HTTPS/WSS path via the existing proxy, created one session and three authenticated connections, and verified all 948 deliveries of 474 one-KiB messages byte-for-byte. End-to-end fanout completion was p50 158.18 ms, p95 164.91 ms, p99 204.14 ms and maximum 423.25 ms. Revocation closed the existing editor socket, an unauthenticated room-creation request was refused, and the synthetic session was removed with HTTP 204. These measurements include that client's network and are not general latency targets.

## Product boundary and negative evidence

The plugin integration test `document_change_flows_to_websocket_and_remote_edit_is_refused_without_ack` passed; it establishes that the current plugin deliberately refuses inbound remote editing, not that editing works. The installed OpenIDEA package was `open-idea-0.1.44-1.x86_64`. Upstream [PR 3236](https://github.com/l0ng-ai/openidea/pull/3236) merged after the [v0.1.44 release](https://github.com/l0ng-ai/openidea/releases/tag/v0.1.44), and the plugin still pins the earlier SDK. A separate unit test confirms that a single text document over 1 MiB is rejected. The current plugin shares at most 32 explicitly selected documents. Therefore neither this relay test nor a 2 MiB wire transfer proves safe whole-project synchronization, editor integration or large-document support.

## Reproduction and safety

`src/tooling/relay-soak.py` spawns an isolated copy using a temporary state directory and creates sessions through its actual HTTP API. On a VPS with the live relay on `127.0.0.1:4444`, run it in a constrained scope, after confirming sufficient available memory:

```sh
sudo systemd-run --scope --quiet -p CPUQuota=60% -p MemoryMax=72M -p TasksMax=32 \
  sudo -u ubuntu env COLLAB_TEST_BINARY=/usr/local/bin/collab-server \
  SOAK_ROOMS=16 SOAK_SECONDS=180 timeout 720 nice -n 10 \
  python3 src/tooling/relay-soak.py
```

The scope contains the load generator and its test relay, not production. It monitors production loopback health and stops if available host memory falls below 48 MiB. Creation of 16 sessions respects the existing token bucket and takes several minutes. `COLLAB_GUARD_HEALTH` can override the health URL. The test emits aggregate results only, uses random synthetic credentials and content, and removes its temporary state on normal exit. It does not test Caddy or TLS.

`src/tooling/relay-public-soak.py` requires Python's `websockets` package and explicitly creates one real, disposable session using an operator admission token; it revokes that session on completion. For example:

```sh
python3 src/tooling/relay-public-soak.py --origin https://collab.synthosia.com \
  --admission-file ~/.config/openidea-collab/admission-token
```

This occupies a production session and generates real traffic. Never include the token value in shell history or share it. The load is synthetic, but the test may affect concurrent users on a small host.
