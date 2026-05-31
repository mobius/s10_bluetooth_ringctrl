# S10 蓝牙设备协议破解报告

## 1. 设备身份识别

| 属性 | 值 |
|------|-----|
| 设备名称 | S10 |
| MAC 地址 | FA:1F:4C:4A:22:F8 |
| 制造商 (GATT) | `zhuhai_jieli` (珠海杰理科技) |
| 型号 | `hid_mouse` |
| 硬件版本 | 0.0.1 |
| 固件版本 | 0.0.1 |
| 软件版本 | 0.0.1 |
| 电池电量 | 89%-96% (动态) |
| 芯片方案 | 杰理蓝牙芯片 (ManufacturerData: `JLAISDK`) |

### 冒充身份
设备伪造 **Apple Magic Trackpad** 身份以欺骗操作系统：
- **Vendor ID**: `0x05AC` (Apple Inc.)
- **Product ID**: `0x0220` (Apple Magic Trackpad)
- **Product Version**: `0x0110`
- **Appearance**: `0x03C1` (Keyboard icon - 但功能为触摸板)

## 2. 服务结构 (GATT)

### 2.1 标准服务

| Handle | UUID | 名称 | 说明 |
|--------|------|------|------|
| 0x0001 | 0x1800 | Generic Access Profile | 设备名称: "S10" |
| 0x0008 | 0x1801 | Generic Attribute Profile | Service Changed |
| 0x000C | 0x180A | Device Information | 制造商/型号/版本信息 |
| 0x001F | 0x180F | Battery Service | 电池电量通知 |
| 0x0023 | 0x1812 | Human Interface Device | **HID 触摸板服务** |

### 2.2 厂商自定义服务

| Handle | UUID | 特征 | 属性 | 说明 |
|--------|------|------|------|------|
| 0x0080 | **0xAE00** | AE01 | Write-Without-Response | 命令通道 #1 |
| 0x0080 | 0xAE00 | AE02 | **Notify** | 状态/数据通道 #1 |
| 0x0050 | **0xAE30** | AE01 | Write-Without-Response | 命令通道 #2 |
| 0x0050 | 0xAE30 | AE02 | **Notify** | 状态/数据通道 #2 |
| 0x0050 | 0xAE30 | AE03 | Write-Without-Response | 命令通道 #3 |
| 0x0050 | 0xAE30 | AE04 | **Notify** | 状态/数据通道 #3 |
| 0x0050 | 0xAE30 | AE05 | **Notify** | 状态/数据通道 #4 |
| 0x0050 | 0xAE30 | AE10 | Read + Write | 配置/控制寄存器 |
| 0x004A | **0xAE40** | AE41 | Write-Without-Response | 命令通道 #4 |
| 0x004A | 0xAE40 | AE42 | **Notify** | 状态/数据通道 #5 |

**关键发现**: AE10 当前值为 `00 00 00 00`，尝试写入 `00` 返回 ATT Error `0x0E` (Unlikely Error)，随后设备断开。说明该寄存器有严格的校验机制。

## 3. HID 协议解析

### 3.1 HID 信息
- **HID 版本**: 1.11 (0x0111)
- **Country Code**: 0
- **Flags**: RemoteWake + NormallyConnectable
- **Protocol Mode**: Report Mode (非 Boot Mode)

### 3.2 Report Map (141 bytes) 解析

设备声明了 **2 个 Report ID**，但实际注册了 **7 个 Report 特征**（6 Input + 1 Output）。

#### Report ID 1: 单点触摸数据
```
Usage Page: Digitizer
Usage: Touch Screen
  Report ID: 1
  Collection: Logical (Finger)
    Tip Switch     [1 bit]  (0/1) - 触摸按下/抬起
    In Range       [1 bit]  (0/1) - 手指是否在感应区
    Confidence     [1 bit]  (0/1) - 数据可信度
    Contact ID     [5 bits] (0-5) - 触点编号
    X              [12 bits](0-1000) - X 坐标
    Y              [12 bits](0-1000) - Y 坐标
```
- **总长度**: 4 bytes / 报告
- **坐标系**: 绝对坐标，1000×1000 分辨率
- **Unit**: English Linear (inch), Exponent -2

#### Report ID 2: 消费者控制 (多媒体按键)
```
Usage Page: Consumer
Usage: Consumer Control
  Report ID: 2
  16 x 1-bit 开关:
    - Power (0x30)
    - Menu (0x40)
    - Menu Pick (0x82)
    - Menu Up (0xA0)
    - Scan Next Track (0xB5)
    - Scan Previous Track (0xB6)
    - Volume Increment (0xE9)
    - Volume Decrement (0xEA)
    - Mute (0xE2)
    - Play/Pause (0xCD)
    - AL Consumer Control Config (0x183)
    - AL Keyboard Layout (0x196)
    - AC Home (0x223)
    - AC Back (0x224)
    - AC Soft Left (0x22D)
    - AC Soft Right (0x22E)
```
- **总长度**: 2 bytes / 报告

### 3.3 Report Reference 映射

| Report ID | 类型 | 对应 GATT 特征 | 用途 |
|-----------|------|----------------|------|
| 1 | Input | char0026 | 触摸数据 |
| 2 | Input | char002a | 消费者控制 |
| 1 | Output | char002e | 触摸反馈/LED? |
| 3 | Input | char0031 | **未在 Report Map 定义** |
| 4 | Input | char0035 | **未在 Report Map 定义** |
| 5 | Input | char0039 | **未在 Report Map 定义** |
| 6 | Input | char003d | **未在 Report Map 定义** |

**异常**: Report ID 3-6 未在 Report Map 中声明，说明设备可能：
1. 使用简化的 Report Map 欺骗操作系统，实际通过非标准 HID 报告传输扩展数据
2. 报告描述符不完整，导致 Linux 仅识别为单点触控

## 4. 实际输入事件分析

通过 `/dev/input/event22` 捕获的触摸事件：

```
ABS_MT_TRACKING_ID  = 14      (手指按下)
ABS_MT_POSITION_X   = 700     (X 坐标)
ABS_MT_POSITION_Y   = 500     (Y 坐标)
BTN_TOUCH           = 1       (触摸按下)
ABS_X               = 700
ABS_Y               = 500
SYNC
...
ABS_MT_TRACKING_ID  = -1      (手指抬起)
BTN_TOUCH           = 0
```

Linux 输入子系统将蓝牙 HID 报告翻译为 **MT (Multi-Touch) 协议**，但设备实际只发送单点数据。

## 5. 自定义服务协议分析

### 5.1 协议特征
- 所有命令特征 (AE01, AE03, AE41) 使用 **Write-Without-Response**
  - 优点: 低延迟，无需等待 ATT Write Response
  - 缺点: 无写确认机制
- 所有数据特征 (AE02, AE04, AE05, AE42) 使用 **Notify**
  - 需要客户端订阅 CCCD (0x2902) 才能接收数据
- AE10 是唯一的 **Read + Write** 特征，可能是设备模式寄存器

### 5.2 AE10 寄存器分析
- **当前值**: `00 00 00 00`
- **写入 `00`**: 返回 ATT Error `0x0E` (Unlikely Error)
- **推测用途**: 
  - 工作模式切换 (触摸板模式 / 鼠标模式 / 键盘模式)
  - DPI/灵敏度设置
  - 休眠超时配置
  - 固件更新标志位

### 5.3 可能的 OTA/固件升级协议
杰理芯片通常使用类似以下协议的 OTA：
1. **AE30/AE01** (命令) + **AE30/AE02** (数据) 用于固件分包传输
2. **AE30/AE10** 用于触发升级模式/验证
3. **AE40** 服务可能用于备份区或二级引导加载程序通信

### 5.4 已观察到的自定义通知
- `AE02 (AE00 service)`: `FF` — 可能为心跳包或状态同步

## 6. 安全与逆向建议

### 6.1 已识别的安全特征
- 设备使用 **LE Secure Connections** (LegacyPairing: no)
- 已绑定 (Bonded: yes)，使用 LTK (Long Term Key) 加密
- 有 IRK (Identity Resolving Key) 用于隐私保护

### 6.2 进一步破解方向
1. **HID 报告逆向**: 通过 `btmon` + `uhid` 在 HCI 层捕获原始 HID 报告，对比 Linux 输入事件还原 Report ID 3-6 的格式
2. **自定义协议 fuzzing**: 对 AE10 进行结构化 fuzzing，测试各字节位功能
3. **固件提取**: 通过杰理芯片公开的烧录工具/协议，尝试读取 Flash 固件
4. **APP 逆向**: 查找配套手机 APP，逆向其蓝牙通信逻辑
5. **OTA 协议重放**: 如果存在官方升级包，可分析其分包结构和校验算法

## 7. 协议总结图

```
┌─────────────────────────────────────────────────────────────┐
│                        S10 设备                              │
│                   (珠海杰理 / 仿 Apple)                      │
├─────────────────────────────────────────────────────────────┤
│  GAP/GATT Layer                                             │
│  ├── 0x1800 设备信息 (名称/外观)                            │
│  ├── 0x180F 电池服务                                        │
│  ├── 0x180A 设备信息 (厂商/版本)                            │
│  ├── 0x1812 HID 服务                                        │
│  │     ├── Report ID 1: 单点触摸 (4 bytes)                 │
│  │     ├── Report ID 2: 消费者控制 (2 bytes)               │
│  │     └── Report ID 3-6: [隐藏/未声明]                     │
│  ├── 0xAE00 厂商服务 #1                                     │
│  │     ├── AE01: 命令写入 (write-without-response)         │
│  │     └── AE02: 状态通知 (notify)                         │
│  ├── 0xAE30 厂商服务 #2                                     │
│  │     ├── AE01/AE03: 命令写入                             │
│  │     ├── AE02/AE04/AE05: 数据通知                        │
│  │     └── AE10: 配置寄存器 (read/write)                   │
│  └── 0xAE40 厂商服务 #3                                     │
│        ├── AE41: 命令写入                                  │
│        └── AE42: 数据通知                                  │
├─────────────────────────────────────────────────────────────┤
│  HID Transport (L2CAP ATT Channel)                          │
│  └── BlueZ uhid → /dev/input/event22 (触摸板)              │
│                  → /dev/input/event23 (多媒体键)           │
└─────────────────────────────────────────────────────────────┘
```
