---
title: 提权辅助子进程 helper
type: module
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
tags: [提权, helper, windows, dns, mac]
---

## Overview

本模块解决一个权限落差问题：Tauri 主进程以普通用户运行，而 Windows 上设置 DNS+DoH、改写 MAC、启停适配器、精确删除路由、设置接口跃点、PnP 设备级启用/重枚举、注册计划任务/无头核心开机任务等操作都要求管理员或 SYSTEM 权限。做法是**让进程以提权身份重启自己**，在提权上下文内执行单个 op，把结果写进固定目录下的 JSON 文件，主进程再取回。

两种执行形态共用本模块的 op 分发与结果回写（`tauri-app/src-tauri/src/helper/mod.rs:1-7`）：

| 形态 | 入口 | 触发方式 | UAC |
| --- | --- | --- | --- |
| `--helper <op> [参数] --result <文件名>` | `parse_helper_args` (`helper/mod.rs:260-289`) → `run_helper` (`helper/mod.rs:292-331`) | 主进程经提权（CMSTPLUA 静默 / `runas`）启动管理员副本执行单次操作 (`helper/mod.rs:4-5`) | 视提权通道而定 |
| `--helper-task` | `run_helper_task` (`helper/mod.rs:350-373`) | 计划任务代理（`platform/task_proxy.rs` 注册的 SYSTEM 主体任务）拉起本 exe，worker 扫描请求目录逐个执行 op (`helper/mod.rs:6-7`) | 无 |

本文只覆盖被提权一侧（服务端）的 `helper/mod.rs`；发起侧（通道选择、命令行拼装、结果轮询解析）位于 `platform/helper_spawn.rs`，提权原语位于 `platform/elevation.rs`，两者源码不在本文输入范围内，此处不对其内部实现作断言。

本模块固化的设计决定：

1. **结果收口**：结果一律写进 `helper_results_dir()` 下的**纯文件名**，绝不接受请求方给的任意路径（P0-2，`helper/mod.rs:9-10`）。`resolve_result_path` (`helper/mod.rs:88-100`) 只放行 ASCII 字母数字和 `-`、`_`、`.`，拒绝 `/`、`\`、`..`、`.` 开头、空串与超过 128 字符的名字，杜绝「SYSTEM 写任意路径」原语（如覆盖 hosts 或系统 DLL）。
2. **helper 不初始化 logger**：文件头注释 (`helper/mod.rs:12-13`) 明确说明，避免与主进程跨进程写同一日志文件竞争；诊断信息收集进 `HelperResult::logs`，由主进程读取后统一落日志。
3. **原子写**：`write_result_file` (`helper/mod.rs:1235-1245`) 先写 `<name>.json.tmp` 再 `rename`，主进程不会读到半截 JSON；结果目录首次可能不存在（固定目录而非 `%TEMP%`），写前 `create_dir_all` (`helper/mod.rs:1236-1238`)。
4. **提权侧对入参零信任**：worker 以 SYSTEM 身份运行，而请求文件落在 ProgramData 下任何本地用户可写的位置，因此每个 op 都在交给 Win32 或外部命令前做白名单/字符集/存在性校验：`valid_instance_id` (`helper/mod.rs:975-983`)、`decode_set_metric_row` (`helper/mod.rs:610-640`)、`decode_route_row` (`helper/mod.rs:878-898`)、适配器名双校验 (`helper/mod.rs:733`、`helper/mod.rs:742-744`、`helper/mod.rs:794`、`helper/mod.rs:803-805`)。
5. **ProgramData 路径可信化**：`SHGetKnownFolderPath` 与环境变量都可能被劫持/损坏，`proxy_base_dir` 三级降级且每一级都过 `validate_programdata_path` (`helper/mod.rs:19-28`、`helper/mod.rs:37-43`)。
6. **`route_add` 已废弃**：`HelperOp` 中不再有添加路由的变体，只保留精确删除 `RouteDelete` (`helper/mod.rs:131-136`)。原因是 2026-10-18 事故后确认：同网关+接口的默认路由无法与目标卡自己的 DHCP 默认路由共存（Windows 判重复），兜底路由这一设计已废弃 (`helper/mod.rs:131-134`)。
7. **请求文件先读后删**：`process_request` 把请求读入内存后**立即删文件** (`helper/mod.rs:406`)，之后的篡改/重建不影响本次执行。

## Key Components

### 路径与目录

- `validate_programdata_path(p: &std::path::Path) -> bool` (`helper/mod.rs:19-28`)：审计 A11。要求绝对路径、非空，且首个 component 是 `Component::Prefix`（盘符前缀 `C:\`）或 `Component::RootDir`（根 `\\server\share`）(`helper/mod.rs:24-27`)。
- `proxy_base_dir() -> std::path::PathBuf` (`helper/mod.rs:31-50`)：代理根目录（请求/结果/自检标记的固定位置）。Windows 上优先 `known_folder_programdata()` 且过滤 `validate_programdata_path`，失败回退 `PROGRAMDATA` 环境变量（同样过滤），再不行硬编码 `r"C:\ProgramData"`，最后 `.join("CampusLogin")` (`helper/mod.rs:37-44`)；非 Windows 返回 `std::env::temp_dir().join("campus-login-proxy")` (`helper/mod.rs:46-49`)。
- `known_folder_programdata() -> Option<std::path::PathBuf>` (`helper/mod.rs:55-72`)：`SHGetKnownFolderPath(FOLDERID_ProgramData, KNOWN_FOLDER_FLAG(0), HANDLE::default())`，空指针返回 `None`，成功后 `CoTaskMemFree` 释放返回的 `PWSTR` (`helper/mod.rs:59-70`)；复用已有 `Win32_UI_Shell` feature，不新增依赖 (`helper/mod.rs:52-53`)。
- `helper_results_dir() -> std::path::PathBuf` (`helper/mod.rs:75-77`)：`proxy_base_dir().join("results")`，worker 以 SYSTEM 身份只在此目录内写结果。
- `helper_requests_dir() -> std::path::PathBuf` (`helper/mod.rs:80-82`)：`proxy_base_dir().join("requests")`，计划任务 worker 扫描此目录。
- `resolve_result_path(name: &str) -> Result<std::path::PathBuf, String>` (`helper/mod.rs:88-100`)：校验结果文件名并拼到固定结果目录。空串或 `len > 128` 拒绝 (`helper/mod.rs:90-92`)；含 `/`、`\`、`..` 或以 `.` 开头拒绝 (`helper/mod.rs:93-95`)；只允许 ASCII 字母数字与 `-`、`_`、`.` (`helper/mod.rs:96-98`)；非法时错误文案为 `"结果文件名非法: {name:?}（要求纯文件名）"` (`helper/mod.rs:89`)。

### 数据结构

`HelperResult` (`helper/mod.rs:102-112`)：提权操作的统一回执，`#[derive(Debug, Clone, Serialize)]`。

| 字段 | 类型 | 行号 | 说明 |
| --- | --- | --- | --- |
| `success` | `bool` | `:104` | 是否成功，决定进程退出码 |
| `message` | `String` | `:105` | 摘要消息，直接透传给前端 |
| `op` | `String` | `:106` | 操作类型名 |
| `logs` | `Vec<String>` | `:108` | 诊断日志；`#[serde(skip_serializing_if = "Vec::is_empty")]` (`:107`)，空时不出现该键 |
| `details` | `Option<serde_json::Value>` | `:111` | 操作完整明细（如 DNS 设置的 `dnsSuccess`/`dnsFailed`/`dohAdded`、`clear_dns` 的 `restored`/`failed`），供主进程透传前端；`#[serde(skip_serializing_if = "Option::is_none")]` (`:110`) |

`HelperOp` (`helper/mod.rs:114-164`)：op 枚举，`#[derive(Debug, Clone, PartialEq)]`；带 `#[cfg(target_os = "windows")]` 的变体在非 Windows 构建中不存在。

| 变体 | 行号 | 语义 |
| --- | --- | --- |
| `Dns { targets: Vec<String>, family: String }` | `:119`（注释 `:117-118`） | 设置 DNS + 启用 DoH，只处理 `targets` 名单内的适配器（主进程 resolve 后传入）；`family` 为优化目标 `"ipv4"`/`"ipv6"`/`"both"` |
| `ClearDns { targets: Vec<String> }` | `:122`（注释 `:120-121`） | 恢复 DNS 自动获取（清除适配器级 DNS），`targets` 是适配器 GUID 名单，复用 Dns 的位置参数传入方式 |
| `Mac { guid: String, mac_no_dash: String }` | `:124` | 修改适配器 MAC（注册表 `NetworkAddress` + 重启网卡） |
| `EnableAdapter { name: String }` | `:127`（cfg `:126`） | `netsh interface set interface enable` |
| `DisableAdapter { name: String }` | `:130`（cfg `:129`） | 禁用网络适配器（夜间切换临时下线校园网卡） |
| `RouteDelete { rows: Vec<String> }` | `:136`（cfg `:135`，注释 `:131-134`） | 精确删除历史兜底默认路由，`rows` 为 `"{dest}\|{mask}\|{gateway}\|{metric}\|{if_index}"` 编码条目，全部严格校验后执行 |
| `EnableDevice { instance_id: String }` | `:139`（cfg `:138`） | PnP 设备级启用（`pnputil /enable-device`），含 problem 22 解除复核 |
| `ReenumerateDevice { instance_id: String }` | `:144`（cfg `:143`，注释 `:140-142`） | 设备所在总线重枚举（`CM_Reenumerate_DevNode`，等价「扫描检测硬件改动」）；USB 网卡禁用后端口复位会让设备实例在 PnP 树里消失（幽灵态），`pnputil /enable-device` 与 `netsh` 都无从下手，必须先重扫总线拉回设备 |
| `SetMetric { rows: Vec<String> }` | `:150`（cfg `:149`，注释 `:145-148`） | 设置接口跃点（出站切换），`rows` 为 `"{guid}:{family}:{automatic}:{metric}"`；family 2=IPv4 / 23=IPv6，automatic 1=恢复自动跃点、0=静态 metric |
| `RegisterTaskProxy` | `:153`（cfg `:152`） | 注册计划任务提权代理（仅计划任务 worker 内执行，本身处于提权上下文） |
| `RegisterBootTask { config_b64: String }` | `:158`（cfg `:157`，注释 `:154-156`） | 注册无头核心开机任务：`config_b64` 为 CoreConfig JSON 的 base64（worker 写配置 + 收紧 ACL + 注册 BOOT 触发的 SYSTEM 计划任务，见 `platform/core_autostart.rs`） |
| `RemoveBootTask` | `:161`（cfg `:160`，注释 `:159`） | 移除无头核心开机任务（删任务 + 删配置文件，幂等） |
| `SelfCheck` | `:163`（注释 `:162`） | 通道自检（写结果即通过，供代理注册后验证全链路） |

`TaskRequest` (`helper/mod.rs:336-343`)：计划任务代理的请求 JSON（worker 与主进程共享的文件协议），`#[derive(Debug, serde::Deserialize)]`。

| 字段 | 类型 | 行号 | 说明 |
| --- | --- | --- | --- |
| `op` | `String` | `:338` | op 名，交给 `build_op_from_args` |
| `args` | `Vec<String>` | `:340` | `#[serde(default)]` (`:339`)，缺省为空数组，因此 `selfcheck` 这类无参 op 的请求可省略该键 |
| `result` | `String` | `:342`（注释 `:341`） | 结果文件名（纯文件名，worker 经 `resolve_result_path` 收口到固定目录） |

### 入口函数

- `build_op_from_args(op: &str, args: &[String]) -> Result<HelperOp, String>` (`helper/mod.rs:169-256`)：命令行 `--helper` 与计划任务请求 JSON **共用**的构造器。扫描时把 `--family <v>` 收进开关（缺取值报 `"--family 缺少取值"`），其余 `--` 开头参数跳过，剩下的进 `positional` (`helper/mod.rs:174-189`)；随后按 op 名匹配 (`helper/mod.rs:190-254`)，缺参数与未知 op 一律 `Err`（如 `"helper mac 缺少 GUID"`、`"helper mac 缺少 MAC"`、`"未知 helper 操作: {other}"`）。
- `parse_helper_args(args: &[String]) -> Result<Option<(HelperOp, Option<String>)>, String>` (`helper/mod.rs:260-289`)：解析 `--helper <op> [op参数...] [--family <v>] --result <文件名>`。未命中 `--helper` 返回 `Ok(None)`；命中但参数非法返回 `Err`（调用方不应启动正常应用）。`--result` 单独抽出（缺取值报 `"--result 缺少路径"`），其余参数交给 `build_op_from_args` (`helper/mod.rs:271-287`)。
- `run_helper(op: HelperOp, result_path: Option<String>) -> i32` (`helper/mod.rs:292-331`)：执行单个 op 并返回进程退出码（成功 0 / 失败 1）。按 op 分发到各 `run_*` (`helper/mod.rs:294-323`)；有 `result_path` 时经 `resolve_result_path` 收口后 `write_result_file` (`helper/mod.rs:324-329`)，名字非法时只 `eprintln!("helper 结果路径非法: {e}")` (`helper/mod.rs:327`)，退出码仍由 op 结果决定 (`helper/mod.rs:330`)。
- `run_helper_task() -> i32` (`helper/mod.rs:350-373`)：计划任务 worker 入口（`main.rs` 拦截 `--helper-task` 后调用）。循环取最旧请求 → `process_request`；空扫累计 3 轮（每轮 `sleep(400ms)`）后退出 (`helper/mod.rs:354-371`)。多扫两轮是为兜住 `MultipleInstances=IgnoreNew` 下「第二个请求在首个实例运行期落盘」的竞态 (`helper/mod.rs:347-349`)。任一请求返回 1 即把整体退出码置 1 (`helper/mod.rs:359-361`)；绝不初始化 logger (`helper/mod.rs:346`)。

### 请求扫描与处理

- `oldest_request(dir: &std::path::Path) -> Option<std::path::PathBuf>` (`helper/mod.rs:376-392`)：取请求目录中最旧的 `req-*.json`。只收 `starts_with("req-") && ends_with(".json")` 的文件，按 `modified()` 升序排序后取第一条；读目录失败、无匹配或元数据缺失时返回 `None`。
- `process_request(path: &std::path::Path) -> i32` (`helper/mod.rs:396-461`)：处理单个请求。读文件失败时 `eprintln!` 并删文件返回 1 (`helper/mod.rs:397-404`)；读入内存后立即删请求文件 (`helper/mod.rs:406`)；JSON 解析失败 `eprintln!` 并返回 1 (`helper/mod.rs:407-413`)；`build_op_from_args` 与 `resolve_result_path` 任一失败都产出 `success: false` 的回执 (`helper/mod.rs:449-456`)；op 分发逻辑与 `run_helper` 同构 (`helper/mod.rs:417-447`)；最后再解析一次结果路径写回 (`helper/mod.rs:457-459`)；返回 `if result.success { 0 } else { 1 }` (`helper/mod.rs:460`)。

### 各 op 的执行实现

- `run_dns(targets: &[String], family: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:463-482`)：先记日志（`"helper: 开始设置DNS+DoH（目标适配器: ...，优化目标: {family}）"`，空名单显示「无」，`helper/mod.rs:464-467`），调 `crate::network::dns_setup::setup_dns_doh_admin(targets, family)` (`helper/mod.rs:468`)，从返回 JSON 取 `success`（缺省 false）与 `message`（缺省 `"设置DNS+DoH完成"`），并把**整个 JSON** 放进 `details` (`helper/mod.rs:469-481`)。
- `run_clear_dns(targets: &[String], logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:487-542`)：非 Windows 分支直接返回「仅支持Windows」(`helper/mod.rs:488-498`)；Windows 分支取 `crate::network::get_adapters_force()`（失败按空列表处理）(`helper/mod.rs:505`)，逐 GUID 调 `crate::platform::dns_config::clear_adapter_dns_via_api(guid)` (`helper/mod.rs:515`)，日志与明细优先用适配器名、查不到时以 GUID 兜底 (`helper/mod.rs:510-514`)。明细入 `details.restored` / `details.failed`（失败格式 `"{name}: {e}"`）(`helper/mod.rs:516-525`)；三态文案（全成功 `"已恢复DNS自动获取（{} 个适配器）"` / 全失败 `"恢复DNS自动获取失败: ..."` / 部分失败 `"部分适配器恢复失败: ..."`）与管理员路径 `commands/network_cmd.rs` 的 `reset_dns` 保持一致 (`helper/mod.rs:527-533`)。
- `run_mac(guid: &str, mac_no_dash: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:544-604`)：先校验 MAC 必须是 12 位十六进制、无分隔符 (`helper/mod.rs:546`)，否则直接失败——`NetworkAddress` 注册表值直接写裸 MAC，非法格式会静默写坏网卡配置 (`helper/mod.rs:545`)；再 `get_adapters_force()` 枚举 (`helper/mod.rs:556`) 并按 GUID（忽略大小写）定位适配器 (`helper/mod.rs:568`)；`apply_mac_change_via_registry(guid, &adapter.name, mac_no_dash)` 成功后**在管理员上下文内清除注册表伪装值** `remove_mac_from_registry(guid)` (`helper/mod.rs:585-587`)——主进程非提升时对 HKLM Class 键无写权限，仅靠主进程清理会让伪装 MAC 每次重启后持续生效且用户无从恢复 (`helper/mod.rs:582-584`)。
- `decode_set_metric_row(row: &str) -> Result<(String, u16, bool, u32), String>` (`helper/mod.rs:610-640`)：解码 `"{guid}:{family}:{automatic}:{metric}"`，返回（原始带花括号 GUID、协议栈 AF_INET=2/AF_INET6=23、是否自动跃点、静态跃点值）。字段数必须为 4 (`helper/mod.rs:611-616`)，GUID 非空 (`helper/mod.rs:617-619`)，`family` 只接受 `"2"`/`"23"` (`helper/mod.rs:620-628`)，`automatic` 只接受 `"1"`/`"0"` (`helper/mod.rs:629-635`)，`metric` 必须能解析为 `u32` (`helper/mod.rs:636-638`)。抽成纯函数是为了可单测。
- `run_set_metric(rows: &[String], logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:645-727`)：空条目直接失败（报成功会让调用方误判切换已生效，`helper/mod.rs:658-660`）；逐条 `decode_set_metric_row` → `crate::platform::metric::interface_rows_for_guid(&guid)` 枚举当前接口行 (`helper/mod.rs:674`) → 按 `r.Family.0 == family` 找到对应行 (`helper/mod.rs:685`) → 改副本：`UseAutomaticMetric = BOOLEAN(automatic as u8)`、`Metric = metric`、`SitePrefixLength = 0` (`helper/mod.rs:691-694`，后者不置 0 会让 `SetIpInterfaceEntry` 报 `ERROR_INVALID_PARAMETER`) → `SetIpInterfaceEntry(&mut target)` (`helper/mod.rs:695`)。单条解码/枚举/未找到/调用失败都 `continue` 并累入 `failed`；任一条失败即整体失败，明细汇总进 `message`（`"接口跃点设置失败（{} 条失败）: ..."`）(`helper/mod.rs:722-726`)。
- `run_enable_adapter(name: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:731-782`)：双校验——`crate::network::adapter_cache::validate_adapter_name(name)` 字符集 (`helper/mod.rs:733`) + 必须存在于 `get_adapters_force()` 列表（防参数注入）(`helper/mod.rs:742-744`)。随后 `crate::network::discovery::new_command("netsh").args(["interface", "set", "interface", name, "enable"])` (`helper/mod.rs:755-757`)；失败时用 `crate::platform::console_output::decode_console_bytes(&o.stderr)` 解出可读原因，stderr 为空时报 `"netsh 返回非零退出码但未输出错误信息"` (`helper/mod.rs:762-768`)。
- `run_disable_adapter(name: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:792-873`)：双校验同上 (`helper/mod.rs:794`、`helper/mod.rs:803-805`)。优先走**非持久化设备级禁用** `crate::network::discovery::devnode::disable_adapter_nonpersistent(name)` (`helper/mod.rs:816`)：`netsh disable` 的禁用态会跨重启残留，重启后物理网卡停在 PnP problem 22，USB 卡（RTL8156 等）还会因驱动不加载停在出厂 CD-ROM 模式，Windows 弹「正在安装设备驱动程序」且需点击才能用 (`helper/mod.rs:787-791`)。返回 `Ok(Some(instance_id))` 即成功并报「设备级禁用适配器成功（非持久化）」(`helper/mod.rs:817-827`)；`Ok(None)`（无 PnP 实例 ID 的虚拟卡，Wintun/TAP，不涉及驱动安装）才回退 `netsh` (`helper/mod.rs:828-831`、`helper/mod.rs:845-848`)；`Err(e)` 时**不静默回退 netsh**，宁可报错交上层重试，以免重新引入跨重启残留缺陷 (`helper/mod.rs:832-843`)。
- `decode_route_row(row: &str) -> Result<(std::net::Ipv4Addr, std::net::Ipv4Addr, std::net::Ipv4Addr, u32, u32), String>` (`helper/mod.rs:878-898`)：解码 `"{dest}|{mask}|{gateway}|{metric}|{if_index}"`，字段数必须为 5 (`helper/mod.rs:880-883`)，三个地址走 `std::net::Ipv4Addr` 解析（注入尝试如空格、斜杠、`-flag` 全部挡在解析层），`metric`/`if_index` 必须能解析为 `u32` (`helper/mod.rs:884-897`)。
- `run_route_delete(rows: &[String], logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:907-970`)：空条目失败 (`helper/mod.rs:916-918`)；先整体解码，任一条非法即整体失败 (`helper/mod.rs:919-925`)；再逐条 `crate::platform::route_table::mask_to_prefix_len(*mask)` → `crate::platform::route_table::delete_route_exact(dest, prefix_len, gateway, if_index, metric)` (`helper/mod.rs:929-942`)。不用 `route.exe delete` 的原因写在 `helper/mod.rs:900-906`：它只按（目的+掩码+网关+接口）匹配、跃点不参与，会把目标卡自己的 DHCP 默认路由一起删掉（2026-10-18 事故：WLAN 失去默认路由，只能重连恢复）。`DeleteExact::Deleted` 计数并记日志 (`helper/mod.rs:943-948`)，`DeleteExact::AlreadyGone` 只记日志、不算失败 (`helper/mod.rs:949-953`)；有失败则整体失败 (`helper/mod.rs:957-969`)。删除门禁（是否仍在用、是否为当前唯一默认路径）由调用方按实时路由表判定后才会发起本操作 (`helper/mod.rs:904-906`)。
- `valid_instance_id(instance_id: &str) -> bool` (`helper/mod.rs:975-983`)：设备实例 ID 字符集白名单（形如 `USB\VID_0BDA&PID_8156\4013000001`）——非空、`len <= 200`、不以 `-` 开头，字符只允许 ASCII 字母数字与 `\`、`&`、`.`、`_`、`-`、`*`，因此拒绝 `/`（pnputil 开关前缀）、空格与控制字符。设备级启用与总线重枚举共用。
- `invalid_instance_id_result(instance_id: &str, op: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:987-1000`)：实例 ID 白名单校验失败的统一结果（`"设备实例 ID 非法: {instance_id:?}"`），避免两个 worker 逐字重复四字段构造。
- `run_enable_device(instance_id: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:1005-1062`)：`valid_instance_id` 白名单 (`helper/mod.rs:1007-1009`) + `devnode::devnode_problem(instance_id)` 存在性复核 (`helper/mod.rs:1010-1018`) 双校验，再 `crate::network::discovery::new_command("pnputil").args(["/enable-device", instance_id])` (`helper/mod.rs:1020-1022`)。**不信 pnputil 返回值**：轮询 `devnode_problem` 直到 `problem != PROBLEM_DISABLED` (`helper/mod.rs:1038-1049`)，窗口取 `crate::network::discovery::devnode::verify_window_ms(instance_id)`（与主进程同源；USB 设备等价物理拔插，3s 必然误判，`helper/mod.rs:1033-1037`），超时报 `"pnputil 已执行但设备仍处于 PnP 禁用态: {instance_id}"`，轮询间隔 200ms (`helper/mod.rs:1050-1061`)。
- `run_reenumerate_device(instance_id: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:1066-1088`)：同样先过 `valid_instance_id` (`helper/mod.rs:1068-1070`)，再调 `crate::network::discovery::devnode::reenumerate_device_raw(instance_id)` (`helper/mod.rs:1072`)——SYSTEM worker 内执行 `CM_Reenumerate_DevNode(SYNCHRONOUS)`，把停在幽灵态的 USB 设备重新枚举出来 (`helper/mod.rs:1064-1065`)。
- `run_register_task(logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:1091-1110`)：调 `crate::platform::task_proxy::register_task_via_com()` (`helper/mod.rs:1094`)，仅计划任务/提权副本上下文内调用（COM 注册需要管理员，`helper/mod.rs:1090`）。
- `run_register_boot_task(config_b64: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:1117-1199`)：反解并校验核心配置 → 写配置文件并收紧 ACL（SYSTEM+Administrators 专有，移除继承）→ 校验 core exe 存在 → 注册 BOOT 触发的 SYSTEM 计划任务。配置含密码机器作用域密文，ACL 是访问控制边界。先 `decode_core_config_arg` (`helper/mod.rs:1129-1132`)，校验账号密码非空 (`helper/mod.rs:1133-1135`)；`create_dir_all` (`helper/mod.rs:1137-1140`)；原子写 `core-config.json.tmp` 后 `rename` (`helper/mod.rs:1142-1154`)；`icacls` 收紧目录 ACL (`helper/mod.rs:1157-1171`)；校验 `core_exe_path` 存在 (`helper/mod.rs:1173-1179`)；调 `register_boot_task_via_com` (`helper/mod.rs:1182-1197`)，任务注册失败时删除已写配置不留凭据孤儿 (`helper/mod.rs:1194-1196`)。
- `run_remove_boot_task(logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:1202-1231`)：调 `remove_boot_task_via_com` (`helper/mod.rs:1214`)；任务已删后删配置文件（`NotFound` 忽略，其他错误仅告警）(`helper/mod.rs:1218-1223`)，幂等返回成功 (`helper/mod.rs:1224-1230`)。
- `write_result_file(path: &std::path::Path, result: &HelperResult)` (`helper/mod.rs:1235-1245`)：原子写——`create_dir_all(parent)` (`helper/mod.rs:1236-1238`) → `serde_json::to_vec` (`helper/mod.rs:1239`) → 写 `path.with_extension("json.tmp")` (`helper/mod.rs:1240-1241`) → `rename` 到目标 (`helper/mod.rs:1242`)。

### 测试（`helper/mod.rs:1247-1579`，`mod tests`）

| 测试 | 行号 | 覆盖点 |
| --- | --- | --- |
| `validate_programdata_path_rules` | `:1253-1260` | A11 路径合法性：`C:\ProgramData`、`D:\` 通过；相对路径、`..\evil`、空串拒绝 |
| `parse_no_helper_returns_none` | `:1263-1266` | 未命中 `--helper` 返回 `Ok(None)` |
| `parse_dns_with_result` | `:1269-1279` | 无位置参数时 `Dns { targets: [], family: "both" }`，`--result` 抽取（含绝对路径字符串，仅测试解析行为） |
| `parse_dns_with_targets_and_family` | `:1282-1302` | 位置参数 `以太网`/`Wi-Fi` 与 `--family ipv6` 扫描 |
| `parse_clear_dns_with_targets` | `:1305-1325` | GUID 名单位置参数 |
| `parse_mac` / `parse_mac_missing_args_is_err` / `parse_unknown_op_is_err` | `:1328-1355` | mac 参数顺序、缺 MAC 报错、未知 op 报错 |
| `helper_result_roundtrip` | `:1358-1372` | JSON 字段名（`success`/`message`/`op`/`logs`/`details`） |
| `helper_result_omits_empty_logs_and_details` | `:1375-1386` | 空 `logs` 与 `details: None` 时对应键不出现 |
| `resolve_result_path_accepts_plain_name` | `:1391-1395` | 纯名 `r-1234-1695.json` 落在 `helper_results_dir()` 内 |
| `resolve_result_path_rejects_traversal_and_absolute` | `:1398-1410` | P0-2 收口：绝对路径、`..\..\evil.json`、`sub/evil.json`、`sub\evil.json`、`..`、`.hidden.json`、空串全部拒绝 |
| `build_op_enable_adapter` | `:1415-1418` | 适配器名含空格（`以太网 2`）原样保留 |
| `build_op_enable_device` / `build_op_reenumerate_device` | `:1421-1448` | 设备实例 ID 原样传入（含 `\` 与 `&`） |
| `instance_id_whitelist_rejects_injection` | `:1451-1458` | 防 `pnputil` 开关注入：`/remove-device`、空格、`--remove-device`、空串拒绝 |
| `build_op_selfcheck_and_register_task` | `:1461-1467` | 无参 op 构造（含 `RegisterTaskProxy`） |
| `build_op_unknown_is_err` | `:1470-1472` | 未知 op 报错 |
| `set_metric_round_trip_and_reject` | `:1477-1524` | 编码条目 round-trip（`{ABC-DEF}:2:0:1` → family 2/automatic false/metric 1；`{ABC-DEF}:23:1:0` → family 23/automatic true/metric 0）；字段数不足/多余、空 GUID、`family=6`、`automatic=true`、`-1`、`abc`、越界 u32、空串全部拒绝 |
| `task_request_json_roundtrip` | `:1527-1539` | 请求 JSON 反序列化与缺省 `args` |
| `decode_route_row_accepts_valid_and_rejects_injection` | `:1543-1578` | 合法五元组 `0.0.0.0\|0.0.0.0\|192.168.6.1\|2\|14` 通过；字段数错误、越界地址 `256.168.6.1`、非点分 `192.168.6`、`-flag`、`/remove`、非数字跃点、空串全部拒绝 |

## Architecture

```text
┌──────────────── 主进程（普通用户，发起侧不在本文范围） ────────────────┐
│  拼装 --helper <op> [位置参数] [--family v] --result <纯文件名>        │
│  或写 requests/req-*.json = {op, args, result}（计划任务代理通道）      │
└──────────────────────────────────┬────────────────────────────────────┘
                                   ▼ 提权副本 / SYSTEM 计划任务拉起本 exe
┌──────────────────── helper/mod.rs（被提权一侧） ──────────────────────┐
│ main.rs 拦截参数                                                      │
│   --helper ──► parse_helper_args (:260) ──► build_op_from_args (:169) │
│                  └─► run_helper (:292) ──► run_dns/run_mac/...        │
│   --helper-task ──► run_helper_task (:350)                            │
│                  ├─ oldest_request (:376) 取最旧 req-*.json           │
│                  ├─ process_request (:396) 读入即删请求 (:406)         │
│                  └─ build_op_from_args + resolve_result_path (:414)   │
│   入参校验层：valid_instance_id (:975) / decode_set_metric_row (:610) │
│              decode_route_row (:878) / validate_adapter_name (:733)   │
│   结果层：resolve_result_path (:88) ► write_result_file (:1235) 原子写│
└──────────────────────────────────┬────────────────────────────────────┘
                                   ▼
        ProgramData\CampusLogin\results\<纯文件名>.json（HelperResult）
                                   ▼
        主进程读取结果文件、logs 并入主进程日志、删除结果文件
```

## Data Flow

1. **发起**：调用方生成唯一结果名（同一进程内多次提权需互不覆盖），经提权通道启动本 exe；或把 `{op, args, result}` 写成 `requests/req-*.json` 交给计划任务代理。结果名必须是纯文件名，否则后面 `resolve_result_path` 会拒绝 (`helper/mod.rs:88-100`)。
2. **命令行形态解析**：`parse_helper_args` 找到 `--helper` 后的 op 名，把 `--result <名>` 抽出，其余参数交给 `build_op_from_args`；未命中 `--helper` 返回 `Ok(None)`（正常应用启动），命中但非法返回 `Err` (`helper/mod.rs:260-289`)。
3. **worker 形态解析**：`run_helper_task` 循环 `oldest_request` 取最旧 `req-*.json`；`process_request` 读入内存后**立即删文件** (`helper/mod.rs:406`)，再反序列化为 `TaskRequest`，`build_op_from_args(&req.op, &req.args)` 与 `resolve_result_path(&req.result)` 并行校验 (`helper/mod.rs:414`)。
4. **执行**：按 op 分发到对应 `run_*`，内部调用 Win32 API（`SetIpInterfaceEntry`、`CM_Reenumerate_DevNode`）、`netsh`/`pnputil`（经 `crate::network::discovery::new_command`，参数不拼 shell），或 `crate::network` / `crate::platform` 的既有实现（如 `run_register_boot_task` 的 `core_autostart` 与 `task_proxy` 调用，`helper/mod.rs:1117-1199`）；诊断信息累积进 `logs`。
5. **结果回写**：`HelperResult` 序列化为 JSON，先写 `results\<name>.json.tmp` 再 `rename` 到 `results\<name>.json` (`helper/mod.rs:1235-1245`)；`logs` 为空、`details` 为 `None` 时对应键不出现 (`helper/mod.rs:107-111`)。
6. **主进程读取**：主进程轮询结果文件，把 `logs` 并入主进程日志并删除结果文件（该读取逻辑在发起侧，本文不展开）。
7. **退出码**：`run_helper` 与 `process_request` 均按 `result.success` 返回 0/1 (`helper/mod.rs:330`、`helper/mod.rs:460`)；worker 任一条失败即整体置 1 (`helper/mod.rs:359-361`)。

## Connections

- [[platform-com-elevation-undocumented]]：CMSTPLUA 静默提权与虚表布局等未公开细节（`RegisterTaskProxy` 与提权副本由该通道拉起）。
- [[task-proxy-registration]]：计划任务代理的注册（`register_task_via_com`）、探测与 `run_via_task` 触发。
- [[core-autostart]]：无头核心开机任务的配置解码、目录/路径解析与 ACL 边界（`run_register_boot_task` / `run_remove_boot_task` 调用）。
- [[single-notification-channel]]：提权失败/降级的用户可见反馈通道。
- [[helper-spawn]]：发起侧通道选择、命令行拼装与结果轮询（`platform/helper_spawn.rs`）。
- [[devnode]]：`devnode_problem`、`verify_window_ms`、`disable_adapter_nonpersistent`、`reenumerate_device_raw` 的 PnP 实现。

## Known Issues

1. **结果写入错误被全部忽略**：`write_result_file` (`helper/mod.rs:1235-1245`) 忽略 `create_dir_all`、`write`、`rename` 的全部 IO 错误，失败时函数静默返回；主进程只能等满超时才失败，无法立即感知原因。
2. **helper 进程无自身执行超时**：`run_helper` 与 `run_helper_task` 都没有执行超时，单个 op 卡住（如 `netsh` 长时间不返回）只能靠主进程侧超时兜底；worker 的空扫 3 轮 (`helper/mod.rs:354-371`) 只保证**无请求时**退出，不约束正在执行的 op。
3. **MAC 伪装值清除失败仍报成功**：`run_mac` 在 `remove_mac_from_registry` 失败时只 push 一条日志 `"helper: 清除MAC注册表伪装值失败: {e}（重启后将维持伪装MAC）"` (`helper/mod.rs:585-587`)，随后仍构造 `success: true` (`helper/mod.rs:588-594`)，重启后伪装 MAC 会持续生效。
4. **`--result` 名字非法时静默丢结果**：`run_helper` 遇到非法结果名只 `eprintln!` (`helper/mod.rs:324-329`)，退出码仍由 op 成败决定；主进程会一直轮询到超时，把「写不进去」误判成「操作超时」。
5. **worker 无回执的失败分支**：请求 JSON 非法时 `process_request` 直接返回 1 (`helper/mod.rs:407-413`)，结果名非法时 `if let Ok(result_path)` 分支不成立 (`helper/mod.rs:457-459`)——两种情况下请求文件已被删除、却没有任何结果文件，请求方只能靠轮询超时感知。
6. **`run_helper_task` 退出码语义粗糙**：任一请求失败即把整体退出码置 1 (`helper/mod.rs:359-361`)，无法从退出码区分是哪个请求失败，只能翻结果文件。
7. **`SelfCheck` 只证明写通道可用**：该 op 不做任何权限验证（`run_helper` `:316-322`、`process_request` `:439-445` 直接构造 `success: true`），它能确认「结果目录可写、worker 可被拉起」，但不保证具体 op 所需的 Win32 权限齐备。
8. **适配器枚举失败被误报为「不存在」**：`run_enable_adapter`/`run_disable_adapter` 用 `get_adapters_force().map(...).unwrap_or(false)` (`helper/mod.rs:742-744`、`helper/mod.rs:803-805`)，枚举本身失败与「名字不在列表」都落到同一条 `"未找到本机适配器: {name}"` 文案，排障时无法区分权限问题与拼写问题。
9. **`run_clear_dns` 的适配器枚举失败被吞**：`get_adapters_force().unwrap_or_default()` (`helper/mod.rs:505`) 让枚举失败退化为空列表，此时适配器名回退为 GUID（`helper/mod.rs:510-514`），日志可读性下降但不影响按 GUID 清除。
10. **COM/runas 路径无重入保护**：计划任务代理侧靠 `MultipleInstances=IgnoreNew` 串行化 (`helper/mod.rs:347-349`)，而提权副本通道可被并发触发多个实例；结果文件名唯一只避免结果文件互相覆盖，操作本身（如同时改同一张网卡的 MAC）没有互斥。
