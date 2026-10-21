---
title: 网络质量检测后端实现与前端展示架构
type: decision
source_files:
  - tauri-app/src-tauri/src/network/quality.rs
  - tauri-app/frontend/src/monitor/QualityPanel.tsx
tags: [决策, 质量检测, i18n, 前端, Rust]
---

## Overview

网络质量检测模块由后端探测与前端展示两半构成，二者只通过一份 JSON 契约对接：后端在 `tauri-app/src-tauri/src/network/quality.rs` 里跑分阶段并发探测，产出 `NetworkQualityResult`（含 `details`/`metrics` 两个自由形态 JSON 对象）；前端 `tauri-app/frontend/src/monitor/QualityPanel.tsx` 按 `details` 的 **name 键**取数渲染，键名即契约，任何一侧改名都会让对应条目退化成 `--`。

本文记录三件事：后端的分波并发结构（预览波 → Phase 1 → Phase 2）与轻量复核模式、`details`/`metrics` 的键与字段契约、前端四个分类页如何消费这些键。前端每个分类页的 `names` 数组是这份契约在前端侧的唯一落点（`tauri-app/frontend/src/monitor/QualityPanel.tsx:123-160`）。

## Key Components

### 后端：`tauri-app/src-tauri/src/network/quality.rs`

**数据结构**

* `NetworkQualityResult`（`tauri-app/src-tauri/src/network/quality.rs:7-18`）：输出给前端的最终结构，标注 `#[serde(rename_all = "camelCase")]`。
  * `gateway_latency: i64`（L10）：网关延迟，未测到为 `-1`。
  * `external_latency: i64`（L11）：外网测试项延迟的**中位数**。
  * `average_external_latency: i64`（L12）：外网测试项延迟的**截断平均值**。
  * `gateway: String`（L13）：网关地址字符串，默认 `10.2.127.254`。
  * `quality: String`（L14）：质量档位，取值 `excellent`/`great`/`good`/`fair`/`poor`/`bad`/`unknown`，增量帧另有 `busy`。
  * `timestamp: u64`（L15）：Unix 毫秒时间戳，由 `SystemTime::now()` 生成（L439-L442），前端用它推算自动检测倒计时。
  * `details: serde_json::Value`（L16）：对象，键为任务 `name`，值为逐项延迟分解。
  * `metrics: serde_json::Value`（L17）：`{"totalElapsed": u64, "tests": {<name>: {...}}}`，见 L444-L447。

* `LatencyTask`（`tauri-app/src-tauri/src/network/quality.rs:20-26`）：探测任务枚举，5 个变体。
  * `Gateway { name: String, target: String }`（L21）
  * `Doh { name: String, doh_server: String, doh_ip: String, doh_host: String }`（L22）
  * `Https { name: String, host: String }`（L23）
  * `DnsServer { name: String, ip: String, domain: String }`（L24）
  * `SystemDns { name: String, domains: Vec<String> }`（L25）

* `LatencyTaskCtx`（`tauri-app/src-tauri/src/network/quality.rs:28-31`）：`task: LatencyTask` + `bind_addr: Option<std::net::IpAddr>`。`bind_addr` 把 TCP/HTTPS 出口绑到经 Portal 认证的适配器 IP，解决双网卡选错出口的问题。

* `LatencyResult`（`tauri-app/src-tauri/src/network/quality.rs:33-46`，`Clone`）：单任务中间结果。
  * `name: String`（L35）、`target: String`（L36）、`latency: i64`（L37）、`lat_type: String`（L38）、`is_external: bool`（L39）。
  * 阶段分解：`dns_ms`、`tcp_ms`、`tls_ms`、`udp_ms`、`ttfb_ms`、`content_ms`（L40-L45），均为 `i64`，`-1` 表示该阶段不适用或未测到。

**函数**

* `ping_host_async(host: &str, timeout_ms: u32) -> Result<u64, String>`（`tauri-app/src-tauri/src/network/quality.rs:48-93`）：基于 `surge_ping` 的 ICMP。V4/V6 分别构造客户端（L55-L58）；发 3 个包且**丢弃 seq 0**（L78-L81），对成功包求均值再 `.max(1)`；全丢包返回 `Err("ping failed: 100% packet loss")`。

* `check_tcp_latency_async(host: &str, port: u16, timeout_ms: u64, bind_addr: Option<std::net::IpAddr>) -> i64`（`tauri-app/src-tauri/src/network/quality.rs:99-138`）：TCP 建连耗时，微秒转毫秒四舍五入 `((elapsed_us + 500) / 1000).max(1)`（L129-L130）；超时 `clamp(100, 30000)`（L99）；绑定源地址时先 `TcpSocket::bind`（L105-L120）；失败返回 `-1`。

* `tcp_then_icmp_latency(host: &str, ports: &[u16], tcp_timeout_ms: u64, bind_addr: Option<std::net::IpAddr>) -> (i64, &'static str)`（`tauri-app/src-tauri/src/network/quality.rs:136-165`）：对各端口并发 `check_tcp_latency_async`，任一成功即 `abort_all()` 并返回 `(lat, "tcp")`（L145-L152）；全失败退回 ICMP，类型标为 `"icmp"`（L154-L164）。

* `execute_task(ctx: LatencyTaskCtx, skip_ttfb: bool, skip_content: bool) -> LatencyResult`（`tauri-app/src-tauri/src/network/quality.rs:167-300`）：5 个变体的分发器。
  * `Gateway`（L169-L175）：`tcp_then_icmp_latency(&target, &[80, 53], 800, ctx.bind_addr)`；`is_external: false`。
  * `Doh`（L176-L196）：`network::timing::measure_doh_timing`，超时 2000ms；顺手 `network::dns::update_doh_server_latency`（L182）；`ttfb_ms` 取 `r.http_ms`（L193）。
  * `Https`（L197-L228）：`network::timing::measure_https_timing(&host, 443, ctx.bind_addr, std::time::Duration::from_secs(3), ...)`；若绑定直连失败且存在 `ctx.bind_addr`，**回退系统路由重测一次**（L207-L210）。L198-L205 的注释给了两个实测原因：双网卡下默认路由选到未认证网卡导致 TLS 全部超时；Clash/mihomo 的 TUN 模式注入跃点 0 的 `0.0.0.0/1 + 128.0.0.0/1` 路由劫持外网目标，源绑定流量因出接口不匹配被丢弃。
  * `DnsServer`（L229-L249）：`network::timing::measure_dns_query`，3s 超时；`latency` 优先取 `tcp_ms`，否则用 `udp_ms`（L231）；`network::dns::update_dns_server_latency` 回写（L235）。
  * `SystemDns`（L250-L298）：域名 `chunks(2)` 串行分批、批内并发（L257-L276），每域名 `network::dns::resolve_host_smart(domain, 3s, ctx.bind_addr)`；成功项计入 `latencies`（`.max(1)`），失败域名汇总成一条告警（L282-L284）；`latency` 为成功项均值，全失败为 `-1`（L277-L281）；`target` 形如 `"{n}个域名"`（L287）。聚合结果仍是**单个** `dnsResolve` 条目（BE-A-03 注释，L251-L254）。

* `get_latency_level(latency: i64) -> usize`（`tauri-app/src-tauri/src/network/quality.rs:302-310`）：`<0 → 5`，`<=20 → 0`，`<=50 → 1`，`<=100 → 2`，`<=200 → 3`，`<=400 → 4`，其余 `5`。

* `run_phase1_batch(tasks: Vec<LatencyTaskCtx>, skip_ttfb: bool, skip_content: bool, is_quitting: &Arc<AtomicBool>) -> Vec<LatencyResult>`（`tauri-app/src-tauri/src/network/quality.rs:315-338`）：内部 `JoinSet` 并发一批任务，每收一个结果前检查 `is_quitting`，置位则 `abort_all()` 中断（L328-L336）。

* `build_quality_result<'a>(results: impl Iterator<Item = &'a LatencyResult>, gateway_str: &str, start: Instant) -> NetworkQualityResult`（`tauri-app/src-tauri/src/network/quality.rs:340-449`）：契约的实际生产点。
  * `details` 每项固定三字段 `target`/`latency`/`type`（L357-L359）。
  * `dns_ms >= 0` 时补 `dnsLatency`/`tcpLatency`/`tlsLatency`，并在 `latency > dns+tcp+tls` 时补 `networkLatency = latency - segments`（L360-L368）。
  * 仅 `lat_type == "dns"` 时写 `udpLatency`（L369-L371）与 `tcpLatency`（L372-L374）。
  * `ttfb_ms >= 0` 写 `ttfbLatency`（L375-L377）；`content_ms >= 0` 写 `contentLatency`（L378-L380）。
  * `metrics` 每项为 `{latency, type, elapsed}`（L382-L384）。
  * 外网聚合（L387-L416）：先排序取中位数；样本 ≥4 时两端各裁 `ceil(15%)`（不超过一半）再平均，样本为 3 时去掉首尾再平均，≤2 时直接平均；两个输出都 `.max(1)`。
  * 档位（L418-L429）：`LEVEL_NAMES = ["excellent", "great", "good", "fair", "poor", "bad"]`，网关与外网都有效时取**两者等级的最大值**；只有一侧有效时用该侧；都无效为 `unknown`。

* `check_network_quality_async(_adapter_name: &str, adapter_ip: &str, skip_ttfb: bool, skip_content: bool, fixed_gateway: &str, is_quitting: Arc<AtomicBool>, app_handle: Option<&AppHandle>, lightweight: bool) -> NetworkQualityResult`（`tauri-app/src-tauri/src/network/quality.rs:455-670`，带 `#[allow(clippy::too_many_arguments)]`，见 L454）：主入口，前端一次刷新/一轮定时检测对应一次调用。

### 前端：`tauri-app/frontend/src/monitor/QualityPanel.tsx`

* `DETAIL_CATEGORIES`（`tauri-app/frontend/src/monitor/QualityPanel.tsx:38-75`）：四个分类页，每项 `{key, labelKey, icon, names, color, bg, border}`。`names` 就是它消费的 `details` 键集合：
  * `gatewayDns`（L40-L47）：`['gateway', 'aliDoh', 'tencentDoh', 'dnsResolve']`，labelKey `quality.gatewayDns`，图标 `Router`。
  * `http`（L48-L56）：`['baidu', 'jd', 'bing', 'railway12306']`，labelKey `quality.websiteTest`，图标 `MonitorSmartphone`。
  * `stream`（L57-L65）：`['bilibili', 'douyin', 'bilibiliLive', 'douyinLive']`，labelKey `quality.videoPlatform`，图标 `Tv`。
  * `game`（L66-L74）：`['lol', 'genshin', 'pubg', 'naraka']`，labelKey `quality.gameServer`，图标 `Gamepad2`。
  * L35-L37 注释记录 v2.4.2 的合并决策：定时测试卡与详情卡合并，`aliDns`/`tencentDns`/`xinfengDns` **不展示**（后端仍探测，首页 Capsule 消费 `dnsResolve`/`aliDns` 等键）。
* `PAGE_COUNT`（L77）= `DETAIL_CATEGORIES.length`（=4），横滑落页边界与指示条分母都用它。
* `cardItemVariantsNoY`（L79-L82）：只含透明度的进出场变体。
* `QualityPanel`（L84-L536，`memo` 包裹）：props 为 `onUpdateConfig`、`onRefreshQuality?`、`onToggleLatencyTest?`（L29-L33）。内部要点：
  * 订阅 `useQualityStore` 的 `networkQuality`/`isRefreshingQuality`（L86-L87），用 `useConfigStore(useShallow(...))` 订阅 `config`（L90），避免外壳级联重渲染。
  * `isPoorQuality = ['poor','bad'].includes(...)`（L92）驱动外圈红光辉（L197-L203）。
  * `resolveQualityDisplay(networkQuality)` 拆出 `effectiveQuality`/`gatewayLatency`/`externalLatency`/`displayLatency`（L95）；`QUALITY_CONFIG` 未命中回落 `unknown`（L96-L98）。
  * 间隔：`intervalSec = (config.latencyTestInterval || 30000) / 1000`（L100）；`intervalDraft` 为本地草稿，`commitInterval` 在 blur/Enter 时 `clamp(10, 600)` 后回写（L106-L115）。
  * 启停走 `useAsyncLock`（L118-L121），防连点并发。
  * `details = networkQuality?.details ?? {}`（L123），`hasData = !!networkQuality?.details`（L125）。
  * 横滑：`activeIdx`（L131）、`viewportRef` + `ResizeObserver` 测宽（L132-L144）、`x = useMotionValue(0)`（L146）、视口宽度变化时把轨道校正回当前页（L148-L152）、`goTo` 300ms 缓动滑动（L155-L159）、拖拽中按最近页实时改高亮（L164-L169）、`handleDragEnd` 用速度 `±450` 或位移过 `1/3` 视口补判翻页并 spring 落位（stiffness 480 / damping 40，L171-L186）、指示条百分比由 `x` 连续派生（L189-L191）。
  * 渲染：质量徽章 + 刷新按钮（L214-L240）；`LatencyPair`（L246-L249）；两种空态——概览卡四段式（L253-L267）与分类页单块空态（L414-L428）；控制条 `Switch` + 数字 `Input` + 周期进度线（L328-L362）；标签栏 4 等分 + 指示条（L366-L389）；内容轨道 `drag="x"`（L393-L405）。
  * 逐项：`item = detail ? { name, ...detail } : { name, latency: -1, target: '', type: '' }`（L431）；延迟 `< 0` 或 `!hasData` 时显示 `--`（L446-L451）；`LatencyTimeline` 接收 `dnsLatency`/`tcpLatency`/`tlsLatency`/`udpLatency`/`networkLatency`/`ttfbLatency`/`contentLatency`（L453-L464）；UDP 异常提示条件为 `udp > tcp*3 && (udp - tcp) >= 20`（L470-L472）；Tooltip 展示 `target`/`type`/UDP/TCP（L477-L511）；底部显示检测时间（L522-L529）。
  * 名称文案统一走 `t('quality.names.' + name)`（L445），新增探测项必须同时补 `quality.names.*` 词条，否则界面显示原始键名。

## Architecture

```text
┌──────────────────────────────┐        ┌───────────────────────────────────┐
│ App.tsx（前端外壳）           │        │ quality.rs（Rust 后端）           │
│ case 'quality'                │        │ check_network_quality_async       │
│  └ enableNetworkQuality?      │        │  ├ 预览波 gateway+baidu  L482     │
│     QualityPanel              │        │  ├ lightweight 直接返回  L526     │
└───────────┬──────────────────┘        │  ├ Phase1 三批 DNS/DoH   L531     │
            │ props                     │  ├ Phase2 HTTPS 4/批     L617     │
            ▼                           │  └ build_quality_result  L657     │
┌──────────────────────────────┐        │     ├ 中位数+截断均值    L387     │
│ QualityPanel  L84-536         │        │     └ LEVEL_NAMES 取最大 L418     │
│  useQualityStore              │        └───────────────┬───────────────────┘
│  └ networkQuality.details     │◄── EventBus ───────────┘
│  DETAIL_CATEGORIES L38-75     │  emit_network_quality_result（增量 busy 帧）
│  └ 四页横滑 x: useMotionValue │
└──────────────────────────────┘
```

## Data Flow

1. **触发**：手动刷新走 `refreshQuality`（前端回调进入 `QualityPanel`），自动检测由 `config.enableLatencyTest` 的后台循环驱动；两条路径最终都进入 `check_network_quality_async`。
2. **参数归一**：`fixed_gateway` 为空则用默认网关 `10.2.127.254`（`tauri-app/src-tauri/src/network/quality.rs:458-462`）；`adapter_ip` 解析成 `bind_addr`，解析失败打 warn 并置 `None`（L466-L476）。
3. **预览波**（`tauri-app/src-tauri/src/network/quality.rs:458-494`）：仅并发 `gateway` 与 `baidu` 两个连接；完成后若有 `app_handle`，用 `build_quality_result` 造一帧、把 `quality` 覆写为 `"busy"`（L509）后经 `EventBus::emit_network_quality_result` 发出。动机见 L478-L481 注释：全量 19 项约 18s，首屏等不起，需 1-3s 出结论。
4. **轻量复核分支**（`tauri-app/src-tauri/src/network/quality.rs:526-529`）：`lightweight == true` 时直接返回预览波汇总，跳过 Phase 1/2。用于质量恶化复核轮，避免一次恶化触发 3 倍外网流量（L451-L453 注释）。
5. **Phase 1**（`tauri-app/src-tauri/src/network/quality.rs:531-574`）：三批，每批并发 ≤3——
   * 批 1：`aliDns`（`223.5.5.5`）、`tencentDns`（`1.12.12.12`），探测域名 `www.baidu.com`（L532-L542）。
   * 批 2：`xinfengDns`（`114.114.114.114`）、`aliDoh`（`dns.alidns.com` / `223.5.5.5`）、`tencentDoh`（`doh.pub` / `1.12.12.12`）（L544-L562）。
   * 批 3：`dnsResolve`（`www.baidu.com`/`www.bilibili.com`/`www.jd.com`/`cn.bing.com`，内部 2 域名/批）（L564-L574）。
   每批都过 `run_phase1_batch`，`is_quitting` 置位即整批中止。
6. **中断兜底**（`tauri-app/src-tauri/src/network/quality.rs:576-584`）：Phase 1 后若已在退出流程，返回 `quality: "unknown"`、三个延迟 `-1`、`details` 空对象、`metrics` 仅 `totalElapsed`。
7. **Phase 1 增量帧**（`tauri-app/src-tauri/src/network/quality.rs:586-597`）：再次以 `quality="busy"` 发出累计结果，前端先显示网关 + DNS + DoH。
8. **Phase 2**（`tauri-app/src-tauri/src/network/quality.rs:599-652`）：`https_hosts` 11 个站点（jd、bing、railway12306、lol、genshin、pubg、naraka、bilibili、bilibiliLive、douyin、douyinLive，L599-L612），`HTTPS_BATCH_SIZE = 4`（L615）分批并发，每批结束发一帧 `busy`（L640-L651）。
9. **终帧**：合并 `phase1_results + phase2_results` 调 `build_quality_result`（`tauri-app/src-tauri/src/network/quality.rs:654-657`），统计 `latency < 0` 的失败项数并记 warn/info 日志（L659-L667）。
10. **前端消费**：收到帧后 `hasData` 置真，四个分类页按 `DETAIL_CATEGORIES[].names` 从 `details` 取键；缺失键退化为 `latency: -1` 显示 `--`；`timestamp` 同时驱动倒计时进度线（`tauri-app/frontend/src/monitor/QualityPanel.tsx:103-104`、`352-361`）。
11. **跨面板复用**：`details` 的 `dnsResolve`/`aliDns` 等键还被首页 Capsule 消费（`tauri-app/frontend/src/monitor/QualityPanel.tsx:37` 注释），因此后端键名改动的影响面大于本面板。

## Connections

[[network-quality-default-off]]、[[deferred-panel-transition]]、[[dual-platform-sharing]]

## Known Issues

* `_adapter_name` 参数未被使用（`tauri-app/src-tauri/src/network/quality.rs:455`），适配器名不参与判定；出口绑定只依赖 `adapter_ip`。
* `details` 键名即前后端契约，但契约没有类型或常量收口：前端靠 `DETAIL_CATEGORIES[].names` 字面量（`tauri-app/frontend/src/monitor/QualityPanel.tsx:123-160`）匹配，后端靠 `execute_task` 里的字面量（如 `tauri-app/src-tauri/src/network/quality.rs:490`、`533`、`566`）产生，任一侧改名不会有编译错误，只会静默变成 `--`。
* 后端共探测 19 项（gateway + baidu + 11 个 HTTPS + 3 个 DNS 直连 + 2 个 DoH + dnsResolve），前端只展示其中 16 个键；`aliDns`/`tencentDns`/`xinfengDns` 有意不展示（`tauri-app/frontend/src/monitor/QualityPanel.tsx:36`）。
* `udpLatency` 只在 `lat_type == "dns"` 时写入 `details`（`tauri-app/src-tauri/src/network/quality.rs:370-372`），所以 UDP 异常告警（`tauri-app/frontend/src/monitor/QualityPanel.tsx:470-472`）只可能出现在 DNS 类条目上。
* 质量档位取网关等级与外网等级的最大值（`tauri-app/src-tauri/src/network/quality.rs:420-429`），一侧差就整体降档，不参考该侧样本量。
* 增量帧一律把 `quality` 覆写为 `"busy"`（`tauri-app/src-tauri/src/network/quality.rs:509`、`589`、`643`），真实档位只在终帧出现；消费方若直接按档位名匹配，需自行处理该中间态。
* `Https` 绑定直连失败会回退系统路由重测一次（`tauri-app/src-tauri/src/network/quality.rs:207-210`），存在代理 TUN 时第二次测量走的是代理出口而非校园网直连，两次结果的语义不同。