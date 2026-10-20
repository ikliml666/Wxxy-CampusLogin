---
title: USB 网卡禁用后重启弹出「正在安装设备驱动程序」——禁用必须用非持久化设备级原语
type: learning
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
tags: [根因分析, USB, RTL8156, PnP, NDIS, CM_Disable_DevNode, CD-ROM模式, 重启残留]
---

## 现象

USB 网卡（Realtek RTL8156，适配器名「以太网 2」）被禁用后重启电脑，Windows 在
资源管理器中多出一个「驱动安装」入口（带安装器的盘符），**必须点击安装完才能使用网卡**。

## 核心结论：三层禁用，副作用完全不同

| 层 | 操作方式 | 禁用期间驱动状态 | 重启后 |
| --- | --- | --- | --- |
| NDIS / 接口层 | `netsh interface set interface ... disable`、`Disable-NetAdapter` | **保持加载** | 禁用态跨重启残留，物理卡重启后停在 PnP problem 22（见下文实证） |
| PnP / 设备层（持久） | 设备管理器、`pnputil /disable-device`、`CM_Disable_DevNode(CM_DISABLE_PERSIST)` | **不加载** | 仍是 problem 22，且触发次生安装活动 |
| PnP / 设备层（**非持久**） | `CM_Disable_DevNode(devinst, 0)`（不传 PERSIST） | 不加载 | **自动恢复启用**，驱动正常加载 ← 本应用采用 |

### 2026-10-20 本机实测：netsh disable 同样会跨重启残留 problem 22

早期调研（本文章初稿）根据文档推断「netsh 只翻 NDIS 的 `AdminStatus`，PnP devnode 不进入
`CM_PROB_DISABLED(22)`」，**该推断被本机日志推翻**：

- 2026-10-19 23:00 夜切只用 netsh 禁用（`[helper] netsh 禁用适配器成功: 以太网`、
  `netsh 禁用适配器成功: 以太网 2`，全项目此时无任何 `pnputil /disable-device` 调用）。
- 2026-10-20 08:36 重启后应用启动对账：
  `[adapter] 适配器处于设备级禁用(PnP problem 22)，走 pnputil 启用: 以太网 (PCI\VEN_10EC&DEV_8168&...)`
  与 `... 走 pnputil 启用: 以太网 2 (USB\VID_0BDA&PID_8156\4013000001)` —— **两张卡双双 problem 22**。
- 逐日日志统计也一致：10-13~10-17 禁用 2 次、重启启用 0 次（期间未重启）；
  10-18 重启后启用 3 次；10-19 启用 2 次；10-20 启用 2 次。

推论：USB 网卡被 netsh 禁用时，NDIS miniport 停机连带其 PnP devnode 也进入禁用状态并
**写入持久配置标志**（`CM_DISABLE_PERSIST` 语义等价的 ConfigFlags `DISABLED=0x1`），
重启后设备以禁用态重新枚举、驱动堆栈不加载。这与「PnP 层持久禁用」在重启后的效果相同，
区别只在于当前会话里 netsh 是软状态 vs `pnputil` 是硬状态。

已排除的替代解释：快速启动已关闭（`HiberbootEnabled=0`、无 `hiberfil.sys`、
`powercfg /a` 只支持 S0），不是休眠持久化；`setupapi.dev.log` 无该设备安装记录，
Kernel-PnP/Configuration 无该设备安装事件，所以不是设备被重新安装。

## 官方文档证据（learn.microsoft.com）

1. **`CM_Disable_DevNode`**：「By default, CM_Disable_DevNode disables a device at one
   time, but after reboot the device is enabled again. Starting in Windows 10, you can
   specify the `CM_DISABLE_PERSIST` flag to disable the device across reboots.」——默认
   非持久，持久需显式标志。
2. **`SP_PROPCHANGE_PARAMS`**：`DICS_DISABLE` = 「Windows disables the device」，
   **不卸载驱动堆栈**；只有 `DICS_STOP` 才「The driver stack will be unloaded and the
   CSCONFIGFLAG_DO_NOT_START flag will be set」；`DICS_PROPCHANGE` 是「stops and restarts
   the device」；`DICS_FLAG_CONFIGSPECIFIC` 只写注册表配置标志不改当前状态，
   `DICS_FLAG_GLOBAL` 才真正禁用。
3. **Finish-Install Action**（`how-finish-install-actions-are-processed`）：「In Windows 8,
   Windows 8.1, and Windows 10, finish-install actions must be completed in the Action
   Center by an administrator... Users must click on 'Finish installing device software'.」
   ——installer 返回 `DIF_NEWDEVICEWIZARD_FINISHINSTALL` 并设
   `DI_FLAGSEX_FINISHINSTALL_ACTION` 后，设备被标记但任务**排队不自动运行**。
4. **`CONFIGFLAG_REINSTALL`**：OR 进 `ConfigFlags` 经
   `SetupDiSetDeviceRegistryProperty(SPDRD_CONFIGFLAGS)` 写回后，「This causes the device
   to be reinstalled the next time that the device is reenumerated.」
5. **`DEVPKEY_Device_ConfigFlags`**：`DEVPROP_TYPE_INT32`，可读写，官方标注「for internal
   use only」。位值取自 SDK `regstr.h`：`DISABLED=0x1`、`REMOVED=0x2`、
   `MANUAL_INSTALL=0x4`、`FAILEDINSTALL=0x10`、`FINISH_INSTALL=0x100`、`REINSTALL=0x800`。
6. **`pnputil /disable-device`**：Win10 2004 起可用，官方文档**无任何持久化说明**，
   参数仅 `[/reboot] [/force] [/class] [/bus] [/deviceid]`。
7. **`CM_Reenumerate_DevNode`**：`CM_REENUMERATE_RETRY_INSTALLATION` 官方警告「can cause
   the PnP manager to prompt the user to perform installation」，需 `SeLoadDriverPrivilege`。

以下两项**查无官方出处，勿依赖**：`*IfAdminStatus` NDIS 关键字（Standardized INF
Keywords 与 Keywords-not-displayed-in-UI 两页均无）、`PnPCapabilities` 逐位语义
（仅社区共识，`0x18=24` 禁电源管理）。

## 机制定位：RTL8156 的 CD-ROM 模式（本机取证）

重启后「资源管理器多出一个带安装器的盘符、点击后才可用」，对应的是 USB 复合设备的
**CD-ROM 模式**而非 Finish-Install Action。同一序列号 `4013000001` 下 PnP 树里存在
三个同源设备（`Get-PnpDevice` 实测）：

| 设备实例 ID | 状态 | 说明 |
| --- | --- | --- |
| `USB\VID_0BDA&PID_8156\4013000001` | OK / Class=Net | 现在工作的网卡 |
| `USB\VID_0BDA&PID_8151\4013000001` | Unknown(幽灵) / Class=USB | 通用 USB 大容量存储设备（`Service=USBSTOR`） |
| `USBSTOR\CDROM&VEN_REALTEK&PROD_USB_CD-ROM&REV_2.00\4013000001&0` | 幽灵 / Class=CDROM | Realtek USB CD-ROM |

时间线（今早）：`08:35:36` 该 USB 设备以 CD-ROM/大容量存储模式被枚举（`LastArrival`），
`08:36:05` 消失（`LastRemoval`），`08:36:20` 才以 `PID_8156` Net 模式重新枚举。
即 RTL8156 上电默认 CD-ROM 模式，靠驱动发 USB 控制命令切到 NIC 模式；
**禁用期间驱动不加载 → 模式切换不发生 → 停在 CD-ROM 模式**，Windows 把它当新的
大容量存储设备，于是出现安装盘符。

官方 Finish-Install 机制（`DIF_NEWDEVICEWIZARD_FINISHINSTALL` →
`DI_FLAGSEX_FINISHINSTALL_ACTION` → Action Center 排队等用户点击）确实是真实的
「点击后才可用」机制，但本机 `ConfigFlags=0`（`0x100 FINISH_INSTALL` / `0x800 REINSTALL`
均未命中），故不是本现象的主因。

## 本应用修复：禁用改走非持久化设备级禁用

2026-10-20 修复（分支 `fix/usb-adapter-night-disable-nonpersistent`）：

- **新实现**：`tauri-app/src-tauri/src/network/discovery/devnode.rs`
  `disable_device(instance_id)` 调 `CM_Disable_DevNode(devinst, 0)`（**不传**
  `CM_DISABLE_PERSIST`）→ 轮询复核 problem 22 出现（3s / 200ms，与 enable 侧对称）。
  `disable_adapter_nonpersistent(adapter_name)` 按名字查 GUID → 读 PnPInstanceId →
  设备级禁用，返回 `Ok(Some(instance_id))` / `Ok(None)`（无 PnP 实例 ID 的虚拟卡）。
- **helper 分流**：`tauri-app/src-tauri/src/helper/mod.rs` `run_disable_adapter` 先试
  设备级禁用；`Ok(None)`（Wintun/TAP 虚拟卡，不涉及驱动安装）才回退 netsh；
  物理卡设备级禁用失败时**直接报错**，不静默回退 netsh——那会重新引入跨重启残留。
- 启用侧不变（`devnode.rs` `enable_device` → `pnputil /enable-device` + 复核），
  与禁用侧构成对称的非持久化设备级开关。

为什么不是「继续用 netsh 并加启动自愈」：netsh 的残留让设备在无人值守的重启窗口里
停在 problem 22 / CD-ROM 模式，USB 卡此时是「需要用户点击」的僵态，自愈只能事后补救
且必然有一段无网可用期。非持久化禁用从源头保证重启后设备是启用态。

## 未决点

- `pnputil /disable-device` 内部是否使用 `CM_DISABLE_PERSIST`（官方文档未说明）。
- `CM_Disable_DevNode(devinst, 0)` 在 USB 网卡上重启后是否 100% 自动恢复——本次改动
  已按官方语义实现，需实测下一次关机重启（重启后应看到 `ConfigFlags=0` 且
  problem 不等于 22，应用日志里不再出现「走 pnputil 启用」）。

## 教训

- Windows 的「禁用」横跨 NDIS / PnP / 组策略多层，**层选错则副作用完全不同**。
- **不要根据文档推断跨重启行为，要看日志**：本文章初稿据 `DICS_DISABLE` 文档断言
  「netsh 不进 problem 22」，而本机日志显示 netsh 禁用同样跨重启残留 problem 22。
  判定真实行为只需一条 grep：重启后应用日志里有没有「走 pnputil 启用」。
- 面对「重启后要点击安装」这类现象，先查该设备 `ConfigFlags` 的三个安装标志位
  （`0x100`/`0x800`/`0x10`）与 PnP problem 码，再查同一序列号下有没有 USBSTOR/CDROM
  幽灵兄弟设备（后者是 CD-ROM 模式的直接物证）。
- 需要「运行时下线、重启后恢复」时，`CM_Disable_DevNode` 不传 `CM_DISABLE_PERSIST`
  是唯一同时满足「设备级确实不加载」与「重启自动恢复」的原语。

## Connections

[[usb-adapter-enable-pnp-vs-ndis-layers]]（启用方向的姊妹篇：同一两层模型的另一面）、
[[adapter-disabled-classification-blindspot]]、[[adapter-visibility-cache-staleness]]
