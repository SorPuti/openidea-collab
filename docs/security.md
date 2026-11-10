# Security and data handling

## Current behavior

A save sends the full file text and path to the configured server, without ignore or secret filtering. Presence includes the display name, optional GitHub username, focused file and cursor location. The plugin can read `.git/config` to suggest profile information. Room/session and profile settings are persisted through plugin storage.

The server holds room content in memory. Any caller with a room ID can currently retrieve content/events, submit updates or remove members. User IDs and GitHub usernames do not verify identity. The server operator and all participants must be trusted with all transmitted data and with the ability to propose project writes. There is no end-to-end encryption. Non-loopback HTTP is not an acceptable deployment for sensitive data.

Remote file events currently result in project writes without an explicit sharing boundary or reliable conflict recovery. The SDK's filesystem protections must not be assumed to provide the plugin's required sharing policy. The server binds to `0.0.0.0` by default, and restored sessions are not bound to the active project.

## Required remediation before listing

- Explicitly select the shared subtree or file set; apply the same policy to outgoing reads/uploads and incoming writes.
- Exclude VCS/IDE control directories, hooks, secrets and ignored files. Reject path traversal and writes through existing symlinks, including ancestor components. Verify host API support before choosing an implementation.
- Bind sessions and consent to a project identity. Switching projects must stop transmission until consent is renewed.
- Issue per-member credentials and authorize reads, writes, events, presence and membership operations. Separate invitations from member identity.
- Bind to loopback by default. Require TLS for remote deployment and validate the supported proxy/deployment model.
- Retain merge bases, local backups and pending operations. Surface conflicts; retry transient failures; durably record refused events before advancing past them.
- Test forged identities, forbidden paths, symlinks, project switches, simultaneous edits, HTTP 409, offline recovery and process restart.

The current changes only establish documentation, functional regression coverage and release preparation. They do not resolve these requirements. See [the original review](https://github.com/l0ng-ai/openidea-plugins/pull/102#issuecomment-6756170352).
