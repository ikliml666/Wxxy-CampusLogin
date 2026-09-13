---
title: 网络质量检测：并发延迟测量与等级评定
type: module
source_files:
  - tauri-app/src-tauri/src/network/quality.rs
tags: [network, quality, latency, concurrency, jitter, icmp, doh, tauri-events]
---

## Overview

本模块实现"网络质量检测"：把网关、3 台公共 DNS、2 台 DoH、4 个域名的系统解析、11 个外网站点的 HTTPS 合计 19 项延迟测量，分四波（预览波 → 3 个 Phase1 批次 → 11 项 HTTPS 分批）执行，每一项都返回"基准延迟 + 分段耗时"，再由 `build_quality_result` 聚合成中位数、截尾平均与 `excellent/great/good/fair/poor/bad` 六档等级。结果既作为 Tauri 命令的返回值，也通过 `EventBus::emit_network_quality_result` 增量推送给前端（预览波与 Phase1 完成时 `quality` 被强制标为 `"busy"`）。

本模块全部内容的 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`。`quality.rs` 无任何 `#[cfg]` 平台门控，跨平台编译（安卓端 `android/src-tauri/src/quality_cmds.rs:9` 直接复用 `campus_login_lib::network::quality::{check_network_quality_async, NetworkQualityResult}`），也**没有任何单元测试模块**（全文件 662 行，最后一行即 `check_network_quality_async` 的收尾 `}`）。

## Key Components

### 结果类型与任务模型（`network/quality.rs`）

- `pub struct NetworkQualityResult` — `network/quality.rs:7-18`，`#[derive(Debug, Clone, Serialize)]` + `#[serde(rename_all = "camelCase")]`，前端契约类型。
- `enum LatencyTask` — `network/quality.rs:20-26`，5 个变体描述"测什么"：

```rust
enum LatencyTask {
    Gateway { name: String, target: String },
    Doh { name: String, doh_server: String, doh_ip: String, doh_host: String },
    Https { name: String, host: String },
    DnsServer { name: String, ip: String, domain: String },
    SystemDns { name: String, domains: Vec<String> },
}
```

- `struct LatencyTaskCtx { task: LatencyTask, bind_addr: Option<std::net::IpAddr> }` — `network/quality.rs:28-31`，任务 + 源绑定地址。
- `#[derive(Clone)] struct LatencyResult` — `network/quality.rs:33-46`，单项测量结果（内部类型，不序列化）。

### 底层测量原语

- `async fn ping_host_async(host: &str, timeout_ms: u32) -> Result<u64, String>` — `network/quality.rs:48-93`。`surge_ping` 实现；`V4` 用 `Config::default()`、`V6` 用 `Config::builder().kind(ICMP::V6).build()`（`network/quality.rs:55-58`）；整体 `deadline = now + timeout`（`network/quality.rs:65`）；`PingIdentifier` 取系统时间 `subsec_nanos & 0xFFFF`（`network/quality.rs:69`）；**串行 ping 3 次（seq 0..2），seq 0 的耗时被丢弃不计入**（`network/quality.rs:73-86`，注释意图是跳过首包冷启动）；返回 `total_ms / success_count` 且 `max(1)`；`success_count == 0` 时返回 `Err("ping failed: 100% packet loss")`（`network/quality.rs:88-92`）。
- `async fn check_tcp_latency_async(host, port, timeout_ms: u64, bind_addr) -> i64` — `network/quality.rs:95-134`。`host` 为空直接 -1（`96-98`）；`timeout_ms.clamp(100, 30000)`（`99`）；目标串 `"{host}:{port}"` 解析失败返回 -1（`101-104`）；有 bind 时用 `TcpSocket::new_v4/new_v6` + `bind` + `connect`（`105-120`），任一失败返回 -1；成功返回 `((us + 500) / 1000).max(1)`（`128-131`）。
- `async fn tcp_then_icmp_latency(host, ports, tcp_timeout_ms, bind_addr) -> (i64, &'static str)` — `network/quality.rs:136-165`。先并发 TCP 探测所有 `ports`，**首个非负结果立即 `abort_all()` 并返回 `(lat, "tcp")`**（`145-152`）；全部失败才退回 `ping_host_async(host, 500)`（`154-158`，固定 500ms 预算）；ICMP 成功返回 `(lat, "icmp")`，失败返回 `(-1, "icmp")`（`160-164`）。

### 任务执行与批次调度

- `async fn execute_task(ctx: LatencyTaskCtx, skip_ttfb: bool, skip_content: bool) -> LatencyResult` — `network/quality.rs:167-300`，5 个变体的统一执行体，每个变体的超时与延迟取值口径见下方"各任务参数表"。
- `fn get_latency_level(latency: i64) -> usize` — `network/quality.rs:302-310`，延迟 → 等级下标，阈值见下方表格。
- `async fn run_phase1_batch(tasks: Vec<LatencyTaskCtx>, skip_ttfb, skip_content, is_quitting: &Arc<AtomicBool>) -> Vec<LatencyResult>` — `network/quality.rs:315-338`。用 `JoinSet` 并发整批；循环中每个任务完成时检查 `is_quitting`，置位则 `abort_all()` 并 `break`（`network/quality.rs:329-332`）；结果收集顺序取决于完成顺序。
- `fn build_quality_result<'a>(results: impl Iterator<Item = &'a LatencyResult>, gateway_str: &str, start: Instant) -> NetworkQualityResult` — `network/quality.rs:340-449`，聚合入口，包含中位数、截尾平均、等级判定与 `details`/`metrics` 组装（细节见下方"聚合算法"）。
- `pub async fn check_network_quality_async(_adapter_name: &str, adapter_ip: &str, skip_ttfb: bool, skip_content: bool, fixed_gateway: &str, is_quitting: Arc<std::sync::atomic::AtomicBool>, app_handle: Option<&AppHandle>) -> NetworkQualityResult` — `network/quality.rs:451-662`，**模块唯一公开入口**（经 `network/mod.rs:50` re-export）。

### 检查项的固定目标（硬编码常量）

| 检查项 name | 任务类型 | 目标 | 位置 |
|---|---|---|---|
| `gateway` | Gateway | `fixed_gateway`，为空时 `10.2.127.254`；TCP 端口 `[80, 53]` / TCP 超时 `800ms` / ICMP 预算 `500ms` | `network/quality.rs:454-458`、`170` |
| `baidu` | Https | `www.baidu.com`（预览波） | `network/quality.rs:492-497` |
| `aliDns` | DnsServer | `223.5.5.5`，域名 `www.baidu.com` | `network/quality.rs:524-528` |
| `tencentDns` | DnsServer | `1.12.12.12`，域名 `www.baidu.com` | `network/quality.rs:529-533` |
| `xinfengDns` | DnsServer | `114.114.114.114`，域名 `www.baidu.com` | `network/quality.rs:537-541` |
| `aliDoh` | Doh | 域名 `dns.alidns.com`，IP `223.5.5.5`，查询 `baidu.com` | `network/quality.rs:542-547` |
| `tencentDoh` | Doh | 域名 `doh.pub`，IP `1.12.12.12`，查询 `baidu.com` | `network/quality.rs:548-553` |
| `dnsResolve` | SystemDns | 域名 `www.baidu.com`、`www.bilibili.com`、`www.jd.com`、`cn.bing.com` | `network/quality.rs:557-565` |
| `jd` | Https | `www.jd.com` | `network/quality.rs:593` |
| `bing` | Https | `cn.bing.com` | `network/quality.rs:594` |
| `railway12306` | Https | `www.12306.cn` | `network/quality.rs:595` |
| `lol` | Https | `lol.qq.com` | `network/quality.rs:596` |
| `genshin` | Https | `mhyy.mihoyo.com` | `network/quality.rs:597` |
| `pubg` | Https | `pubg.qq.com` | `network/quality.rs:598` |
| `naraka` | Https | `www.yjwujian.cn` | `network/quality.rs:599` |
| `bilibili` | Https | `www.bilibili.com` | `network/quality.rs:600` |
| `bilibiliLive` | Https | `live.bilibili.com` | `network/quality.rs:601` |
| `douyin` | Https | `www.douyin.com` | `network/quality.rs:602` |
| `douyinLive` | Https | `live.douyin.com` | `network/quality.rs:603` |

合计 19 项（与注释 `network/quality.rs:474-476` 的"全量 19 项约 18s"一致）。

### 各任务类型的超时与延迟取值口径

| 任务 | 超时 | `latency` 取值 | `lat_type` | `is_external` |
|---|---|---|---|---|
| `Gateway` | TCP `800ms` / ICMP `500ms` | TCP 连接耗时，不行则 ICMP 平均 RTT | `"tcp"` 或 `"icmp"` | `false` |
| `Doh` | `2000ms` | `DohTimingResult.total_ms`（失败 -1） | `"doh"` | `true` |
| `Https` | `3000ms` | `HttpTimingResult.total_ms`（失败 -1） | `"https"` | `true` |
| `DnsServer` | `3000ms` | `tcp_ms >= 0 ? tcp_ms : udp_ms` | `"dns"` | `true` |
| `SystemDns` | 每域名 `3000ms` | 成功域名耗时的算术平均（整数除法）；全失败 -1 | `"system-dns"` | `true` |

### 聚合算法（`build_quality_result`，`network/quality.rs:340-449`）

- `external_latency` = 所有 `is_external && latency >= 0` 项延迟的**中位数**（`network/quality.rs:387-395`，偶数个取中间两数平均）。
- `average_external_latency` = 同一集合的**截尾平均**（`network/quality.rs:396-413`）：
  - 样本数 ≥ 4：`trim = ceil(n * 0.15)`，再 `trim = min(trim, n / 2)`，取 `sorted[trim .. n-trim]` 求平均；
  - 样本数 == 3：去掉最大最小后求平均；
  - 样本数 < 3：直接平均；
  - 截尾集合为空时退回全量平均。
  - 两个值都做 `.max(1)` 保护（`network/quality.rs:413`）。
- 无任何外部样本时两者均为 `-1`（`network/quality.rs:414-416`）。
- `gateway_latency` 由 `r.name == "gateway"` 的项直接取（`network/quality.rs:354-356`）。
- 等级：`LEVEL_NAMES: [&str; 6] = ["excellent", "great", "good", "fair", "poor", "bad"]`（`network/quality.rs:418`）；网关与外部都有值时取**两者等级的较大值（更差者）**（`network/quality.rs:420-422`）；只有一方有值时用该方（`423-426`）；都没有则 `"unknown"`（`427-429`）。
- 等级阈值（`get_latency_level`，`network/quality.rs:302-310`）：

| 延迟（ms） | level | 名称 |
|---|---|---|
| `< 0` | 5 | `bad` |
| `<= 20` | 0 | `excellent` |
| `<= 50` | 1 | `great` |
| `<= 100` | 2 | `good` |
| `<= 200` | 3 | `fair` |
| `<= 400` | 4 | `poor` |
| `> 400` | 5 | `bad` |

- `details[name]` 组装规则（`network/quality.rs:357-381`）：基线始终为 `{target, latency, type}`；`dns_ms >= 0` 时追加 `dnsLatency`/`tcpLatency`/`tlsLatency`，且当 `latency > dns_ms + tcp_ms + tls_ms` 时追加 `networkLatency = latency - 三段之和`（`network/quality.rs:360-368`）；`lat_type == "dns"` 时另加 `udpLatency` 与 `tcpLatency`（`369-374`）；`ttfb_ms >= 0` 加 `ttfbLatency`（`375-377`）；`content_ms >= 0` 加 `contentLatency`（`378-380`）。
- `metrics[name] = {"latency", "type", "elapsed"}`（`network/quality.rs:382-384`），其中 `elapsed` 取的是**整轮检测**到当前的耗时（`start.elapsed()`），不是该项自身耗时。
- 顶层 `metrics = {"totalElapsed": 毫秒, "tests": {...}}`（`network/quality.rs:444-447`），`timestamp` = 系统时间的毫秒时间戳（`network/quality.rs:439-442`）。

## 结构体与字段

### `NetworkQualityResult`（`network/quality.rs:7-18`，序列化后为 camelCase）

| 字段 | 类型 | 含义 |
|---|---|---|
| `gateway_latency` | `i64` | 网关延迟；无网关项或失败时为 -1 |
| `external_latency` | `i64` | 外部项延迟的**中位数**；无样本为 -1 |
| `average_external_latency` | `i64` | 外部项延迟的**截尾平均**；无样本为 -1 |
| `gateway` | `String` | 本次检测使用的网关地址（`fixed_gateway` 或默认值） |
| `quality` | `String` | `excellent`/`great`/`good`/`fair`/`poor`/`bad`/`busy`/`unknown`（`busy` 仅出现在增量推送中） |
| `timestamp` | `u64` | 结果生成时刻（UNIX epoch 毫秒） |
| `details` | `serde_json::Value` | 按检查项 name 索引的对象，每项含 `target`/`latency`/`type` 及可选分段字段 |
| `metrics` | `serde_json::Value` | `{"totalElapsed": u64, "tests": {name: {latency, type, elapsed}}}` |

### `LatencyTask`（`network/quality.rs:20-26`，私有）

| 变体 | 字段 | 含义 |
|---|---|---|
| `Gateway` | `name: String`, `target: String` | 网关探测，`name` 固定传 `"gateway"`（`network/quality.rs:486`） |
| `Doh` | `name: String`, `doh_server: String`, `doh_ip: String`, `doh_host: String` | DoH 域名、预置 IP、被查询域名 |
| `Https` | `name: String`, `host: String` | 站点标识与主机名（端口固定 443） |
| `DnsServer` | `name: String`, `ip: String`, `domain: String` | 传统 DNS 服务器与探测域名 |
| `SystemDns` | `name: String`, `domains: Vec<String>` | 一组域名，结果聚合成单项 |

### `LatencyTaskCtx`（`network/quality.rs:28-31`，私有）

| 字段 | 类型 | 含义 |
|---|---|---|
| `task` | `LatencyTask` | 待执行任务 |
| `bind_addr` | `Option<std::net::IpAddr>` | 源绑定地址，来自 `adapter_ip` 解析结果；`None` 表示走系统路由 |

### `LatencyResult`（`network/quality.rs:33-46`，私有，`#[derive(Clone)]`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `name` | `String` | 检查项标识，同时是 `details`/`metrics` 的 key |
| `target` | `String` | 展示用目标描述（如 `10.2.127.254`、`https://www.jd.com`、`223.5.5.5:53`、`4个域名`） |
| `latency` | `i64` | 该项的基准延迟（口径随 `lat_type` 不同），-1 表示失败 |
| `lat_type` | `String` | `"tcp"` / `"icmp"` / `"doh"` / `"https"` / `"dns"` / `"system-dns"` |
| `is_external` | `bool` | 是否参与 `external_latency` / `average_external_latency` 统计（网关为 `false`） |
| `dns_ms` | `i64` | DNS 分段耗时，无该分段为 -1 |
| `tcp_ms` | `i64` | TCP 分段耗时，无该分段为 -1 |
| `tls_ms` | `i64` | TLS 分段耗时，无该分段为 -1 |
| `udp_ms` | `i64` | UDP 查询耗时（仅 `DnsServer` 有值） |
| `ttfb_ms` | `i64` | 首字节耗时（`Https` 取 `HttpTimingResult.ttfb_ms`，`Doh` 取 `DohTimingResult.http_ms`） |
| `content_ms` | `i64` | 内容读取耗时（仅 `Https` 有值） |

## Data Flow

### 整体调度（`check_network_quality_async`）

```text
check_network_quality_async(adapter_name, adapter_ip, skip_ttfb, skip_content,
                            fixed_gateway, is_quitting, app_handle)      [quality.rs:451]
  ├─ gateway = fixed_gateway 非空 ? fixed_gateway : "10.2.127.254"        [:454-458]
  ├─ bind_addr = adapter_ip.parse()（失败记 warning 后 None）             [:462-472]
  ├─ 波 0 · 预览波（2 连接并发，~1-2s）                                    [:478-514]
  │     ├─ Gateway(gateway)
  │     └─ Https("baidu", www.baidu.com)
  │     → build_quality_result(已收集项)；quality 强制 = "busy"
  │     → app_handle.is_some() → EventBus::emit_network_quality_result
  ├─ 批次 1（run_phase1_batch，≤2 并发）                                   [:523-534]
  │     ├─ DnsServer("aliDns", 223.5.5.5, www.baidu.com)
  │     └─ DnsServer("tencentDns", 1.12.12.12, www.baidu.com)
  ├─ 批次 2（≤3 并发）                                                     [:536-554]
  │     ├─ DnsServer("xinfengDns", 114.114.114.114, www.baidu.com)
  │     ├─ Doh("aliDoh", dns.alidns.com, 223.5.5.5, baidu.com)
  │     └─ Doh("tencentDoh", doh.pub, 1.12.12.12, baidu.com)
  ├─ 批次 3（1 并发；内部再按 2 域名/批串行）                              [:556-566]
  │     └─ SystemDns("dnsResolve", [www.baidu.com, www.bilibili.com, www.jd.com, cn.bing.com])
  ├─ is_quitting 检查 → 直接返回全 -1 + quality="unknown" 的空结果        [:568-576]
  ├─ Phase1 增量推送（quality = "busy"）                                   [:579-589]
  ├─ Phase2 · HTTPS 分批（HTTPS_BATCH_SIZE = 4）                           [:606-644]
  │     jd / bing / railway12306 / lol / genshin / pubg
  │     / naraka / bilibili / bilibiliLive / douyin / douyinLive
  │     每批结束 → 拼接 phase1+phase2 再 emit（quality = "busy"）
  └─ 最终聚合：build_quality_result(phase1 + phase2)                       [:646-661]
        并统计 latency < 0 的失败项数并记日志
```

### 单项执行（`execute_task`，`network/quality.rs:167-300`）

```text
Gateway  → tcp_then_icmp_latency(target, [80, 53], 800ms, bind)
             ├─ 并发 TCP 80/53，首个成功即 abort_all → (lat, "tcp")
             └─ 全失败 → ping_host_async(host, 500) → (lat, "icmp") 或 (-1, "icmp")
             （失败额外记 warning，[:171-173]）

Doh      → timing::measure_doh_timing(server, ip, host, bind, 2000ms, skip_ttfb)
             → lat = success ? total_ms : -1
             → dns::update_doh_server_latency(server, lat, success)   [:182]
             → ttfb_ms 字段借位存 r.http_ms                            [:193]

Https    → timing::measure_https_timing(host, 443, bind, 3000ms, skip_ttfb, skip_content) [:206]
             ├─ 失败且 bind.is_some() → 记 info 日志后以 bind = None 重试一次  [:207-210]
             └─ 重试原因：Clash/mihomo TUN 注入 0.0.0.0/1 + 128.0.0.0/1 路由，
                源绑定直连流量出接口不匹配被丢弃（注释 [:198-205]）

DnsServer→ timing::measure_dns_query(ip, domain, bind, 3000ms)        [:230]
             → lat = tcp_ms >= 0 ? tcp_ms : udp_ms
             → dns::update_dns_server_latency(ip, lat, success)       [:235]

SystemDns→ domains.chunks(2) 逐批串行，批内 JoinSet 并发               [:257-276]
             每域名 dns::resolve_host_smart(domain, 3000ms, bind)
             成功项收耗时，失败项记入 failed_domains（汇总 warning）
             lat = 成功项耗时的整数平均，全失败为 -1                    [:277-281]
```

### 增量推送时序（前端视角）

```text
t ≈ 1-2s   预览波完成         → emit quality="busy"（2 项 details）
t ≈ 3-5s   批次 1 完成         （不 emit）
t ≈ 5-7s   批次 2 完成         （不 emit）
t ≈ 7-10s  批次 3 完成
           Phase1 整体完成      → emit quality="busy"（7 项 details）
t ≈ 10-14s HTTPS 第 1 批完成    → emit quality="busy"（11 项）
t ≈ 14-17s HTTPS 第 2 批完成    → emit quality="busy"（15 项）
t ≈ 17-19s HTTPS 第 3 批完成    → emit quality="busy"（19 项）
最后       函数返回            → 真正的 quality 等级（非 busy）由返回值携带
```

（时间轴为按注释 `network/quality.rs:474-476` 与各阶段超时推算的量级，非实测数据。）

## Connections

- [[desktop-network-dns]] — 本模块是 `dns::update_dns_server_latency`（`network/quality.rs:235`）与 `dns::update_doh_server_latency`（`network/quality.rs:182`）的**唯一写入方**，这些分数直接决定 `get_best_dns_servers` / `get_best_doh_servers` 的排序；同时通过 `timing::measure_dns_query` / `measure_doh_timing` / `measure_https_timing` 与 `dns::resolve_host_smart` 完成实际测量。
- [[desktop-network-core]] — `adapter_ip` 作为 `bind_addr` 是 TCP/TLS 出口正确性的关键；测试目标从 `Adapter`/`AdapterDetail` 视图派生。
- [[desktop-monitor]] — `monitor/quality_scheduler.rs:27` 是周期驱动入口（`run_quality_check`），经 `perform_quality_check` 调用本模块；`perform_quality_check` 用 `is_quality_checking` 信号量与手动检测互斥，并在 `emit_network_quality_result` 之外追加了 `classify_quality_change` 的通知门控（`SPIKE_CONFIRM_COUNT = 2`、`SPIKE_CONFIRM_INTERVAL_SECS = 15`，见 `monitor/quality_scheduler.rs:12-13`）。
- [[desktop-commands]] — `commands/network_cmd.rs:195` 是手动检测路径，**传 `app_handle = None`**，因此手动检测不会触发增量推送，只有最终返回值。
- [[desktop-auth]] — 登录/探测链路与质量检测共享 `timing.rs` 的计时原语，通过 adapter 的 `bind_addr` 语义保持一致。
- [[desktop-platform]] — `infra::events::EventBus::emit_network_quality_result`（`infra/events.rs:44`）与 `infra::command_context::CommandContext` 提供事件与状态访问。

## Known Issues

1. **`_adapter_name` 参数完全未使用** — `network/quality.rs:451` 的下划线前缀标明这一点：检测结果不携带"测的是哪张网卡"的任何标识，只有 `adapter_ip` 通过 `bind_addr` 隐式生效。多次检测结果无法在结果体里区分适配器来源。
2. **默认网关硬编码为 `10.2.127.254`** — `network/quality.rs:457`。`fixed_gateway` 为空（默认配置）时**不会**去读适配器的实际网关，而是直接测这个写死的地址；对非该网段的校园网，`gateway` 项会稳定失败（`gateway_latency = -1`），等级退化到只由外部项决定。
3. **`LatencyResult.latency` 的语义随 `lat_type` 变化** — `network/quality.rs:167-300`：`tcp`/`icmp` 是单一握手/往返耗时，`dns` 优先取 `tcp_ms`，`doh`/`https` 是含 DNS+TCP+TLS+HTTP 的 `total_ms`，`system-dns` 是 4 个域名的平均。这些值被一起丢进 `external_values` 参与中位数与截尾平均（`network/quality.rs:351-353`），量级与含义并不齐质。
4. **`metrics` 的 `elapsed` 不是该项耗时** — `network/quality.rs:382-384`：`"elapsed": start.elapsed().as_millis()` 用的是整轮检测起点到当前的时间；由于 `build_quality_result` 会在增量推送中被多次调用，同一个任务在不同推送里的 `elapsed` 值不同，且始终大于等于其自身耗时。
5. **`run_phase1_batch` 的退出检查发生在任务完成之后** — `network/quality.rs:328-332`：循环体先 `join_next().await` 再检查 `is_quitting`，退出标志置位后至少要等一个任务（最长 3 秒，`DnsServer` 超时）返回才生效；预览波（`network/quality.rs:498-502`）则完全没有 `is_quitting` 检查。
6. **`check_network_quality_async` 的提前退出返回残缺结果** — `network/quality.rs:568-576`：`is_quitting` 置位时返回的空结果 `quality = "unknown"`、`details` 与 `metrics.tests` 均为空对象，与"正常但全部失败"的结果在结构上有差异（后者有 19 个 key），消费方需自行区分。
7. **`ping_host_async` 丢弃 seq 0 导致两次成功才有效** — `network/quality.rs:73-82`：`seq == 0` 的耗时不计入 `total_ms`/`success_count`。只有第一个包返回、后两个超时时 `success_count == 0`，结果为 `Err("ping failed: 100% packet loss")`，尽管确实收到了一个响应。
8. **网关探测的两个 TCP 端口在校园网基本不开放** — `network/quality.rs:170`：`[80, 53]`。网关通常不监听这两个端口，实际几乎总是落到 `ping_host_async(host, 500)` 的 ICMP 分支；而 ICMP 在部分网络被禁时该项直接失败。
9. **HTTPS 项在代理 TUN 场景会多花一整轮超时** — `network/quality.rs:206-210`：绑定失败后无绑定重试一次，单站点最坏消耗 `3s + 3s`；11 个站点按 4 并发分 3 批，最坏叠加明显（注释 `network/quality.rs:198-205` 说明了回退的必要性）。
10. **`SystemDns` 的失败域名只进日志不进结果** — `network/quality.rs:282-284`：`failed_domains` 仅用于 warning 日志，`details.dnsResolve` 里只有平均延迟与 `target = "4个域名"`，前端无法知道哪几个域名解析失败。
11. **`SystemDns` 平均延迟掩盖部分失败** — `network/quality.rs:277-281`：4 个域名中 1 个失败、3 个各 50ms 时，`latency = 50`，等级判为 `great`，但注释里的 warning 已经指出失败比例（`3/4` 之类），该信息不进入 `quality` 判定。
12. **所有 `details`/`metrics` 的 key 都是 `name`** — `network/quality.rs:381-384`：`name` 重复会静默覆盖。当前 19 项名字唯一，但新增检查项时必须自行保证唯一性，没有去重或冲突告警。
13. **`baidu` 项从 `https_hosts` 移出但仍在预览波** — `network/quality.rs:591-592`（注释）与 `network/quality.rs:492-497`。若后续改动只在 `https_hosts` 增删站点，容易忽略预览波这条独立路径。
14. **`quality = "busy"` 是协议层约定而非类型约束** — `network/quality.rs:505`、`581`、`635` 手动赋值，`NetworkQualityResult.quality` 是 `String`，"busy" 不在 `LEVEL_NAMES`（`network/quality.rs:418`）里，前端必须自行识别该哨兵值。
15. **模块没有任何单元测试** — `network/quality.rs` 全文件 662 行无 `#[cfg(test)]` 模块；`get_latency_level`、`build_quality_result` 的中位数/截尾平均逻辑属于纯函数，是明显可测但当前未覆盖的部分（对比 `network/dns.rs:608-976` 与 `network/subnet.rs:187-222` 均有测试）。
