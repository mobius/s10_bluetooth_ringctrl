# S10 RingCtrl - Rust 跨平台重写方案

## 目标

用 Rust 重写 `s10-remapper`，实现统一的代码库，原生支持 Linux(Hyprland/Wayland/X11)、Windows、macOS。

---

## 核心问题：为什么选 Rust

| 维度 | Python (当前) | Rust (目标) |
|------|--------------|-------------|
| 跨平台 | 依赖系统级库（evdev 仅限 Linux） | 单二进制，条件编译原生支持三平台 |
| 部署 | 需 Python + pip + 系统包 | `cargo build --release` 单文件 |
| 性能 | 解释型，事件循环有 GC 延迟 | 零成本抽象，微秒级事件响应 |
| 启动 | 脚本加载慢 | 毫秒级冷启动 |
| 安全 | 运行时崩溃风险 | 编译期内存安全 |
| 蓝牙 | 依赖系统 BlueZ | 可选 `btleplug` 直连 BLE |

---

## 系统差异分析

S10 在三平台的表现完全不同，决定架构设计：

### Linux
- **设备暴露**：BlueZ 通过 `uhid` 创建 `/dev/input/event*` 
- **读取方式**：`evdev` 直接读取内核输入事件
- **注入方式**：`wtype` (Wayland) / `xdotool` (X11) / `uinput`
- **特点**：需要 root 或 `input` 组权限抓取设备

### Windows
- **设备暴露**：系统自动识别为 HID 触摸板 + 消费者控制
- **读取方式**：Raw Input API / Interception Driver / LowLevel hooks
- **注入方式**：`SendInput` / `keybd_event`
- **特点**：触摸板方向键与真实鼠标事件混合，需通过 **设备句柄过滤**

### macOS
- **设备暴露**：IOKit 自动枚举为 HID 设备
- **读取方式**：`IOHIDManager` + `CGEventTap`
- **注入方式**：`CGEventPost` (需 Accessibility 权限)
- **特点**：触摸板手势由系统解析为 NSEvent，低层拦截困难

---

## 架构设计

```
s10-ringctrl/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI: daemon / debug / config
│   ├── config.rs            # TOML/JSON 配置 + 热重载
│   ├── platform.rs          # cfg 平台分发
│   ├── device/              # 设备发现与识别
│   │   ├── mod.rs
│   │   ├── linux.rs         # evdev枚举 + udev匹配
│   │   ├── windows.rs       # Raw Input枚举 + VID/PID匹配
│   │   └── macos.rs         # IOHIDManager枚举
│   ├── capture/             # 输入拦截
│   │   ├── mod.rs
│   │   ├── linux.rs         # evdev GRAB
│   │   ├── windows.rs       # RawInput/Interception
│   │   └── macos.rs         # CGEventTap + IOHIDCallback
│   ├── gesture/             # 手势解析（平台无关）
│   │   ├── mod.rs
│   │   └── detector.rs      # 滑动/点击/双击状态机
│   ├── action/              # 动作执行
│   │   ├── mod.rs
│   │   ├── command.rs       # shell exec
│   │   ├── key.rs           # 按键注入
│   │   ├── text.rs          # 文字输入
│   │   └── wm/              # 窗口管理器集成
│   │       ├── mod.rs
│   │       ├── hyprland.rs  # hyprctl IPC socket
│   │       ├── sway.rs      # swaymsg IPC
│   │       ├── windows.rs   # Win32 API
│   │       └── macos.rs     # AppleScript/AX API
│   └── ipc.rs               # 可选：Unix socket控制接口
├── config.example.toml
├── windows/
│   └── s10-ringctrl.ahk     # AutoHotkey 备选方案
└── macos/
    └── karabiner.json       # Karabiner-Elements 备选方案
```

---

## 核心 Crate 选型

### 1. 配置解析
```toml
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

### 2. Linux 输入层
```toml
[target.'cfg(target_os = "linux")'.dependencies]
evdev-rs = "0.6"           # /dev/input/event* 读写
input-linux-sys = "0.9"    # ioctl 常量
udev = "0.9"               # 设备发现与监控
```

### 3. Windows 输入层
```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_Input",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Devices_HumanInterfaceDevice",
] }
rawinput = "0.4"           # Raw Input API 封装
```

### 4. macOS 输入层
```toml
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10"
core-foundation-sys = "0.8"
IOKit-sys = "0.4"
```

### 5. 跨平台按键注入
```toml
enigo = "0.2"              # Linux(X11)/Win/macOS 按键模拟
                          # ⚠️ Wayland 下无效，需回退到外部 wtype
```

### 6. 日志与 CLI
```toml
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
```

---

## 关键代码示例

### 平台分发入口

```rust
// src/platform.rs
#[cfg(target_os = "linux")]
pub use crate::device::linux::LinuxDevice as PlatformDevice;
#[cfg(target_os = "linux")]
pub use crate::capture::linux::LinuxCapture as PlatformCapture;

#[cfg(target_os = "windows")]
pub use crate::device::windows::WindowsDevice as PlatformDevice;
#[cfg(target_os = "windows")]
pub use crate::capture::windows::WindowsCapture as PlatformCapture;

#[cfg(target_os = "macos")]
pub use crate::device::macos::MacOSDevice as PlatformDevice;
#[cfg(target_os = "macos")]
pub use crate::capture::macos::MacOSCapture as PlatformCapture;
```

### Linux evdev 抓取

```rust
// src/capture/linux.rs
use evdev_rs::{Device, GrabMode, enums::EventType};

pub struct LinuxCapture {
    device: Device,
}

impl LinuxCapture {
    pub fn new(path: &str) -> Result<Self> {
        let mut device = Device::open(path)?;
        device.grab(GrabMode::Grab)?;  // EVIOCGRAB
        Ok(Self { device })
    }

    pub fn read_event(&mut self) -> Result<InputEvent> {
        loop {
            if let Some(ev) = self.device.next_event(evdev_rs::ReadFlag::NORMAL)? {
                return Ok(ev);
            }
        }
    }
}
```

### 手势检测状态机

```rust
// src/gesture/detector.rs
pub struct GestureDetector {
    threshold: i32,
    double_tap_ms: u64,
    state: TouchState,
    last_tap: Option<Instant>,
}

enum TouchState {
    Idle,
    Tracking { start_x: i32, start_y: i32, current_x: i32, current_y: i32 },
}

impl GestureDetector {
    pub fn feed(&mut self, event: &PlatformEvent) -> Option<Gesture> {
        match event {
            PlatformEvent::TrackingStart => {
                self.state = TouchState::Tracking { 
                    start_x: event.x, start_y: event.y,
                    current_x: event.x, current_y: event.y 
                };
                None
            }
            PlatformEvent::Move { x, y } => {
                if let TouchState::Tracking { ref mut current_x, ref mut current_y, .. } = self.state {
                    *current_x = *x;
                    *current_y = *y;
                }
                None
            }
            PlatformEvent::TrackingEnd => {
                let result = self.resolve();
                self.state = TouchState::Idle;
                result
            }
        }
    }

    fn resolve(&mut self) -> Option<Gesture> {
        let TouchState::Tracking { start_x, start_y, current_x, current_y } = self.state else { return None };
        let dx = current_x - start_x;
        let dy = current_y - start_y;

        // Swipe detection
        if dx.abs() > self.threshold || dy.abs() > self.threshold {
            self.last_tap = None;
            return Some(if dx.abs() > dy.abs() {
                if dx > 0 { Gesture::Left } else { Gesture::Right }
            } else {
                if dy > 0 { Gesture::Up } else { Gesture::Down }
            });
        }

        // Tap / Double-tap detection
        let now = Instant::now();
        if let Some(last) = self.last_tap {
            if now.duration_since(last).as_millis() < self.double_tap_ms as u128 {
                self.last_tap = None;
                return Some(Gesture::DoubleTap);
            }
        }
        self.last_tap = Some(now);
        Some(Gesture::Tap)
    }
}
```

### Windows Raw Input 拦截

```rust
// src/capture/windows.rs
use windows::Win32::UI::Input::*;

pub struct WindowsCapture {
    device_handle: HANDLE,
}

impl WindowsCapture {
    pub fn register(hwnd: HWND) {
        let rid = RAWINPUTDEVICE {
            usUsagePage: 0x01,      // Generic Desktop
            usUsage: 0x02,          // Mouse (S10 方向键走触摸模拟)
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        };
        unsafe {
            RegisterRawInputDevices(&[rid], size_of::<RAWINPUTDEVICE>() as u32)
                .expect("RegisterRawInputDevices failed");
        }
    }

    pub fn parse(raw: &RAWINPUT) -> Option<PlatformEvent> {
        if raw.header.dwType.0 == RIM_TYPEMOUSE.0 {
            let mouse = unsafe { raw.data.mouse };
            // 通过设备句柄过滤 S10（VID=0x05AC, PID=0x0220）
            // 解析 lLastX/lLastY 为滑动方向
            Some(PlatformEvent::MouseDelta {
                dx: mouse.lLastX,
                dy: mouse.lLastY,
            })
        } else {
            None
        }
    }
}
```

### macOS CGEventTap 拦截

```rust
// src/capture/macos.rs
use core_foundation::runloop::*;
use core_graphics::event::*;

pub fn create_tap() -> CFMachPort {
    let callback = |proxy: CGEventTapProxy, event_type: CGEventType, event: CGEvent| -> Option<CGEvent> {
        match event_type {
            CGEventType::MouseMoved => {
                // 过滤 S10 设备（通过 location 变化模式或 IOKit 属性）
                // 消费事件返回 None，转发返回 Some(event)
            }
            _ => {}
        }
        Some(event)
    };

    unsafe {
        CGEventTapCreate(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            CGEventMask::from(CGEventType::MouseMoved),
            callback as *const _,
            std::ptr::null_mut(),
        )
    }
}
```

---

## Wayland 注入难题与解法

`enigo` 在 Wayland 下**完全无效**（安全模型禁止任意应用模拟输入）。解法：

### 方案 A：调用外部 wtype（最简单，当前 Python 做法）
```rust
use std::process::Command;

fn wtype_text(text: &str) {
    Command::new("wtype")
        .arg(text)
        .env("WAYLAND_DISPLAY", "wayland-1")
        .spawn()
        .unwrap();
}
```

### 方案 B：实现 zwp_virtual_keyboard_v1 协议
```rust
// 需 wayland-client + zwp-virtual-keyboard 协议
// 优点：无外部依赖，原生 Wayland
// 缺点：Compositor 必须支持该协议（Hyprland ✓, Sway ✓, GNOME ✗）
```

### 方案 C：通过 uinput 绕过 Wayland
```rust
// 创建 evdev 虚拟设备，让内核转发给 Wayland compositor
// 与当前 Python 的 UInput 方案相同
// 问题：与之前一样，uinput 在某些系统上初始化异常
```

**推荐**：方案 A 为主，方案 B 为可选高级特性。

---

## 开发路线图

### Phase 1: Linux 功能对等（2-3 周）
- [ ] evdev 设备发现 + GRAB
- [ ] 手势检测状态机移植
- [ ] TOML 配置 + 热重载
- [ ] `command:` / `key:` / `text:` / `combo:` 动作执行
- [ ] wtype / hyprctl 集成
- [ ] systemd 集成
- [ ] 与 Python 版本行为完全一致

### Phase 2: Windows 支持（3-4 周）
- [ ] Raw Input 设备枚举
- [ ] 通过 VID/PID 过滤 S10
- [ ] 触摸板方向键解析（区分真实鼠标）
- [ ] `SendInput` 按键注入
- [ ] Win32 窗口管理器 API
- [ ] AutoHotkey 生成脚本（备选）

### Phase 3: macOS 支持（2-3 周）
- [ ] IOHIDManager 设备枚举
- [ ] CGEventTap 事件拦截
- [ ] Accessibility 权限申请
- [ ] `CGEventPost` 按键注入
- [ ] Karabiner JSON 生成（备选）

### Phase 4:  polish（1-2 周）
- [ ] GUI 配置工具（egui/iced）
- [ ] 蓝牙直连模式（btleplug 绕过系统 HID）
- [ ] 固件 OTA 分析（AE00/AE30/AE40 服务）

---

## 与 Python 方案共存策略

不需要完全替换 Python 项目。Rust 版本开发期间：

1. **保持 Python 项目为默认推荐**
2. **Rust 版本作为 `s10-ringctrl-rs` 新仓库**
3. **共享配置格式**：两份实现读取同一套 TOML
4. **逐步迁移**：Rust 版本成熟后，Python 版本进入维护模式

---

## 立即启动的最小 PoC

如果现在要开始，最小可运行原型只需 **1 个文件**：

```bash
cargo new s10-ringctrl-rs
cd s10-ringctrl-rs
cargo add evdev-rs clap serde toml tracing
```

实现 Linux -only 的读取 + 打印，验证架构可行：

```rust
// src/main.rs (Linux MVP)
use evdev_rs::Device;

fn main() {
    let mut dev = Device::open("/dev/input/event22").unwrap();
    dev.grab(evdev_rs::GrabMode::Grab).unwrap();
    loop {
        if let Ok(Some(ev)) = dev.next_event(evdev_rs::ReadFlag::NORMAL) {
            println!("{:?}", ev);
        }
    }
}
```

```bash
cargo run --release
```

---

要我帮你启动这个 Rust PoC 吗？或者先写 Windows/macOS 的 AutoHotkey/Karabiner 配置作为过渡方案？
