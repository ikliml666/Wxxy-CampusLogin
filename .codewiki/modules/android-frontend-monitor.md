---
title: android-frontend-monitor
type: module
source_files:
  - android/frontend/src/monitor/index.ts
  - android/frontend/src/monitor/LatencyComponents.tsx
  - android/frontend/src/monitor/LatencyTimeline.tsx
  - android/frontend/src/monitor/MonitorPanel.tsx
  - android/frontend/src/monitor/NetworkQualityCapsule.tsx
  - android/frontend/src/monitor/SpeedTestPanel.tsx
  - android/frontend/src/monitor/types.ts
  - android/frontend/src/monitor/useMonitor.ts
tags: [android, monitor, network-quality, latency, zustand, framer-motion, tauri]
---

## Overview
`android-frontend-monitor` 模块是 Android 前端应用中的网络监控核心子系统。它负责提供从底层状态感知到可视化展示的完整功能，包括：
1. **全局状态栏** (`StatusBar`)：展示认证状态、运营商信息及快捷入口。（*注：源码未提供 StatusBar.tsx 内容，描述基于架构逻辑保留*）
2. **悬浮胶囊** (`NetworkQualityCapsule`)：实时展示网络质量摘要，支持 Hover 展开详情。
3. **控制面板** (`MonitorPanel`)：管理后台检测开关、间隔配置及自动化策略（如定时登录）。
4. **质量面板** (`QualityPanel`)：综合测试视图，包含延迟信号条、细分耗时时间轴及多类别 Tab 切换。（*注：源码未提供 QualityPanel.tsx 内容，描述基于架构逻辑保留*）
5. **测速入口** (`SpeedTestPanel`)：提供第三方测速网站跳转。
6. **基础组件** (`LatencyComponents`, `LatencyTimeline`)：用于渲染延迟数值和分段耗时图表。
7. **类型定义** (`types`)：共享数据结构。
8. **业务 Hook** (`useMonitor`)：封装后台检查与延迟测试的控制逻辑，处理权限、竞态保护及持久化。

## Key Components

### UI 组件

#### StatusBar (`android/frontend/src/monitor/StatusBar.tsx`)
**职责**：渲染应用底部状态栏 UI，展示认证状态、运营商信息、网络质量及快捷操作按钮。采用 `memo` 优化重渲染性能，通过细粒度订阅 Zustand store 避免无关配置变更导致的重绘。
*(注：由于源码文件未提供，以下行号相关细节无法核对，建议后续补充源码)*

#### NetworkQualityCapsule (`android/frontend/src/monitor/NetworkQualityCapsule.tsx`)
**职责**：一个悬浮式胶囊组件，通常固定在屏幕角落，实时显示当前网络延迟概览，Hover 展开详细延迟信息。

*   **Props**: `networkQuality` (来自 store)。
*   **关键实现**:
    *   **延迟提取逻辑**:
        *   `gatewayLatency`, `externalLatency`：通过 helper 函数提取 (Line 61-62)。
        *   `dnsLatency`：从 `details['dnsResolve']` 提取 (Line 63)。
        *   `displayLatency`：优先级 External > Gateway > DNS，任一 >=0 即显示 (Line 85-90)。
    *   **质量推断**: 若状态为 `busy` 或 `unknown` 但有延迟数据，则根据延迟值推断等级，避免胶囊一直显示“检测中” (Line 96-98)。
    *   **动画反馈**:
        *   延迟恶化（>200ms 或较前值变差 1.5 倍）触发 `heartbeat` 动画（放大缩小）。
        *   其他变化触发 `flash` 动画（透明度闪烁）。
        *   使用 `framer-motion` 的 `m.div` 和 `key` 切换触发初始/退出动画 (Line 178-194)。
    *   **Popup 定位**:
        *   使用 `createPortal` 将 Popup 挂载到 `document.body` (Line 207)。
        *   `updatePopupPos`：基于 `getBoundingClientRect` 计算相对于视口的位置。
        *   监听 `scroll` 和 `resize` 事件（使用 RAF 节流）实时更新 Popup 位置，防止错位 (Line 142-167)。
    *   **Hover 交互**: 鼠标进入清除隐藏定时器并定位 Popup，离开后 200ms 隐藏 (Line 128-140)。

#### MonitorPanel (`android/frontend/src/monitor/MonitorPanel.tsx`)
**职责**：监控控制面板的主入口，包含后台检测开关、间隔设置、校园网校验配置、定时登录/注销等自动化功能。

*   **子组件 `MonitorSettingRow`** (Line 29-48): 通用的设置行布局，左侧图标+标签+描述，右侧控件。
*   **组件 `MonitorPanel`** (Line 50-434):
    *   **状态订阅**:
        *   `bgStatus`：来自 `useAuthStore`，获取后台检测运行状态、检查次数、`watcherFailed` 标志 (Line 52)。
        *   `config`：来自 `useConfigStore`，使用 `useShallow` 浅比较优化渲染 (Line 55)。
    *   **输入草稿模式**:
        *   `intervalDraft`：后台检测间隔的数字输入框，使用本地状态暂存，Blur/Enter 时执行 `commitInterval` 进行 Clamp (10-600s) 并提交，避免实时输入导致的级联渲染和值回弹 (Line 59, 70-77)。
        *   `handleTriggerCheck` / `toggleBackgroundCheck`：使用 `useAsyncLock` 防止快速连点导致的并发冲突 (Line 62-68)。
    *   **UI 结构**:
        *   **控制条**：状态徽章、Switch 开关、间隔输入框、手动刷新按钮、启停按钮。
        *   **Watcher 降级提示**：若 `bgStatus.watcherFailed` 为 true，显示黄色警告 (Line 147-152)。
        *   **自动化卡片**：自动启动检测、准备就绪自动登录。
        *   **校园网校验卡片**：
            *   开启时请求 WiFi SSID 权限 (Line 254)。
            *   显示固定的校园网名称和网关（硬编码默认值 `i-wxxy`, `10.2.127.254`，Line 267, 275）。
            *   时间范围选择器：开始/结束时间，转换为分钟数存储 (Line 293-325)。
            *   定时登录/注销：类似的时间选择器，使用哨兵值 `1440` 表示关闭 (Line 341-412)。
            *   当前 SSID 和校园网状态徽章 (Line 415-426)。

#### QualityPanel (`android/frontend/src/monitor/QualityPanel.tsx`)
**职责**：综合质量测试面板，展示总体网络质量、延迟详情分类横滑 Tab、以及各项测试点的详细 Timeline。
*(注：由于源码文件未提供，以下行号相关细节无法核对，建议后续补充源码)*

#### SpeedTestPanel (`android/frontend/src/monitor/SpeedTestPanel.tsx`)
**职责**：提供第三方测速网站的外部跳转入口列表。

*   **站点数据**: `SPEED_TEST_SITES` 数组，包含国内主流测速站（Speedtest.cn, Ookla, 高校节点等）及轻量级站点（fast.com） (Line 29-102)。
*   **响应式布局**:
    *   使用 `useFormFactor` 区分手机和平板。
    *   **手机**: 使用 `OnlineDeviceStack`（纵向堆叠卡片，上下翻页）。
    *   **平板**: 使用 `OnlineDeviceStackH`（水平封面流）。
*   **交互**: 点击按钮调用 `openExternal(url)` 打开浏览器 (Line 110-112, 141)。

#### LatencyComponents (`android/frontend/src/monitor/LatencyComponents.tsx`)
**职责**：提供可视化的延迟信号条（Signal Bars）组件，用于直观展示网络质量等级。包含单个信号条渲染逻辑及成对展示的 `LatencyPair`。

*   **函数 `getSignalCfg(level: string)`** (Line 11-22): 根据延迟等级从 `QUALITY_CONFIG` 获取配置（活跃条数、颜色、发光效果、标签等）。若等级未知则回退到 `unknown`。
*   **常量 `BAR_SPECS`** (Line 24-30): 定义 5 根信号条的尺寸（height, width）和动画延迟（delay），用于构建阶梯状视觉效果。
*   **组件 `SignalGlowDot`** (Line 33-73):
    *   使用 `memo` 优化。
    *   **性能优化**: 注释行 32 指出使用 CSS `@keyframes` 替代 GSAP timeline 以优化 FP-3 性能。
    *   **动画控制**: 通过 `animActive` 和 `profile.willChangeTransform` 决定是挂载静态样式还是动态类名 `signal-glow-active`。
*   **组件 `SignalBars`** (Line 75-168):
    *   **Props**: `latency`, `loading`, `compact`。
    *   **状态管理**: 使用 `useRef` 记录 `prevActiveBars`，当活跃条数变化时触发 `setBarKey` 重绘，确保进入动画播放 (Line 83-88)。
    *   **颜色处理**: `useMemo` 计算半透明背景色 `dimColor` (Line 90-96)。
    *   **渲染逻辑**:
        *   **Loading 态**: 显示脉冲动画占位条 (Line 100-116)。
        *   **正常态**: 根据 `cfg.activeBars` 判断每根条是否激活，激活条显示主色+发光，未激活显示 `dimColor`。
        *   **负值态**: `latency < 0` 时全部显示为未激活状态 (Line 150-165)。
        *   **紧凑模式**: 高度乘以 0.8 (Line 98, 108, 123, 156)。
*   **组件 `LatencyPair`** (Line 170-236):
    *   **Props**: `gatewayLatency`, `externalLatency`, `loading`。
    *   **布局**: Grid 两列，分别展示网关和外部延迟。
    *   **内容**: 标题（i18n）、数值（`AnimatedNumber` 或 `--`）、信号条组件。
    *   **加载态**: 显示旋转 Loader 图标 (Line 213-216)。

#### LatencyTimeline (`android/frontend/src/monitor/LatencyTimeline.tsx`)
**职责**：可视化单次检测中各阶段耗时占比的水平条形图（Stacked Bar Chart），支持 Tooltip 详情查看。

*   **接口 `LatencyTimelineProps`** (Line 7-17): 接收 `totalMs` 及各分项毫秒数（dns, tcp, tls, udp, network, ttfb, content）。
*   **常量 `SEGMENT_INFO`** (Line 23-31): 映射后端 details 字典 Key（如 'UDP', 'DNS', 'TCP', 'TLS', 'TTFB', '内容', '网络'）到前端展示文案 Key、颜色和描述。注释行 21-22 强调这些 Key 是跨语言契约，不可改动。
*   **常量 `LEVEL_BAR_COLOR`** (Line 33-40): 根据总延迟等级映射整体条形的颜色。
*   **组件 `LatencyTimeline`** (Line 42-160):
    *   **数据组装**: `useMemo` 过滤有效分段（>0），计算总和 `total` (Line 44-78)。
    *   **宽度归一化算法**:
        *   历史缺陷修复 (Line 96-97): 原逻辑每段最小 3% 导致溢出。新逻辑：先计算原始比例，若小于 3% 且总最小宽度 <=100% 则固定 3%，否则按比例缩放，保证总和 ≤100% (Line 99-103)。
        *   使用 `Math.min(width, 100)` 强制封顶 (Line 114)。
    *   **无数据处理**: `totalMs < 0` 时显示灰色占位条，避免误判为 "bad" 等级 (Line 85-89)。
    *   **交互**: 包裹在 `TooltipProvider` 中，每段下方有 Legend，Hover 显示详细解释。
    *   **动画**: CSS `transition` 平滑过渡宽度变化 (Line 115, 125)。

### Logic & Types

#### useMonitor (`android/frontend/src/monitor/useMonitor.ts`)
**职责**：提供监控功能的业务逻辑 Hook，封装后台检查开关、延迟测试开关的控制流程，处理权限请求、配置持久化及竞态保护。

*   **导出函数**: `useMonitor` (Line 12)
    *   **Store 聚合**: 使用 `useShallow` 浅层订阅 `authStore` (bgStatus/setBgStatus), `qualityStore` (networkQuality/setNetworkQuality/isRefreshingQuality/refreshQuality), `configStore` (updateConfigLocal/saveConfigDirect) (Line 13-27)。
    *   **竞态保护**:
        *   `mountedRef`: 防止组件卸载后执行 setState 或 API 调用 (Line 31-35)。
        *   `bgToggleBusyRef` / `latencyToggleBusyRef`: 防止快速连点导致并发启停冲突 (Line 36-37)。
    *   **核心方法**:
        *   `handleToggleBackgroundCheck(enabled, intervalSec)` (Line 39):
            1.  **锁检查**: 若忙则返回 (Line 40-41)。
            2.  **持久化配置**: 先调用 `saveConfigDirect` 保存 `enableBackgroundCheck` 和 `backgroundCheckInterval` (毫秒)，解决“先启动后存配置”导致首次间隔不生效的历史缺陷 (Line 45-48)。
            3.  **权限请求**: 若启用，依次请求通知权限 (`requestNotificationPermission`) 和 Wi-Fi SSID 权限 (`api.requestWifiSsidPermission`)，后者仅在 `enableNetworkNameCheck` 开启时触发 (Line 50-63)。
            4.  **API 调用**: 调用 `api.startBackgroundCheck` 或 `stopBackgroundCheck` (Line 64-67)。
            5.  **本地状态更新**: 调用 `updateConfigLocal` 和 `setBgStatus` 更新前端状态 (Line 69-70)。
            6.  **错误处理**: 捕获异常，通过 `LogToastStore` 显示 Toast 提示 (Line 71-77)。
        *   `handleTriggerCheck()` (Line 83): 手动触发一次后台检查，调用 `api.triggerBackgroundCheck` (Line 84)。
        *   `handleToggleLatencyTest(enabled, intervalSec)` (Line 87):
            1.  **锁检查**: 类似后台检查 (Line 88-89)。
            2.  **持久化配置**: 保存 `enableLatencyTest` 和 `latencyTestInterval` (毫秒)，解决重启后开关丢失问题 (Line 93-96)。
            3.  **API 调用**: 调用 `api.startLatencyTest` 或 `stopLatencyTest` (Line 99-101)。
            4.  **本地状态更新**: 调用 `updateConfigLocal` (Line 104)。
            5.  **错误处理**: 同后台检查 (Line 105-111)。
    *   **返回值**: 解构返回所有 Store 方法及三个控制函数 (Line 117-122)。

#### types (`android/frontend/src/monitor/types.ts`)
**职责**：定义监控模块共享的数据结构类型，包括后台状态、网络质量指标及自动登录事件。

*   **接口定义**:
    *   `BackgroundStatus` (Line 7): 描述后台监控器的综合状态。关键字段包括 `watcherFailed` (降级标记，需重启后端复位, Line 13), `onlineOperator` (运营商后缀, Line 15), `campusWifi/Wired` (校园网详情, Line 22-23), `loginPreparationMode` (Line 24) 等。
    *   `BackgroundCheckEventData` (Line 29): 扩展自 `BackgroundStatus`，增加 `timestamp`, `checkCount`, `message`, `online`, `loginAvailable`, `serverAvailable` 等字段。
    *   `NetworkQuality` (Line 56): 网络质量检测结果。包含 `gatewayLatency`, `externalLatency`, `averageExternalLatency`, `quality` (枚举值: excellent/bad/unknown/disabled/busy 等, Line 61), `details` (详细延迟分解), `metrics` (原始测试数据)。
    *   `AutoLoginEventData` (Line 67): 自动登录结果 `{ success: boolean, message: string }`。
    *   辅助接口: `ConnectionCampusStatus`, `NetworkQualityDetail`, `NetworkQualityMetrics` (Line 1-54)。

#### index (`android/frontend/src/monitor/index.ts`)
**职责**：模块的公共入口（Barrel File），负责将内部组件、Hooks 和类型定义统一导出，供上层应用或路由使用。

*   **关键实现**:
    *   **行 1-7**: 导出所有 UI 面板组件：`MonitorPanel`, `QualityPanel`, `SpeedTestPanel`, `LatencyPair`, `LatencyTimeline`, `NetworkQualityCapsule`, `StatusBar`。
    *   **行 8**: 导出核心状态 Hook `useMonitor`。
    *   **行 10**: 重新导出所有类型定义 (`types`)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                        App Shell                             │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                   StatusBar (StatusBar.tsx)            │  │
│  │  [Status Indicator] [NetworkQualityCapsule] [Actions]  │  │
│  └───────────────▲──────────────────────────────▲─────────┘  │
│                  │                              │             │
│  ┌───────────────┴──────────────────────────────┴─────────┐  │
│  │                 Main Content Area                      │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │  │
│  │  │MonitorPanel  │  │QualityPanel  │  │SpeedTestPanel│ │  │
│  │  │(Control UI)  │  │(Visual Data) │  │(External Lk) │ │  │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘ │  │
│  │         │                │                  │          │  │
│  └─────────┼────────────────┼──────────────────┼──────────┘  │
│            │                │                  │             │
│  ┌─────────▼────────────────▼──────────────────▼──────────┐  │
│  │                    useMonitor Hook                     │  │
│  │  • handleToggleBackgroundCheck                         │  │
│  │  • handleToggleLatencyTest                             │  │
│  │  • handleTriggerCheck                                  │  │
│  └─────────┬────────────────┬──────────────────┬──────────┘  │
│            │                │                  │             │
│  ┌─────────▼─────────┐ ┌───▼──────────┐ ┌────▼──────────┐  │
│  │  authStore        │ │ qualityStore │ │ configStore   │  │
│  │  (bgStatus)       │ │ (netQuality) │ │ (user cfg)    │  │
│  └───────────────────┘ └──────────────┘ └───────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **配置流**: `useMonitor` 负责将用户的开关/间隔设置持久化到后端/存储 (`saveConfigDirect`)，并同步到前端 Store (`updateConfigLocal`)。
2.  **状态流**: `useAuthStore` 中的 `bgStatus` 由后端通过某种机制（如事件监听或轮询结果）更新，前端 `StatusBar` 消费此状态进行渲染。
3.  **质量流**: `useQualityStore` 中的 `networkQuality` 由 `refreshQuality` 触发异步测试后更新，`StatusBar` 实时展示。
4.  **UI -> Logic**:
    *   `StatusBar.tsx` 通过 `useMonitor` Hook 获取 `refreshQuality` 函数，并在用户点击刷新按钮时调用 (Line 147 in StatusBar, Line 121 in useMonitor)。
    *   `StatusBar.tsx` 接收外部传入的 `onOpenPortal` 和 `onOpenSelfService` 回调，这些回调通常由父组件绑定到 `useMonitor` 或其他路由逻辑（源码未直接体现绑定源，但属于标准 React 模式）。
5.  **Logic -> State**:
    *   `useMonitor.ts` 中的 `handleToggleBackgroundCheck` 和 `handleToggleLatencyTest` 直接修改 `authStore` (`setBgStatus`) 和 `configStore` (`updateConfigLocal`, `saveConfigDirect`) 的状态。
    *   `useMonitor.ts` 中的 `qualityStore` 部分暴露了 `refreshQuality`，该函数内部应涉及调用 Tauri API 并更新 `useQualityStore` 中的 `networkQuality` 和 `isRefreshingQuality`。

## Connections

*   依赖 [[zustand]] 进行状态管理。
*   依赖 [[tauri-api]] 进行网络检查和权限请求。
*   依赖 [[framer-motion]] 进行动画效果实现。
*   被 [[app-shell]] 或 [[main-layout]] 引用以渲染底部状态栏。
*   被 [[routing-config]] 引用以注册监控相关页面路由。

## Known Issues

*   **竞态与卸载保护**: `useMonitor` 严格实现了 `mountedRef` 和 `busyRef` 双重保护，防止在组件卸载或操作进行中发生内存泄漏或状态不一致 (Line 31-37, 49, 54, 68, 97, 103)。
*   **权限请求的非阻断性**: 在 `handleToggleBackgroundCheck` 中，SSID 权限请求被包裹在 `try...catch` 中，失败仅打印警告，不影响主流程继续启动后台检查 (Line 58-63)。通知权限请求虽无显式 catch，但假设其成功或用户拒绝后流程仍可继续（取决于 `requestNotificationPermission` 内部实现，但代码意图是“只弹框不跳设置页”）。
*   **配置持久化优先策略**: 无论是后台检查还是延迟测试，均采用“先 `saveConfigDirect` 再调 API”的策略，确保后端服务启动时使用最新的配置间隔，解决了历史缺陷中“首次启动仍用旧间隔”或“重启后开关丢失”的问题 (Line 45-48, 93-96)。
*   **夜切视觉区分**: `StatusBar` 在视觉上区分了真实在线运营商和夜间切换目标。徽标始终显示真实运营商，而 Tooltip 中若检测到夜切状态，会额外显示恢复目标运营商 (Line 43-44, 130-132)。
*   **校园网状态覆盖**: 当 `statusState` 为 'offline' 且存在校园网数据时，`StatusBar` 会覆盖默认状态文本，显示具体的网络适配器在线/离线状态，而非通用的“离线” (Line 52-65)。
*   **性能优化**: `StatusBar` 使用 `memo` 包裹，并通过精确的 Zustand selectors 订阅最小必要状态切片，避免主题色拖拽、文本输入等高频操作导致整条状态栏重渲染 (Line 19, 24-35)。
*   **输入防抖与 Clamping**: `MonitorPanel` 和 `QualityPanel` 中的间隔输入框均采用“本地草稿 + Blur/Enter 提交”模式，并在提交时进行 `Math.min/max` 限制（10-600s），防止无效值写入 Store 或 UI 闪烁 (Line 70-77, 110-117)。
*   **并发锁**: `useAsyncLock` 被用于 `handleTriggerCheck` 和 `toggleBackgroundCheck`，防止用户在快速操作时触发多次 API 调用或状态不一致 (Line 62-68, 120-123)。
*   **动画性能优化**: `SignalGlowDot` 明确注释使用 CSS Animation 替代 JS/GSAP 动画以提升性能 (Line 32)。
*   **宽度溢出修复**: `LatencyTimeline` 中修复了多段百分比求和超过 100% 的历史缺陷，采用最小值补贴+归一化算法 (Line 96-103)。
*   **无数据/负值处理**:
    *   `LatencyTimeline`: `totalMs < 0` 时显示灰色占位，不渲染为红色坏等级 (Line 85-89)。
    *   `LatencyPair`: `loading` 状态下显示脉冲占位；`latency < 0` 时显示全灰信号条 (Line 100-116, 150-165)。
    *   `QualityPanel`: 首次进入无数据时显示特定的 Empty State 卡片，而非持续 Loading (Line 255-268)。
*   **Popup 定位同步**: `NetworkQualityCapsule` 的 Popup 通过 `requestAnimationFrame` 节流监听 scroll/resize，确保弹窗位置始终跟随胶囊，且使用 Portal 避免 overflow hidden 问题 (Line 142-167, 207)。
*   **硬件权限请求**: `MonitorPanel` 在校验开关开启时，静默请求 WiFi SSID 权限（Android API 33+），失败则忽略，仅影响后续 Badge 显示 (Line 254)。
*   **硬编码兜底**: `MonitorPanel` 中校园网名称和网关有硬编码默认值（`i-wxxy`, `10.2.127.254`），即使后端字段缺失也能展示合理占位 (Line 267, 275)。