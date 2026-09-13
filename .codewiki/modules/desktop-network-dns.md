---
title: 网络 DNS：智能解析、DoH 回退链与 DNS/DoH 一键设置
type: module
source_files:
  - tauri-app/src-tauri/src/network/dns.rs
  - tauri-app/src-tauri/src/network/timing.rs
  - tauri-app/src-tauri/src/network/dns_setup.rs
  - tauri-app/src-tauri/src/platform/dns_config.rs
tags: [network, dns, doh, hickory-resolver, tls, timing, windows, netsh]
---

## Overview

本模块负责三件事：① **域名解析**——按历史延迟评分为 DNS 服务器排序，传统 DNS 与 DoH 两路竞速取先到者，带 60 秒结果缓存、Resolver 复用池与"系统 DNS 兜底"；② **分段计时**——把一次 HTTPS/DNS/DoH 请求拆成 DNS、TCP、TLS、TTFB、内容各阶段毫秒数；③ **Windows DNS/DoH 一键设置**——按目标适配器名单写 NameServer 并挂 DoH 模板、注册全局 DoH 加密服务器、清空 DNS 缓存。

本模块全部内容的 `文件:行号` 引用均相对于 `tauri-app/src-tauri/src/`。三个源文件的平台门控差异很大：`dns.rs` 与 `timing.rs` 完全跨平台（安卓端也编译，`dns_setup.rs` 除外）；`dns_setup.rs` 的 Windows 实现是 `#[cfg(target_os = "windows")]`，非 Windows 只剩一个返回 `{"success": false, "message": "仅支持Windows"}` 的存根。

## Key Components

### 解析缓存与评分表（`network/dns.rs`，共 976 行）

- `static ref DNS_CACHE: DashMap<String, (IpAddr, Instant)>` — `network/dns.rs:7`，key = `host` 或 `host@bind_addr`，value = `(IP, 写入时刻)`。
- `static ref DNS_SERVER_SCORES: DashMap<String, ServerScore>` — `network/dns.rs:9`，key = DNS 服务器 IP（如 `223.5.5.5`）。
- `static ref DOH_SERVER_SCORES: DashMap<String, ServerScore>` — `network/dns.rs:10`，key = DoH 域名（如 `dns.alidns.com`）。
- `static ref SYSTEM_RESOLVER_CONFIG: hickory_resolver::config::ResolverConfig` — `network/dns.rs:16-20`，**懒初始化一次**：`hickory_resolver::system_conf::read_system_conf()` 成功则用真实 OS DNS 配置，失败回退 `ResolverConfig::default()`。注释 `network/dns.rs:12-15` 记录历史缺陷：此处曾硬编码 `ResolverConfig::default()`（Google 8.8.8.8/8.8.4.4），在仅 Portal 可达的校园网/捕获门户场景必然失败，使"系统 DNS fallback"形同虚设。
- `const DNS_FALLBACK_SERVERS: &[&str] = &["223.5.5.5", "1.12.12.12", "114.114.114.114"]` — `network/dns.rs:23`，评分表为空的兜底服务器列表。
- `const DOH_FALLBACK_SERVERS: &[(&str, &str)] = &[("dns.alidns.com", "223.5.5.5"), ("doh.pub", "1.12.12.12")]` — `network/dns.rs:24-27`，`(DoH 域名, 预置 IP)`。
- `const DNS_CACHE_TTL_SECS: u64 = 60` — `network/dns.rs:143`。
- `const DNS_CACHE_MAX_ENTRIES: usize = 64` — `network/dns.rs:144`。
- `const RESOLVER_POOL_SIZE: usize = 4` — `network/dns.rs:38`，每个 key 下最多缓存的 Resolver 实例数。
- `const RESOLVER_CACHE_MAX_KEYS: usize = 16` — `network/dns.rs:39`，超出即整体重建。
- `static ref RESOLVER_CACHE: parking_lot::Mutex<HashMap<String, ResolverPoolEntry>>` — `network/dns.rs:47-48`。

### 解析器复用池（`network/dns.rs`）

- `fn resolver_cache_key(bind_addr: Option<IpAddr>, servers: &[String], timeout: Duration) -> String` — `network/dns.rs:51-53`，格式 `"{bind_addr:?}|{servers 逗号连接}|{timeout 毫秒}"`。
- `fn resolver_get_or_create(key, config: ResolverConfig, opts: ResolverOpts) -> Result<Arc<hickory_resolver::Resolver>, String>` — `network/dns.rs:57-82`。key 数超 `RESOLVER_CACHE_MAX_KEYS` 且是新 key 时整体 `cache.clear()`（`network/dns.rs:63-65`）；池未满就 `Resolver::new(config, opts)` 扩容（`network/dns.rs:70-76`），池满后轮转取用 `entry.next`（`network/dns.rs:77-81`）。
- 设计动机（注释 `network/dns.rs:29-37`）：hickory 0.24 同步 `Resolver` 内部自带 current-thread Tokio Runtime，每次新建会重复创建 Runtime + AsyncResolver；而共享单个 Resolver 会被其内部 Mutex 串行化（`quality` 的 `SystemDns` 会并发解析多个域名，串行会把离线超时逐域名累加），故每 key 维护小池。

### 评分写入与排序（`network/dns.rs`）

- `pub fn update_dns_server_latency(ip: &str, latency_ms: i64, success: bool)` — `network/dns.rs:91-97`，写 `DNS_SERVER_SCORES`。
- `pub fn update_doh_server_latency(server: &str, latency_ms: i64, success: bool)` — `network/dns.rs:99-105`，写 `DOH_SERVER_SCORES`。
- `pub(crate) fn get_best_dns_servers() -> Vec<String>` — `network/dns.rs:107-119`。过滤条件：`success == true` **且** `last_tested.elapsed().as_secs() < 600`（`network/dns.rs:109`）；为空返回 `DNS_FALLBACK_SERVERS`（`113-115`）；否则按 `latency_ms` **升序**排序（`117`）。
- `pub(crate) fn get_best_doh_servers() -> Vec<(String, String)>` — `network/dns.rs:121-141`。同样 600 秒窗口与成功过滤；每条的 IP 从 `DOH_FALLBACK_SERVERS` 按名反查，查不到给空串（`124-129`）；为空返回默认两条（`133-137`）；按延迟升序（`139`）。

### DNS 缓存读删（`network/dns.rs`）

- `fn dns_cache_key(host: &str, bind_addr: Option<IpAddr>) -> String` — `network/dns.rs:149-154`，`"{host}@{ip}"` 或裸 `host`。`bind_addr` 参与 key 的理由（注释 `network/dns.rs:146-148`）：split-horizon / 地理 DNS 下同一域名经不同接口可解析到不同 IP。
- `pub(crate) fn dns_cache_get(host: &str, bind_addr: Option<IpAddr>) -> Option<IpAddr>` — `network/dns.rs:156-168`，命中且未过期返回值，否则 `remove_if` 原子清掉过期项并返回 `None`（`network/dns.rs:164-167`）。
- `pub(crate) fn dns_cache_put(host: &str, bind_addr: Option<IpAddr>, ip: IpAddr)` — `network/dns.rs:170-195`。容量清理分两段（BE-D-03）：超 64 条先 `retain` 剔除过期项（`177-180`），仍超限则收集全部 `(key, ts)` 排序后删最旧的 N 条（`181-194`）。
- `pub fn cleanup_expired_dns_cache()` — `network/dns.rs:197-200`，全表 `retain` 清过期；由 `monitor/adapter_watch.rs:35` 的 15s 循环（周期常量 `ADAPTER_WATCH_INTERVAL = 15000`，`monitor/adapter_watch.rs:8`）调用。

### 传统 DNS 解析（`network/dns.rs`）

- `pub(crate) async fn resolve_host_uncached_with_bind(host, timeout, bind_addr) -> Result<IpAddr, String>` — `network/dns.rs:202-284`。流程见 Data Flow。关键配置：服务器端口固定 `53`、`Protocol::Udp`、`trust_negative_responses: false`、`bind_addr` 落到每个 `NameServerConfig`（`network/dns.rs:214-222`）；`ResolverOpts`: `try_tcp_on_error = true`、`timeout`、`attempts = 2`、`num_concurrent_reqs = servers.len().min(3)`（`225-229`）。失败后走系统 fallback 分支（`243-275`），其中 bind 会被重写到每个系统 name server（`247-256`，注释 `244-246` 记录历史缺陷：fallback 曾丢弃 bind_addr 导致 egress 走错接口），`sys_opts.num_concurrent_reqs = 2`，缓存 key 带 `sys|` 前缀（`264`）。
- `pub(crate) async fn dns_lookup(server_ip, domain, bind_addr, timeout, protocol) -> (Result<(), String>, i64)` — `network/dns.rs:286-334`。**单服务器 + 指定协议**的探测（供 `timing.rs` 计时用），返回 `(结果, 耗时毫秒)`；毫秒算法 `((elapsed_us + 500) / 1000).max(1)`（`network/dns.rs:330-332`），失败也返回耗时（便于展示"失败但花了多久"）。缓存 key 形如 `single|{server}|{protocol:?}|{bind:?}|{timeout}`（`network/dns.rs:319`）。

### DNS 报文构造与解析（`network/dns.rs`）

- `pub(crate) fn build_dns_query_wire(domain: &str, qtype: u16) -> Vec<u8>` — `network/dns.rs:336-357`。TXID 取系统时间 `subsec_nanos & 0xFFFF`（`338-341`）；flags 固定 `0x0100`（标准查询 + RD，`343`）；qdcount = 1，其余计数 0（`344-347`）；域名按 label 长度前缀编码（`348-353`）；qtype 与 class IN(1) 追加在尾部（`354-355`）。
- `pub(crate) fn base64url_encode_no_pad(data: &[u8]) -> String` — `network/dns.rs:359-362`，`URL_SAFE_NO_PAD`（RFC 8484 要求）。
- `pub(crate) fn parse_dns_response_wire(data: &[u8]) -> Result<Vec<IpAddr>, String>` — `network/dns.rs:364-401`。长度 < 12 报错（`365-367`）；**校验 QR 位必须为 1**（`371-376`）与 **RCODE 必须为 0**（`377-379`，历史缺陷：原实现只看 ANCOUNT，NXDOMAIN/劫持响应被当作"无答案"吞掉，无法区分 DNS 劫持与解析失败）；跳过 qdcount 个问题段后遍历 ancount，只收 `rtype == 1 && rdlength == 4` 的 A 记录（`388-399`）。
- `pub(crate) fn skip_dns_name(data: &[u8], pos: usize) -> Result<usize, String>` — `network/dns.rs:403-426`，处理 RFC 1035 压缩指针（`len >= 0xC0`）；用 `HashSet<usize>` 记录跳过的 offset，**循环指针直接报错**（`network/dns.rs:419`）。

### DoH 解析（`network/dns.rs`）

- `pub(crate) async fn resolve_via_doh(doh_server, doh_ip, domain, bind_addr, timeout) -> Result<IpAddr, String>` — `network/dns.rs:428-513`。TCP(443) → TLS 握手 → 发 `GET /dns-query?dns=<base64url>`，请求头 `Accept: application/dns-message` + `Connection: close`（`network/dns.rs:449-451`）；读取循环受整体 deadline 约束、响应体上限 `64 * 1024`（`457-482`）；**校验 HTTP 状态必须为 200**（`488-493`，防 4xx/5xx 响应体被当 DNS 报文解析）；按 `Transfer-Encoding: chunked` 重组（`497-503`）；最终 `parse_dns_response_wire` 并取首个 IP（`509-512`）。依赖 `timing::bind_and_connect`（`network/dns.rs:439`）与 `timing::do_tls_handshake`（`network/dns.rs:443`）。
- `fn decode_chunked_body(input: &[u8]) -> Option<Vec<u8>>` — `network/dns.rs:516-536`，RFC 9110 chunked 重组，容忍 chunk 扩展（`3;a=1`）与结尾 CRLF 可选；畸形输入返回 `None`。

### 智能解析入口（`network/dns.rs`）

- `pub async fn resolve_host_smart(host: &str, timeout: Duration, bind_addr: Option<IpAddr>) -> Result<IpAddr, String>` — `network/dns.rs:538-606`。**全项目域名解析的统一入口**（`timing::measure_https_timing` 与 `quality::SystemDns` 均调它）。流程见 Data Flow；DoH 超时被额外钳到 `min(timeout, 3s)`（`network/dns.rs:544`）。

### 分段计时（`network/timing.rs`，共 554 行）

- `static ref TLS_CONNECTOR: TlsConnector` — `network/timing.rs:11-30`。根证书来自 `webpki_roots::TLS_SERVER_ROOTS`（`12-13`），crypto provider 为 `rustls::crypto::ring::default_provider()`（`14`），`with_safe_default_protocol_versions()` 失败时回退显式 `[TLS13, TLS12]`（`17-25`），`Resumption::default()`（`28`）。**全局唯一 TLS 配置**，`timing.rs` 与 `dns.rs` 的 DoH 共用。
- `pub struct HttpTimingResult` — `network/timing.rs:33-46`，字段见下方结构表。
- `pub struct DnsQueryResult` — `network/timing.rs:48-57`。
- `pub struct DohTimingResult` — `network/timing.rs:59-72`。
- `fn ms_from(start: Instant) -> i64` — `network/timing.rs:74-77`，`((us + 500) / 1000).max(1)`，四舍五入且下限 1ms。
- `pub async fn measure_https_timing(host, port, bind_addr, timeout, skip_ttfb, skip_content) -> HttpTimingResult` — `network/timing.rs:79-245`。详见 Data Flow 与下方"阶段超时预算"。
- `pub(crate) async fn bind_and_connect(addr, bind_addr, timeout) -> Result<TcpStream, String>` — `network/timing.rs:247-274`。有 bind 时按目标地址族 `TcpSocket::new_v4/new_v6` → `socket.bind` → `timeout(connect)`（`252-266`）；无 bind 时 `timeout(TcpStream::connect)`（`267-272`）。
- `pub(crate) async fn do_tls_handshake(host, tcp_stream, timeout) -> Result<(TlsStream<TcpStream>, String), String>` — `network/timing.rs:276-300`，返回协商到的协议版本字符串（`TLSv1_3` → `"TLS 1.3"`，`TLSv1_2` → `"TLS 1.2"`，其它走 `{:?}`，`289-297`）。
- `pub async fn measure_dns_query(server_ip, domain, bind_addr, timeout) -> DnsQueryResult` — `network/timing.rs:302-347`。**UDP 与 TCP 同时发起**（`tokio::join!`，`network/timing.rs:319-322`），两者结果独立记录；`success = udp_ms >= 0 || tcp_ms >= 0`（`345`）；双失败时把两条错误拼成 `"UDP: {u} | TCP: {t}"`（`336-343`）。
- `pub async fn measure_doh_timing(doh_server, doh_ip, query_domain, bind_addr, timeout, skip_http) -> DohTimingResult` — `network/timing.rs:349-434`。`connect_timeout = min(timeout, 5s)`、`http_timeout = min(timeout, 5s)`（`373-374`）；`doh_ip` 为空才先做域名解析并记 `dns_ms`（`377-394`）；首次失败且**未**做过域名解析时，回退"解析域名 → 再试一次"（`410-427`）；最终失败固定报 `"DoH请求失败(443不可达)"`（`429`），`total_ms` 退化为各完成阶段求和（`430-432`）。
- `async fn do_doh_https(doh_server, doh_ip, query_domain, bind_addr, connect_timeout, http_timeout, skip_http) -> DohTimingResult` — `network/timing.rs:436-554`，底层实现；`skip_http == true` 时握手完成即 `success = true`、`http_ms = -1`（`486-491`）；读阶段共享一个 `read_deadline`（`513`）防止慢速滴流响应无限拖长；`64 * 1024` 上限（`524`）。

### DNS/DoH 一键设置（`network/dns_setup.rs`，共 207 行）

- `#[cfg(target_os = "windows")] pub fn setup_dns_doh_admin(targets: &[String], family: &str) -> serde_json::Value` — `network/dns_setup.rs:13-202`。从 `commands/network_cmd.rs` 的管理员分支抽出，管理员路径与提权 helper 路径共用同一实现（`helper/mod.rs:111`），避免两处逻辑漂移（注释 `network/dns_setup.rs:3-7`）。`family` 取值 `"ipv4"` / `"ipv6"` / 其它（视作 both，`network/dns_setup.rs:31-40`）。
- `#[cfg(not(target_os = "windows"))] pub fn setup_dns_doh_admin(_targets, _family) -> serde_json::Value` — `network/dns_setup.rs:204-207`，恒返回 `{"success": false, "message": "仅支持Windows"}`。

### 平台 DNS 常量（`platform/dns_config.rs`，被 `dns_setup.rs` 引用）

| 常量 | 值 | 位置（`platform/dns_config.rs`） |
|---|---|---|
| `PRIMARY_DNS` | `223.5.5.5`（阿里） | `:2` |
| `SECONDARY_DNS` | `1.12.12.12`（腾讯 DNSPod） | `:4` |
| `PRIMARY_DNS_V6` | `2400:3200::1` | `:7` |
| `SECONDARY_DNS_V6` | `2402:4e00::` | `:9` |
| `DOH_SERVERS` | 7 条 `(IP, 模板)`：`223.5.5.5`/`223.6.6.6` → `https://dns.alidns.com/dns-query`；`1.12.12.12`/`120.53.53.53` → `https://doh.pub/dns-query`；`2400:3200::1`/`2400:3200:baba::1` → `https://dns.alidns.com/dns-query`；`2402:4e00::` → `https://doh.pub/dns-query` | `:12-21` |

`platform/dns_config.rs` 另提供 `set_dns_via_api`（`:215`，接口级）、`set_profile_dns_via_api`（`:226`，配置文件级）、`clear_adapter_dns_via_api`（`:241`）、`doh_bindings`（`:39`，纯函数，只为实际在 NameServer 列表中的服务器生成 DoH 绑定）。

## 结构体与字段

### `ServerScore`（`network/dns.rs:84-89`，`#[derive(Clone)]`，私有）

| 字段 | 类型 | 含义 |
|---|---|---|
| `latency_ms` | `i64` | 最近一次测得的延迟毫秒（失败时为 -1） |
| `success` | `bool` | 最近一次是否成功；`false` 的条目在 `get_best_*` 中被过滤掉 |
| `last_tested` | `Instant` | 最近测试时刻；超过 600 秒即视为过期不参与排序 |

### `ResolverPoolEntry`（`network/dns.rs:41-44`，私有）

| 字段 | 类型 | 含义 |
|---|---|---|
| `resolvers` | `Vec<Arc<hickory_resolver::Resolver>>` | 该 key 下已创建的 Resolver，最多 `RESOLVER_POOL_SIZE = 4` 个 |
| `next` | `usize` | 池满后的轮转下标 |

### `HttpTimingResult`（`network/timing.rs:33-46`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `url` | `String` | `https://{host}:{port}/` |
| `success` | `bool` | 仅当完整走完 TCP→TLS→(TTFB→内容) 才为 true |
| `error` | `Option<String>` | 失败原因（中文，带阶段名） |
| `dns_ms` | `i64` | DNS 阶段耗时；缓存命中时也计（近似 0）；失败为 -1 |
| `tcp_ms` | `i64` | TCP 连接耗时；失败为 -1 |
| `tls_ms` | `i64` | TLS 握手耗时；失败为 -1 |
| `ttfb_ms` | `i64` | 首字节时间；`skip_ttfb` 或未收到任何字节时为 -1 |
| `content_ms` | `i64` | 首字节到读取结束；`skip_content` 时仍会记账（读到首字节即 break 前记时）、`skip_ttfb` 时为 -1 |
| `total_ms` | `i64` | 从函数入口到结束的总耗时 |
| `tls_version` | `String` | `"TLS 1.3"` / `"TLS 1.2"` / `"{:?}"` |

### `DnsQueryResult`（`network/timing.rs:48-57`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `server` | `String` | 被探测的 DNS 服务器 IP（回填入参） |
| `domain` | `String` | 被解析的域名（回填入参） |
| `success` | `bool` | UDP 或 TCP 任一成功 |
| `error` | `Option<String>` | 双失败时的组合错误文本 |
| `udp_ms` | `i64` | UDP 查询耗时；失败为 -1 |
| `tcp_ms` | `i64` | TCP 查询耗时；失败为 -1 |

### `DohTimingResult`（`network/timing.rs:59-72`）

| 字段 | 类型 | 含义 |
|---|---|---|
| `server` | `String` | DoH 域名（如 `dns.alidns.com`） |
| `success` | `bool` | 请求成功（`skip_http` 时握手成功即算成功） |
| `error` | `Option<String>` | 失败原因 |
| `dns_ms` | `i64` | 前置域名解析耗时（`doh_ip` 直接给定时为 `ms_from` 的近似 0） |
| `tcp_ms` | `i64` | TCP 耗时 |
| `tls_ms` | `i64` | TLS 耗时 |
| `http_ms` | `i64` | 从发出请求到首字节；`skip_http` 为 -1 |
| `total_ms` | `i64` | 成功时总耗时；失败时退化为各完成阶段之和，全无则 -1 |
| `tls_version` | `String` | 协商到的协议版本 |
| `port` | `u16` | 固定 `443` |

## Data Flow

### 智能解析主链路（`resolve_host_smart`）

```text
resolve_host_smart(host, timeout, bind_addr)                    [dns.rs:538]
  ├─ 1. dns_cache_get(host, bind_addr)  命中 → 直接返回          [dns.rs:539]
  ├─ 2. get_best_doh_servers()  → 取 .first()（延迟最低的 1 台）  [dns.rs:543, 552]
  │     doh_timeout = min(timeout, 3s)                            [dns.rs:544]
  └─ 3. JoinSet 并发两条（BE-D-02 竞速降级）                       [dns.rs:546-567]
        ├─ 路 A：resolve_via_doh(server, doh_ip, host, ba, doh_timeout)
        │         TCP(443) → TLS → GET /dns-query?dns=<b64url>
        │         → 校验 HTTP 200 → （chunked?）→ parse_dns_response_wire
        │         → 校验 QR=1 / RCODE=0（DNS 劫持与 NXDOMAIN 均在此被拒）
        └─ 路 B：resolve_host_uncached_with_bind(host, timeout, ba)
                  get_best_dns_servers()（延迟升序，600s 窗口）
                  → 组装 ResolverConfig（多服务器，port 53，UDP，bind_addr）
                  → resolver_get_or_create（复用池）
                  → lookup_ip 成功 → 优先 IPv4，否则首个地址
                  → 失败 → SYSTEM_RESOLVER_CONFIG.clone() + 重写 bind_addr
                          → sys_resolver.lookup_ip（系统 DNS 兜底）
  4. 首个 Ok → set.abort_all() + dns_cache_put(host, bind_addr, ip) + return   [dns.rs:574-578]
  5. 全部失败 → 按 "DoH" 关键字分类统计 doh_failed / dns_failed，
     拼出 "DNS解析失败: DoH(n个失败)+传统DNS均不可用 - {first_error}"      [dns.rs:569-605]
```

### 分段计时主链路（`measure_https_timing`）

```text
measure_https_timing(host, port, bind_addr, timeout, skip_ttfb, skip_content)  [timing.rs:79]
  deadline = now + timeout                                     [timing.rs:103]
  ├─ DNS 阶段：dns_timeout = timeout / 3                       [timing.rs:104]
  │    dns_cache_get 命中 → dns_ms = 近似 0
  │    未命中 → dns::resolve_host_smart(...) → dns_cache_put + dns_ms
  │    失败 → error 文案分四类（DoH+传统均不可用 / 超时 / 可能被劫持 / 其它）[timing.rs:117-131]
  ├─ TCP 阶段：tcp_timeout = min(deadline 剩余, timeout / 3)    [timing.rs:137-148]
  ├─ TLS 阶段：tls_timeout = min(deadline 剩余, timeout / 3)    [timing.rs:153-164]
  └─ HTTP 阶段（skip_ttfb == false 时）
       发 "GET / HTTP/1.1" + Host + Connection: close           [timing.rs:169-171]
       读循环：每轮 http_timeout = min(deadline 剩余, timeout / 3)
              首字节 → ttfb_ms；skip_content 则立即 break       [timing.rs:195-228]
              累计 > 64KB 强制 break                            [timing.rs:207-209]
       一个字节都没收到 → ttfb_ms = content_ms = -1             [timing.rs:233-236]
       skip_ttfb == true → ttfb_ms = content_ms = -1            [timing.rs:237-240]
  total_ms = ms_from(overall_start)；success = true
```

### DoH 计时链路（`measure_doh_timing`）

```text
measure_doh_timing(doh_server, doh_ip, query_domain, bind, timeout, skip_http)  [timing.rs:349]
  ├─ doh_ip 非空 → 直接 parse（used_dns_resolve = false）        [timing.rs:377-384]
  │   doh_ip 为空 → dns::resolve_host_uncached_with_bind（used_dns_resolve = true）[timing.rs:385-393]
  ├─ do_doh_https(...)                                          [timing.rs:397]
  │     TCP（bind_and_connect）→ TLS（do_tls_handshake）
  │     → skip_http ? 标记 success 返回
  │     → 构造 DNS wire → base64url → GET /dns-query?dns=
  │     → read_deadline 内读取（首字节记 http_ms，>64KB 中止）
  ├─ 成功 → 回填 dns_ms 与 total_ms 返回
  └─ 失败且 used_dns_resolve == false → 回退"解析域名再试一次"     [timing.rs:410-427]
      仍失败 → error = "DoH请求失败(443不可达)"                   [timing.rs:429]
```

### DNS/DoH 一键设置链路（`setup_dns_doh_admin`）

```text
commands/network_cmd.rs:299（管理员） 或 helper/mod.rs:111（提权 helper）
  → setup_dns_doh_admin(targets, family)                        [dns_setup.rs:13]
    ├─ get_adapters_force()  → 过滤 !ip.is_empty() && !is_blacklisted && name ∈ targets  [:17-21]
    │    空 → 返回 {"success": false, "message": "未找到目标网络适配器（主/副适配器均无活跃连接）"}  [:23-28]
    ├─ dns_list 按 family 选：
    │    "ipv4" → [223.5.5.5, 1.12.12.12]
    │    "ipv6" → [2400:3200::1, 2402:4e00::]
    │    both   → 四者混合                                        [:31-40]
    ├─ 逐适配器设置：
    │    无线 → clear_adapter_dns_via_api → set_profile_dns_via_api
    │             profile 级失败 → 降级 set_dns_via_api（接口级）
    │             清除失败 → 直接 set_dns_via_api（接口级优先级更高，会覆盖 profile）[:51-90]
    │    有线 → set_dns_via_api                                    [:91-101]
    ├─ 逐条注册全局 DoH：netsh dns add encryption server=<ip>
    │     dohtemplate=<tpl> autoupgrade=yes udpfallback=yes
    │     "已存在"/"already exists" 视为幂等成功，其余入 doh_failed  [:108-143]
    ├─ ipconfig /flushdns（结果被 let _ 忽略）                     [:145-147]
    └─ 拼装结果：success = api_fail.is_empty() && doh_failed.is_empty()
       返回 {success, message, dnsSuccess, dnsFailed, dohAdded, dohFailed}  [:149-194]
```

## Connections

- [[desktop-network-core]] — `dns.rs` 的 `resolve_via_doh` 依赖 `subnet.rs` 之外的 `timing::bind_and_connect` / `do_tls_handshake`；`update_dns_server_latency` / `update_doh_server_latency` 的**唯一写入方**是 [[desktop-network-quality]] 的 `LatencyTask::DnsServer` / `LatencyTask::Doh`。
- [[desktop-network-quality]] — 质量检测通过 `dns::update_*_latency` 喂分，再通过 `dns::resolve_host_smart` 与 `timing::measure_dns_query` / `measure_doh_timing` / `measure_https_timing` 拿分段数据。
- [[desktop-auth]] — 登录/探测流程经 `timing::measure_https_timing` 得到 Portal 可达性与延迟。
- [[desktop-commands]] — `commands/network_cmd.rs:299` 是 DNS/DoH 设置的命令入口；`platform/dns_config.rs:298` 的 `read_adapter_dns_from_registry` 提供当前 DNS/DoH 状态供 UI 展示。
- [[desktop-platform]] — `platform::dns_config`（NameServer/DoH 写入、`doh_bindings` 配对）、`platform::elevation::parse_guid`、`platform::console_output::decode_console_bytes`、`platform::helper_spawn`（提权 helper 复用 `setup_dns_doh_admin`）。
- [[desktop-monitor]] — `monitor/adapter_watch.rs:35` 每 15s 调 `dns::cleanup_expired_dns_cache()`。

## Known Issues

1. **`SYSTEM_RESOLVER_CONFIG` 一生只读一次** — `network/dns.rs:16-20` 是 `lazy_static`，进程内首次访问后不再重读系统 DNS 配置。用户切换 WiFi / 插拔网卡导致系统 DNS 变化后，"系统 DNS fallback"仍会用旧的 name server 列表，直到进程重启。
2. **`resolve_host_smart` 只并发一台 DoH** — `network/dns.rs:552` 明确取 `doh_servers.first()`（BE-D-02 的取舍，注释 `network/dns.rs:548-551`）。若这台 DoH 被墙/被劫持，即使 `get_best_doh_servers()` 里还有第二台可用，也不会在本轮尝试。
3. **错误分类靠字符串包含判断** — `network/dns.rs:580-583`：`if e.contains("DoH") { doh_failed += 1 } else { dns_failed = true }`。传统 DNS 分支的错误文本若碰巧含 "DoH"（例如系统 DNS 报错里带域名）会被错记成 DoH 失败，最终汇总文案（`network/dns.rs:597-603`）可能误述。
4. **`DNS_FALLBACK_SERVERS` 是公网 DNS** — `network/dns.rs:23` 硬编码 `223.5.5.5` / `1.12.12.12` / `114.114.114.114`。在仅 Portal 可达的校园网内这三台都不通，此时传统 DNS 分支必失败，只能靠 DoH 或系统 DNS。BP-5 只修了"系统 fallback 用真实 OS 配置"，没有改这两个兜底列表（`network/dns.rs:24-27` 的 `DOH_FALLBACK_SERVERS` 同理）。
5. **`get_best_dns_servers` 会返回所有 600 秒内成功过的服务器** — `network/dns.rs:108-118` 只按成功 + 时间窗过滤，没有"取前 N 个"的上限。`quality` 只测 3 台（`quality.rs:524-541`），但任何被 `update_dns_server_latency` 写过的服务器都会进列表，`ResolverConfig` 会逐个 `add_name_server`（`network/dns.rs:213-223`），`num_concurrent_reqs` 只钳到 3（`network/dns.rs:229`）。
6. **`RESOLVER_CACHE` 超限即全清** — `network/dns.rs:63-65`：新 key 且已满 16 个时 `cache.clear()`。已取出的 `Arc<Resolver>` 仍存活（无害），但接下来每个 key 都要重建内部 Tokio Runtime，正是 BE-D-01 想避免的开销，属于低频退化。
7. **`parse_dns_response_wire` 遇截断即静默停** — `network/dns.rs:390`：`if pos + 10 > data.len() { break; }`，不报错。畸形响应会返回"部分解析结果"，若一条 A 记录都没解析到则表现为 `"DoH响应无有效A记录"`（`network/dns.rs:512`），与 RCODE 错误的可区分性下降。
8. **`measure_https_timing` 各阶段预算由 `timeout / 3` 硬切** — `network/timing.rs:104`、`137-140`、`153-156`。`timeout` 较小时（例如 `quality` 传 3 秒，`quality.rs:206`）DNS 只有 1 秒预算，慢 DNS 会直接把整次测量判失败。
9. **`measure_doh_timing` 的失败错误文本固定** — `network/timing.rs:429`：无论真实原因是 TCP 不通还是 TLS 握手失败，最终都记为 `"DoH请求失败(443不可达)"`。分阶段数据（`tcp_ms` / `tls_ms`）能部分还原，但 `error` 字段本身会误导。
10. **`measure_doh_timing` 只在"IP 直连"路径做回退重试** — `network/timing.rs:410`：`if !used_dns_resolve` 才回退。若首次就走的域名解析路径且失败（`doh_ip` 为空），不会换 IP 再试。
11. **`skip_http` 路径的成功判定偏松** — `network/timing.rs:486-491`：TLS 握手成功即 `success = true`，不发 DNS 查询。调用方若用 `skip_http = true` 判断"DoH 可用"，得到的其实是"443 端口可握手"。
12. **`flushdns` 结果被丢弃** — `network/dns_setup.rs:145-147` 用 `let _ = ...output()`。清缓存失败不会体现在返回 JSON 里（注释 `network/dns_setup.rs:106-107` 明确批评过同类写法，此处仍保留）。
13. **DoH 注册的幂等判定靠本地化文本** — `network/dns_setup.rs:129`：匹配 `"已存在"` 或 `"already exists"`。非中英文 Windows 上重复注册会被计为 `doh_failed`（注释 `network/dns_setup.rs:124-125` 已声明 netsh 退出码语义未实测确证）。
14. **`setup_dns_doh_admin` 的 `targets` 为空时会误报"未找到适配器"** — `network/dns_setup.rs:18-28`：`targets.iter().any(...)` 对空列表恒为 false，`active` 为空即返回"未找到目标网络适配器（主/副适配器均无活跃连接）"，与实际原因是"调用方没传目标"不可区分。
15. **IPv6-only 模式的已知风险已用文案提示但未做前置校验** — `network/dns_setup.rs:177-179`：`family == "ipv6"` 时追加"请确保当前网络支持IPv6出口，否则域名解析可能失败"，仅提示不阻断。
