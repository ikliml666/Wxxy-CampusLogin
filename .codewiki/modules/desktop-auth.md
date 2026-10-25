---
title: 桌面端认证（登录/注销/Portal 探测/失败计数）
type: module
source_files:
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/auth/session.rs
  - tauri-app/src-tauri/src/auth/failure_tracker.rs
tags: [认证, Portal, Dr.COM, 双适配器, 失败计数, 跨平台, 桌面端]
---

## Overview

本模块实现校园网 Dr.COM / ePortal 认证协议核心：构造并发送登录（`/eportal/portal/login`）、Radius 注销（`/eportal/portal/logout`）、MAC 解绑（`/eportal/portal/mac/unbind`）HTTP 请求，解析 JSONP 响应为统一 `{code, message, success, retryable, radiusOk, unbindOk}` 结果。模块声明见 `auth/mod.rs`：`failure_tracker`、`portal`、`protocol` 无条件编译（跨平台）；`dual_adapter_executor`、`session`、`service` 仅 `#[cfg(desktop)]` 编译（`mod.rs:1-6`），依赖桌面适配器发现与 Tauri 命令面。

本次更新基于提供的源码：`protocol.rs` 负责请求与响应解析（含同步/异步限长读体、JSONP 切片、登录/注销重试）、`session.rs`（桌面）负责单适配器动作包装与日志/历史、附加“已经在线”假成功复核、`failure_tracker.rs` 负责认证失败计数与 Portal 请求失败触发 MAC 重置。`portal.rs`、`service.rs`、`dual_adapter_executor.rs` 源码未内联，其声明见 `mod.rs` 但不展开。

## Key Components

### 模块声明（auth/mod.rs）

| 位置 | 可见性 | 项 | 说明 |
| --- | --- | --- | --- |
| `mod.rs:1` | pub | `mod failure_tracker` | 认证失败计数与 Portal 请求失败处理，跨平台 |
| `mod.rs:2` | pub | `mod portal` | Portal 探测（源码未内联） |
| `mod.rs:3` | pub | `mod protocol` | 登录/注销协议请求与响应解析，跨平台 |
| `mod.rs:6-7` | pub `#[cfg(desktop)]` | `mod dual_adapter_executor` | 双适配器并行执行器 |
| `mod.rs:8-9` | pub `#[cfg(desktop)]` | `mod session` | 单适配器动作 + 日志/历史包装 |
| `mod.rs:10-11` | pub `#[cfg(desktop)]` | `mod service` | 登录/注销顶层编排 |

### auth/protocol.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `protocol.rs:3-4` | 私有 const | `LOGOUT_PLACEHOLDER_ACCOUNT = "drcom"`, `LOGOUT_PLACEHOLDER_PASSWORD = "123"` | 注销请求固定占位凭据 |
| `protocol.rs:6-11` | pub fn | `random_v() -> String` | 生成 1000~9999 的 `v` 参数（拦缓存），种子取 `SystemTime` 纳秒 |
| `protocol.rs:13-14` | 私有 const | `MAX_HTTP_BODY: u64 = 1024*1024` | 响应体读取 1MB 上限 |
| `protocol.rs:21-41` | pub(crate) fn | `read_bounded_body(resp, label) -> String` | 同步读响应体限 1MB，按 charset 解码；失败/超限返回空串+`log_warn!`（tag 硬编码 `logout`） |
| `protocol.rs:46-63` | pub(crate) async fn | `read_bounded_body_async(resp, label) -> Result<String, String>` | 异步版限长读体；失败/超限返回 `Err` 而非空串 |
| `protocol.rs:66-71` | 私有 fn | `content_type_charset(resp) -> Option<String>` | 提取 Content-Type 的 charset |
| `protocol.rs:76-110` | 私有 fn | `jsonp_json_slice(response: &str) -> &str` | 花括号平衡（字符串/转义感知）截取首个完整 JSON 对象；非 JSONP 回退原文 |
| `protocol.rs:113-122` | 私有 fn | `wait_cancellable(duration_ms, is_quitting) -> bool` | 每 100ms 检查退出标志的可中断等待；`duration_ms/100` 为 0 时直接返回 true |
| `protocol.rs:124-193` | 私有 fn | `do_login_request(user, password, operator, adapter_ip) -> Result<Value, String>` | 单次登录：校验→拼 URL→GET→限长读体→解析；密码明文入参（安全审计 A18） |
| `protocol.rs:195-228` | pub fn | `do_login_with_retry(user, password, operator, adapter_ip, max_retries, is_quitting) -> Result<Value, String>` | 登录重试循环；成功或 `retryable=false` 返回；间隔 2000ms 可中断 |
| `protocol.rs:230-279` | 私有 fn | `parse_login_result(response) -> Result<Value, String>` | 按 `result` 0/1/2/3/4 与 `msg` 关键词映射为 `{code, message, success, retryable}` |
| `protocol.rs:281-385` | 私有 fn | `do_logout_request(user, adapter_ip, is_quitting) -> Result<Value, String>` | 单次注销：Radius 注销最多 2 轮→收尾 MAC 解绑；结果含 `radiusOk`/`unbindOk` |
| `protocol.rs:387-406` | 私有 fn | `radius_logout_round(client, logout_url, round) -> Result<bool, String>` | 单轮 Radius 注销，返回是否成功 |
| `protocol.rs:408-427` | 私有 fn | `mac_unbind_request(client, unbind_url) -> Result<bool, String>` | 单轮 MAC 解绑，best-effort |
| `protocol.rs:429-440` | pub fn | `merge_logout_results(any_radius_ok, any_unbind_ok) -> &'static str` | 合并 Radius 与 MAC 解绑文案（4 组合） |
| `protocol.rs:442-447` | 私有 fn | `ip_to_eportal_int(ip: &str) -> Option<u32>` | 点分 IPv4 → ePortal 大端整数；空/非法 None |
| `protocol.rs:449-481` | pub fn | `do_logout_with_retry(user, adapter_ip, max_retries, is_quitting) -> Result<Value, String>` | 注销重试循环，语义同登录侧 |
| `protocol.rs:483-530` | 私有 fn | `parse_logout_result(response) -> Result<Value, String>` | 解析注销响应；JSON 失败回退 HTML/文本关键词 |

协议结果 JSON 约定（由 `parse_login_result`/`parse_logout_result`/`do_logout_request` 产出）：

- `code`: `String` —— `"0"` 成功、`"1"` 非法/失败、`"2"` 已在线、`"3"` 流量超限、`"4"` 账号禁用、`"ac_auth_failed"`、`"parse_error"`、`"unknown_failure"`、`"max_retries"`、`"error"`
- `message`: `String`
- `success`: `bool`
- `retryable`: `bool` —— 缺失时调用方按 `true`（如 `protocol.rs:208-209` 的 `unwrap_or(true)`）
- `radiusOk`: `bool` —— 仅注销结果（`protocol.rs:382`），Radius 注销子步骤原始结果
- `unbindOk`: `bool` —— 仅注销结果（`protocol.rs:383`），MAC 解绑子步骤原始结果；判定离线生效取 `radiusOk || unbindOk`

### auth/session.rs（`#[cfg(desktop)]` 由 mod.rs 包裹）

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `session.rs:11-75` | pub fn | `adapter_action_with_log<F>(adapter, config, app_handle, action_name, log_tag, action_type, do_action) -> Option<CommandResult>` | 单适配器动作通用包装：发日志→执行→前端事件+写历史；`adapter.ip` 空返回 None |
| `session.rs:77-176` | pub fn | `login_adapter_with_log(adapter, config, app_handle, is_quitting) -> Option<CommandResult>` | 单适配器登录：Portal 预检直通→`do_login_with_retry(...,3,...)`→`parse_error` 复核→“已经在线”假成功复核（不通则降级 `code="1"`） |

### auth/failure_tracker.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `failure_tracker.rs:5-6` | 私有 const | `MAX_FAILURES: u32 = 5` | 连续认证失败阈值（仅计数与日志，不触发重置） |
| `failure_tracker.rs:8-9` | 私有 const | `AUTH_FAILURE_CODES: &[&str] = &["ac_auth_failed", "1", "4"]` | 认证失败 code 白名单 |
| `failure_tracker.rs:11-16` | pub enum | `AdapterFailureCounter { A1, A2 }` (`Clone, Copy`) | 双适配器计数器标识 |
| `failure_tracker.rs:20-27` | pub fn | `is_auth_failure(result: &CommandResult) -> bool` | 从 `data.code` 判定是否认证失败 |
| `failure_tracker.rs:27-33` | pub(crate) fn | `get_adapter_failure_count(state, counter) -> u32` | 读 A1/A2 计数快照 |
| `failure_tracker.rs:35-42` | pub(crate) fn | `set_adapter_failure_count(state, counter, value)` | 写 A1/A2 计数 |
| `failure_tracker.rs:44-81` | pub fn | `update_auth_failure_count(state, cmd_result, adapter_name)` | 单适配器计数：成功清零；认证失败原子累加（`update_with_result`）；达 5 仅日志不再触发 MAC 重置 |
| `failure_tracker.rs:84-94` | pub fn | `update_dual_adapter_auth_failure(state, r1, r2, a1_name, a2_name)` | 双适配器分别计数入口 |
| `failure_tracker.rs:95-142` | 私有 fn | `handle_single_adapter_failure(state, result, adapter_name, counter)` | 单适配器计数实现（成功清零/失败累加/达阈值仅日志） |
| `failure_tracker.rs:145-155` | pub fn | `reset_all(state)` | 注销成功清零 `portal_failure_count`、`a1/a2_auth_failure_count`、`prep_login_failures` |
| `failure_tracker.rs:156-158` | 私有 const | `PORTAL_REQUEST_FAILURE_THRESHOLD: u32 = 3` | Portal HTTP 连续不可达阈值（2026-10-25 由 5 改 3） |
| `failure_tracker.rs:170-263` | pub fn | `handle_portal_request_failure(state, app_handle, adapter_ref, adapter_ip, campus_gw, counter, adapter_label)` | Portal 请求失败处理：网关不可达跳过清零；可达累加，达 3 触发该适配器 MAC 重置（`dhcp_release_renew_single`）或非校园网子网提醒 |

`CommandResult` 外部类型（`infra/state`）字段：`success: bool`, `message: Option<String>`, `data: Option<serde_json::Value>`。

## Architecture

```text
┌─────────────────────────┐
│ auth/mod.rs (mod.rs:1-11)│
└───────────┬─────────────┘
            │ declares
   ┌────────┼───────────────┐
   │        │               │
┌──▼──────┐ ┌─▼──────────┐ ┌▼──────────────────┐
│protocol.rs│ │session.rs  │ │failure_tracker.rs │
│(cross-    │ │(desktop cfg)│ │(cross-platform)   │
│ platform) │ │            │ │                   │
└──┬──────┘ └─┬──────────┘ └┬──────────────────┘
   │          │             │
   │ calls    │ calls       │ called by upper
   │          ▼             │ layer & monitor
   │   login_adapter_with_log (session.rs:77)
   │          │
   │          ▼
   └─► do_login_with_retry (protocol.rs:195)
   └─► do_logout_with_retry (protocol.rs:449)
            │
            ▼
   parse_login_result / parse_logout_result
            │
            ▼
   update_auth_failure_count (failure_tracker.rs:44)
   handle_portal_request_failure (failure_tracker.rs:170)
```

## Data Flow

### 登录链路（单适配器，基于 session.rs + protocol.rs）

```
session::login_adapter_with_log (session.rs:77)
  ├─ check_portal_full 预检 (session.rs:87) → online 直通成功并记历史
  ├─ do_login_with_retry(user, password, operator, Some(adapter.ip), 3, is_quitting) (session.rs:111)
  │    └─ do_login_request (protocol.rs:124) → 校验 → GET /eportal/portal/login → read_bounded_body → parse_login_result
  ├─ parse_error 且 retryable==true → 复核 check_portal_full (session.rs:119-135)
  ├─ success 且 message 含"已经在线" → 复核不通则降级 code="1" (session.rs:144-173)
  └─ adapter_action_with_log 包装事件与历史 (session.rs:11)
→ failure_tracker::update_auth_failure_count / update_dual_adapter_auth_failure (failure_tracker.rs:44,83)
```

### 注销链路

```
上层注销调用 protocol::do_logout_with_retry (protocol.rs:449)
  └─ do_logout_request (protocol.rs:281)
       ├─ is_nat_private_ip 判断 (protocol.rs:288)
       ├─ 轮 1..2: radius_logout_round (protocol.rs:390) → 成功 break；轮1后等待 1.5s 手写循环 (protocol.rs:335-340)
       ├─ mac_unbind_request (protocol.rs:411) 收尾
       └─ merge_logout_results → JSON 含 radiusOk/unbindOk (protocol.rs:370-384)
→ 上层注销成功调 failure_tracker::reset_all (failure_tracker.rs:144)
```

## Connections

- [[desktop-network-core]] — `network::client::{PORTAL_URL, create_safe_http_client}`（`protocol.rs:1,129,150`）、`network::check_gateway_reachable_from`（`failure_tracker.rs:179`）、`network::dhcp_release_renew_single`（`failure_tracker.rs:230`）、`network::Adapter` 类型。
- [[desktop-config]] — `config::validate::{validate_username, validate_operator, validate_password}`（`protocol.rs:125-127`）、`config::persist::append_login_history`（`session.rs:41,51`）。
- [[desktop-infra]] — `infra::state::{AppState, CommandResult}`、`infra::events::EventBus`（`session.rs:27`）、`infra::async_util::block_on_sync`（`protocol.rs:27,166,181`）、`platform::console_output::decode_charset_bytes`（`protocol.rs:33,60`）、`infra::notification::emit_notification`（`failure_tracker.rs:242`）。
- 安卓端复用跨平台子集：`protocol.rs` 的 `do_login_with_retry`/`do_logout_with_retry` 与 `portal.rs` 的 `check_portal_full`（后者源码未内联）；`session.rs`/`service.rs`/`dual_adapter_executor.rs` 桌面 cfg 不编译，安卓端有自有监控循环。

## Known Issues

1. **注销硬编码占位凭据**：`protocol.rs:128-129` 定义 `LOGOUT_PLACEHOLDER_ACCOUNT="drcom"`、`LOGOUT_PLACEHOLDER_PASSWORD="123"`，`do_logout_request` 在 `protocol.rs:128-129` 填入 `user_account`/`user_password`；注销不校验身份，依赖 `wlan_user_ip`。Portal 侧收紧校验会静默退化。
2. **`read_bounded_body` 日志 tag 硬编码 `logout`**：`protocol.rs:23` 与 `protocol.rs:30` 使用 `crate::log_warn!("logout", ...)`。该函数被通用调用（如 `protocol.rs:396` Radius 注销、`protocol.rs:417` MAC 解绑，且 `portal.rs` 页面探测也会复用），导致非注销场景的超限/失败日志被错误归类 `logout` tag。
3. **`wait_cancellable` 短时长失效**：`protocol.rs:114` `let steps = duration_ms / 100;` 当 `duration_ms < 100` 得 0，直接返回 `true`（未取消）。当前调用传 2000/1500 未触发，但逻辑脆弱。
4. **注销轮次间隔手写循环且不复用**：`protocol.rs:335-340` 在 `if round == 1` 内手写 `for _ in 0..15 { sleep(100ms) }`，与 `wait_cancellable`（`protocol.rs:113-122`）功能重复却不调用；第 2 轮失败后无间隔直接进入 MAC 解绑。
5. **登录大响应体错误被无谓重试**：`protocol.rs:177-179` 返回 `Err("登录响应体过大")`，经 `do_login_with_retry` 的 `Err` 分支（`protocol.rs:214-219`）包装为 `retryable: true`，同一超大响应体被重复请求最多 `max_retries=3` 次。
6. **`result == 1` 且 `msg` 空被判登录成功**：`protocol.rs:254` 空 msg 落 `else` 返回 `"Portal协议认证成功"`；若 Portal 改版 `result=1` 不带 msg 可能误报成功。
7. **认证失败计数不再触发 MAC 重置但日志混淆**：`failure_tracker.rs:77-80` 连续 5 次认证失败仅 `log_warn` 并清零，不再重置 MAC（设计调整见 `failure_tracker.rs:73-76`）。但 `session.rs:144-173` 把“已经在线但网络不通”降级为 `code="1"` 仍进入此计数，仅产生警告日志，无自愈动作。
8. **认证失败与 Portal 请求失败共用同一计数器**：`handle_single_adapter_failure`（`failure_tracker.rs:119-128`）与 `handle_portal_request_failure`（`failure_tracker.rs:196-205`）均读写 `a1_auth_failure_count`/`a2_auth_failure_count`。后者阈值 `PORTAL_REQUEST_FAILURE_THRESHOLD=3`（`failure_tracker.rs:158`），前者 `MAX_FAILURES=5`（`failure_tracker.rs:6`），混合累加可能提前凑满 3 触发非预期 MAC 重置（网关可达时）。
9. **`merge_logout_results` 文案组合依赖调用方理解**：`protocol.rs:433-440` 返回静态文案，但 `do_logout_request` 结果 JSON（`protocol.rs:207-219`）中 `success` 仅取 `radiusOk`，调用方须用 `radiusOk || unbindOk` 判离线（注释强调）。若调用方忽略 `unbindOk` 会误判 `(false,true)` 为失败。
10. **预检错误被静默吞掉**：`session.rs:87` 与 `session.rs:122` 用 `if let Ok(...)`，`check_portal_full` 的 `Err` 不记录日志。