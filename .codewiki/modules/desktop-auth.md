---
title: 桌面端认证（登录/注销/Portal 探测/失败计数）
type: module
source_files:
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/auth/portal.rs
  - tauri-app/src-tauri/src/auth/session.rs
  - tauri-app/src-tauri/src/auth/service.rs
  - tauri-app/src-tauri/src/auth/failure_tracker.rs
  - tauri-app/src-tauri/src/auth/dual_adapter_executor.rs
  - tauri-app/src-tauri/tests/repro_logout_panic.rs
tags: [认证, Portal, Dr.COM, 双适配器, 失败计数, 跨平台, 桌面端]
---

## Overview

本模块是校园网 Dr.COM / ePortal 认证协议的唯一实现点：构造并发送登录（`/eportal/portal/login`）、Radius 注销（`/eportal/portal/logout`）、MAC 解绑（`/eportal/portal/mac/unbind`）三类 HTTP 请求，解析 JSONP 响应为统一的 `{code, message, success, retryable}` 结果，并做 Portal 可达性/登录状态探测。上层由 `service` 负责适配器解析、双适配器错峰并行与结果合并，由 `session` 负责把结果落成登录历史与前端事件，由 `failure_tracker` 做连续认证失败计数与 MAC 重置自愈。

模块边界按 `cfg(desktop)` 一分为二：`protocol.rs` / `portal.rs` / `failure_tracker.rs` 无条件编译（安卓端经 `campus-login` path 依赖直接复用，见 `tauri-app/src-tauri/src/auth/mod.rs:1-3`）；`session.rs` / `service.rs` / `dual_adapter_executor.rs` 仅桌面编译（`mod.rs:6-11`），因为它们依赖适配器发现（`GetAdaptersAddresses`）、托盘与 Tauri `AppHandle` 命令面。

## Key Components

### 模块声明（auth/mod.rs）

| 位置 | 可见性 | 项 | 说明 |
| --- | --- | --- | --- |
| `auth/mod.rs:1` | pub | `mod failure_tracker` | 认证失败计数与 MAC 重置，跨平台 |
| `auth/mod.rs:2` | pub | `mod portal` | Portal 页面探测 + URL/脱敏/截断工具，跨平台 |
| `auth/mod.rs:3` | pub | `mod protocol` | 登录/注销协议请求与响应解析，跨平台 |
| `auth/mod.rs:6-7` | pub | `mod dual_adapter_executor` | `#[cfg(desktop)]`，双适配器并行执行器 |
| `auth/mod.rs:8-9` | pub | `mod session` | `#[cfg(desktop)]`，单适配器动作 + 日志/历史包装 |
| `auth/mod.rs:10-11` | pub | `mod service` | `#[cfg(desktop)]`，登录/注销顶层编排 |

本模块未定义任何 `trait`。

### auth/protocol.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `protocol.rs:3` | 私有 const | `LOGOUT_PLACEHOLDER_ACCOUNT: &str = "drcom"` | 注销请求里固定填充的 `user_account` 占位值 |
| `protocol.rs:4` | 私有 const | `LOGOUT_PLACEHOLDER_PASSWORD: &str = "123"` | 注销请求里固定填充的 `user_password` 占位值 |
| `protocol.rs:6-11` | pub fn | `random_v() -> String` | 生成 1000~9999 的 `v` 查询参数（拦缓存），种子取 `SystemTime` 纳秒 |
| `protocol.rs:16-37` | pub(crate) fn | `read_bounded_body(resp: reqwest::Response, label: &str) -> String` | 读响应体并硬限制 1MB（函数内 `MAX_BODY` 于 `protocol.rs:17`）；按 Content-Type charset 解码；读取失败/超限返回空串并 `log_warn!` |
| `protocol.rs:40-45` | 私有 fn | `content_type_charset(resp) -> Option<String>` | 从 Content-Type 提取 charset 参数（UTF-8 响应无该参数时 None） |
| `protocol.rs:50-84` | 私有 fn | `jsonp_json_slice(&str) -> &str` | 花括号平衡（字符串/转义感知）截取第一个完整 JSON 对象；非 JSONP 响应回退原文 |
| `protocol.rs:87-96` | 私有 fn | `wait_cancellable(duration_ms: u64, is_quitting: &AtomicBool) -> bool` | 每 100ms 检查退出标志的可中断等待；返回 false 表示已取消 |
| `protocol.rs:98-153` | 私有 fn | `do_login_request(user, password, operator, adapter_ip: Option<&str>) -> Result<serde_json::Value, String>` | 单次登录请求：校验凭据 → 拼 URL → 发 GET → 限长读体 → 解析 |
| `protocol.rs:155-188` | pub fn | `do_login_with_retry(user, password, operator, adapter_ip, max_retries: u32, is_quitting: &AtomicBool) -> Result<serde_json::Value, String>` | 登录重试循环；成功或 `retryable=false` 立即返回；重试间隔 2000ms 可中断 |
| `protocol.rs:190-239` | 私有 fn | `parse_login_result(&str) -> Result<serde_json::Value, String>` | 解析登录 JSONP：按 `result` 0/1/2/3/4 与 `msg` 关键词映射为 `{code, message, success, retryable}` |
| `protocol.rs:241-355` | 私有 fn | `do_logout_request(user, adapter_ip, is_quitting) -> Result<serde_json::Value, String>` | 单次注销：Radius 注销最多 2 轮（`logout` 优先）→ 成功后补一次 MAC 解绑 |
| `protocol.rs:361-368` | pub fn | `merge_logout_results(any_radius_ok: bool, any_unbind_ok: bool) -> &'static str` | 合并 Radius 注销与 MAC 解绑的结果文案（4 种组合） |
| `protocol.rs:373-375` | 私有 fn | `ip_to_eportal_int(ip: &str) -> Option<u32>` | 点分 IPv4 → ePortal 前端 `ip_to_int` 同语义的大端整数；非法/空返回 None |
| `protocol.rs:377-409` | pub fn | `do_logout_with_retry(user, adapter_ip, max_retries: u32, is_quitting: &AtomicBool) -> Result<serde_json::Value, String>` | 注销重试循环，语义同登录侧 |
| `protocol.rs:411-458` | 私有 fn | `parse_logout_result(&str) -> Result<serde_json::Value, String>` | 解析注销响应；JSON 解析失败时回退 HTML/文本关键词（"注销成功"/"下线成功"/"已下线"/"解绑成功"） |

### auth/portal.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `portal.rs:4-18` | 私有 mod | `portal_config` | 常量集合，模块本身私有（其 `pub const` 仅在本文件可见） |
| `portal.rs:8` | 私有 const | `CLIENT_TIMEOUT: Duration = 8s` | HTTP 客户端创建超时 |
| `portal.rs:10` | 私有 const | `REQUEST_TIMEOUT: Duration = 3s` | 单次页面探测请求超时 |
| `portal.rs:13` | 私有 const | `PAGE_INDICATOR_LOGOUT = "Dr.COMWebLoginID_1"` | 已在线页面特征 |
| `portal.rs:14` | 私有 const | `PAGE_INDICATOR_LOGIN_0 = "Dr.COMWebLoginID_0"` | 未登录页面特征 |
| `portal.rs:15` | 私有 const | `PAGE_INDICATOR_LOGIN_2 = "Dr.COMWebLoginID_2"` | 未登录页面特征 |
| `portal.rs:16` | 私有 const | `PAGE_TITLE_LOGOUT = "<title>注销页</title>"` | 已在线页面标题特征 |
| `portal.rs:17` | 私有 const | `PAGE_TITLE_LOGIN = "<title>登录页</title>"` | 未登录页面标题特征 |
| `portal.rs:32-38` | 私有 fn | `block_on_http<F: Future>(future: F) -> F::Output` | 同步上下文驱动 async HTTP；转发到 `infra::async_util::block_on_sync` |
| `portal.rs:42-53` | pub fn | `ensure_portal_port(base: &str) -> String` | 确保 Portal 地址带 `:801`；已带端口不动；含路径/非法 URL 用字符串兜底 |
| `portal.rs:55-65` | pub fn | `safe_truncate(s: &str, max_len: usize) -> &str` | 按字符边界截断（不切多字节字符），用于日志与错误文案 |
| `portal.rs:69-81` | pub fn | `redact_credentials(msg: String, url: &str, base_url: &str, password: &str) -> String` | 错误信息脱敏：完整 URL → `{base_url}?***`，URL 编码密码与明文密码 → `***` |
| `portal.rs:83-93` | pub struct | `PortalStatus` | Portal 探测结果（见"结构体与字段"） |
| `portal.rs:95-120` | pub fn | `check_portal_full(adapter_ip: &str, adapter_name: Option<&str>) -> Result<PortalStatus, String>` | Portal 状态探测入口：只做页面 GET，绝不携凭据调登录端点 |
| `portal.rs:129-139` | 私有 fn | `handle_unknown_page_status(adapter_name, adapter_ip) -> PortalStatus` | 页面无法识别时返回 `error_kind="need_manual_check"`，要求用户手动确认 |
| `portal.rs:142-148` | 私有 fn | `parse_adapter_ip(&str) -> Option<std::net::IpAddr>` | 空串/非法返回 None |
| `portal.rs:151-161` | 私有 fn | `build_determined_status(online: bool) -> PortalStatus` | 构造已判定状态（online=true 时 `login_available=false`） |
| `portal.rs:164-173` | 私有 fn | `build_request_failed_status() -> PortalStatus` | 构造请求失败状态（`reachable=false`, `error_kind="request_failed"`） |
| `portal.rs:175-197` | pub(crate) fn | `is_nat_private_ip(&str) -> bool` | NAT 私有网段判定：10./192.168./169.254./172.16-31./100.64-127.（用于注销不发送 `wlan_user_ip`） |
| `portal.rs:199-203` | 私有 enum | `PageCheckResult { Determined(bool), Unknown, Failed }` | 页面探测三态 |
| `portal.rs:205-242` | 私有 fn | `check_portal_page(client, portal_base) -> PageCheckResult` | 用配置原始地址发 GET（有意不探 `:801`，见 `portal.rs:206-213` 注释）；非 2xx 或空体判 Failed |
| `portal.rs:245-271` | 私有 fn | `analyze_portal_page_content(html) -> PageCheckResult` | 按特征串顺序匹配，再查用户会话特征，最后 Unknown |
| `portal.rs:274-279` | 私有 fn | `has_user_session_indicators(html) -> bool` | `uid='非空'` 或（`v4ip='非 0.` 且 `oltime=非 0`） |
| `portal.rs:283-286` | 私有 fn | `log_portal_query_start(adapter_name, adapter_ip)` | 查询开始 debug 日志（tag `network`） |
| `portal.rs:288-291` | 私有 fn | `log_portal_page_result(elapsed, adapter_name, adapter_ip, status)` | 探测结果 debug 日志（含耗时毫秒） |
| `portal.rs:293-296` | 私有 fn | `log_portal_page_failed(adapter_name, adapter_ip)` | 页面请求失败 debug 日志 |
| `portal.rs:298-301` | 私有 fn | `log_portal_no_credentials(adapter_name, adapter_ip)` | 无法判断且无凭据 debug 日志 |
| `portal.rs:303-306` | 私有 fn | `log_page_indicator_found(indicator, is_online)` | 命中特征串 debug 日志 |

### auth/session.rs（`#[cfg(desktop)]`）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `session.rs:11-75` | pub fn | `adapter_action_with_log<F>(adapter, config, app_handle, action_name, log_tag, action_type, do_action: F) -> Option<CommandResult>` | 单适配器动作通用包装：发"正在 XX"日志 → 执行 → 成功/失败发前端事件 + 写登录历史；`adapter.ip` 为空返回 None |
| `session.rs:77-176` | pub fn | `login_adapter_with_log(adapter, config, app_handle, is_quitting) -> Option<CommandResult>` | 单适配器登录：Portal 预检直通 → `do_login_with_retry(..., 3, ...)` → `parse_error` 复核 → "已经在线"假成功复核 |

### auth/service.rs（`#[cfg(desktop)]`）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `service.rs:20-111` | pub fn | `full_login(state, app_handle, adapter_name: Option<&str>) -> CommandResult` | 登录顶层编排：读配置 → 取适配器 → 单适配器或双适配器路径 → 失败计数 |
| `service.rs:113-126` | pub fn | `logout_adapter_with_log(adapter, config, app_handle, is_quitting) -> Option<CommandResult>` | 单适配器注销，内层调 `do_logout_with_retry(..., 2, ...)` |
| `service.rs:128-214` | pub fn | `full_logout(state, app_handle, adapter_name: Option<&str>) -> CommandResult` | 注销顶层编排（结构与 `full_login` 对称） |
| `service.rs:218-249` | pub fn | `post_login_handler(app_handle, state)` | 登录成功后处理：解除注销保护期 → 500ms 后按需触发后台检查 → 按需触发自动退出 |

### auth/failure_tracker.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `failure_tracker.rs:6` | 私有 const | `MAX_FAILURES: u32 = 5` | 连续认证失败阈值，达到即触发 MAC 重置 + DHCP 续租 |
| `failure_tracker.rs:9` | 私有 const | `AUTH_FAILURE_CODES: &[&str] = &["ac_auth_failed", "1", "4"]` | 被认定为"认证失败"（而非网络错误）的 code 白名单 |
| `failure_tracker.rs:12-16` | pub enum | `AdapterFailureCounter { A1, A2 }` | 双适配器计数器标识（`#[derive(Clone, Copy)]`） |
| `failure_tracker.rs:19-25` | pub fn | `is_auth_failure(result: &CommandResult) -> bool` | 从 `result.data.code` 判定是否认证失败 |
| `failure_tracker.rs:27-33` | pub(crate) fn | `get_adapter_failure_count(state, counter) -> u32` | 读 A1/A2 认证失败计数快照 |
| `failure_tracker.rs:35-42` | pub(crate) fn | `set_adapter_failure_count(state, counter, value)` | 写 A1/A2 认证失败计数 |
| `failure_tracker.rs:47-95` | pub fn | `update_auth_failure_count(state, app_handle, cmd_result, campus_gw, adapter_name)` | 单适配器计数：成功清零；认证失败累加（`update_with_result` 原子读-增-判）；达 5 次对**该适配器**做 MAC 重置 + DHCP 续租 |
| `failure_tracker.rs:98-109` | pub fn | `update_dual_adapter_auth_failure(state, app_handle, r1, r2, a1_name, a2_name, campus_gw)` | 双适配器分别计数入口 |
| `failure_tracker.rs:112-179` | 私有 fn | `handle_single_adapter_failure(state, app_handle, result, adapter_name, campus_gw, counter)` | 单适配器计数实现（成功清零 / 失败累加 / 达阈值重置 MAC） |
| `failure_tracker.rs:182-191` | pub fn | `reset_all(state)` | 注销成功时清零 `portal_failure_count`、`a1/a2_auth_failure_count`、`prep_login_failures` |
| `failure_tracker.rs:194` | 私有 const | `PORTAL_REQUEST_FAILURE_THRESHOLD: u32 = 5` | Portal HTTP 请求失败阈值 |
| `failure_tracker.rs:204-281` | pub fn | `handle_portal_request_failure(state, app_handle, adapter_ref, adapter_ip, campus_gw, counter, adapter_label)` | 后台巡检的 Portal 请求失败计数：网关不可达则跳过并清零；可达则累加，达 5 次触发该适配器 MAC 重置 |

### auth/dual_adapter_executor.rs（`#[cfg(desktop)]`）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `dual_adapter_executor.rs:7-10` | pub struct | `DualAdapterResult` | 双适配器结果容器 |
| `dual_adapter_executor.rs:13-16` | pub fn | `DualAdapterResult::success(&self) -> bool` | 任一适配器成功即整体成功 |
| `dual_adapter_executor.rs:19-37` | pub fn | `DualAdapterResult::build_command_result(&self) -> CommandResult` | 合并两条消息为 `"a1_msg, a2_msg"`，`code` 为 `"0"`/`"1"` |
| `dual_adapter_executor.rs:46-89` | pub fn | `execute_dual<F1, F2>(a1_action: F1, a2_action: F2, is_quitting: Arc<AtomicBool>) -> DualAdapterResult` | 适配器 1 立即执行；适配器 2 延迟 1s（10×100ms 可中断）后启动；`spawn_blocking` panic 转为失败 `CommandResult` |

### 集成测试（tauri-app/src-tauri/tests/repro_logout_panic.rs）

| 位置 | 项 | 用途 |
| --- | --- | --- |
| `tests/repro_logout_panic.rs:10-45` | `#[test] fn repro_logout_page_check_panic()` | 回归锁定"注销后 sleep(1s) + 页面检测"路径：模拟 `main.rs` 装配 `build_runtime` + `tauri::async_runtime::set(handle)`，在 `spawn_blocking` 内的 `std::thread::scope` 裸子线程中先 `tauri::async_runtime::handle().inner().enter()` 再调 `campus_login_lib::auth::portal::check_portal_full`（`:26-34`）。需要真实校园网环境（`tests/repro_logout_panic.rs:6` 注释），对 `10.2.106.187` 发真实只读 GET |

## 结构体与字段

### `PortalStatus`（`portal.rs:83-93`）

`#[derive(Debug, Clone, serde::Serialize)]` + `#[serde(rename_all = "camelCase")]`，序列化到前端字段名为 `reachable` / `loginAvailable` / `online` / `message` / `dataLength` / `errorKind`。安卓端 `android/src-tauri/src/monitor_loop.rs:506,524,535` 与 `protocol_cmds.rs:143,152` 也直接消费该类型。

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `reachable` | `bool` | Portal 页面是否请求成功（HTTP 2xx 且非空体） |
| `login_available` | `bool` | 是否处于可登录态（已在线时为 `false`）；无法判断时故意保持 `true`，交由用户手动确认 |
| `online` | `bool` | 是否已登录 |
| `message` | `String` | 人类可读状态："已在线" / "未登录" / "Portal 页面无法判断登录状态，请手动确认" / "Portal页面请求失败" |
| `data_length` | `usize` | 预留字段：**当前所有构造路径都填 0**（`portal.rs:136`、`portal.rs:158`、`portal.rs:170`），无实际语义 |
| `error_kind` | `Option<String>` | 机器可读错误类型：`None`（已判定）/ `"need_manual_check"` / `"request_failed"`；`#[serde(skip_serializing_if = "Option::is_none")]` 为 None 时不出现 |

### `PageCheckResult`（`portal.rs:199-203`，私有）

| 变体 | 载荷 | 含义 |
| --- | --- | --- |
| `Determined` | `bool` | 已判定：`true`=已在线，`false`=未登录 |
| `Unknown` | — | 页面内容无任何已知特征，无法判断 |
| `Failed` | — | 请求失败 / 非 2xx / 响应体为空 |

### `DualAdapterResult`（`dual_adapter_executor.rs:7-10`）

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `primary` | `Option<CommandResult>` | 适配器 1 的结果；任务 panic 时被填为 `CommandResult::err("适配器操作任务异常退出")`（`dual_adapter_executor.rs:71-77`） |
| `secondary` | `Option<CommandResult>` | 适配器 2 的结果；退出流程启动时被跳过，保持 `None`（`dual_adapter_executor.rs:79-83`） |

### `AdapterFailureCounter`（`failure_tracker.rs:12-16`）

| 变体 | 含义 |
| --- | --- |
| `A1` | 对应 `state.network` 快照的 `a1_auth_failure_count` |
| `A2` | 对应 `state.network` 快照的 `a2_auth_failure_count` |

### 外部依赖类型 `CommandResult`（定义于 `tauri-app/src-tauri/src/infra/state/mod.rs:213-231`）

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `success` | `bool` | 操作是否成功 |
| `message` | `Option<String>` | 展示文案（`skip_serializing_if = "Option::is_none"`） |
| `data` | `Option<serde_json::Value>` | 协议原始结果，本模块约定键：`code`（`"0"`/`"1"`/`"2"`/`"3"`/`"4"`/`"ac_auth_failed"`/`"parse_error"`/`"unknown_failure"`/`"max_retries"`/`"error"`）、`message`、`retryable` |

### 协议结果 JSON 约定（`parse_login_result` / `parse_logout_result` 的产出）

| 键 | 类型 | 说明 |
| --- | --- | --- |
| `code` | `String` | 业务码；`"0"` 成功、`"1"` 非法/失败、`"2"` 已在线、`"3"` 流量超限、`"4"` 账号禁用、`"ac_auth_failed"` AC 认证失败、`"parse_error"` 无法解析、`"unknown_failure"` 未识别 msg、`"max_retries"` 重试耗尽、`"error"` 请求异常 |
| `message` | `String` | 展示文案（含服务端原文或本地兜底文案） |
| `success` | `bool` | 是否成功 |
| `retryable` | `bool` | 是否值得重试；缺失时调用方按 `true` 处理（`protocol.rs:168`、`protocol.rs:391`） |

## Data Flow

### 登录链路（单适配器）

```
命令层 commands/login.rs:74 / app/tray.rs:70 / monitor/auto_auth.rs:72,173,430
  → auth::service::full_login(state, app_handle, adapter_name)
      ├─ state.config.load() 校验 user/password 非空            (service.rs:21-28)
      ├─ network::get_adapters_cached() → 失败则 wait_for_adapter(10000, quit)  (service.rs:32-38)
      ├─ network::ensure_ethernet_ip_for_login(...)             (service.rs:44)
      ├─ network::get_adapters_force() 绕缓存重取（DHCP 续租后 IP 变化）(service.rs:48-51)
      ├─ 指定适配器分支：network::find_with_valid_ip → session::login_adapter_with_log
      └─ 自动分支：network::resolve_adapter_names + find_dual_adapters
             ├─ 单适配器：session::login_adapter_with_log(a1, ...)
             └─ 双适配器：dual_adapter_executor::execute_dual(
                    || login_adapter_with_log(a1,…),      // 立即
                    || login_adapter_with_log(a2,…),      // 1s 后
                    state.exit.is_quitting)
  → auth::session::login_adapter_with_log(adapter, config, app_handle, is_quitting)
      ├─ 预检 portal::check_portal_full(adapter.ip, Some(name))     (session.rs:87)
      │     └─ online==true → append_login_history(..., "login") + 直接返回成功
      ├─ auth::protocol::do_login_with_retry(user, password, operator,
      │        Some(adapter.ip), 3, is_quitting)                    (session.rs:111)
      │     └─ 每轮 auth::protocol::do_login_request(...)
      │           ├─ config::validate::{validate_username,validate_operator,validate_password}
      │           ├─ network::client::PORTAL_URL.load() → portal::ensure_portal_port → /eportal/portal/login
      │           ├─ network::client::create_safe_http_client(15s, local_addr)
      │           ├─ portal::redact_credentials(...) 脱敏错误串（失败分支）
      │           ├─ protocol::read_bounded_body 语义的内联限长读体（1MB）
      │           └─ parse_login_result(body) → {code,message,success,retryable}
      ├─ parse_error 且 retryable==true → 复核 check_portal_full，online 则改判成功  (session.rs:119-135)
      ├─ success 且 message 含"已经在线" → 复核 check_portal_full，探测不通则降级为失败
      │      （code="1"，进入失败计数触发 MAC 重置自愈）                (session.rs:144-173)
      ├─ adapter_action_with_log 包装：EventBus::emit_login_log + config::persist::append_login_history
      └─ 返回 Option<CommandResult>
  → auth::failure_tracker::update_auth_failure_count / update_dual_adapter_auth_failure
       成功 → 计数清零；认证失败 → 累加，达 MAX_FAILURES(5)
       → network::dhcp_release_renew_single(adapter_name, campus_gw) + emit_login_log
```

### 注销链路

```
commands/login.rs:100 → auth::service::full_logout(state, app_handle, adapter_name)
  → 适配器解析（与登录同构，无 ensure_ethernet_ip_for_login）      (service.rs:140-182)
  → auth::service::logout_adapter_with_log → auth::protocol::do_logout_with_retry(user, adapter_ip, 2, quit)
      └─ 每轮 auth::protocol::do_logout_request(...)
            ├─ portal::is_nat_private_ip(adapter_ip) → NAT 场景 wlan_user_ip 传空串
            ├─ 轮 1..2：GET /eportal/portal/logout?callback=dr100{round+2}
            │     account/password 用占位值 "drcom"/"123"（protocol.rs:273-274）
            │     成功即 break；轮 1 失败后等待 1.5s（可中断）
            ├─ Radius 全败或成功后收尾：GET /eportal/portal/mac/unbind
            │     wlan_user_ip 走 ip_to_eportal_int 转整数（NAT 空串回退原样）
            └─ merge_logout_results(any_radius_ok, any_unbind_ok)
                  → {"code": radius_ok?"0":"1", "success": any_radius_ok, "retryable": !any_radius_ok}
  → 命令层 logout.rs 成功后调用 failure_tracker::reset_all(state)
```

### Portal 探测链路（无凭据，只读）

```
monitor 后台巡检 / session 预检 / commands → portal::check_portal_full(adapter_ip, name)
  → network::client::create_safe_http_client(8s, local_addr)
  → check_portal_page: GET {portal_url}/ （原始端口，不加 :801）
       非 2xx → Failed；read_bounded_body 空 → Failed        (portal.rs:227-237)
  → analyze_portal_page_content(html)                        (portal.rs:245-271)
       命中特征串 → Determined(is_online)
       命中 uid/v4ip+oltime → Determined(true)
       否则 → Unknown → handle_unknown_page_status（need_manual_check）
```

## Connections

- [[desktop-commands]] — `commands/login.rs`（`full_login`/`full_logout`/`post_login_handler`/`reset_all`）、`commands/network_cmd.rs:105`（`config::model::default_campus_gateway`）、`commands/account.rs`（登录历史与账号切换）。
- [[desktop-monitor]] — `monitor/background_check.rs:193,201` 调 `handle_portal_request_failure`；`monitor/auto_auth.rs:72,173,430` 调 `full_login`；`monitor/watcher.rs::run_background_check` 由 `post_login_handler`（`service.rs:242`）触发。
- [[desktop-network-core]] — `network::client::{PORTAL_URL, create_safe_http_client}`、`network::{get_adapters_cached, get_adapters_force, wait_for_adapter, find_with_valid_ip, find_dual_adapters, resolve_adapter_names, ensure_ethernet_ip_for_login, dhcp_release_renew_single, check_gateway_reachable_from}`、`network::Adapter`。
- [[desktop-config]] — `config::validate::{validate_username, validate_operator, validate_password}`（协议层第二道校验）、`config::persist::append_login_history`、`config::model::Config`。
- [[desktop-infra]] — `infra::state::{AppState, CommandResult}`、`infra::events::EventBus`、`infra::async_util::block_on_sync`、`infra::lifecycle::start_auto_exit`、`infra::logger` 的 `log_*!` 宏（tag：`login` / `logout` / `network` / `background` / `auth`）、`platform::console_output::decode_charset_bytes`。
- 安卓端（`android/src-tauri/src/`）直接复用本模块的**跨平台子集**：`monitor_loop.rs:524,535` 与 `protocol_cmds.rs:152` 用 `check_portal_full`；`protocol_cmds.rs:73,109` 用 `do_login_with_retry` / `do_logout_with_retry`。`session.rs` / `service.rs` / `dual_adapter_executor.rs` 在安卓 target 不编译，安卓端有自己的监控循环（`monitor_loop.rs`）。

## Known Issues

1. **注销请求携带硬编码占位凭据**：`protocol.rs:3-4` 定义 `LOGOUT_PLACEHOLDER_ACCOUNT = "drcom"`、`LOGOUT_PLACEHOLDER_PASSWORD = "123"`，`do_logout_request` 在 `protocol.rs:273-274` 把它们填入 `user_account`/`user_password` —— 注销不校验身份凭据，只依赖 `wlan_user_ip`。若 Portal 侧收紧校验，注销会静默退化。
2. **注销轮次间隔只覆盖第 1 轮**：`protocol.rs:298-310` 的 1.5s 等待写死在 `if round == 1` 分支内，第 2 轮失败后直接进入 MAC 解绑，没有间隔（首次请求与解绑请求几乎同时发出）。同一处还有 `protocol.rs:300` 的 `for _ in 0..15` 手写循环，与 `wait_cancellable`（`protocol.rs:87-96`）功能重复但不复用。
3. **`wait_cancellable` 在 `duration_ms < 100` 时不等待**：`protocol.rs:88` 的 `let steps = duration_ms / 100;` 会得到 0，函数直接返回 `true`（未取消）。当前调用点都传 2000，暂未触发。
4. **`result == 1` 且 `msg` 为空被判定为登录成功**：`protocol.rs:210-215`，空 msg 时落到 `else` 分支返回 `"Portal协议认证成功"`。若 Portal 改版后 `result=1` 语义变化且不带 msg，会把失败误报为成功。
5. **`read_bounded_body` 的日志 tag 固定为 `logout`**：`protocol.rs:19` 与 `protocol.rs:26` 硬编码 `crate::log_warn!("logout", ...)`。该函数也被 Portal 页面探测复用（`portal.rs:234` 与 `protocol.rs:285`/`protocol.rs:334`），因此"Portal 页面响应体超限"会被错误归类到 `logout` 日志标签下，排障时按 tag 过滤会漏看。
6. **登录大响应体错误会被无谓重试**：`protocol.rs:138` 与 `protocol.rs:144` 返回 `Err("登录响应体过大")`，经 `do_login_with_retry` 的 `Err` 分支被包装为 `retryable: true`（`protocol.rs:178`），同一超大响应体会被重复请求 3 次。
7. **注销失败结果缺 `retryable` 字段**：`protocol.rs:398` 的 Err 分支构造的 JSON 只含 `code`/`message`/`success`，`do_logout_with_retry` 在 `protocol.rs:391` 用 `unwrap_or(true)` 兜底 → 恒为可重试（最多 `max_retries=2` 轮）。
8. **`PortalStatus.data_length` 恒为 0**：三个构造点 `portal.rs:136`、`portal.rs:158`、`portal.rs:170` 都写死 `0`，该字段已无生产意义但仍随 IPC 出站（camelCase `dataLength`）。
9. **Portal 页面特征全部硬编码 Dr.COM 文案**：`portal.rs:13-17` 的 5 个特征串 + `portal.rs:275-277` 的 `uid='` / `v4ip='` / `oltime=` 特征，Portal 改版即整体退化为 `Unknown`（`error_kind="need_manual_check"`）。历史上已因此踩坑，回退记录见 `portal.rs:206-213` 注释（v2.2.x 曾强制探 `:801` SPA 页面导致必然 Unknown）。
10. **页面探测端口与协议端口不一致（有意为之）**：`portal.rs:214` 用配置原始地址探测，而登录/注销强制 `:801`（`protocol.rs:104`、`protocol.rs:244`）。但 `config::validate::normalize_portal_url` 只归一化 `"http://10.1.99.100:801"` 这一个字面值（`validate.rs:81-85`），若用户填了其他带 `:801` 的地址，页面探测会打到 EPortal SPA 前端，必然返回 `need_manual_check`。
11. **`PageCheckResult` 与 `portal_config` 均为私有**：`portal.rs:4`、`portal.rs:199`，外部（含安卓端）只能拿到 `PortalStatus`，无法对页面判定细节做断言或复用。
12. **预检错误被静默吞掉**：`session.rs:87` 与 `session.rs:122` 用 `if let Ok(...)`，`check_portal_full` 的 `Err`（HTTP 客户端创建失败等）不会记录任何日志。
13. **"已经在线"降级依赖 `message` 文本匹配**：`session.rs:151` 的 `m.contains("已经在线")` 与 `protocol.rs:200`、`protocol.rs:217` 的中文关键词耦合；文案变化即失效。同一逻辑还依赖 `parse_error` + `retryable` 组合（`session.rs:119-120`）来区分 HTML 分支，注释已说明这是为消除跨文件文案耦合。
14. **`get_adapters_force` 失败静默回退旧快照**：`service.rs:48-51`，回退后可能仍使用 DHCP 续租前的空 IP 快照，异常链路上无日志。
15. **`AUTH_FAILURE_CODES` 与协议 code 字符串跨文件耦合**：`failure_tracker.rs:9` 的 `["ac_auth_failed", "1", "4"]` 必须与 `protocol.rs:205`、`protocol.rs:212`、`protocol.rs:225` 产出的 code 一致；`"1"` 同时覆盖"非法/失败/错误/拒绝"四类语义，无法区分。反证在 `session.rs:144-173`：它把"已在线但网络不通"也降级为 `code="1"`，从而进入认证失败计数路径（注释明说这是为了触发 MAC 重置自愈）。
16. **MAC 重置逻辑在两处重复实现**：`failure_tracker.rs:73-94`（认证失败路径）与 `failure_tracker.rs:154-178`（双适配器路径）与 `failure_tracker.rs:246-280`（Portal 请求失败路径）是三段近乎相同的 `dhcp_release_renew_single` + 日志代码，阈值常量却分属 `MAX_FAILURES`（`failure_tracker.rs:6`）与 `PORTAL_REQUEST_FAILURE_THRESHOLD`（`failure_tracker.rs:194`）。
17. **认证失败与 Portal 请求失败共用同一计数器**：`handle_portal_request_failure`（`failure_tracker.rs:230-239`）与 `handle_single_adapter_failure`（`failure_tracker.rs:137-146`）都读写 `a1_auth_failure_count` / `a2_auth_failure_count`。注释（`failure_tracker.rs:196-203`）承认二者"触发条件不同但共用计数访问器"，因此两类失败混合累加会提前凑满阈值 5，触发非预期的 MAC 重置。
18. **`execute_dual` 不校验调用线程**：`dual_adapter_executor.rs:55` 直接用 `block_on_sync` 驱动 `spawn_blocking`；`infra/async_util.rs` 顶部注释指出该函数在 async worker 线程上会 panic（`Handle::block_on` 嵌套限制）。当前唯一调用方 `service.rs:90`、`service.rs:198` 是同步函数，但函数签名未做防护或断言。
19. **`execute_dual` 内适配器 2 的结果串行等待**：`dual_adapter_executor.rs:79-85` 先 `await` 适配器 2 的 join，再 `await` 适配器 1（`dual_adapter_executor.rs:85`）。两条任务本身并发，但若适配器 2 长时间不返回，适配器 1 已完成的结果也不会提前返回。
20. **`DualAdapterResult::success()` 是"或"语义**：`dual_adapter_executor.rs:14-15`，一台成功即整体 `success=true`，失败端的信息只体现在合并消息与 `failure_tracker` 的分别计数里。
21. **`post_login_handler` 的后台检查依赖未取消 token 的兜底**：`service.rs:239-241`，`task_manager.cancel_token("background_check")` 取不到时新建 token，即可能并发跑起第二个后台检查任务。
22. **集成测试无断言、依赖真实校园网**：`tests/repro_logout_panic.rs:43-44` 只 `println!` 结果（"复现测试完成（未 panic 则说明该结构本身安全）"），没有任何 `assert`；测试要到真实 `10.2.106.187` 发 GET（`:27`、`:32`），在非校园网环境会因 `check_portal_full` 返回 `Failed` 而照样"通过"，无法起到回归锁定作用。
