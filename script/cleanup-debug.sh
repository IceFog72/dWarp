#!/usr/bin/env bash

# Run a check with disposable Cargo output, preserving shared build caches and logs.
set -euo pipefail

if [[ $# -eq 0 ]]; then
    echo "Usage: $0 COMMAND [ARGUMENT ...]" >&2
    echo "Example: $0 cargo check -p warp --bin dwarp" >&2
    echo "Use $0 --clean to remove abandoned disposable builds (including /tmp)." >&2
    echo "Set DWARP_DEBUG_TMPDIR to choose a temporary directory on a larger disk." >&2
    exit 2
fi

project_root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
debug_temp_root="${DWARP_DEBUG_TMPDIR:-$project_root/target}"

if [[ $1 == --clean ]]; then
    if [[ $# -gt 2 ]]; then
        echo "Usage: $0 --clean [DIRECTORY]" >&2
        exit 2
    fi
    # Older disposable builds have no owner marker. Refuse cleanup while Cargo or
    # rustc is running rather than risk removing their output.
    if ! command -v pgrep >/dev/null; then
        echo "pgrep is required to check for active builds before cleanup." >&2
        exit 1
    fi
    for build_process in cargo rustc; do
        if pgrep -x "$build_process" >/dev/null; then
            echo "Build process $build_process is running; retry cleanup after it exits." >&2
            exit 1
        else
            search_status=$?
            if [[ $search_status -ne 1 ]]; then
                echo "Could not check for active $build_process processes." >&2
                exit 1
            fi
        fi
    done
    cleanup_roots=("$debug_temp_root" /tmp)
    if [[ $# -eq 2 ]]; then
        cleanup_roots=("$2")
    fi
    shopt -s nullglob
    for cleanup_root in "${cleanup_roots[@]}"; do
        for abandoned_dir in "$cleanup_root"/dwarp-debug.???????? "$cleanup_root"/build-install.????????; do
            [[ -d $abandoned_dir && ! -L $abandoned_dir && -O $abandoned_dir ]] || continue
            if [[ -f $abandoned_dir/.owner-pid ]]; then
                owner_pid="$(cat -- "$abandoned_dir/.owner-pid")"
                if [[ $owner_pid =~ ^[1-9][0-9]*$ ]] && kill -0 "$owner_pid" 2>/dev/null; then
                    echo "Keeping active temporary build: $abandoned_dir" >&2
                    continue
                fi
            fi
            rm -rf -- "$abandoned_dir"
            echo "Removed abandoned Cargo output: $abandoned_dir" >&2
        done
    done
    exit 0
fi

mkdir -p -- "$debug_temp_root"
debug_temp_root="$(cd -- "$debug_temp_root" && pwd)"
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

echo "$$" > "$debug_target_dir/.owner-pid"
export CARGO_TARGET_DIR="$debug_target_dir"
echo "Temporary Cargo output: $CARGO_TARGET_DIR" >&2
"$@"
