---
title: 日志脱敏与 Portal 状态探测模块
type: module
source_files:
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/auth/portal.rs
tags: [日志, 脱敏, Portal, 安全, 架构]
---

## Overview

本文档记录 `infra/logger`（日志基础设施）与 `auth/portal`（Portal 状态探测与安全脱敏）两个核心模块。

**logger.rs**：负责应用日志的异步写入、轮转、清理与读取。核心设计包括有界通道防背压、非阻塞丢弃策略、批量落盘优化及文件尾部高效读取。
**portal.rs**：负责检测校园网 Portal 登录状态及安全处理凭据。核心设计包括基于 HTML 特征的只读状态判定（绝不携带密码）、错误信息统一脱敏（`redact_credentials`）及 NAT IP 识别。

## Key Components

### infra/logger.rs (`tauri-app/src-tauri/src/infra/logger.rs`)

**常量与配置**
- `MAX_LOG_SIZE`: `5 * 1024 * 1024` — 单日志文件最大大小，触发轮转。
- `MAX_LOG_FILES`: `5` — 保留的最大轮转日志文件数。
- `FLUSH_INTERVAL_MS`: `2000` — 默认批量落盘间隔（毫秒）。
- `LOG_CHANNEL_CAPACITY`: `8192` — 审计 A17：日志消息通道容量，满载时丢弃。
- `DROP_REPORT_INTERVAL`: `30s` — 丢弃报告最小间隔。
- `DROP_REPORT_THRESHOLD`: `1000` — 触发丢弃报告的累计阈值。

**枚举与结构体**
- `LogLevel` (行 35): `Debug`, `Info`, `Warn`, `Error`。包含 `as_str()` 方法 (行 40)。
- `LogMessage` (行 54): 枚举类型，变体包括 `Entry { line }`, `Flush { ack }`, `Shutdown`。
- `LoggerState` (行 59): 内部状态，包含 `log_dir`, `current_writer`, `current_date`, `last_flush`。

**全局静态变量**
- `LOGGER_SENDER`: `ArcSwap<Option<SyncSender<LogMessage>>>` — 当前发送端。
- `CLEAR_LOGS_MUTEX`: `Mutex<()>` — 用于 `clear_logs` 的互斥锁。
- `LOGGER_THREAD`: `Mutex<Option<JoinHandle>>` — 当前工作线程句柄。
- `MIN_LOG_LEVEL`: `ArcSwap<LogLevel>` — 当前最低日志级别。
- `LOG_DROPPED`: `AtomicU64` — 丢弃计数。
- `LOG_RETENTION_DAYS`: `AtomicU32` — 保留天数，默认 7。
- `LOG_DIR`: `OnceLock<PathBuf>` — 实际日志目录。

**公开函数**
- `set_log_retention_days(days: u32)` — 设置保留天数并立即执行一次清理。
- `get_log_retention_days()` — 获取保留天数。
- `init_logger(log_dir: PathBuf)` — 初始化日志系统，创建 worker 线程和有界通道。
- `set_log_level(level: LogLevel)` — 设置最低日志级别。
- `get_log_level()` — 获取当前日志级别。
- `set_debug_mode(enabled: bool)` — Tauri 命令，切换 Debug/Info 模式。
- `get_debug_mode()` — Tauri 命令，获取 Debug 状态。
- `log(level, module, message)` — 核心日志函数。检查级别后通过 `try_send` 发送，Warn 及以上若丢弃则兜底打印到 stderr。
- `read_recent_logs(app_handle, lines)` — 从当天日志文件尾部倒读指定行数（使用倍增窗口法，避免全量加载）。
- `flush()` — 阻塞等待 worker 完成当前缓冲区的写入。
- `flush_quick()` — 快速 flush，超时 500ms，用于 panic hook。
- `shutdown()` — 关闭 logger，发送 Shutdown 消息并 join 线程。
- `is_app_log_file(name)` — 判断文件名是否为 `app-*.log`。
- `clear_logs(app_handle)` — 删除所有应用日志文件并重建 logger worker。
- `get_log_dir(app_handle)` — 根据平台获取日志目录（Android 用 `app_data_dir`，桌面用 `current_exe` 父目录）。

**宏定义**
- `log_debug!`, `log_info!`, `log_warn!`, `log_error!` — 简化日志调用。

**内部函数**
- `logger_worker(state, receiver)` — 核心循环。处理接收、排水、丢弃报告、定期清理（1小时）和批量落盘。
- `drain_channel(receiver, buffer)` — 非阻塞地清空通道到缓冲区。
- `flush_messages(state, buffer)` — 将缓冲区消息写入文件，处理 Rotate。
- `rotate_if_needed(state)` — 按日期或大小轮转日志文件。
- `cleanup_old_logs(log_dir)` — 按修改时间排序，删除超出 `MAX_LOG_FILES` 的文件。
- `cleanup_old_logs_by_time(log_dir, retention_days)` — 按保留天数清理日志。

**测试**
- `is_app_log_file_positive` / `is_app_log_file_negative` — 验证文件名匹配逻辑。

### auth/portal.rs (`tauri-app/src-tauri/src/auth/portal.rs`)

**常量与配置**
- `portal_config::CLIENT_TIMEOUT`: `8s` — HTTP 客户端连接超时。
- `portal_config::REQUEST_TIMEOUT`: `3s` — 单次请求超时。
- `PAGE_INDICATOR_LOGOUT`: `"Dr.COMWebLoginID_1"` — 在线页特征。
- `PAGE_INDICATOR_LOGIN_0`: `"Dr.COMWebLoginID_0"` — 登录页特征。
- `PAGE_INDICATOR_LOGIN_2`: `"Dr.COMWebLoginID_2"` — 登录页特征。
- `PAGE_TITLE_LOGOUT`: `"<title>注销页</title>"` — 在线页标题。
- `PAGE_TITLE_LOGIN`: `"<title>登录页</title>"` — 登录页标题。

**公开函数**
- `ensure_portal_port(base: &str)` — 确保 Portal URL 包含 `:801` 端口，正确处理路径。
- `safe_truncate(s, max_len)` — 安全的字符串截断，不破坏 UTF-8 字符。
- `redact_credentials(msg, url, base_url, password)` — **关键安全函数**。替换完整 URL、URL 编码密码和明文密码为占位符，防止凭据泄露。
- `check_portal_full(adapter_ip, adapter_name)` — 完整的 Portal 状态检测入口。返回 `PortalStatus`。
- `operator_suffix_from_uid(uid)` — 从 UID 推导运营商后缀（如 `@telecom`）。
- `is_nat_private_ip(ip)` — 判断 IP 是否为内网/NAT IP。

**内部函数**
- `block_on_http(future)` — 在同步上下文中运行异步 HTTP 请求，处理 Tokio runtime 上下文。
- `handle_unknown_page_status(...)` — 页面无法识别时的处理。**只读操作**，不携带密码，返回需人工确认状态。
- `parse_adapter_ip(...)` — 解析适配器 IP。
- `build_determined_status(...)` — 构建已确定的状态。
- `build_request_failed_status()` — 构建请求失败状态。
- `check_portal_page(client, portal_base)` — 发起 HTTP 请求并分析 HTML 内容。
- `analyze_portal_page_content(html)` — 分析 HTML 特征，判断在线/离线/未知。
- `has_user_session_indicators(html)` — 检查会话特征（uid, v4ip, oltime）。
- `extract_uid_from_html(html)` — 从 HTML 提取 uid 值。
- 日志辅助函数：`log_portal_query_start`, `log_portal_page_result`, `log_portal_page_failed`, `log_portal_no_credentials`, `log_page_indicator_found`。

**测试**
- `ensure_portal_port_*` — 端口处理测试。
- `is_nat_private_ip_*` — NAT IP 识别测试。
- `parse_adapter_ip_*` — IP 解析测试。
- `build_determined_status_*` — 状态构建测试。
- `handle_unknown_page_status_*` — 未知状态测试。
- `analyze_page_*` — 页面内容分析测试。
- `has_user_session_indicators_*` — 会话特征测试。
- `extract_uid_*` — UID 提取测试。
- `operator_suffix_*` — 运营商后缀测试。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                       Application Layer                            │
│  (Tauri Commands, Network Watcher, Self-Service)                   │
└──────────┬──────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     infra/logger.rs                                  │
│  ┌──────────────┐    ┌──────────────────┐    ┌──────────────────┐   │
│  │  log!* Macros│───►│  log()           │    │  read_recent_logs│   │
│  └──────────────┘    │  (Level Check)   │    └──────────────────┘   │
│                     └──────┬───────────┘                          │
│                            │ try_send (Non-blocking)               │
│                            ▼                                      │
│                     ┌──────────────────┐                         │
│                     │ SyncChannel      │ ◄── LOG_CHANNEL_CAPACITY│
│                     │ (Capacity 8192)  │                         │
│                     └──────┬───────────┘                         │
│                            │                                     │
│                            ▼                                      │
│                     ┌──────────────────┐                         │
│                     │ logger_worker    │                         │
│                     │ (Thread)         │                         │
│                     │ - drain_channel  │                         │
│                     │ - batch_flush    │                         │
│                     │ - rotate/cleanup │                         │
│                     └──────────────────┘                         │
└─────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     auth/portal.rs                                   │
│  ┌──────────────────┐    ┌──────────────────┐                      │
│  │ check_portal_full│───►│ block_on_http    │                      │
│  └──────────────────┘    │ (Async->Sync)    │                      │
│                          └──────┬───────────┘                      │
│                                 │ GET Portal Page                  │
│                                 ▼                                  │
│                          ┌──────────────────┐                      │
│                          │ analyze_portal   │                      │
│                          │ _page_content    │                      │
│                          │ (HTML Features)  │                      │
│                          └──────────────────┘                      │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐     │
│  │ redact_credentials(msg, url, base_url, password)         │     │
│  │ - Replaces full URL with base_url?***                    │     │
│  │ - Replaces encoded password with ***                     │     │
│  │ - Replaces plaintext password with ***                   │     │
│  └──────────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 日志写入流程
1. 调用方通过 `log_info!` 等宏调用 `log(level, module, message)`。
2. `log` 检查 `MIN_LOG_LEVEL`，若低于阈值则直接返回。
3. 格式化日志行 `[timestamp] [level] [module] message`。
4. 尝试 `sender.try_send(LogMessage::Entry { line })`。
   - **成功**：消息进入有界通道。
   - **失败（通道满）**：`LOG_DROPPED` 原子计数加 1。若级别 >= Warn，则 `eprint!` 兜底输出到 stderr。
5. `logger_worker` 线程通过 `recv_timeout` 或 `try_recv` 获取消息，累积到缓冲区。
6. 当满足批量条件（数量 ≥ 32，或时间 ≥ 2s，或有 Flush 请求）时，调用 `flush_messages`。
7. `flush_messages` 检查文件大小和日期，必要时调用 `rotate_if_needed` 进行轮转。
8. 将缓冲区内容写入当前日志文件。

### Portal 状态检测流程
1. `check_portal_full` 被调用（通常由网络 watcher 周期性触发）。
2. 构建 HTTP 客户端，请求 Portal 基础页面（通常是 80 端口，而非 801）。
3. `check_portal_page` 获取 HTML 响应体（限制 1MB）。
4. `analyze_portal_page_content` 扫描 HTML 中的已知特征字符串（如 `Dr.COMWebLoginID_1`）。
   - **找到特征**：返回 `Determined(online, uid)`。
   - **未发现特征但发现会话信息**（uid/v4ip/oltime）：返回 `Determined(true, uid)`。
   - **完全未知**：返回 `Unknown`，触发 `handle_unknown_page_status`，返回需人工确认的状态（**不携带密码**）。
   - **请求失败**：返回 `Failed`。
5. 若涉及错误信息展示，必须经过 `redact_credentials` 处理，确保 URL 和密码不被泄露。

## Connections

- [[config-mask-single-exit]]：旧版配置掩码机制，已被更完善的脱敏策略替代。
- [[mask-placeholder-persisted-as-plaintext]]：历史教训，强调持久化存储中的敏感数据风险。
- [[infra-async-util]]：`portal.rs` 中 `block_on_http` 依赖的异步工具模块。