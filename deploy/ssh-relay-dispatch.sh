#!/bin/bash
# Forced command: the deploy key cannot obtain a shell or forward connections.
set -euo pipefail
read -r commit checksum extra <<< "${SSH_ORIGINAL_COMMAND:-}"
[[ -z ${extra:-} && $commit =~ ^[0-9a-f]{40}$ && $checksum =~ ^[0-9a-f]{64}$ ]] || exit 2
exec /usr/bin/sudo -n /usr/local/sbin/openidea-collab-activate "$commit" "$checksum"
