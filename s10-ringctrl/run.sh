#!/bin/bash
# S10 RingCtrl launcher — preserves env vars needed by herdr and Hyprland
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$DIR/target/release/s10_ringctrl"
CONFIG="$DIR/s10-ringctrl.toml"

# Hyprland instance signature (needed for hyprctl)
export HYPRLAND_INSTANCE_SIGNATURE="${HYPRLAND_INSTANCE_SIGNATURE:-$(ls /run/user/1000/hypr/ 2>/dev/null | head -1)}"

# herdr socket path (needed when running under sudo)
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/1000}"

# Wayland display for wtype
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-1}"

# Validate binary exists
if [[ ! -x "$BINARY" ]]; then
    echo "Error: $BINARY not found. Run: cargo build --release"
    exit 1
fi

# Parse args
REMAP=""
if [[ "$1" == "--remap" ]]; then
    REMAP="--remap"
    echo "Starting S10 RingCtrl in REMAP mode..."
else
    echo "Starting S10 RingCtrl in DEBUG mode (add --remap for active remapping)..."
fi

# Run with sudo, preserving all critical env vars
exec sudo -E "$BINARY" $REMAP -c "$CONFIG"
