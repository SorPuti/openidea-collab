# OpenIDEA Collab

An experimental collaborative editor for OpenIDEA. Two clients can edit the same `.collabtest` document through a Collab server. The native editor and whole-project synchronization are not implemented. The previous save-based sidebar is no longer registered.

## Install a prebuilt client (Linux x86_64)

Download the `openidea-collab-client-linux-x86_64.tar.gz` artifact from a release you trust, extract it, and run:

```sh
cd collab-client
bash install.sh
```

The script requires Python 3.11+, verifies the included files, copies them to a new directory under `~/.local/share/openidea-collab-client`, and registers the plugin in OpenIDEA's development links. No Rust toolchain, OpenIDEA plugin CLI, server or tunnel is installed on the client. To also install the prebuilt local server on a host, run `bash install.sh --server`; it prints a script to start it manually and does not start it automatically. Checksums detect corruption but are not a signature. This is currently an **unsigned experimental package**.

In OpenIDEA, open **Settings → Plugins** to load the registered development link. In its `process:openidea-collab-helper` permission, choose and approve the printed helper executable. The helper can execute code on your machine; approve it only if you trust the package. Open a trusted project and open a `.collabtest` file: it now opens directly in the collaborative editor. **Open as Text** remains available from the IDE. If an installed package with the same ID already exists, remove it using OpenIDEA first; the script will not overwrite it.

On both machines select the same server and document name. Create a session in one editor, make a one-use invite, and enter it in the other editor. Both local files must initially have the same text. Do not post the invite publicly: it grants room access. The server URL `http://127.0.0.1:4445` only reaches the current machine; it is not a connection between different machines.

## Server and connectivity

A remote server is a separate operator choice. The experimental server listens on loopback only; remote clients require an independently operated HTTPS reverse proxy and an existing publicly reachable address. The client does not install ngrok. There is no hosted service supplied by this project yet, and no deployment should be described as production-ready before security review and operational testing.

For local development, the source installer can build and start a separate server:

```sh
bash install.sh --experimental --mode local --start-server --persist
```

The server journal contains source text and credential hashes without encryption. Its `--persist` option is explicit; the process is not configured to start on login. The server and client require security, resource limits, backup and recovery review before exposure. Transport TLS does not provide end-to-end encryption from the server operator.

## Current limitations

- Only `.collabtest` files in the custom editor; no full-project bootstrap, file tree synchronization, native-editor integration or language tooling parity.
- The helper's text limit is 4 KiB. Server document state is limited to 1 MiB and journal event payloads to 64 MiB, with 4 MiB reserved for revocations. No on-disk compaction yet.
- Pending CRDT changes are not recovered automatically after the helper exits. An optional local plaintext text copy supports manual recovery and must be removed manually.
- The package and helper are experimental and not signed. Linux x86_64 is the only prebuilt target currently prepared; other systems need their own build and testing.
- A maintained managed endpoint, project-wide permissions, encrypted project transfer, large-project filtering, and installation on multiple platforms are still work to do.

## Build from source

Requirements: Rust, the `wasm32-wasip2` target and the `openidea-plugin` CLI matching the IDE.

```sh
cargo test --locked --manifest-path collab-core/Cargo.toml
cargo test --locked --manifest-path collab-helper/Cargo.toml
cargo test --locked --manifest-path collab-server/Cargo.toml
openidea-plugin build openidea-collab
cargo build --release --locked --manifest-path collab-helper/Cargo.toml
cargo build --release --locked --manifest-path collab-server/Cargo.toml
bash scripts/package-client.sh ./openidea-collab-client-linux-x86_64.tar.gz
```

The CI workflow tests components and prepares artifacts; it does not itself publish a GitHub release. `scripts/release.sh` prepares a signed plugin package only when supplied with the publisher's signing key. Never commit the key.

## License

MIT; see [LICENSE](LICENSE). Review third-party dependency notices before distribution.
