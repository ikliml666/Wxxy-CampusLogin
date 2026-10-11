---
title: 在线设备卡片与堆叠轮播组件规范
type: learning
source_files:
  - tauri-app/frontend/src/account/SelfOnlineCards.tsx
  - android/frontend/src/account/SelfOnlineStack.tsx
tags: [前端, React, CSS, 双端, 组件]
---

## Overview

本文档记录 `SelfOnlineCards`（桌面/Tauri）与 `SelfOnlineStack`（Android/WebView）中在线设备卡片及其堆叠轮播组件的实现细节。两个平台共享相同的视觉规格（方案 A），但在交互模式上存在差异：桌面端采用水平封面流（Coverflow）或窄屏变体，移动端采用垂直封面流。核心设计原则是“整副牌连续滚动”，即所有槽位常挂载并通过 `transform` 过渡切换，无离场淡出动画。

## Key Components

### tauri-app/frontend/src/account/SelfOnlineCards.tsx

#### OnlineDeviceCardProps (行 13-30)
定义单张设备卡片的 Props 接口。
- `loginTime: string`: 上线时间字符串。
- `ip: string`: IP 地址。
- `mac: string`: MAC 地址。
- `flowMb: string`: 流量数据（MB）。
- `durationMin: string`: 使用时长（分钟）。
- `terminalType: string`: 终端类型徽章。
- `alias?: string`: 设备自定义别名。空值时不显示别名标签。
- `onRename?: () => void`: 重命名回调。缺省不渲染铅笔图标。
- `offlineDisabled?: boolean`: 注销按钮禁用状态。
- `offlineLoading?: boolean`: 注销加载状态。
- `onOffline?: () => void`: 注销回调。缺省不渲染注销按钮（用于总览卡掩码态）。

#### OnlineDeviceCard (行 32-104)
纯展示组件，负责渲染单个设备卡片。
- **布局结构**：
    - 顶部：左侧显示上线时间（带绿色在线指示点），右侧显示别名标签、终端类型徽章及可选的重命名按钮。
    - 中部：IP/MAC 垂直芯片组。IP 在上，MAC 在下，使用反色背景保持层次。
    - 底部：左侧显示流量和使用时长，右侧显示注销按钮。
- **样式策略**：浅色主题使用纯白卡面，深色主题使用 `muted` 背景。字体大小严格控制在 `[10px]` 至 `[11px]` 之间，使用 `font-mono` 确保数据对齐。

#### OnlineDeviceCardStack (行 119-276)
桌面端水平堆叠容器，支持 `coverflow` 和 `narrow` 两种变体。
- **常量定义**：
    - `CF_X = [60, 85]`: 相邻/次邻横向偏移百分比。
    - `CF_INSET = '27%'`: 卡宽占舞台 46%（左右各留 27%）。
    - `SIDE_SCALE = 0.6`, `SIDE_SCALE_2 = 0.45`: 侧卡缩放比例。
    - `DRAG_THRESHOLD = 60`, `DRAG_CLAMP = 80`: 拖拽阈值与上限。
    - `SLIDE_MS = 500`, `EASE`: 动画时长与缓动函数。
    - `NARROW_MIN_W = 213`: 窄屏模式下最小卡宽。
- **状态管理**：
    - `index`: 当前中心卡片索引。
    - `dragX`, `dragging`: 拖拽状态。
    - `cardH`: 当前卡片高度，通过 `ResizeObserver` 动态测量。
- **交互逻辑**：
    - `go(next)`: 切换到目标索引，处理循环边界。
    - `onPointerDown/Move/Up/Cancel`: 处理水平拖拽。左拖下一张，右拖上一张。
    - `onClick`: 以中线划分点击区，左半上一张，右半下一张。
    - `suppressClick`: 防止拖拽结束后误触切换。
- **定位算法**：
    - `coverflow`：使用 `left/right` 百分比设定宽度，`translateX` 配合 `calc()` 实现居中与偏移。**注意**：必须使用 `calc(${pageX}% + ${dragX}px)` 格式，禁止裸写 `-50% + x%`，否则浏览器会丢弃整个 transform 值。
    - `narrow`：使用 `left: 50%` 结合 `width: max-content` 和 `minWidth: 213px` 居中。

### android/frontend/src/account/SelfOnlineStack.tsx

#### OnlineDeviceStackProps (行 72-75)
安卓端堆叠容器 Props。
- `cards: React.ReactNode[]`: 子卡片节点数组。
- `variant?: 'coverflow' | 'narrow'`: 变体标识。

#### OnlineDeviceStack (行 77-225)
安卓端垂直堆叠容器（竖排封面流）。
- **常量定义**：
    - `PEEK_TOP = 44`, `PEEK_BOTTOM = 44`: 上下露出边高度。
    - `DRAG_THRESHOLD = 60`: 拖拽阈值。
    - `DRAG_MIN = -110`, `DRAG_MAX = 70`: 垂直拖拽范围。
    - `V_SIDE_SCALE = 0.75`: 垂直侧卡缩放比例（比水平的 0.6 大，保证露出部分可读性）。
    - 其他常量同桌面端。
- **布局逻辑**：
    - 容器高度 = `cardH + PEEK_TOP + peekBottom`（两张卡时 `peekBottom` 为 0）。
    - 槽位姿势表基于相对距离 `r = (i - index + count) % count` 计算。
    - 中央卡 (`r=0`) 位于 `top: PEEK_TOP`，全亮。
    - 下一张 (`r=1`) 位于 `top: 0`，缩小压暗。
    - 上一张 (`r=count-1`) 位于 `bottomTop`，缩小压暗。
    - 更深的卡透明隐藏。
- **交互逻辑**：
    - 垂直拖拽：上滑下一张，下滑上一张。
    - 点击：上半区上一张，下半区下一张。

#### OnlineDeviceStackCard (行 233-316)
安卓端单张设备卡片组件。
- 结构与 `tauri-app` 的 `OnlineDeviceCard` 完全同构。
- 注释指出上线时间由面板截取为 HH:mm:ss 后传入。

#### OnlineDeviceStackH (行 324-473)
安卓端水平堆叠容器（平板模式）。
- 逻辑与 `tauri-app` 的 `OnlineDeviceCardStack` 完全一致。
- 复用相同的常量（`CF_X`, `SIDE_SCALE` 等）和定位算法。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     App Shell / WebView                      │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              SelfServicePanel / AccountPage               │ │
│  │  (负责数据获取、格式化 flowMb/durationMin 等)             │ │
│  └───────────────────────┬─────────────────────────────────┘ │
│                          │ provides formatted data           │
│  ┌──────────────────────▼─────────────────────────────────┐ │
│  │           Stack Container (OnlineDeviceCardStack)       │ │
│  │  (tauri-app) / (OnlineDeviceStack / H)                  │ │
│  │  - 管理 index, drag state                               │ │
│  │  - 计算 slot pose (x/scale/z/opacity)                   │ │
│  │  - 渲染 <div> wrapper with pointer events               │ │
│  └──────────┬──────────────────────────────────────────────┘ │
│             │ passes node                                     │
│  ┌──────────▼──────────────────────────────────────────────┐ │
│  │         Card Component (OnlineDeviceCard / StackCard)   │ │
│  │  (tauri-app) / (android)                                │ │
│  │  - Pure presentation                                    │ │
│  │  - Layout: Header (Time/Alias), Body (IP/MAC), Footer   │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Data Preparation**: `SelfServicePanel` 调用后端 API 获取在线设备列表。对每个设备，格式化流量（`formatFlowMb`）、时长（`formatUseTimeMinutes`）和时间戳。查找 `config.deviceAliases` 获取别名。
2.  **Component Mounting**:
    *   **Desktop**: `SelfOnlineCards.tsx` 中的 `OnlineDeviceCardStack` 接收 `children`（已格式化的 `OnlineDeviceCard` 实例）。
    *   **Android**: `SelfOnlineStack.tsx` 中的 `OnlineDeviceStack` 接收 `cards` 数组，内部映射生成 `OnlineDeviceStackCard`。
3.  **Interaction Handling**:
    *   Pointer events (`onPointerDown`, etc.) are attached to the stack container.
    *   Drag distance is compared against `DRAG_THRESHOLD`.
    *   If threshold met, `go(index +/- 1)` is called.
    *   `useEffect` updates `index`, triggering re-render of styles.
4.  **Rendering**:
    *   Each card is absolutely positioned.
    *   `transform` values are computed using `calc()` for mixed units (e.g., `%` and `px`).
    *   `z-index` and `opacity` are updated based on the card's relative position in the stack.

## Connections

- [[learnings/translatex-sum-requires-calc]]: 记录了 `translateX` 多值求和必须包裹 `calc()` 的历史教训，直接影响了 `OnlineDeviceCardStack` 和 `OnlineDeviceStackH` 的定位逻辑实现。
- [[learnings/browser-smoke-test-tauri-mock]]: 提供了验证此类静默 CSS 错误的探针方法。