---
title: Desktop Auth
type: module
source_files:
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/auth/session.rs
  - tauri-app/src-tauri/src/auth/failure_tracker.rs
tags: [desktop, auth, tauri]
---

## Overview

本模块提供 Tauri 桌面端（`tauri-app`）的认证功能，源码路径位于 `tauri-app/src-tauri/src/auth/`。所有关联的 Rust 源文件均已成功读取，包含模块声明 (`mod.rs`)、登录/注销协议与响应解析 (`protocol.rs`)、适配器级登录动作封装 (`session.rs`)、认证失败计数与 MAC 重置触发 (`failure_tracker.rs`)。

## Key Components

基于源码提取的公开函数、结构体与常量（行号以 `行号| ` 前缀为准）：

- `tauri-app/src-tauri/src/auth/mod.rs`
  - 行 1-11：声明 `failure_tracker`、`portal`、`protocol` 子模块；桌面专属 `#[cfg(desktop)]` 下声明 `dual_adapter_executor`(行 7)、`session`(行 9)、`service`(行 11)。
- `tauri-app/src-tauri/src/auth/protocol.rs`
  - `random_v` (行 6)：生成随机 `v` 参数。
  - `read_bounded_body` (行 21) 与 `read_bounded_body_async` (行 46)：限制 1MB 的响应体读取。
  - `jsonp_json_slice` (行 76)：平衡花括号截取 JSONP 中完整 JSON 对象。
  - `do_login_request` (行 124) 与 `do_login_with_retry` (行 195)：登录请求与重试。
  - `parse_login_result` (行 230)：解析登录响应 JSON/JSONP。
  - `do_logout_request` (行 281)、`radius_logout_round` (行 390)、`mac_unbind_request` (行 411)、`merge_logout_results` (行 433)、`do_logout_with_retry` (行 449)、`parse_logout_result` (行 483)。
- `tauri-app/src-tauri/src/auth/session.rs`
  - `adapter_action_with_log` (行 11)：通用适配器动作执行与前端日志/历史记录。
  - `login_adapter_with_log` (行 77)：封装登录流程，含 Portal 预检直通、parse_error 复核、以及「已经在线」假成功复核（行 144-173）。
- `tauri-app/src-tauri/src/auth/failure_tracker.rs`
  - `AdapterFailureCounter` 枚举 (行 13)：`A1`/`A2` 双适配器计数标识。
  - `is_auth_failure` (行 19)：依据 code 判定是否为认证失败。
  - `update_auth_failure_count` (行 47) 与 `update_dual_adapter_auth_failure` (行 84)：失败计数（认证失败不再触发 MAC 重置，仅记录）。
  - `PORTAL_REQUEST_FAILURE_THRESHOLD` (行 158，值为 3)：门户请求失败连续阈值。
  - `handle_portal_request_failure` (行 170)：门户 HTTP 不可达时累加计数并触发 MAC 重置（唯一触发源）。
  - `reset_all` (行 145)：注销成功时重置所有失败计数。

## Architecture

模块结构：`mod.rs` 聚合子模块；`protocol.rs` 负责纯协议请求与解析；`session.rs` 在适配器维度编排登录/动作并写日志；`failure_tracker.rs` 维护认证/MAC 重置计数状态。各组件通过 `CommandResult` 与 `AppState` 网络状态快照交互。

## Data Flow

登录调用链：`session::login_adapter_with_log` (行 77) → `adapter_action_with_log` (行 11) → `protocol::do_login_with_retry` (行 195) → `do_login_request` (行 124) → `parse_login_result` (行 230)。结果回传后由 `failure_tracker::update_auth_failure_count` (行 47) 或 `update_dual_adapter_auth_failure` (行 84) 处理计数；门户不可达路径由 `handle_portal_request_failure` (行 170) 独立处理。

## Connections

- [[tauri-app]]：本模块作为 `tauri-app` 工程下的认证子系统，物理路径为 `tauri-app/src-tauri/src/auth/`。

## Known Issues

- 原文档误称全部 4 个源文件读取失败，实际均已读取并提取到函数签名与行号，本文已重写。
- 注意 `failure_tracker.rs` 中 `MAX_FAILURES` (行 6) 原为 5，但 MAC 重置触发已改为门户不可达阈值 `PORTAL_REQUEST_FAILURE_THRESHOLD = 3` (行 158)，认证失败仅计数不触发重置。
