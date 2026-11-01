#!/usr/bin/env bash

# Run a check with disposable Cargo output, preserving shared build caches and logs.
set -euo pipefail

if [[ $# -eq 0 ]]; then
    echo "Usage: $0 COMMAND [ARGUMENT ...]" >&2
    echo "Example: $0 cargo check -p warp --bin dwarp" >&2
    echo "Set DWARP_DEBUG_TMPDIR to choose a temporary directory on a larger disk." >&2
    exit 2
fi

debug_temp_root="${DWARP_DEBUG_TMPDIR:-${TMPDIR:-/tmp}}"
debug_target_dir="$(mktemp -d "$debug_temp_root/dwarp-debug.XXXXXXXX")"

cleanup() {
    local status=$?
    trap - EXIT
    if rm -rf -- "$debug_target_dir"; then
        echo "Removed temporary Cargo output: $debug_target_dir" >&2
    else
        echo "Could not remove temporary Cargo output: $debug_target_dir" >&2
        if [[ $status -eq 0 ]]; then
            status=1
        fi
    fi
    exit "$status"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

export CARGO_TARGET_DIR="$debug_target_dir"
echo "Temporary Cargo output: $CARGO_TARGET_DIR" >&2
"$@"
