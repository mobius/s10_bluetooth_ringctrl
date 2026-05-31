# S10 蓝牙遥控器 Hyprland 重映射工具

将珠海杰理（JieLi）芯片方案的 S10 蓝牙遥控器/飞鼠，在 Hyprland/Omarchy 环境下重映射为窗口切换、工作区切换等操作。

---

## 硬件识别

| 属性 | 值 |
|------|-----|
| 设备名称 | S10 |
| MAC | `FA:1F:4C:4A:22:F8` |
| 芯片 | 珠海杰理 (zhuhai_jieli) |
| 伪装身份 | Apple Magic Trackpad (`0x05AC:0x0220`) |
| 输入设备 | `event22` (触摸模拟) + `event23` (消费者控制) |

**协议破解详情**：见 `S10_PROTOCOL_ANALYSIS.md`

---

## 实体按键与事件映射

S10 遥控器有 7 个实体按键，但**全部通过触摸模拟发送**，而非标准 HID 消费者控制键：

| 实体按键 | 事件来源 | 检测方式 |
|---------|---------|---------|
| **上** | `event22` | Y 坐标增加滑动 |
| **下** | `event22` | Y 坐标减少滑动 |
| **左** | `event22` | X 坐标增加滑动 |
| **右** | `event22` | X 坐标减少滑动 |
| **play** | `event22` | 单击（无位移） |
| **mode** | `event22` | 双击（两次短按） |
| **power** | `event23` | `KEY_VOLUMEUP` |

> 注意：方向键坐标系与标准触摸板相反（Y 增加 = 上，X 增加 = 左）。

---

## 安装依赖

```bash
sudo pacman -S python-evdev wtype
```

- `python-evdev`：读取 Linux 输入设备
- `wtype`：Wayland 下模拟按键输入（可选，仅 `key:` / `combo:` 映射需要）

---

## 使用方法

### 1. Debug 模式（只输出事件，不执行映射）

用于确认遥控器按键能否被正确识别：

```bash
cd /home/joey/Work/dev/temp/bluetooth
sudo python3 s10-remapper.py
```

按遥控器按键，终端会显示识别结果：

```
[DIR     ] >>> SWIPE DETECTED: UP    <<<
[TOUCH   ] >>> TAP DETECTED <<<
[TOUCH   ] >>> DOUBLE_TAP DETECTED <<<
[CONSUMER] PRESS   KEY_VOLUMEUP  -> passthrough
```

### 2. Remap 模式（执行实际映射）

```bash
sudo python3 s10-remapper.py --remap
```

### 3. 指定配置文件

```bash
sudo python3 s10-remapper.py --remap -c ./my-config.json
```

---

## 默认映射

编辑 `s10-remapper.json` 自定义：

```json
{
  "mappings_direction": {
    "UP":         "command:hyprctl dispatch cyclenext prev",
    "DOWN":       "command:hyprctl dispatch cyclenext",
    "LEFT":       "command:hyprctl dispatch workspace e-1",
    "RIGHT":      "command:hyprctl dispatch workspace e+1",
    "TAP":        "command:hyprctl dispatch exec 'playerctl play-pause'",
    "DOUBLE_TAP": "command:hyprctl dispatch togglespecialworkspace"
  },
  "mappings_consumer": {
    "KEY_VOLUMEUP": "command:hyprctl dispatch exec 'pamixer -i 5'"
  }
}
```

### 映射类型

| 前缀 | 格式 | 说明 | 示例 |
|------|------|------|------|
| `command:` | `command:shell_cmd` | 执行任意 shell 命令 | `command:hyprctl dispatch cyclenext` |
| `text:` | `text:字符串` | 通过 `wtype` 输入文字 | `text:继续` |
| `key:` | `key:KEY_NAME` | 通过 `wtype` 模拟单键 | `key:KEY_1` |
| `combo:` | `combo:KEY1+KEY2` | 通过 `wtype` 模拟组合键 | `combo:KEY_LEFTMETA+KEY_TAB` |
| `passthrough` | `passthrough` | 原样转发，不拦截 | — |

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

## 配置参数

```json
{
  "device_consumer": "/dev/input/event23",
  "device_touch": "/dev/input/event22",
  "swipe": {
    "threshold": 80,
    "double_tap_ms": 500
  },
  "grab": true,
  "verbose": true
}
```

| 参数 | 说明 |
|------|------|
| `threshold` | 滑动识别阈值（像素），低于此值视为单击 |
| `double_tap_ms` | 双击间隔阈值（毫秒） |
| `grab` | `true` 时拦截原始事件，不让系统处理 |
| `verbose` | 是否在终端打印事件日志 |

---

## 设为开机自启

```bash
sudo cp s10-remapper.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now s10-remapper

# 查看状态
sudo systemctl status s10-remapper

# 查看实时日志
sudo journalctl -u s10-remapper -f

# 重启服务（改完配置后）
sudo systemctl restart s10-remapper
```

---

## 常见问题

### Q: `hyprctl` 命令执行了但无效果
A: 脚本以 `sudo` 运行时丢失了 `HYPRLAND_INSTANCE_SIGNATURE` 环境变量。本脚本已自动从 `/run/user/1000/hypr/` 检测并注入该变量。如果仍有问题，检查 Hyprland 是否正常运行：

```bash
echo $HYPRLAND_INSTANCE_SIGNATURE
ls /run/user/$UID/hypr/
```

### Q: 终端里 `wtype` 输出字符正常，但切到其他窗口没反应
A: `wtype` 只能发送给**当前焦点窗口**。如需跨窗口执行操作，请使用 `command:hyprctl dispatch ...` 而非 `key:` 映射。

### Q: 只有部分按键能被识别
A: 运行 debug 模式确认每个实体按键对应的事件：

```bash
sudo python3 s10-remapper.py
```

不同批次硬件的坐标系或消费者控制键可能有差异，根据输出调整 `mappings_direction` 或 `mappings_consumer` 即可。

---

## 文件清单

| 文件 | 说明 |
|------|------|
| `s10-remapper.py` | 主程序 |
| `s10-remapper.json` | 用户配置文件 |
| `s10-remapper.service` | systemd 服务模板 |
| `S10_PROTOCOL_ANALYSIS.md` | 完整蓝牙协议破解报告 |
| `s10-debug.py` | 独立调试工具（可选） |

---

## License

MIT
