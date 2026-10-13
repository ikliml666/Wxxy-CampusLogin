---
title: 注销请求携带硬编码占位凭据
type: learning
source_files:
  - tauri-app/src-tauri/src/auth/protocol.rs
tags: [教训, 协议, 注销, 脆弱性]
---

## Overview

本文档记录 Dr.COM/ePortal 认证协议中**注销（Logout）链路不校验用户身份凭据**的设计现状。

在 `tauri-app/src-tauri/src/auth/protocol.rs` 中，注销请求使用硬编码的占位账号 (`drcom`) 和占位密码 (`123`) 代替真实用户凭据。这是由 Portal 服务端协议逻辑决定的：注销操作主要依赖 `wlan_user_ip` 在服务端在线表中定位会话，而非验证用户身份。

这一设计导致注销链路存在“外部未校验”的脆弱性假设——如果未来 Portal 侧收紧对 `user_account` / `user_password` 字段的校验（如比对登录时的真实凭据），当前实现将静默退化（失败）。

## Key Components

### 常量定义

- **`LOGOUT_PLACEHOLDER_ACCOUNT`**: `const LOGOUT_PLACEHOLDER_ACCOUNT: &str = "drcom";` (第 3 行)
    - 注销请求中使用的硬编码占位账号。
- **`LOGOUT_PLACEHOLDER_PASSWORD`**: `const LOGOUT_PLACEHOLDER_PASSWORD: &str = "123";` (第 4 行)
    - 注销请求中使用的硬编码占位密码。
- **`MAX_HTTP_BODY`**: `const MAX_HTTP_BODY: u64 = 1024 * 1024;` (第 17 行)
    - HTTP 响应体读取上限（1MB），用于防止恶意/异常 Portal 耗尽内存。

### 核心函数

- **`do_logout_request`**: `fn do_logout_request(user: &str, adapter_ip: Option<&str>, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String>` (第 219–308 行)
    - 执行注销主流程。
    - **关键行为**：构建 Radius 注销 URL 时，使用 `LOGOUT_PLACEHOLDER_ACCOUNT` 和 `LOGOUT_PLACEHOLDER_PASSWORD` 填充 `user_account` 和 `user_password` 参数（第 245–252 行）。
    - **NAT 处理**：检测 NAT 内网 IP 时，不发送 `wlan_user_ip`（第 227–232 行）。
    - **Android 旁路**：在 Android 平台优先尝试旁路代理客户端（第 235–240 行）。
    - **策略变更**：改为“Radius 注销先行（最多 2 轮），成功后至多补一次 MAC 解绑；全败时 unbind 作兜底”（第 238–308 行注释及逻辑）。旧实现是每轮先 unbind 再 logout，易诱发僵尸状态。

- **`radius_logout_round`**: `fn radius_logout_round(client: &reqwest::Client, logout_url: &str, round: u32) -> Result<bool, String>` (第 311–326 行)
    - 单轮 Radius 注销请求。
    - 调用 `read_bounded_body` 读取响应并限制 1MB。
    - 解析结果后返回 `success` 布尔值。请求失败降级为 `false` 继续下一轮。

- **`mac_unbind_request`**: `fn mac_unbind_request(client: &reqwest::Client, unbind_url: &str) -> Result<bool, String>` (第 329–344 行)
    - MAC 解绑请求（best-effort）。
    - 清理 MAC 免认证绑定；Radius 注销全败时作兜底踢下线。
    - 使用真实用户账号 `validated_user` 和转换后的整数 IP 参数构建 URL（第 283–290 行）。

- **`merge_logout_results`**: `pub fn merge_logout_results(any_radius_ok: bool, any_unbind_ok: bool) -> &'static str` (第 347–355 行)
    - 合并 Radius 注销与 MAC 解绑的结果消息。
    - 返回值包括："注销成功"、"Radius注销成功，MAC解绑失败"、"Radius注销失败，MAC解绑成功"、"注销失败"。

- **`do_logout_with_retry`**: `pub fn do_logout_with_retry(user: &str, adapter_ip: Option<&str>, max_retries: u32, is_quitting: &std::sync::atomic::AtomicBool) -> Result<serde_json::Value, String>` (第 358–385 行)
    - 带重试机制的注销入口。
    - 每次尝试调用 `do_logout_request`。
    - 支持可中断等待（`wait_cancellable`），检查 `is_quitting` 标志。

- **`parse_logout_result`**: `fn parse_logout_result(response: &str) -> Result<serde_json::Value, String>` (第 388–433 行)
    - 解析注销响应 JSON。
    - 使用 `jsonp_json_slice` 提取 JSON 对象（支持字符串/转义感知的花括号平衡）。
    - 根据 `result` 码和 `msg` 关键词判断成功/失败。
    - 包含 HTML 响应回退逻辑：若解析失败且响应含 HTML，检查是否包含“注销成功”等关键词。

- **`ip_to_eportal_int`**: `fn ip_to_eportal_int(ip: &str) -> Option<u32>` (第 358–361 行)
    - 点分 IPv4 → ePortal 整数形式（大端序数值），用于 MAC 解绑 URL。

- **`random_v`**: `pub fn random_v() -> String` (第 6–11 行)
    - 生成注销请求中的随机 `v` 参数（基于纳秒时间戳）。

- **`read_bounded_body` / `read_bounded_body_async`**: (第 20–76 行)
    - 读取 HTTP 响应体并限制 1MB 上限，按 Content-Type charset 解码。

- **`jsonp_json_slice`**: `fn jsonp_json_slice(response: &str) -> &str` (第 79–106 行)
    - 从 JSONP 响应中截取第一个完整 JSON 对象，解决 msg 含半角 `)` 或 `{}` 导致的截断问题。

- **`wait_cancellable`**: `fn wait_cancellable(duration_ms: u64, is_quitting: &std::sync::atomic::AtomicBool) -> bool` (第 109–116 行)
    - 可中断等待，每 100ms 检查退出标志。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     do_logout_with_retry                    │
│                   (protocol.rs:358)                         │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    do_logout_request                  │  │
│  │                   (protocol.rs:219)                   │  │
│  │                                                       │  │
│  │  ┌─────────────────┐   ┌──────────────────────────┐  │  │
│  │  │  Radius 注销循环 │   │      MAC 解绑收尾        │  │  │
│  │  │  (最多2轮)       │──►│  (best-effort, 单次)     │  │  │
│  │  │                 │   │                          │  │  │
│  │  │  • radius_      │   │  • mac_unbind_request    │  │  │
│  │  │    logout_round │   │                          │  │  │
│  │  │                 │   │  使用真实 user_account     │  │  │
│  │  │  使用占位凭据:  │   │  和整数 IP 参数            │  │  │
│  │  │  drcom / 123    │   │                          │  │  │
│  │  └─────────────────┘   └──────────────────────────┘  │  │
│  │                                                       │  │
│  │  • NAT 检测: 内网 IP 不发送 wlan_user_ip              │  │
│  │  • Android 旁路优先: bypass_proxy_addr                │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │               merge_logout_results                    │  │
│  │               (protocol.rs:347)                       │  │
│  │                                                       │  │
│  │  合并 Radius 和 MAC 解绑结果，返回静态消息串           │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **入口**: `do_logout_with_retry` 被调用，传入用户账号、适配器 IP 和退出标志。
2. **重试循环**: 最多重试 `max_retries` 次，每次间隔 2 秒（可中断）。
3. **单次注销请求 (`do_logout_request`)**:
    a. **参数准备**:
       - 验证用户名和操作符。
       - 检测 NAT 内网 IP：若是，则 `wlan_user_ip` 设为空串。
       - 创建 HTTP 客户端（Android 平台优先旁路）。
    b. **Radius 注销循环 (最多 2 轮)**:
       - 构建注销 URL：`{portal_base}/eportal/portal/logout?callback=...&user_account={LOGOUT_PLACEHOLDER_ACCOUNT}&user_password={LOGOUT_PLACEHOLDER_PASSWORD}&wlan_user_ip={...}&v={random_v()}...`
       - **关键**: `user_account` 和 `user_password` 始终为 `"drcom"` 和 `"123"`。
       - 发送 GET 请求，读取响应（限 1MB），解析 JSON。
       - 若成功，跳出循环；否则等待 1.5 秒后进入下一轮。
    c. **MAC 解绑收尾 (单次)**:
       - 仅当未退出时执行。
       - 构建解绑 URL：使用**真实** `validated_user` 和整数形式的 `wlan_user_ip`。
       - 发送 GET 请求，解析响应。
       - 无论成功与否，不中断流程。
4. **结果合并**: `merge_logout_results` 根据 `any_radius_ok` 和 `any_unbind_ok` 生成消息串。
5. **返回**: 返回包含 `code`, `message`, `success`, `retryable`, `radiusOk`, `unbindOk` 的 JSON 对象。

## Connections

[[logout-radius-first]]