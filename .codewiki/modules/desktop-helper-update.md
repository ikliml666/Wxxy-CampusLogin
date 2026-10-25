---
title: 提权辅助子进程（helper）与自动更新（update）
type: module
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/update/mod.rs
  - tauri-app/src-tauri/src/update/updater.rs
tags: [helper, elevation, uac, mac, dns, doh, metric, 跃点, 夜间出站, updater, version-check, sha256, mirrors, tauri-events]
---

## Overview

本模块文档覆盖两块桌面端能力：**helper**（`tauri-app/src-tauri/src/helper/mod.rs`）——以管理员身份重启自身执行“改 MAC”“设 DNS+DoH”“改接口跃点（metric）”“路由清理”“启用/禁用适配器/设备”“注册计划任务代理/无头核心开机任务”等提权操作的子进程入口，负责参数解析、执行、原子落结果文件；**update**（`tauri-app/src-tauri/src/update/mod.rs` + `tauri-app/src-tauri/src/update/updater.rs`）——版本比较、多源 `version.json` 拉取、Release 资产探测、SHA256 多源校验、24h 周期自检循环与临时目录清理。

helper 进程不初始化 logger（避免与主进程跨进程写同一日志文件竞争），诊断信息收集进 `HelperResult::logs`，由主进程读取后统一落日志（`helper/mod.rs:12-13`）。update 模块复用短超时 HTTP 客户端（`update/updater.rs:100`），不接入 `network/client.rs` 的代理池。

本模块全部 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`（测试代码包含在各文件末尾 `#[cfg(test)] mod tests`）。

## Key Components

### `helper/mod.rs`

- `fn validate_programdata_path(p: &std::path::Path) -> bool` — `helper/mod.rs:19-28`。审计 A11：校验 ProgramData 候选路径合法性（绝对、非相对、含盘符/根）。SHGetKnownFolderPath 与环境变量都可能被劫持/损坏，写入前提是路径可信。
- `pub fn proxy_base_dir() -> std::path::PathBuf` — `helper/mod.rs:31-50`。代理根目录：Windows 优先 `SHGetKnownFolderPath(FOLDERID_ProgramData)`（经 `validate_programdata_path` 校验），失败回退 `PROGRAMDATA` 环境变量，再不行硬编码 `C:\ProgramData`，拼接 `"CampusLogin"`；非 Windows 返回 `%TEMP%/campus-login-proxy`。
- `fn known_folder_programdata() -> Option<std::path::PathBuf>` — `helper/mod.rs:55-72`。审计 A11：经 SHGetKnownFolderPath 读取 ProgramData 实际位置（已有 windows Win32_UI_Shell feature，不新增依赖）。
- `pub fn helper_results_dir() -> std::path::PathBuf` — `helper/mod.rs:75-77`。结果文件固定目录：`proxy_base_dir().join("results")`。
- `pub fn helper_requests_dir() -> std::path::PathBuf` — `helper/mod.rs:80-82`。请求文件目录：`proxy_base_dir().join("requests")`。
- `pub fn resolve_result_path(name: &str) -> Result<std::path::PathBuf, String>` — `helper/mod.rs:88-100`。校验结果文件名并拼接到固定结果目录。只接受纯文件名（拒绝路径分隔符、`..`、空串、`.` 开头、非 ASCII 字母数字/`-_."`），杜绝请求方把 SYSTEM 写重定向到任意路径。
- `pub struct HelperResult` — `helper/mod.rs:102-112`，`#[derive(Debug, Clone, Serialize)]`（无 `camelCase` 重命名）。字段：`success: bool`、`message: String`、`op: String`、`logs: Vec<String>`（空时跳过序列化）、`details: Option<serde_json::Value>`（None 时跳过序列化）。
- `pub enum HelperOp` — `helper/mod.rs:115-164`，`#[derive(Debug, Clone, PartialEq)]`，变体：`Dns { targets: Vec<String>, family: String }`、`ClearDns { targets: Vec<String> }`、`Mac { guid: String, mac_no_dash: String }`、`EnableAdapter { name: String }`（仅 Windows）、`DisableAdapter { name: String }`（仅 Windows）、`RouteDelete { rows: Vec<String> }`（仅 Windows，注：2026-10-18 事故后不再提供 route_add）、`EnableDevice { instance_id: String }`（仅 Windows）、`ReenumerateDevice { instance_id: String }`（仅 Windows）、`SetMetric { rows: Vec<String> }`（仅 Windows）、`RegisterTaskProxy`（仅 Windows）、`RegisterBootTask { config_b64: String }`（仅 Windows）、`RemoveBootTask`（仅 Windows）、`SelfCheck`。
- `fn build_op_from_args(op: &str, args: &[String]) -> Result<HelperOp, String>` — `helper/mod.rs:169-256`（私有）。命令行 `--helper` 与计划任务请求 JSON 共用：先扫描出位置参数与 `--family <v>`（`--family` 缺值 → `Err`，默认 `"both"`，`:172-189`），再按 op 名分派（`:190-254`）：`dns` / `clear_dns` / `mac`（位置参数 0=GUID、1=MAC，任一缺失 → `Err`）/ `enable_adapter` / `enable_device` / `reenumerate_device` / `disable_adapter` / `route_delete` / `set_metric` / `register_task` / `register_boot_task`（位置参数 0=config_b64） / `remove_boot_task` / `selfcheck`；其他 → `Err("未知 helper 操作: {other}")`。
- `pub fn parse_helper_args(args: &[String]) -> Result<Option<(HelperOp, Option<String>)>, String>` — `helper/mod.rs:260-289`。语义：未出现 `--helper` → `Ok(None)`；出现但参数非法 → `Err`（调用方不启动正常应用）。扫描 op 之后到 `--result` 之间的参数交 `build_op_from_args`，`--result` 缺值 → `Err`。
- `pub fn run_helper(op: HelperOp, result_path: Option<String>) -> i32` — `helper/mod.rs:292-331`。按变体分派到各 `run_*`（`:294-323`）；`result_path` 为 `Some` 时经 `resolve_result_path` 收口后写结果文件（`:324-329`）；返回退出码 0（成功）/ 1（失败）（`:330`）。
- `struct TaskRequest` — `helper/mod.rs:336-343`（私有，反序列化）。字段：`op: String`、`args: Vec<String>`（默认空）、`result: String`（纯文件名）。
- `pub fn run_helper_task() -> i32` — `helper/mod.rs:350-373`。计划任务 worker（`--helper-task`）：扫 `requests/` 取最旧 `req-*.json` → `process_request`（`:355`）；连续 3 次空扫（间隔 400ms）后退出（`:363-371`）。
- `fn oldest_request(dir: &std::path::Path) -> Option<std::path::PathBuf>` — `helper/mod.rs:376-392`（私有）。取请求目录中最旧的 `req-*.json`（无则 None）。
- `fn process_request(path: &std::path::Path) -> i32` — `helper/mod.rs:396-461`（私有）。读入内存 → 立即删请求文件（防重复执行）→ `build_op_from_args` + `resolve_result_path` → 执行 → 写结果（`:414-459`）。返回该请求的退出码语义（0/1）。
- `fn run_dns(targets: &[String], family: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:463-482`（私有）。写起始日志，调 `network::dns_setup::setup_dns_doh_admin(targets, family)`（`:468`），取返回 JSON 的 `success`/`message`（缺省 `"设置DNS+DoH完成"`），把整个 JSON 塞进 `details`。
- `fn run_clear_dns(targets: &[String], logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:484-542`（私有）。逐 GUID 调 `dns_config::clear_adapter_dns_via_api`，明细进 `details.{restored,failed}`，三态汇总文案（全成功/部分失败/全失败）。
- `fn run_mac(guid: &str, mac_no_dash: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:544-604`（私有）：
  - **MAC 格式前置校验**：必须 `len == 12` 且全部为 ASCII 十六进制字符，否则直接返回 `success:false` + `"MAC 格式非法: ..."`（`:546-554`）——防“非法格式静默写坏网卡配置”；
  - `network::get_adapters_force()` 枚举，按 `guid.eq_ignore_ascii_case` 找适配器（`:556-579`）；
  - `network::dhcp::apply_mac_change_via_registry(guid, &adapter.name, mac_no_dash)`（`:580`）；成功后**在同一管理员上下文内**调 `network::dhcp::remove_mac_from_registry(guid)` 清除 `NetworkAddress` 持久伪装值，失败只 push 一条日志、**不影响 success`（`:585-587`）。
- `fn decode_set_metric_row(row: &str) -> Result<(String, u16, bool, u32), String>` — `helper/mod.rs:610-640`（私有）。把 `"{guid}:{family}:{automatic}:{metric}"` 拆成带花括号 GUID / 协议栈 / 是否自动跃点 / 跃点值，字段格式（字段数、`family` 只认 2 与 23、`automatic` 只认 0 与 1、`metric` 必须 `u32`）一律严格校验后才交给 Win32。
- `fn run_set_metric(rows: &[String], logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:645-727`（私有，仅 Windows）：空 rows 直接失败（`:658-660`）→ 逐条解码 → `platform::metric::interface_rows_for_guid` 取当前行（`:674-682`）→ 按 `Family` 命中该协议栈的行 → 整行改副本（覆写 `UseAutomaticMetric`、`Metric`、`SitePrefixLength = 0`（`:691-694`，非 0 会报 `ERROR_INVALID_PARAMETER`））→ `SetIpInterfaceEntry`（`:695`）。任一条失败即整体 `success:false` 并把失败明细汇总进 `message` 与 `logs`。
- `fn run_enable_adapter(name: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:731-782`（私有，仅 Windows）。适配器名双校验（`validate_adapter_name` + 本机适配器列表存在性）；走 `netsh interface set interface name enable`。
- `fn run_disable_adapter(name: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:793-873`（私有，仅 Windows）。同上，优先走**非持久化设备级禁用**（`CM_Disable_DevNode`），无 PnP 实例 ID 的虚拟卡回退 `netsh disable`，物理卡设备级禁用失败不静默回退。
- `fn decode_route_row(row: &str) -> Result<(std::net::Ipv4Addr, std::net::Ipv4Addr, std::net::Ipv4Addr, u32, u32), String>` — `helper/mod.rs:878-898`（私有，仅 Windows）。解码兜底路由条目 "{dest}|{mask}|{gateway}|{metric}|{if_index}"。三个地址必须是合法 IPv4 点分十进制，metric/if_index 必须是纯数字。
- `fn run_route_delete(rows: &[String], logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:907-970`（私有，仅 Windows）。先全量校验要求 5 段 IPv4 点分十进制 + 数字 metric/if_index 再逐条执行 `delete_route_exact`；删除时若已不在表中视为成功（幂等）。
- `fn valid_instance_id(instance_id: &str) -> bool` — `helper/mod.rs:976-983`（私有，仅 Windows）。设备实例 ID 字符集白名单：拒绝 `/`、空格与控制字符，禁止以 `-` 开头。
- `fn invalid_instance_id_result(...)` — `helper/mod.rs:988-1000`（私有，仅 Windows）。设备实例 ID 白名单校验失败的统一结果构造。
- `fn run_enable_device(instance_id: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:1006-1062`（私有，仅 Windows）。PnP 设备级启用（`pnputil /enable-device`）：字符集白名单 + `CM_Locate_DevNodeW(PHANTOM)` 存在性复核 + problem 22 解除轮询复核。
- `fn run_reenumerate_device(instance_id: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:1067-1088`（私有，仅 Windows）。设备所在总线重枚举（`CM_Reenumerate_DevNode`），把停在幽灵态的 USB 设备重新枚举出来。
- `fn run_register_task(logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:1092-1110`（私有，仅 Windows）。`task_proxy::register_task_via_com`（COM 注册需管理员）。
- `fn run_register_boot_task(config_b64: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:1118-1217`（私有，仅 Windows）。反解校验核心配置 → 写配置文件并收紧 ACL（SYSTEM+Administrators）→ 校验 core exe 存在 → 注册 BOOT 触发 SYSTEM 计划任务；任务注册失败时删除已写入配置。
- `fn run_remove_boot_task(logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:1221-1249`（私有，仅 Windows）。删计划任务 + 删配置文件（密码密文一并清除），幂等。
- `fn write_result_file(path: &std::path::Path, result: &HelperResult)` — `helper/mod.rs:1251-1263`（私有）。先写 `"{path}.json.tmp"` 再 `std::fs::rename` 覆盖（原子写）；两步的返回错误都被忽略（`if ... is_ok()`）。
- `#[cfg(test)] mod tests` — `helper/mod.rs:1265-1597`。用例：`validate_programdata_path_rules`(1272)、`parse_no_helper_returns_none`(1281)、`parse_dns_with_result`(1287)、`parse_dns_with_targets_and_family`(1300)、`parse_clear_dns_with_targets`(1323)、`parse_mac`(1346)、`parse_mac_missing_args_is_err`(1364)、`parse_unknown_op_is_err`(1370)、`helper_result_roundtrip`(1376)、`helper_result_omits_empty_logs_and_details`(1393)、`resolve_result_path_accepts_plain_name`(1409)、`resolve_result_path_rejects_traversal_and_absolute`(1416)、`build_op_enable_adapter`(1433)、`build_op_enable_device`(1439)、`build_op_reenumerate_device`(1454)、`instance_id_whitelist_rejects_injection`(1469)、`build_op_selfcheck_and_register_task`(1479)、`build_op_unknown_is_err`(1488)、`set_metric_round_trip_and_reject`(1495)、`task_request_json_roundtrip`(1545)、`decode_route_row_accepts_valid_and_rejects_injection`(1561)。Windows API 真实调用不在单测覆盖。

### `update/mod.rs`

- `update/mod.rs:1` 仅一行 `pub mod updater;`，无其他内容。

### `update/updater.rs` 常量与静态

| 名称 | 值 | 位置 | 可见性 |
|---|---|---|---|
| `GITHUB_REPO` | `"ikliml666/Wxxy-CampusLogin"` | `update/updater.rs:9` | 私有 |
| `AUTO_CHECK_INTERVAL_SECS` | `86400`（24h） | `update/updater.rs:10` | 私有 |
| `BACKOFF_RETRY_SECS` | `[300, 900, 3600]`（5min/15min/1h） | `update/updater.rs:13` | 私有 |
| `STARTUP_CHECK_DELAY_SECS` | `5` | `update/updater.rs:15` | 私有 |
| `VERSION_FILE_URL` | `https://raw.githubusercontent.com/.../main/version.json` | `update/updater.rs:24` | 私有 |
| `VERSION_MIRRORS` | 3 项：`ghfast.top`、`gh-proxy.com`、`ghproxy.net` 前缀 + 官方 raw URL | `update/updater.rs:27-31` | 私有 |
| `LAST_CHECK_ERROR` | `Mutex<Option<String>>` | `update/updater.rs:21` | 私有 static |
| `LAST_CHECK_TIME_MS` | `AtomicU64` | `update/updater.rs:23` | 私有 static |
| `SHORT_TIMEOUT_CLIENT` | `OnceLock<Result<reqwest::Client, String>>` | `update/updater.rs:100` | 私有 static |
| `UPDATE_TEMP_DIR` | `"campus-login-update"` | `update/updater.rs:254` | 公开常量 |
| `UPDATE_STALE_SECS` | `24 * 3600`（24h） | `update/updater.rs:257` | 私有 |
| `UPDATE_ASSET_EXTS` | `["exe", "msi", "sha256"]` | `update/updater.rs:259` | 私有 |

### `update/updater.rs` 数据结构

- `struct ReleaseAsset` — `update/updater.rs:33-38`：`name: String`、`url: String`、`size: u64`。
- `struct UpdateInfo` — `update/updater.rs:40-56`，`#[serde(rename_all = "camelCase")]`：`has_update: bool`、`latest_version: String`、`release_notes: String`、`assets: Vec<ReleaseAsset>`、`sha256_checksum: Option<String>`、`last_check_error: Option<String>`（默认 None）、`last_check_time: Option<u64>`（默认 None）。
- `struct DownloadProgress` — `update/updater.rs:58-64`：`downloaded: u64`、`total: u64`、`speed: u64`、`percent: f64`。

### `update/updater.rs` 函数

- `pub fn compare_versions(current: &str, latest: &str) -> bool` — `update/updater.rs:66-93`。逐段按 `u32` 比较，`trim_start_matches('v')` 后按 `.` 切分，每段取前导 ASCII 数字；比较长度取两侧最大（`:81`），缺失段按 0；`latest > current` 才返回 true。
- `fn build_short_timeout_http_client() -> Result<&'static reqwest::Client, String>` — `update/updater.rs:102-114`（私有）。`get_or_init` 缓存 `Result`，超时 10 秒；不接入 `network/client.rs` 的 `CLIENT_POOL`。
- `pub fn decide_checksum_missing(all_client_errors: bool, had_transport_error: bool, allow_skip: bool) -> Result<(), String>` — `update/updater.rs:121-135`。`all_client_errors && !had_transport_error` 且 `allow_skip` → `Ok(())` 降级通过；同条件未开跳过 → `Err`；其余（含 5xx / 传输错误）→ `Err`。
- `fn extract_checksum(url: &str, text: &str) -> Option<String>` — `update/updater.rs:142-170`（私有）。URL 含 `api.github.com` 时按 JSON 处理取 `.exe` 资产 `digest`；否则按纯文本取首个空白 token 或 `=` 后 token，统一小写。
- `pub async fn verify_download_sha256(file_path: &str, checksum_urls: &[String], allow_skip_missing: bool) -> Result<bool, String>` — `update/updater.rs:172-251`：空 URL → `Err`；顺序尝试 URL；全部失败调 `decide_checksum_missing`，`Ok(())` 时直接返回 `Ok(true)`；否则流式 64KB 计算 SHA256 比较。
- `pub fn update_temp_dir() -> PathBuf` — `update/updater.rs:261-263`。返回 `%TEMP%/campus-login-update`。
- `pub fn schedule_update_cleanup(app_handle: &AppHandle)` — `update/updater.rs:268-290`。`spawn` 任务：`sleep(24 * 3600)` 后查 `tasks.is_downloading.is_active()`（审计 A13：下载中跳过），否则 `spawn_blocking(remove_dir_all)`。
- `pub fn cleanup_stale_update_dir()` — `update/updater.rs:298-329`。`spawn_blocking` 扫 `update_temp_dir()`，删扩展名属 `exe`/`msi`/`sha256` 且 mtime 超 `UPDATE_STALE_SECS` 的文件；目录不存在视为正常。
- `pub fn now_epoch_ms() -> u64` — `update/updater.rs:331-336`。
- `fn last_check_error_snapshot() -> Option<String>` / `fn last_check_time_snapshot() -> Option<u64>` — `update/updater.rs:338-345`。
- `fn backoff_delay_secs(fail_count: u32) -> u64` — `update/updater.rs:350-356`。0→24h；1..=3→退避档；>3→24h。
- `pub async fn check_and_record(mirror_first: bool) -> Result<UpdateInfo, String>` — `update/updater.rs:361-372`。执行检查并记入 `LAST_CHECK_ERROR`/`LAST_CHECK_TIME_MS`。
- `pub fn start_update_check_loop(app_handle: &AppHandle)` — `update/updater.rs:374-412`。任务名 `update_check_loop`；先 `select!` 等 5s 或 cancel；循环内 `do_update_check` 后按退避等待。注册失败只 `log_warn!`。
- `async fn do_update_check(app_h: &AppHandle, state: &AppState)` — `update/updater.rs:415-446`（私有）。`mirror_first = config.update_source != "github"`；成功后发 `emit_update_available`；有更新且 `update_notified` CAS 成功时弹通知（Windows 走 `show_update_toast` 失败降级普通通知）。失败只 `log_warn!`。
- `pub async fn fetch_latest_release(mirror_first: bool) -> Result<(bool, String, String, Option<String>), String>` — `update/updater.rs:450-476`。按 `mirror_first` 排源顺序，顺序尝试 `fetch_version_from_url`。
- `async fn fetch_version_from_url(url: &str) -> Result<(bool, String, String, Option<String>), String>` — `update/updater.rs:478-517`（私有）。带 `User-Agent` 请求；`current = env!("APP_VERSION")`；`has_update = compare_versions(...)`。
- `pub async fn check_update_inner(mirror_first: bool) -> Result<UpdateInfo, String>` — `update/updater.rs:521-589`。拉取版本 → 构造 exe 名与 URL → HEAD 探测资产存在性（404 则 `has_update=false`）→ 构造 5 个校验和 URL → 组装 `UpdateInfo`（assets 两个条目，`sha256_checksum` 为 URL 数组 JSON 串）。
- `#[cfg(test)] mod tests` — `update/updater.rs:591-739`。用例：`all_4xx_without_explicit_optin_is_hard_error`(596)、`backoff_retry_schedule_steps_then_24h_cycle`(604)、`update_info_serializes_last_check_fields_as_camel_case`(613)、`hotfix_four_segment_version_is_detected`(637)、`version_compare_numeric_and_v_prefix`(644)、`extract_checksum_supports_bsd_style`(651)、`all_4xx_with_explicit_optin_is_allowed`(665)、`server_error_or_transport_error_always_blocks`(671)、`stale_update_asset_classification`(683)、`stale_update_dir_threshold_is_24h`(701)、`extract_checksum_from_github_api_json`(708)、`extract_checksum_api_json_without_exe_asset_is_none`(719)、`extract_checksum_from_sha256_file_text`(728)。

## Architecture

```text
┌──────────────────────────────────────┐   ┌────────────────────────────────────┐
│ helper/mod.rs                       │   │ update/updater.rs                 │
│ parse_helper_args :260              │   │ start_update_check_loop :374      │
│ run_helper :292 (--helper 单发)     │   │ check_update_inner :521           │
│ run_helper_task :350 (--helper-task)│   │ fetch_latest_release :450         │
│ process_request :396               │   │ verify_download_sha256 :172      │
│ run_* ops :463-1249                │   │ schedule_update_cleanup :268      │
│ write_result_file :1251            │   │ cleanup_stale_update_dir :298     │
└───────────────┬────────────────────┘   └───────────────┬────────────────────┘
                │                                        │
                ▼                                        ▼
┌──────────────────────────────────────┐   ┌────────────────────────────────────┐
│ proxy_base_dir (ProgramData/CampusLog│   │ update_temp_dir (%TEMP%/campus-    │
│ in) -> results/ requests/            │   │ login-update)                     │
│ (helper/mod.rs:31)                   │   │ (updater.rs:261)                  │
└──────────────────────────────────────┘   └────────────────────────────────────┘
```

## Data Flow

### helper：提权往返链路（基于 `helper/mod.rs`）

```
主进程经提权启动管理员副本（CMSTPLUA 静默 / runas）
  → 副本入口拦截 --helper （parse_helper_args :260）
     ├─ Ok(None)      → 继续正常应用
     ├─ Ok(Some(op,path)) → std::process::exit(run_helper(op, path))   :292
     └─ Err(e)        → eprintln + exit(2)
  → run_helper :292 按变体分派：
     ├─ Dns → run_dns :463 → dns_setup::setup_dns_doh_admin
     ├─ ClearDns → run_clear_dns :484 → dns_config::clear_adapter_dns_via_api
     ├─ Mac → run_mac :544 → dhcp::apply_mac_change_via_registry + remove_mac_from_registry
     ├─ SetMetric → run_set_metric :645 → interface_rows_for_guid + SetIpInterfaceEntry
     ├─ Enable/DisableAdapter → run_enable_adapter :731 / run_disable_adapter :793
     ├─ RouteDelete → run_route_delete :907 → delete_route_exact
     ├─ Enable/ReenumerateDevice → run_enable_device :1006 / run_reenumerate_device :1067
     ├─ RegisterTaskProxy / BootTask / RemoveBootTask → :1092 / :1118 / :1221
     └─ SelfCheck → 直接成功
  → write_result_file :1251（.tmp → rename 原子替换）
  → 退出码 0（成功）/ 1（失败）

计划任务通道（无 UAC）：
  → 主进程写 requests/req-*.json （TaskRequest :336）
  → schtasks /run 拉起 --helper-task
  → run_helper_task :350 循环 oldest_request :376 → process_request :396（读→删→执行→写）
  → 连续 3 次空扫退出
```

### update：周期自检与清理链路（基于 `update/updater.rs`）

```
start_update_check_loop :374
  → sleep 5s (STARTUP_CHECK_DELAY_SECS :15)
  → loop:
     → do_update_check :415
        → mirror_first = (config.update_source != "github") :416
        → check_update_inner :521
           → fetch_latest_release :450 → fetch_version_from_url :478 (GET version.json, 10s超时)
           → HEAD github_exe_url :535 (404→has_update=false)
           → 构造 5 个 sha256 URL :554
           → 组装 UpdateInfo :567 (assets[0]=exe, assets[1]=exe.sha256, sha256_checksum=URL数组串)
        → emit_update_available + 通知
     → backoff_delay_secs 退避等待 :398
启动期兜底：cleanup_stale_update_dir :298 删 24h 前 exe/msi/sha256 残留
```

### SHA256 多源解析与降级决策

```
校验源顺序（update/updater.rs:554-565）:
  1) https://api.github.com/repos/{GITHUB_REPO}/releases/tags/v{tag}   → JSON digest
  2) {github_exe_url}.sha256                                            → 文本 token
  3) ghfast.top / gh-proxy.com / ghproxy.net 前缀的 (2)
决策（update/updater.rs:121-135）:
  全部 4xx 且无传输错误 + allow_skip=true  → Ok（降级通过）
  全部 4xx 且无传输错误 + allow_skip=false → Err（拒绝安装）
  含 5xx 或传输错误                        → Err（一律拒绝）
```

### 版本比较语义

`compare_versions`（`update/updater.rs:66-93`）要求任意段 `latest > current` 才判有更新：段数不等时缺失段按 0，因此 `2.3.0` → `2.3.0.1` 返回 true；`v` 前缀被剥离；`2.9.0` → `2.10.0` 按数值比较为 true。

### 周期与超时常量

| 名称 | 值 | 位置 |
|---|---|---|
| 启动延迟首查 | `5` s | `update/updater.rs:15` |
| 自检周期 | `86400` s（24h） | `update/updater.rs:10` |
| 更新检查 / 哈希请求超时 | `10` s | `update/updater.rs:100` |
| 临时目录清理延迟 | `24 * 3600` s | `update/updater.rs:274` |
| 残留安装包超龄阈值 | `24 * 3600` s | `update/updater.rs:257` |
| 残留安装包扩展名白名单 | `exe`/`msi`/`sha256` | `update/updater.rs:259` |
| SHA256 流式块大小 | `65536` 字节 | `update/updater.rs:238` |
| helper 结果路径收口目录 | `ProgramData/CampusLogin/results` | `helper/mod.rs:75-77` |

## Connections

- [[desktop-platform]]：`platform/task_proxy.rs`（注册计划任务提权代理、`register_boot_task_via_com`）、`platform/metric.rs`（`interface_rows_for_guid`）、`platform/route_table.rs`（`delete_route_exact`、`mask_to_prefix_len`）、`platform/core_autostart.rs`（核心配置解码与路径）、`platform/toast.rs`（`show_update_toast`）。
- [[desktop-network-dns]]：`helper` 的 DNS 分支直接调 `network::dns_setup::setup_dns_doh_admin` 与 `network::dns_config::clear_adapter_dns_via_api`。
- [[desktop-network-core]]：`helper` 的 MAC 分支调 `network::get_adapters_force`、`network::dhcp::{apply_mac_change_via_registry, remove_mac_from_registry}`；`network::discovery::devnode` 提供设备禁用/重枚举/问题码复核。
- [[desktop-config]]：`update_source`（决定 mirror_first）、`skip_sha256_when_missing`（决定 allow_skip）字段影响 `updater.rs` 行为；`update_stats.update_notified` 参与通知门控。
- [[desktop-infra]]：`infra::state::AppState`、`infra::events::EventBus::emit_update_available`、`infra::task_manager`（任务注册与 cancel token）、`infra::notification::emit_notification`。
- [[desktop-app-lifecycle]]：`run_helper` 与 `run_helper_task` 作为独立进程入口被主二进制拦截调用；`start_update_check_loop` 由应用启动挂载。
- [[android-backend]]：安卓不复用本模块（`helper` 与 `updater` 均位于 `src-tauri/src/` 桌面端路径，安卓有独立更新策略）。

## Known Issues

1. **`UpdateInfo.assets[1].url` 是 GitHub API 地址而非 `.sha256` 文件地址（命名误导）** — `update/updater.rs:571-583` 用 `sha256_urls.first()` 填充名为 `"{exe_name}.sha256"` 的资产，而 `sha256_urls[0]` 实为 `https://api.github.com/repos/.../releases/tags/v{tag}`（`:555`）。真正的多源列表放在 `sha256_checksum`（`:583`）。前端若按 `assets` 里的 `.sha256` 取校验和会拿到 API JSON。
2. **`ReleaseAsset.size` 恒为 0** — `update/updater.rs:571-582`。前端在下载前无法预判安装包体积。
3. **`decide_checksum_missing` 的 `all_client_errors` 语义比名字宽** — 初值 `true`（`update/updater.rs:181`），"响应 200 但解析不出任何有效哈希"（`:194`）与"读取响应体失败"（`:197`）都不会把它置 false，于是会被归入"全部 4xx"分支：未开跳过时报出 `"校验和源全部不可用（4xx）"`（`:130`）这一与实际原因不符的错误文案；开跳过时则直接降级通过（`:223`）。MSI-only Release（无 `.exe` 资产）会稳定命中此路径（`:148` 只认 `.exe`）。
4. **`verify_download_sha256` 用 `Ok(true)` 表示"未校验但放行"** — `update/updater.rs:217-224`。调用方无法区分"哈希一致"与"降级放行"，前端拿不到"本次未校验"的信号。
5. **`.sha256` 文本解析过窄** — `update/updater.rs:142-170` 只取"首个空白 token"或"最后一个 `=` 后的 token"，且不做 BOM 剥离/逐行扫描：文件中含 UTF-8 BOM、首行为注释、或采用多行列表时均返回 `None`，继续降级到下一个源。
6. **`GITHUB_REPO` / `VERSION_FILE_URL` / `VERSION_MIRRORS` 硬编码，未从配置读取** — `update/updater.rs:9`、`:24-31`。fork 或仓库改名需改源码，且 `version.json` 路径写死为 `main` 分支。
7. **临时更新目录启动期清理已落地** — 原缺口已由 `cleanup_stale_update_dir`（`update/updater.rs:298-329`）补齐，在进程启动时扫 `%TEMP%/campus-login-update` 删 24h 前残留，目录名收敛为 `UPDATE_TEMP_DIR` 常量。
8. **`write_result_file` 忽略全部 IO 错误** — `helper/mod.rs:1251-1263`（写入与 rename 都不检查）。写失败时主进程只能等到超时，而 helper 侧因不初始化 logger 不留任何落盘痕迹。
9. **MAC 伪装值清除失败仍报成功** — `helper/mod.rs:580-587`：`remove_mac_from_registry` 失败只在 `logs` 里 push 一条中文提示，`success` 仍为 `true`。注册表 `NetworkAddress` 残留会使伪装 MAC 在重启后继续生效。
10. **helper 无自我超时、无重入保护** — `helper/mod.rs:292-331` 一旦进入 `run_*` 就同步跑到结束；两个并发提权副本写同一 `--result` 路径时后写覆盖先写（调用方路径含 pid+毫秒时间戳，实际碰撞概率低但无锁）。
11. **`--family` 对 mac/set_metric 操作静默生效为 `"both"` 且被忽略** — `helper/mod.rs:169-256` 默认值 + 分派只看变体，`HelperOp::Mac` / `HelperOp::SetMetric` 都不携带 family；若调用方误传 `--family` 不会有任何提示。
12. **`run_dns` 不做 targets 空校验** — `helper/mod.rs:463-482` 空名单会原样传给 `setup_dns_doh_admin`；`run_set_metric:658-660` 对空 rows 反倒直接失败，两者口径不一致。
13. **`run_mac` 的 GUID 未经格式校验** — `helper/mod.rs:544-568` 只做 `eq_ignore_ascii_case` 查找，找不到即返回错误；与 MAC 的严格 12 位校验（`:546`）不对称。
14. **HEAD 资产探测对镜像不通的环境偏保守** — `update/updater.rs:533-550`：探测请求失败（`Err`）时日志打 `log_debug!`（`:547`）并维持 `has_update = true`，纯镜像网络下用户可能收到更新通知但下载走不到官方源。
15. **`do_update_check` 失败静默** — `update/updater.rs:439-444` 只有一行 `log_warn!`，不向前端发事件、不写前端可见状态，设置界面"上次检查时间"会停留在旧值。
16. **`start_update_check_loop` 注册失败只告警** — `update/updater.rs:374-412` 中注册失败仅 `log_warn!`（`:410`），自动更新静默失效时无 UI 反馈。
17. **`set_metric` 的成败粒度是整条 op，没有结构化明细** — `helper/mod.rs:645-727`：对任一条条目失败即整体 `success:false`，失败明细只拼进 `message` 与 `logs`（`details` 恒为 `None`）。一次提交 IPv4+IPv6 两条而只成功一条时，调用方只能从中文 `message` 文本里分辨哪条生效。
18. **更新提示只弹一次** — `update/updater.rs:426` 用 `update_stats.update_notified` 的 `compare_exchange(false, true, ...)` 做一次性门控，且全仓库没有任何地方把它重置为 false。用户忽略首次通知后，本进程生命周期内不会再收到提醒。