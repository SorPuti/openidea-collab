#!/usr/bin/env bash
# Install an extracted, prebuilt Collab client bundle. No Rust or server needed.
set -euo pipefail
bundle="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="${OPENIDEA_COLLAB_CLIENT_ROOT:-$HOME/.local/share/openidea-collab-client}"
if [[ "${1:-}" == --help ]]; then
  echo 'Usage: bash install-client.sh [--server] [--dry-run]'
  echo 'Installs a prebuilt Linux x86_64 client; import the package and approve the helper in OpenIDEA.'
  exit 0
fi
server=false dry_run=false
while (($#)); do
  case "$1" in --server) server=true;; --dry-run) dry_run=true;; *) echo "Unknown argument: $1" >&2; exit 2;; esac
  shift
done
[[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || { echo 'This bundle supports Linux x86_64 only.' >&2; exit 1; }
[[ "$root" == /* && "$root" != / && ! -L "$root" ]] || { echo 'Installation root must be absolute and not a symlink.' >&2; exit 1; }
command -v python3 >/dev/null || { echo 'Python 3 is required to verify the bundle.' >&2; exit 1; }
python3 - "$bundle" <<'PY'
import hashlib, pathlib, sys
root = pathlib.Path(sys.argv[1])
manifest = root / 'SHA256SUMS'
if manifest.is_symlink() or not manifest.is_file(): raise SystemExit('Missing checksums')
entries = {}
for line in manifest.read_text().splitlines():
    checksum, sep, name = line.partition('  ')
    if not sep or name not in ('plugin.oiplugin', 'openidea-collab-helper', 'collab-server') or name in entries or len(checksum) != 64:
        raise SystemExit('Invalid checksums')
    entries[name] = checksum
if len(entries) != 3: raise SystemExit('Incomplete checksums')
for name, expected in entries.items():
    file = root/name
    if file.is_symlink() or not file.is_file(): raise SystemExit('Missing or linked bundle file: '+name)
    digest = hashlib.sha256()
    with file.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024*1024), b''): digest.update(chunk)
    if digest.hexdigest() != expected: raise SystemExit('Checksum mismatch: '+name)
PY
printf 'Verified bundle: %s\nDestination root: %s\n' "$bundle" "$root"
echo 'The checksum detects corruption; verify the download source separately. The package is unsigned.'
if $dry_run; then exit 0; fi
mkdir -p "$root/releases"
release="$(mktemp -d "$root/releases/client.XXXXXXXX")"
install -m 644 "$bundle/plugin.oiplugin" "$release/plugin.oiplugin"
install -m 755 "$bundle/openidea-collab-helper" "$release/openidea-collab-helper"
if $server; then
  install -m 755 "$bundle/collab-server" "$release/collab-server"
  mkdir -m 700 "$release/data"
  cat > "$release/start-server.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export COLLAB_API=realtime COLLAB_BIND=127.0.0.1 COLLAB_PORT="${COLLAB_PORT:-4445}"
export COLLAB_JOURNAL="$dir/data/journal.sqlite"
exec "$dir/collab-server"
SH
  chmod 700 "$release/start-server.sh"
  printf 'Optional local server (not started automatically): %s/start-server.sh\n' "$release"
  echo 'Its journal contains document text without encryption. Remote clients need a configured HTTPS proxy.'
fi
printf '\nIn OpenIDEA, use Settings → Plugins → Install Plugin from Package File… and select:\n%s/plugin.oiplugin\n' "$release"
printf 'Under the plugin permission process:openidea-collab-helper, choose and approve:\n%s/openidea-collab-helper\n' "$release"
echo 'Open a .collabtest file. For another machine use a separately configured HTTPS Collab server; localhost refers to this machine.'
