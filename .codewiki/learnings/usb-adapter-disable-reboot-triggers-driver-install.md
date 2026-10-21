---
title: learnings/usb-adapter-disable-reboot-triggers-driver-install.md
type: learning
source_files:
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
tags: [根因分析, USB, RTL8156, PnP, NDIS, CM_Disable_DevNode, CD-ROM模式, 重启残留, 缓存一致性]
---

## 现象

USB 网卡（Realtek RTL8156，适配器名「以太网 2」）被禁用后重启电脑，Windows 在资源管理器中多出一个「驱动安装」入口（带安装器的盘符），**必须点击安装完才能使用网卡**。

该现象源于禁用操作选择了错误的层级或持久化标志，导致设备在重启后停留在 CD-ROM 模式或 Problem 22 状态，触发了 Windows 的 Finish-Install Action。本应用通过引入非持久化 PnP 禁用原语、严格的缓存一致性管理及后台任务调度解决了此问题。

## 核心结论：三层禁用，副作用完全不同

| 层 | 操作方式 | 禁用期间驱动状态 | 重启后 |
| --- | --- | --- | --- |
| NDIS / 接口层 | `netsh interface set interface ... disable`、`Disable-NetAdapter` | **保持加载** | 禁用态跨重启残留，物理卡重启后停在 PnP problem 22（见下文实证） |
| PnP / 设备层（持久） | 设备管理器、`pnputil /disable-device`、`CM_Disable_DevNode(CM_DISABLE_PERSIST)` | **不加载** | 仍是 problem 22，且触发次生安装活动 |
| PnP / 设备层（**非持久**） | `CM_Disable_DevNode(devinst, 0)`（不传 PERSIST） | 不加载 | **自动恢复启用**，驱动正常加载 ← 本应用采用 |

### 2026-10-20 本机实测：netsh disable 同样会跨重启残留 problem 22

早期调研根据文档推断「netsh 只翻 NDIS 的 `AdminStatus`，PnP devnode 不进入 `CM_PROB_DISABLED(22)`」，**该推断被本机日志推翻**：

*   2026-10-19 23:00 夜切只用 netsh 禁用（`[helper] netsh 禁用适配器成功: 以太网`、`netsh 禁用适配器成功: 以太网 2`，全项目此时无任何 `pnputil /disable-device` 调用）。
*   2026-10-20 08:36 重启后应用启动对账：
    `[adapter] 适配器处于设备级禁用(PnP problem 22)，走 pnputil 启用: 以太网 (PCI\VEN_10EC&DEV_8168&...)`
    与 `... 走 pnputil 启用: 以太网 2 (USB\VID_0BDA&PID_8156\4013000001)` —— **两张卡双双 problem 22**。
*   逐日日志统计也一致：10-13~10-17 禁用 2 次、重启启用 0 次（期间未重启）；
    10-18 重启后启用 3 次；10-19 启用 2 次；10-20 启用 2 次。

推论：USB 网卡被 netsh 禁用时，NDIS miniport 停机连带其 PnP devnode 也进入禁用状态并**写入持久配置标志**（`CM_DISABLE_PERSIST` 语义等价的 ConfigFlags `DISABLED=0x1`），重启后设备以禁用态重新枚举、驱动堆栈不加载。这与「PnP 层持久禁用」在重启后的效果相同，区别只在于当前会话里 netsh 是软状态 vs `pnputil` 是硬状态。

已排除的替代解释：快速启动已关闭（`HiberbootEnabled=0`、无 `hiberfil.sys`、`powercfg /a` 只支持 S0），不是休眠持久化；`setupapi.dev.log` 无该设备安装记录，Kernel-PnP/Configuration 无该设备安装事件，所以不是设备被重新安装。

## 官方文档证据（learn.microsoft.com）

1.  **`CM_Disable_DevNode`**：「By default, CM_Disable_DevNode disables a device at one time, but after reboot the device is enabled again. Starting in Windows 10, you can specify the `CM_DISABLE_PERSIST` flag to disable the device across reboots.」——默认非持久，持久需显式标志。
2.  **`SP_PROPCHANGE_PARAMS`**：`DICS_DISABLE` = 「Windows disables the device」，**不卸载驱动堆栈**；只有 `DICS_STOP` 才「The driver stack will be unloaded and the CSCONFIGFLAG_DO_NOT_START flag will be set」；`DICS_PROPCHANGE` 是「stops and restarts the device」；`DICS_FLAG_CONFIGSPECIFIC` 只写注册表配置标志不改当前状态，`DICS_FLAG_GLOBAL` 才真正禁用。
3.  **Finish-Install Action**（`how-finish-install-actions-are-processed`）：「In Windows 8, Windows 8.1, and Windows 10, finish-install actions must be completed in the Action Center by an administrator... Users must click on 'Finish installing device software'.」——installer 返回 `DIF_NEWDEVICEWIZARD_FINISHINSTALL` 并设 `DI_FLAGSEX_FINISHINSTALL_ACTION` 后，设备被标记但任务**排队不自动运行**。
4.  **`CONFIGFLAG_REINSTALL`**：OR 进 `ConfigFlags` 经 `SetupDiSetDeviceRegistryProperty(SPDRD_CONFIGFLAGS)` 写回后，「This causes the device to be reinstalled the next time that the device is reenumerated.」
5.  **`DEVPKEY_Device_ConfigFlags`**：`DEVPROP_TYPE_INT32`，可读写，官方标注「for internal use only」。位值取自 SDK `regstr.h`：`DISABLED=0x1`、`REMOVED=0x2`、`MANUAL_INSTALL=0x4`、`FAILEDINSTALL=0x10`、`FINISH_INSTALL=0x100`、`REINSTALL=0x800`。
6.  **`pnputil /disable-device`**：Win10 2004 起可用，官方文档**无任何持久化说明**，参数仅 `[/reboot] [/force] [/class] [/bus] [/deviceid]`。
7.  **`CM_Reenumerate_DevNode`**：`CM_REENUMERATE_RETRY_INSTALLATION` 官方警告「can cause the PnP manager to prompt the user to perform installation」，需 `SeLoadDriverPrivilege`。

以下两项**查无官方出处，勿依赖**：`*IfAdminStatus` NDIS 关键字（Standardized INF Keywords 与 Keywords-not-displayed-in-UI 两页均无）、`PnPCapabilities` 逐位语义（仅社区共识，`0x18=24` 禁电源管理）。

## 机制定位：RTL8156 的 CD-ROM 模式（本机取证）

重启后「资源管理器多出一个带安装器的盘符、点击后才可用」，对应的是 USB 复合设备的 **CD-ROM 模式**而非 Finish-Install Action。同一序列号 `4013000001` 下 PnP 树里存在三个同源设备（`Get-PnpDevice` 实测）：

| 设备实例 ID | 状态 | 说明 |
| --- | --- | --- |
| `USB\VID_0BDA&PID_8156\4013000001` | OK / Class=Net | 现在工作的网卡 |
| `USB\VID_0BDA&PID_8151\4013000001` | Unknown(幽灵) / Class=USB | 通用 USB 大容量存储设备（`Service=USBSTOR`） |
| `USBSTOR\CDROM&VEN_REALTEK&PROD_USB_CD-ROM&REV_2.00\4013000001&0` | 幽灵 / Class=CDROM | Realtek USB CD-ROM |

时间线（今早）：`08:35:36` 该 USB 设备以 CD-ROM/大容量存储模式被枚举（`LastArrival`），`08:36:05` 消失（`LastRemoval`），`08:36:20` 才以 `PID_8156` Net 模式重新枚举。即 RTL8156 上电默认 CD-ROM 模式，靠驱动发 USB 控制命令切到 NIC 模式；**禁用期间驱动不加载 → 模式切换不发生 → 停在 CD-ROM 模式**，Windows 把它当新的容量存储设备，于是出现安装盘符。

官方 Finish-Install 机制（`DIF_NEWDEVICEWIZARD_FINISHINSTALL` → `DI_FLAGSEX_FINISHINSTALL_ACTION` → Action Center 排队等用户点击）确实是真实的「点击后才可用」机制，但本机 `ConfigFlags=0`（`0x100 FINISH_INSTALL` / `0x800 REINSTALL` 均未命中），故不是本现象的主因。

## 本应用修复：禁用改走非持久化设备级禁用

2026-10-20 修复（分支 `fix/usb-adapter-night-disable-nonpersistent`）：

*   **新实现**：`tauri-app/src-tauri/src/network/discovery/devnode.rs` `disable_device(instance_id)` 调 `CM_Disable_DevNode(devinst, 0)`（**不传** `CM_DISABLE_PERSIST`）→ 轮询复核 problem 22 出现（3s / 200ms，与 enable 侧对称）。`disable_adapter_nonpersistent(adapter_name)` 按名字查 GUID → 读 PnPInstanceId → 设备级禁用，返回 `Ok(Some(instance_id))` / `Ok(None)`（无 PnP 实例 ID 的虚拟卡）。
*   **helper 分流**：`tauri-app/src-tauri/src/helper/mod.rs` `run_disable_adapter` 先试设备级禁用；`Ok(None)`（Wintun/TAP 虚拟卡，不涉及驱动安装）才回退 netsh；物理卡设备级禁用失败时**直接报错**，不静默回退 netsh——那会重新引入跨重启残留。
*   启用侧不变（`devnode.rs` `enable_device` → `pnputil /enable-device` + 复核），与禁用侧构成对称的非持久化设备级开关。

为什么不是「继续用 netsh 并加启动自愈」：netsh 的残留让设备在无人值守的重启窗口里停在 problem 22 / CD-ROM 模式，USB 卡此时是「需要用户点击」的僵态，自愈只能事后补救且必然有一段无网可用期。非持久化禁用从源头保证重启后设备是启用态。

## 架构与关键组件

### 1. 适配器缓存层 (`adapter_cache.rs`)

**核心职责**：提供基于 TTL（5秒）的适配器状态缓存层，封装 Win32 `GetAdaptersAddresses` 调用，支持同步/异步访问、强制刷新及后台预刷新。同时包含适配器启用逻辑（区分设备级禁用与 NDIS 层禁用）及 IP 变更等待工具函数。

**关键常量与类型**：
*   **`AdapterCacheEntry`** (15|): 元组类型 `(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)`，存储完整三元组数据及时间戳。
*   **`ADAPTER_CACHE_TTL_SECS`** (21|): `5` (u64)，缓存有效期。
*   **`CACHE_REFRESH_INTERVAL_SECS`** (272|): `4` (u64)，后台刷新间隔，略短于 TTL 以保持新鲜。
*   **`ADAPTER_CACHE`** (18|): `lazy_static!` 定义的 `RwLock<Option<AdapterCacheEntry>>`，全局单例缓存。
*   **`FORBIDDEN`** (110|): 非法字符数组 `['&', '|', ';', ...]`，用于 `validate_adapter_name`。

**关键函数与导出名**：

*   **`query_adapters_cached_inner()`** (23|-38|): 内部核心逻辑。先读锁检查 TTL，命中则克隆返回；未命中则调用 `crate::network::discovery::query_adapters_addresses()` (32|) 获取全量数据，写锁更新缓存并返回。
*   **`get_all_adapters_cached()`** (44|-46|): 公共入口，直接委托给 `query_adapters_cached_inner`。注释提及替代原 `get_all_adapters_force` 以解决 BE-B-05 中 adapter_watch 高频重查问题。
*   **`get_adapters_cached()`** (50|-65|): 仅返回 `Vec<Adapter>`。命中时仅 clone adapters 字段 (55|)；未命中时查询并写入全量缓存 (62|)。
*   **`get_adapters_cached_async()`** (72|-86|): 异步版本。快速路径（74|-81|）同同步版但避免阻塞；慢路径 (83|) 使用 `tokio::task::spawn_blocking(get_adapters_cached)` 将阻塞 IO 移出 async 运行时。
*   **`get_adapters_force()`** (88|-91|): 强制刷新。通过 `take()` 清空缓存 (89|) 后调用 `get_adapters_cached()`。
*   **`get_adapter_details_cached()`** (93|-105|): 仅返回 `Vec<AdapterDetail>`。命中时仅 clone details (99|)；未命中委托 `query_adapters_cached_inner` (103|)。
*   **`validate_adapter_name(name: &str)`** (107|-113|): 校验适配器名称非空、长度<=128且不含特殊字符。
*   **`enable_adapter(adapter_name, allow_uac_prompt)`** (122|-220|): 启用适配器的复杂分支逻辑。
    *   **设备级分流** (130|-168|): 仅在 Windows 桌面端生效。调用 `find_disabled_device` (134|)。
        *   `DeviceState::Disabled`: 调用 `enable_device` (pnputil 级别) (138|)。
        *   `DeviceState::Unlocatable` (幽灵态): 调用 `reenumerate_device` (总线重枚举) (150|)，随后可能再次尝试 `enable_device` (152|)。
        *   两种情况均触发 `ADAPTER_CACHE.write().take()` (141|/161|) 及注册表缓存失效 (142|-143|, 162|-163|)。
    *   **NDIS 层启用** (170|-206|):
        *   管理员: 直接执行 `netsh interface set interface ... enable` (173|-176|)。
        *   非管理员: 通过 `helper_spawn::spawn_elevated_helper` (191|) 提权执行。
    *   **后置清理** (208|-217|): 无论哪条路径，均清空缓存并刷新注册表缓存 (`refresh_class_subkey_cache`, `invalidate_show_in_ncpa_cache`)。
*   **`wait_for_adapter(max_wait_ms, is_quitting)`** (222|-241|): 轮询等待适配器出现。指数退避策略：初始 delay 1000ms，每次翻倍上限 5000ms (237|)。退出时返回空 vec (228|)。超时后回退到 `get_adapters_cached()` (240|)。
*   **`poll_adapter_ip_quick(adapter_name, timeout_ms, is_quitting)`** (243|-268|): 检测特定适配器 IP 是否变化。记录初始 IP (250|)，每 300ms (247|, BE-A-04 优化) 调用 `get_adapters_force()` 对比。IP 非空且不同则返回 true。
*   **`start_cache_refresh_task(task_manager)`** (279|-308|): 启动后台任务。每 4 秒通过 `spawn_blocking(query_adapters_cached_inner)` (289|) 刷新缓存。处理取消令牌 (302|) 及错误日志。

### 2. 夜间出站切换执行层 (`outbound_night.rs`)

本批源码为夜间出站切换功能的**执行层核心**。它承接了 `outbound_switch`（策略/快照解析）的定义，实现了具体的网络配置变更（跃点修改、网卡禁用/启用、路由清理）、状态管理（退避、互斥、告警）以及生命周期控制（启动对账、手动命令）。

**A. 全局状态与常量定义 (20|-76|)**
*   **超时与退避:**
    *   `OUTBOUND_HELPER_TIMEOUT_SECS = 30`: Helper 提权操作超时。
    *   `OUTBOUND_BACKOFF_START_MS = 60_000`, `MAX = 300_000`: 失败退避阶梯（60s, 120s, 240s, 300s封顶）。
    *   `OUTBOUND_RESTORE_ALERT_FAILS = 3`, `REMIND_EVERY = 20`: 还原失败告警阈值与重复间隔。
    *   `OUTBOUND_RESTORE_GIVE_UP_FAILS = 40`: 损坏快照放弃阀（约 3.3 小时），防止永久卡死。
*   **原子状态变量 (用于并发控制与统计):**
    *   `OUTBOUND_SWITCH_LAST_FAIL_MS`, `OUTBOUND_SWITCH_FAIL_COUNT`: 切换动作的退避状态。
    *   `OUTBOUND_RESTORE_LAST_FAIL_MS`, `OUTBOUND_RESTORE_FAIL_COUNT`: 还原动作的独立退避状态。
    *   `OUTBOUND_CHANNEL_FAIL_COUNT`: 提权通道失败计数（仅 UAC 拒绝/超时等通道级失败才增加，业务失败不增加）。
    *   `OUTBOUND_ACTION_BUSY`: 单飞闸 (`AtomicBool`)，确保切换、还原、重放、看门狗互斥。
    *   `OUTBOUND_NO_CANDIDATE_WARN_DAY` 等四个 `AtomicI32`: 每日一次的非阻塞类问题去重标记。
*   **RAII 锁结构:**
    *   `struct OutboundActionGuard`: 79|-94|。通过 `compare_exchange` 获取 `OUTBOUND_ACTION_BUSY`，`Drop` 时释放。用于防止并发冲突导致的静默状态丢失。

**B. 辅助函数与工具 (98|-265|)**
*   **执行环境:**
    *   `run_outbound_blocking` (98|): 将同步动作送入 `spawn_blocking`，处理线程异常。
*   **状态判定:**
    *   `outbound_switch_active` (114|): 检查三份快照是否任一非空。
    *   `in_outbound_restore_window` (125|): 判断当前是否在恢复窗口 [07:30, 23:00)。
    *   `needs_replay` (203|): 纯函数。仅在“处于切换态”且“有失败历史”时返回 true，避免稳态下无意义重放。
*   **退避逻辑:**
    *   `outbound_backoff_ms` (137|): 计算退避时长。
    *   `backoff_gate` (148|): 判断是否超过退避时间。时钟回拨视为压制。
    *   `switch_gate_open` / `restore_gate_open`: 分别检查切换/还原的退避闸。
*   **Helper 调用封装:**
    *   `run_helper_op` (239|): 通用提权 Helper 调用入口。区分“通道失败”（UAC/超时）和“业务失败”（Helper 成功但操作报错）。
    *   `note_channel_failure` / `clear_outbound_channel_failure`: 管理 UAC 降级触发条件。
*   **校园卡专用启用:**
    *   `enable_campus_adapter` (276|/291|): Windows 下使用 `adapter_cache::enable_adapter` 处理 Problem 22 分流；非桌面平台返回错误。

**C. 核心动作：出站切换 (`apply_outbound_switch`, 391|-583|)**
*   **流程:**
    1.  **互斥检查:** 尝试获取 `OutboundActionGuard`，失败则跳过。
    2.  **退避检查:** 检查 `switch_gate_open`。
    3.  **候选选择:** 获取适配器详情，结合优先级、网关可达性、Portal 探测，选出目标出站卡 (`select_outbound_candidate`)。若无候选，记录每日告警并返回错误。
    4.  **禁用名单选择:** 选出需禁用的校园网卡 (`select_campus_to_disable`)。若为空但有其他候选，记录每日告警。
    5.  **快照落盘:** 先写 `outbound_metric_restore` 和 `outbound_disabled_adapters` 到配置并持久化。**这是切换态成立的标志。**
    6.  **执行操作:**
        *   `write_outbound_metric`: 将目标卡的 IPv4/IPv6 跃点设为 1。
        *   循环禁用校园网卡 (`run_helper_op("disable_adapter")`)。
    7.  **路由验证:** 检查到校园网关的最优路由是否已离开校园卡。若未离开，标记失败。
    8.  **结果:** 全部成功返回 `Ok`，否则返回错误列表。注意：**即使 Helper 失败，快照仍保留**，供补齐路径重试。

**D. 核心动作：出站还原 (`apply_outbound_restore`, 776|-1089|)**
*   **流程:**
    1.  **互斥与退避:** 同上。
    2.  **快照解析:** 解析跃点快照。若损坏，记录每日告警，按“已还原”继续后续步骤（不阻塞整体流程，但标记部分失败）。
    3.  **三步反向执行:**
        *   **① 清理兜底路由:** 解析 `outbound_standby_route`。根据路由表现状（不存在、非本应用创建、唯一默认路径、安全可删）采取不同策略。若删除失败或读取失败，标记 `route_ok = false`。
        *   **② 跃点写回:** 遍历快照中的协议栈，若当前存在且值不同，调用 `run_helper_op("set_metric")` 恢复原值。若网卡消失或值一致，视为已还原。
        *   **③ 重新启用校园卡:** 遍历禁用名单，调用 `enable_campus_adapter`。若快照损坏且达到放弃阀 (`GIVE_UP_FAILS`)，按空名单放行收尾。
    4.  **收尾:**
        *   若任一步骤失败，返回错误，保留快照进入下一轮重试。
        *   若全部成功，发送通知，调用 `finish_outbound_restore` 清空快照和退避状态。

**E. 辅助动作与命令入口 (600|-1191|)**
*   **`replay_outbound_switch` (600|):** 补齐/重放。在切换态且有失败历史时调用。
    *   分支①：补写跃点。
    *   分支②：补做禁用。
    *   任一分支失败不影响其余。全部分支成功后清除切换失败计数。
*   **`watchdog_re_disable_campus` (697|):** 夜间看门狗。在守护窗口内，复核禁用名单中的网卡是否被外部重新启用。若是，则再次禁用。
*   **`manual_outbound_switch` (1156|):** 用户手动切换入口。覆盖退避闸，设置 `manual_hold_day` 防止当日自动切换叠加。
*   **`manual_outbound_restore` (1178|):** 用户手动还原入口。清 Hold 和退避，执行还原。
*   **`reconcile_outbound_on_startup` (1198|):** 启动对账。
    *   若有残留快照：
        *   在恢复窗口或功能关闭 → 立即还原。
        *   在夜间窗口 → 检查目标卡是否仍可用且非校园网。若是，重放切换；否则还原。

### 3. 提权辅助子进程与 PnP 设备控制 (`helper/mod.rs` & `devnode.rs`)

#### `helper/mod.rs`
**职责**：作为提权辅助子进程的核心逻辑层，支持两种执行形态：
1.  **命令行模式 (`--helper`)**：主进程经 CMSTPLUA/runas 提权启动管理员副本，执行单次操作。
2.  **计划任务 Worker 模式 (`--helper-task`)**：由 SYSTEM 身份的计划任务代理拉起，扫描请求目录批量执行，无需 UAC。

**关键类型与常量**：
*   **`HelperOp`** (116|-156|)：枚举所有支持的提权操作。包括 `Dns`, `ClearDns`, `Mac`, `EnableAdapter`, `DisableAdapter`, `RouteDelete`, `EnableDevice`, `ReenumerateDevice`, `SetMetric`, `RegisterTaskProxy`, `SelfCheck`。
    *   注意：`RouteDelete` 注释说明废弃了 `route_add`，因同网关+接口默认路由无法共存。
*   **`HelperResult`** (103|-112|)：序列化结构体，包含 `success`, `message`, `op`, `logs` (可选), `details` (可选, serde_json::Value)。
*   **`TaskRequest`** (315|-321|)：计划任务 Worker 使用的 JSON 结构，含 `op`, `args`, `result`。
*   **路径常量/函数**：
    *   `proxy_base_dir()` (31|-50|)：确定 ProgramData 下的基础目录。优先 `SHGetKnownFolderPath(FOLDERID_ProgramData)`，回退环境变量 `PROGRAMDATA`，再回退硬编码 `C:\ProgramData`。审计 A11 强调防劫持。
    *   `helper_results_dir()` (75|-77|)：固定结果目录 `results\`。
    *   `helper_requests_dir()` (80|-82|)：固定请求目录 `requests\`。
    *   `resolve_result_path(name)` (88|-100|)：**P0-2 安全收口**。严格校验文件名（纯文件名、无路径分隔符、无 `..`、ASCII alnum/hyphen/underscore/dot），防止 SYSTEM 进程被诱导写入任意路径（如覆盖 hosts）。

**核心函数**：
*   **解析与分发**：
    *   `parse_helper_args(args)` (242|-271|)：解析 `--helper <op> [params] --result <path>`。返回 `(HelperOp, Option<String>)`。
    *   `build_op_from_args(op, args)` (161|-238|)：将 op 名和位置参数构造为 `HelperOp`。处理 `--family` 开关。
    *   `run_helper(op, result_path)` (274|-309|)：单例执行入口。匹配 `HelperOp` 调用具体 runner，最后原子写结果文件。
    *   `run_helper_task()` (328|-351|)：Worker 循环。扫描 `requests\` 下最旧的 `req-*.json`，处理完删除，空扫 3 轮后退出。
    *   `process_request(path)` (374|-435|)：读取 JSON -> 立即删除请求文件（防重入）-> 解析并执行 -> 写结果。

*   **具体操作 Runner** (均接收 `&mut Vec<String> logs`)：
    *   `run_dns` (437|-456|)：调用 `crate::network::dns_setup::setup_dns_doh_admin`。
    *   `run_clear_dns` (461|-516|)：仅 Windows。遍历 targets，调用 `crate::platform::dns_config::clear_adapter_dns_via_api`。汇总 restored/failed 到 details。
    *   `run_mac` (518|-578|)：校验 MAC 格式 (12 hex chars)。查找适配器，调用 `apply_mac_change_via_registry` 修改注册表并重启网卡。**关键点**：同时调用 `remove_mac_from_registry` 清除伪装值，因为主进程非提升时无权限清理 HKLM Class 键。
    *   `run_set_metric` (620|-701|)：夜间出站切换。解码 `{guid}:{family}:{automatic}:{metric}` 条目。使用 `windows::Win32::NetworkManagement::IpHelper::SetIpInterfaceEntry`。强制 `SitePrefixLength = 0` 防错。
    *   `run_enable_adapter` (706|-756|)：双校验（名称合法 + 存在于本机列表）。调用 `netsh interface set interface ... enable`。
    *   `run_disable_adapter` (767|-847|)：**核心逻辑差异**。优先尝试 `crate::network::discovery::devnode::disable_adapter_nonpersistent` (非持久化 PnP 禁用，避免重启残留 problem 22)。若虚拟卡无 PnP ID 则回退 `netsh disable`。严禁物理卡失败时静默回退 netsh。
    *   `run_route_delete` (882|-944|)：精确四元组删除路由 (`dest|mask|gateway|metric|if_index`)。调用 `crate::platform::route_table::delete_route_exact`。
    *   `run_enable_device` (963|-1025|)：字符集白名单校验 -> `CM_Locate_DevNodeW` 存在性复核 -> `pnputil /enable-device` -> **轮询复核 problem 22 解除** (不信返回值)。
    *   `run_reenumerate_device` (1030|-1057|)：字符集校验 -> 调用 `crate::network::discovery::devnode::reenumerate_device_raw`。用于 USB 设备禁用后端口复位导致的“幽灵态”。
    *   `run_register_task` (1061|-1079|)：调用 `crate::platform::task_proxy::register_task_via_com`。
    *   `write_result_file` (1083|-1093|)：原子写（先写 `.tmp` 再 rename）。

#### `network/discovery/devnode.rs`
**职责**：封装 Windows PnP (Plug and Play) 设备层的禁用、启用、重枚举操作，解决 NDIS 层 (`netsh`) 无法处理的设备级状态问题。

**关键常量**：
*   `NET_CLASS_GUID`: `"{4D36E972-E325-11CE-BFC1-08002BE10318}"`
*   `CONTROL_NETWORK_ROOT`: `"SYSTEM\\CurrentControlSet\\Control\\Network"`
*   `ENABLE_VERIFY_TOTAL_MS`: 3000ms (普通设备)
*   `USB_VERIFY_TOTAL_MS`: 15000ms (USB 设备，因涉及 CD-ROM/NIC 切换)
*   `REENUMERATE_VERIFY_TOTAL_MS`: 15000ms

**核心函数**：
*   `verify_window_ms(instance_id)` (46|-52|)：根据是否以 `USB\` 开头选择复核窗口时长。
*   `locate_devinst(instance_id)` (55|-68|)：调用 `CM_Locate_DevNodeW` (带 `PHANTOM` 标志)，允许定位已移除/禁用的幽灵设备。
*   `devnode_problem(instance_id)` (72|-85|)：调用 `CM_Get_DevNode_Status` 获取 problem code (如 22 表示禁用)。
*   `read_pnp_instance_id(guid)` (92|-98|)：从注册表 `HKLM\...\Control\Network\{GUID}\{AdapterGuid}\Connection\PnPInstanceId` 读取。
*   `find_guid_by_name(adapter_name)` (102|-116|)：回退方案，遍历注册表按 Name 匹配 GUID。
*   `DeviceState` (120|-126|)：枚举 `Disabled(String instance_id)` 或 `Unlocatable(String instance_id)`。
*   `find_disabled_device(adapter_name)` (137|-160|)：**状态判定核心**。
    1.  获取 GUID (优先适配器缓存，回退注册表)。
    2.  读取 PnP Instance ID。
    3.  查询 Problem Code。
    4.  若 Problem == 22 -> `Disabled`。
    5.  若查询失败 (Err) -> `Unlocatable` (**关键修复**：旧代码将 Err 视为未禁用，导致 USB 重枚举窗口内误判)。
*   `disable_device(instance_id)` (175|-192|)：调用 `CM_Disable_DevNode(devinst, 0)` (非持久化)。轮询确认 Problem 变为 22。
*   `disable_adapter_nonpersistent(adapter_name)` (199|-212|)：组合函数，获取 Instance ID 后调用 `disable_device`。返回 `Ok(Some(id))` 表示成功 PnP 禁用，`Ok(None)` 表示虚拟卡回退 netsh。
*   `enable_device(instance_id, allow_uac_prompt)` (219|-267|)：
    *   若非管理员，通过 `helper_spawn` 提权执行。
    *   若管理员，直接运行 `pnputil /enable-device`。
    *   **复核**：轮询 `devnode_problem` 直到 != 22 或超时。USB 设备给更长窗口。
*   `reenumerate_device(instance_id, allow_uac_prompt)` (285|-307|)：
    *   若非管理员，提权执行。
    *   若管理员，调用 `reenumerate_device_raw`。
*   `reenumerate_device_raw(instance_id)` (310|-337|)：
    1.  `CM_Locate_DevNodeW` 获取 devinst。
    2.  `CM_Get_Parent` 获取父节点 (USB Hub)。若失败则对自身重枚举。
    3.  `CM_Reenumerate_DevNode(parent, SYNCHRONOUS)`。
    4.  轮询 `devnode_problem` 成功（设备重新可定位）即止。

## 数据流

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                                User / Cron                                  │
└───────────────┬───────────────────────────────┬─────────────────────────────┘
                │                               │
                ▼                               ▼
┌──────────────────────────────┐   ┌──────────────────────────────┐
│      outbound_night.rs       │   │     adapter_cache.rs         │
│                              │   │                              │
│  apply_outbound_switch()     │   │  get_adapters_cached()       │
│  ├── select_candidate()       │   │  ├── query_adapters_cached_ │
│  ├── write_snapshot()         │   │  │   inner()                  │
│  ├── run_helper_op(disable)   │   │  └── update ADAPTER_CACHE   │
│  │   └── helper/mod.rs        │   │                              │
│  │       run_disable_adapter  │   │  enable_adapter()            │
│  │           └── devnode.rs   │   │  ├── find_disabled_device() │
│  │               disable_...  │   │  │   ├── DeviceState::Disab │
│  │               (CM_Disable) │   │  │   └── Unlocatable        │
│  └── replay_outbound_switch() │   │  ├── enable_device()        │
│                              │   │  │   └── pnputil /enable    │
│  reconcile_outbound_on_start │   │  └── clear cache & registry│
│                              │   │                              │
└──────────────────────────────┘   └──────────────────────────────┘
```

1.  **入口触发**：
    *   `main.rs` (未展示) 拦截 `--helper` 或 `--helper-task`。
    *   `--helper` -> `helper::parse_helper_args` -> `helper::run_helper`。
    *   `--helper-task` -> `helper::run_helper_task` -> `oldest_request` -> `process_request`。

2.  **操作分发**：
    *   `run_helper` / `process_request` 内部 match `HelperOp`。
    *   例如 `DisableAdapter` -> `run_disable_adapter`。
    *   `run_disable_adapter` 调用 `crate::network::discovery::devnode::disable_adapter_nonpersistent`。
    *   `disable_adapter_nonpersistent` 内部调用 `devnode::disable_device` -> `CM_Disable_DevNode`。

3.  **提权链路**：
    *   `devnode::enable_device` / `devnode::reenumerate_device` 检测 `!is_admin()`。
    *   调用 `crate::platform::helper_spawn::spawn_elevated_helper`。
    *   此 spawn 最终会再次进入 `helper/mod.rs` 的 `run_helper` 流程（通过 `--helper enable_device ...` 参数），形成递归或并行提权子进程。
    *   结果通过 `result_path` 文件交换。

4.  **数据流向**：
    *   **请求**：CLI args 或 JSON (`requests/*.json`) -> `HelperOp` enum。
    *   **执行**：`HelperOp` -> specific runner function -> Win32 API / netsh / pnputil。
    *   **结果**：`HelperResult` struct -> JSON serialization -> `results/*.json` (atomic write)。
    *   **日志**：runner 产生的 `logs: Vec<String>` -> 合并进 `HelperResult.logs` -> 写入结果文件或供主进程统一落盘。

## Connections

[[usb-adapter-enable-pnp-vs-ndis-layers]]（启用方向的姊妹篇：同一两层模型的另一面）、
[[adapter-disabled-classification-blindspot]]、
[[adapter-visibility-cache-staleness]]