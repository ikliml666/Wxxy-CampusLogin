---
title: frontend-monitoring
type: module
source_files:
  - android/frontend/src/monitor/useMonitor.ts
  - android/frontend/src/monitor/index.ts
  - android/frontend/src/monitor/LatencyComponents.tsx
  - android/frontend/src/monitor/LatencyTimeline.tsx
  - android/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/monitor/NetworkQualityCapsule.tsx
  - android/frontend/src/monitor/QualityPanel.tsx
  - android/frontend/src/monitor/SpeedTestPanel.tsx
  - android/frontend/src/monitor/StatusBar.tsx
  - android/frontend/src/monitor/types.ts
tags: [network, monitoring, latency, tauri, frontend, quality]
---

## Overview
`frontend-monitoring` 是应用核心的网络质量监控模块，负责协调前端 UI 与原生后端（Tauri）之间的交互。该模块实现了后台自动检测、延迟测试、实时网络质量可视化以及第三方测速入口。核心职责包括：
1. **生命周期管理**：通过 `useMonitor` Hook 统一管理后台检查（Background Check）和延迟测试（Latency Test）的启停、权限请求及配置持久化。
2. **数据可视化**：提供从宏观胶囊状态到微观延迟分解（DNS/TCP/TLS/TTFB/Content）的多层级视图。
3. **状态同步**：确保前端 UI 与后端服务状态（如 Watcher 崩溃标记、校园网 SSID）保持低延迟同步。
4. **用户体验优化**：处理竞态条件、草稿模式输入、横滑交互及空态引导，解决历史缺陷（如 P1-F6）。

## Key Components

### Hooks & Logic

#### `useMonitor` (`android/frontend/src/monitor/useMonitor.ts`)
中央控制器 Hook，协调后台检查和延迟测试的生命周期。

*   **Store 集成 (L1-L13)**:
    *   使用 `useShallow` 提取最小必要切片：
        *   `useAuthStore`: `bgStatus`, `setBgStatus`。
        *   `useQualityStore`: `networkQuality`, `setNetworkQuality`, `isRefreshingQuality`, `refreshQuality`。
        *   `useConfigStore`: `updateConfigLocal`, `saveConfigDirect`。
*   **并发控制与守卫 (L16-L24)**:
    *   `mountedRef`: 防止组件卸载后更新 State。
    *   `bgToggleBusyRef` & `latencyToggleBusyRef`: 忙闲标志位，防止快速连点导致的并发冲突。
*   **`handleToggleBackgroundCheck(enabled, intervalSec)` (L26-L75)**:
    *   切换后台检查功能。
    *   **流程**: 锁检查 -> `saveConfigDirect` 持久化配置 (修复“先 start 后存”缺陷) -> 权限请求 (通知权限 + Wi-Fi SSID 权限可选) -> `api.startBackgroundCheck` / `stopBackgroundCheck` -> `updateConfigLocal` & `setBgStatus`。
    *   **错误处理**: DEV 模式打印，非 DEV 模式通过 `useLogToastStore.addToast` 展示提示。
*   **`handleTriggerCheck()` (L77-L79)**:
    *   手动触发一次后台检查，调用 `api.triggerBackgroundCheck`。
*   **`handleToggleLatencyTest(enabled, intervalSec)` (L81-L115)**:
    *   切换延迟测试功能。
    *   **流程**: 锁检查 -> `saveConfigDirect` 持久化配置 -> `api.startLatencyTest` / `stopLatencyTest` -> `updateConfigLocal`。
    *   **注意**: 未显式更新 AuthStore 中的特定 status 字段，依赖其他机制联动。
*   **返回值 (L107-L113)**:
    *   展开后的 store 对象及三个控制函数。

### UI Components

#### `MonitorPanel` (`android/frontend/src/monitor/MonitorPanel.tsx`)
后台监控控制面板，管理自动检测启停、间隔设置、校园网校验及定时任务。

*   **`MonitorSettingRow` (L33-L46)**: 通用设置行组件，左侧图标+标题+描述，右侧控件。
*   **核心逻辑**:
    *   **状态订阅 (L53-L55)**: 使用 `useShallow` 浅比较订阅 `config` 和 `bgStatus`。
    *   **草稿模式 (L58-L66)**: `intervalDraft` 和文本输入草稿，仅在 `blur`/`Enter` 时 clamp 并提交，修复 P1-F6 历史缺陷（数字回灌导致无法输入）。
    *   **并发锁 (L67-L75)**: 使用 `useAsyncLock` 包裹 `handleTriggerCheck` 和 `toggleBackgroundCheck`。
    *   **降级提示 (L133-L138)**: 若 `bgStatus.watcherFailed` 为 true，显示 Amber 警告，提示重启后端。
    *   **校园网校验 (L196-L330)**:
        *   开启时请求 Wi-Fi SSID 权限。
        *   固定展示 `requiredNetworkName` ('i-wxxy') 和 `campusGateway` ('10.2.127.254')。
        *   时间范围 `campusCheckStartMinutes` / `EndMinutes`，默认 460-1380。
        *   定时登录/注销：哨兵值 1440 表示禁用；开启时若为哨兵则设默认 480。
        *   状态徽章：实时显示当前 SSID 和是否在校网。

#### `QualityPanel` (`android/frontend/src/monitor/QualityPanel.tsx`)
详细的网络质量检测面板，包含主延迟视图、分类横滑详情页。

*   **`DETAIL_CATEGORIES` (L33-L54)**: 定义 4 个测试类别及其关联的后端 detail keys (`gatewayDns`, `http`, `stream`, `game`)。
*   **核心逻辑**:
    *   **质量展示 (L63-L65)**: 调用 `resolveQualityDisplay`。低质量时触发 `useGlowAnimation` 红色光晕。
    *   **进度轨 (L73-L75, L185-L191)**: 基于 `lastTestAt` 和 `intervalSec` 计算已流逝比例，驱动底部 `scaleX` 动画。
    *   **横滑交互 (L98-L140)**:
        *   使用 `framer-motion` 的 `useMotionValue` 和 `drag="x"`。
        *   `ResizeObserver` 监听视口宽度变化校正轨道位置。
        *   拖拽过程中实时计算 `nearest` 页码。
        *   松手时根据速度和位移判断翻页，并使用 spring 动画落位。
    *   **空态处理 (L165-L175, L235-L245)**: 首次无数据时显示引导卡片，而非纯 Loading。
    *   **详情渲染 (L248-L260)**: 遍历 `DETAIL_CATEGORIES`，渲染 `LatencyTimeline` 和 UDP/TCP 异常警告。
    *   **UDP/TCP 异常检测 (L255-L257)**: 若 `udpLatency > tcpLatency * 3` 且差值 >= 20ms，显示黄色警告 ⚠。

#### `LatencyComponents` (`android/frontend/src/monitor/LatencyComponents.tsx`)
延迟可视化核心组件，包括信号强度条渲染和网关/外部延迟对比视图。

*   **`getSignalCfg(level)` (L12-L20)**: 根据延迟等级从 `QUALITY_CONFIG` 获取配置，默认 fallback 到 `unknown`。
*   **`BAR_SPECS` (L23-L28)**: 定义 5 个信号条的静态规格（高度、宽度、动画延迟）。
*   **`SignalGlowDot` (L31-L64)**: 记忆化组件，渲染信号条顶部的发光点。根据 `animActive` 决定 CSS class 或静态样式。
*   **`SignalBars` (L66-L135)**:
    *   接收 `latency`, `loading`, `compact` 属性。
    *   内部使用 `useRef` 跟踪 `cfg.activeBars` 变化，触发 `barKey` 更新以重置动画。
    *   加载态渲染 `animate-pulse` 灰色条；正常态区分高亮与暗淡条，应用 `signal-bar-enter` 动画。
*   **`LatencyPair` (L137-L182)**: 组合组件，左右分栏显示 Gateway 和 External 延迟。使用 `AnimatedNumber` 显示数值。

#### `LatencyTimeline` (`android/frontend/src/monitor/LatencyTimeline.tsx`)
将单次检测的总延迟拆解为 DNS, TCP, TLS, TTFB, Content 等阶段的进度条可视化。

*   **`SEGMENT_INFO` (L24-L31)**: 映射后端字段键名（如 'UDP', 'DNS', '内容', '网络'）到前端展示信息。注意 '内容'/'网络' 是中文键名，需保持与后端 `quality.rs` 契约一致。
*   **`LEVEL_BAR_COLOR` (L33-L39)**: 定义不同质量等级对应的整体条颜色。
*   **核心逻辑**:
    *   **Segments 计算 (L42-L68)**: 过滤出 >0 的分项，构建段数组并计算总和。
    *   **溢出处理 (L85-L91)**: 若多段最小宽度下限(3%)之和超过 100%，则按比例归一化，保证 `width <= 100%`。
    *   **无数据处理 (L71-L73)**: `totalMs < 0` 时显示 muted 灰色占位，避免误判为 'bad' 红色。
    *   **Tooltip (L99-L115)**: 展示阶段描述。

#### `NetworkQualityCapsule` (`android/frontend/src/monitor/NetworkQualityCapsule.tsx`)
悬浮胶囊组件，用于在页面其他位置紧凑展示当前网络质量，Hover 展开详情。

*   **`LatencyRow` (L18-L33)**: 单行延迟展示，带图标、标签、子标签和动态数字。
*   **`getQualityCapsuleBg` (L36-L41)**: 根据质量等级 hex 码生成半透明背景色。
*   **核心逻辑**:
    *   **数据提取 (L52-L53)**: 使用 `extractGatewayLatency`, `extractExternalLatency` 解析 `networkQuality`。
    *   **有效质量推断 (L78-L80)**: 若状态为 'busy'/'unknown' 但有延迟数据，按延迟值反推等级，避免真机上胶囊永远显示“检测中”。
    *   **动画反馈 (L83-L92)**: 监听延迟变化，恶化 (>200ms 或 >1.5倍) 触发 `heartbeat` 动画，改善触发 `flash` 动画。
    *   **Popup 定位 (L94-L126)**: 使用 `createPortal` 挂载到 `document.body`。通过 `getBoundingClientRect` 计算位置，并在 scroll/resize 时 RAF 更新位置。
    *   **Hover 交互 (L128-L133)**: 进入延迟 200ms 后隐藏 popup，防止鼠标移出胶囊但进入 popup 时的闪烁。

#### `SpeedTestPanel` (`android/frontend/src/monitor/SpeedTestPanel.tsx`)
提供第三方测速网站入口，适配移动端和平板的不同布局。

*   **`SPEED_TEST_SITES` (L19-L66)**: 定义 8 个测速站点（Speedtest.cn, Ookla, USTC, NEU, BUPT, Fast.com, Chinaz, ip.cn），包含 URL、分类和图标。
*   **核心逻辑**:
    *   使用 `useFormFactor` 区分手机和平板。
    *   手机端使用 `OnlineDeviceStack` (纵向堆叠)，平板端使用 `OnlineDeviceStackH` (水平封面流)。

#### `StatusBar` (`android/frontend/src/monitor/StatusBar.tsx`)
应用顶部状态栏，聚合连接状态、运营商信息、网络质量胶囊和操作按钮。

*   **细粒度订阅 (L24-L28)**: 单独订阅 `config.user`, `config.operator`, `nightOperatorRestore` 等字段，避免无关变更触发重渲染。
*   **夜切逻辑 (L36-L38)**: 检测 `configOperator === '' && nightOperatorRestore !== ''`，此时徽标显示真实运营商，Tooltip 标注恢复目标。
*   **离线/在线切换动画 (L58-L60)**: 使用 `wasOffline` 标记，离线转在线时应用 `status-enter-from-offline` 动画，纯离线时应用 `status-offline-shake`。
*   **Campus Tooltip (L42-L53)**: 若处于离线状态且有校园网数据，Tooltip 显示具体的 WiFi/有线消息。
*   **Network Quality Capsule (L100-L101)**: 仅在 `enableNetworkQuality` 开启时渲染。
*   **操作按钮 (L110-L133)**: 自助服务（耳机图标）和 Portal 入口（外链图标），带 Hover 动画。

### Types

#### `types.ts` (`android/frontend/src/monitor/types.ts`)
定义模块共享的数据结构。

*   **`BackgroundStatus` (L6-L24)**:
    *   `watcherFailed`: 后端崩溃标记。
    *   `onlineOperator`: 运营商后缀。
    *   `currentSsid`: 当前 SSID。
*   **`NetworkQuality` (L45-L53)**:
    *   `gatewayLatency`: 网关延迟。
    *   `externalLatency`: 外部延迟。
    *   `averageExternalLatency`: 平均外部延迟。
    *   `details`: Record<string, NetworkQualityDetail>。
    *   `metrics`: 指标数据。
*   **`NetworkQualityDetail` (L33-L43)**:
    *   `dnsLatency`, `tcpLatency`, `tlsLatency`, `udpLatency`, `networkLatency`, `ttfbLatency`, `contentLatency`。
*   **`ConnectionCampusStatus` (L1-L4)**: 校园网连接状态细节。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      App / Screen                           │
│  (Imports from index.ts)                                    │
└──────────┬──────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────┐
│                   useMonitor Hook                           │
│  (android/frontend/src/monitor/useMonitor.ts)               │
│  - Manages Lifecycle (Start/Stop Background/Latency)        │
│  - Persists Config (saveConfigDirect)                       │
│  - Requests Permissions (Notification/WiFi SSID)            │
│  - Calls Tauri API (startBackgroundCheck, etc.)             │
└──┬────────────────────┬────────────────────┬────────────────┘
   │                    │                    │
   ▼                    ▼                    ▼
┌─────────┐      ┌──────────────┐      ┌──────────────┐
│AuthStore│      │QualityStore  │      │ConfigStore   │
│(bgStatus)│      │(networkQual.)│      │(local/direct)│
└─────────┘      └──────────────┘      └──────────────┘
           │                    │
           ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│                      UI Components                          │
│                                                             │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │ MonitorPanel │   │QualityPanel  │   │ SpeedTestPanel │  │
│  │ (Settings)   │   │ (Detailed)   │   │ (External)     │  │
│  └──────┬───────┘   └──────┬───────┘   └────────────────┘  │
│         │                  │                               │
│         │                  ▼                               │
│         │          ┌──────────────┐                        │
│         │          │ LatencyPair  │◄─── SignalBars         │
│         │          └──────────────┘                        │
│         │                  │                               │
│         │                  ▼                               │
│         │          ┌──────────────┐                        │
│         │          │LatencyTimeline│                       │
│         │          └──────────────┘                        │
│         │                                                │
│         ▼                                                │
│  ┌──────────────┐                                        │
│  │ StatusBar    │──► NetworkQualityCapsule               │
│  │ (Top Bar)    │    (Floating Popup)                    │
│  └──────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **UI 交互 -> Hook 控制**:
    *   用户在 `MonitorPanel` 或 `QualityPanel` 中触发操作（如切换开关、输入间隔）。
    *   调用 `useMonitor` 暴露的 `handleToggleBackgroundCheck` 或 `handleToggleLatencyTest`。
2.  **配置持久化优先**:
    *   Hook 首先调用 `store.saveConfigDirect` 将配置写入磁盘/远程。这确保了即使后续 API 调用失败或应用崩溃，配置也是最新的（特别是时间间隔转换：秒 -> 毫秒）。
3.  **权限请求**:
    *   若开启后台检查，Hook 调用 `requestNotificationPermission` (阻塞) 和 `api.requestWifiSsidPermission` (可选链+try-catch，非阻塞)。
4.  **原生 API 调用**:
    *   调用 Tauri API (`startBackgroundCheck`, `stopBackgroundCheck`, `startLatencyTest`, `stopLatencyTest`)。
5.  **本地状态同步**:
    *   调用 `store.updateConfigLocal` 更新本地缓存。
    *   调用 `store.setBgStatus` 更新 AuthStore 中的运行状态。
6.  **数据回流 UI**:
    *   `useQualityStore` 中的 `networkQuality` 状态更新。
    *   `StatusBar` 和 `MonitorPanel` 等组件通过 `useShallow` 订阅状态变化，触发重渲染。
    *   `QualityPanel` 和 `NetworkQualityCapsule` 使用新的 `networkQuality` 数据重新计算延迟等级、颜色及动画。

## Connections

*   **[[auth-module]]**: `useMonitor` 依赖 `useAuthStore` 读写后台服务运行状态 (`bgStatus`)。
*   **[[config-module]]**: `useMonitor` 依赖 `useConfigStore` 读写配置（本地缓存 vs 直接持久化）。
*   **[[shared-ui-components]]**:
    *   `StatusBar` 导入 `RefreshButton` 和 `IspMark`。
    *   `LatencyComponents` 和 `NetworkQualityCapsule` 导入 `AnimatedNumber`。
    *   `SpeedTestPanel` 导入 `OnlineDeviceStack` 和 `OnlineDeviceStackH`。
*   **[[tauri-api-module]]**: `useMonitor` 直接调用 `@/hooks/tauriApi` 中的原生接口。
*   **[[network-lib]]**:
    *   `LatencyComponents` 和 `NetworkQualityCapsule` 调用 `@/lib/latency` 中的 `getLatencyLevel`, `getLatencyColor`, `extract...`。
    *   调用 `@/network/constants` 中的 `QUALITY_CONFIG`。
*   **[[hooks-collection]]**:
    *   `MonitorPanel` 和 `QualityPanel` 使用 `useAsyncLock` 处理异步锁。
    *   `QualityPanel` 使用 `useDailyMascots` 和 `useGlowAnimation`。
    *   `LatencyComponents` 使用 `usePageIdle` 和 `useAnimationProfile`。

## Known Issues

1.  **P1-F6 历史缺陷修复 (MonitorPanel/QaulityPanel)**:
    *   **问题**: 之前的实现中，输入框的值在每次按键时都被 `clamp` 并立即回写，导致用户无法输入超出范围的值（如输入 "120" 被截断或光标丢失）。
    *   **现状**: 已在 `MonitorPanel.tsx` (L58-L66) 和 `QualityPanel.tsx` (L83-L89) 中通过引入 `draft` 状态和仅在 `blur`/`Enter` 时提交来解决。
2.  **后端 Watcher 崩溃 (MonitorPanel)**:
    *   **问题**: 后端监控循环可能崩溃，仅前台通知仍在运行。
    *   **现状**: `MonitorPanel.tsx` (L133-L138) 检查 `bgStatus.watcherFailed` 并显示警告，提示用户重启后端。此标记需手动复位。
3.  **Wi-Fi SSID 权限降级 (useMonitor)**:
    *   **行为**: `requestWifiSsidPermission` (L45-L49) 失败时仅打印警告，不阻断主流程。
    *   **影响**: 后台检查服务仍可启动，但可能无法获取精确的网络名称（SSID）。这是“降级可用”的设计。
4.  **进度条溢出 (LatencyTimeline)**:
    *   **问题**: 当细分阶段过多时，最小宽度（3%）之和可能超过 100%。
    *   **现状**: `LatencyTimeline.tsx` (L85-L91) 实现了归一化逻辑，按比例缩小各段宽度以确保总和不超过 100%。
5.  **胶囊状态假死 (NetworkQualityCapsule)**:
    *   **问题**: 在慢网络下，后端可能长时间返回 'busy' 或 'unknown' 状态，导致胶囊一直显示“检测中”。
    *   **现状**: `NetworkQualityCapsule.tsx` (L78-L80) 增加了有效质量推断逻辑，若有延迟数值则反推等级，避免假死。
6.  **横滑错位 (QualityPanel)**:
    *   **问题**: 设备旋转或分屏窗口大小改变时，横滑轨道位置可能错位。
    *   **现状**: `QualityPanel.tsx` (L98-L103) 使用 `ResizeObserver` 监听视口变化并强制校正轨道位置。