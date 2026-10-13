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

本模块文档覆盖两块桌面端能力：**helper**（`tauri-app/src-tauri/src/helper/mod.rs`）——以管理员身份重启自身执行"改 MAC""设 DNS+DoH""改接口跃点（metric）"的提权子进程入口，负责参数解析、执行、原子落结果文件；**update**（`tauri-app/src-tauri/src/update/mod.rs` + `tauri-app/src-tauri/src/update/updater.rs`）——版本比较、多源 version.json 拉取、Release 资产探测、SHA256 多源校验、24h 周期自检循环与临时目录清理。

两者都在 `lib.rs:15-20` 以 `#[cfg(desktop)]` 门控，**安卓 target 完全不编译**（安卓有独立的 `android/src-tauri/src/update_cmds.rs` 与 `monitor_loop.rs`，`android/src-tauri/Cargo.toml:34` 以 path 依赖 `campus-login` crate 时只会拿到跨平台协议核心）。helper 额外在桌面二进制入口 `main.rs:13` 声明，并在 `main.rs:30-39` **早于 Tauri Builder、单实例、托盘装配**拦截 `--helper` 参数。

本模块全部 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`（`version.json`、`Cargo.toml` 除外，按仓库根相对路径标注）。

## Key Components

### `helper/mod.rs`

- `pub fn proxy_base_dir() -> PathBuf` — `helper/mod.rs:38-55`。代理根目录：Windows 优先 `SHGetKnownFolderPath(FOLDERID_ProgramData)`（经 `validate_programdata_path` 校验），失败回退 `PROGRAMDATA` 环境变量，再不行硬编码 `C:\ProgramData`，拼接 `"CampusLogin"`；非 Windows 返回 `%TEMP%/campus-login-proxy`。
- `pub fn helper_results_dir() -> PathBuf` — `helper/mod.rs:67-69`。结果文件固定目录：`proxy_base_dir().join("results")`。
- `pub fn helper_requests_dir() -> PathBuf` — `helper/mod.rs:72-74`。请求文件目录：`proxy_base_dir().join("requests")`。
- `pub fn resolve_result_path(name: &str) -> Result<PathBuf, String>` — `helper/mod.rs:82-93`。校验结果文件名并拼接到固定结果目录。只接受纯文件名（拒绝路径分隔符、`..`、空串、`.` 开头、非 ASCII 字母数字/`-_`），杜绝请求方把 SYSTEM 写重定向到任意路径。
- `pub struct HelperResult` — `helper/mod.rs:96-106`，`#[derive(Debug, Clone, Serialize)]`（**无** `camelCase` 重命名）。
- `pub enum HelperOp` — `helper/mod.rs:109-135`，`#[derive(Debug, Clone, PartialEq)]`，9 个变体：`Dns { targets, family }`、`ClearDns { targets }`、`Mac { guid, mac_no_dash }`、`EnableAdapter { name }`、`DisableAdapter { name }`、`EnableDevice { instance_id }`、`SetMetric { rows }`、`RouteAdd { rows }`、`RouteDelete { rows }`、`RegisterTaskProxy`、`SelfCheck`；后九个（含 `SetMetric`/`Route*`/`EnableAdapter`/`DisableAdapter`/`EnableDevice`/`RegisterTaskProxy`）带 `#[cfg(target_os = "windows")]`。
- `fn build_op_from_args(op: &str, args: &[String]) -> Result<HelperOp, String>` — `helper/mod.rs:138-199`（私有）。命令行 `--helper` 与计划任务请求 JSON 共用：先扫描出位置参数与 `--family <v>`（`--family` 缺值 → `Err`，默认 `"both"`，`:141-152`），再按 op 名分派（`:153-195`）：`dns` / `clear_dns` / `mac`（位置参数 0=GUID、1=MAC，任一缺失 → `Err`，`:156-166`）/ `enable_adapter`（`:168-174`）/ `disable_adapter`（`:176-182`）/ `enable_device`（`:184-190`）/ `route_add` / `route_delete`（`:192-193`）/ `set_metric`（rows = 全部位置参数，`:194`）/ `register_task` / `selfcheck`；其他 → `Err("未知 helper 操作: {other}")`（`:198`）。
- `pub fn parse_helper_args(args: &[String]) -> Result<Option<(HelperOp, Option<String>)>, String>` — `helper/mod.rs:202-232`。语义：未出现 `--helper` → `Ok(None)`（`:203-205`）；出现但参数非法 → `Err`（调用方不启动正常应用）。扫描 op 之后到 `--result` 之间的参数交 `build_op_from_args`（`:206-221`），`--result` 缺值 → `Err`（`:218`）。
- `pub fn run_helper(op: HelperOp, result_path: Option<String>) -> i32` — `helper/mod.rs:235-253`。按变体分派到各 `run_*`（`:239-250`，三处 match 的另一处见 `process_request:368-385`）；`result_path` 为 `Some` 时经 `resolve_result_path` 收口后写结果文件（`:251-256`）；**返回退出码 0（成功）/ 1（失败）**（`:257`）。
- `pub fn run_helper_task() -> i32` — `helper/mod.rs:275-289`。计划任务 worker（`--helper-task`，main.rs 最先拦截）：扫 `requests/` 取最旧 `req-*.json` → `process_request`（`:316-371`：读入内存后立即删请求文件防重复执行 → `build_op_from_args` → 执行 → 结果写固定目录）；连续 3 次空扫后退出（每次间隔 400ms，兜住连续触发的竞态）。
- `fn run_dns(targets, family, logs)` — `helper/mod.rs:391-413`（私有）。写一条起始日志，调 `network::dns_setup::setup_dns_doh_admin(targets, family)`（`:396`），取返回 JSON 的 `success`/`message`（缺省 `"设置DNS+DoH完成"`），把整个 JSON 塞进 `details`。
- `fn run_clear_dns(targets, logs)` — `helper/mod.rs:415-470`（私有）。逐 GUID 调 `dns_config::clear_adapter_dns_via_api`，明细进 `details.{restored,failed}`，三态汇总文案（全成功/部分失败/全失败）。
- `fn run_mac(guid, mac_no_dash, logs)` — `helper/mod.rs:472-536`（私有）：
  - **MAC 格式前置校验**：必须 `len == 12` 且全部为 ASCII 十六进制字符，否则直接返回 `success:false` + `"MAC 格式非法: ..."`（`:474-481`）——防"非法格式静默写坏网卡配置"；
  - `network::get_adapters_force()` 枚举，按 `guid.eq_ignore_ascii_case` 找适配器（`:497`）；
  - `network::dhcp::apply_mac_change_via_registry(guid, &adapter.name, mac_no_dash)`（`:509`）；成功后**在同一管理员上下文内**调 `network::dhcp::remove_mac_from_registry(guid)` 清除 `NetworkAddress` 持久伪装值，失败只 push 一条日志、**不影响 success**（`:514-516`）。
- `fn decode_set_metric_row(row: &str) -> Result<(String, u16, bool, u32), String>` — `helper/mod.rs:538-569`（私有）。把 `"{guid}:{family}:{automatic}:{metric}"` 拆成带花括号 GUID / 协议栈 / 是否自动跃点 / 跃点值，字段格式（字段数、`family` 只认 2 与 23、`automatic` 只认 0 与 1、`metric` 必须 `u32`）**一律严格校验后才交给 Win32**。
- `fn run_set_metric(rows, logs)` — `helper/mod.rs:571-652`（私有）：空 rows 直接失败（`:583`，"什么都没做"不能报成功）→ 逐条解码 → `platform::metric::interface_rows_for_guid` 取当前行（`:599-608`）→ 按 `Family` 命中该协议栈的行 → **整行改副本**（覆写 `UseAutomaticMetric`（`:616`）、`Metric`、`SitePrefixLength = 0`（`:619`，非 0 会报 `ERROR_INVALID_PARAMETER`））→ `SetIpInterfaceEntry`（`:620`）。任一条失败即整体 `success:false` 并把失败明细汇总进 `message` 与 `logs`。
- `fn run_enable_adapter(name, logs)` / `fn run_disable_adapter(name, logs)` — `helper/mod.rs:654-708` / `:710-764`（私有）。适配器名双校验（`validate_adapter_name` + 本机适配器列表存在性）；逻辑相同，仅 `netsh` 参数 `enable`/`disable` 不同。
- `fn run_route_add(rows, logs)` / `fn run_route_delete(rows, logs)` — `helper/mod.rs:780-785` / `:787-792`（私有）。公共执行体 `run_route_rows`（`:794-865`）：先全量校验 `decode_route_row`（`:812-819`，要求 5 段 IPv4 点分十进制 + 数字 metric/if_index）再逐条执行 `route add/delete`；删除时若输出包含"找不到"/"not found"视为成功（幂等还原，`:796-799`）。
- `fn run_enable_device(instance_id, logs)` — `helper/mod.rs:867-927`（私有）。PnP 设备级启用（`pnputil /enable-device`）：字符集白名单（防 pnputil 开关注入）+ `CM_Locate_DevNodeW(PHANTOM)` 存在性复核 + problem 22 解除轮询复核（3s 上限，`:897-926`）。
- `fn run_register_task(logs)` — `helper/mod.rs:929-949`（私有）。`task_proxy::register_task_via_com`（COM 注册需管理员）。
- `fn write_result_file(path, result)` — `helper/mod.rs:951-962`（私有）。先写 `"{path}.tmp"` 再 `std::fs::rename` 覆盖（原子写，避免主进程读到半截）；**两步的返回错误都被忽略**（`if ... is_ok()`，`:956-959`）。
- `#[cfg(test)]` 用例 **22 个**（`helper/mod.rs:965-1135`）：解析类（`parse_no_helper_returns_none` / `parse_dns_with_result` / `parse_dns_with_targets_and_family` / `parse_clear_dns_with_targets` / `parse_mac` / `parse_mac_missing_args_is_err` / `parse_unknown_op_is_err`）、构造类（`build_op_enable_adapter` / `build_op_enable_device` / `build_op_selfcheck_and_register_task` / `build_op_unknown_is_err`）、结果文件与序列化（`helper_result_roundtrip` / `helper_result_omits_empty_logs_and_details` / `resolve_result_path_accepts_plain_name` / `resolve_result_path_rejects_traversal_and_absolute` / `task_request_json_roundtrip`）、`set_metric_round_trip_and_reject`（编码条目 round-trip + 8 种非法条目拒绝）、`decode_route_row_accepts_valid_and_rejects_injection`（路由条目校验 + 注入尝试）、`route_delete_miss_detection`（幂等删除检测）。**Windows API 的真实调用不在单测覆盖**（需真实网卡/提权环境），真机冒烟在发布前手工执行。

### `update/mod.rs`

- `update/mod.rs:1` 仅一行 `pub mod updater;`，无其他内容。

### `update/updater.rs` 常量与静态

| 名称 | 值 | 位置 | 可见性 |
|---|---|---|---|
| `GITHUB_REPO` | `"ikliml666/Wxxy-CampusLogin"` | `update/updater.rs:8` | 私有 |
| `AUTO_CHECK_INTERVAL_SECS` | `86400`（24h） | `update/updater.rs:9` | 私有 |
| `BACKOFF_RETRY_SECS` | `[300, 900, 3600]`（5min/15min/1h） | `update/updater.rs:11` | 私有 |
| `STARTUP_CHECK_DELAY_SECS` | `5` | `update/updater.rs:13` | 私有 |
| `VERSION_FILE_URL` | `https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json` | `update/updater.rs:15` | 私有 |
| `VERSION_MIRRORS` | 3 项：`ghfast.top`、`gh-proxy.com`、`ghproxy.net` 前缀 + 官方 raw URL | `update/updater.rs:18-22` | 私有 |
| `LAST_CHECK_ERROR` | `Mutex<Option<String>>` | `update/updater.rs:25` | 私有 static |
| `LAST_CHECK_TIME_MS` | `AtomicU64` | `update/updater.rs:27` | 私有 static |
| `SHORT_TIMEOUT_CLIENT` | `OnceLock<Result<reqwest::Client, String>>` | `update/updater.rs:82` | 私有 static |
| `UPDATE_TEMP_DIR` | `"campus-login-update"` | `update/updater.rs:233` | 公开常量 |
| `UPDATE_STALE_SECS` | `24 * 3600`（24h） | `update/updater.rs:235` | 私有 |
| `UPDATE_ASSET_EXTS` | `["exe", "msi", "sha256"]` | `update/updater.rs:237` | 私有 |

### `update/updater.rs` 函数

- `pub fn compare_versions(current: &str, latest: &str) -> bool` — `:48-75`。逐段按 `u32` 比较，`trim_start_matches('v')` 后按 `.` 切分，每段取**前导 ASCII 数字**（`"2-beta"` → `2`，解析失败按 0）；比较长度取两侧最大（`:63`），缺失段按 0（`:65-66`）；`latest > current` 才返回 true。3 个用例在 `:468-482`（含 4 段 hotfix `2.3.0.1`）。
- `fn build_short_timeout_http_client() -> Result<&'static reqwest::Client, String>` — `:84-96`（私有）。`get_or_init` 缓存 `Result`（工具链 `get_or_try_init` 未稳定），超时 **10 秒**；注释说明**不接入** `network/client.rs` 的 `CLIENT_POOL`（池内客户端强制 `.no_proxy()`，会改变更新下载对系统代理的语义）。
- `pub fn decide_checksum_missing(all_client_errors: bool, had_transport_error: bool, allow_skip: bool) -> Result<(), String>` — `:103-117`。`all_client_errors && !had_transport_error` 且 `allow_skip` → `Ok(())` 降级通过（`:104-111`）；同条件但未开跳过 → `Err("校验和源全部不可用（4xx），且未开启 skipSha256WhenMissing，拒绝安装未校验的安装包")`（`:112`）；其余（含 5xx / 传输错误）→ `Err("所有校验和源均失败（含 5xx/网络错误），拒绝安装")`（`:115`）。3 个用例在 `:461-466`、`:499-512`。
- `fn extract_checksum(url: &str, text: &str) -> Option<String>` — `:124-152`（私有）。URL 含 `api.github.com` 时按 JSON 处理：遍历 `assets`，找 `name` 以 `.exe` 结尾的项，取其 `digest` 去掉 `sha256:` 前缀且为 64 位十六进制（`:128-141`）；否则按纯文本：取首个空白分隔 token，是 64 位十六进制即用（shasum 风格，`:143-147`），否则取最后一个 `=` 之后 trim 的 token（BSD 风格，`:148-150`）。返回值统一 `to_lowercase()`。4 个用例在 `:485-496`、`:517-548`。
- `pub async fn verify_download_sha256(file_path: &str, checksum_urls: &[String], allow_skip_missing: bool) -> Result<bool, String>` — `:154-233`：
  - 空 URL 列表 → `Err("未提供校验和URL")`（`:155-157`）；
  - 顺序尝试每个 URL（`:167-198`）：成功响应（`is_success`）→ 取文本 → `extract_checksum` 命中即 `break`；响应非成功 → 记录 `"HTTP {status}"`，且**非 4xx 时**把 `all_client_errors` 置 false（`:186-189`）；`send` 失败 → 置 `all_client_errors=false` 且 `had_transport_error=true`（`:191-196`）；
  - 全部失败 → 调 `decide_checksum_missing(...)`，`Ok(())` 时**直接返回 `Ok(true)`**（`:205`），`Err` 时包装最后错误返回（`:203-204`）；
  - 计算实际哈希：`tokio::task::spawn_blocking` 内以 **64KB buffer** 流式读取（`:213-230`，注释说明避免 50MB+ 安装包一次性读入造成内存峰值）；最终返回 `actual_hash == expected_hash`（`:232`）。
- `pub fn update_temp_dir() -> PathBuf` — `:239-241`。返回 `%TEMP%/campus-login-update`。
- `pub fn schedule_update_cleanup(app_handle: &AppHandle)` — `:243-256`。`spawn` 一个任务：`sleep(24 * 3600)` 后 `spawn_blocking(remove_dir_all(temp_dir))`，`temp_dir = %TEMP%/campus-login-update`（`:244`，经 `update_temp_dir()` 单点拼接，常量 `UPDATE_TEMP_DIR`）；完成后 `log_debug!("updater", "更新临时目录已清理")`（`:254`）。注释说明原值为 600s，因用户可能在 UAC 等待/稍后安装时被提前删除，改 24h（`:246-247`）。**审计 A13**：下载进行中不删目录（检查 `tasks.is_downloading.is_active()`，`:250-253`）。
- `pub fn cleanup_stale_update_dir()` — `:270-301`（2026-10-15 新增）。`spawn_blocking` 扫 `update_temp_dir()`，删扩展名属 `exe`/`msi`/`sha256`（`UPDATE_ASSET_EXTS`，大小写不敏感，含 `.sha256` 附属校验文件）且 mtime 超 `UPDATE_STALE_SECS`（24h，与 `schedule_update_cleanup` 同档）的文件。定位为 `schedule_update_cleanup` 的**启动期兜底**：后者定时挂在进程 runtime 上，用户 24h 内退出即失效；目录不存在视为正常（从未下载过）。调用方 `app/startup.rs`（`start_update_check_loop` 之后）。
- `pub fn now_epoch_ms() -> u64` — `:303-307`。
- `fn last_check_error_snapshot() -> Option<String>` / `fn last_check_time_snapshot() -> Option<u64>` — `:309-317`。快照锁状态供 `UpdateInfo` 构造使用。
- `fn backoff_delay_secs(fail_count: u32) -> u64` — `:319-325`。第 fail_count 次连续失败后的等待秒数：0=成功/首轮 → 24h 正常周期；1..=3 → 对应退避档位（5min/15min/1h）；超过 3 → 24h（调用方据此重置计数，下一轮 24h 周期重新按退避重试）。
- `pub async fn check_and_record(mirror_first: bool) -> Result<UpdateInfo, String>` — `:327-338`。执行一次检查并把结果记入最近检查状态（lastCheckError/lastCheckTime 供前端展示）。成功清除错误记录，失败写入原因；时间总是更新。自动循环与手动命令统一走本函数。
- `pub fn start_update_check_loop(app_handle: &AppHandle)` — `:340-372`。任务名 `update_check_loop`（`:343`），注册失败只 `log_warn!`（`:370`）。流程：先 `select!` 等 `STARTUP_CHECK_DELAY_SECS`（5s）或 cancel（`:347-350`）；进入 `loop` 判 cancel / `is_quitting`（`:352-354`）→ `do_update_check`（`:355`）→ **退避等待**（`:358-368`，`backoff_delay_secs` 决定睡眠时长，等待期监听 cancel 与 `is_quitting`）。调用方 `app/startup.rs:193`。
- `async fn do_update_check(app_h: &AppHandle, state: &AppState)` — `:374-405`（私有）。`mirror_first = config.update_source != "github"`（`:375`，默认 `update_source = "mirror"`，见 `config/model.rs:198`，即默认镜像优先）；`check_update_inner` 成功后发 `emit_update_available(has_update, latest_version, release_notes)`（`:378-382`）；有更新且 `update_stats.update_notified` 的 `compare_exchange(false, true, Acquire, Relaxed)` 成功时弹通知（`:385`）：Windows 桌面走 `platform::toast::show_update_toast`（可点击跳转关于界面）**失败降级**为 `emit_notification(app_h, "发现新版本", body, "mascot-update")`（`:388-392`，`#[cfg(all(desktop, target_os = "windows"))]`），非该组合直接走普通通知（`:393-394`）。失败只 `log_warn!`（`:402-404`）。
- `pub async fn fetch_latest_release(mirror_first: bool) -> Result<(bool, String, String, Option<String>), String>` — `:407-433`。按 `mirror_first` 排源顺序（`:408-412`，镜像优先时 `[M0, M1, M2, 官方]`，否则 `[官方, M0, M1, M2]`），顺序尝试 `fetch_version_from_url`；首个成功即返回（降级生效时 `i > 0` 记 `log_info!`，`:418`）；全失败返回 `Err("所有更新源均失败（首选: {primary_err}）")`（`:432`）。
- `async fn fetch_version_from_url(url: &str) -> Result<(bool, String, String, Option<String>), String>` — `:435-474`（私有）。带 `User-Agent: CampusLogin-UpdateChecker` 请求；非 2xx → `Err("version.json不可用: HTTP {status}")`；`data["version"]` 去 `v` 前缀，空 → `Err("version.json中缺少版本号")`（`:447-455`）；`current = env!("APP_VERSION")`（编译期由 `tauri-app/src-tauri/build.rs:37` 从 `tauri.conf.json` 注入）；`has_update = compare_versions(current, &latest_tag)`；`notes` 缺省 `""`（`:460`），`asset` 缺省 `None`（`:463`）。返回四元组 `(has_update, latest_tag, notes, asset)`。
- `pub async fn check_update_inner(mirror_first: bool) -> Result<UpdateInfo, String>` — `:476-540`。步骤：
  - 拉取版本（`:477`）；
  - 安装包名：`asset.unwrap_or_else(|| format!("Wxxy-CampusLogin_{latest_tag}_x64-setup.exe"))`（`:479`）——仓库根 `version.json` 当前只有 `{"version": "v2.3.6"}`，所以实际走回退命名；
  - 构造 `github_exe_url = https://github.com/{GITHUB_REPO}/releases/download/v{tag}/{exe_name}`（`:480-482`）；
  - **资产存在性探测**：`client.head(&github_exe_url)` 返回 `404` 时把 `has_update` 置 false 并 `log_warn!`（`:486-488`，防"version.json 先行、Release 未发布"导致用户下载必败）；其他状态码视为存在；`Err`（网络失败）**保守维持** `has_update`（`:491-493`）；
  - 构造 5 个校验和 URL（`:499-510`）：GitHub API `releases/tags/v{tag}`、`{github_exe_url}.sha256`、3 个镜像前缀版本；
  - 组装 `UpdateInfo`（`:512-538`）：`assets` 固定两个条目（exe 与 `"{exe_name}.sha256"`，`size` 均为 `0`），`sha256_checksum = Some(serde_json::to_string(&sha256_urls))`（`:536`，即把 5 个 URL 序列化成 JSON 字符串放在该字段里）。
- `#[cfg(test)]`：常量 `HASH_LOWER`（`:547`）与 8 个用例（`:460-548`）。

### 主进程侧辅助链路（本模块的调用方，供定位用）

- `platform/helper_spawn.rs:13-20` `unique_result_path()`：`%TEMP%/campus-login-helper-<pid>-<毫秒时间戳>.json`。
- `platform/helper_spawn.rs:23-35` `read_helper_result`：解析 JSON → 把 `logs[]` 逐条 `log_info!("helper", ...)` → 删除结果文件。
- `platform/helper_spawn.rs:41-80` `spawn_elevated_helper(op, args, result_path, timeout)`：拼 `--helper {op} "{arg}"... --result "{path}"`（所有位置参数强制加引号，`:52-58`）；先试 COM `shell_exec_elevated(..., true)` 静默提权，失败降级 `run_elevated`（弹 UAC，`:61-64`）；随后 **100ms 间隔轮询**结果文件直到 `timeout`（`:67-73`），超时后**再查一次**（`:76-78`，避免恰好越过 deadline 的写入被误报超时），仍未读到 → `Err("提权操作超时，未收到helper结果")`。
- 两个实际调用点与超时：`commands/network_cmd.rs:302-313`（DNS 提权分支，`Duration::from_secs(30)` 在 `:312`）、`network/dhcp.rs:287-295`（MAC 提权分支，`Duration::from_secs(25)` 在 `:294`）；MAC 成功返回前还会 `poll_ip_change(..., 25_000)` 等 IP 变更（`network/dhcp.rs:308`）。
- 管理员直通路径：`commands/network_cmd.rs:298-300`（`is_admin()` → 直接 `setup_dns_doh_admin`）、`network/dhcp.rs:278-285`（`is_admin()` → `set_mac_via_registry`）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Desktop App (Tauri)                          │
│                                                                     │
│  ┌──────────────────┐    ┌──────────────────────────────────────┐   │
│  │ main.rs          │    │ app/startup.rs                       │   │
│  │ :13              │    │ :193                                 │   │
│  │ --helper 拦截     │    │ start_update_check_loop              │   │
│  │ :30-39           │    │      ▼                               │   │
│  └────────┬─────────┘    │   update_check_loop (24h cycle)       │   │
│           │              └──────────────────────────────────────┘   │
│           │                                                        │
│           ▼                                                        │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ helper/mod.rs                                                │   │
│  │                                                              │   │
│  │  ┌──────────────┐  ┌────────────────────────────────────┐    │   │
│  │  │ parse_helper │  │ run_helper_task (--helper-task)     │    │   │
│  │  │ args         │  │   ▲                                  │    │   │
│  │  └──────┬───────┘  │   │ (SYSTEM worker, no UAC)         │    │   │
│  │         │          │   │ scan requests\                   │    │   │
│  │         ▼          │   └────────────────────────────────┘    │    │   │
│  │  ┌──────────────┐  │                                         │    │   │
│  │  │ build_op_from│  │  ┌────────────────────────────────┐    │    │   │
│  │  │ _args        │◄─┼──┤ process_request                │    │    │   │
│  │  └──────┬───────┘  │  │ (read → delete → execute)      │    │    │   │
│  │         │          │  └────────────────────────────────┘    │    │   │
│  │         ▼          │                                         │    │   │
│  │  ┌──────────────┐  │  ┌────────────────────────────────┐    │    │   │
│  │  │ run_helper   │  │  │ run_* ops                      │    │    │   │
│  │  │ (single-run) │  │  │ dns / clear_dns / mac          │    │    │   │
│  │  └──────┬───────┘  │  │ enable/disable adapter         │    │    │   │
│  │         │          │  │ route add/delete               │    │    │   │
│  │         │          │  │ set_metric                     │    │    │   │
│  │         │          │  │ register_task / selfcheck      │    │    │   │
│  │         │          │  └────────────────────────────────┘    │    │   │
│  │         │          │                                         │    │   │
│  │         ▼          │                                         │    │   │
│  │  ┌──────────────┐  │  ┌────────────────────────────────┐    │    │   │
│  │  │ resolve_res  │  │  │ write_result_file              │    │    │   │
│  │  │ ult_path     │  │  │ (.tmp → rename atomic)         │    │    │   │
│  │  └──────┬───────┘  │  └────────────────────────────────┘    │    │   │
│  │         │          │                                         │    │   │
│  │         ▼          │                                         │    │   │
│  │  ┌──────────────┐  │                                         │    │   │
│  │  │ HelperResult │  │                                         │    │   │
│  │  │ (JSON file)  │  │                                         │    │   │
│  │  └──────────────┘  │                                         │    │   │
│  └────────────────────┴─────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ update/updater.rs                                            │   │
│  │                                                              │   │
│  │  ┌──────────────┐  ┌────────────────────────────────────┐    │   │
│  │  │ check_update │  │ fetch_latest_release               │    │   │
│  │  │ _inner       │  │ (mirror first or github first)     │    │   │
│  │  └──────┬───────┘  └──────────────┬─────────────────────┘    │   │
│  │         │                         │                           │   │
│  │         │                    ┌────▼────┐                     │   │
│  │         │                    │ GET     │                     │   │
│  │         │                    │version.json│                  │   │
│  │         │                    └────┬─────┘                     │   │
│  │         │                         │                           │   │
│  │         │                    ┌────▼────┐                     │   │
│  │         │                    │ HEAD    │                     │   │
│  │         │                    │release? │                     │   │
│  │         │                    └────┬─────┘                     │   │
│  │         │                         │                           │   │
│  │         │                    ┌────▼─────┐                    │   │
│  │         │                    │ GET .sha │                    │   │
│  │         │                    │ 256 URLs │                    │   │
│  │         │                    └────┬─────┘                     │   │
│  │         │                         │                           │   │
│  │         │                    ┌────▼─────┐                    │   │
│  │         │                    │ SHA256   │                    │   │
│  │         │                    │ verify   │                    │   │
│  │         │                    └──────────┘                     │   │
│  │  ┌──────────────┐  ┌────────────────────────────────────┐    │   │
│  │  │ schedule_upd │  │ cleanup_stale_update_dir            │    │   │
│  │  │ ate_cleanup  │  │ (startup fallback, 24h stale)       │    │   │
│  │  └──────────────┘  └────────────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### helper：提权往返链路

```
用户点击"设置 DNS"/"修改 MAC"（夜间出站切换的写 metric 走同一通道，由任务 5 接线）
  ├─ is_admin() → 直接在进程内执行（commands/network_cmd.rs:299-301；network/dhcp.rs:277-286）
  └─ 非管理员
     → unique_result_path()                        platform/helper_spawn.rs:13
     → spawn_elevated_helper(op, args, path, t)     platform/helper_spawn.rs:41
        → "--helper dns|mac|set_metric <位置参数> [--family v] --result <path>"
        → 计划任务代理（SYSTEM worker）→ COM ICMLuaUtil 静默提权 → ShellExecuteW runas 弹 UAC
     → 提权副本进程启动（同一 exe）
        → main.rs:30  parse_helper_args（早于 Tauri Builder / 单实例 / 托盘）
           ├─ Ok(None)      → 继续正常应用启动（main.rs:34）
           ├─ Ok(Some(op,path)) → std::process::exit(run_helper(op, path))   main.rs:31-33
           └─ Err(e)        → eprintln + exit(2)，不启动 UI                     main.rs:35-38
        → run_helper                                    helper/mod.rs:235
           ├─ Dns → run_dns → dns_setup::setup_dns_doh_admin(targets, family)  helper/mod.rs:391-413
           ├─ Mac → run_mac → 格式校验 → get_adapters_force → 按 GUID 查找
           │         → dhcp::apply_mac_change_via_registry
           │         → dhcp::remove_mac_from_registry（清除持久伪装值）          helper/mod.rs:472-536
           └─ SetMetric → decode_set_metric_row → interface_rows_for_guid
                     → 整行改副本（SitePrefixLength=0）→ SetIpInterfaceEntry    helper/mod.rs:571-652
        → write_result_file（.tmp → rename 原子替换）    helper/mod.rs:951-962
        → 退出码 0（成功）/ 1（失败）                     helper/mod.rs:257
     → 主进程 100ms 轮询结果文件（超时后再补查一次）      platform/helper_spawn.rs:66-78
        → read_helper_result：logs 进日志、删文件、返回 JSON
  → 调用方按 JSON 的 success/message/details 决定 UI 表现与后续动作
     （MAC：network/dhcp.rs:300-320 之后还会 poll_ip_change 最多 25s）
```

计划任务通道的另一条入口（无 UAC）：`platform/task_proxy.rs` 写 `requests\req-*.json` → `schtasks /run` → worker（`run_helper_task:275`）逐个执行同一批 op。

超时值：DNS **30s**（`commands/network_cmd.rs:312`）、MAC **25s**（`network/dhcp.rs:294`）、metric **30s**（消费方任务 5 约定，helper 侧不设超时）。helper 自身没有任何自我超时。

### update：周期自检链路

```
app/startup.rs:193  start_update_check_loop
  → 任务 "update_check_loop"
  → sleep 5s（STARTUP_CHECK_DELAY_SECS，可与 cancel 竞争）
  → loop
     → do_update_check
        → mirror_first = (config.update_source != "github")        update/updater.rs:375
        → check_update_inner(mirror_first)
           → fetch_latest_release  按序尝试 [镜像×3, 官方] 或 [官方, 镜像×3]
              → fetch_version_from_url：GET version.json（UA: CampusLogin-UpdateChecker，10s 超时）
                 → version / notes / asset → compare_versions(env!("APP_VERSION"), latest)
           → exe_name = version.json.asset 或 "Wxxy-CampusLogin_{tag}_x64-setup.exe"
           → HEAD github_exe_url：404 → has_update=false；其他/网络失败 → 维持
           → 构造 5 个 sha256 URL（api.github.com / 原始 .sha256 / 3 镜像 .sha256）
           → UpdateInfo { assets: [exe, exe.sha256], sha256_checksum: <5 URL 的 JSON 数组串> }
        → EventBus::emit_update_available(has_update, latestVersion, releaseNotes)
        → if has_update && update_notified.CAS(false→true) 成功
             Windows 桌面：platform::toast::show_update_toast（失败降级 emit_notification）  updater.rs:388-392
             其他：emit_notification("发现新版本", ..., "mascot-update")
     → 退避等待（backoff_delay_secs，最长 1h，用尽恢复 24h）  updater.rs:358-368
```

手动检查走 `commands/updater.rs:8-22`（`check_update` 命令，同样 `check_update_inner`；检查时间由 `update/updater.rs` 的 `LAST_CHECK_TIME_MS` 静态量记录）。

### update：下载 → 校验 → 安装 → 清理链路

```
checked via commands/updater.rs
  check_update（:8-22）→ UpdateInfo 给前端
  download_update（:24-192）
     → is_downloading 互斥（:31-32）
     → 只允许 https + 域名白名单（13 项，含 github.com / ghfast.top / gh-proxy.com / ghproxy.net / moeyy.cn 等，:39-54）
     → 文件名取自 URL 末段，清洗 Windows 非法字符、含 ".." 或空则回退 "update.exe"
     → 目标：%TEMP%/campus-login-update/<filename>（:76-78，目录经 update_temp_dir() 单点拼接）
     → MAX_DOWNLOAD_SIZE = 500MB（声明在 :74，响应头与逐块写入前各判一次）
     → reqwest 客户端：connect_timeout 30s / total timeout 1800s（:81-82）
     → tokio::fs 流式写；每 ≥200ms 或收满时发 DownloadProgress（:138 判定，:150-155 发送）
  install_update（:195-292）
     → 路径必须 canonicalize 后位于 %TEMP%/campus-login-update 之内（先于校验；目录经 update_temp_dir() 单点拼接）
     → checksum_url 缺失/空 → Err("未提供SHA256校验和，安装已阻止")（:238-243）
     → verify_download_sha256(file_path, urls, config.skip_sha256_when_missing)（:222-227）   update/updater.rs:154
        ├─ Ok(true)  → 通过（可能来自"允许跳过且全部 4xx"的降级）  updater.rs:205
        ├─ Ok(false) → 删文件 + Err("安装包校验失败：SHA256不匹配，文件可能被篡改")
        └─ Err(e)    → 删文件 + Err("SHA256校验过程失败，安装已阻止: {e}")
     → 扩展名分派：exe → open::that；msi → msiexec /i（raw_arg，路径含引号则拒绝）
     → 成功后 schedule_update_cleanup()（24h 后删 %TEMP%/campus-login-update）  updater.rs:243
   启动期兜底：app/startup.rs 在 start_update_check_loop（:229）后调 cleanup_stale_update_dir()（:231-233）
     → 删 %TEMP%/campus-login-update 中 mtime 超 24h 的 exe/msi/sha256（含附属校验文件）
     → 进行中的下载刷新 mtime 不受影响；目录名走 UPDATE_TEMP_DIR 常量（updater.rs:239）
```

### SHA256 多源解析与降级决策

```
校验源顺序（update/updater.rs:499-510）:
  1) https://api.github.com/repos/{GITHUB_REPO}/releases/tags/v{tag}   → JSON，取首个 .exe 资产的 digest
  2) {github_exe_url}.sha256                                            → 文本，首个 64 位 hex token 或 "=" 后 token
  3) ghfast.top / gh-proxy.com / ghproxy.net 前缀的 (2)
决策（update/updater.rs:103-117）:
  全部 4xx 且无传输错误 + allow_skip=true  → Ok（降级通过，记 warn 日志）
  全部 4xx 且无传输错误 + allow_skip=false → Err（拒绝安装）
  含 5xx 或传输错误                        → Err（一律拒绝，即使 allow_skip=true）
```

`allow_skip` 来自 `config.skip_sha256_when_missing`（`config/model.rs:100-101`，默认 `false`；`commands/updater.rs:222-225` 读取）。

### 版本比较语义

`compare_versions` 要求**任意段** `latest > current` 才判有更新（`update/updater.rs:62-71`）：段数不等时缺失段按 0，因此 `2.3.0` → `2.3.0.1` 返回 true、反向返回 false；`v` 前缀被剥离；`2.9.0` → `2.10.0` 按数值比较为 true。

### 周期与超时常量

| 名称 | 值 | 位置 |
|---|---|---|
| 启动延迟首查 | `5` s | `update/updater.rs:13` |
| 自检周期 | `86400` s（24h，5s 步进等待） | `update/updater.rs:9`、`:264-272` |
| 更新检查 / 哈希请求超时 | `10` s | `update/updater.rs:87` |
| 下载 connect / total 超时 | `30` s / `1800` s | `commands/updater.rs:81-82` |
| 下载进度事件节流 | `200` ms | `commands/updater.rs:147` |
| 单文件下载上限 | `500 * 1024 * 1024` | `commands/updater.rs:73` |
| 下载互斥 | `tasks.is_downloading` | `commands/updater.rs:33` |
| 临时目录清理延迟 | `24 * 3600` s | `update/updater.rs:248` |
| 残留安装包超龄阈值 | `24 * 3600` s | `update/updater.rs:235`（`UPDATE_STALE_SECS`，与上同档） |
| 残留安装包扩展名白名单 | `exe`/`msi`/`sha256` | `update/updater.rs:237`（`UPDATE_ASSET_EXTS`） |
| helper 结果轮询间隔 | `100` ms | `platform/helper_spawn.rs:72` |
| helper 超时（DNS / MAC） | `30` s / `25` s | `commands/network_cmd.rs:314`、`network/dhcp.rs:293` |
| SHA256 流式块大小 | `65536` 字节 | `update/updater.rs:218` |

## Connections

- [[desktop-platform]]：`platform/helper_spawn.rs`（提权启动与结果轮询）、`platform/elevation.rs`（COM ICMLuaUtil / ShellExecuteW runas）、`platform/toast.rs`（`show_update_toast`，仅 Windows 桌面）。
- [[desktop-network-dns]]：`helper` 的 DNS 分支直接调 `network::dns_setup::setup_dns_doh_admin`；`commands/network_cmd.rs` 的非管理员分支即本链路入口。
- [[desktop-network-core]]：`helper` 的 MAC 分支调 `network::get_adapters_force`、`network::dhcp::{apply_mac_change_via_registry, remove_mac_from_registry}`；主进程侧 `commands/updater.rs` 走 `reqwest` 而非 `network::client` 池。
- [[desktop-config]]：`update_source`（默认 `"mirror"`）、`skip_sha256_when_missing`（默认 `false`）两个字段直接影响本模块行为；`update_stats.update_notified` / `last_update_check_epoch_ms` 供设置界面显示上次检查时间。
- [[desktop-app-lifecycle]]：`main.rs:26-39` 在 Tauri Builder 之前的 helper 拦截与 `exit(2)`；`app/startup.rs:193` 挂载自检循环；`is_quitting` 参与循环退出。
- [[desktop-infra]]：`infra::task_manager`（任务注册与 cancel token）、`infra::events::EventBus::{emit_update_available, emit_update_download_progress, emit_update_notification_click}`、`infra::notification::emit_notification`、`infra::state::AppState`。
- [[desktop-commands]]：`commands/updater.rs` 的 4 个命令（`check_update`、`download_update`、`install_update`、`get_mirror_urls`）与 `commands/network_cmd.rs` 的 DNS 提权分支。
- [[desktop-auth]]：无直接依赖；MAC 伪装能力被 `auth` 的 MAC 重置逻辑间接使用（见 `auth/failure_tracker.rs` 的 5 次失败触发路径）。
- [[desktop-frontend-panels]]：前端"关于/更新"面板消费 `UpdateInfo`（`hasUpdate`/`latestVersion`/`releaseNotes`/`sha256Checksum`）与 `update-download-progress` 事件。
- [[android-backend]]：安卓**不复用**本模块（`lib.rs:19-20` 的 `#[cfg(desktop)]` 排除），安卓有自己的 `android/src-tauri/src/update_cmds.rs` 与独立的更新/安装策略。

## Known Issues

1. **`UpdateInfo.assets[1].url` 是 GitHub API 地址而非 `.sha256` 文件地址（命名误导）** — `update/updater.rs:444-448` 用 `sha256_urls.first()` 填充名为 `"{exe_name}.sha256"` 的资产，而 `sha256_urls[0]` 实为 `https://api.github.com/repos/.../releases/tags/v{tag}`（`:422`）。真正的多源列表放在 `sha256_checksum`（`:450`）。前端若按 `assets` 里的 `.sha256` 取校验和会拿到 API JSON。
2. **`ReleaseAsset.size` 恒为 0** — `update/updater.rs:440`、`:447`。前端在下载前无法预判安装包体积；下载进度里的 `total` 只能依赖 `Content-Length`（`commands/updater.rs:97`），无该响应头时 `percent` 恒为 `0.0`（`:144-148`）。
3. **`decide_checksum_missing` 的 `all_client_errors` 语义比名字宽** — 初值 `true`（`update/updater.rs:161`），"响应 200 但解析不出任何有效哈希"（`:174`）与"读取响应体失败"（`:177`）都不会把它置 false，于是会被归入"全部 4xx"分支：未开 `skipSha256WhenMissing` 时报出 `"校验和源全部不可用（4xx）"`（`:110`）这一与实际原因（文件存在但格式不符）不符的错误文案；开跳过时则直接降级通过（`:203`）。MSI-only Release（无 `.exe` 资产）会稳定命中此路径（`:128` 只认 `.exe`）。
4. **`verify_download_sha256` 用 `Ok(true)` 表示"未校验但放行"** — `update/updater.rs:203`。调用方 `commands/updater.rs:227-231` 无法区分"哈希一致"与"降级放行"，日志只留 `update/updater.rs:104-108` 的 warn，前端也拿不到"本次未校验"的信号。
5. **`.sha256` 文本解析过窄** — `update/updater.rs:141-149` 只取"首个空白 token"或"最后一个 `=` 后的 token"，且不做 BOM 剥离/逐行扫描：文件中含 UTF-8 BOM、首行为注释、或采用多行 `<hash>  <file>` 列表（第二行起的目标文件）时均返回 `None`，继续降级到下一个源。
6. **`VERSION_MIRRORS` 与可下载域名白名单不一致** — 检查阶段只走 3 个镜像（`update/updater.rs:15-19`），而 `commands/updater.rs:39-54` 的白名单列了 13 个域名（含 `gh-proxy.org`、`gh.ddlc.top`、`githubproxy.cc` 等）。新增镜像必须两处同步，否则会出现"能检查到版本但下载被拒绝"或反之。
7. **`GITHUB_REPO` / `VERSION_FILE_URL` / `VERSION_MIRRORS` 硬编码，未从配置读取** — `update/updater.rs:8`、`:12`、`:15-19`。fork 或仓库改名需改源码，且 `--result`、`version.json` 的路径同样写死为 `main` 分支。
8. ~~**临时更新目录没有启动期清理**~~（**已修复 2026-10-15**，`fix/sess-session--apk-cleanup`，与安卓端 APK 清理同源）— 原缺口：仅进程内 `sleep(24h)` 会删，用户提前退出即残留。现新增 `update/updater.rs` 的 `cleanup_stale_update_dir()`，`app/startup.rs` 在 `start_update_check_loop` 后调用，启动时删 `%TEMP%/campus-login-update` 中 mtime 超 24h 的 `exe`/`msi`/`sha256`；目录名收敛为 `UPDATE_TEMP_DIR` 常量 + `update_temp_dir()`，`commands/updater.rs` 两处硬编码拼接改为复用。
9. **`write_result_file` 忽略全部 IO 错误** — `helper/mod.rs:765-776`（写入与 rename 都不检查）。写失败时主进程只能等到 25s/30s 超时（`platform/helper_spawn.rs:79`），而 helper 侧因不初始化 logger（`helper/mod.rs:12-13`）不留任何落盘痕迹。
10. **MAC 伪装值清除失败仍报成功** — `helper/mod.rs:470-479`：`remove_mac_from_registry` 失败只在 `logs` 里 push 一条中文提示，`success` 仍为 `true`。用户看到"MAC已修改并重启网卡"，但注册表 `NetworkAddress` 残留会使伪装 MAC 在重启后继续生效（该注释本身指出这是"用户无从恢复"的场景）。
11. **helper 无自我超时、无重入保护** — `helper/mod.rs:200-218` 一旦进入 `run_dns`/`run_mac`/`run_set_metric` 就同步跑到结束；两个并发提权副本写同一 `--result` 路径时后写覆盖先写（路径含 pid+毫秒时间戳，`platform/helper_spawn.rs:13-19`，实际碰撞概率低但无锁）。
12. **`--family` 对 mac/set_metric 操作静默生效为 `"both"` 且被忽略** — `helper/mod.rs:108` 默认值 + `:204-215` 分派只看变体，`HelperOp::Mac` / `HelperOp::SetMetric` 都不携带 family；若调用方误传 `--family` 不会有任何提示（set_metric 的协议栈由条目自身携带）。
13. **`run_dns` 不做 targets 空校验** — `helper/mod.rs:351-373` 空名单会原样传给 `setup_dns_doh_admin`；目前安全性靠调用方 `commands/network_cmd.rs:288-294` 提前拦截空 `targets`，helper 侧无二次防线（`run_set_metric:543` 对空 rows 反倒直接失败，两者口径不一致）。
14. **`run_mac` 的 GUID 未经格式校验** — `helper/mod.rs:456` 只做 `eq_ignore_ascii_case` 查找，找不到即返回 `"未找到GUID对应的适配器"`；与 MAC 的严格 12 位校验（`:434`）不对称，恶意/错误 GUID 会让 helper 在管理员上下文内遍历全部适配器（只读，无写副作用）。新增的 `set_metric` 路径相反——GUID 经 `platform::elevation::parse_guid` 严格解析（`platform/metric.rs:24`）。
15. **更新提示只弹一次** — `update/updater.rs:292` 用 `update_stats.update_notified` 的 `compare_exchange(false, true, ...)` 做一次性门控，且**全仓库没有任何地方把它重置为 false**（`grep update_notified` 仅 `infra/state/mod.rs:88`、`:113` 的定义/初值与此处）。用户忽略首次通知后，本进程生命周期内不会再收到提醒（前端仍可通过 `emit_update_available` 显示状态）。
16. **HEAD 资产探测对镜像不通的环境偏保守** — `update/updater.rs:400-417`：探测请求失败（`Err`）时日志打 `log_debug!`（`:414`）并**维持** `has_update = true`，于是纯镜像网络下用户可能收到更新通知但下载走不到官方源（`download_update` 的 URL 由前端从 `assets[0].url` 选镜像，见 `commands/updater.rs:294-330` 的 `get_mirror_urls`）。
17. **`do_update_check` 失败静默** — `update/updater.rs:309-311` 只有一行 `log_warn!("updater", "更新检查失败: {}", e)`，不向前端发事件、不写 `last_update_check_epoch_ms`（`:303-307` 仅在成功分支），设置界面的"上次检查时间"会停留在旧值。
18. **`start_update_check_loop` 注册失败只告警** — `update/updater.rs:276`；`app/startup.rs:193` 的返回值被完全忽略，自动更新静默失效时无 UI 反馈。
19. **`set_metric` 的成败粒度是整条 op，没有结构化明细** — `helper/mod.rs:531-612`：`run_set_metric` 对任一条条目失败即整体 `success:false`，失败明细只拼进 `message` 与 `logs`（`details` 恒为 `None`）。一次提交 IPv4+IPv6 两条而只成功一条时，调用方只能从中文 `message` 文本里分辨哪条生效；需要机读时必须给 `details` 补 `{applied, failed}`（TODO 留给消费方任务 5/6 按需加）。