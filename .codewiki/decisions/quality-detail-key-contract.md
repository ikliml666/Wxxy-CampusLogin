---
title: 网络质量检测后端实现与前端展示架构
type: decision
source_files:
  - tauri-app/src-tauri/src/network/quality.rs
  - tauri-app/frontend/src/monitor/QualityPanel.tsx
  - tauri-app/frontend/src/App.tsx
tags: [决策, 质量检测, i18n, 前端, Rust]
---

## Overview

本文档记录网络质量检测模块的后端实现逻辑、前后端数据契约以及前端的展示架构。核心设计包括：后端的分阶段并发探测机制（Preview Phase -> Phase 1 -> Phase 2）、轻量复核模式、基于中位数的延迟聚合算法；前端的横滑标签页详情展示、增量推送更新、以及面板的三处联动控制。

## Key Components

### Backend: `tauri-app/src-tauri/src/network/quality.rs`

**核心数据结构：**

*   **`NetworkQualityResult`**: 最终输出的 JSON 结构。
    *   `gateway_latency`: i64, 网关延迟。
    *   `external_latency`: i64, 外部网络中位延迟。
    *   `average_external_latency`: i64, 外部网络平均延迟。
    *   `gateway`: String, 网关地址字符串。
    *   `quality`: String, 质量等级 ("excellent", "great", ..., "unknown")。
    *   `timestamp`: u64, 时间戳。
    *   `details`: serde_json::Value, 每个测试项的详细延迟分解（按 name 键索引）。
    *   `metrics`: serde_json::Value, 汇总指标（包含 `totalElapsed` 和 `tests` 字典）。

*   **`LatencyTask`**: 枚举，定义不同类型的探测任务。
    *   `Gateway { name, target }`: 网关探测。
    *   `Doh { name, doh_server, doh_ip, doh_host }`: DoH 探测。
    *   `Https { name, host }`: HTTPS 网站探测。
    *   `DnsServer { name, ip, domain }`: DNS 服务器直连探测。
    *   `SystemDns { name, domains }`: 系统 DNS 解析探测。

*   **`LatencyTaskCtx`**: 任务上下文。
    *   `task`: LatencyTask.
    *   `bind_addr`: Option<IpAddr>, 绑定网卡 IP 以解决双网卡/代理路由问题。

*   **`LatencyResult`**: 单个任务的中间结果。
    *   `name`, `target`, `latency`, `lat_type`, `is_external`.
    *   细分延迟：`dns_ms`, `tcp_ms`, `tls_ms`, `udp_ms`, `ttfb_ms`, `content_ms`.

**关键函数：**

*   **`check_network_quality_async`**: 主入口函数。
    *   **参数**: `_adapter_name`, `adapter_ip`, `skip_ttfb`, `skip_content`, `fixed_gateway`, `is_quitting`, `app_handle`, `lightweight`。
    *   **流程**:
        1.  **预览波 (Preview Phase)**: 无论是否轻量模式，首先并发执行 `Gateway` 和 `baidu` (HTTPS)。完成后立即通过 `EventBus` 发送一次 `quality="busy"` 的增量结果，让前端快速显示首屏数据。
        2.  **轻量模式分支**: 如果 `lightweight=true`，直接返回预览波结果。用于质量恶化时的快速复核，避免触发大量外网流量。
        3.  **Phase 1 (DNS/DoH)**: 分三批并发执行：
            *   Batch 1: `aliDns`, `tencentDns`。
            *   Batch 2: `xinfengDns`, `aliDoh`, `tencentDoh`。
            *   Batch 3: `dnsResolve` (SystemDns, 内部域名每批 2 个并发)。
            *   完成后再次通过 `EventBus` 发送增量结果。
        4.  **Phase 2 (HTTPS Sites)**: 遍历 `https_hosts` 数组（除 baidu 外的 11 个站点），每批 4 个并发执行。每批完成后发送增量结果。
        5.  **汇总**: 调用 `build_quality_result` 生成最终结果并记录日志。

*   **`run_phase1_batch`**: 执行一批 Phase 1 任务，支持 `is_quitting` 中断检查。

*   **`execute_task`**: 根据 `LatencyTask` 类型分发具体的探测逻辑。
    *   `Gateway`: 调用 `tcp_then_icmp_latency`。
    *   `Doh`: 调用 `crate::network::timing::measure_doh_timing`。
    *   `Https`: 调用 `crate::network::timing::measure_https_timing`。包含特殊的源地址绑定回退逻辑：先尝试绑定 `ctx.bind_addr`，若失败且绑定了地址，则重试不绑定（处理代理 TUN 劫持场景）。
    *   `DnsServer`: 调用 `crate::network::timing::measure_dns_query`。
    *   `SystemDns`: 调用 `crate::network::dns::resolve_host_smart`，域名按 2 个分批并发。

*   **`build_quality_result`**: 构建最终结果对象。
    *   **延迟聚合算法**: 对外部成功测试项的延迟值进行排序，计算中位数 (`external_latency`) 和截断平均值 (`average_external_latency`)。
        *   如果样本数 >= 4，移除顶部和底部 15% 的数据后求平均。
        *   如果样本数为 3，移除最大最小值后求平均。
    *   **质量等级判定**: 取网关和外网延迟等级的最大值，映射到 `"excellent"` 到 `"bad"`。
    *   **Details 构造**: 为每个任务构建包含 `target`, `latency`, `type` 及细分延迟字段（如 `dnsLatency`, `tcpLatency` 等）的 JSON 对象。

*   **`ping_host_async`**: ICMP Ping 实现，使用 `surge_ping`，发送 3 个包，计算平均成功包的延迟。

*   **`check_tcp_latency_async`**: TCP 连接延迟测量，支持绑定源地址。

*   **`tcp_then_icmp_latency`**: 先并发尝试 TCP (端口 80, 53)，任一成功即返回；否则 fallback 到 ICMP Ping。

*   **`get_latency_level`**: 延迟转等级映射：<20ms(0), <50ms(1), <100ms(2), <200ms(3), <400ms(4), >=400ms(5)。

### Frontend: `tauri-app/frontend/src/monitor/QualityPanel.tsx`

**组件与状态：**

*   **`QualityPanel`**: 主面板组件。
    *   **状态管理**: 使用 `useQualityStore` 获取 `networkQuality` 和 `isRefreshingQuality`。使用 `useConfigStore` 获取 `config`（浅比较订阅）。
    *   **详情分类 (`DETAIL_CATEGORIES`)**: 将后端返回的 `details` 键名分组为四个标签页：
        *   `gatewayDns`: `['gateway', 'aliDoh', 'tencentDoh', 'dnsResolve']`
        *   `http`: `['baidu', 'jd', 'bing', 'railway12306']`
        *   `stream`: `['bilibili', 'douyin', 'bilibiliLive', 'douyinLive']`
        *   `game`: `['lol', 'genshin', 'pubg', 'naraka']`

*   **横滑交互逻辑**:
    *   使用 `framer-motion` 的 `useMotionValue` 和 `drag` 属性实现内容轨道的拖拽和平移。
    *   `viewportRef` 监听宽度变化以校正轨道位置。
    *   `goTo` 函数处理点击标签页切换，带动画。
    *   `handleDragEnd` 处理松手后的落页判定，结合速度和位移判断翻页。

*   **数据展示**:
    *   **概览区**: 显示整体质量徽章、网关/外网延迟对 (`LatencyPair`)。
    *   **控制条**: 自动检测开关 (`Switch`) 和间隔输入 (`Input`)。开启时底部有进度条动画。
    *   **详情列表**: 在选中的标签页下，遍历 `cat.names`，从 `details` 中提取对应键名的数据进行展示。使用 `LatencyTimeline` 组件显示延迟分解。

### Integration: `tauri-app/frontend/src/App.tsx`

*   **App.tsx**: `QualityPanel` 的渲染受 `config.enableNetworkQuality` 控制。若为 `false`，返回 `null`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         App.tsx (Frontend)                          │
│  ┌──────────────────┐     ┌─────────────────────────────────────┐   │
│  │  QualityPanel    │     │  useQualityStore                    │   │
│  │  (React Component)│    │  - networkQuality (JSON Result)      │   │
│  │                  │     │  - isRefreshingQuality               │   │
│  │  - Tab Sliding   │     └─────────────────────────────────────┘   │
│  │  - Detail Grid   │                ▲                              │
│  └──────────────────┘                │ EventBus / IPC               │
│         ▲                           │                              │
│         │ emit_network_quality_result│ (Incremental Updates)        │
│         │                           ▼                              │
│  ┌──────────────────┐     ┌─────────────────────────────────────┐   │
│  │  Config Store    │     │  Backend: quality.rs                 │   │
│  │  - enableNetQual │     │  check_network_quality_async         │   │
│  └──────────────────┘     │  ├─ Preview Phase (GW + Baidu)       │   │
│                          │  ├─ Phase 1 (DNS/DoH, Batched)        │   │
│                          │  └─ Phase 2 (HTTPS, Batched)          │   │
│                          └─────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **启动/触发**: 前端通过 `refreshQuality` 或定时循环调用后端 `check_network_quality_async`。
2.  **预览波 (Fast Path)**: 后端立即执行网关和百度的探测，通过 `EventBus` 推送第一次结果（`quality="busy"`）。前端接收到后更新 UI，用户可在 1-2 秒内看到初步数据。
3.  **分阶段探测**:
    *   后端继续执行 Phase 1 (DNS/DoH) 和 Phase 2 (HTTPS)。
    *   每完成一个批次（Phase 1 结束或每批 HTTPS 结束），再次通过 `EventBus` 推送增量结果。
    *   前端在收到增量结果后，实时更新详情页中的各项延迟数值，无需等待全部完成。
4.  **最终结果**: 所有探测完成后，后端发送最后一次完整结果，前端更新整体质量等级和总耗时。
5.  **轻量模式**: 若 `lightweight=true`（例如检测到网络质量恶化时的复核），后端仅执行预览波并立即返回，跳过后续耗时的 DNS 和 HTTPS 批量测试，以减少流量消耗和等待时间。

## Connections

[[quality-detail-key-contract]]、[[network-quality-default-off]]、[[deferred-panel-transition]]