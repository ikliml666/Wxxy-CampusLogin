---
title: 网络核心：适配器发现、缓存、DHCP 与子网
type: module
source_files:
  - tauri-app/src-tauri/src/network/mod.rs
  - tauri-app/src-tauri/src/network/adapter.rs
  - tauri-app/src-tauri/src/network/adapter_cache.rs
  - tauri-app/src-tauri/src/network/client.rs
  - tauri-app/src-tauri/src/network/dhcp.rs
  - tauri-app/src-tauri/src/network/subnet.rs
  - tauri-app/src-tauri/src/network/discovery/mod.rs
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/network/discovery/registry.rs
  - tauri-app/src-tauri/src/network/discovery/devnode.rs
tags: [network, adapter, dhcp, mac-reset, discovery, windows, registry, tauri]
---

## Overview

本模块是桌面端网络能力的核心，负责五件事：① **适配器发现**——经 Win32 `GetAdaptersAddresses` 枚举网卡，叠加注册表可见性检查（ncpa.cpl / Win11 高级网络设置）、名称黑名单与 PnP 设备树交叉验证，输出严格的四分类状态；② **适配器缓存**——5 秒 TTL 的进程内缓存 + 4 秒后台常驻刷新，避免高频 Win32/注册表调用；③ **适配器选择**——"自动检测"/手选/降级策略，决定登录、检测、优化等操作的作用范围；④ **DHCP 与 MAC 重置**——`ipconfig` 释放/续租、注册表 `NetworkAddress` 伪装 + 网卡重启，管理员直跑、非管理员经 helper 提权；⑤ **HTTP 客户端池**——Portal 请求复用的 reqwest 客户端池（TTL 600s、LRU 上限 32）与安卓旁路客户端。

平台门控：`discovery/windows.rs`、`discovery/registry.rs`、`discovery/devnode.rs` 仅 Windows 编译（`discovery/mod.rs:13-20` 三连声明）；非 Windows 平台 `query_adapters_addresses` 返回空三元组（`discovery/mod.rs:130-133`），`dhcp_release_renew_single` 降级为 skipped 语义存根（`network/dhcp.rs:469-476`）；`enable_adapter` 与 `enable_device` 的 netsh/pnputil 路径 `#[cfg(desktop)]`；`create_bypass_http_client` 仅安卓编译（`network/client.rs:95`）。本模块全部内容的 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`。

## Key Components

### 模块门面（`network/mod.rs`，共 53 行）

纯 re-export 门面（15 行注释：消除 adapter.rs 中转层）：

- 子模块声明 `1-13`：`adapter`、`adapter_cache`、`bound_socket`（`3-4` 注释说明安卓 SO_BINDTODEVICE 旁路纯函数全平台编译供单测）、`client`、`dhcp`、`discovery`、`dns`、`dns_setup`、`quality`、`subnet`、`timing`。
- `16-19` re-export `discovery::{Adapter, AdapterDetail, DisabledAdapter, is_blacklisted}`；`20` `dhcp_renew_wired_only`；`22-23` Windows 专属 `dhcp_release_renew_all` / `dhcp_release_renew_single`；`25-26` 非 Windows 的 `dhcp_release_renew_single` 存根版（failure_tracker 跨平台调用）。
- `27-31` re-export `subnet::{check_gateway_reachable, check_gateway_reachable_from, is_same_subnet_18, get_wireless_ssid, get_wired_network_profile}`。
- `34-39` re-export `adapter::{resolve_adapter_names, select_adapter, filter_operation_adapters, ensure_ethernet_ip_for_login, find_by_name, find_with_valid_ip, find_dual_adapters, is_secondary_adapter_enabled}`。
- `41-46` re-export `adapter_cache::{get_adapters_cached, get_adapters_cached_async, get_adapters_force, get_disabled_adapters_cached, get_adapter_details_cached, get_all_adapters_cached, wait_for_adapter}`；`48-49` `#[cfg(desktop)]` 额外导出 `enable_adapter`（netsh + 提权，仅桌面）。
- `51` `client::update_portal_url`；`53` `quality::check_network_quality_async`。

### 跨平台类型与平台分发（`network/discovery/mod.rs`，共 184 行）

- `22` `AdapterQueryResult = Result<(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>), String>`。
- `24-32` `BL_REGEX` 名称/描述黑名单：hyper-v/virtual/vmware/docker/wsl/vpn/代理软件全家桶/中文虚拟词等；`26-27` 注释记录 `nat|virtual|tor` 用 `\b` 词边界防误伤（Native/Toronto 等），中文补全覆盖"虚拟/伪/假/测试/模拟/隧道"；`28` **"本地连接"被强制排除是用户业务规则**（Win11 高级网络设置不可见），非虚拟网卡判定。
- `34-57` `AdapterStatus` 四分类枚举（`39-46`，serde camelCase）+ `as_str` 中文标签（`48-57`）：Disabled=已禁用、Disconnected=未连接、EnabledNoIp=未禁用无IP、Connected=已连接。分类依据 IF_OPER_STATUS + IP 是否为空，决策表见 `discovery/windows.rs` 的 `classify_adapter_status`。
- `59-95` 三个 Serialize 结构体：`Adapter`（`59-71`）、`AdapterDetail`（`73-87`）、`DisabledAdapter`（`89-95`），字段全表见下文「结构体与字段」。
- `98-104` `new_command`：子进程附加 `CREATE_NO_WINDOW`（`0x08000000`）隐藏控制台窗口。
- `106-108` `is_blacklisted`：正则匹配入口。
- `111-122` `prefix_len_to_mask`（cfg windows）：IPv4 前缀长度 → 点分十进制掩码。
- `125-134` `query_adapters_addresses` 平台分发：Windows 走 `self::windows::query_adapters_addresses`，其余平台返回空三元组。
- `136-184` 单元测试 4 个：黑名单词边界不误伤合法物理网卡（`143-153`）、仍命中已知虚拟网卡（`156-164`）、中文虚拟关键字（`167-176`）、中文合法网卡不误伤（`179-183`）。

### Windows 发现实现（`network/discovery/windows.rs`，共 479 行）

**`query_adapters_addresses`（`12-63`）**：`GAA_FLAGS = 0x0080 | 0x0100`（`16`）、以太网/无线 IfType 常量 6/71（`17-18`）；先空参探测所需缓冲区大小（`20-23`），`size == 0` 直接返回空（`25-27`）；随后最多 3 次重试循环（`29-60`）：重试时缓冲区追加 4096 字节余量（`31`），返回 111（ERROR_BUFFER_OVERFLOW）或实际尺寸超缓冲时更新 size 重试（`44-50`），其他错误重探尺寸后重试（`52-57`），重试耗尽报错。

**`parse_adapter_addresses`（`65-284`）**——链表遍历，逐适配器：
1. `FriendlyName`（`81`）与 GUID 规整化：`AdapterName` 不带花括号时补 `{}`（`83-98`）。
2. 过滤：IfType 非以太网/无线跳过（`100-104`）；`PhysicalAddressLength == 0` 跳过（`106-109`）；`is_visible_in_ncpa(&guid)` 不可见跳过（`114`）；名称或描述命中黑名单跳过（`121`）。
3. 字段提取：`is_wireless`/`if_index`（`126-127`）；`link_speed` 来自 `ReceiveLinkSpeed`，未连接时 Windows 返回 `u64::MAX` 哨兵归 0（`128-134`）；MAC 取前 6 字节冒号格式（`136-141`）。
4. 仅 `is_up` 时提取：首个 AF_INET 单播地址 + `OnLinkPrefixLength`（`147-163`），`169.254.` APIPA 自配地址清空视为无 IP（`164-167`）；首个网关地址（`173-188`）与 DHCPv4 服务器（`190-197`）。
5. 四分类输入采集（`200-225`）：非 Up 时查 `GetIfEntry2` 的 `AdminStatus`（`205` → `is_admin_status_down`，`317-328`，查询失败返回 `None` 回退）、注册表 `ConfigFlags`（`206` → `is_admin_disabled_via_registry`）、PnP 设备树存活（`212-217` → `devnode::read_pnp_instance_id` + `devnode_problem`）；`218-225` 调 `classify_adapter_status`。
6. **所有**适配器都推入 `adapters` 列表（`228-237`，前端统一展示与启用操作）；按状态分发（`241-278`）：Connected → details（含 subnet_mask/gateway/dhcp_server）、EnabledNoIp → details（IP/掩码置空但保留 dhcp_server 供诊断）、Disabled → disabled 列表（兼容旧 API）。

**`classify_adapter_status`（`345-367`，纯函数便于单测）**：is_up → 有 IP Connected / 无 IP EnabledNoIp（`353-355`）；PnP 设备树已无活动 devnode（`pnp_present == Some(false)`）一律按未连接——USB 网卡拔出后 AdminStatus/ConfigFlags 残值都是残影（`357-359`）；`AdminStatus == Down` → Disabled（`360-362`，微软文档化的管理性禁用标志）；`NotPresent && ConfigFlags DISABLED` → Disabled（`363-365`，旧行为回退）；否则 Disconnected（`366`）。

**辅助函数**：`read_pwstr`（`286-307`，`to_string` 失败时手动计长，超 4096 UTF-16 单元告警并返回空）、`ipv4_from_in_addr`（`309-311`）。

**单元测试 `369-479`**（11 个用例，`classify_adapter_status` 决策表全覆盖）：Up 有/无 IP（`378-384`、`386-392`）、Down + AdminStatus Down 判禁用（`396-402`，核心修复）、Down + AdminStatus Up 未连接（`404-411`）、AdminStatus 查询失败回退（`413-420`）、NotPresent + AdminStatus Down（`422-429`）、NotPresent + 注册表标志（`431-438`）、NotPresent 无任何标志（`440-447`）、幽灵设备 + AdminStatus 残值 / 注册表残值均按未连接（`451-457`、`459-465`）、PnP 查询不可用回退旧行为（`467-478`）。

### 注册表可见性（`network/discovery/registry.rs`，共 216 行）

- `54-77` `is_visible_in_ncpa`：空 GUID 回退"可见"（`57-59`，历史缺陷：物理适配器 AdapterName 为空时曾被静默隐藏）；可见 = 注册表 1（`Connection\ShowInNetworkConnections != 0`，`72`）**且**注册表 2（Class 子键存在同 GUID 的 `NetCfgInstanceId`，`76`）——后者防 Wi-Fi Direct 幽灵虚拟副本（WLAN 2/3/4/5）。
- `81-86` `SHOW_IN_NCPA_CACHE`（guid → bool 整表缓存）+ TTL 5 秒（与适配器缓存对齐）；`89-102` `query_show_in_ncpa`（值 ≠ 0 可见、值或子键缺失视为可见交由 PnP 检查把关）；`105-131` `show_in_ncpa_cached`：TTL 内逐条补入新 guid，过期后单条重建换新缓存（`126-129`，注册表 I/O 在锁外）；`135-137` `invalidate_show_in_ncpa_cache`——**适配器启用等可见性变更路径必须显式失效**。
- `140-143` `ClassSubkeyEntry { exists: bool, config_flags: Option<u32> }`；`145-148` `CLASS_SUBKEY_CACHE`（小写 guid → entry）；`151-171` `build_class_subkey_cache`（遍历 `Class\{4D36E972-...}` 子键读 `NetCfgInstanceId` + `ConfigFlags`）；`174-177` `refresh_class_subkey_cache`（显式刷新入口）；`181-190` `ensure_cache_initialized`（双重检查锁，构建在锁外）。
- `192-202` `class_subkey_has_matching_guid`；`205-216` `is_admin_disabled_via_registry`：`ConfigFlags & 0x1`（CONFIGFLAG_DISABLED，`214`），区分 NotPresent 下的"管理员禁用"与"硬件缺失"。
- `15-50` 集成测试（需真实 Windows）：空/不存在 GUID 返回 false（`19-21`、`24-26`）、真实 WLAN GUID（`29-35`）、四个已知幽灵 WLAN GUID 表（`38-49`）。

### PnP 设备级禁用（`network/discovery/devnode.rs`，共 200 行）

Windows「网卡被禁用」是两层独立状态（`1-11` 模块注释）：NDIS admin 层（netsh 可翻）与 PnP 设备层（设备管理器式禁用，problem 22）——外接 USB 网卡被系统设置禁用后 netsh enable 只触发重连、清不掉 PnP 禁用。

- `25-26` 常量：`NET_CLASS_GUID`（{4D36E972-...}）、`CONTROL_NETWORK_ROOT`；`31-32` 复核轮询参数 `ENABLE_VERIFY_TOTAL_MS = 3000` / `ENABLE_VERIFY_INTERVAL_MS = 200`（实测 PnP 状态落盘与 problem 状态有 96~330ms 时差）。
- `36-59` `devnode_problem`（**pub(crate)**）：`CM_Locate_DevNodeW`（PHANTOM 允许幽灵，`43`）+ `CM_Get_DevNode_Status`（`50`），返回 problem code；Err = devnode 不可定位或状态查询失败（含 USB 重枚举重建窗口）。
- `66-72` `read_pnp_instance_id`（**pub(crate)**）：读 `Control\Network\{class}\{guid}\Connection\PnPInstanceId`（未文档化注册表值）；`63-65` 注释说明 pub(crate) 原因——monitor 出站切换判总线可判定性（USB 卡自 2026-10 起不再豁免夜间禁用）与 windows.rs 幽灵误判修复都需要。
- `76-90` `find_guid_by_name`（私有回退）：接口行缺失时遍历 `Control\Network` 按 `Connection\Name` 匹配。
- `96-116` `find_disabled_device`：guid 两级来源（`98-101`：先 `get_adapters_force` 适配器列表在线取，回退注册表按名称）；仅 problem 22 返回 `Some(instance_id)`（`110-115`），其余（无 problem/查询失败）返回 None 维持 netsh 原路径——不对已启动设备做设备级操作触发不必要重枚举。
- `123-169` `enable_device`：非管理员经 `spawn_elevated_helper("enable_device", 30s, allow_uac_prompt)` 由 SYSTEM worker 执行（`124-141`，worker 内含相同校验与复核）；管理员直跑 `pnputil /enable-device`（`143-155`，stderr 为空时回退 stdout 取错误细节）；随后**不信命令返回值**，轮询复核 problem 22 确实解除（`159-168`）：CM 查询失败（重枚举窗口）继续等，只认 problem 离开 22，超时仍 22 或持续失败报错交上层重试。
- `171-200` 测试 4 个：名称匹配不存在返回 None（`175-178`）、不存在的 GUID（`180-183`）、虚构实例 ID 的 problem 查询（`185-194`）、不存在适配器的 `find_disabled_device`（`196-199`）。

### 适配器选择（`network/adapter.rs`，共 398 行）

历史职责留存：适配器发现已迁 `discovery`、缓存已迁 `adapter_cache`、DHCP/MAC 已迁 `dhcp`、子网/SSID 已迁 `subnet`（`1-5` 模块注释）。

- `16-23` `find_by_name` / `find_with_valid_ip`；`26-39` `find_dual_adapters`（a2 需 `dual_adapter` 开启且名称非空且 ≠ a1，`33`）。
- `44-46` `is_secondary_adapter_enabled`：`dual_adapter && adapter2 非空`——仅判断"启用"语义，不判断与 a1 不同/非哨兵（`41-43` 注释），附加条件由调用方负责。
- `53-64` `configured_disabled_adapters`：用户手选（非空且 ≠ `AUTO_DETECT_ADAPTER`）且当前处于禁用列表的适配器；adapter1 手选即纳入，adapter2 需 dual 开启且手选（`57-60`）；纯函数供 adapter_watch 监控循环与单测。
- `66-118` `resolve_adapter_names`：自动检测优先有线有 IP → 任意有 IP → 第一个（`68-75`）；配置名存在则用配置名，**不在当前可见列表时降级自动检测并告警**（`79-88`）；adapter2 同构三战线（`90-115`，降级 `101-111`），dual 关闭返回空串。
- `123-128` `filter_operation_adapters`：检测/优化等操作只作用于 resolve 出的主/副适配器，UI 展示仍用全量列表。
- `130-142` `select_adapter`：空列表早退（`131`）；统一经 `resolve_adapter_names` 解析（`136`，历史决策：只解析出的主适配器上取 IP，不回退配置范围外适配器——与登录/注销作用范围一致，`133-135` 注释）。
- `144-222` `ensure_ethernet_ip_for_login`：登录前以太网 IP 检查——resolve 主/副（`150`）中筛"有线且 IP 为空"的候选（`152-165`），逐个执行：退出标志检查（`172-174`）→ 登录日志（`176-180`）→ `ipconfig /renew` spawn（`182-184`，**spawn 失败告警不再静默吞掉**，`187-191` 历史缺陷注释）→ `poll_adapter_ip_quick(name, 5000)`（`194`）→ poll 结束 kill+wait 回收 ipconfig（`196-202`，kill 可能中断半途 DHCP 事务但防子进程残留）→ 成功时 `get_adapters_force` 取新 IP 记录日志（`204-220`）。
- `224-398` 单元测试：`make_test_config`（`229-292`，Config 全字段构造）、`make_test_adapter`（`294-310`）、配置名失效降级（`312-322`）、配置名存在（`324-334`）、自动检测有线优先（`336-345`）、`configured_disabled_adapters_manual_only` 全矩阵（`355-397`，空串/哨兵/单选手选/双双手选/不在禁用列表等 8 场景）。

### 适配器缓存（`network/adapter_cache.rs`，共 294 行）

- `15-21` 缓存条目 `(Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant)`，`ADAPTER_CACHE: RwLock<Option<_>>`，TTL 5 秒。
- `23-38` `query_adapters_cached_inner`：读锁 TTL 判定 → 未命中全量查询并写缓存；`44-46` `get_all_adapters_cached`（BE-B-05：adapter_watch 15s 监听改走缓存版，最多陈旧 4s，避免与后台刷新叠加强刷）；`50-65` `get_adapters_cached`（命中路径仅 clone adapters，BE-B-04）；`72-86` `get_adapters_cached_async`：快速路径读锁 clone 非阻塞返回，慢路径 `spawn_blocking` 转移阻塞的 Win32 调用（`83`）；`88-100` / `107-119` disabled / details 版本（同样仅克隆所需 Vec）；`102-105` `get_adapters_force`（take 清缓存重查）。
- `121-127` `validate_adapter_name`：非空、≤128 字节、禁字符表 `& | ; \` $ ( ) < > " ' / \n \r \0`（`124`）——所有 netsh/ipconfig/pnputil 拼接前的注入防线（调用方举例：`network/dhcp.rs:36`、`network/adapter_cache.rs:137`）。
- `135-205` `enable_adapter`（cfg desktop）：
  1. PnP 设备级禁用分流（`143-153`）：`devnode::find_disabled_device` 命中 problem 22 → `devnode::enable_device`（pnputil 设备级启用 + 复核）→ 失效 `ADAPTER_CACHE` + `refresh_class_subkey_cache`（`150`）+ `invalidate_show_in_ncpa_cache`（`151`）后返回；netsh 只翻 NDIS admin 层清不掉设备管理器式禁用（`139-142` 注释）。
  2. 管理员：直接 `netsh interface set interface <名> enable`（`155-170`，失败时经 `decode_console_bytes` 取 stderr 错误细节）。
  3. 非管理员：`spawn_elevated_helper("enable_adapter", 30s, allow_uac_prompt)` 走 helper 框架（`171-191`）；`allow_uac_prompt` 语义（`129-134` 注释）：手动按钮传 true；监控循环按失败计数——首试 false（CMSTPLUA 可用时零打扰），重试 true（被系统封堵时 UAC 是唯一恢复通道）。
  4. 收尾三重失效（`194-201`）：`ADAPTER_CACHE.take()` + `refresh_class_subkey_cache`（`197`）+ `invalidate_show_in_ncpa_cache`（`201`，BE-B-04：可见性变更必须立即失效短 TTL 缓存）。
- `207-226` `wait_for_adapter`：轮询直到拿到非空列表或超时——退出标志返回空表（`212-214`）、`get_adapters_force` 强查（`216`）、指数退避 1000ms → ×2 → cap 5000ms（`209`、`221-222`）、超时最后一次 `get_adapters_cached`（`225`）。
- `228-253` `poll_adapter_ip_quick`：DHCP 续租等待——300ms 间隔（`232`，BE-A-04：原 100ms 高频强刷放宽，IP 变更检测至多延迟 300ms）；**记录初始 IP，IP 变为非空且 ≠ 初始值才算续租成功**（`235-238`、`246`）。
- `257-294` 4 秒后台刷新：`CACHE_REFRESH_INTERVAL_SECS = 4`（略短于 TTL 保证新鲜，`255-256`）；`start_cache_refresh_task`（`264-294`）经 `BackgroundTaskManager.spawn("adapter_cache_refresh")` 托管（`267`），跳过首次立即 tick（`269`），每 4s `spawn_blocking(query_adapters_cached_inner)`（`274`），**刷新失败保留旧缓存避免雪崩**（`279-281`），cancel_token 取消退出（`287-290`）。

### DHCP 与 MAC 重置（`network/dhcp.rs`，共 498 行）

- `22-33` `ipconfig_output_failed`：ipconfig 失败时**退出码常仍为 0**，按中英错误关键字（error/cannot/failed/失败/错误/无法，`30`）兜底判定。
- `35-67` `dhcp_renew` / `dhcp_release`：`ipconfig /renew|/release <适配器名>`（先过 `validate_adapter_name`），退出码 + 输出关键字双判定。
- `71-93` `dhcp_renew_wired_only`：对 resolve 后的主/副适配器中的**有线**网卡续租（不再遍历系统全部有线适配器，`69-70` 注释），返回 `[{name, success}]` JSON。
- `99-122` `generate_random_mac`（cfg windows）：`getrandom::fill`（Windows 走 BCryptGenRandom，`104`）；失败时降级回"时间+计数器"种子 LCG（`106-115`，BE-A-06：原实现同一毫秒内 MAC 可推算）；首字节 `& 0xFC | 0x02` 保证单播 + 本地管理位（`117`）；输出 12 位十六进制无分隔（`118-121`）。`125-131` `mac_with_dashes` 转连字符格式。
- `139-191` 注册表读写（cfg windows）：`set_mac_via_registry`（`139-163`）遍历 `Class\{4D36E972-...}` 子键按 `NetCfgInstanceId` 匹配后写 `NetworkAddress`，打开失败且 Access Denied 时给出"需管理员"提示（`145-149`）；`remove_mac_from_registry`（`166-191`）删除该值，删除失败仅告警。
- `193-213` `netsh_disable` / `netsh_enable`（`name=` / `admin=` 参数形式，先过名称校验）。
- `215-247` `poll_ip_change` / `poll_adapter_has_ip`：300ms 间隔轮询 `get_adapters_force`，前者要求 IP ≠ 旧值并返回新 IP，后者只要求非空。
- `252-273` `apply_mac_change_via_registry`（cfg windows）：管理员 / helper 提权上下文共用——写 `NetworkAddress`（`257`）→ `dhcp_release`（`258`）→ `netsh_disable` + 500ms（`259-264`）→ **`netsh_enable` 失败即整体失败**（网卡停在停用态会静默断网，`265-270`）→ `dhcp_renew`（`271`）。
- `277-323` `try_modify_mac`：管理员直写 `set_mac_via_registry`（`278-285`）；非管理员 `spawn_elevated_helper("mac", [guid, mac], 25s, true)`（`290-296`）——helper 已完成写注册表 + 重启网卡，主进程侧 `poll_ip_change` 25 秒确认 IP 变更（`309-315`），提权失败提示以管理员运行（`317-320`）。
- `327-436` `renew_adapter_with_mac`：单适配器 MAC 重置 + DHCP 全流程，两层前置保护——虚拟适配器黑名单跳过（`329-338`，is_blacklisted）、非校园网 /18 子网跳过（`339-348`，is_same_subnet_18）；三分支（`359-425`）：注册表写失败仅补 DHCP 释放/续租（`359-368`）；helper 提权路径由 helper 完成注册表清理（`369-382`，`378-379` 注释：本进程非提升再清只会 Access Denied 产生误导告警）；管理员路径完整执行释放 → disable + 500ms → enable + `poll_adapter_has_ip` 3s → renew + `poll_ip_change` 5s → 清理 `NetworkAddress`（`383-425`）；返回 `{name, wireless, ip, regOk, success, skipped, reason}`（`427-435`）。
- `442-465` 入口（均 cfg windows）：`dhcp_release_renew_all`（`442-457`，网关为空报错 `443-445`，targets 为空返回空——不触碰名单外适配器，`438-440` 注释）、`dhcp_release_renew_single`（`460-465`）；`469-476` 非 Windows 存根（skipped 语义，failure_tracker 跨平台可用）。
- `478-498` 测试：虚拟网卡黑名单命中（`483-489`）与物理网卡保留（`492-497`）。

### 子网 / SSID / 网关（`network/subnet.rs`，共 231 行）

- `12` `NETSH_QUERY_CACHE_TTL_SECS = 60`（BE-B-03：后台巡检每 15s 固定 spawn netsh 子进程，加 TTL 缓存；SSID 变更检测基于适配器列表不受影响，`9-11` 注释）；`15-20` `NetshCache` 类型与 `SSID_CACHE` / `WIRED_PROFILE_CACHE`（仅缓存 Ok 结果）；`23-35` 缓存读写辅助。
- `37-77` `get_wireless_ssid`（缓存包装 `37-46`）+ `get_wireless_ssid_uncached`（`48-77`）：`netsh wlan show interfaces`，取 `SSID` 开头（排除 `BSSID`）行冒号后内容，过滤"不在/not connected/disconnected"。
- `79-117` `get_wired_network_profile`：`netsh lan show interfaces`，匹配 profile/配置文件/設定檔 行（`103-105`）。
- `119-180` 网关 ICMP 探测：`check_gateway_reachable`（`119-121`）→ `check_gateway_reachable_from`（`163-180`）——**Windows IPv4 优先走 `platform::icmp_probe::icmp_probe_v4`（IcmpSendEcho2Ex，2000ms，`171-175`）**：surge_ping 的 socket2 绑源 DGRAM 在 TUN 环境下系统性失败导致校园网卡被误判非校园（`167-169` 注释）；其余平台/v6 走 `gateway_reachable_async`（`126-161`，surge_ping 异步实现：按目标地址族选 ICMP v4/v6（`131-134`）、`Config::bind` 绑源 IP（`135-141`）、pinger timeout 2000ms + 外层 tokio timeout 硬上限兜底（`151-160`）），调用链均运行在 spawn_blocking 线程内故用 `block_on_sync` 驱动（`176-179` 注释）。
- `183-194` `is_same_subnet_18`：`u32` 按位与掩码 `0xFFFF_C000`（255.255.192.0）比较。
- `196-231` 测试 4 个：同 /18 真（`201-206`）、跨 /18 边界与异网段假（`209-216`）、非法 IP（`219-223`）、非法网关（`226-230`）。

### HTTP 客户端池（`network/client.rs`，共 172 行）

- `7-10` 全局态：`PORTAL_URL: ArcSwap<String>`（初值 `default_portal_url()`）与 `CLIENT_POOL: DashMap<ClientPoolKey, (reqwest::Client, Instant)>`；`14` `ClientPoolKey = (Option<IpAddr>, u8, u64)`——本机地址、TLS 版本紧凑标识（`18-26` `tls_version_id`）、超时毫秒，热路径零堆分配；`29` 池 TTL 600 秒（与 dns.rs DNS 缓存对齐）、`31` 容量上限 32。
- `33-37` `update_portal_url`：非空才 store（启动装载 `app/startup.rs:207`，配置保存 `commands/config_cmd.rs:200`、`:256`）。
- `43-70` `build_client`：默认头 `Cache-Control: no-store` + `Pragma: no-cache`（`44-52`）、`min_tls_version` / 总超时 / 连接超时 3s / `no_proxy` / 重定向上限 5 / 每主机空闲连接 4 / 空闲 90s / TCP keepalive 30s（`54-63`）、可选 `local_address` 绑定出口（`65-67`）。
- `75-89` `client_pool_get`：命中用读路径 clone 返回、**不刷新时间戳**（get_mut 写锁会串行化同 key 并发请求，代价是淘汰退化为按插入时间，`72-74` 注释）；过期经 `remove_if` 原子判断 + 删除（`82-86`，防 drop 与 remove 之间另一线程 or_insert 新条目被误删）。
- `120-155` `create_safe_http_client`：先查 TLS 1.3 键（`121-123`）→ TLS 1.2 键（`125-128`）→ 都未命中新建 TLS 1.3、构建失败降级 TLS 1.2（`130-141`）→ 入池（`143`）→ 容量超 32 按 Instant 最早插入剔除（`145-153`）。
- `95-118` `create_bypass_http_client`（**cfg(target_os = "android")**，单参数 `timeout`）：经 `bound_socket::bypass_proxy_addr()` 取本地旁路代理地址（`97-99`，代理未启动——能力封堵/无物理网卡——返回 Err，调用方回退原路径），默认头/重定向/超时与安全客户端同款（`100-108`、`110-114`），请求经 `reqwest::Proxy::all("http://{addr}")` 转发（`109`、`115`），传输层由旁路 socket SO_BINDTODEVICE 物理网卡直连（`91-94` 模块注释的对照实验记录）；**不设 local_address**——出口由旁路 socket 决定。
- `164-172` `clear_client_pool`：**网络路由变化后必须调用**（安卓 `bindProcessToNetwork` 的 fwmark 只在 socket 创建时打标，池内 keep-alive 旧连接仍走绑定前路由，`158-162` 注释）；`#[allow(dead_code)]`（桌面 bin 不调用），唯一调用方 `android/src-tauri/src/protocol_cmds.rs:289`。

## 结构体与字段

| 结构体 | 位置 | 字段 |
|---|---|---|
| `Adapter` | `discovery/mod.rs:59-71` | name, ip, wireless, guid, mac（XX:XX:… 冒号格式）, if_index(u32), status(`AdapterStatus`), link_speed(u64 bit/s，0=未知；未连接时 u64::MAX 已归 0) |
| `AdapterDetail` | `discovery/mod.rs:73-87` | name, ip, wireless, subnet_mask, gateway, dhcp_server, mac, if_index, status, link_speed |
| `DisabledAdapter` | `discovery/mod.rs:89-95` | name, status(String 中文标签), description |
| `AdapterStatus` | `discovery/mod.rs:39-46` | Disabled / Disconnected / EnabledNoIp / Connected（serde camelCase；中文标签 `as_str` `48-57`） |
| `AdapterCacheEntry` | `adapter_cache.rs:15` | (Vec\<Adapter\>, Vec\<AdapterDetail\>, Vec\<DisabledAdapter\>, Instant) |
| `ClientPoolKey` | `client.rs:14` | (Option\<IpAddr\>, u8 TLS 版本标识, u64 超时毫秒) |
| `ClassSubkeyEntry` | `discovery/registry.rs:140-143` | exists(bool), config_flags(Option\<u32\>) |

三个结构体均 `#[serde(rename_all = "camelCase")]`，前端契约即 camelCase 字段名。

## Data Flow

1. **适配器查询主路径**：UI/命令层 → `get_adapters_cached_async`（`adapter_cache.rs:72-86`，命中读锁 clone / 未命中 spawn_blocking）→ `query_adapters_addresses`（`discovery/windows.rs:12-63`，GAA 三次重试）→ `parse_adapter_addresses`（`65-284`：可见性 `is_visible_in_ncpa` → 黑名单 → 四分类）→ 写入 `ADAPTER_CACHE`（TTL 5s）。
2. **4 秒后台刷新**：`app/startup.rs:223` 启动 `start_cache_refresh_task`（`adapter_cache.rs:264-294`）→ 每 4s spawn_blocking 全量刷新，失败保留旧缓存；shutdown 经 BackgroundTaskManager 取消。
3. **15 秒适配器监听**（[[desktop-monitor]] 的 adapter_watch）：`monitor/adapter_watch.rs:9`（`ADAPTER_WATCH_INTERVAL = 15000`）→ `:64` `get_all_adapters_cached`（走缓存，BE-B-05）→ `:49-56` 每轮 `refresh_class_subkey_cache`（BE-B-01）→ 检测到禁用变更时 `:195` `configured_disabled_adapters` 过滤手选项 → `:242` `enable_adapter`（allow_uac 按失败计数阶梯）。
4. **登录前 DHCP 续租**：登录流程 → `ensure_ethernet_ip_for_login`（`adapter.rs:144-222`）→ 主/副中有线无 IP 者 `ipconfig /renew` + `poll_adapter_ip_quick`(5s)，结果写登录日志（EventBus）。
5. **MAC 重置链路**：命令入口 `commands/network_cmd.rs:202`（single）/ `dhcp_release_renew_all`（`dhcp.rs:442-457`）→ `renew_adapter_with_mac`（`327-436`，黑名单 + /18 子网两层前置过滤）→ `try_modify_mac`（`277-323`）：管理员直写注册表 + netsh 重启；非管理员经 `spawn_elevated_helper("mac", 25s)` → `helper/mod.rs:502` 调 `apply_mac_change_via_registry`、`:507` 调 `remove_mac_from_registry`（helper 提权上下文内完成写伪装值与清理）→ 主进程 `poll_ip_change` 确认 IP 变更。失败重试场景由 [[desktop-auth]] 的 failure_tracker 触发（`auth/failure_tracker.rs:77`、`:161`、`:262`）。
6. **网关可达性探测**：campus_check / failure_tracker / background_check → `check_gateway_reachable_from`（`subnet.rs:163-180`）→ Windows IPv4 走 `platform::icmp_probe::icmp_probe_v4`，其余走 surge_ping 异步探测（spawn_blocking 线程内 block_on_sync 驱动）。
7. **HTTP 出口**：[[desktop-auth]] 的 portal/protocol 层 `auth/portal.rs:1`、`auth/protocol.rs:1` 导入 `PORTAL_URL` + `create_safe_http_client`（调用点 `auth/portal.rs:103`、`auth/protocol.rs:147`、`:291`）→ 客户端池（TTL 600s、LRU 32、TLS 1.3→1.2 降级）；安卓旁路 `create_bypass_http_client`（`auth/portal.rs:228`、`auth/protocol.rs:155`、`:296`）经 bound_socket 旁路代理直连物理网卡。
8. **Portal URL 热更新**：启动（`app/startup.rs:207`）与配置保存（`commands/config_cmd.rs:200`、`:256`）→ `update_portal_url`（`client.rs:33-37`）→ ArcSwap 原子替换，读侧无锁。

## Connections

- [[desktop-monitor]] — `monitor/adapter_watch.rs` 每 15s 消费 `get_all_adapters_cached`（`:64`）与 `configured_disabled_adapters`（`:195`）、自动启用走 `enable_adapter`（`:242`）、每轮刷新 `refresh_class_subkey_cache`（`:56`）；出站切换经 `devnode::read_pnp_instance_id` 判总线可判定性。
- [[desktop-auth]] — 登录/探测/注销全链路依赖本模块：`PORTAL_URL` + `create_safe_http_client` / `create_bypass_http_client`（`auth/portal.rs:1`、`auth/protocol.rs:1`）、适配器选择与登录前 DHCP（`ensure_ethernet_ip_for_login`）、MAC 重置重试（`auth/failure_tracker.rs:77`、`:161`、`:262`）。
- [[desktop-commands]] — `commands/network_cmd.rs` 的适配器/DHCP/MAC 命令面（`4-7` 批量导入缓存与 DHCP API、`:158-161` `resolve_adapter_names` + `filter_operation_adapters` + `dhcp_renew_wired_only`、`:202` `dhcp_release_renew_single`、`:228` `select_adapter`）；`commands/config_cmd.rs:200`、`:256` 热更 `update_portal_url`。
- [[desktop-platform]] — `platform::elevation::is_admin`（管理员直跑判定）、`platform::helper_spawn::spawn_elevated_helper`（enable_adapter / mac / enable_device 三类提权操作）、`platform::console_output::decode_console_bytes`（netsh/ipconfig/pnputil 输出解码）、`platform::icmp_probe::icmp_probe_v4`（Windows 网关探测首选通道）。
- [[desktop-helper-update]] — helper 进程的 MAC 分支调 `network::dhcp::apply_mac_change_via_registry`（`helper/mod.rs:502`）与 `remove_mac_from_registry`（`helper/mod.rs:507`）、`EnableAdapter` 操作分发（`helper/mod.rs:229`、`:348`，worker 实现 `:654`）。
- [[android-backend]] — 安卓端复用 `is_same_subnet_18` 与 `clear_client_pool`（`android/src-tauri/src/protocol_cmds.rs:289`，网络绑定切换后必须清池）；`create_bypass_http_client` 与 `bound_socket` 旁路链路仅安卓编译。

## Known Issues

1. **ipconfig 退出码不可信**：失败时退出码常仍为 0，`dhcp.rs:22-33` 按中英错误关键字兜底判定；适配器名不含这些词，误报面可忽略。
2. **`ensure_ethernet_ip_for_login` 的历史缺陷两则**（`adapter.rs:187-191`、`197-200`）：spawn 失败曾被 `if let Ok` 静默吞掉后误报"续租超时"（已改为告警 + 跳过）；poll 结束即 kill 可能中断半途 DHCP 事务，但保留 kill 防 ipconfig 悬挂，kill 失败仅记录。
3. **`poll_adapter_ip_quick` 的轮询权衡**（`adapter_cache.rs:230-232`，BE-A-04）：原 100ms 每次 `get_adapters_force` 强刷 GetAdaptersAddresses，放宽到 300ms 后 IP 变更检测至多延迟 300ms。
4. **随机 MAC 的可预测性降级路径**（`dhcp.rs:100-116`，BE-A-06）：`getrandom` 低概率失败时降级回"时间+计数器"种子 LCG，理论上同毫秒可推算；保证函数总能返回合法单播/本地管理 MAC。
5. **禁用判定的文档陷阱**（`windows.rs:200-204`、`345-367`）：微软文档明文禁用后 OperStatus「Down 或 NotPresent 皆可能」——旧实现只认 NotPresent + ConfigFlags，禁用报 Down 的网卡被归为"未连接"，自动启用永远不触发；现以 `GetIfEntry2` 的 AdminStatus（文档化管理性禁用标志）优先。
6. **幽灵 USB 网卡误判**（`windows.rs:207-217`、`357-359`，六期修复）：拔出后接口行 AdminStatus 残留 Down、注册表 ConfigFlags 残留 DISABLED 位，旧判定把"未连接"误报成"已禁用"；现先问 PnP 设备树（CM_Locate_DevNodeW 定位失败或状态查询失败 → 残影，一律按未连接），PnPInstanceId 读不到时回退旧行为不凭空改判。
7. **空 GUID 曾被静默隐藏**（`registry.rs:55-59`）：空 GUID 无法做注册表匹配，现回退"可见"交由黑名单/IP 过滤兜底。
8. **`ShowInNetworkConnections` 短 TTL 缓存**（`registry.rs:85-86`，5s）：平时按 TTL 失效，**适配器启用等可见性变更路径必须立即失效**（BE-B-04），否则短窗内返回陈旧结果——`enable_adapter` 收尾三重失效之一（`adapter_cache.rs:201`）。
9. **Class subkey 缓存只显式刷新**（`registry.rs:174-190`）：构建后不过期，仅 `refresh_class_subkey_cache` 显式重建；调用点 `adapter_cache.rs:150`、`:197` 与 `adapter_watch.rs:56`（每 15s，BE-B-01）。
10. **客户端池 LRU 精度退化**（`client.rs:72-74`、`143-153`）：命中路径不刷新时间戳（避免 get_mut 写锁串行化同 key 并发），淘汰退化为按条目插入时间，TTL 语义不变。
11. **客户端池并发误删防护**（`client.rs:82-86`）：过期清理用 `remove_if` 原子判断，防 drop(entry) 与 remove 之间另一线程 or_insert 的新客户端被无条件 remove 误删。
12. **surge_ping 绑源在 TUN 环境系统性失败**（`subnet.rs:167-175`）：socket2 绑源 DGRAM 会使校园网卡被误判为非校园（夜间禁用名单漏卡），Windows IPv4 已改走 `IcmpSendEcho2Ex`（`platform::icmp_probe`）；v6 网关与非 Windows 仍走 surge_ping。
13. **`"本地连接"` 是业务规则排除**（`discovery/mod.rs:28`）：Win11 高级网络设置中不可见故强制排除，与虚拟网卡黑名单无关；`nat/virtual/tor` 等模式串用 `\b` 词边界防误伤 Native/Toronto 等合法名（`26-27`）。
14. **link_speed 哨兵值与平台存根**：未连接时 Windows 返回 `u64::MAX`（内部 -1），`windows.rs:128-134` 归 0 表示未知（原样透传会被前端换算成 18446744073.7 Gbps）；非 Windows 平台 `dhcp_release_renew_single` 返回 skipped 语义存根（`dhcp.rs:469-476`）保持 failure_tracker 跨平台可用。
15. **缓存后台刷新失败保留旧缓存**（`adapter_cache.rs:262`、`279-284`）：刷新异常只告警不写 None，避免缓存雪崩；helper 提权路径的 MAC 伪装值清理由 helper 在提权上下文内完成（`dhcp.rs:378-379`），主进程非提升状态再清只会 Access Denied 产生误导性告警，故不再尝试。
