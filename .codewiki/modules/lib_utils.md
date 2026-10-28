---
title: lib_utils
type: module
source_files:
  - tauri-app/frontend/src/lib/animations.ts
  - tauri-app/frontend/src/lib/color.ts
  - tauri-app/frontend/src/lib/color.test.ts
  - tauri-app/frontend/src/lib/easing-config.ts
  - tauri-app/frontend/src/lib/latency.ts
  - tauri-app/frontend/src/lib/latency.test.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/utils.ts
  - tauri-app/frontend/src/lib/utils.test.ts
  - tauri-app/frontend/src/lib/glass-dock.tsx
  - tauri-app/frontend/src/network/constants.ts
  - tauri-app/frontend/src/monitor/types.ts
tags: [utils, animation, latency, color, easing, spring, liveness, utility-libraries]
---

## Overview

`lib_utils` 是 Tauri 前端（`tauri-app/frontend/src/lib/`）下的一组**无业务状态、纯工具**前端库，集中存放动效、颜色、延迟解析、渲染存活探测与通用助手等可复用函数。它不持有任何领域模型，只服务于 `monitor`、`network`、`settings` 等上层模块。

本模块共两个层面：

1. **直接属于本模块的纯工具**（本文档覆盖）：
   - `animations.ts` — 两条 Framer Motion 变体工厂（`createLogEntryVariants`、`createPanelAppleVariants`），把动效观感收敛成统一入口。
   - `easing-config.ts` — 零依赖的缓动曲线配置（`EasingConfig` 接口 + 60Hz/120Hz 两套预设 + 取值函数），规避循环依赖。
   - `color.ts` — `hexToHsl()` 十六进制色转 HSL。
   - `latency.ts` — 网络质量延迟的解析、分级、配色与增量合并（`getLatencyLevel`、`getLatencyColor`、`mergeNetworkQuality`、`extractGatewayLatency`、`extractExternalLatency`、`resolveQualityDisplay`）。
   - `renderLiveness.ts` — `isRenderLoopAlive()` 渲染链存活探测，用于发现 GPU/合成器崩溃但 JS 仍存活的假死场景。
   - `utils.ts` — `cn()`（clsx + tailwind-merge）、`extractErrorMessage()`、带内存降级的 `safeStorage`。

2. **悬浮栏镜头系统与折射玻璃子模块**（独立成篇，见 [[lib-utils-dock]]）：
   - `dock-spring.ts`、`dock-lens.ts`、`glass-dock.tsx` 及其测试。它把 iOS/FlClash 的 tab bar 镜头语言与弯折玻璃搬进 DOM 悬浮栏，是最大的内聚特性，约 950 行，因此派生成子模块单独记录。

## Key Components

### animations.ts — Framer Motion 变体工厂

文件 `tauri-app/frontend/src/lib/animations.ts`（共 37 行），对外导出两个返回 Framer Motion `variants` 对象的工厂函数，入参为 `EasingConfig`（来自 `./easing-config`）。

- `createLogEntryVariants(easing: EasingConfig)` — 第 3–17 行。`LogPanel` 日志条入场/退场的变体：`initial { opacity: 0, x: 20 }`；`animate` 用 0.3s、`easing.snappy`（转成 `[number,number,number,number]` 四元组）；`exit` 向左滑出 `x: -16`、0.15s、`easing.exit`。
- `createPanelAppleVariants(easing: EasingConfig)` — 第 19–36 行。面板转场变体：`initial { y: 8, opacity: 0.9 }`；`animate` 用 spring（`stiffness: 320, damping: 24, mass: 0.7`，注释注明 dampingRatio≈0.80，对应 Apple HIG 0.7–0.9）；`exit` 轻微下沉 `y: -4` 并 `scale: 0.99`，0.04s 过渡用 `easing.exit`。

### easing-config.ts — 零依赖缓动配置

文件 `tauri-app/frontend/src/lib/easing-config.ts`（共 30 行）。文件头注释（第 1 行）明确它是**独立缓动配置模块、零依赖**，所以不存在循环依赖风险。

- `EasingConfig` 接口 — 第 3–9 行。五个四元组缓动 `[number,number,number,number]` 字段：`enter`（面板转场 expo-out）、`exit`（干脆 ease-in）、`smooth`（卡片/列表 ease-out）、`snappy`（日志/徽章 micro）、`overshoot`（弹性效果）。
- `EASING_60HZ` 常量 — 第 11–17 行。60Hz 下的五条曲线预设（`enter: [0.16,1,0.3,1]` 等）。
- `EASING_120HZ` 常量 — 第 19–25 行。120Hz 下的五条曲线预设（更短、更陡）。
- `getEasingConfig(refreshRate: number)` — 第 27–29 行。按刷新率返回配置：`refreshRate >= 120` 取 `EASING_120HZ`，否则取 `EASING_60HZ`。

### color.ts — 颜色转换

文件 `tauri-app/frontend/src/lib/color.ts`（共 34 行）。

- `hexToHsl(hex: string): { h: number; s: number; l: number }` — 第 1–33 行。把 `#rrggbb`（可选前导 `#`）转成 HSL（h/s/l 均为 0–360/0–100/0–100 整数）。
  - 入参非字符串或长度 `< 4`（如 `#fff` 短写）直接返回默认 `{ h: 230, s: 70, l: 55 }`（第 2 行）。
  - 正则 `/^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i` 匹配失败或解析出 NaN 也回退到同一默认色（第 3–4、10 行）。
  - 标准 sRGB→HSL 算法，h/s/l 分别 `Math.round(h*360)`、`Math.round(s*100)`、`Math.round(l*100)`（第 28–32 行）。
  - 测试 `color.test.ts` 确认：纯红/绿/蓝/白/黑各值，非法串、`''`、`#fff`、无 `#` 前缀均按预期返回（第 5–39 行）。

### latency.ts — 网络质量延迟解析

文件 `tauri-app/frontend/src/lib/latency.ts`（共 77 行）。依赖 `NetworkQuality`（`@/monitor`，见 `src/monitor/types.ts` 第 83–92 行）与 `QUALITY_CONFIG`（`@/network/constants`，见 `src/network/constants.ts` 第 1–11 行）。

- 类型别名 `LatencyLevel` — 第 4 行：`'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad'`。
- 类型别名 `LatencyType` — 第 5 行：`'gateway' | 'external'`（目前仅用于类型，本文件内未被函数消费）。
- `getLatencyLevel(latency: number): LatencyLevel` — 第 7–15 行。按阈值分级：`< 0`→`bad`；`<= 20`→`excellent`；`<= 50`→`great`；`<= 100`→`good`；`<= 200`→`fair`；`<= 400`→`poor`；其余→`bad`。`latency.test.ts` 第 23–53 行逐项覆盖边界。
- `getLatencyColor(latency: number)` — 第 17–23 行。返回 `{ text, bg, borderBg }` 三个 Tailwind 类。`latency < 0` 直接返回 rose 红（`text-rose-500` / `bg-rose-500/8` / `bg-rose-500/10`，第 18 行）；否则用 `getLatencyLevel` + `QUALITY_CONFIG[level]`，缺失则回退 `text-muted-foreground` / `bg-muted`；每项取 `cfg.color/cfg.bg/cfg.borderBg` 并各自带 `?? 'text-muted-foreground'/'bg-muted'` 兜底（第 19–22 行）。
- `mergeNetworkQuality(old: NetworkQuality | null, incoming: NetworkQuality): NetworkQuality` — 第 25–36 行。增量合并质量数据：
  - 入参 `incoming.quality === 'disabled'` 时原样返回 `old ?? incoming`（第 26 行）；
  - `incoming.quality === 'busy'`（后端每批 HTTPS 增量推送）时**只合并 `details` 与 `metrics`、不采纳等级字段**，且 `old` 缺省时降级为 `unknown`（第 29–33 行；注释解释：慢网下终态迟迟不到，采纳 busy 会让胶囊永远显示「检测中」）；
  - `old` 为空或 `old.quality === 'unknown'` 时直接采用 `incoming`（第 34 行）；
  - 其余情况采用 `incoming` 并浅合并 `details`（保留旧值）与 `metrics`（以 `incoming.metrics ?? old.metrics` 优先，第 35 行）。
  - 测试 `latency.test.ts` 第 81–121 行覆盖上述分支，确认 `details` 合并与 `metrics` 优先序。
- `extractGatewayLatency(nq: NetworkQuality | null): number` — 第 38–43 行。`nq` 为空返回 `-1`；优先用 `nq.gatewayLatency`（≥0 即返回）；否则回退到 `nq.details['gateway'].latency`（≥0 时返回），都没有返回 `-1`。测试第 123–145 行覆盖。
- `extractExternalLatency(nq: NetworkQuality | null): number` — 第 45–59 行。`nq` 为空返回 `-1`；取值优先级：`averageExternalLatency`（≥0）→`externalLatency`（≥0）→`details` 中**排除 `gateway`** 的所有外网明细的**中位数**（升序后取 `floor(len/2)`，第 54–56 行）→ 都没有则 `-1`。测试第 147–181 行验证中位数逻辑（如 `[10,30,50]` 取 index 1 = 30）。
- `resolveQualityDisplay(nq: NetworkQuality | null)` — 第 66–76 行。卡片/页面统一展示解析（首页卡与质量页共用；胶囊另有 DNS 延迟源故不用此函数，见第 61–65 行注释）。返回 `{ quality, gatewayLatency, externalLatency, displayLatency }`：`displayLatency` 取 `externalLatency >= 0 ? externalLatency : gatewayLatency`；当 `quality` 停在 `unknown`/`busy` 且 `displayLatency >= 0` 时，用 `getLatencyLevel(displayLatency)` 按延迟推断等级，否则沿用原始 `quality`（可能是 `unknown`/`busy`/`disabled`）。

### renderLiveness.ts — 渲染链存活探测

文件 `tauri-app/frontend/src/lib/renderLiveness.ts`（共 41 行）。用途：GPU/合成器崩溃时 `requestAnimationFrame` 会停，但 `setInterval` 仍跳动——纯 interval 心跳检测不到「渲染进程 JS 存活但画面假死」的目标故障场景（第 1–3 行）。

- 模块级状态：`lastRafTime`（第 14 行，初值 `performance.now()`）、`probing`（第 15 行，探测窗口是否开启）。
- 常量：`PROBE_REFRESH_MS = 4_000`（第 18 行，距上次探测超过一半阈值才重开窗口）；`PROBE_TIMEOUT_MS = 500`（第 20 行，第二帧永不回调时复位兜底）；`RENDER_STALL_THRESHOLD_MS = 10_000`（第 21 行，停滞判定阈值）。
- `startProbe(): void` — 第 24–34 行。已探测中则跳过；否则连开两帧 `requestAnimationFrame` 刷新 `lastRafTime`，第 2 帧后把 `probing` 复位；并用 `setTimeout(PROBE_TIMEOUT_MS)` 兜底复位。
- `isRenderLoopAlive(): boolean` — 第 36–40 行。**对外唯一出口**。窗口隐藏（`document.hidden`）直接返回 `true`（避免浏览器对隐藏页节流 rAF 被误判为停滞，调用方须先做可见性短路，见第 11–13 行注释）；否则距上次探测超过 `PROBE_REFRESH_MS` 时 `startProbe()`，最终返回 `performance.now() - lastRafTime <= RENDER_STALL_THRESHOLD_MS`。

> 功耗修复背景（第 4–9 行注释）：旧实现是模块级常驻 rAF 无限循环（导入即启动、永不停止），导致前台静态功耗高；改为「按需短探测」后判定语义（10s 停滞阈值）不变。

### utils.ts — 通用工具

文件 `tauri-app/frontend/src/lib/utils.ts`（共 32 行）。

- `cn(...inputs: ClassValue[])` — 第 4–6 行。组合 `clsx` + `tailwind-merge` 的 `twMerge(clsx(inputs))`，用于合并/去重冲突的 Tailwind 类名。`utils.test.ts` 第 4–20 行验证冲突去重（`cn('px-2','px-4') → 'px-4'`）、保留非冲突、过滤 falsy、空参返回 `''`。
- `extractErrorMessage(e: unknown): string` — 第 8–18 行。把任意错误转成可读字符串：是 `string` 原样返回；是 `Error` 取 `message`；是普通对象（Tauri 插件 reject 的 `{code,message}` 这类）优先取 `message` 字段（避免 `String()` 得到 `"[object Object]"`）；其余 `String(e)`。`utils.test.ts` 第 22–38 行验证：字符串原样、Error 取 message、普通对象得 `"[object Object]"`、数字得 `"42"`。
- 模块级 `memoryFallback` — 第 20 行。`Map<string,string>`，`localStorage` 不可用时的内存兜底。
- `safeStorage` 对象 — 第 22–29 行。封装 `localStorage` 的 `get`（第 23–25 行，catch 落 `memoryFallback`）、`set`（第 26–28 行，先写 `localStorage` 并把副本存入 `memoryFallback`，成功返回 `true`、异常返回 `false`）。
- 导出：`export { safeStorage }`（第 31 行）。

## Architecture

本模块以「无状态纯函数 / React Hook」为主，少数带模块级可变状态（`renderLiveness.ts` 的探测时间戳、`utils.ts` 的 `memoryFallback` 字典）。按依赖方向可画成三层：最底层是零依赖的 `easing-config` 与 `color`；其上是消费 `EasingConfig` 的 `animations` 与消费 `@/monitor`/`@/network/constants` 的 `latency`；再往上是面向渲染/运行的 `renderLiveness`、`utils` 以及独立成篇的悬浮栏子模块（[[lib-utils-dock]]，依赖上方的 `dock-spring`）。

```text
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ easing-config │   │    color      │   │  dock-spring  │
│ (EasingConfig)│   │ (hexToHsl)    │   │ (Motion/spring)│
└──────┬────────┘   └───────────────┘   └──────┬────────┘
       │ uses EasingConfig                      │ imported by
       ▼                                        ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│  animations   │   │   latency     │   │ dock-lens /   │
│ (variants fx) │   │ (getLatency*) │   │ glass-dock    │
└───────────────┘   └───┬───────┬───┘   └───────────────┘
                        │       │ uses QUALITY_CONFIG / NetworkQuality
                        ▼       ▼
                 ┌──────────────┐   ┌───────────────┐
                 │ @/network/   │   │  @/monitor/   │
                 │  constants   │   │   types       │
                 └──────────────┘   └───────────────┘

  renderLiveness ── rAF probe ──► isRenderLoopAlive()
  utils ── cn/extractErrorMessage/safeStorage (standalone helpers)
```

> 悬浮栏子模块（dock-spring / dock-lens / glass-dock）的细节见 [[lib-utils-dock]]，此处不展开。

## Data Flow

典型调用链路：

1. **动效链路**：调用方（如 `LogPanel`、`AnimatedCard`）从 `easing-config` 取 `getEasingConfig(refreshRate)` → 把 `EasingConfig` 传给 `animations.ts` 的 `createLogEntryVariants` / `createPanelAppleVariants` 生成 Framer Motion `variants` → 渲染层消费。
2. **延迟展示链路**：后端推送 `NetworkQuality`（来自 `@/monitor`）→ `mergeNetworkQuality` 增量合并 → `resolveQualityDisplay` 解析出 `quality/gatewayLatency/externalLatency/displayLatency` → `getLatencyColor` / `getLatencyLevel` 取配色与等级 → UI 渲染（首页卡与质量页共用；胶囊除外，见 `latency.ts` 第 61–65 行）。`extractGatewayLatency` / `extractExternalLatency` 在合并前后提取具体数值，并带 `details` 中位数的兜底。
3. **渲染存活链路**：调用方（已在 `main.tsx` 的 `isVisible` 分支、`useHeartbeat` 的 `paused` 分支做可见性短路）周期性调用 `isRenderLoopAlive()` → 内部按需开 2 帧 rAF 探测窗刷新 `lastRafTime` → 超过 10s 停滞判定为渲染链假死，触发整页重载。
4. **悬浮栏链路**（子模块 [[lib-utils-dock]]）：`useGlassRefraction` + `useDockLens` 同时挂载到悬浮栏 DOM，`dock-spring.Motion` 通过共享 rAF 驱动镜头/抬起/果冻/光斑曲线，每帧只写 `style` 不触发 React 重渲染。

## Connections

- [[lib-utils-dock]] — 悬浮栏镜头系统与折射玻璃子模块（dock-spring / dock-lens / glass-dock），本模块派生出去的独立文档。
- [[monitor]] — `latency.ts` 直接消费 `src/monitor/types.ts` 的 `NetworkQuality` 类型；延迟展示链路服务于质量页、状态栏胶囊。
- [[network]] — `latency.ts` 消费 `src/network/constants.ts` 的 `QUALITY_CONFIG` 取各等级配色；胶囊/面板展示同源于此。
- 前端通用动效基座见 `tauri-app/frontend/ANIMATION_GUIDE.md`（Framer Motion / Anime.js 分工），本模块的 `animations.ts` 与 `easing-config.ts` 是其中变体工厂的底座。

## Known Issues

- **`glass-dock.tsx` 依赖的 vendored Hyalite 资源在仓库中缺失**：`glass-dock.tsx` 第 19 行 `import './vendor/hyalite.js'`、第 20 行 `import type { HyaliteOptions } from './vendor/hyalite.d.ts'`，但 `tauri-app/frontend/src/lib/vendor` 目录在仓库中为空（通过 `list_files` 核实无文件）。这意味着该文件在当前源码树里**无法编译**，Hyalite 很可能是构建期/发布期注入或 vendored 但被 `.gitignore` 排除。接手前务必确认 Hyalite 的注入方式，否则 `useGlassRefraction` 会因找不到模块而构建失败。`useGlassRefraction` 内部对 `window.Hyalite` 是否存在做了防御（第 186–187、211 行 `hyalite.supported()`），但模块顶层 import 仍会先失败。
- **`latency.ts` 的 `LatencyType` 类型别名（第 5 行）未被本文件任何函数消费**——仅作为对外类型声明，属冗余但无害。
- **`latency.test.ts` 未覆盖 `resolveQualityDisplay`**（第 66–76 行）：该函数是卡片/页面的统一展示入口，但测试只覆盖了 `getLatencyLevel`、`getLatencyColor`、`mergeNetworkQuality`、`extractGatewayLatency`、`extractExternalLatency` 五个函数，缺少对「`unknown`/`busy` 按下延迟推断等级」分支的断言。
- **`color.ts` 不处理 3 位短写十六进制**（如 `#fff`）：第 2–4 行把长度 `< 4` 直接回退默认色，测试 `color.test.ts` 第 33–35 行也确认 `#fff` 返回默认 `{ h: 230, s: 70, l: 55 }`。若调用方期望短写支持需注意。
- **`renderLiveness.ts` 的 `isRenderLoopAlive()` 依赖调用方先做可见性短路**：文件注释（第 11–13 行）明确要求 `main.tsx`、`useHeartbeat` 在隐藏态先 return，函数本身对 `document.hidden` 直接 return `true`；若某调用方漏做短路，隐藏页被节流期间不会被误判，但一旦恢复可见、且此前 `lastRafTime` 已陈旧超过 10s，可能在恢复瞬间误报一次停滞。
