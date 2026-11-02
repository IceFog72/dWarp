#!/usr/bin/env bash
#
# Build and install Warp to the system.
#
# Usage:
#   ./script/build-and-install.sh [oss]
#   ./script/build-and-install.sh --channel oss
#
# Environment variables:
#   WARP_CPU_RENDERER=1          Use the softbuffer CPU renderer instead of wgpu
#   WARP_CPU_FPS=30              Cap FPS for every CPU renderer frame class
#   WARP_CPU_TERMINAL_FPS=15     Cap terminal spinner/output redraw FPS
#

set -euo pipefail

WORKSPACE_ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$WORKSPACE_ROOT_DIR/logs/build-and-install"
mkdir -p "$LOG_DIR"
LOG_FILE="$(mktemp "$LOG_DIR/$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX.log")"

# Keep logging alive long enough to record cleanup after a group interrupt.
exec > >(trap '' INT TERM; exec tee "$LOG_FILE") 2>&1
LOG_TEE_PID=$!
BUILD_TEMP_DIR=""
cleanup() {
  local status=$?
  trap - EXIT
  if [[ -n "$BUILD_TEMP_DIR" ]] && ! rm -rf -- "$BUILD_TEMP_DIR"; then
    echo "Could not remove build temporary files: $BUILD_TEMP_DIR" >&2
    [[ $status -ne 0 ]] || status=1
  fi
  printf '\nBuild/install exited with status %s. Log: %s\n' "$status" "$LOG_FILE"
  exec 1>&- 2>&-
  if ! wait "$LOG_TEE_PID"; then
    [[ $status -ne 0 ]] || status=1
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
echo "Build/install log: $LOG_FILE"
cd "$WORKSPACE_ROOT_DIR"

if [[ "${1:-}" == "--channel" ]]; then
  CHANNEL="${2:-oss}"
elif [[ "${1:-}" == --channel=* ]]; then
  CHANNEL="${1#--channel=}"
else
  CHANNEL="${1:-oss}"
fi
CARGO_PROFILE="release-lto"

case "$CHANNEL" in
  oss)
    WARP_BIN="dwarp"
    BINARY_NAME="dwarp"
    DESKTOP_COMMAND="dwarp"
    APP_NAME="dWarp"
    BUNDLE_ID="dev.warp.dWarp"
    FEATURES="release_bundle,gui,pprof_cpu_profiling"
    ;;
  *)
    echo "Unknown channel: $CHANNEL (use oss)"
    exit 1
    ;;
esac

echo "=== Installing build dependencies ==="
"$WORKSPACE_ROOT_DIR/script/linux/install_runtime_deps"

echo ""
echo "=== Building Warp ($CHANNEL channel) ==="
echo "  Binary: $WARP_BIN"
echo "  Profile: $CARGO_PROFILE"
echo "  Features: $FEATURES"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WORKSPACE_ROOT_DIR/target}"
mkdir -p "$CARGO_TARGET_DIR"
CARGO_TARGET_DIR="$(cd "$CARGO_TARGET_DIR" && pwd)"
export CARGO_TARGET_DIR
BUILD_TEMP_DIR="$(mktemp -d "$CARGO_TARGET_DIR/build-install.XXXXXXXX")"
echo "$$" > "$BUILD_TEMP_DIR/.owner-pid"
export TMPDIR="$BUILD_TEMP_DIR"
cargo build -p warp --profile "$CARGO_PROFILE" --bin "$WARP_BIN" --features "$FEATURES"

EXECUTABLE_PATH="$CARGO_TARGET_DIR/$CARGO_PROFILE/$WARP_BIN"
if [[ ! -x "$EXECUTABLE_PATH" ]]; then
  echo "Built executable not found: $EXECUTABLE_PATH"
  exit 1
fi

echo ""
echo "=== Installing to system ==="

# Remove legacy launchers only after a successful build. Shared settings stay intact.
if [[ "$CHANNEL" == "oss" ]]; then
  sudo "$WORKSPACE_ROOT_DIR/script/uninstall.sh" --legacy-only
fi

# Install binary
RUNTIME_COMMAND="/usr/local/bin/$DESKTOP_COMMAND"
REAL_COMMAND="/usr/local/bin/$DESKTOP_COMMAND.real"
sudo install -D "$EXECUTABLE_PATH" "$REAL_COMMAND"

if [[ -n "${WARP_CPU_RENDERER:-}" || -n "${WARP_CPU_FPS:-}" || -n "${WARP_CPU_TERMINAL_FPS:-}" ]]; then
  WRAPPER_PATH="$BUILD_TEMP_DIR/launcher"
  cat > "$WRAPPER_PATH" <<EOF
#!/usr/bin/env bash
export WARP_CPU_RENDERER="${WARP_CPU_RENDERER:-}"
export WARP_CPU_FPS="${WARP_CPU_FPS:-}"
export WARP_CPU_TERMINAL_FPS="${WARP_CPU_TERMINAL_FPS:-}"
exec "$REAL_COMMAND" "\$@"
EOF
  chmod 755 "$WRAPPER_PATH"
  sudo install -Dm755 "$WRAPPER_PATH" "$RUNTIME_COMMAND"
  rm -f "$WRAPPER_PATH"
  echo "  Installed runtime wrapper: $RUNTIME_COMMAND"
  echo "  Installed executable: $REAL_COMMAND"
else
  sudo ln -sf "$REAL_COMMAND" "$RUNTIME_COMMAND"
  echo "  Installed executable: $REAL_COMMAND"
  echo "  Installed runtime symlink: $RUNTIME_COMMAND"
fi
if [[ "$BINARY_NAME" != "$DESKTOP_COMMAND" ]]; then
  sudo ln -sf "$RUNTIME_COMMAND" "/usr/local/bin/$BINARY_NAME"
fi
echo "  Installed binary: /usr/local/bin/$DESKTOP_COMMAND"


# Install .desktop file
DESKTOP_SRC="$WORKSPACE_ROOT_DIR/app/channels/$CHANNEL/$BUNDLE_ID.desktop"
if [ -f "$DESKTOP_SRC" ]; then
  sudo install -Dm644 "$DESKTOP_SRC" "/usr/share/applications/$BUNDLE_ID.desktop"
  echo "  Installed desktop entry: /usr/share/applications/$BUNDLE_ID.desktop"
fi

# Register the terminal in XFCE's preferred-application chooser.
HELPER_SRC="$WORKSPACE_ROOT_DIR/app/channels/$CHANNEL/xfce4-helper.desktop"
if [ -f "$HELPER_SRC" ]; then
  sudo install -Dm644 "$HELPER_SRC" "/usr/share/xfce4/helpers/$BUNDLE_ID.desktop"
fi

# Install icons
ICON_DIR="$WORKSPACE_ROOT_DIR/app/channels/$CHANNEL/icon/no-padding"
if [ -d "$ICON_DIR" ]; then
  for icon in "$ICON_DIR"/*.png; do
    [ -e "$icon" ] || continue
    size="$(basename "$icon" .png)"
    sudo install -Dm644 "$icon" "/usr/share/icons/hicolor/${size}/apps/$BUNDLE_ID.png"
  done
  echo "  Installed icons"
fi

echo ""
echo "=== Build & install complete ==="
echo ""
echo "Run '$DESKTOP_COMMAND'"
echo ""
echo "You may need to run: sudo update-desktop-database"
echo "And:                sudo update-icon-caches /usr/share/icons/*"
