---
title: 提权辅助子进程 helper
type: module
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
tags: [提权, helper, windows, dns, mac]
---

## Overview

本模块解决一个权限落差问题：Tauri 主进程以普通用户运行，而 Windows 上设置 DNS+DoH、改写 MAC、启停适配器、精确删除路由、设置接口跃点、PnP 设备级启用/重枚举、注册计划任务等操作都要求管理员或 SYSTEM 权限。做法是**让进程以提权身份重启自己**（`--helper <op> ... --result <文件名>`），或由 SYSTEM 计划任务拉起自己（`--helper-task`），在提权上下文内执行单个 op，把结果写进固定目录下的 JSON 文件，主进程轮询取回。

三个文件分三层：

| 文件 | 角色 | 关键入口 |
| --- | --- | --- |
| `tauri-app/src-tauri/src/helper/mod.rs` | 被提权一侧（服务端）：op 定义、参数解析、请求扫描、结果回写 | `parse_helper_args` (`helper/mod.rs:242-271`)、`run_helper` (`helper/mod.rs:274-309`)、`run_helper_task` (`helper/mod.rs:328-351`) |
| `tauri-app/src-tauri/src/platform/helper_spawn.rs` | 发起侧（客户端）：通道选择、命令行拼装、结果轮询解析 | 见源码 |
| `tauri-app/src-tauri/src/platform/elevation.rs` | 提权原语：管理员判定、CMSTPLUA 静默提权、`runas` 兜底 | 见源码 |

本模块固化的六条设计决定：

1. **结果收口**：结果一律写进 `proxy_base_dir()/results` 下的**纯文件名**，绝不接受请求方给的任意路径（P0-2）。`resolve_result_path` (`helper/mod.rs:88-100`) 只放行 ASCII 字母数字与 `-_`，拒绝 `/`、`\`、`..`、`.` 开头、空串与超过 128 字符的名字，杜绝「SYSTEM 写任意路径」原语（如覆盖 hosts 或系统 DLL）。
2. **三级降级链**：计划任务代理（全程无 UAC）→ CMSTPLUA COM 静默提权 → `ShellExecuteW runas`（弹 UAC）。见 `spawn_elevated_helper`。
3. **原子写**：`write_result_file` (`helper/mod.rs:1083-1093`) 先写 `<name>.json.tmp` 再 `rename`，主进程不会读到半截 JSON；结果目录首次可能不存在，写前 `create_dir_all`（`helper/mod.rs:1084-1086`）。
4. **helper 不初始化 logger**：文件头注释 (`helper/mod.rs:12-13`) 明确说明，避免与主进程跨进程争抢同一日志文件。诊断信息收进 `HelperResult::logs`，由主进程读取后逐条落盘后删除结果文件。
5. **提权侧对入参零信任**：worker/提权副本以 SYSTEM 或管理员身份运行，而请求文件落在 ProgramData 下任何本地用户可写的位置（`helper/mod.rs:582-583` 注释），因此每个 op 都在交给 Win32 或外部命令前做白名单/字符集/存在性校验（`valid_instance_id` (`helper/mod.rs:950-957`)、`decode_set_metric_row` (`helper/mod.rs:584-614`)、`decode_route_row` (`helper/mod.rs:853-872`)、适配器名双校验）。
6. **`route_add` 已废弃**：`HelperOp` 中不再有添加路由的变体，只保留精确删除（`RouteDelete`，`helper/mod.rs:135-136`）。原因是 2026-10-18 事故——同网关+接口的默认路由无法与目标卡自己的 DHCP 默认路由共存（Windows 判重复），兜底路由这一设计已废弃。

## Key Components

### `tauri-app/src-tauri/src/helper/mod.rs`（被提权侧）

**公开函数：**

- `proxy_base_dir() -> PathBuf` (`helper/mod.rs:31-50`)：代理根目录，请求/结果/自检标记的固定位置。Windows 上优先 `SHGetKnownFolderPath(FOLDERID_ProgramData)`（系统权威来源，不受进程环境变量影响），失败或结果不合法时回退 `PROGRAMDATA` 环境变量，再不行硬编码 `C:\ProgramData`，最后 `.join("CampusLogin")`；非 Windows 用 `std::env::temp_dir().join("campus-login-proxy")` (`helper/mod.rs:46-49`)。
- `helper_results_dir() -> PathBuf` (`helper/mod.rs:75-77`)：`proxy_base_dir().join("results")`，worker 以 SYSTEM 身份只在此目录内写结果。
- `helper_requests_dir() -> PathBuf` (`helper/mod.rs:80-82`)：`proxy_base_dir().join("requests")`，计划任务 worker 扫描此目录。
- `resolve_result_path(name: &str) -> Result<PathBuf, String>` (`helper/mod.rs:88-100`)：校验结果文件名并拼到固定结果目录，见 Overview 第 1 条。
- `parse_helper_args(args: &[String]) -> Result<Option<(HelperOp, Option<String>)>, String>` (`helper/mod.rs:242-271`)：解析 `--helper <op> [op参数...] [--family <v>] --result <文件名>`。未命中 `--helper` 返回 `Ok(None)`；命中但参数非法返回 `Err`（调用方不应继续启动正常应用）。op 之后的参数全部交给 `build_op_from_args`，`--result` 单独抽出。
- `run_helper(op: HelperOp, result_path: Option<String>) -> i32` (`helper/mod.rs:274-309`)：执行单个 op 并返回退出码（成功 0 / 失败 1）。按 op 分发（`helper/mod.rs:276-301`），有 `result_path` 时经 `resolve_result_path` 收口后 `write_result_file`；名字非法时只 `eprintln!("helper 结果路径非法: {e}")` (`helper/mod.rs:305`)，退出码仍由 op 结果决定。
- `run_helper_task() -> i32` (`helper/mod.rs:328-351`)：计划任务 worker 入口（`main.rs` 拦截 `--helper-task` 后调用）。循环取最旧请求 → `process_request`；空扫累计 3 轮（每轮 `sleep(400ms)`，`helper/mod.rs:346`）后退出。多扫两轮是为兜住 `MultipleInstances=IgnoreNew` 下「第二个请求在首个实例运行期落盘」的竞态 (`helper/mod.rs:325-327`)。任一请求失败即把退出码置 1（`helper/mod.rs:337-339`）。

**数据结构：**

`HelperResult` (`helper/mod.rs:102-112`)：提权操作的统一回执，序列化为 JSON。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `success` | `bool` (`:104`) | 是否成功，决定进程退出码 |
| `message` | `String` (`:105`) | 摘要消息，直接透传给前端 |
| `op` | `String` (`:106`) | 操作类型名 |
| `logs` | `Vec<String>` (`:108`) | 诊断日志；`#[serde(skip_serializing_if = "Vec::is_empty")]`，空时不出现该键 |
| `details` | `Option<serde_json::Value>` (`:111`) | 操作完整明细（如 DNS 的 `dnsSuccess`/`dnsFailed`/`dohAdded`、`clear_dns` 的 `restored`/`failed`）；`skip_serializing_if = "Option::is_none"` |

`HelperOp` (`helper/mod.rs:115-156`)：op 枚举，`#[cfg(target_os = "windows")]` 变体在非 Windows 构建中不存在。

| 变体 | 行号 | 语义 |
| --- | --- | --- |
| `Dns { targets: Vec<String>, family: String }` | `:119` | 设置 DNS + 启用 DoH，只处理 `targets` 名单内适配器；`family` 为 "ipv4"/"ipv6"/"both" |
| `ClearDns { targets: Vec<String> }` | `:122` | 恢复 DNS 自动获取，`targets` 是适配器 GUID 名单 |
| `Mac { guid: String, mac_no_dash: String }` | `:124` | 改 MAC（注册表 `NetworkAddress` + 重启网卡） |
| `EnableAdapter { name: String }` | `:127` | `netsh interface set interface enable` |
| `DisableAdapter { name: String }` | `:130` | 禁用适配器（夜间切换临时下线校园网卡） |
| `RouteDelete { rows: Vec<String> }` | `:136` | 精确删除历史兜底默认路由，`rows` 为 `"{dest}\|{mask}\|{gateway}\|{metric}\|{if_index}"` |
| `EnableDevice { instance_id: String }` | `:139` | `pnputil /enable-device`，含 problem 22 解除复核 |
| `ReenumerateDevice { instance_id: String }` | `:144` | `CM_Reenumerate_DevNode` 总线重扫，把幽灵态 USB 设备拉回 PnP 树 |
| `SetMetric { rows: Vec<String> }` | `:150` | 设置接口跃点，`rows` 为 `"{guid}:{family}:{automatic}:{metric}"` |
| `RegisterTaskProxy` | `:153` | 注册计划任务提权代理（本身须在提权上下文执行） |
| `SelfCheck` | `:155` | 通道自检，写结果即通过 |

`TaskRequest` (`helper/mod.rs:314-321`)：worker 与主进程共享的请求文件协议（`serde::Deserialize`）。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `op` | `String` (`:316`) | op 名，交给 `build_op_from_args` |
| `args` | `Vec<String>` (`:318`) | `#[serde(default)]`，缺省为空数组，因此 `selfcheck` 这类无参 op 的请求可省略该键 |
| `result` | `String` (`:320`) | 结果文件名（纯文件名，经 `resolve_result_path` 收口） |

**私有函数：**

- `validate_programdata_path(p: &Path) -> bool` (`helper/mod.rs:19-28`)：审计 A11。要求绝对路径、非空、首个 component 为盘符前缀（`C:\`）或根（`\\server\share`）——`SHGetKnownFolderPath` 与环境变量都可能被劫持/损坏，写入前提是路径可信。
- `known_folder_programdata() -> Option<PathBuf>` (`helper/mod.rs:55-72`)：调 `SHGetKnownFolderPath(FOLDERID_ProgramData)`，成功后 `CoTaskMemFree` 释放，不新增依赖（复用已有 `Win32_UI_Shell` feature）。
- `build_op_from_args(op: &str, args: &[String]) -> Result<HelperOp, String>` (`helper/mod.rs:161-238`)：命令行 `--helper` 与计划任务请求 JSON 共用的构造器。扫描时把 `--family <v>` 收进开关、跳过其余 `--` 开头参数、其余入 `positional` (`helper/mod.rs:166-181`)；随后按 op 名匹配 (`helper/mod.rs:182-236`)，缺参数与未知 op 一律 `Err`（如 `"helper mac 缺少 GUID"`、`"未知 helper 操作: {other}"`）。
- `oldest_request(dir: &Path) -> Option<PathBuf>` (`helper/mod.rs:354-370`)：取请求目录中最旧的 `req-*.json`（按 `modified()` 升序）。
- `process_request(path: &Path) -> i32` (`helper/mod.rs:374-435`)：处理单个请求。**读入内存后立即删请求文件** (`helper/mod.rs:383-384`)，之后的篡改/重建不影响本次执行；`build_op_from_args` 与 `resolve_result_path` 任一失败都产出 `success: false` 的回执 (`helper/mod.rs:423-429`)，最后再解析一次结果路径写回 (`helper/mod.rs:431-433`)。
- `run_dns(targets: &[String], family: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:437-456`)：调 `crate::network::dns_setup::setup_dns_doh_admin(targets, family)`，把返回的整个 JSON 放进 `details`。
- `run_clear_dns(targets: &[String], logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:461-516`)：非 Windows 分支直接返回「仅支持Windows」(`helper/mod.rs:462-472`)；Windows 分支逐 GUID 调 `crate::platform::dns_config::clear_adapter_dns_via_api(guid)`，明细入 `details.restored` / `details.failed`，三态文案（全成功 / 部分失败 / 全失败）与管理员路径 `commands/network_cmd.rs` 的 `reset_dns` 保持一致 (`helper/mod.rs:458-460`)。
- `run_mac(guid: &str, mac_no_dash: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:518-578`)：先校验 MAC 必须 12 位十六进制无分隔符 (`helper/mod.rs:520`)，否则会静默写坏网卡配置；再枚举适配器按 GUID 定位；`apply_mac_change_via_registry` 成功后**在管理员上下文内清除注册表伪装值** (`helper/mod.rs:559-561`)——主进程非提升时对 HKLM Class 键无写权限，仅靠主进程清理会让伪装 MAC 每次重启后持续生效。
- `decode_set_metric_row(row: &str) -> Result<(String, u16, bool, u32), String>` (`helper/mod.rs:584-614`)：解码 `"{guid}:{family}:{automatic}:{metric}"`，字段数必须为 4，`family` 只接受 `2`/`23`，`automatic` 只接受 `0`/`1`，`metric` 必须能解析为 `u32`。抽成纯函数是为了可单测。
- `run_set_metric(rows: &[String], logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:620-701`)：空条目直接失败（报成功会让调用方误判切换已生效，`helper/mod.rs:631-634`）；逐条解码 → `interface_rows_for_guid` 枚举 → 按 `Family` 找当前行 → 改副本（`UseAutomaticMetric`、`Metric`、`SitePrefixLength = 0`，后者不置 0 会报 `ERROR_INVALID_PARAMETER`，`helper/mod.rs:667-668`）→ `SetIpInterfaceEntry`。任一条失败即整体失败，明细汇总进 `message`。
- `run_enable_adapter(name: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:706-756`)：双校验——`validate_adapter_name` 字符集 (`helper/mod.rs:707`) + 必须存在于 `get_adapters_force()` 列表 (`helper/mod.rs:716-718`)，防参数注入；随后 `netsh interface set interface <name> enable`，失败时用 `decode_console_bytes` 解 stderr 给出可读原因。
- `run_disable_adapter(name: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:767-847`)：双校验同上。优先**非持久化设备级禁用** `disable_adapter_nonpersistent(name)` (`helper/mod.rs:790`)：`netsh disable` 的禁用态会跨重启残留，重启后物理网卡停在 PnP problem 22，USB 卡（RTL8156 等）还会停在出厂 CD-ROM 模式并弹「正在安装设备驱动程序」(`helper/mod.rs:761-765`)。只有没有 PnP 实例 ID 的虚拟卡（Wintun/TAP）才回退 `netsh` (`helper/mod.rs:802-805`)；物理卡设备级禁用失败时**不静默回退 netsh**，宁可报错交上层重试 (`helper/mod.rs:806-816`)。
- `decode_route_row(row: &str) -> Result<(Ipv4Addr, Ipv4Addr, Ipv4Addr, u32, u32), String>` (`helper/mod.rs:853-872`)：解码 `"{dest}|{mask}|{gateway}|{metric}|{if_index}"`，字段数必须为 5，三个地址走 `std::net::Ipv4Addr` 解析、`metric`/`if_index` 必须纯数字——注入尝试（空格、斜杠、`-flag`）全部挡在解析层。
- `run_route_delete(rows: &[String], logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:882-944`)：先整体解码，任一条非法即整体失败 (`helper/mod.rs:894-899`)；再逐条 `mask_to_prefix_len` → `delete_route_exact(dest, prefix_len, gateway, if_index, metric)`。不用 `route.exe delete` 的原因写在 `helper/mod.rs:874-880`：它只按目的+掩码+网关+接口匹配、跃点不参与，会把目标卡自己的 DHCP 默认路由一起删掉（2026-10-18 事故：WLAN 失去默认路由只能重连恢复）。`DeleteExact::AlreadyGone` 记日志但不算失败。
- `valid_instance_id(instance_id: &str) -> bool` (`helper/mod.rs:950-957`)：设备实例 ID 字符集白名单（形如 `USB\VID_0BDA&PID_8156\4013000001`），非空、≤200 字符、不以 `-` 开头、只允许 ASCII 字母数字与 `\ & . _ - *`，拒绝 `/`（pnputil 开关前缀）、空格与控制字符。设备级启用与总线重枚举共用。
- `run_enable_device(instance_id: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:963-1025`)：字符集白名单 (`helper/mod.rs:964`) + `devnode_problem` 存在性复核 (`helper/mod.rs:973`) 双校验，再 `pnputil /enable-device`。**不信 pnputil 返回值**：轮询 `devnode_problem` 直到脱离 problem 22，窗口取 `devnode::verify_window_ms(instance_id)`（与主进程同源，USB 设备等价物理拔插，3s 必然误判，`helper/mod.rs:995-1000`），超时则报「pnputil 已执行但设备仍处于 PnP 禁用态」。
- `run_reenumerate_device(instance_id: &str, logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:1030-1057`)：同样先过 `valid_instance_id`，再调 `devnode::reenumerate_device_raw(instance_id)`（SYSTEM worker 内 `CM_Reenumerate_DevNode(SYNCHRONOUS)`）。存在的理由见 `helper/mod.rs:140-143`：USB 网卡禁用后端口复位会让设备实例在 PnP 树里消失（幽灵态），此时 `pnputil /enable-device` 与 `netsh` 都无从下手，必须先重扫总线拉回设备。
- `run_register_task(logs: &mut Vec<String>) -> HelperResult` (`helper/mod.rs:1061-1079`)：调 `crate::platform::task_proxy::register_task_via_com()`，仅计划任务/提权副本上下文内调用（COM 注册需要管理员）。
- `write_result_file(path: &Path, result: &HelperResult)` (`helper/mod.rs:1083-1093`)：原子写，见 Overview 第 3 条。

**测试**（`helper/mod.rs:1095-1427`，`mod tests`）：

| 测试 | 行号 | 覆盖点 |
| --- | --- | --- |
| `validate_programdata_path_rules` | `:1100-1108` | A11 路径合法性（`C:\ProgramData`、`D:\` 通过；相对路径、`..\evil`、空串拒绝） |
| `parse_no_helper_returns_none` | `:1110-1114` | 未命中 helper 模式返回 `Ok(None)` |
| `parse_dns_with_result` / `parse_dns_with_targets_and_family` | `:1116-1150` | 位置参数 + `--family` 扫描、`--result` 抽取 |
| `parse_clear_dns_with_targets` | `:1152-1173` | GUID 名单位置参数 |
| `parse_mac` / `parse_mac_missing_args_is_err` / `parse_unknown_op_is_err` | `:1175-1203` | mac 参数顺序与缺参、未知 op 报错 |
| `helper_result_roundtrip` / `helper_result_omits_empty_logs_and_details` | `:1205-1234` | JSON 字段名与空 `logs`/`details` 的省略 |
| `resolve_result_path_accepts_plain_name` / `resolve_result_path_rejects_traversal_and_absolute` | `:1238-1258` | P0-2 收口：纯名通过；绝对路径、`..\..\`、`sub/`、`sub\`、`..`、`.hidden.json`、空串全部拒绝 |
| `build_op_enable_adapter` / `build_op_enable_device` / `build_op_reenumerate_device` / `build_op_selfcheck_and_register_task` / `build_op_unknown_is_err` | `:1262-1320` | `build_op_from_args` 各分支 |
| `instance_id_whitelist_rejects_injection` | `:1298-1306` | 防 `pnputil` 开关注入：`/remove-device`、空格、`--remove-device`、空串拒绝 |
| `set_metric_round_trip_and_reject` | `:1324-1372` | 编码条目 round-trip；字段数、空 GUID、`family=6`、`automatic=true`、`-1`、`abc`、越界 u32 全部拒绝 |
| `task_request_json_roundtrip` | `:1374-1387` | 请求 JSON 反序列化与缺省 `args` |
| `decode_route_row_accepts_valid_and_rejects_injection` | `:1389-1426` | 合法五元组通过；字段数错误、越界地址、非点分、`-flag`、`/remove`、非数字跃点全部拒绝 |

### `tauri-app/src-tauri/src/platform/helper_spawn.rs`（发起侧）

**公开函数：**

- `new_result_name() -> String`：生成 `r-<pid>-<ts>.json`，`ts` 取 `SystemTime::now().duration_since(UNIX_EPOCH)` 的纳秒值，用于同一进程内多次提权的隔离。
- `spawn_elevated_helper(op: &str, args: &[&str], result_name: &str, timeout: Duration, allow_uac_prompt: bool) -> Result<serde_json::Value, String>`：通道选择器。`proxy_usable()` 为真 → `task_proxy::run_via_task(op, args, timeout)`，失败则 `invalidate_proxy_cache()` 并降级；代理不可用但注册态非 `Disabled` → `ensure_registered()` 成功后同样走代理，注册失败或触发失败均降级；最终一律落 `spawn_elevated_raw`。
- `spawn_elevated_raw(op: &str, args: &[&str], result_name: &str, timeout: Duration, allow_uac_prompt: bool) -> Result<serde_json::Value, String>`：绕过代理的提权链。拼 `--helper <op>` + 每个参数加双引号（适配器名可含空格如「以太网 2」，不加引号会被拆碎）+ ` --result "<result_name>"`；先 `shell_exec_elevated(..., true)`（COM 静默），失败且 `allow_uac_prompt=false` 时直接返回 `"COM静默提权失败(首试不弹UAC，重试将降级): {com_err}"`，否则 `run_elevated` 弹 UAC；随后每 100ms 轮询 `resolve_result_path(result_name)` 直到 `timeout`，**超时后还追加一次检查**——UAC 等待/DHCP 慢路径可能恰好越过 deadline 才写完，此时操作实际已成功，按正常结果返回而非误报超时。

**私有函数：**

- `read_helper_result(content: &str, result_path: &Path) -> Result<serde_json::Value, String>`：解析结果 JSON；把 `logs` 数组逐条并入 `crate::log_info!("helper", "{s}")`；删除结果文件后返回 JSON。解析失败返回 `"解析helper结果文件失败: {e}"`。

### `tauri-app/src-tauri/src/platform/elevation.rs`（提权原语）

**公开函数：**

- `is_admin() -> bool`：`OpenProcessToken` + `GetTokenInformation(TokenElevation)`，`elevation != 0` 即管理员；句柄用 `CloseHandle` 释放。
- `run_elevated(cmd: &str, args: &str) -> Result<(), String>`：`ShellExecuteW` + `"runas"` verb + `SW_HIDE`；返回值 ≤32 视为失败并报 `"ShellExecuteW runas 失败，错误码: {val}"`。
- `shell_exec_elevated(file: &str, params: &str, hide_window: bool) -> Result<(), String>`：CMSTPLUA 静默提权。`CoInitializeEx(COINIT_APARTMENTTHREADED)` 成功（S_OK/S_FALSE）时每个退出路径都配对 `CoUninitialize`，失败（如 `RPC_E_CHANGED_MODE`）不释放；moniker 为 `Elevation:Administrator!new:{3E5FC7F9-9A51-4367-9063-A120244FBEC7}`，IID 为 `{6EDD6D74-C007-4E75-B76A-E5740995E24C}`；**必须传 `BIND_OPTS3`（x64 48 字节）且 `dwClassContext = CLSCTX_LOCAL_SERVER`**——历史缺陷是传了 16 字节的 `BIND_OPTS`，`cbStruct` 过小被 appinfo 按「提升激活配置无效」拒绝（`0x80080017 CO_E_ELEVATION_DISABLED`），静默提权从未真正通过过，此前被手动路径的 runas 降级掩盖。
- `parse_guid(s: &str) -> Result<windows::core::GUID, String>`：去掉 `{}` 后按 `-` 切 5 段，前 3 段分别按 u32/u16/u16 十六进制解析，后两段拼接必须 16 字符再逐字节解析；各段失败给出 `GUID dataN解析失败: {e}`。
- `co_get_object_raw(pszname, pbindoptions, riid, ppv) -> i32`：`#[allow(non_snake_case)]` 的 `ole32` `CoGetObject` 外部函数封装（`#[link(name = "ole32")]`）。

**内部结构：**

`ICMLuaUtilVtbl`：`#[repr(C)]` 手工虚表布局。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `_query_interface` | `usize` | slot 0，仅占位 |
| `_add_ref` | `usize` | slot 1，仅占位 |
| `release` | `unsafe extern "system" fn(*mut c_void) -> u32` | slot 2，真实调用 |
| `_slots_3_to_8` | `[usize; 6]` | slot 3~8：`SetRasCredentials`/`SetRasEntryProperties`/`DeleteRasEntry`/`LaunchInfSection`/`LaunchInfSectionEx`/`CreateLayerDirectory`，不调用，仅占位对齐 |
| `shell_exec` | `unsafe extern "system" fn(*mut c_void, *const u16, *const u16, *const u16, u32, u32) -> HRESULT` | slot 9 |

**测试**：

- `vtbl_shell_exec_slot_is_9`：用 `offset_of!(ICMLuaUtilVtbl, shell_exec) / size_of::<usize>()` 断言 `shell_exec` 位于 slot 9。历史缺陷是它被错排到 slot 4（实为 `SetRasEntryProperties`），参数 marshaling 不匹配报 `RPC_X_BAD_STUB_DATA` (`0x800706F4`)。
- `shell_exec_elevated_smoke`：`#[ignore = "真实提权冒烟测试，仅本机手动执行"]`，跑 `cmd.exe /c exit`（无副作用）验证 BIND_OPTS3 修复，手动执行方式 `cargo test shell_exec_elevated_smoke -- --ignored --nocapture`。

## Architecture

```text
┌─────────────────── 主进程（普通用户） ────────────────────┐
│ helper_spawn::spawn_elevated_helper                      │
│   ├─ proxy_usable() ► task_proxy::run_via_task  （无 UAC）  │
│   ├─ ensure_registered() ► run_via_task        （先注册）  │
│   └─ spawn_elevated_raw                               │
│        ├─ elevation::shell_exec_elevated ► CMSTPLUA 静默    │
│        └─ elevation::run_elevated ► ShellExecuteW runas     │
│   轮询 resolve_result_path(name) ► read_helper_result       │
└────────────────────────────┬───────────────────────────────┘
                             ▼  --helper <op> … --result <name>
┌────────────── 提权副本 / SYSTEM worker ────────────────────┐
│ parse_helper_args ► build_op_from_args (helper/mod.rs:161) │
│   run_helper (helper/mod.rs:274) │ run_helper_task (:328)   │
│   ► run_dns / run_mac / run_route_delete / run_set_metric … │
│   ► write_result_file (helper/mod.rs:1083) 原子写 results\ │
└────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **发起**：调用方（命令层）取 `new_result_name()` 拿到唯一结果名，调 `spawn_elevated_helper(op, args, result_name, timeout, allow_uac_prompt)`。
2. **通道选择**：`proxy_usable()` 为真直接走计划任务代理；否则若注册态非 `Disabled` 先 `ensure_registered()`（内部经 CMSTPLUA/runas 提权一次）再走代理；两者都不成则 `spawn_elevated_raw` 走 COM 静默 → `runas`。`allow_uac_prompt=false`（自动启用首试）时禁止 `runas` 降级，失败即 `Err` 交调用方退避重试——代理通道本身从不弹 UAC。
3. **子进程启动**：exe 以管理员/SYSTEM 身份启动，命令行形如 `--helper set_metric "{GUID}:2:0:1" --result "r-1234-1695.json"`；代理通道则把 `{op, args, result}` 写成 `requests/req-*.json`，SYSTEM 任务拉起 `--helper-task`。
4. **执行**：`parse_helper_args`（或 worker 的 `process_request`）→ `build_op_from_args` → 对应 `run_*`，内部调用 Win32 API、`netsh`/`pnputil`（经 `crate::network::discovery::new_command`，参数不拼 shell）或 `crate::network` / `crate::platform` 的既有实现。worker 在读完请求文件后立即删除它 (`helper/mod.rs:383-384`)。
5. **结果回写**：`write_result_file` 把 `HelperResult` 序列化为 JSON，先写 `.json.tmp` 再 rename 到 `ProgramData\CampusLogin\results\<name>`；`logs` 为空、`details` 为 `None` 时对应键不出现。
6. **主进程读取**：`spawn_elevated_raw` 每 100ms 读结果文件，命中即 `read_helper_result` —— 把 `logs` 并入主进程日志、删除结果文件、返回 JSON；超时后仍追加一次检查，彻底没有才报 `"提权操作超时，未收到helper结果"`。

## Connections

- [[platform-com-elevation-undocumented]]：CMSTPLUA moniker、`BIND_OPTS3` 与虚表 slot 布局等未公开细节。
- [[task-proxy-registration]]：计划任务代理的注册、探测（`RegistrationProbe`）与 `run_via_task` 触发。
- [[single-notification-channel]]：提权失败/降级的用户可见反馈通道。

## Known Issues

1. **结果写入错误被忽略**：`write_result_file` (`helper/mod.rs:1083-1093`) 忽略 `create_dir_all`、`write`、`rename` 的全部 IO 错误。写失败时主进程只能等满 `timeout` 才失败，无法立即感知原因。
2. **helper 进程无自身超时**：`run_helper` 与 `run_helper_task` 都没有执行超时，单个 op 卡住（如 `netsh` 长时间不返回）只能靠主进程轮询超时兜底；worker 的空扫 3 轮（`helper/mod.rs:341-347`）只保证**无请求时**退出，不约束正在执行的 op。
3. **COM/runas 路径无重入保护**：计划任务代理有 `MultipleInstances=IgnoreNew`，但 `spawn_elevated_raw` 的 COM/runas 通道可被并发触发多个提权副本；结果文件名含 pid + 纳秒（`helper_spawn.rs:21-28`）避免了结果文件互相覆盖，但操作本身（如同时改同一张网卡的 MAC）没有互斥。
4. **MAC 伪装值清除失败仍报成功**：`run_mac` 在 `remove_mac_from_registry` 失败时只 push 一条日志 `"helper: 清除MAC注册表伪装值失败: {e}（重启后将维持伪装MAC）"` (`helper/mod.rs:559-561`)，`HelperResult.success` 仍为 `true`，重启后伪装 MAC 会持续生效。
5. **`--result` 名字非法时静默丢结果**：`run_helper` 遇到非法结果名只 `eprintln!` (`helper/mod.rs:302-306`)，进程退出码仍由 op 成败决定；主进程会一直轮询到超时，把「写不进去」误判成「操作超时」。
6. **`run_helper_task` 退出码语义粗糙**：任一请求失败即把整体退出码置 1 (`helper/mod.rs:337-339`)，无法从退出码区分是哪个请求失败，只能翻结果文件。
7. **`SelfCheck` 只证明写通道可用**：该 op 不做任何权限验证（`helper/mod.rs:294-300`、`helper/mod.rs:413-419` 直接构造 `success: true`），它能确认「结果目录可写、worker 可被拉起」，但不保证具体 op 所需的 Win32 权限齐备。