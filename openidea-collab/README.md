# OpenIDEA Collab plugin

Experimental OpenIDEA plugin for room-based file synchronization and presence. **Not approved for marketplace listing.**

The plugin sends the complete text and path of saved project files to the configured collaboration server. It also sends the chosen name, optional GitHub username, focused file and cursor position. Other room participants can retrieve shared content. Remote updates can write into the open project. There is currently no safe file allowlist, secret filtering, project-bound consent or participant authentication. Use only disposable projects with synthetic data and an isolated server.

The manifest requests project read/write, events, storage, editor read and loopback networking. Stored room state is restored across activations and is not yet bound to a project. A room ID is not proof of identity. Failed operations and conflicts do not yet have reliable recovery guarantees.

```sh
cargo test --locked
openidea-plugin build .
openidea-plugin link .
```

See the repository [README](../README.md), [security model](../docs/security.md), [manual validation](../docs/validation.md) and [release process](../docs/releasing.md). Packages include this README and a copy of the repository MIT license. The schedule is 60 seconds; no subsecond synchronization guarantee is made.
