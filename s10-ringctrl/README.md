# S10 RingCtrl

Linux input remapper for the S10 Bluetooth Remote (JieLi BR17 chip, fake Apple Magic Trackpad 05AC:0220).

## Features

- **Swipe gestures**: Up/Down/Left/Right on the touch ring
- **Single-tap Mod**: Switch between profiles (herdr ↔ Hyprland)
- **Single-tap Play**: Send configurable text/key
- **Power key**: Escape
- **Dual-profile TOML configs** with bidirectional switching via `alt_config`

## Dependencies

- `wtype` (for key/text injection on Wayland)
- `evdev` access to `/dev/input/event*` (handled by udev rules)

## Install

### Option A: `install.sh` (any distro)

```bash
cd s10-ringctrl
./install.sh
```

Then start the service:
```bash
systemctl --user start s10-ringctrl
systemctl --user status s10-ringctrl
journalctl --user -u s10-ringctrl -f
```

### Option B: `make` (any distro)

```bash
cd s10-ringctrl
make
sudo make install
systemctl --user enable --now s10-ringctrl
```

### Option C: Arch Linux (AUR)

```bash
cd s10-ringctrl
makepkg -si
systemctl --user enable --now s10-ringctrl
```

## Uninstall

```bash
cd s10-ringctrl
sudo make uninstall
systemctl --user disable s10-ringctrl
```

## Config

Edit `/etc/s10-ringctrl.toml` (or `/etc/s10-ringctrl-hyprland.toml`).

Key mappings support prefixes:
- `command:` — shell command
- `key:` — key sequence via `wtype`
- `text:` — text input via `wtype`

## Hardware

| Button | Input Source | Gesture |
|--------|-------------|---------|
| Up/Down/Left/Right | touch | swipe |
| Mod | touch (y > 600) | tap → switch config |
| Play | touch (y < 600) | tap → TAP mapping |
| Power | consumer | KEY_VOLUMEDOWN → Escape |

## License

MIT
