#!/bin/bash
# Installed root-owned on the relay host. Accept only a commit, checksum and ELF on stdin.
set -Eeuo pipefail
umask 077
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH
commit=${1:-} checksum=${2:-}
[[ $# == 2 && $commit =~ ^[0-9a-f]{40}$ && $checksum =~ ^[0-9a-f]{64}$ ]] || exit 2
base=/opt/openidea-collab/releases
binary=/usr/local/bin/collab-server
mkdir -p "$base"
exec 9>/run/lock/openidea-collab-deploy.lock
flock -n 9 || { echo 'Another deployment is running' >&2; exit 1; }
artifact=$(mktemp "$base/.incoming.XXXXXXXX")
link="${binary}.next"
cleanup() { rm -f -- "$artifact" "$link"; }
trap cleanup EXIT
timeout 120 head -c 25000001 > "$artifact"
[[ $(stat -c %s "$artifact") -le 25000000 ]] || { echo 'Artifact too large' >&2; exit 1; }
[[ $(sha256sum "$artifact" | cut -d' ' -f1) == "$checksum" ]] || { echo 'Checksum mismatch' >&2; exit 1; }
[[ $(od -An -tx1 -N4 "$artifact" | tr -d ' \n') == 7f454c46 ]] || { echo 'Not an ELF executable' >&2; exit 1; }
release="$base/$commit"
if [[ -e "$release" ]]; then
 [[ -f "$release" && ! -L "$release" && $(sha256sum "$release" | cut -d' ' -f1) == "$checksum" ]] || { echo 'Existing release differs' >&2; exit 1; }
else
 chmod 755 "$artifact"
 mv -T -- "$artifact" "$release"
fi
previous=$(readlink -f "$binary")
[[ $previous == "$base/"* && -f "$previous" && -x "$previous" ]] || { echo 'No safe previous release' >&2; exit 1; }
healthy() {
 local attempt
 for attempt in {1..15}; do
  if systemctl is-active --quiet openidea-collab && curl -fsS --max-time 3 http://127.0.0.1:4444/healthz >/dev/null 2>&1; then return 0; fi
  sleep 1
 done
 return 1
}
if [[ $previous == "$release" ]]; then healthy; exit; fi
ln -s -- "$release" "$link"
mv -Tf -- "$link" "$binary"
if systemctl restart openidea-collab && healthy; then
 echo "Active relay commit: $commit"
 exit 0
fi
echo 'New release failed health check; restoring previous binary' >&2
ln -s -- "$previous" "$link"
mv -Tf -- "$link" "$binary"
if ! systemctl restart openidea-collab || ! healthy; then
 echo 'Rollback failed health check; operator intervention required' >&2
 exit 1
fi
echo 'Previous binary restored and healthy' >&2
exit 1
