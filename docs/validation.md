# Validation

## Automated

Run `./scripts/check.sh`. The HTTP suite starts its own loopback server on an OS-assigned port; it needs no existing service and runs during normal `cargo test`. It covers room lifecycle, nested file paths, update/event delivery and conflict rejection. SDK mock tests cover scheduled dispatch separately from actual IDE scheduling.

## Manual IDE exercise (required before release)

Use two isolated IDE instances/profiles and disposable projects containing synthetic text only. Build/link the plugin using OpenIDEA CLI `v0.1.40`. This exercise has not been automated and must be recorded with the IDE version and commit tested.

1. Start a server isolated from untrusted networks and grant loopback network access to both clients.
2. Create a room in client A and join it in B. Confirm both panels show the other member.
3. Save a nested text file in A. Without running another command in B, wait for scheduled polling and confirm the content arrives. Record latency; the requested interval is 60 seconds, plus request time.
4. Edit a different file in B and verify delivery to A. Focus files and check presence updates.
5. Make simultaneous edits and confirm a 409 is visible. Do not interpret the current warning as a reliable overwrite/recovery mechanism.
6. Leave the room, verify membership removal and ensure subsequent saves do not sync.
7. Record permission failures, notifications and IDE logs with secrets removed.

Successful delivery does not establish safe remote writes, project binding or lossless conflict recovery. Those require the separate security regression suite described in [security.md](security.md).
