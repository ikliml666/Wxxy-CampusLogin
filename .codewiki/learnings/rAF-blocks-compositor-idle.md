---
title: 用 rAF 做的帧率控制器自己阻止了合成器休眠（已修复，保留作回归判据）
type: learning
source_files:
  - android/frontend/src/hooks/useAdaptiveFramePace.ts
  - android/frontend/src/lib/renderLiveness.ts
  - tauri-app/frontend/src/lib/renderLiveness.ts
  - android/frontend/src/index.css
tags: [教训, 安卓, 性能, 省电, 渲染, rAF]
---

## 现象

安卓前台静置仍满帧合成：10s 2438 帧、RenderThread 40%+、宿主 ≈0.8 核，整机 2.5W / 20% CPU（2026-09-13 真机 25060RK16C 实测）。**已修复**，修复方式与落地位置见「解决」；本条保留作回归判据。

## 根因

**页面存在常驻 rAF**（`useAdaptiveFramePace` 的 rAF 递归轮询 + `renderLiveness` 模块级 rAF 无限循环，导入即启动、永不停止）：Chromium 对"有活跃 rAF"的页面持续满帧派发 BeginFrame，合成器永不休眠——**用 rAF 做的帧率控制器本身阻止了合成器休眠**。同期 `.anim-idle .animate-pulse` 的 idle 冻结失效（见 [[css-comma-selector-shared-body-pitfall]]），雷达脉冲与骨架脉冲在用户无输入时仍在驱动合成器。

## 解决

三处已修（真机复测：静置 10s **0 帧、全线程 <1%**）：

- 帧率轮询由 rAF 改 `setInterval`：`android/frontend/src/hooks/useAdaptiveFramePace.ts:64`（轮询间隔 `PACE_POLL_MS = 250` 见 :18），交互起始由事件回调即时提帧 `markInteraction`（:26-29，零感知延迟），轮询只负责松手后的静止回落。
- 渲染存活判定改按需短探测：`android/frontend/src/lib/renderLiveness.ts:24-34`（`startProbe` 开一个 2 帧 ≈33ms 的 rAF 窗口，仅在距上次探测超过 `PROBE_REFRESH_MS = 4_000`（:18）时开窗；10s 停滞判定阈值与 `isRenderLoopAlive`（:36-40）语义不变）、桌面同法 `tauri-app/frontend/src/lib/renderLiveness.ts:24-34`。
- `.anim-idle .animate-pulse` 独立成规则、恢复 idle 冻结：`android/frontend/src/index.css:718-720`。

## 教训

① 省电/帧率控制**不要用常驻 rAF 实现**，用 `setInterval` 或按需短探测；② "空转但没渲染"不等于不耗电——rAF 本身就在阻止合成器休眠；③ 省电优化必须真机实测帧数与线程占用，不能只看代码意图；④ 回归判据：若再观察到静置满帧，先 grep 是否存在**模块级或递归**的 `requestAnimationFrame`——按需短探测的两处调用方（`main.tsx:95`、`useHeartbeat.ts:13`）都先做了可见性/暂停短路，属正常用法。

## Connections

[[android-power-three-fixes]]、[[android-keepalive-fgs-architecture]]、[[css-comma-selector-shared-body-pitfall]]
