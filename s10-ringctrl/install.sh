#!/bin/bash
set -e

PREFIX="${PREFIX:-/usr}"
BINDIR="$PREFIX/bin"
SYSCONFDIR="/etc"
SYSTEMD_USER_DIR="$PREFIX/lib/systemd/user"
UDEV_RULES_DIR="$PREFIX/lib/udev/rules.d"

echo "==> Building s10-ringctrl..."
cargo build --release

echo "==> Installing binary..."
sudo install -Dm755 target/release/s10_ringctrl "$BINDIR/s10-ringctrl"
sudo install -Dm755 s10-herdr.sh "$BINDIR/s10-herdr.sh"

echo "==> Installing configs..."
sudo install -Dm644 s10-ringctrl.toml "$SYSCONFDIR/s10-ringctrl.toml"
sudo install -Dm644 s10-ringctrl-hyprland.toml "$SYSCONFDIR/s10-ringctrl-hyprland.toml"

echo "==> Installing systemd user service..."
sudo install -Dm644 s10-ringctrl.user.service "$SYSTEMD_USER_DIR/s10-ringctrl.service"

echo "==> Installing udev rules..."
sudo install -Dm644 99-s10-ringctrl.rules "$UDEV_RULES_DIR/99-s10-ringctrl.rules"
sudo udevadm control --reload-rules
sudo udevadm trigger

echo "==> Enabling user service..."
systemctl --user daemon-reload
systemctl --user enable s10-ringctrl.service

echo ""
echo "Installation complete!"
echo "  Start:  systemctl --user start s10-ringctrl"
echo "  Status: systemctl --user status s10-ringctrl"
echo "  Logs:   journalctl --user -u s10-ringctrl -f"
