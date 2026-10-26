---
title: 在线设备卡片与堆叠轮播组件规范
type: learning
source_files:
  - tauri-app/frontend/src/account/SelfOnlineCards.tsx
  - android/frontend/src/account/SelfOnlineStack.tsx
tags: [前端, React, CSS, 双端, 组件]
---

## Overview

本文档记录 `SelfOnlineCards`（桌面/Tauri）与 `SelfOnlineStack`（Android/WebView）中在线设备卡片及其堆叠轮播组件的实现细节。两个平台共享相同的视觉规格（方案 A），但在交互模式上存在差异：桌面端采用水平封面流（Coverflow）或窄屏变体（narrow），移动端竖排采用垂直封面流（`OnlineDeviceStack`），平板采用水平封面流（`OnlineDeviceStackH`）。核心设计原则是“整副牌连续滚动”，即所有槽位常挂载并通过 `transform` 过渡切换，无离场淡出动画。

## Key Components

### tauri-app/frontend/src/account/SelfOnlineCards.tsx

#### OnlineDeviceCardProps (行 8-23)
定义单张设备卡片的 Props 接口。
- `loginTime: string` (行 9): 上线时间字符串。
- `ip: string` (行 10): IP 地址。
- `mac: string` (行 11): MAC 地址。
- `flowMb: string` (行 12): 流量数据（MB），由面板格式化后传入。
- `durationMin: string` (行 13): 使用时长（分钟），由面板格式化后传入。
- `terminalType: string` (行 14): 终端类型徽章。
- `alias?: string` (行 16): 设备自定义别名（`config.deviceAliases[mac 原始 12 位 hex]`）；空 = 未命名，卡头只显示时间。
- `onRename?: () => void` (行 18): 重命名回调；缺省不渲染铅笔图标（总览卡掩码态不提供编辑入口）。
- `offlineDisabled?: boolean` (行 19): 注销按钮禁用状态。
- `offlineLoading?: boolean` (行 20): 注销加载状态。
- `onOffline?: () => void` (行 22): 注销回调；缺省不渲染注销按钮（总览卡掩码态：验证通过前不提供注销入口）。

#### OnlineDeviceCard (行 25-108)
纯展示组件，负责渲染单个设备卡片。格式化由面板完成后传入，避免与 `SelfServicePanel` 循环依赖（行 6-7 注释）。
- **布局结构**：
    - 顶部 (行 42-67)：左侧显示上线时间（带绿色在线指示点，行 44），右侧显示别名标签（行 48-52，仅 `alias` 存在时渲染）、终端类型徽章（行 53-55）及可选的重命名按钮（行 56-65，仅 `onRename` 存在时渲染）。
    - 中部 (行 69-78)：IP/MAC 垂直芯片组，IP 在上（行 70-73），MAC 在下（行 74-77），芯片与卡面互为反色（`bg-muted` / `dark:bg-card`）保持层次。
    - 底部 (行 79-105)：左侧显示流量（行 81-87）和使用时长（行 88-91），右侧显示注销按钮（行 93-104，仅 `onOffline` 存在时渲染）。
- **样式策略**：浅色主题用纯白卡面（`bg-card`，行 41），深色主题用 `muted`（`dark:bg-muted`）。字体大小严格控制在 `[10px]` 至 `[11px]` 之间，数据使用 `font-mono` 确保对齐。

#### 堆叠布局常量 (行 114-126)
- `CF_X = [60, 85] as const` (行 114): 相邻/次邻横向偏移（% 卡宽）。
- `CF_INSET = '27%'` (行 115): 卡宽 = 舞台 46%（左右各留 27%）。
- `SIDE_SCALE = 0.6` (行 116): 相邻卡缩放比例。
- `SIDE_SCALE_2 = 0.45` (行 117): 次邻卡缩放比例。
- `DRAG_THRESHOLD = 60` (行 118): 拖拽判翻页阈值。
- `DRAG_CLAMP = 80` (行 119): 拖拽行程上限。
- `SLIDE_MS = 500` (行 120): 切换动画时长。
- `EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'` (行 121): 缓动函数。
- `NARROW_MIN_W = 213` (行 126): narrow 变体最小卡宽（`max(213px, max-content)`，学号行自然宽 ~133 + 80px 留白）。

#### OnlineDeviceCardStack (行 128-315)
桌面端水平堆叠容器，支持 `coverflow` 和 `narrow` 两种变体。
- **参数** (行 128): 内联类型 `{ children: React.ReactNode; className?: string; variant?: 'coverflow' | 'narrow'; focusIndex?: number }`。
- **状态管理** (行 132-137):
    - `index` (行 132): 当前中心卡片索引。
    - `dragX` (行 133), `dragging` (行 134): 拖拽状态。
    - `cardH` (行 135): 卡片高度，通过 `ResizeObserver` 动态测量（行 152-168）。
    - `drag` ref (行 136), `suppressClick` ref (行 137): 拖拽上下文与点击抑制标记。
- **效果**:
    - 列表变短夹住页码 (行 140-142)。
    - 外部焦点 `focusIndex` 变化滚中央 (行 145-148)。
    - 量高：取所有已挂载卡最大自然高度，舞台高度固定（行 152-168）。
- **交互逻辑**:
    - `go(next)` (行 174-178): 循环切换索引。
    - `onPointerDown` (行 180-191): 单卡 (`count < 2`) 直接 return 不可移动 (行 181)；忽略按钮点击 (行 182)。
    - `onPointerMove` (行 192-197): 更新 `dragX` 并 clamp 至 `DRAG_CLAMP`。
    - `endDrag` (行 198-214): 达阈值则翻页，否则吞掉紧随的 click 防止误触。
    - `onClick` (行 215-226): 单卡不换页 (行 216)；以中线划分，左半上一张右半下一张 (行 224-225)。
- **定位算法** (行 245-296):
    - 姿势表 `pos` (行 250-259)：中央 `s===0` 全亮，相邻 `s===1`/`count-1` 缩 0.6 压暗，次邻 `s===2`/`count-2` 缩 0.45，≥6 张时更外透明槽常挂载。
    - `coverflow` 基样式 `left/right: CF_INSET` (行 266)；`narrow` 基样式 `left:50%; width:max-content; minWidth:213` (行 265)。
    - 水平偏移合并：必须使用 `translateX(calc(${narrow ? `${pageX}%` : `${pos.x}%`} + ${dragX}px))` 格式 (行 273, 280)，禁止裸写 `-50% + x%`（行 268-269 注释说明浏览器会丢弃整条）。
    - 侧卡 `pointer-events-none` (行 293)。
- **指示器** (行 298-312): 多于 1 卡时显示圆点，当前卡 `w-4 bg-primary`。

### android/frontend/src/account/SelfOnlineStack.tsx

#### OnlineDeviceStackProps (行 43-48)
安卓端堆叠容器 Props 接口。
- `cards: React.ReactNode[]` (行 44): 子卡片节点数组。
- `variant?: 'coverflow' | 'narrow'` (行 45): 变体标识（实际仅 `OnlineDeviceStackH` 使用，行 342）。
- `focusIndex?: number` (行 47): 外部焦点，变化时把该序号卡滚到中央。

#### OnlineDeviceStack (行 50-230)
安卓端垂直堆叠容器（竖排封面流）。
- **常量** (行 14-41):
    - `PEEK_TOP = 44` (行 14), `PEEK_BOTTOM = 44` (行 15): 上下露出边高度。
    - `DRAG_THRESHOLD = 60` (行 16), `DRAG_MIN = -110` (行 18), `DRAG_MAX = 70` (行 19): 垂直拖拽范围。
    - `SLIDE_MS = 500` (行 20), `EASE` (行 21): 同桌面。
    - `V_SIDE_SCALE = 0.75` (行 23): 竖排侧卡缩放（比水平 0.6 大，保证露出边可读）。
    - `V_CARD_INSET = 'inset-x-[6%]'` (行 27): 竖排卡宽 = 舞台 88%（旧值 18% 导致窄屏挤压，见行 25-26 注释）。
    - 水平复用常量 `CF_X` (行 32), `CF_INSET` (行 33), `SIDE_SCALE` (行 34), `SIDE_SCALE_2` (行 35), `DRAG_CLAMP` (行 36), `NARROW_MIN_W` (行 41)。
- **状态** (行 51-58): `index`, `dragY`, `dragging`, `cardH`, `stageRef`, `drag` ref, `suppressClick` ref。
- **效果**: 列表变短夹页码 (行 61-63)；外部焦点 (行 66-69)；量高 `ResizeObserver` (行 74-89)。
- **布局逻辑** (行 91-186):
    - 单卡返回 null (行 91)。
    - 两张卡时 `peekBottom = 0` (行 93) 避免底部空留。
    - 容器高度 = `cardH + PEEK_TOP + peekBottom` (行 162)。
    - 姿势表 `pose` 基于 `r = (i-index+count)%count` (行 94, 173-186)：中央 `r=0` 位于 `top: PEEK_TOP` 全亮；下一张 `r=1` 顶部露出 `top:0` 缩 0.75 压暗；上一张 `r=count-1` 底部露出 `top: bottomTop` 缩 0.75；更深透明常挂载。
- **交互** (行 104-150): 垂直拖拽上滑下一张 (行 132) 下滑上一张 (行 133)；点击上半上一张下半下一张 (行 148-149)；单卡不可移动 (行 105, 140)；按钮点击忽略 (行 106, 141)。
- **渲染** (行 152-212): 舞台 `w-full overflow-x-clip` (行 160) 防止错位；卡片 `absolute top-0 origin-top` + `V_CARD_INSET` (行 204)；侧卡 `pointer-events-none` (行 208)。

#### OnlineDeviceStackCardProps (行 233-248)
同桌面 `OnlineDeviceCardProps` 完全同构：
- `loginTime` (行 234), `ip` (行 235), `mac` (行 236), `flowMb` (行 237), `durationMin` (行 238), `terminalType` (行 239), `alias?` (行 241), `onRename?` (行 243), `offlineDisabled?` (行 244), `offlineLoading?` (行 245), `onOffline?` (行 247)。注释强调上线时间由面板截取 HH:mm:ss 传入 (行 232)。

#### OnlineDeviceStackCard (行 250-336)
安卓端单张设备卡片，结构与桌面 `OnlineDeviceCard` 同构。
- 差异点 (行 306-319)：左列包 `flex min-w-0 gap-4` (行 308)，流量/时长块加 `min-w-0` 与 `whitespace-nowrap`，防止长流量值（如 81449.169M）挤出右侧注销按钮（`shrink-0`，行 325）。
- 卡面内边距 `p-3` 与桌面同构 (行 267，注释 8da3b1c 曾误删导致内容贴边)。

#### OnlineDeviceStackH (行 342-519)
安卓端水平堆叠容器（平板模式），逻辑与桌面 `OnlineDeviceCardStack` 一致。
- 参数 (行 342): `{ cards, variant = 'coverflow' }: OnlineDeviceStackProps`。
- 常量复用 (行 32-41)，状态/效果/交互/定位算法同桌面（行 353-489 对应）。
- 单卡不可移动 (行 386, 421)；点击左半上一张右半下一张 (行 429-430)。
- 渲染舞台 `marginInline: count > 1 && !narrow ? 32 : 0` (行 441)，定位算法使用 `calc()` 合并 (行 478, 485)。

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
│  │  Desktop: OnlineDeviceCardStack (SelfOnlineCards.tsx)  │ │
│  │  Android: OnlineDeviceStack / OnlineDeviceStackH       │ │
│  │           (SelfOnlineStack.tsx)                        │ │
│  │  - 管理 index, drag state                               │ │
│  │  - 计算 slot pose (x/scale/z/opacity or top/scale)      │ │
│  │  - 渲染 <div> wrapper with pointer events               │ │
│  └──────────┬──────────────────────────────────────────────┘ │
│             │ passes node                                     │
│  ┌──────────▼──────────────────────────────────────────────┐ │
│  │  Card: OnlineDeviceCard (desktop) / StackCard (android) │ │
│  │  - Pure presentation                                    │ │
│  │  - Layout: Header (Time/Alias), Body (IP/MAC), Footer   │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Data Preparation**: `SelfServicePanel` 调用后端 API 获取在线设备列表，对每个设备格式化流量（`formatFlowMb`）、时长（`formatUseTimeMinutes`）和时间戳（安卓端截取 HH:mm:ss），查找 `config.deviceAliases` 获取别名。
2. **Component Mounting**:
    - **Desktop**: `SelfOnlineCards.tsx` 的 `OnlineDeviceCardStack` 接收 `children`（已格式化的 `OnlineDeviceCard` 实例，行 128）。
    - **Android**: `SelfOnlineStack.tsx` 的 `OnlineDeviceStack` 接收 `cards` 数组（行 50），内部映射生成 `OnlineDeviceStackCard`；平板用 `OnlineDeviceStackH` 同样接收 `cards`（行 342）。
3. **Interaction Handling**:
    - Pointer events 绑定在堆叠容器 stage div 上（桌面行 238-242，安卓垂直行 165-169，水平行 443-447）。
    - 拖拽距离与 `DRAG_THRESHOLD` 比较（桌面行 205，安卓垂直行 129，水平行 410）。
    - 达阈值调用 `go(index +/- 1)`，更新 `index` 触发重渲染。
    - `suppressClick` 防止拖拽松手误触（桌面行 206-212，安卓对应）。
4. **Rendering**:
    - 每张卡绝对定位，计算 `transform` 使用 `calc()` 混合单位（桌面行 273/280，安卓水平行 478/485，垂直行 190/196）。
    - `z-index` 与 `opacity` 按相对位置更新，实现整副牌连续滚动无离场淡出。

## Connections

- [[learnings/translatex-sum-requires-calc]]: 记录了 `translateX` 多值求和必须包裹 `calc()` 的历史教训，直接影响了 `OnlineDeviceCardStack`（行 268-269, 273, 280）和 `OnlineDeviceStackH`（行 473-474, 478, 485）的定位逻辑实现。
- [[learnings/browser-smoke-test-tauri-mock]]: 提供了验证此类静默 CSS 错误的探针方法。

## Known Issues

- **CSS `calc()` 拼接陷阱**：在 `OnlineDeviceCardStack`（行 268-269）与 `OnlineDeviceStackH`（行 473-474）中，水平偏移合并若裸写 `translateX(-50% + x%)` 会被浏览器整条丢弃，必须使用 `calc()` 包装（如行 273、280、478、485）。这是双端堆叠定位的核心已知坑。
- **单卡交互禁用**：当设备列表仅剩 1 张时，桌面端 `OnlineDeviceCardStack`（行 181、216）与安卓端 `OnlineDeviceStack`/`OnlineDeviceStackH`（行 105、140、386、421）均禁用拖拽与点击换页，卡片居中定格。
- **安卓竖排宽度**：`V_CARD_INSET` 曾误用 `inset-x-[18%]`（卡宽仅 64%）导致真机窄屏排版挤压，现改为 `inset-x-[6%]`（行 27）修复。
