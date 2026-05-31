#!/usr/bin/env python3
"""
S10 Bluetooth Remote Input Remapper
Maps S10 directional buttons and consumer keys to custom actions.
Uses 'wtype' for key simulation on Wayland.

Requirements:
    sudo pacman -S wtype python-evdev

Usage:
    sudo python3 s10-remapper.py              # debug mode
    sudo python3 s10-remapper.py --remap      # active remapping
"""

import os
import sys
import json
import time
import subprocess
import select
import argparse
import shutil
from pathlib import Path

try:
    from evdev import InputDevice, ecodes
except ImportError:
    print("Error: python-evdev not installed. Run: sudo pacman -S python-evdev")
    sys.exit(1)

DEFAULT_CONFIG = {
    "device_consumer": "/dev/input/event23",
    "device_touch": "/dev/input/event22",
    "mappings_direction": {
        "UP":         "command:hyprctl dispatch cyclenext prev",
        "DOWN":       "command:hyprctl dispatch cyclenext",
        "LEFT":       "command:hyprctl dispatch workspace e-1",
        "RIGHT":      "command:hyprctl dispatch workspace e+1",
        "TAP":        "text:继续\\n",
        "DOUBLE_TAP": "command:hyprctl dispatch togglespecialworkspace"
    },
    "mappings_consumer": {
        "KEY_VOLUMEUP": "command:hyprctl dispatch exec 'pamixer -i 5 && notify-send \"Volume +\"'"
    },
    "swipe": {
        "threshold": 80,
        "double_tap_ms": 500
    },
    "grab": True,
    "verbose": True
}


class TouchDetector:
    def __init__(self, threshold=80, double_tap_ms=500):
        self.threshold = threshold
        self.double_tap_ms = double_tap_ms / 1000.0
        self.reset()
        self._last_tap = None
    
    def reset(self):
        self.active = False
        self.start_x = None
        self.start_y = None
        self.end_x = None
        self.end_y = None
        self.tap_start_time = None
    
    def feed(self, event):
        if event.type == ecodes.EV_ABS:
            if event.code == ecodes.ABS_MT_TRACKING_ID:
                if event.value >= 0 and not self.active:
                    self.active = True
                    self.start_x = None
                    self.start_y = None
                    self.tap_start_time = time.time()
                elif event.value < 0 and self.active:
                    return self._resolve()
            elif event.code == ecodes.ABS_MT_POSITION_X:
                if self.active:
                    if self.start_x is None:
                        self.start_x = event.value
                    self.end_x = event.value
            elif event.code == ecodes.ABS_MT_POSITION_Y:
                if self.active:
                    if self.start_y is None:
                        self.start_y = event.value
                    self.end_y = event.value
            elif event.code == ecodes.ABS_X:
                if self.active and self.start_x is None:
                    self.start_x = event.value
                self.end_x = event.value
            elif event.code == ecodes.ABS_Y:
                if self.active and self.start_y is None:
                    self.start_y = event.value
                self.end_y = event.value
        return None
    
    def _resolve(self):
        self.active = False
        now = time.time()
        
        has_x = self.start_x is not None and self.end_x is not None
        has_y = self.start_y is not None and self.end_y is not None
        has_pos = has_x or has_y
        
        dx = (self.end_x - self.start_x) if has_x else 0
        dy = (self.end_y - self.start_y) if has_y else 0
        
        if has_pos and (abs(dx) >= self.threshold or abs(dy) >= self.threshold):
            self._last_tap = None
            result = None
            if has_x and has_y:
                if abs(dx) > abs(dy):
                    result = "LEFT" if dx > 0 else "RIGHT"
                else:
                    result = "UP" if dy > 0 else "DOWN"
            elif has_x:
                result = "LEFT" if dx > 0 else "RIGHT"
            elif has_y:
                result = "UP" if dy > 0 else "DOWN"
            self.reset()
            return result
        
        tap_x = self.end_x if has_x else (self.start_x if self.start_x is not None else 0)
        tap_y = self.end_y if has_y else (self.start_y if self.start_y is not None else 0)
        tap = (now, tap_x, tap_y)
        
        if self._last_tap is not None:
            last_time, last_x, last_y = self._last_tap
            if now - last_time < self.double_tap_ms:
                self._last_tap = None
                self.reset()
                return "DOUBLE_TAP"
        
        self._last_tap = tap
        self.reset()
        return "TAP"


class S10Remapper:
    def __init__(self, config_path=None, remap_mode=False):
        self.config = self._load_config(config_path)
        self.remap_mode = remap_mode
        self.running = False
        self.detector = TouchDetector(
            threshold=self.config.get("swipe", {}).get("threshold", 80),
            double_tap_ms=self.config.get("swipe", {}).get("double_tap_ms", 500)
        )
        self.wtype_available = shutil.which("wtype") is not None
        self._hypr_env = self._discover_hypr_env()
    
    def _discover_hypr_env(self):
        """Discover Hyprland environment variables needed for hyprctl."""
        env = {}
        
        # Try to find HYPRLAND_INSTANCE_SIGNATURE from runtime dir
        import glob
        for hypr_dir in glob.glob('/run/user/*/hypr/*'):
            if os.path.isdir(hypr_dir):
                sig = os.path.basename(hypr_dir)
                if '_' in sig:
                    env['HYPRLAND_INSTANCE_SIGNATURE'] = sig
                    env['XDG_RUNTIME_DIR'] = os.path.dirname(os.path.dirname(hypr_dir))
                    break
        
        # Also grab WAYLAND_DISPLAY from user environment if available
        for wayland in ['wayland-1', 'wayland-0', 'wayland-2']:
            for run_dir in glob.glob('/run/user/*/'):
                if os.path.exists(os.path.join(run_dir, wayland)):
                    env['WAYLAND_DISPLAY'] = wayland
                    if 'XDG_RUNTIME_DIR' not in env:
                        env['XDG_RUNTIME_DIR'] = run_dir.rstrip('/')
                    break
        
        return env
    
    def _load_config(self, path):
        cfg = DEFAULT_CONFIG.copy()
        if path and Path(path).exists():
            with open(path) as f:
                user = json.load(f)
            for k, v in user.items():
                if k in cfg and isinstance(v, dict) and isinstance(cfg[k], dict):
                    cfg[k].update(v)
                else:
                    cfg[k] = v
        return cfg
    
    def _find_devices(self):
        consumer = self.config["device_consumer"]
        touch = self.config["device_touch"]
        
        import glob
        for uevent in glob.glob('/sys/class/input/event*/device/uevent'):
            try:
                with open(uevent) as f:
                    content = f.read()
                if 'NAME="S10"' in content and 'Consumer' not in content:
                    name = uevent.split('/')[-3]
                    touch = f'/dev/input/{name}'
                elif 'NAME="S10 Consumer Control"' in content:
                    name = uevent.split('/')[-3]
                    consumer = f'/dev/input/{name}'
            except Exception:
                pass
        return consumer, touch
    
    def _wtype_key(self, key_name):
        """Send key via wtype."""
        try:
            if key_name.startswith("KEY_"):
                wtype_name = key_name[4:]
            else:
                wtype_name = key_name
            
            env = {**os.environ}
            env.update(self._hypr_env)
            if 'XDG_CURRENT_DESKTOP' not in env:
                env['XDG_CURRENT_DESKTOP'] = 'Hyprland'
            subprocess.run(['wtype', wtype_name], capture_output=True, text=True, timeout=3, env=env)
            return True
        except Exception as e:
            print(f"    -> [wtype error] {e}")
            return False
    
    def _execute(self, action, source=""):
        if not action or action == "passthrough":
            return False
        verbose = self.config.get("verbose", True)
        
        # Build environment with Hyprland vars
        base_env = {**os.environ}
        base_env.update(self._hypr_env)
        if 'DISPLAY' not in base_env:
            base_env['DISPLAY'] = ':1'
        if 'XDG_CURRENT_DESKTOP' not in base_env:
            base_env['XDG_CURRENT_DESKTOP'] = 'Hyprland'
        
        if action.startswith("key:"):
            key_name = action.split(":", 1)[1]
            if self.wtype_available:
                ok = self._wtype_key(key_name)
                if verbose and ok:
                    print(f"    -> [wtype] {key_name}")
            else:
                code = getattr(ecodes, key_name, None)
                if code is not None:
                    with open('/tmp/s10_keys.txt', 'a') as f:
                        f.write(f"{key_name}\n")
                    if verbose:
                        print(f"    -> [fallback] {key_name} (install wtype for real input)")
                else:
                    print(f"    -> Unknown key: {key_name}")
            return True
        
        elif action.startswith("combo:"):
            keys = action.split(":", 1)[1].split("+")
            if self.wtype_available:
                wtype_args = []
                for k in keys:
                    name = k.strip()
                    if name.startswith("KEY_"):
                        name = name[4:]
                    wtype_args.extend(['-k', name])
                subprocess.run(['wtype'] + wtype_args, capture_output=True, text=True, timeout=3, env=base_env)
                if verbose:
                    print(f"    -> [wtype combo] {'+'.join(keys)}")
            else:
                print(f"    -> [fallback combo] {'+'.join(keys)} (install wtype)")
            return True
        
        elif action.startswith("text:"):
            text = action.split(":", 1)[1]
            if self.wtype_available:
                try:
                    # Split by \n to support multi-line text with Return keys
                    parts = text.split('\\n')
                    for i, part in enumerate(parts):
                        if part:
                            subprocess.run(['wtype', part], capture_output=True, text=True, timeout=3, env=base_env)
                        # Send Return after each part except the last one (unless text ends with \n)
                        if i < len(parts) - 1 or (i == len(parts) - 1 and text.endswith('\\n')):
                            subprocess.run(['wtype', '-k', 'Return'], capture_output=True, text=True, timeout=3, env=base_env)
                    if verbose:
                        display = text.replace('\\n', '[Enter]')
                        print(f"    -> [wtype text] {display[:50]}")
                except Exception as e:
                    print(f"    -> [wtype text fail] {e}")
            else:
                print(f"    -> [wtype not found] cannot type text: {text[:50]}")
            return True
        
        elif action.startswith("command:"):
            cmd = action.split(":", 1)[1]
            try:
                result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=5, env=base_env)
                if verbose:
                    print(f"    -> [command] {cmd}")
                    if result.returncode != 0 and result.stderr:
                        err = result.stderr.strip()
                        print(f"       Error: {err[:200]}")
                    elif result.stdout.strip():
                        out = result.stdout.strip()
                        print(f"       Output: {out[:100]}")
            except Exception as e:
                print(f"    -> [command fail] {e}")
            return True
        
        return False
    
    def run(self):
        consumer_path, touch_path = self._find_devices()
        
        mode = "REMAP" if self.remap_mode else "DEBUG"
        print("╔══════════════════════════════════════════════╗")
        print(f"║     S10 Bluetooth Remote  ({mode:10s})       ║")
        print("╚══════════════════════════════════════════════╝")
        
        if self.remap_mode and not self.wtype_available:
            print("  WARNING: wtype not found. Install with: sudo pacman -S wtype")
            print("  Falling back to /tmp/s10_keys.txt for key actions.\n")
        elif self.remap_mode:
            print("  Backend:  wtype (Wayland key injection)\n")
        
        devices = {}
        
        if Path(consumer_path).exists():
            consumer = InputDevice(consumer_path)
            print(f"  Consumer: {consumer.name}")
            if self.config.get("grab", True):
                consumer.grab()
                print("    [grabbed]")
            devices[consumer.fd] = (consumer, "consumer")
        else:
            print(f"  [MISSING] Consumer: {consumer_path}")
        
        if Path(touch_path).exists():
            touch = InputDevice(touch_path)
            print(f"  Touch:    {touch.name}")
            if self.config.get("grab", True):
                touch.grab()
                print("    [grabbed]")
            devices[touch.fd] = (touch, "touch")
        else:
            print(f"  [MISSING] Touch: {touch_path}")
        
        if not devices:
            print("No devices found!")
            sys.exit(1)
        
        print("\n  Active. Press Ctrl+C to stop.\n")
        
        self.running = True
        while self.running:
            try:
                r, _, _ = select.select(devices.keys(), [], [], 0.5)
                for fd in r:
                    dev, dtype = devices[fd]
                    for event in dev.read():
                        if dtype == "consumer":
                            self._handle_consumer(event)
                        elif dtype == "touch":
                            self._handle_touch(event)
            except (select.error, OSError):
                time.sleep(0.1)
            except KeyboardInterrupt:
                break
        
        self._shutdown(devices)
    
    def _handle_consumer(self, event):
        if event.type != ecodes.EV_KEY:
            return
        code_name = ecodes.KEY.get(event.code, f"CODE_{event.code}")
        action = self.config["mappings_consumer"].get(code_name, "passthrough")
        state = {0: "RELEASE", 1: "PRESS  ", 2: "REPEAT "}.get(event.value, str(event.value))
        mapped = "[*]" if action != "passthrough" else ""
        
        print(f"[CONSUMER] {state} {code_name:20s} -> {action} {mapped}")
        
        if not self.remap_mode:
            return
        
        if action != "passthrough":
            if event.value == 1:
                self._execute(action, code_name)
            return
    
    def _handle_touch(self, event):
        if not self.remap_mode:
            if event.type == ecodes.EV_ABS:
                code_name = ecodes.ABS.get(event.code, f"ABS_{event.code}")
                print(f"[TOUCH   ] ABS  {code_name:20s} value={event.value}")
            elif event.type == ecodes.EV_KEY:
                code_name = ecodes.KEY.get(event.code, f"CODE_{event.code}")
                print(f"[TOUCH   ] KEY  {code_name:20s} value={event.value}")
        
        gesture = self.detector.feed(event)
        if gesture:
            action = self.config["mappings_direction"].get(gesture, "passthrough")
            if self.remap_mode:
                mapped = "[*]" if action != "passthrough" else ""
                print(f"[TOUCH   ] {gesture:11s} -> {action} {mapped}")
                if action != "passthrough":
                    self._execute(action, f"touch:{gesture}")
            else:
                print(f"[TOUCH   ] >>> {gesture} DETECTED <<<")
    
    def _shutdown(self, devices):
        print("\nShutting down...")
        self.running = False
        for dev, _ in devices.values():
            try:
                dev.ungrab()
                dev.close()
            except Exception:
                pass
        print("Stopped.")


def main():
    parser = argparse.ArgumentParser(description='S10 Bluetooth Remote')
    parser.add_argument('-c', '--config', help='Path to JSON config')
    parser.add_argument('--remap', action='store_true', help='Enable active remapping')
    parser.add_argument('--no-grab', action='store_true', help='Do not grab devices')
    parser.add_argument('-q', '--quiet', action='store_true', help='Quiet output')
    args = parser.parse_args()
    
    remapper = S10Remapper(args.config, remap_mode=args.remap)
    if args.no_grab:
        remapper.config["grab"] = False
    if args.quiet:
        remapper.config["verbose"] = False
    
    if os.geteuid() != 0:
        print("Run with: sudo python3 s10-remapper.py --remap")
        sys.exit(1)
    
    remapper.run()


if __name__ == '__main__':
    main()
