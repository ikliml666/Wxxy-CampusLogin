---
title: 协议核心单点共享（双端铁律）
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/mod.rs
  - tauri-app/src-tauri/src/self_service/mod.rs
  - tauri-app/src-tauri/src/network/quality.rs
  - android/src-tauri/Cargo.toml
tags: [决策, 双端, 协议, 依赖]
---

## 背景

项目同时有 Windows 桌面端与安卓端，两端都要实现同一套校园网协议（登录/注销/Portal/自助服务/网络质量）。若两端各写一份实现，协议细节会在两处漂移。

## 决策

登录/注销/Portal/自助服务/网络质量的实现**只存在于桌面 crate**（`tauri-app/src-tauri`），安卓以 Cargo path 依赖复用（见 `android/src-tauri/Cargo.toml:40` 的 `campus-login = { path = "../../tauri-app/src-tauri" }`），**禁止复制协议逻辑**。桌面 cfg 门控模块（`dual_adapter_executor`/`session`/`service`）对安卓不可见（仅 `#[cfg(desktop)]` 编译，见 `auth/mod.rs:6-11`）。

## 理由

协议实现单点存在于桌面 crate，安卓经 path 依赖继承，因此不存在两端实现的兼容窗口问题（同仓库同发版）。所有协议的逆向细节（如 Dr.COM Self 的 hidden checkcode、csrftoken 提取、MD5 密码提交、FLDEXTRA 整体保存陷阱、绑定时忽略预热导致 verify 失败等）集中在一处维护，降低漂移风险。

## 备选方案

未记录。

## 影响与约束

新增协议逻辑只能写在桌面 crate；安卓不得复制。协议之外的能力（平台专属能力如 DPAPI/Keystore、托盘/前台服务）各端自理。在 `auth/mod.rs` 中，桌面专属的适配器发现与后台服务模块被 `#[cfg(desktop)]` 保护（行 6-11），不污染安卓构建；安卓使用 `network-interface` 及专属插件（如 `tauri-plugin-campus-network-bind` 等，见 `Cargo.toml:33-44`）处理平台差异。

## Key Components

### `auth/mod.rs` (tauri-app/src-tauri/src/auth/mod.rs:1-11)

协议认证入口模块，声明子模块：

- **公共模块**（双端均编译，行 1-3）：
  - `failure_tracker` — 登录失败追踪
  - `portal` — Portal/Dr.COM 协议实现
  - `protocol` — 通用协议工具（含 `read_bounded_body_async` 等）
- **桌面门控模块**（`#[cfg(desktop)]`，仅桌面端编译，行 6-11）：
  - `dual_adapter_executor` — 双网卡适配器执行器（依赖桌面适配器发现 `GetAdaptersAddresses`，行 5-7）
  - `session` — 会话管理（行 8-9）
  - `service` — 后台服务（行 10-11）

### `self_service/mod.rs` (tauri-app/src-tauri/src/self_service/mod.rs:1-836)

Dr.COM Self 自助服务系统协议实现，逆向于 2026-09-05 至 2026-10-10（见模块注释行 1-52）。核心内容：

- **常量**：`SELF_BASE_URL = "http://10.1.80.200:8080/Self"`（行 62）
- **正则模式**（行 64-74）：`RE_CHECKCODE`（行 65-66）、`RE_CSRFTOKEN`（行 67-68）、`RE_FLD_VALUE`（行 70-71）、`RE_SWAL_MSG`（行 73-74）
- **数据结构**：
  - `BindParams<'a>`（行 77-88）：绑定请求参数（account/password/operator/phone/sms_password）
  - `OperatorBinding`（行 269-275）：已绑定运营商凭据状态（masked_account/password_set）
  - `SelfServiceChannel`（行 169-215）：执行通道（base/client/local_addr），支持 `direct`（行 181-187）与 `via_webvpn`（行 190-198）构造，含 `fresh_session_client`（行 203-209）与 `url`（行 212-214）
- **公开函数**：
  - `operator_fld_pair(operator: &str) -> Option<(usize, usize)>`（行 91-98）：运营商后缀 → FLDEXTRA 字段序号映射（@cmcc=1/2, @telecom=3/4, @unicom=5/6）
  - `extract_checkcode(html: &str) -> Option<String>`（行 101-103）：提取登录页 hidden checkcode
  - `extract_csrftoken(html: &str) -> Option<String>`（行 106-108）：提取绑定/改密页 csrftoken（UUID）
  - `extract_fld_values(html: &str) -> [String; 6]`（行 113-123）：提取 FLDEXTRA1..6 预填值（整体保存陷阱，只发目标会清空其他）
  - `extract_swal_msg(html: &str) -> Option<String>`（行 126-128）：提取页面内嵌 swal 提示
  - `is_bind_success(msg: &str) -> bool`（行 131-133）：判定绑定成功文本
  - `md5_hex(input: &str) -> String`（行 136-141）：标准小写 MD5 hex
  - `bind_operator_via(params, channel) -> Result<String, String>`（行 220-267）：完整绑定流程（通道化变体，直连或 WebVPN 包装）
  - `query_bind_status_via(account, password, channel) -> Result<[Option<OperatorBinding>; 3], String>`（行 280-292）：查询三大运营商绑定状态（掩码）
  - `reveal_credential_via(account, password, operator, channel) -> Result<(String, String), String>`（行 297-313）：查看明文凭据（调用方需先身份验证）
  - `change_password_via(account, password, new_password, channel) -> Result<String, String>`（行 338-383）：修改自助服务密码（=校园网密码，旧密码即登录密码，成功强制下线）
  - `mask_account(account: &str) -> String`（行 387-402）：隐私掩码（11位手机前3后2，其他按长度打码）
  - `query_dashboard_via(account, password, channel) -> Result<(serde_json::Value, serde_json::Value), String>`（行 572-581）：查询在线设备 + 近期上网记录
  - `query_online_log_via(account, password, start_time, end_time, channel) -> Result<serde_json::Value, String>`（行 586-614）：查询账单页上网记录（原始 `{rows, summary, total}`）
  - `offline_session_via(account, password, session_id, channel) -> Result<(), String>`（行 626-651）：注销指定会话
- **私有/内部函数**：
  - `parse_change_password_response(text: &str) -> Result<String, String>`（行 318-332）：解析改密 JSON 响应
  - `binding_from(acct, pwd) -> Option<OperatorBinding>`（行 404-412）：构造绑定状态
  - `build_session_client(local_addr) -> Result<reqwest::Client, String>`（行 145-157）：构建独立会话客户端（无代理、禁重定向、带 cookie）
  - `login_session(channel, account, password) -> Result<(), String>`（行 416-473）：登录自助系统（前 3 步）
  - `is_retryable_transport_error(err: &str) -> bool`（行 479-488）：传输层错误重试判定
  - `login_session_with_retry(channel, account, password) -> Result<(), String>`（行 497-521）：登录重试包装（最多 3 次，间隔 1s）
  - `login_and_fetch_bind_page(...)`（行 525-545）：登录并返回绑定表单页
  - `fetch_dashboard_json(...)`（行 549-567）：请求 dashboard JSON 接口
  - `parse_offline_success(text: &str) -> bool`（行 617-622）：解析注销响应
- **测试模块**（行 653-836）：`md5_hex_standard_vectors`（行 657-662）、`parse_change_password_response_shapes`（行 664-692）、`extract_checkcode_from_login_page`（行 694-700）、`extract_csrftoken_from_bind_page`（行 702-710）、`extract_swal_msg_success_and_empty`（行 712-728）、`bind_success_judgement`（行 730-735）、`mask_account_phone_front3_back2`（行 737-748）、`operator_fld_mapping`（行 750-757）、`transport_error_retry_classification`（行 759-783）、`offline_success_parsing`（行 785-793）、`extract_fld_values_prefilled_kept_for_other_operators`（行 795-836）

### `network/quality.rs` (tauri-app/src-tauri/src/network/quality.rs:1-669)

网络质量检测模块，多阶段并发延迟测量：

- **数据结构**：
  - `NetworkQualityResult`（行 7-18）：序列化结果（gateway_latency/external_latency/average_external_latency/gateway/quality/timestamp/details/metrics）
  - `LatencyTask`（行 20-26）：延迟任务枚举（Gateway/Doh/Https/DnsServer/SystemDns）
  - `LatencyTaskCtx`（行 28-31）：任务上下文（task + bind_addr）
  - `LatencyResult`（行 33-46）：单次测量结果（name/target/latency/lat_type/is_external/dns_ms/tcp_ms/tls_ms/udp_ms/ttfb_ms/content_ms）
- **私有测量函数**：
  - `ping_host_async(host, timeout_ms) -> Result<u64, String>`（行 48-93）：ICMP ping（surge_ping，3 次取平均）
  - `check_tcp_latency_async(host, port, timeout_ms, bind_addr) -> i64`（行 95-134）：TCP 连接延迟（支持绑定本地地址）
  - `tcp_then_icmp_latency(host, ports, tcp_timeout_ms, bind_addr) -> (i64, &'static str)`（行 136-165）：TCP→ICMP 回退
  - `execute_task(ctx, skip_ttfb, skip_content) -> LatencyResult`（行 167-300）：执行单任务（分派各 LatencyTask 类型；HTTPS 测试绑定失败时回退系统路由，行 207-210）
  - `get_latency_level(latency) -> usize`（行 302-310）：延迟等级（0=excellent … 5=bad）
  - `run_phase1_batch(tasks, skip_ttfb, skip_content, is_quitting) -> Vec<LatencyResult>`（行 315-338）：Phase1 分批执行（支持退出标志中止）
  - `build_quality_result(results, gateway_str, start) -> NetworkQualityResult`（行 340-449）：构建最终结果（中位数/截断平均/质量等级）
- **公开入口**：
  - `check_network_quality_async(_adapter_name, adapter_ip, skip_ttfb, skip_content, fixed_gateway, is_quitting, app_handle, lightweight) -> NetworkQualityResult`（行 455-669）：全量检测流程
    - 预览波（行 483-518：网关 + baidu，1–2s 首屏返回增量 `"busy"`）
    - Phase1 分批（行 531-574：阿里DNS+腾讯DNS → 信风DNS+阿里DoH+腾讯DoH → SystemDns（4 域名2个/批））
    - HTTPS 分批（行 599-652：11 站，每批 4 个，增量推送）
    - `lightweight=true` 时仅预览波即返回（行 526-529）

### `android/src-tauri/Cargo.toml` (android/src-tauri/Cargo.toml:1-58)

安卓端 Cargo 清单，关键依赖：

- `campus-login = { path = "../../tauri-app/src-tauri" }`（行 31）：以 path 依赖形式直接引用桌面端 crate，实现协议核心单点共享。
- 平台专属插件（行 33-44）：`tauri-plugin-campus-network-bind`（WLAN 绑定）、`tauri-plugin-campus-keystore`（AES-GCM 替代 DPAPI）、`tauri-plugin-campus-monitor-service`（前台服务）、`tauri-plugin-biometric`（生物识别验证门）等，处理协议之外的平台能力。

## Architecture

```text
桌面 crate (tauri-app/src-tauri)
┌─────────────────────────────────────────────┐
│ auth/mod.rs                                  │
│  ├─ portal / protocol / failure_tracker      │
│  └─ [cfg(desktop)] session / service / dual  │
├─────────────────────────────────────────────┤
│ self_service/mod.rs  (Dr.COM Self 协议)       │
│  └─ bind_operator / change_password / query_*│
├─────────────────────────────────────────────┤
│ network/quality.rs   (网络质量检测)           │
│  └─ check_network_quality_async              │
└─────────────────────────────────────────────┘
         │
         │ Cargo path 依赖 (android/src-tauri/Cargo.toml:31)
         ▼
┌─────────────────────────────────────────────┐
│ 安卓 crate (android/src-tauri)                │
│  ├─ campus-login (协议复用，无复制)           │
│  └─ 平台插件: keystore / network-bind /      │
│      foreground-service / biometric           │
└─────────────────────────────────────────────┘
```

桌面端的 `#[cfg(desktop)]` 门控模块在安卓 target 下不编译，不影响协议核心的可用性。

## Data Flow

### 自助服务绑定流程（self_service/mod.rs:220-267）

1. `login_and_fetch_bind_page`（行 525-545） → 登录页取 checkcode（行 421-430） → 预热 `randomCode`（行 432-440，跳过则 verify 必失败） → `verify` 登录（行 443-472） → 取绑定表单页（行 532-544）
2. 解析 `csrftoken`（行 227-229） + `FLDEXTRA1..6` 预填值（行 229, 113-123）
3. POST `/Self/service/bind-operator`（行 245-251，按运营商映射填目标字段，其余带回预填原值防清空，行 233-244）
4. 解析 swal 消息判定成功/失败（行 259-266）

### 网络质量检测流程（network/quality.rs:455-669）

1. **预览波**（行 483-518）：网关 + baidu 并发 → 1–2s 后首次 emit `"busy"` 质量
2. **Phase1 分批**（行 531-574）：阿里 DNS+腾讯 DNS → 信风 DNS+阿里 DoH+腾讯 DoH → SystemDns（4 域名 2 个/批）
3. **Phase2 HTTPS**（行 599-652）：11 站分 3 批（每批 4 个），每批完成后增量 emit
4. **汇总**（行 654-669）：聚合全部结果，计算中位数/截断平均/质量等级，输出最终 `NetworkQualityResult`

## Connections

[[ipc-command-name-alignment]]、[[config-field-sets-bidirectional-sync]]、[[android-generated-project-discipline]]、[[credential-single-source]]

## Known Issues

- **绑定表单整体保存陷阱**（`self_service/mod.rs:110-112`）：提交 `bind-operator` 时必须带回其他运营商的预填原值，只发目标运营商字段、其余填空串会把已有绑定清掉（2026-09-05 实测缺陷）。
- **HTTPS 测试双网卡/代理 TUN 劫持**（`network/quality.rs:198-205`）：双网卡场景下系统默认路由可能选中未认证网卡导致 TLS 超时；Clash/mihomo 等代理 TUN 模式会劫持全部外网目标，已通过绑定直连失败回退系统路由缓解（行 207-210）。
- **自助服务服务器偶发连接截断**（`self_service/mod.rs:497-521`）：校园网自助服务服务器（10.1.80.200:8080）偶发连接中途断开/chunked 截断，表现为 `error decoding response body`，需经 `login_session_with_retry` 最多 3 次重建客户端重试。
