#!/usr/bin/env bash
# Remove dWarp/OpenWarp installations. Use --purge to also delete local user data.
set -euo pipefail
exec python3 "$(cd "$(dirname "$0")" && pwd)/uninstall.py" "$@"
