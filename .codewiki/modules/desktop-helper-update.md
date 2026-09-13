---
title: 提权辅助子进程（helper）与自动更新（update）
type: module
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/update/mod.rs
  - tauri-app/src-tauri/src/update/updater.rs
tags: [helper, elevation, uac, mac, dns, doh, updater, version-check, sha256, mirrors, tauri-events]
---

## Overview

本模块文档覆盖两块桌面端能力：**helper**（`helper/mod.rs`）——以管理员身份重启自身执行"改 MAC""设 DNS+DoH"的提权子进程入口，负责参数解析、执行、原子落结果文件；**update**（`update/mod.rs` + `update/updater.rs`）——版本比较、多源 version.json 拉取、Release 资产探测、SHA256 多源校验、24h 周期自检循环与临时目录清理。

两者都在 `lib.rs:15-20` 以 `#[cfg(desktop)]` 门控，**安卓 target 完全不编译**（安卓有独立的 `android/src-tauri/src/update_cmds.rs` 与 `monitor_loop.rs`，`android/src-tauri/Cargo.toml:34` 以 path 依赖 `campus-login` crate 时只会拿到跨平台协议核心）。helper 额外在桌面二进制入口 `main.rs:13` 声明，并在 `main.rs:30-39` **早于 Tauri Builder、单实例、托盘装配**拦截 `--helper` 参数。

本模块全部 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`（`version.json`、`Cargo.toml` 除外，按仓库根相对路径标注）。

## Key Components

### `helper/mod.rs`

- `pub struct HelperResult` — `helper/mod.rs:14-24`，`#[derive(Debug, Clone, Serialize)]`（**无** `camelCase` 重命名）。
- `pub enum HelperOp` — `helper/mod.rs:27-34`，`#[derive(Debug, Clone, PartialEq)]`，两个变体 `Dns { targets: Vec<String>, family: String }`、`Mac { guid: String, mac_no_dash: String }`。
- `pub fn parse_helper_args(args: &[String]) -> Result<Option<(HelperOp, Option<String>)>, String>` — `helper/mod.rs:38-91`。语义：未出现 `--helper` → `Ok(None)`；出现但参数非法 → `Err`（调用方不启动正常应用）。解析规则：
  - `position(|a| a == "--helper")`（`:39-41`），取紧随其后的操作名，缺失 → `Err("helper 缺少操作类型")`（`:42-45`）；
  - 收集操作名之后、**第一个以 `--` 开头参数之前**的所有位置参数（`:46-51`）；
  - 之后顺序无关地扫描剩余参数：`--family <v>`（缺值 `Err`，默认 `"both"`，`:54-63`）与 `--result <path>`（缺值 `Err`，`:64-71`），其余参数 `i += 1` 跳过（`:72`）；
  - 操作名分派：`"dns"` → `HelperOp::Dns { targets: positional, family }`（`:76`）；`"mac"` → 位置参数 0 为 GUID、1 为 MAC，任一缺失 → `Err`（`:77-87`）；其他 → `Err("未知 helper 操作: {other}")`（`:88`）。
- `pub fn run_helper(op: HelperOp, result_path: Option<String>) -> i32` — `helper/mod.rs:94-104`。按变体调 `run_dns`/`run_mac`（`:96-99`）；`result_path` 为 `Some` 时写结果文件（`:100-102`）；**返回退出码 0（成功）/ 1（失败）**（`:103`）。
- `fn run_dns(targets: &[String], family: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:106-125`（私有）。写一条起始日志（空名单显示为"无"，`:107-110`），调 `network::dns_setup::setup_dns_doh_admin(targets, family)`（`:111`，安卓/非 Windows 版实体在 `network/dns_setup.rs:205`，Windows 版在 `:13`），取返回 JSON 的 `success`/`message`（缺省 `"设置DNS+DoH完成"`），把整个 JSON 塞进 `details`（`:118-124`）。
- `fn run_mac(guid: &str, mac_no_dash: &str, logs: &mut Vec<String>) -> HelperResult` — `helper/mod.rs:127-187`（私有）：
  - **MAC 格式前置校验**：必须 `len == 12` 且全部为 ASCII 十六进制字符，否则直接返回 `success:false` + `"MAC 格式非法: {mac}（要求 12 位十六进制字符、无分隔符）"`（`:129-137`）——防"非法格式静默写坏网卡配置"；
  - `network::get_adapters_force()` 枚举（`:139-150`），按 `guid.eq_ignore_ascii_case` 找适配器（`:151-162`）；两处失败都返回 `success:false`；
  - `network::dhcp::apply_mac_change_via_registry(guid, &adapter.name, mac_no_dash)`（`:163`）；成功后**在同一管理员上下文内**调 `network::dhcp::remove_mac_from_registry(guid)` 清除 `NetworkAddress` 持久伪装值，失败只 push 一条日志、**不影响 success**（`:168-170`）；返回 `"MAC已修改并重启网卡: {name}"`（`:173`）。
- `fn write_result_file(path: &str, result: &HelperResult)` — `helper/mod.rs:190-197`（私有）。先写 `"{path}.tmp"` 再 `std::fs::rename` 覆盖（原子写，避免主进程读到半截）；**两步的返回错误都被忽略**（`if ... is_ok()`，`:193-195`）。
- 模块头注释 `helper/mod.rs:1-10` 声明两条关键约定：① 提权由主进程用 `ShellExecuteW(runas)` / COM ICMLuaUtil 启动当前 exe 并附 `--helper <op>`；② **helper 进程不初始化 logger**（避免与主进程跨进程写同一日志文件竞争），诊断信息只进 `HelperResult::logs`，由主进程读取后统一落日志。
- `#[cfg(test)]` 用例 7 个：`parse_no_helper_returns_none`（`:204-207`）、`parse_dns_with_result`（`:210-220`）、`parse_dns_with_targets_and_family`（`:223-243`）、`parse_mac`（`:246-261`）、`parse_mac_missing_args_is_err`（`:264-267`）、`parse_unknown_op_is_err`（`:270-273`）、`helper_result_roundtrip`（`:276-290`）、`helper_result_omits_empty_logs_and_details`（`:293-304`）。

### `update/mod.rs`

- `update/mod.rs:1` 仅一行 `pub mod updater;`，无其他内容。

### `update/updater.rs` 常量与静态

| 名称 | 值 | 位置 | 可见性 |
|---|---|---|---|
| `GITHUB_REPO` | `"ikliml666/Wxxy-CampusLogin"` | `update/updater.rs:8` | 私有 |
| `AUTO_CHECK_INTERVAL_SECS` | `86400`（24h） | `update/updater.rs:9` | 私有 |
| `STARTUP_CHECK_DELAY_SECS` | `5` | `update/updater.rs:11` | 私有 |
| `VERSION_FILE_URL` | `https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json` | `update/updater.rs:12` | 私有 |
| `VERSION_MIRRORS` | 3 项：`ghfast.top`、`gh-proxy.com`、`ghproxy.net` 前缀 + 官方 raw URL | `update/updater.rs:15-19` | 私有 |
| `SHORT_TIMEOUT_CLIENT` | `OnceLock<Result<reqwest::Client, String>>` | `update/updater.rs:80` | 私有 static |

### `update/updater.rs` 函数

- `pub fn compare_versions(current: &str, latest: &str) -> bool` — `:46-73`。逐段按 `u32` 比较，`trim_start_matches('v')` 后按 `.` 切分，每段取**前导 ASCII 数字**（`"2-beta"` → `2`，解析失败按 0）；比较长度取两侧最大（`:61`），缺失段按 0（`:63-64`）；`latest > current` 才返回 true。3 个用例在 `:466-480`（含 4 段 hotfix `2.3.0.1`）。
- `fn build_short_timeout_http_client() -> Result<&'static reqwest::Client, String>` — `:82-94`（私有）。`get_or_init` 缓存 `Result`（工具链 `get_or_try_init` 未稳定），超时 **10 秒**；注释说明**不接入** `network/client.rs` 的 `CLIENT_POOL`（池内客户端强制 `.no_proxy()`，会改变更新下载对系统代理的语义）。
- `pub fn decide_checksum_missing(all_client_errors: bool, had_transport_error: bool, allow_skip: bool) -> Result<(), String>` — `:101-115`。`all_client_errors && !had_transport_error` 且 `allow_skip` → `Ok(())` 降级通过（`:102-109`）；同条件但未开跳过 → `Err("校验和源全部不可用（4xx），且未开启 skipSha256WhenMissing，拒绝安装未校验的安装包")`（`:110`）；其余（含 5xx / 传输错误）→ `Err("所有校验和源均失败（含 5xx/网络错误），拒绝安装")`（`:113`）。3 个用例在 `:459-464`、`:497-510`。
- `fn extract_checksum(url: &str, text: &str) -> Option<String>` — `:122-150`（私有）。URL 含 `api.github.com` 时按 JSON 处理：遍历 `assets`，找 `name` 以 `.exe` 结尾的项，取其 `digest` 去掉 `sha256:` 前缀且为 64 位十六进制（`:126-139`）；否则按纯文本：取首个空白分隔 token，是 64 位十六进制即用（shasum 风格，`:141-145`），否则取最后一个 `=` 之后 trim 的 token（BSD 风格，`:146-148`）。返回值统一 `to_lowercase()`。4 个用例在 `:483-494`、`:515-546`。
- `pub async fn verify_download_sha256(file_path: &str, checksum_urls: &[String], allow_skip_missing: bool) -> Result<bool, String>` — `:152-231`：
  - 空 URL 列表 → `Err("未提供校验和URL")`（`:153-155`）；
  - 顺序尝试每个 URL（`:165-196`）：成功响应（`is_success`）→ 取文本 → `extract_checksum` 命中即 `break`；响应非成功 → 记录 `"HTTP {status}"`，且**非 4xx 时**把 `all_client_errors` 置 false（`:184-187`）；`send` 失败 → 置 `all_client_errors=false` 且 `had_transport_error=true`（`:189-194`）；
  - 全部失败 → 调 `decide_checksum_missing(...)`，`Ok(())` 时**直接返回 `Ok(true)`**（`:203`），`Err` 时包装最后错误返回（`:201-202`）；
  - 计算实际哈希：`tokio::task::spawn_blocking` 内以 **64KB buffer** 流式读取（`:211-228`，注释说明避免 50MB+ 安装包一次性读入造成内存峰值）；最终返回 `actual_hash == expected_hash`（`:230`）。
- `pub fn schedule_update_cleanup()` — `:233-244`。`spawn` 一个任务：`sleep(24 * 3600)` 后 `spawn_blocking(remove_dir_all(temp_dir))`，`temp_dir = %TEMP%/campus-login-update`（`:234`）；完成后 `log_debug!("updater", "更新临时目录已清理")`（`:242`）。注释说明原值为 600s，因用户可能在 UAC 等待/稍后安装时被提前删除，改 24h（`:236-237`）。
- `pub fn start_update_check_loop(app_handle: &tauri::AppHandle)` — `:246-278`。任务名 `update_check_loop`（`:249`），注册失败只 `log_warn!`（`:276`）。流程：先 `select!` 等 `STARTUP_CHECK_DELAY_SECS`（5s）或 cancel（`:253-256`）；进入 `loop` 判 cancel / `is_quitting`（`:258-260`）→ `do_update_check`（`:261`）→ **5 秒步进**累计到 `AUTO_CHECK_INTERVAL_SECS`（86400，`:264-273`），步进间同时监听 cancel 与 `is_quitting`。调用方 `app/startup.rs:193`。
- `async fn do_update_check(app_h: &tauri::AppHandle, state: &AppState)` — `:281-313`（私有）。`mirror_first = config.update_source != "github"`（`:282`，默认 `update_source = "mirror"`，见 `config/model.rs:198`，即默认镜像优先）；`check_update_inner` 成功后发 `emit_update_available(has_update, latest_version, release_notes)`（`:285-289`）；有更新且 `update_stats.update_notified` 的 `compare_exchange(false, true, Acquire, Relaxed)` 成功时弹通知（`:292`）：Windows 桌面走 `platform::toast::show_update_toast`（可点击跳转关于界面）**失败降级**为 `emit_notification(app_h, "发现新版本", body, "mascot-update")`（`:295-299`，`#[cfg(all(desktop, target_os = "windows"))]`），非该组合直接走普通通知（`:300-301`）；最后写 `last_update_check_epoch_ms`（`:303-307`）。失败只 `log_warn!`（`:309-311`）。
- `pub async fn fetch_latest_release(mirror_first: bool) -> Result<(bool, String, String, Option<String>), String>` — `:317-343`。按 `mirror_first` 排源顺序（`:318-322`，镜像优先时 `[M0, M1, M2, 官方]`，否则 `[官方, M0, M1, M2]`），顺序尝试 `fetch_version_from_url`；首个成功即返回（降级生效时 `i > 0` 记 `log_info!`，`:328`）；全失败返回 `Err("所有更新源均失败（首选: {primary_err}）")`（`:342`）。
- `async fn fetch_version_from_url(url: &str) -> Result<(bool, String, String, Option<String>), String>` — `:345-384`（私有）。带 `User-Agent: CampusLogin-UpdateChecker` 请求；非 2xx → `Err("version.json不可用: HTTP {status}")`；`data["version"]` 去 `v` 前缀，空 → `Err("version.json中缺少版本号")`（`:364-372`）；`current = env!("APP_VERSION")`（编译期由 `tauri-app/src-tauri/build.rs:37` 从 `tauri.conf.json` 注入）；`has_update = compare_versions(current, &latest_tag)`；`notes` 缺省 `""`（`:378`），`asset` 缺省 `None`（`:381`）。返回四元组 `(has_update, latest_tag, notes, asset)`。
- `pub async fn check_update_inner(mirror_first: bool) -> Result<UpdateInfo, String>` — `:388-452`。步骤：
  - 拉取版本（`:389`）；
  - 安装包名：`asset.unwrap_or_else(|| format!("Wxxy-CampusLogin_{latest_tag}_x64-setup.exe"))`（`:391`）——仓库根 `version.json` 当前只有 `{"version": "v2.3.6"}`，所以实际走回退命名；
  - 构造 `github_exe_url = https://github.com/{GITHUB_REPO}/releases/download/v{tag}/{exe_name}`（`:392-394`）；
  - **资产存在性探测**：`client.head(&github_exe_url)` 返回 `404` 时把 `has_update` 置 false 并 `log_warn!`（`:408-410`，防"version.json 先行、Release 未发布"导致用户下载必败）；其他状态码视为存在；`Err`（网络失败）**保守维持** `has_update`（`:413-415`）；
  - 构造 5 个校验和 URL（`:421-432`）：GitHub API `releases/tags/v{tag}`、`{github_exe_url}.sha256`、3 个镜像前缀版本；
  - 组装 `UpdateInfo`（`:434-451`）：`assets` 固定两个条目（exe 与 `"{exe_name}.sha256"`，`size` 均为 `0`），`sha256_checksum = Some(serde_json::to_string(&sha256_urls))`（`:450`，即把 5 个 URL 序列化成 JSON 字符串放在该字段里）。
- `#[cfg(test)]`：常量 `HASH_LOWER`（`:512`）与 8 个用例（`:458-546`）。

### 主进程侧辅助链路（本模块的调用方，供定位用）

- `platform/helper_spawn.rs:13-20` `unique_result_path()`：`%TEMP%/campus-login-helper-<pid>-<毫秒时间戳>.json`。
- `platform/helper_spawn.rs:23-35` `read_helper_result`：解析 JSON → 把 `logs[]` 逐条 `log_info!("helper", ...)` → 删除结果文件。
- `platform/helper_spawn.rs:41-80` `spawn_elevated_helper(op, args, result_path, timeout)`：拼 `--helper {op} "{arg}"... --result "{path}"`（所有位置参数强制加引号，`:52-58`）；先试 COM `shell_exec_elevated(..., true)` 静默提权，失败降级 `run_elevated`（弹 UAC，`:61-64`）；随后 **100ms 间隔轮询**结果文件直到 `timeout`（`:67-73`），超时后**再查一次**（`:76-78`，避免恰好越过 deadline 的写入被误报超时），仍未读到 → `Err("提权操作超时，未收到helper结果")`。
- 两个实际调用点与超时：`commands/network_cmd.rs:302-313`（DNS 提权分支，`Duration::from_secs(30)` 在 `:312`）、`network/dhcp.rs:287-295`（MAC 提权分支，`Duration::from_secs(25)` 在 `:294`）；MAC 成功返回前还会 `poll_ip_change(..., 25_000)` 等 IP 变更（`network/dhcp.rs:308`）。
- 管理员直通路径：`commands/network_cmd.rs:298-300`（`is_admin()` → 直接 `setup_dns_doh_admin`）、`network/dhcp.rs:278-285`（`is_admin()` → `set_mac_via_registry`）。

## 结构体与字段

### `HelperResult`（`helper/mod.rs:14-24`）

`#[derive(Debug, Clone, Serialize)]`；序列化键名**不加** `camelCase`，即字段名原样。

| 字段 | 类型 | 序列化 | 含义 |
|---|---|---|---|
| `success` | `bool` | 始终输出 | 操作是否成功；同时决定 `run_helper` 的退出码 |
| `message` | `String` | 始终输出 | 面向用户/日志的中文结果消息 |
| `op` | `String` | 始终输出 | 操作名回显（`"dns"` / `"mac"`），由各自 `run_*` 写入 |
| `logs` | `Vec<String>` | `#[serde(skip_serializing_if = "Vec::is_empty")]`（`:19`） | helper 进程内的诊断日志（因不初始化 logger），主进程读取后逐条落日志 |
| `details` | `Option<serde_json::Value>` | `#[serde(skip_serializing_if = "Option::is_none")]`（`:22`） | 完整操作明细；DNS 为 `setup_dns_doh_admin` 的整个 JSON（含 `dnsSuccess`/`dnsFailed`/`dohAdded`/`dohFailed`），MAC 恒为 `None` |

### `HelperOp`（`helper/mod.rs:27-34`）

`#[derive(Debug, Clone, PartialEq)]`，不序列化。

| 变体 | 字段 | 类型 | 含义 |
|---|---|---|---|
| `Dns` | `targets` | `Vec<String>` | 目标适配器名列表（由主进程 `resolve_adapter_names` + `filter_operation_adapters` 解析后传入；helper 不做范围判断） |
| | `family` | `String` | 优化目标，`"ipv4"` / `"ipv6"` / `"both"`；未传 `--family` 时为 `"both"`（`:54`） |
| `Mac` | `guid` | `String` | 适配器 GUID（位置参数 0），匹配时忽略大小写（`helper/mod.rs:151`） |
| | `mac_no_dash` | `String` | 12 位无分隔符十六进制 MAC（位置参数 1），格式在 `helper/mod.rs:129` 校验 |

### `ReleaseAsset`（`update/updater.rs:21-26`）

`#[derive(Debug, Serialize, Deserialize, Clone)]`，无 `camelCase`。

| 字段 | 类型 | 含义 |
|---|---|---|
| `name` | `String` | 资产文件名 |
| `url` | `String` | 资产地址 |
| `size` | `u64` | 字节大小；`check_update_inner` 构造时恒为 `0`（`:440`、`:447`），前端无法据此预知安装包体积 |

### `UpdateInfo`（`update/updater.rs:28-36`）

`#[derive(Debug, Serialize, Deserialize, Clone)]` + `#[serde(rename_all = "camelCase")]`，前端契约类型。

| 字段 | 类型 | JSON 键 | 含义 |
|---|---|---|---|
| `has_update` | `bool` | `hasUpdate` | 是否有更新；被 HEAD 探测（404 时）修正为 false |
| `latest_version` | `String` | `latestVersion` | 最新版本号，已去 `v` 前缀 |
| `release_notes` | `String` | `releaseNotes` | version.json 的 `notes`，缺省空串 |
| `assets` | `Vec<ReleaseAsset>` | `assets` | 固定 2 项：安装包与 `"{exe}.sha256"`（后者 `url` 实际是校验和 URL 列表首项） |
| `sha256_checksum` | `Option<String>` | `sha256Checksum` | 5 个校验和 URL 的 JSON 数组字符串（`:450`）；`commands/updater.rs:214-227` 解析它传给 `install_update` |

### `DownloadProgress`（`update/updater.rs:38-44`）

`#[derive(Debug, Serialize, Deserialize, Clone)]`，无 `camelCase`；本模块只定义不发，实际由 `commands/updater.rs:150-155`（≥200ms 节流发送）与 `:181-186`（结束时 `percent: 100.0`、`speed: 0`）通过 `notify_update_download_progress` 发出。

| 字段 | 类型 | 含义 |
|---|---|---|
| `downloaded` | `u64` | 已下载字节数 |
| `total` | `u64` | 总字节数（响应无 `Content-Length` 时为 `0`，此时 `percent` 固定 `0.0`） |
| `speed` | `u64` | 本区间速度（字节/秒），按 `(downloaded - last_downloaded) / elapsed` 估算 |
| `percent` | `f64` | 百分比；`total > 0` 时为 `downloaded / total * 100.0`，否则 `0.0` |

### 结果文件契约（helper → 主进程）

`HelperResult` 的 JSON 即结果文件内容（`helper/mod.rs:190-196`）。主进程侧读取行为：`logs[]` 逐条进日志、随后删除文件（`platform/helper_spawn.rs:23-35`）；文件不存在时按超时处理（`:76-79`）。DNS 路径下主进程还会把 `details` 的对象成员**提升到顶层**，使返回结构与管理员直通路径一致（`commands/network_cmd.rs:315-328`）。

## Data Flow

### helper：提权往返链路

```
用户点击"设置 DNS"/"修改 MAC"
  ├─ is_admin() → 直接在进程内执行（commands/network_cmd.rs:299-301；network/dhcp.rs:277-286）
  └─ 非管理员
     → unique_result_path()                        platform/helper_spawn.rs:13
     → spawn_elevated_helper(op, args, path, t)     platform/helper_spawn.rs:41
        → "--helper dns|mac <位置参数> [--family v] --result <path>"
        → COM ICMLuaUtil 静默提权（失败降级 ShellExecuteW runas 弹 UAC）
     → 提权副本进程启动（同一 exe）
        → main.rs:30  parse_helper_args（早于 Tauri Builder / 单实例 / 托盘）
           ├─ Ok(None)      → 继续正常应用启动（main.rs:34）
           ├─ Ok(Some(op,path)) → std::process::exit(run_helper(op, path))   main.rs:31-33
           └─ Err(e)        → eprintln + exit(2)，不启动 UI                     main.rs:35-38
        → run_helper                                    helper/mod.rs:94
           ├─ Dns → run_dns → dns_setup::setup_dns_doh_admin(targets, family)  helper/mod.rs:106-125
           └─ Mac → run_mac → 格式校验 → get_adapters_force → 按 GUID 查找
                    → dhcp::apply_mac_change_via_registry
                    → dhcp::remove_mac_from_registry（清除持久伪装值）          helper/mod.rs:127-187
        → write_result_file（.tmp → rename 原子替换）    helper/mod.rs:190-197
        → 退出码 0（成功）/ 1（失败）                     helper/mod.rs:103
     → 主进程 100ms 轮询结果文件（超时后再补查一次）      platform/helper_spawn.rs:66-78
        → read_helper_result：logs 进日志、删文件、返回 JSON
  → 调用方按 JSON 的 success/message/details 决定 UI 表现与后续动作
     （MAC：network/dhcp.rs:300-320 之后还会 poll_ip_change 最多 25s）
```

超时值：DNS **30s**（`commands/network_cmd.rs:312`）、MAC **25s**（`network/dhcp.rs:294`）。helper 自身没有任何自我超时。

### update：周期自检链路

```
app/startup.rs:193  start_update_check_loop
  → 任务 "update_check_loop"
  → sleep 5s（STARTUP_CHECK_DELAY_SECS，可与 cancel 竞争）
  → loop
     → do_update_check
        → mirror_first = (config.update_source != "github")        update/updater.rs:282
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
             Windows 桌面：platform::toast::show_update_toast（失败降级 emit_notification）  updater.rs:295-301
             其他：emit_notification("发现新版本", ..., "mascot-update")
        → update_stats.last_update_check_epoch_ms = now
     → 5s 步进 × 86400s（每步监听 cancel / is_quitting）
```

手动检查走 `commands/updater.rs:8-22`（`check_update` 命令，同样 `check_update_inner`，并写 `last_update_check_epoch_ms`）。

### update：下载 → 校验 → 安装 → 清理链路

```
checked via commands/updater.rs
  check_update（:8-22）→ UpdateInfo 给前端
  download_update（:24-192）
     → is_downloading 互斥（:31-32）
     → 只允许 https + 域名白名单（13 项，含 github.com / ghfast.top / gh-proxy.com / ghproxy.net / moeyy.cn 等，:39-54）
     → 文件名取自 URL 末段，清洗 Windows 非法字符、含 ".." 或空则回退 "update.exe"
     → 目标：%TEMP%/campus-login-update/<filename>（:76-78）
     → MAX_DOWNLOAD_SIZE = 500MB（声明在 :74，响应头与逐块写入前各判一次）
     → reqwest 客户端：connect_timeout 30s / total timeout 1800s（:81-82）
     → tokio::fs 流式写；每 ≥200ms 或收满时发 DownloadProgress（:138 判定，:150-155 发送）
  install_update（:195-292）
     → 路径必须 canonicalize 后位于 %TEMP%/campus-login-update 之内（先于校验）
     → checksum_url 缺失/空 → Err("未提供SHA256校验和，安装已阻止")（:238-243）
     → verify_download_sha256(file_path, urls, config.skip_sha256_when_missing)（:222-227）   update/updater.rs:152
        ├─ Ok(true)  → 通过（可能来自"允许跳过且全部 4xx"的降级）  updater.rs:203
        ├─ Ok(false) → 删文件 + Err("安装包校验失败：SHA256不匹配，文件可能被篡改")
        └─ Err(e)    → 删文件 + Err("SHA256校验过程失败，安装已阻止: {e}")
     → 扩展名分派：exe → open::that；msi → msiexec /i（raw_arg，路径含引号则拒绝）
     → 成功后 schedule_update_cleanup()（24h 后删 %TEMP%/campus-login-update）  updater.rs:233
```

### SHA256 多源解析与降级决策

```
校验源顺序（update/updater.rs:421-432）:
  1) https://api.github.com/repos/{GITHUB_REPO}/releases/tags/v{tag}   → JSON，取首个 .exe 资产的 digest
  2) {github_exe_url}.sha256                                            → 文本，首个 64 位 hex token 或 "=" 后 token
  3) ghfast.top / gh-proxy.com / ghproxy.net 前缀的 (2)
决策（update/updater.rs:101-115）:
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
| 启动延迟首查 | `5` s | `update/updater.rs:11` |
| 自检周期 | `86400` s（24h，5s 步进等待） | `update/updater.rs:9`、`:264-272` |
| 更新检查 / 哈希请求超时 | `10` s | `update/updater.rs:87` |
| 下载 connect / total 超时 | `30` s / `1800` s | `commands/updater.rs:81-82` |
| 下载进度事件节流 | `200` ms | `commands/updater.rs:147` |
| 单文件下载上限 | `500 * 1024 * 1024` | `commands/updater.rs:73` |
| 下载互斥 | `tasks.is_downloading` | `commands/updater.rs:33` |
| 临时目录清理延迟 | `24 * 3600` s | `update/updater.rs:238` |
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
8. **临时更新目录没有启动期清理** — 仅 `update/updater.rs:233-244` 的进程内 `sleep(24h)` 会删 `%TEMP%/campus-login-update`；应用在 24 小时内退出（或安装后用户一直不重启）时该目录残留，仓库内除 `commands/updater.rs:76`、`:202` 与 `update/updater.rs:234` 外没有任何扫尾逻辑（`grep "campus-login-update"` 只有这三处）。
9. **`write_result_file` 忽略全部 IO 错误** — `helper/mod.rs:190-197`（写入与 rename 都不检查）。写失败时主进程只能等到 25s/30s 超时（`platform/helper_spawn.rs:79`），而 helper 侧因不初始化 logger（`helper/mod.rs:9-10`）不留任何落盘痕迹。
10. **MAC 伪装值清除失败仍报成功** — `helper/mod.rs:168-176`：`remove_mac_from_registry` 失败只在 `logs` 里 push 一条中文提示，`success` 仍为 `true`。用户看到"MAC已修改并重启网卡"，但注册表 `NetworkAddress` 残留会使伪装 MAC 在重启后继续生效（该注释本身指出这是"用户无从恢复"的场景）。
11. **helper 无自我超时、无重入保护** — `helper/mod.rs:94-104` 一旦进入 `run_dns`/`run_mac` 就同步跑到结束；两个并发提权副本写同一 `--result` 路径时后写覆盖先写（路径含 pid+毫秒时间戳，`platform/helper_spawn.rs:13-19`，实际碰撞概率低但无锁）。
12. **`--family` 对 mac 操作静默生效为 `"both"` 且被忽略** — `helper/mod.rs:54` 默认值 + `:96-99` 分派只看变体，`HelperOp::Mac` 不携带 family；若调用方误传 `--family` 不会有任何提示。
13. **`run_dns` 不做 targets 空校验** — `helper/mod.rs:106-125` 空名单会原样传给 `setup_dns_doh_admin`；目前安全性靠调用方 `commands/network_cmd.rs:288-294` 提前拦截空 `targets`，helper 侧无二次防线。
14. **GUID 未经格式校验** — `helper/mod.rs:151` 只做 `eq_ignore_ascii_case` 查找，找不到即返回 `"未找到GUID对应的适配器"`；与 MAC 的严格 12 位校验（`:129`）不对称，恶意/错误 GUID 会让 helper 在管理员上下文内遍历全部适配器（只读，无写副作用）。
15. **更新提示只弹一次** — `update/updater.rs:292` 用 `update_stats.update_notified` 的 `compare_exchange(false, true, ...)` 做一次性门控，且**全仓库没有任何地方把它重置为 false**（`grep update_notified` 仅 `infra/state/mod.rs:88`、`:113` 的定义/初值与此处）。用户忽略首次通知后，本进程生命周期内不会再收到提醒（前端仍可通过 `emit_update_available` 显示状态）。
16. **HEAD 资产探测对镜像不通的环境偏保守** — `update/updater.rs:400-417`：探测请求失败（`Err`）时日志打 `log_debug!`（`:414`）并**维持** `has_update = true`，于是纯镜像网络下用户可能收到更新通知但下载走不到官方源（`download_update` 的 URL 由前端从 `assets[0].url` 选镜像，见 `commands/updater.rs:294-330` 的 `get_mirror_urls`）。
17. **`do_update_check` 失败静默** — `update/updater.rs:309-311` 只有一行 `log_warn!("updater", "更新检查失败: {}", e)`，不向前端发事件、不写 `last_update_check_epoch_ms`（`:303-307` 仅在成功分支），设置界面的"上次检查时间"会停留在旧值。
18. **`start_update_check_loop` 注册失败只告警** — `update/updater.rs:276`；`app/startup.rs:193` 的返回值被完全忽略，自动更新静默失效时无 UI 反馈。
