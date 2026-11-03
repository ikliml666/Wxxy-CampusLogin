---
title: 桌面后端认证协议模块
type: module
source_files:
  - tauri-app/src-tauri/src/auth/failure_tracker.rs
  - tauri-app/src-tauri/src/auth/portal.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
tags: []
generated_by: fsoft-native
fsoft_page: desktop_backend_auth_protocol.md
---
# 桌面后端认证协议模块

## 简介

`desktop_backend_auth_protocol` 是桌面端（Tauri Windows 应用）后端 `src-tauri` 中 `auth` 子系统的**协议层核心**，负责与无锡学院校园网 Dr.COM ePortal 认证系统进行 HTTP 交互。它包含三个文件：

| 文件 | 职责 |
|------|------|
| `auth/portal.rs` | Portal 状态探测：通过 GET 门户首页 HTML 特征判断当前在线/未登录状态，提取在线账号 uid 与运营商后缀 |
| `auth/protocol.rs` | Dr.COM ePortal 登录/注销协议实现：构造并发送登录请求（`/eportal/portal/login`）、Radius 注销（`/eportal/portal/logout`）、MAC 解绑（`/eportal/portal/mac/unbind`），并解析 JSONP 响应 |
| `auth/failure_tracker.rs` | 认证失败与 Portal 请求失败的计数追踪，连续失败达到阈值时触发 MAC 重置（DHCP 释放续租）自愈 |

该模块是整个桌面端认证链路的**底层协议引擎**：上层的命令处理（[`commands/login.rs`](desktop-backend-commands.md)）、服务编排（`auth/service.rs`）、会话包装（`auth/session.rs`）、后台巡检（[`monitor/background_check.rs`](desktop-backend-monitoring.md)）均通过它与门户网关通信。

---

## 架构总览

```mermaid
flowchart TB
    subgraph 上层调用方["上层调用方"]
        CMD["commands/login.rs<br/>do_login / do_logout<br/>commands/network_cmd.rs<br/>check_portal_status"]
        SVC["auth/service.rs<br/>full_login / full_logout<br/>auth/session.rs<br/>login_adapter_with_log"]
        MON["monitor/background_check.rs<br/>run_background_check_blocking<br/>monitor/auto_auth.rs<br/>自动登录/断线重连"]
    end

    subgraph 本模块["desktop_backend_auth_protocol"]
        PORTAL["auth/portal.rs<br/>check_portal_full<br/>ensure_portal_port<br/>operator_suffix_from_uid<br/>redact_credentials"]
        PROTO["auth/protocol.rs<br/>do_login_with_retry<br/>do_logout_with_retry<br/>parse_login_result<br/>parse_logout_result"]
        FAIL["auth/failure_tracker.rs<br/>update_auth_failure_count<br/>handle_portal_request_failure<br/>is_auth_failure"]
    end

    subgraph 依赖["基础设施依赖"]
        NET["network/client.rs<br/>create_safe_http_client<br/>PORTAL_URL (ArcSwap)"]
        ASYNC["infra/async_util.rs<br/>block_on_sync"]
        STATE["infra/state/network.rs<br/>NetworkSnapshot<br/>portal_failure_count<br/>a1/a2_auth_failure_count"]
        EVT["infra/events.rs<br/>EventBus::emit_login_log<br/>emit_mac_reset_cycle"]
    end

    subgraph 外部["Dr.COM ePortal 网关"]
        GW["门户 :80<br/>Dr.COMWebLoginID_* 特征页"]
        EP["门户 :801/eportal/portal/<br/>login / logout / mac/unbind"]
    end

    CMD --> SVC
    CMD --> PORTAL
    SVC --> PROTO
    SVC --> FAIL
    MON --> PORTAL
    MON --> FAIL
    PORTAL --> NET
    PROTO --> NET
    PORTAL --> ASYNC
    PROTO --> ASYNC
    FAIL --> STATE
    FAIL --> EVT
    FAIL --> NET
    NET -->|"GET /"| GW
    NET -->|"GET /eportal/portal/*"| EP
```

---

## 核心组件详解

### 1. `auth/portal.rs` — Portal 状态探测

#### 1.1 核心类型 `PortalStatus`

探测结果序列化结构（前端 `PortalStatusResult` 类型对应，见 [desktop_frontend_auth.md](desktop-frontend-auth.md)）：

```rust
pub struct PortalStatus {
    pub reachable: bool,        // 门户是否可达
    pub login_available: bool,  // 是否可登录（在线时=false，离线时=true）
    pub online: bool,           // 当前是否已在线
    pub message: String,        // 人类可读消息
    pub uid: Option<String>,    // 在线时页面携带的完整账号（如 "2023xxxx@telecom"）
    pub error_kind: Option<String>, // "need_manual_check" / "request_failed"
}
```

#### 1.2 页面检测三态 `PageCheckResult`

```rust
enum PageCheckResult {
    Determined(bool, Option<String>), // 已判定：(是否在线, uid)
    Unknown,                           // 页面特征无法识别
    Failed,                            // HTTP 请求失败
}
```

**状态判定特征表**：

| 页面特征 | 判定结果 |
|----------|----------|
| `Dr.COMWebLoginID_1` / `<title>注销页</title>` | 已在线 |
| `Dr.COMWebLoginID_0` / `Dr.COMWebLoginID_2` / `<title>登录页</title>` | 未登录 |
| `uid='xxx'`（非空） / `v4ip='x.x.x.x'` + `oltime=非0` | 已在线 |

**安全约束**：页面特征无法识别时返回 `error_kind="need_manual_check"`，**绝不携带用户密码调用登录端点**——避免错误凭据在轮询时触发账号锁定。

#### 1.3 关键函数

| 函数 | 作用 |
|------|------|
| `check_portal_full(adapter_ip, adapter_name)` | 完整探测入口：创建绑定源 IP 的 HTTP 客户端 → GET 门户首页 → 分析 HTML 特征 → 返回 `PortalStatus` |
| `ensure_portal_port(base)` | 确保 URL 包含 `:801` 端口（登录/注销/协议请求专用端口；页面探测用原始 80 端口） |
| `operator_suffix_from_uid(uid)` | 从 `2023xxxx@telecom` 提取 `@telecom`/`@unicom`/`@cmcc` 后缀，未知后缀返回 `None` |
| `safe_truncate(s, max_len)` | 按字符边界截断字符串（不切断 UTF-8 多字节字符），用于日志脱敏 |
| `redact_credentials(msg, url, base_url, password)` | 错误信息脱敏：将含凭据的完整 URL、URL 编码密码、明文密码统一替换为 `***` |
| `is_nat_private_ip(ip)` | 判断是否为 NAT 内网 IP（10.x / 172.16-31.x / 192.168.x / 100.64-127.x） |

#### 1.4 端口策略

```mermaid
flowchart LR
    A["portal_base (配置的门户地址)"] --> B{"端口用途"}
    B -->|"页面探测"| C["GET / (原始端口 80)<br/>Dr.COMWebLoginID_* 特征页"]
    B -->|"登录/注销/协议请求"| D["ensure_portal_port()<br/>强制 :801"]
    D --> E["/eportal/portal/login<br/>/eportal/portal/logout<br/>/eportal/portal/mac/unbind"]
```

> **历史缺陷**：v2.2.x 曾把页面探测强制对齐 :801，但 801 是 EPortal SPA 登录表单，已在线状态下仍渲染登录页，导致误报"无法判断"。2026-09-03 回退为原始地址探测。

---

### 2. `auth/protocol.rs` — 登录/注销协议实现

#### 2.1 登录请求流程

```mermaid
sequenceDiagram
    participant Caller as 调用方<br/>(service/session)
    participant DR as do_login_with_retry
    participant DLR as do_login_request
    participant Client as network/client.rs
    participant Portal as ePortal :801

    Caller->>DR: do_login_with_retry(user, password, operator, adapter_ip, max_retries=3, is_quitting)
    loop 最多 3 次尝试
        DR->>DR: 检查 is_quitting 退出标志
        DR->>DLR: do_login_request(...)
        DLR->>DLR: validate_username / validate_operator / validate_password
        DLR->>DLR: 拼接 user_account = user + operator
        DLR->>DLR: 构造 URL: /eportal/portal/login?callback=dr1003&user_account=...&user_password=...&v=random_v()
        DLR->>Client: create_safe_http_client(15s, 绑定 adapter_ip)
        DLR->>Portal: GET login_url (15s timeout)
        Portal-->>DLR: JSONP 响应 dr1003({"result":0,"msg":"认证成功"})
        DLR->>DLR: read_bounded_body (1MB 上限, charset 解码)
        DLR->>DLR: jsonp_json_slice 提取 JSON 对象
        DLR->>DLR: parse_login_result 解析 result/msg
        alt success 或 retryable=false
            DLR-->>DR: Ok(result)
            DR-->>Caller: 返回
        else retryable=true 且未达上限
            DLR-->>DR: Ok(result)
            DR->>DR: wait_cancellable(2000ms, is_quitting) 可中断等待
        end
    end
```

#### 2.2 `parse_login_result` 响应码映射

| `result` 字段 | `msg` 关键词 | 输出 code | success | retryable |
|---------------|-------------|-----------|---------|-----------|
| 0 | "已经在线" | `0` | true | false |
| 0 | "认证成功" | `0` | true | false |
| 0 | "AC认证失败" | `ac_auth_failed` | false | false |
| 0 | 其他未知 msg | `unknown_failure` | false | false |
| 1 | "非法"/"失败"/"错误"/"拒绝" | `1` | false | false |
| 1 | 其他 | `0` | true | false |
| 2 | "已经在线" | `2` | true | false |
| 2 | 其他 | `2` | false | false |
| 3 | （流量超限） | `3` | false | false |
| 4 | （账号禁用） | `4` | false | false |
| 其他 | — | `{result}` | false | **true** |
| JSON 解析失败 | HTML 响应 | `parse_error` | false | false |
| JSON 解析失败 | 非 HTML | `parse_error` | false | **true** |

#### 2.3 注销请求流程（Radius 注销 + MAC 解绑两步）

```mermaid
flowchart TB
    START["do_logout_with_retry"] --> REQ["do_logout_request"]
    REQ --> NAT{"is_nat_private_ip?"}
    NAT -->|"是"| SKIP["wlan_user_ip = 空串"]
    NAT -->|"否"| SEND["wlan_user_ip = adapter_ip"]
    SKIP --> ROUND
    SEND --> ROUND

    subgraph Radius注销["Radius 注销（主操作，最多 2 轮）"]
        ROUND["第 round 轮 GET /eportal/portal/logout<br/>callback=dr100{round+2}<br/>user_account=drcom&user_password=123"] --> PARSE["parse_logout_result"]
        PARSE --> OK{"success?"}
        OK -->|"是"| BREAK["break: any_radius_ok=true"]
        OK -->|"否"| WAIT["可中断等待 1.5s"]
        WAIT --> NEXT{"round < 2?"}
        NEXT -->|"是"| ROUND
        NEXT -->|"否"| UNBIND
    end

    BREAK --> UNBIND

    subgraph MAC解绑["MAC 解绑（best-effort 单次）"]
        UNBIND["GET /eportal/portal/mac/unbind<br/>user_account=username<br/>wlan_user_ip=ip_to_eportal_int"] --> UNBIND_PARSE["parse_logout_result"]
        UNBIND_PARSE --> UNBIND_OK{"success?"}
        UNBIND_OK -->|"是"| COMBINE["any_unbind_ok=true"]
        UNBIND_OK -->|"否"| COMBINE
    end

    COMBINE --> MERGE["merge_logout_results(radius_ok, unbind_ok)"]
    MERGE --> OUTPUT["输出 JSON: code/success/retryable/radiusOk/unbindOk"]
```

**关键设计**：
- **Radius 注销先行**：历史缺陷是"先 MAC 解绑再 Radius 注销"×2 轮，ePortal 的 `mac/unbind` 按 `wlan_user_ip` 踢下线，先 unbind 会让后续 logout 作用在被踢残的会话上，诱发"僵尸"状态
- **结果判据**：调用方判定"离线已生效"须取 `radiusOk || unbindOk`——只看 `success`（仅 Radius 单边）会把 `(false, true)` 误判为注销失败

#### 2.4 辅助工具函数

| 函数 | 作用 |
|------|------|
| `random_v()` | 基于纳秒时间戳生成 1000-9999 的随机数，作为请求的 `v` 参数（防缓存） |
| `read_bounded_body(resp, label)` | 限 1MB 读取响应体，按 Content-Type charset 解码（GBK Portal 的中文关键词依赖正确解码）；同步上下文专用 |
| `read_bounded_body_async(resp, label)` | 同上但 async 版本，读取失败返回 `Err` |
| `jsonp_json_slice(response)` | 按花括号平衡（字符串/转义感知）截取 JSONP 响应中的第一个完整 JSON 对象——修复 msg 含半角 `)` 时 `rfind(')')` 截断 JSON 的缺陷 |
| `wait_cancellable(ms, is_quitting)` | 每 100ms 检查退出标志的可中断等待 |
| `ip_to_eportal_int(ip)` | 点分 IPv4 → ePortal 整数形式（大端序数值） |

---

### 3. `auth/failure_tracker.rs` — 失败计数与自愈触发

#### 3.1 双适配器失败计数器 `AdapterFailureCounter`

```rust
pub enum AdapterFailureCounter {
    A1, // 主适配器
    A2, // 副适配器
}
```

对应 `NetworkSnapshot` 中的三个计数字段：

| 字段 | 含义 | 重置时机 |
|------|------|----------|
| `portal_failure_count` | 单适配器全局认证失败计数 | 登录成功 / 注销成功 |
| `a1_auth_failure_count` | 主适配器认证失败计数 | 该适配器登录成功 / Portal 检测成功 |
| `a2_auth_failure_count` | 副适配器认证失败计数 | 同上 |

#### 3.2 认证失败 vs Portal 请求失败——两类触发路径

```mermaid
flowchart TB
    subgraph 认证失败["路径 A：认证失败（ac_auth_failed / code=1 / code=4）"]
        IS_AUTH["is_auth_failure(result)"] --> CNT1["update_auth_failure_count /<br/>update_dual_adapter_auth_failure"]
        CNT1 --> THRESH1{"连续 >= 5 次?"}
        THRESH1 -->|"是"| WARN1["log_warn: 请检查账号状态/<br/>是否在可登录时段<br/>（仅记录，不触发 MAC 重置）"]
        THRESH1 -->|"否"| HOLD1["保持计数"]
    end

    subgraph 请求失败["路径 B：Portal HTTP 请求不可达（request_failed）"]
        HANDLE["handle_portal_request_failure"] --> GW{"check_gateway_reachable_from?"}
        GW -->|"不可达（校园网断网/维护）"| RESET["重置计数，跳过"]
        GW -->|"可达"| CNT2["递增 a1/a2_auth_failure_count"]
        CNT2 --> THRESH2{"连续 >= 3 次?"}
        THRESH2 -->|"是"| MACRESET["触发该适配器<br/>dhcp_release_renew_single (MAC 重置)"]
        THRESH2 -->|"否"| HOLD2["保持计数"]
        MACRESET --> SUBNET{"skipped && 非校园网子网?"}
        SUBNET -->|"是"| NOTIFY["系统通知 + 应用内日志：<br/>请更换路由器 MAC 地址"]
        SUBNET -->|"否"| DONE["完成，重置计数"]
    end
```

> **触发机制重设计（2026-10-25）**：认证失败**不再**触发 MAC 重置——时间窗外/账号态异常也会返回认证失败码，以此为触发条件会在不可登录时段反复换 MAC/IP。MAC 重置的**唯一触发源**改为"门户 HTTP 不可达"（`handle_portal_request_failure`），阈值从 5 调整为 3。

#### 3.3 关键函数

| 函数 | 作用 |
|------|------|
| `is_auth_failure(result)` | 判断 `CommandResult.data.code` 是否为 `ac_auth_failed`/`1`/`4` |
| `update_auth_failure_count(state, cmd_result, adapter_name)` | 单适配器认证失败计数（原子 `update_with_result` 避免 TOCTOU） |
| `update_dual_adapter_auth_failure(state, r1, r2, a1_name, a2_name)` | 双适配器分别计数 |
| `handle_portal_request_failure(state, app_handle, adapter, adapter_ip, campus_gw, counter, label)` | Portal 请求失败统一处理：网关不可达跳过 → 计数 → 达阈值触发 MAC 重置 |
| `reset_all(state)` | 注销成功时重置所有计数（含 `prep_login_failures`） |

**并发安全**：所有计数读写均通过 `state.network.update_with_result(|s| ...)` 在单次闭包内完成"读-增-判定"，修复历史上 `load()` 读旧值 + 单独 `update()` 增 1 非原子导致的并发双重触发缺陷。

---

## 关键数据流

### 登录完整链路

```mermaid
sequenceDiagram
    participant FE as 前端 useAuth
    participant CMD as commands/login.rs
    participant SVC as auth/service.rs
    participant SES as auth/session.rs
    participant PROTO as auth/protocol.rs
    participant PORTAL as auth/portal.rs
    participant FAIL as failure_tracker
    participant NET as network/client.rs
    participant GW as ePortal 网关

    FE->>CMD: invoke('do_login')
    CMD->>CMD: try_acquire is_logging_in 信号量
    CMD->>SVC: full_login(state, app_handle, adapter_name)
    SVC->>SVC: get_adapters_cached → resolve_adapter_names
    SVC->>SES: login_adapter_with_log(adapter, config, ...)
    SES->>PORTAL: check_portal_full(ip) 预检在线状态
    alt 已在线
        PORTAL-->>SES: online=true → 直通成功
    else 未登录
        PORTAL-->>SES: online=false
        SES->>PROTO: do_login_with_retry(user, pass, op, ip, 3, quit)
        PROTO->>NET: create_safe_http_client(绑定源IP)
        PROTO->>GW: GET /eportal/portal/login?user_account=...&user_password=...
        GW-->>PROTO: dr1003({"result":0,"msg":"认证成功"})
        PROTO->>PROTO: parse_login_result → success=true
        PROTO-->>SES: Ok({code:"0", success:true})
        SES->>PORTAL: 复核"已经在线"假成功
        SES->>FAIL: 结果非成功 → update_auth_failure_count
        SES-->>SVC: CommandResult
    end
    SVC-->>CMD: CommandResult
    CMD->>CMD: post_login_handler
    CMD-->>FE: CommandResult
```

### 后台巡检中的 Portal 检测与 MAC 重置

```mermaid
sequenceDiagram
    participant BG as monitor/background_check.rs
    participant PC as monitor/portal_check.rs
    participant PORTAL as auth/portal.rs
    participant FAIL as failure_tracker
    participant GW as ePortal 网关
    participant DHCP as network/dhcp.rs

    BG->>BG: run_background_check_blocking
    BG->>BG: 检查静默期/出站切换态 → 跳过 or 继续
    BG->>PC: check_adapter_portal(adapter, app_handle)
    PC->>PORTAL: check_portal_full(ip, name)
    PORTAL->>GW: GET / (80端口特征页)
    alt 请求失败 (error_kind=request_failed)
        GW-->>PORTAL: 连接超时/拒绝
        PORTAL-->>PC: PortalStatus{reachable:false, error_kind:"request_failed"}
        PC-->>BG: PortalCheckResult::Error{is_request_failed:true}
        BG->>BG: portal_reset_window_open(08:30后?)
        BG->>FAIL: handle_portal_request_failure(...)
        FAIL->>FAIL: check_gateway_reachable_from(gw, ip)
        alt 网关可达且连续3次失败
            FAIL->>DHCP: dhcp_release_renew_single(adapter.name, gw)
            DHCP-->>FAIL: MAC重置结果
            FAIL->>FAIL: emit_login_log + emit_mac_reset_cycle
        end
    else 检测成功
        GW-->>PORTAL: HTML 特征页
        PORTAL-->>PC: PortalStatus{online: true/false, uid}
        PC-->>BG: PortalCheckResult::Success{online, uid}
        BG->>BG: operator_suffix_from_uid(uid) → 运营商徽标
    end
```

---

## 安全设计要点

### 凭据保护

| 措施 | 实现位置 | 说明 |
|------|----------|------|
| 错误信息脱敏 | `portal.rs::redact_credentials` | reqwest 错误 Display 携带完整 URL（含 user_password），统一替换为 `base_url?***` |
| 日志截断 | `portal.rs::safe_truncate` | 日志中的 URL/响应体按字符边界截断 200-500 字符 |
| 响应体限长 | `protocol.rs::read_bounded_body` | 1MB 硬上限，防止异常 Portal 无限响应体耗尽内存 |
| 状态探测不带凭据 | `portal.rs::handle_unknown_page_status` | 页面无法识别时返回 need_manual_check，绝不向登录端点发密码 |
| 请求 URL 日志 | `protocol.rs::do_login_request` | 日志输出 `safe_url = base_url?***`，绝不记录完整查询串 |

> **设计决策**：密码以 GET 查询参数明文传输是 DrCOM/ePortal 协议与校园网环境的固有限制（Portal 页面本身如此提交，无 HTTPS 替代），威胁模型已评估（见 changelog v2.4.2）。

### 并发与退出安全

- **可中断重试**：所有重试循环通过 `is_quitting: &AtomicBool` 每 100ms 检查退出标志，应用退出时立即中止
- **原子计数**：失败计数通过 `update_with_result` 单次闭包完成读-增-判定，避免 TOCTOU 双重触发
- **运行时上下文**：同步线程中的 async HTTP 请求通过 `block_on_sync` 驱动——优先 `Handle::block_on`（带 reactor guard），裸线程回退到自持多线程 Runtime（详见 [desktop_backend_infrastructure.md](desktop-backend-infrastructure.md)）

---

## 与其他模块的关系

### 上游模块（调用本模块）

| 模块 | 调用点 | 说明 |
|------|--------|------|
| [desktop_backend_commands.md](desktop-backend-commands.md) | `commands/login.rs::do_login/do_logout`、`commands/network_cmd.rs::check_portal_status` | Tauri 命令入口，编排本模块的协议调用 |
| [desktop_backend_monitoring.md](desktop-backend-monitoring.md) | `monitor/background_check.rs`、`monitor/auto_auth.rs`、`monitor/portal_check.rs` | 后台巡检周期调用 `check_portal_full`，失败时调用 `handle_portal_request_failure` |
| [desktop_backend_account_management.md](desktop-backend-account-management.md) | 账号凭据加密存储（DPAPI），供登录时读取用户名/密码 | 凭据来源，不直接调用本模块 |

### 下游依赖（本模块依赖）

| 模块 | 依赖内容 | 说明 |
|------|----------|------|
| [desktop_backend_networking.md](desktop-backend-networking.md) | `network/client.rs::create_safe_http_client`、`PORTAL_URL`、`dhcp_release_renew_single`、`check_gateway_reachable_from` | HTTP 客户端池（TLS 1.3/1.2 降级、绑定源 IP、no_proxy）、MAC 重置、网关可达性检测 |
| [desktop_backend_infrastructure.md](desktop-backend-infrastructure.md) | `infra/state/network.rs::NetworkSnapshot`、`infra/events.rs::EventBus`、`infra/async_util.rs::block_on_sync`、`infra/notification::emit_notification` | 原子状态存储、事件总线（login-log/mac-reset-cycle）、同步驱动 async、系统通知 |
| [desktop_backend_configuration.md](desktop-backend-configuration.md) | `config/validate.rs::validate_username/operator/password`、`config/model.rs::Config` | 登录前凭据校验、门户地址/网关等配置 |
| [desktop_backend_platform_specific.md](desktop-backend-platform-specific.md) | `platform/console_output.rs::decode_charset_bytes` | GBK Portal 响应的字符集解码 |

### 前端契约

前端通过 Tauri `invoke` 调用本模块对应的命令（`do_login`、`do_logout`、`check_portal_status`），返回类型对应 [desktop_frontend_auth.md](desktop-frontend-auth.md) 中的 `LoginResult` / `PortalStatusResult` / `CommandResult`。

---

## 测试覆盖

本模块三个文件均包含 `#[cfg(test)] mod tests` 单元测试：

- **`portal.rs`**：端口拼接、NAT IP 识别、适配器 IP 解析、页面特征判定（注销页/登录页/uid/v4ip）、未知页面安全返回、请求失败状态构建
- **`protocol.rs`**：登录成功/已在线/AC认证失败/非法用户/账号禁用解析、JSONP msg 含半角括号不截断、纯 JSON 响应解析、注销结果合并
- **`failure_tracker.rs`**：认证失败码识别（已知码/网络错误/成功/缺失 code/缺失 data）

测试运行方式见 [build_deployment_configuration.md](build-deployment-configuration.md) 的 `tauri-app/src-tauri/Cargo.toml` 配置。
