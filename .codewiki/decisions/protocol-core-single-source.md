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

登录/注销/Portal/自助服务/网络质量的实现**只存在于桌面 crate**（`tauri-app/src-tauri`），安卓以 Cargo path 依赖复用，**禁止复制协议逻辑**。桌面 cfg 门控模块（`app`/`helper`/`monitor`/`update`）对安卓不可见。

## 理由

旧文档只写了"双端铁律"这一结论本身，未展开理由；可确认的事实是协议实现单点存在于桌面 crate，安卓经依赖继承，因此不存在两端实现的兼容窗口问题（旧文档另在质量检测键条目中说明"同仓库同发版无兼容窗口"）。

## 备选方案

旧文档未记录。

## 影响与约束

新增协议逻辑只能写在桌面 crate；安卓不得复制。协议之外的能力（平台专属能力如 DPAPI/Keystore、托盘/前台服务）各端自理。

## Key Components

### `auth/mod.rs`（tauri-app/src-tauri/src/auth/mod.rs:1–11）

协议认证入口模块，声明子模块：

- **公共模块**（双端均编译）：
  - `failure_tracker` — 登录失败追踪
  - `portal` — Portal/Dr.COM 协议实现
  - `protocol` — 通用协议工具（含 `read_bounded_body_async` 等）
- **桌面门控模块**（`#[cfg(desktop)]`，仅桌面端编译，行 6–11）：
  - `dual_adapter_executor` — 双网卡适配器执行器
  - `session` — 会话管理
  - `service` — 后台服务

### `self_service/mod.rs`（tauri-app/src-tauri/src/self_service/mod.rs:1–583）

Dr.COM Self 自助服务系统协议实现，逆向于 2026-09-05。核心内容：

- **常量**：`SELF_BASE_URL = "http://10.1.80.200:8080/Self"`（行 51）
- **正则模式**（行 53–63）：`RE_CHECKCODE`、`RE_CSRFTOKEN`、`RE_FLD_VALUE`、`RE_SWAL_MSG`
- **数据结构**：
  - `BindParams<'a>`（行 66–77）：绑定请求参数（account/password/operator/phone/sms_password）
  - `OperatorBinding`（行 200–205）：已绑定运营商凭据状态（masked_account/password_set）
- **公开函数**：
  - `operator_fld_pair(operator: &str) -> Option<(usize, usize)>`（行 80–87）：运营商 → FLDEXTRA 字段序号映射
  - `extract_checkcode(html: &str) -> Option<String>`（行 90–92）：提取登录页 checkcode
  - `extract_csrftoken(html: &str) -> Option<String>`（行 95–97）：提取绑定页 csrftoken
  - `extract_fld_values(html: &str) -> [String; 6]`（行 102–112）：提取 6 个 FLDEXTRA 预填值
  - `extract_swal_msg(html: &str) -> Option<String>`（行 115–117）：提取 swal 提示文本
  - `is_bind_success(msg: &str) -> bool`（行 120–122）：判定绑定成功
  - `md5_hex(input: &str) -> String`（行 125–130）：小写 MD5 hex
  - `mask_account(account: &str) -> String`（行 245–260）：手机号隐私掩码
  - `async fn bind_operator(params, local_addr) -> Result<String, String>`（行 150–197）：完整绑定流程
  - `async fn query_bind_status(account, password, local_addr) -> Result<[Option<OperatorBinding>; 3], String>`（行 209–221）：查询绑定状态
  - `async fn reveal_credential(account, password, operator, local_addr) -> Result<(String, String), String>`（行 225–241）：查看明文凭据（需身份验证）
  - `async fn login_session(...) -> Result<reqwest::Client, String>`（行 274–329）：登录自助服务系统
  - `async fn query_dashboard(account, password, local_addr) -> Result<(serde_json::Value, serde_json::Value), String>`（行 378–387）：查询在线信息 + 上网记录
  - `async fn query_online_log(account, password, start_time, end_time, local_addr) -> Result<serde_json::Value, String>`（行 392–419）：查询账单页上网记录
  - `async fn offline_session(account, password, session_id, local_addr) -> Result<(), String>`（行 430–454）：注销指定会话
- **测试模块**（行 456–582）：`md5_hex_standard_vectors`、`extract_checkcode_from_login_page`、`extract_csrftoken_from_bind_page`、`extract_swal_msg_success_and_empty`、`bind_success_judgement`、`mask_account_phone_front3_back2`、`operator_fld_mapping`、`offline_success_parsing`、`extract_fld_values_prefilled_kept_for_other_operators`

### `network/quality.rs`（tauri-app/src-tauri/src/network/quality.rs:1–670）

网络质量检测模块，多阶段并发延迟测量：

- **数据结构**：
  - `NetworkQualityResult`（行 7–18）：序列化结果（gateway_latency/external_latency/average_external_latency/gateway/quality/timestamp/details/metrics）
  - `LatencyTask`（行 20–26）：延迟任务枚举（Gateway/Doh/Https/DnsServer/SystemDns）
  - `LatencyTaskCtx`（行 28–31）：任务上下文（task + bind_addr）
  - `LatencyResult`（行 34–46）：单次测量结果（name/target/latency/lat_type/is_external/dns_ms/tcp_ms/tls_ms/udp_ms/ttfb_ms/content_ms）
- **私有测量函数**：
  - `async fn ping_host_async(host, timeout_ms) -> Result<u64, String>`（行 48–93）：ICMP ping（surge_ping）
  - `async fn check_tcp_latency_async(host, port, timeout_ms, bind_addr) -> i64`（行 95–134）：TCP 连接延迟
  - `async fn tcp_then_icmp_latency(host, ports, tcp_timeout_ms, bind_addr) -> (i64, &'static str)`（行 136–165）：TCP→ICMP 回退
  - `async fn execute_task(ctx, skip_ttfb, skip_content) -> LatencyResult`（行 167–299）：执行单任务（按 LatencyTask 类型分派）
  - `fn get_latency_level(latency) -> usize`（行 302–310）：延迟等级（0=excellent … 5=bad）
  - `async fn run_phase1_batch(tasks, skip_ttfb, skip_content, is_quitting) -> Vec<LatencyResult>`（行 315–338）：Phase1 分批执行
  - `fn build_quality_result(results, gateway_str, start) -> NetworkQualityResult`（行 340–449）：构建最终结果（含中位数/截断平均/质量等级计算）
- **公开入口**：
  - `pub async fn check_network_quality_async(_adapter_name, adapter_ip, skip_ttfb, skip_content, fixed_gateway, is_quitting, app_handle, lightweight) -> NetworkQualityResult`（行 455–670）：全量检测流程
    - 预览波（网关 + baidu，1–2s 首屏返回）
    - Phase1 分批（DNS/DoH/SystemDns，每批 ≤3 并发）
    - HTTPS 分批（11 站，每批 4 个，增量推送）
    - `lightweight=true` 时仅预览波即返回

## Architecture

```
桌面 crate (tauri-app/src-tauri)
├── auth/mod.rs              — Portal 协议 + cfg(desktop) 门控模块
├── self_service/mod.rs      — Dr.COM Self 自助服务协议
├── network/quality.rs       — 网络质量检测
└── ...
         │
         │ Cargo path 依赖
         ▼
安卓 crate (android/src-tauri/Cargo.toml:34)
campus-login = { path = "../../tauri-app/src-tauri" }
```

安卓通过 `campus-login` path 依赖直接引用桌面 crate，无需复制任何协议代码。桌面端的 `#[cfg(desktop)]` 门控模块（`dual_adapter_executor`/`session`/`service`）在安卓 target 下不编译，不影响协议核心的可用性。

## Data Flow

### 自助服务绑定流程（self_service/mod.rs:150–197）

1. `login_and_fetch_bind_page` → 登录页取 checkcode → 预热验证码 → verify 登录 → 取绑定表单页
2. 解析 csrftoken + FLDEXTRA1..6 预填值
3. POST `/Self/service/bind-operator`（csrftoken + FLDEXTRA 明文）
4. 解析 swal 消息判定成功/失败

### 网络质量检测流程（network/quality.rs:455–670）

1. **预览波**（行 483–518）：网关 + baidu 并发 → 1–2s 后首次 emit `"busy"` 质量
2. **Phase1 分批**（行 531–574）：阿里 DNS+腾讯 DNS → 信风 DNS+阿里 DoH+腾讯 DoH → SystemDns（4 域名 2 个/批）
3. **Phase2 HTTPS**（行 599–652）：11 站分 3 批（每批 4 个），每批完成后增量 emit
4. **汇总**（行 654–669）：聚合全部结果，计算中位数/截断平均/质量等级，输出最终 `NetworkQualityResult`

## Connections

[[ipc-command-name-alignment]]、[[config-field-sets-bidirectional-sync]]、[[android-generated-project-discipline]]
