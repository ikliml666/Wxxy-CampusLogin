---
title: USB 网卡禁用后重启弹出「正在安装设备驱动程序」——NDIS 层禁用 vs PnP 层禁用
type: learning
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
tags: [调研, USB, 禁用, PnP, NDIS, finish-install, 根因分析]
---

## 现象

USB 网卡（Realtek RTL8156，适配器名「以太网 2」）被禁用后重启电脑，Windows 在
资源管理器中多出一个「驱动安装」入口（带安装器的盘符），**必须点击安装完才能使用网卡**。

## 核心结论：先分清两层，只有一层会引发重启后的安装活动

| 层 | 操作方式 | 禁用期间驱动状态 | USB 设备身份 | 重启后是否会触发安装 |
| --- | --- | --- | --- | --- |
| NDIS / 接口层 | `netsh interface set interface ... disable`、`Disable-NetAdapter` | **保持加载** | 不变 | **不会** |
| PnP / 设备层 | 设备管理器、`pnputil /disable-device`、`CM_Disable_DevNode` | **不加载**（驱动堆栈整体不启动） | 设备以受限态重新枚举 | **会**（见下两节机制） |

`netsh disable` 只翻 NDIS 的 `AdminStatus`，miniport 停掉但驱动仍在，PnP devnode 不进入
`CM_PROB_DISABLED(22)`。而 PnP 层禁用期间设备驱动堆栈根本不加载——这是所有次生
安装活动的共同前提。

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

## 两个候选机制（按可信度）

**e. Finish-Install Action（最可能）**。官方机制存在，与「出现安装入口、点击后才可用」
高度吻合。PnP 禁用期间驱动未加载 → 重启后设备重新枚举 → Realtek 网卡驱动的
co-installer 重新要求 finish-install → 任务进 Action Center 排队，等用户点击。

**a. USB 复合设备的 CD-ROM 模式（次高）**。RTL8153/8156 上电默认以 CD-ROM 模式枚举
（内置驱动盘），靠驱动发 USB 控制命令切到 NIC 模式；三方来源（Hackaday 2023-11-02、
Arch BBS `viewtopic.php?id=228195`、HP 支持文档 h30434）证实该硬件行为。推理链：
PnP 禁用期间驱动不加载 → 模式切换不发生 → 重启后停在 CD-ROM 模式 → Windows 把它
当新的 USBSTOR 设备自动装 → 「此电脑」出现带安装器的盘符。
本机只读取证：`USB\VID_0BDA&PID_8156\4013000001` 在 PnP 树里**当前没有 CD-ROM 复合
子设备**（`Get-PnpDevice` 该实例 ID 下只有 Net 一个节点），即此刻未暴露虚拟光驱接口。

低可信被排除项：DeviceSetupManager 气泡（点击非必需）、未知设备重配对（与 Code 22
语义不符）、pnputil 卸载驱动致重装（与 `DICS_DISABLE` 不卸载驱动的官方事实矛盾）。

## 本应用现状：禁用路径已经是安全的那一层

- **禁用只走 NDIS 层**：`tauri-app/src-tauri/src/helper/mod.rs:744-791` 的
  `run_disable_adapter` 只执行 `netsh interface set interface <name> disable`。
  全项目 grep 确认**没有任何 `pnputil /disable-device` 调用**；`disable_adapter` 的
  调用点仅 `tauri-app/src-tauri/src/monitor/outbound_night.rs:531`、`:669`、`:737`。
- **pnputil 只用于启用**：`devnode.rs:123-169` `enable_device` 跑
  `pnputil /enable-device`，由 `tauri-app/src-tauri/src/network/adapter_cache.rs:104-160`
  的 `enable_adapter` 在 `find_disabled_device` 命中 problem 22 时分流进入。
- 夜间切换先写跃点（`outbound_night.rs:304-343` `write_outbound_metric`，
  `SetIpInterfaceEntry`）再禁用（`:531`），同样不碰设备状态。
- `tauri-app/src-tauri/src/monitor/outbound_switch.rs:278-280` `unsafe_to_disable(guid)`
  只判「PnPInstanceId 读不到」，注释（:273-277）明确 USB 总线网卡不再豁免，
  依据是 2026-10-01 夜切日志实证运行期 netsh disable/enable 对称。

**因此本应用的禁用不会把设备推进 problem 22**，理论上不会触发重启后的次生安装活动。
若用户仍观察到该提示，指向两个方向而非本应用禁用逻辑：① 曾用设备管理器 /
`pnputil /disable-device` 手工禁用过（或旧版本残留 `ConfigFlags` 携带
`FINISH_INSTALL(0x100)` / `REINSTALL(0x800)`）；② 该 USB 卡自身的 CD-ROM 模式切换
在驱动未加载期间未发生——这是硬件+驱动行为，应用层的正确应对是**禁在 NDIS 层**，
而非额外打补丁。

## 若确需自愈（可选，未实现）

1. 启动扫描目标适配器的 `CM_Get_DevNode_Status` 是否 problem 22 → 复用
   `adapter_cache::enable_adapter` 的设备级启用分流清除。
2. 读 `DEVPKEY_Device_ConfigFlags`，命中 `0x100`/`0x800` 时记日志（不要自行清位——
   官方标注该属性 internal use only，写错会让设备下次枚举被强制重装）。
3. 检测 `USB\VID_...` 下是否出现 CD-ROM/USBSTOR 复合子设备，作为该现象的观测指标。
4. **绝不要**为了「彻底禁用」改调 `pnputil /disable-device`——那是把问题从
   「禁用不干净」换成「重启弹安装」。

## 未决点（本机无管理员权限，未能实测）

- `netsh interface set interface ... disable` 是否会在某些驱动上连带置 PnP problem 22
  （实测被拒：`The requested operation requires elevation (Run as administrator).`）。
- `pnputil /disable-device` 内部是否使用 `CM_DISABLE_PERSIST`（官方文档未说明）。
- `%SystemRoot%\INF\setupapi.dev.log` 现有记录与本现象无关（GameViewerIddDriver 被
  Device Installation Restrictions Policy 拦下 `Error = 0xE0000248`；`rtump64.INF`
  已导入为 `oem61.inf`），未见 USBSTOR/CD-ROM 或 finish-install 段落。

## 教训

- Windows 的「禁用」横跨 NDIS / PnP / 组策略多层，**层选错则副作用完全不同**：
  同样的「禁用」在 NDIS 层是运行期状态，在 PnP 层是设备生命周期事件。
- 需要「安静地让一张网卡下线」时，NDIS 层是唯一无次生副作用的选择；PnP 层禁用会
  让设备在重启后以「需要安装」的姿态回来。
- 面对「重启后要点击安装」这类现象，先查该设备 `ConfigFlags` 的三个安装标志位
  （`0x100`/`0x800`/`0x10`）与 PnP problem 码，比猜驱动更省时间。

## Connections

[[usb-adapter-enable-pnp-vs-ndis-layers]]（启用方向的姊妹篇：同一两层模型的另一面）、
[[adapter-disabled-classification-blindspot]]、[[adapter-visibility-cache-staleness]]
