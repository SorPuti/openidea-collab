#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
tmp_root="$(mktemp -d)"
trap 'rm -rf "$tmp_root"' EXIT
installer="$repo_root/scripts/install-realtime.sh"
output="$(bash "$installer" --mode local --root "$tmp_root/install with spaces" --dry-run)"
[[ "$output" == *"local"* && "$output" == *"install with spaces"* ]]
[[ ! -e "$tmp_root/install with spaces" ]]
output="$(bash "$installer" --mode local --root "$tmp_root/persistent" --start-server --persist --dry-run)"
[[ "$output" == *"Persistent server data: true"* ]]
[[ ! -e "$tmp_root/persistent" ]]
if bash "$installer" --mode local --root "$tmp_root/persistent" --persist --dry-run >/dev/null 2>&1; then echo 'persistence must require server start' >&2; exit 1; fi
if bash "$installer" --mode remote --endpoint 'https://example.com' --persist --root "$tmp_root/remote" --dry-run >/dev/null 2>&1; then echo 'remote mode must not configure local persistence' >&2; exit 1; fi
if bash "$installer" --mode remote --root "$tmp_root/remote" --dry-run >/dev/null 2>&1; then echo 'remote must require endpoint' >&2; exit 1; fi
if bash "$installer" --mode remote --endpoint 'http://example.com' --root "$tmp_root/remote" --dry-run >/dev/null 2>&1; then echo 'remote must require HTTPS' >&2; exit 1; fi
if bash "$installer" --mode managed --endpoint 'https://user:pass@example.com' --root "$tmp_root/remote" --dry-run >/dev/null 2>&1; then echo 'credentials must not be in URL' >&2; exit 1; fi
if bash "$installer" --mode remote --endpoint 'https://example.com' --start-server --root "$tmp_root/remote" --dry-run >/dev/null 2>&1; then echo 'remote mode must not start local server' >&2; exit 1; fi
[[ ! -e "$tmp_root/remote" ]]
bash "$repo_root/install.sh" --experimental --mode local --root "$tmp_root/dispatch" --dry-run >/dev/null
[[ ! -e "$tmp_root/dispatch" ]]
bash "$repo_root/install.sh" --mode local --root "$tmp_root/direct" --dry-run >/dev/null
[[ ! -e "$tmp_root/direct" ]]
python3 - "$installer" "$tmp_root/occupied" <<'PY'
import pathlib, socket, subprocess, sys
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0)); sock.listen()
    result = subprocess.run(['bash', sys.argv[1], '--mode', 'local', '--root', sys.argv[2], '--port', str(sock.getsockname()[1]), '--start-server'], capture_output=True, text=True)
    assert result.returncode != 0 and 'already in use' in result.stderr
    assert not pathlib.Path(sys.argv[2]).exists()
PY
echo 'PASS: instalação experimental: dry-run, URL e isolamento por modo'
