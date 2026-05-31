# S10 RingCtrl — 蓝牙遥控器重映射器

将珠海杰理（JieLi）芯片方案的 S10 蓝牙遥控器/飞鼠，重映射为窗口切换、工作区切换、命令执行等操作。

> **Rust 重写已完成** 🦀 — 单二进制、零依赖运行时、微秒级响应。
> Python 原型保留在 `main` 分支作为参考。

---

## 硬件识别

| 属性 | 值 |
|------|-----|
| 设备名称 | S10 |
| MAC | `FA:1F:4C:4A:22:F8` |
| 芯片 | 珠海杰理 (zhuhai_jieli) |
| 伪装身份 | Apple Magic Trackpad (`0x05AC:0x0220`) |
| 输入设备 | `event22` (触摸模拟) + `event23` (消费者控制) |

**协议破解详情**：见 [`S10_PROTOCOL_ANALYSIS.md`](S10_PROTOCOL_ANALYSIS.md)

---

## 实体按键与事件映射

S10 全部通过触摸模拟发送事件：

| 实体按键 | 事件来源 | 检测方式 | 默认动作 |
|---------|---------|---------|---------|
| **上** | `event22` | Y 增加滑动 | 上一个窗口 (`cyclenext prev`) |
| **下** | `event22` | Y 减少滑动 | 下一个窗口 (`cyclenext`) |
| **左** | `event22` | X 增加滑动 | 上一个工作区 (`workspace e-1`) |
| **右** | `event22` | X 减少滑动 | 下一个工作区 (`workspace e+1`) |
| **play** | `event22` | 单击（无位移） | 输入文字 `继续` |
| **mode** | `event22` | 双击 | toggle special workspace |
| **power** | `event23` | `KEY_VOLUMEUP` | 无（可自定义） |

> 注意：方向键坐标系与标准触摸板相反（Y 增加 = 上，X 增加 = 左）。

---

## 快速开始（Rust）

### 编译

```bash
cd s10-ringctrl
cargo build --release
# 输出: target/release/s10_ringctrl
```

依赖：`cargo` + `rustc`（通过 `rustup` 安装即可）。运行时不需要 Python。

### 1. 生成默认配置

```bash
sudo ./target/release/s10_ringctrl --generate-config
# 生成 s10-ringctrl.toml
```

### 2. Debug 模式（只输出事件，不执行映射）

```bash
sudo ./target/release/s10_ringctrl
```

按遥控器按键，终端显示识别结果：

```
[DEBUG] >>> UP DETECTED <<<
[DEBUG] >>> TAP DETECTED <<<
[DEBUG] >>> DOUBLE_TAP DETECTED <<<
[DEBUG] [CONSUMER] PRESS KEY_VOLUMEUP
```

### 3. Remap 模式（执行实际映射）

```bash
sudo -E HYPRLAND_INSTANCE_SIGNATURE=$HYPRLAND_INSTANCE_SIGNATURE \
    ./target/release/s10_ringctrl --remap
```

### 4. 指定设备路径或配置文件

```bash
sudo ./target/release/s10_ringctrl --remap \
    -c ./my-config.toml \
    -t /dev/input/event24 \
    -e /dev/input/event25
```

### 5. 安静模式（仅错误输出）

```bash
sudo ./target/release/s10_ringctrl --remap -q
```

---

## 配置说明

编辑 `s10-ringctrl.toml`：

```toml
[device]
touch = "/dev/input/event22"
consumer = "/dev/input/event23"

[gesture]
threshold = 80          # 滑动识别阈值（像素）
double_tap_ms = 500     # 双击间隔阈值（毫秒）

[mappings]
UP         = "command:hyprctl dispatch cyclenext prev"
DOWN       = "command:hyprctl dispatch cyclenext"
LEFT       = "command:hyprctl dispatch workspace e-1"
RIGHT      = "command:hyprctl dispatch workspace e+1"
TAP        = "text:继续"
DOUBLE_TAP = "command:hyprctl dispatch togglespecialworkspace"
KEY_VOLUMEUP = "command:hyprctl dispatch exec 'pamixer -i 5 && notify-send Volume +'"
```

### 映射类型

| 前缀 | 格式 | 说明 | 示例 |
|------|------|------|------|
| `command:` | `command:shell_cmd` | 执行任意 shell 命令 | `command:hyprctl dispatch cyclenext` |
| `text:` | `text:字符串` | 通过 `wtype` 输入文字 | `text:继续` |
| `key:` | `key:KEY_NAME` | 通过 `wtype` 模拟单键 | `key:KEY_1` |
| `combo:` | `combo:KEY1+KEY2` | 通过 `wtype` 模拟组合键 | `combo:KEY_LEFTMETA+KEY_TAB` |

### 常用 Hyprland 命令参考

```bash
# 窗口切换
hyprctl dispatch cyclenext          # 下一个窗口
hyprctl dispatch cyclenext prev     # 上一个窗口
hyprctl dispatch killactive         # 关闭当前窗口
hyprctl dispatch fullscreen 1       # 全屏
hyprctl dispatch togglefloating     # 浮动

# 工作区切换
hyprctl dispatch workspace N        # 切换到工作区 N
hyprctl dispatch workspace e+1      # 下一个工作区
hyprctl dispatch workspace e-1      # 上一个工作区
hyprctl dispatch workspace previous # 上一个访问的工作区
hyprctl dispatch togglespecialworkspace

# 启动程序
hyprctl dispatch exec 'kitty'
hyprctl dispatch exec 'fuzzel'
hyprctl dispatch exec 'loginctl lock-session'
```

---

## herdr 终端管理器集成

如果你用 [herdr](https://herdr.dev) 管理 Alacritty 会话，可以把字符直接发到指定 pane/agent，**无需窗口焦点切换**：

### 1. 查看你的 agent/pane

```bash
herdr pane list
herdr agent list
```

### 2. 编辑配置切换为 herdr 模式

打开 `s10-ringctrl.toml`，注释掉 Hyprland 映射，启用 herdr 映射：

```toml
[mappings]
# 通过 agent 名称发送（推荐，agent 名称稳定）
TAP        = "command:herdr agent send grok '继续'"
DOUBLE_TAP = "command:herdr agent send grok 'make test'"
UP         = "command:herdr agent send grok 'git push'"
DOWN       = "command:herdr agent send grok 'git pull'"
LEFT       = "command:herdr agent send kimi 'npm run dev'"
RIGHT      = "command:herdr agent send kimi 'cargo build'"

# 通过 pane_id 发送（pane_id 重启会变，不推荐硬编码）
# LEFT = "command:herdr pane send-text w651fddbbf986d1-1 'npm run dev'"

# 发送组合键（如 Ctrl+C）
# RIGHT = "command:herdr pane send-keys w651fddbbf986d1-1 Control_L c"
```

### 3. 使用 wrapper 脚本启动（保留 herdr socket 环境变量）

```bash
cd s10-ringctrl
./run.sh --remap
```

`run.sh` 会自动保留 `XDG_RUNTIME_DIR`（herdr socket 路径）和 `HYPRLAND_INSTANCE_SIGNATURE`。

> ⚠️ `sudo` 默认会丢弃用户环境变量，导致 `herdr` 命令找不到 server。必须用 `sudo -E` 或 wrapper 脚本。

---

## 设为开机自启

```bash
# 1. 复制二进制到系统路径
sudo cp target/release/s10_ringctrl /usr/local/bin/

# 2. 复制配置到 /etc
sudo cp s10-ringctrl.toml /etc/

# 3. 复制并编辑 systemd 服务
sudo cp s10-ringctrl.service /etc/systemd/system/
# 编辑服务文件，确保 ExecStart 指向 /usr/local/bin/s10_ringctrl --remap -c /etc/s10-ringctrl.toml

sudo systemctl daemon-reload
sudo systemctl enable --now s10-ringctrl

# 查看状态
sudo systemctl status s10-ringctrl
sudo journalctl -u s10-ringctrl -f
```

---

## 常见问题

### Q: `hyprctl` 命令执行了但无效果
A: 脚本以 `sudo` 运行时丢失了 `HYPRLAND_INSTANCE_SIGNATURE` 环境变量。程序已自动从 `/run/user/1000/hypr/` 检测并注入该变量。如果仍有问题，检查：

```bash
echo $HYPRLAND_INSTANCE_SIGNATURE
ls /run/user/$UID/hypr/
```

### Q: 终端里 `wtype` 输出字符正常，但切到其他窗口没反应
A: `wtype` 只能发送给**当前焦点窗口**。如需跨窗口执行操作，请使用 `command:hyprctl dispatch ...` 而非 `key:` 映射。

### Q: 提示 "Permission denied" 或 grab 失败
A: 需要 root 或 `input` 组权限：

```bash
sudo usermod -aG input $USER
# 重新登录后生效
```

### Q: 设备路径变了（蓝牙重连后）
A: 通过 `by-id` 查找稳定路径，或每次启动前检查：

```bash
ls -la /dev/input/by-id/ | grep -i apple
evtest  # 按编号试，确认哪个有 S10 事件
```

---

## 项目结构

```
s10-ringctrl/
├── Cargo.toml              # Rust 包配置
├── s10-ringctrl.toml       # 默认配置文件
└── src/
    ├── main.rs             # 入口：CLI + 双线程事件循环
    ├── gesture.rs          # 手势检测（滑动/单击/双击）
    ├── config.rs           # TOML 配置加载/序列化
    └── action.rs           # 动作解析与执行（command/key/text/combo）
```

---

## 跨平台状态

| 平台 | 状态 | 说明 |
|------|------|------|
| **Linux (Wayland/Hyprland)** | ✅ 可用 | 当前主目标，使用 `evdev` + `wtype` |
| **Linux (X11)** | 🔄 待适配 | 需 `xdotool` 替代 `wtype` |
| **Windows** | 📋 计划中 | 需 Raw Input + SendInput |
| **macOS** | 📋 计划中 | 需 IOKit + CGEventPost |

详见 [`RUST_REWRITE_PLAN.md`](RUST_REWRITE_PLAN.md)

---

## License

MIT
