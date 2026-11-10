#!/usr/bin/env bash
# Experimental, local-source installer. It does not replace existing installations.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="" endpoint="" install_root="${HOME}/.local/share/openidea-collab-experimental"
port=4445 dry_run=false link=false start_server=false persist=false
usage() {
  cat <<'HELP'
Usage: install-realtime.sh --mode local|remote|managed [options]
  --endpoint https://host   Required for remote/managed; must be an existing v1 service.
  --root ABSOLUTE_PATH      Installation root (default: ~/.local/share/openidea-collab-experimental).
  --port PORT               Local server port (default: 4445).
  --link                    Link the staged development package in OpenIDEA.
  --start-server            Start the experimental server on loopback (local mode only).
  --persist                 Store server state in a private directory (requires --start-server).
  --dry-run                 Print the plan without building or changing files.
This installs an experimental editor; it is not a marketplace-signed release.
HELP
}
while (($#)); do
  case "$1" in
    --mode|--endpoint|--root|--port) [[ $# -ge 2 ]] || { echo "Missing value for $1" >&2; exit 2; }; key="$1"; value="$2"; shift 2
      case "$key" in --mode) mode="$value";; --endpoint) endpoint="$value";; --root) install_root="$value";; --port) port="$value";; esac ;;
    --dry-run) dry_run=true; shift;;
    --link) link=true; shift;;
    --start-server) start_server=true; shift;;
    --persist) persist=true; shift;;
    -h|--help) usage; exit 0;;
    *) echo "Unknown option: $1" >&2; exit 2;;
  esac
done
case "$mode" in local|remote|managed) ;; *) echo 'Choose --mode local, remote or managed.' >&2; exit 2;; esac
[[ "$install_root" == /* && "$install_root" != / && ! -L "$install_root" ]] || { echo 'Installation root must be an absolute non-symlink directory.' >&2; exit 2; }
[[ "$port" =~ ^[0-9]{1,5}$ ]] && ((10#$port >= 1 && 10#$port <= 65535)) || { echo 'Invalid port.' >&2; exit 2; }
if [[ "$mode" != local ]]; then
  [[ -n "$endpoint" ]] || { echo 'Remote and managed modes require --endpoint.' >&2; exit 2; }
  command -v python3 >/dev/null || { echo 'python3 is required to validate the endpoint.' >&2; exit 1; }
  python3 - "$endpoint" <<'PY'
import sys
from urllib.parse import urlsplit
try:
    assert all(ord(c) >= 32 and not c.isspace() for c in sys.argv[1])
    u = urlsplit(sys.argv[1])
    assert u.scheme == 'https' and u.hostname and not u.username and not u.password
    assert u.path in ('', '/') and not u.query and not u.fragment
    _ = u.port
except (AssertionError, ValueError):
    raise SystemExit('Endpoint must be an HTTPS origin without credentials, path, query or fragment.')
PY
else
  [[ -z "$endpoint" ]] || { echo '--endpoint is not used in local mode.' >&2; exit 2; }
  endpoint="http://127.0.0.1:$port"
fi
if $start_server && [[ "$mode" != local ]]; then echo '--start-server is available only in local mode.' >&2; exit 2; fi
if $persist && { ! $start_server || [[ "$mode" != local ]]; }; then echo '--persist requires --mode local --start-server.' >&2; exit 2; fi
printf 'Mode: %s\nEndpoint: %s\nRoot: %s\nEditor: experimental custom editor (*.collabtest)\nLink plugin: %s\nStart local server: %s\nPersistent server data: %s\n' "$mode" "$endpoint" "$install_root" "$link" "$start_server" "$persist"
if $persist; then echo 'Documents and token hashes will be stored without encryption in the private data directory.'; fi
echo 'Source build; unsigned development link. No service, shell configuration, firewall or system packages will be changed.'
if $dry_run; then exit 0; fi
for required in cargo openidea-plugin python3; do command -v "$required" >/dev/null || { echo "Missing required tool: $required" >&2; exit 1; }; done
if $start_server; then
  python3 - "$port" <<'PY'
import socket, sys
with socket.socket() as sock:
    try: sock.bind(('127.0.0.1', int(sys.argv[1])))
    except OSError: raise SystemExit('Local port is already in use.')
PY
fi
if $link; then
  links="$(openidea-plugin links)" || { echo 'Could not inspect existing plugin links.' >&2; exit 1; }
  if printf '%s\n' "$links" | awk -F '\t' '$1 == "sorputi.openidea-collab" { found = 1 } END { exit !found }'; then
    echo 'An OpenIDEA Collab link already exists. Preserve it; unlink it explicitly before linking this staged package.' >&2
    exit 1
  fi
fi
mkdir -p "$install_root/releases"
stage="$(mktemp -d "$install_root/.staging.XXXXXXXX")"
started_pid="" completed=false
cleanup() {
  if ! $completed; then
    if [[ -n "$started_pid" ]]; then kill "$started_pid" 2>/dev/null || true; wait "$started_pid" 2>/dev/null || true; fi
    [[ -d "$stage" && "$stage" == "$install_root"/.staging.* ]] && rm -rf -- "$stage"
  fi
}
trap cleanup EXIT
cargo build --release --locked --manifest-path "$repo_root/collab-helper/Cargo.toml"
if [[ "$mode" == local ]]; then cargo build --release --locked --manifest-path "$repo_root/collab-server/Cargo.toml"; fi
openidea-plugin build "$repo_root/openidea-collab"
mkdir -p "$stage/bin" "$stage/plugin/dist"
install -m 755 "$repo_root/collab-helper/target/release/openidea-collab-helper" "$stage/bin/openidea-collab-helper"
if [[ "$mode" == local ]]; then install -m 755 "$repo_root/collab-server/target/release/collab-server" "$stage/bin/collab-server"; fi
install -m 644 "$repo_root/openidea-collab/plugin.toml" "$stage/plugin/plugin.toml"
install -m 644 "$repo_root/openidea-collab/dist/plugin.wasm" "$stage/plugin/dist/plugin.wasm"
cp -R "$repo_root/openidea-collab/web" "$stage/plugin/web"
python3 - "$stage/plugin/web/collab-editor/config.js" "$mode" "$endpoint" <<'PY'
import json, pathlib, sys
value = {'mode': sys.argv[2], 'endpoint': sys.argv[3]}
pathlib.Path(sys.argv[1]).write_text('globalThis.COLLAB_DEFAULTS = Object.freeze(' + json.dumps(value, ensure_ascii=True) + ');\n')
PY
openidea-plugin check "$stage/plugin"
if $start_server; then
  if $persist; then
    data_dir="$install_root/data"
    python3 - "$data_dir" <<'PY'
import os, stat, sys
path = sys.argv[1]
try:
    os.mkdir(path, 0o700)
except FileExistsError:
    pass
info = os.lstat(path)
if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
    raise SystemExit('Data directory must be a private, user-owned directory, not a symlink.')
PY
  fi
  {
    printf '#!/usr/bin/env bash\nset -euo pipefail\n'
    printf 'script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"\n'
    if $persist; then
      printf 'export COLLAB_JOURNAL=%q\n' "$data_dir/journal.sqlite"
    else
      printf 'unset COLLAB_JOURNAL\n'
    fi
    printf 'export COLLAB_API=realtime COLLAB_BIND=127.0.0.1 COLLAB_PORT=%q\n' "$port"
    printf 'exec "$script_dir/bin/collab-server"\n'
  } > "$stage/start-server.sh"
  chmod 700 "$stage/start-server.sh"
  "$stage/start-server.sh" >"$stage/server.log" 2>&1 &
  started_pid=$!
  python3 - "$endpoint" "$started_pid" <<'PY'
import json, os, sys, time, urllib.request
for _ in range(50):
    try:
        os.kill(int(sys.argv[2]), 0)
        with urllib.request.urlopen(sys.argv[1] + '/health', timeout=.5) as response:
            body = json.load(response)
            if body.get('service') == 'openidea-collab' and body.get('api') == 'realtime-experimental': break
    except (OSError, ValueError): pass
    time.sleep(.1)
else: raise SystemExit('Server failed to start or returned an unexpected health response.')
PY
  printf '%s\n' "$started_pid" > "$stage/server.pid"
fi
# The random staging name is retained to avoid replacing an existing release.
release="$install_root/releases/$(basename "$stage")"
[[ ! -e "$release" ]] || { echo 'Release destination already exists.' >&2; exit 1; }
mv "$stage" "$release"
# Update cleanup target only for the directory this run created.
stage="$release"
if $link; then
  if ! openidea-plugin link "$release/plugin"; then
    echo "Link failed. Staged files remain at $release; no existing link was removed." >&2
    exit 1
  fi
fi
completed=true
printf '\nStaged at: %s\nPlugin: %s/plugin\nHelper: %s/bin/openidea-collab-helper\n' "$release" "$release" "$release"
if [[ "$mode" == local ]]; then printf 'Server: %s/bin/collab-server\n' "$release"; fi
if $start_server; then printf 'Local server PID: %s\n' "$started_pid"; fi
if $start_server; then printf 'Restart manually: %s/start-server.sh\n' "$release"; fi
if $persist; then printf 'Persistent data: %s\n' "$data_dir"; fi
echo 'In OpenIDEA plugin settings, choose the exact Helper path above using “Choose executable…” for process:openidea-collab-helper and approve it.'
echo 'Use a trusted project and open a .collabtest file with the experimental custom editor.'
echo 'The running IDE must reload the plugin. This does not prove two-IDE collaboration or enable a managed service.'
