---
title: Android Frontend Architecture
type: architecture
source_files:
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/src/index.css
  - android/frontend/src/i18n/index.ts
tags: [architecture, overview, tauri, android, react, glassmorphism]
---

## Overview

安卓前端（`android/frontend`）是 Windows 桌面端的**独立复刻树**：与桌面共用同一套视觉语汇（暗色 surface、主题色、圆角卡、发光 accent、悬浮玻璃栏）与同一套 Rust 协议核心（经 Cargo path 依赖继承，不复制代码），但 UI 布局与外壳为安卓重做。本文只覆盖本次提供的四个源头文件所描述的**应用外壳（app shell）**：进程启动与全局副作用编排、双形态外壳切换、主题与玻璃档位的首帧初始化、以及整套设计令牌/材质 CSS。

外壳的职责边界很清晰：`main.tsx` 负责「在任何 React 组件挂载之前」把所有全局状态（GSAP 全局配置、主题 class、玻璃档位属性、崩溃恢复监听）定下来；`App.tsx` 负责手机/平板两种形态的分支与手机外壳内部的页签导航；`index.css` 承载全部设计令牌（明暗、5 套主题色、三档玻璃）与材质工具类；`i18n/index.ts` 只做 i18next 初始化。

核心设计理念（均可在源码注释中找到依据）：

1. **首帧前定态**：主题与玻璃档位在 `initTheme()`（`android/frontend/src/main.tsx:27-44`）里、`ReactDOM.createRoot` 之前就写入根元素属性，避免启动闪烁一次默认档（注释见 `android/frontend/src/main.tsx:27`）。
2. **崩溃自恢复**：`setupCrashRecovery()`（`android/frontend/src/main.tsx:48-114`）叠加四条独立判据（root 空、GPU/WebGL 错误串、rAF 心跳停滞、可见性切换）在同一套「最多重载 3 次」的预算内。
3. **零重渲染动画**：玻璃材质的全部参数都是 CSS 自定义属性（`--glass-tint`/`--glass-shine`/`--glass-rim`），档位切换只改变量、由 CSS `transition` 过渡（`android/frontend/src/index.css:428-434`），不经过 React 重渲染。
4. **材质只用阴影分层**：卡片、悬浮栏、按钮一律「近影 + 远影 + 上缘高光」，不用整圈描边（`android/frontend/src/index.css:297-309`、`390-424`）。

## Key Components

### `android/frontend/src/main.tsx` — 进程入口与全局副作用

| 行号 | 符号 | 类型 | 用途 |
| --- | --- | --- | --- |
| 17 | `gsap.defaults({ ease: 'expo.out' })` | 调用 | 全局缓动；**不设**全局 `force3D`，因为 transform tween 各自显式声明，全局强制会让合成层不易回收（注释 15-16 行） |
| 18 | `gsap.config({ autoSleep: 5, nullTargetWarn: false })` | 调用 | 空闲 5 帧后自动休眠 ticker，静默空目标告警 |
| 19 | `gsap.ticker.lagSmoothing(500, 33)` | 调用 | 长任务后不补偿时间，避免动画「追帧」跳变 |
| 21 | `prefersReducedMotion` | 常量 | `window.matchMedia('(prefers-reduced-motion: reduce)').matches` |
| 22-25 | 减动效分支 | 条件块 | 命中减动效时把 `duration` 归零并关闭 lagSmoothing |
| 27-44 | `initTheme()` | 函数 | 读 `campus-light-mode` 决定 `dark` class 与 `data-light`；读 `campus-theme` 校验 `VALID_THEMES` 后加 `theme-<name>`（`default` 不加）；读 `campus-glass-level` 校验 `VALID_GLASS_LEVELS` 后写 `data-glass`，非法或缺失回落 `standard` |
| 46 | `initTheme()` | 调用 | 模块顶层立即执行 |
| 48-114 | `setupCrashRecovery()` | 函数 | 崩溃 / 渲染链失活自恢复，内部含 4 个监听器与 2 个 interval（见下） |
| 50 | `MAX_CRASH_RELOADS = 3` | 常量 | 单会话最多自动重载次数 |
| 52-60 | `tryRecover()` | 内部函数 | 计数 +1 且预算内则 1s 后 `window.location.reload()`；超限只打日志 |
| 62-69 | `visibilitychange` 监听 #1 | 监听器 | 回到前台时若 `#root` 无子节点则 `tryRecover()` |
| 71-77 | `window 'error'` 监听 | 监听器 | 错误消息含 `GPU`/`WebGL`/`SharedArrayBuffer` 时 `tryRecover()` |
| 79-91 | 心跳状态 + `visibilitychange` 监听 #2 | 状态/监听器 | 维护 `lastHeartbeatTime`/`isVisible`；不可见时 `gsap.globalTimeline.pause()`，可见时 `resume()` |
| 93-103 | 1s 心跳 interval | 定时器 | **仅当 `isRenderLoopAlive()` 为真**才刷新心跳时间；rAF 停滞时跳过，让 elapsed 判定生效（历史缺陷注释 95-97 行） |
| 105-113 | 2s 巡检 interval | 定时器 | `elapsed > 10000` 判定渲染心跳丢失（阈值从 5s 放宽到 10s，避免低端机长 GC 误判，注释 108 行） |
| 116 | `setupCrashRecovery()` | 调用 | 渲染前执行 |
| 118-120 | `AppWrapper` | 常量 | DEV 用 `React.StrictMode`，生产用 `React.Fragment` |
| 122-132 | `ReactDOM.createRoot(...).render(...)` | 调用 | 组件树嵌套顺序：`AppWrapper` → `ErrorBoundary` → `LazyMotion features={domMax} strict` → `MotionConfig reducedMotion="user"` → `App` |

### `android/frontend/src/App.tsx` — 双形态外壳与页签导航

| 行号 | 符号 | 类型 | 用途 |
| --- | --- | --- | --- |
| 47 | `AboutDialog` | lazy 组件 | `@/auth/AboutDialogMobile` 的 `AboutDialogMobile`，按需加载 |
| 48 | `MobileQuickActions` | lazy 组件 | `@/components/mobile/MobileQuickActions`，仅总览页快捷登录浮条 |
| 49 | `ThemeDialog` | lazy 组件 | `@/settings/ThemeDialog` |
| 50 | `OnboardingWizard` | lazy 组件 | `@/settings/OnboardingWizard` |
| 52-359 | `AppInner()` | 组件 | 手机外壳全部逻辑与渲染 |
| 53 | `useAppInit()` | hook 调用 | 应用初始化（配置与状态拉取） |
| 54 | `useAdaptiveFramePace()` | hook 调用 | 自适应帧率节奏 |
| 57-62 | `tab` state | 状态 | 从 `campus-mobile-tab` 恢复；白名单 `['dashboard','account','selfservice','quality','monitor','more']`（KI#14，注释 59-60 行） |
| 63 | `deferredTab` | 派生 | `useDeferredValue(tab)`，面板切换降优先级 |
| 70-76 | `tabOrder` | 常量数组 | 底栏页签顺序，第 4 位按 `configEnableNetworkQuality` 在 `quality`/`monitor` 间切换，索引差决定转场方向 |
| 77-79 | `navDir` / `tabRef` | 状态 / ref | `navDir` 取 `-1`/`0`/`1` 并传给 `AnimatePresence custom`；`tabRef` 供事件回调读最新页签 |
| 84 | `onlineOperatorLabel` | 派生 | 仅 `status.state === 'online'` 时给出运营商 i18n key |
| 115-117 | `easing` / `panelVariants` | 派生 | `useAnimationProfile()` 取缓动后 `createPanelAppleVariants(easing)` |
| 119-127 | `handleTabChange(next)` | 函数 | `markInteraction()` → 算方向 → 更新 `tabRef`/`tab` → 写 `campus-mobile-tab` |
| 131-136 | 页签归位 effect | effect | 配置就绪后若质量开启而当前是 `monitor`，强制回 `quality`（KI#14 配套） |
| 138-146 | 首启向导 effect | effect | `campus-onboarding-done` 未写且 `config.user` 为空时弹向导；标记由向导在「跳过」或「登录成功」时写 |
| 148-192 | `panelContent` | 局部渲染树 | `switch(deferredTab)` 分派 6 个面板：`MobileDashboard`/`AccountPanel`/`SelfServicePanel`/`QualityPanel`/`MonitorPanel`/`MobileMore` |
| 202-260 | `<header>` | JSX | 绝对定位轻 header：状态色点、在线运营商 chip、更新胶囊、`NetworkQualityCapsule`、赞助/设置/主题/关于四个图标按钮 |
| 263-267 | 快捷登录浮条 | JSX | 仅 `deferredTab === 'dashboard'` 时渲染 `MobileQuickActions` |
| 270-297 | `<main>` | JSX | 滚动容器；`paddingTop` = 安全区 + 78px，`paddingBottom` 首页 216px、其它 108px；内部 `AnimatePresence mode="wait"` + `m.div` 承载面板 |
| 299 | `<BottomNav>` | JSX | 底部 5 tab |
| 301-318 | `<ToastContainer>` / `<SponsorCard>` / `<AboutDialog>` / `<UpdateAvailableDialog>` | JSX | 全局反馈与弹窗 |
| 320-336 | `<ThemeDialog>` | JSX | 内联 `onSetTheme`/`onToggleLightMode`：同时写 store 与 `campus-theme`/`campus-light-mode`（移动外壳无 `useSettings` 实例） |
| 338-345 | `<ConfirmDialog>` | JSX | 删除账号确认；展示用显示名，调用仍传 id |
| 348-356 | `<OnboardingWizard>` | JSX | 新手向导（Radix Portal 到 body，不受外壳裁剪） |
| 363-375 | `App()` | 默认导出组件 | `ErrorBoundary` + `AnimationActiveProvider`；`formFactor === 'tablet'` 渲染 `TabletShell`，否则 `AppInner`；再挂单例 `FaceCaptureDialog` 与 `NotificationPermissionGate` |
| 381-405 | `NotificationPermissionGate()` | 组件 | 首启通知权限引导：未写 `campus-notification-prompt` 时弹 `ConfirmDialog`，确认后调 `requestNotificationPermission({ openSettingsIfDenied: true })`，任意按钮都写标记 |

### `android/frontend/src/index.css` — 设计令牌与材质

| 行号 | 选择器 / 变量 | 用途 |
| --- | --- | --- |
| 8-22 | `@property --glass-tint` / `--glass-shine` / `--glass-rim` | 把三个「未注册即离散」的值注册为可过渡类型（`<percentage>`、`<number>`），否则档位切换只会硬跳 |
| 25-97 | `:root` | 亮色令牌全量（HSL 分量形式）+ surface 三色 `--surface-top/main/side` + 玻璃默认档参数：`--glass-blur: 8px`、`--glass-sat: 2.08`、`--glass-tint: 54%`、`--glass-shine: 0.42`、`--glass-rim: 0.35`、`--glass-near/far`、`--glass-base: #ffffff`、`--glass-r: 18px` |
| 100-106 | `:root[data-glass="off"]` | 真零开销：`blur 0px`、`sat 1`、`tint 100%`、`shine`/`rim` 归零 |
| 131-142 | `:root[data-glass="ultra"]` | 只改通透度：`--glass-blur: 1.2px`、`--glass-tint: 10%`；`shine`/`rim` 不再覆写（与标准档共用，2026-10-21 用户校正） |
| 144-184 | `.dark` | 暗色令牌全量 + 暗色 surface + `--glass-base: #1c2028` + 暗色 `--glass-near/far` |
| 189-199 | `:root.dark[data-glass="off"]`、`:root.dark:is([data-glass="standard"], [data-glass="ultra"])` | 暗底靠亮边分离：off 档归零；standard 与 ultra 统一 `rim 0.62` / `shine 0.58` |
| 201-210 | `.theme-vibrant` / `.theme-forest` / `.theme-midnight` / `.theme-ocean` / `.theme-cherry`（各含 `.dark` 变体） | 5 套主题色只覆盖 `--primary`、`--primary-foreground`、`--ring`、`--accent`、`--accent-foreground` |
| 216-237 | `.force-light-dialog` | 强制亮色作用域：显式 `color` + 重定义全部浅色令牌，修「关于对话框固定白底在暗色下对比度崩坏」 |
| 243-258 | `html` / `body` | 抗锯齿、`optimizeLegibility`、`font-size: 16px`、`overflow: hidden`、`background: var(--surface-main)` |
| 260-288 | 滚动条组 | 6px 半透明细滚动条（完全隐藏会让「内容可滚动」不可发现，注释 261-262 行） |
| 301-327 | `.bg-card` 等卡片材质组 | `border-radius: 16px !important` + 近影 / 远影 / 上缘 1px 高光；`.dark` 变体加深；`.dark .bg-card.dark\:bg-muted` 保留「凹下去」层级 |
| 329-353 | `.rounded-xl`、输入类、`button[class*="rounded-lg"]` 等 | 统一圆角标定（卡片 16px、输入 10px、按钮 12px） |
| 359-380 | `.btn-tonal-glass` | 磨砂 tonal 按钮：`hsl(var(--primary))` 与玻璃膜做 `color-mix`；暗色主色占比 26%；off 档去掉 `backdrop-filter` |
| 393-435 | `.glass-dock`、`.glass-nav` | 悬浮栏三层结构：基膜 `background-color`、`backdrop-filter: var(--hyalite, blur() saturate())`（增强与降级同一行）、外缘暗线 `0 0 0 0.67px`、三档 260ms 变量过渡 |
| 454-457 | `.glass-dock.glass-ready:not(.glass-live)` | 兜底路径（等祖先链超时）钉回实底 `--glass-tint: 100%` |
| 462-467 | `:root:not([data-glass="off"]) .glass-dock:not(.glass-live):not(.glass-ready)` | 玻璃就绪前整条 `opacity: 0` + `pointer-events: none`，避免祖先 transform 使 backdrop root 采样为空而发黑 |
| 482-484 | `.glass-nav` | `--glass-r: 34px`（药丸半径 = 栏高一半），与 `.glass-dock` 的 18px 分开 |
| 495-508 | `--edge-fade-x` | 横向 cos²θ 衰减 mask（`0.3r` / `r` / `1.53r` 三个停靠点），**必须**定义在 `.glass-dock` / `.glass-nav` 而非 `:root`（`var()` 在声明处替换） |
| 510-541 | `.glass-dock::before`、`.glass-nav::before` | 内缘光·顶边：0.67px 起亮的 9 段垂直渐变 + 横向 mask |
| 547-572 | `.glass-dock::after`、`.glass-nav::after` | 内缘光·底边：同一剖面乘 0.72（下缘略弱） |
| 575-579 | `:root[data-glass="off"] .glass-dock`、`.glass-nav` | off 档 `backdrop-filter: none !important` |
| 583-626 | `.onboarding-deck-card`（含 `.dark` 与 `::after`） | 引导卡堆：三层阴影 + 顶部亮线，无描边 |
| 629-638 | `.interval-input` | 隐藏 number spinner |
| 640-682 | `.app-outer-square`、`.surface-*-square`、`.app-maximized*` | 窗口外框 16px 圆角与三面分别圆角；最大化时全部归零 |
| 684-700 | `.log-entry-flash` + `@keyframes logFlash` | 新日志条目 0.8s 底色闪烁 |
| 702-744 | `.btn-physical` | 按钮手感：hover `scale(1.03)` + 径向光斑（`--mouse-x`/`--mouse-y`），active `0.97`；玻璃栏内禁用 hover 放大 |
| 746-782 | `.btn-press` | 轻量版按钮手感（1.02 / 0.96） |
| 784-835 | `.soft-btn` | 新拟物软按钮（亮暗两套四向阴影，active 内凹） |
| 837-862 | `.animated-card-interactive` | 悬停描边 + 上浮 2px + `focus-visible` ring；`will-change` 仅在 hover 期生效 |
| 905-909 | `.layout-smooth-resize` | 布局尺寸过渡 + `contain` |
| 911-917 | `.panel-content` | 强制 `content-visibility: visible`（`auto` 会跳过视口外含动画元素，注释 912-914 行） |
| 930-933 | `.card-enter` | 卡片入场 0.35s，按 `--stagger-i` 错峰 50ms |
| 935-983 | `@keyframes dialog*` + `.dialog-overlay-*` / `.dialog-content-*` | 弹窗遮罩 0.2s 入 / 0.12s 出，内容 0.3s 入 / 0.12s 出 |
| 997-1019 | `.titlebar-icon-btn` / `.titlebar-win-btn` | 标题栏图标与窗口按钮的缩放手感 |
| 1021-1071 | `iconSpinIn`、`statusEnter*`、`statusOfflineShake`、`labelBounceIn`、`numScaleIn` | 状态点、标签、数字的入场与抖动关键帧及工具类 |
| 1082-1084 | `.anim-idle .animate-pulse` | idle 冻结脉冲；**独立成规则**，不得并入逗号列表（注释 1077-1081 行记了曾因合并而失效） |
| 1087-1088 | `.scrollbar-none` | 移动端隐藏滚动条 |
| 1090-1092 | `.anim-idle .signal-glow-active` | idle 冻结信号光晕 |
| 1094-1132 | `.capsule-hover-wrap` / `.signal-bar-enter` / `.signal-glow-*` | 胶囊悬停放大、信号条入场、5s 呼吸光晕 |
| 1134-1160 | `.list-item-interactive`（含 `::before`） | 列表行悬停右移 4px + 左侧 3px 主题色竖条 |
| 1162-1201 | `@media (prefers-reduced-motion: reduce)` | 统一 `animation: none !important`，并关掉玻璃栏过渡与 `will-change` |

### `android/frontend/src/i18n/index.ts` — 国际化初始化

| 行号 | 符号 | 用途 |
| --- | --- | --- |
| 1-3 | `i18n` / `initReactI18next` / `LanguageDetector` | 三个依赖导入 |
| 5-6 | `zh` / `en` | 静态导入 `./locales/zh.json`、`./locales/en.json` |
| 8-26 | `i18n.use(...).init(...)` | `fallbackLng: 'zh'`；`lng` 从 `localStorage['app-language']` 读（`try/catch` 包裹，隐私模式取不到时回落 `undefined` 交给探测器）；`detection.order: ['localStorage','navigator']`、`caches: ['localStorage']`；`interpolation.escapeValue: false` |
| 28 | `export default i18n` | 供 `main.tsx` 的 `import './i18n'` 副作用加载 |

## Architecture

```text
┌──────────────────────── android/frontend (WebView) ────────────────────────┐
│ main.tsx                                                                   │
│   initTheme() 27-44 ─► <html> .dark / data-light / theme-* / data-glass    │
│   setupCrashRecovery() 48-114 ─► error / visibility / 2× interval 监听     │
│   createRoot().render 122-132                                              │
│      └► AppWrapper(StrictMode|Fragment) ► ErrorBoundary ► LazyMotion       │
│                              ► MotionConfig ► App              (App.tsx)   │
├────────────────────────────────────────────────────────────────────────────┤
│ App.tsx  App() 363-375                                                    │
│   useFormFactor() ──┬─ 'tablet' ─► <TabletShell/>   (桌面 Dock 布局)        │
│                     └─ 其它     ─► <AppInner/>      (手机移动外壳)          │
│   共用单例: <FaceCaptureDialog/> (tauriApi 命令式驱动)                     │
│             <NotificationPermissionGate/> 381-405                          │
├────────────────────────────────────────────────────────────────────────────┤
│ AppInner 52-359                                                           │
│   header 202-260 ► main 270-297 ► panelContent switch 148-192              │
│   BottomNav 299 ─► handleTabChange 119-127 ─► campus-mobile-tab            │
│   AnimatePresence(mode=wait, custom=navDir) ─► m.div(panelVariants)        │
│   面板: MobileDashboard│AccountPanel│SelfServicePanel│QualityPanel          │
│         │MonitorPanel│MobileMore    ← 均为 @/… 静态或 lazy 引入             │
├────────────────────────────────────────────────────────────────────────────┤
│ index.css  设计令牌层                                                      │
│   :root 25-97 ── .dark 144-184 ── data-glass off/ultra 100-142             │
│   theme-* 201-210 ── 材质 .bg-card 301 / .glass-dock 393 / .btn-* 702-835  │
│   i18n/index.ts 8-26 ──► 全树 t() 文案                                     │
├────────────────────────────────────────────────────────────────────────────┤
│                                                   ▼ IPC (invoke / event)  │
│                     Rust 协议核心（Cargo path 依赖，两端共用，不复制）       │
└────────────────────────────────────────────────────────────────────────────┘
```

要点：外壳只在 `App()` 一处分叉形态（`android/frontend/src/App.tsx:363-375`），手机与平板两条路径**共用同一套面板组件**（`AccountPanel`、`QualityPanel` 等），差异只在容器布局与是否渲染桌面件——`TitleBar`/`StatusBar`/`RightPanel`/`DockNav`/`FluidBackground`/`Onboarding`/`Sponsor` 在手机外壳不再渲染（`android/frontend/src/App.tsx:4`）。

## Data Flow

以「切换页签」与「切换玻璃档位」两条最容易出错的数据流为例：

```text
[用户点击底部 tab]
   │  BottomNav.onChange                      (App.tsx:299)
   ▼
handleTabChange(next)                        (App.tsx:119-127)
   ├─ markInteraction()        → 唤醒自适应帧率 (useAdaptiveFramePace)
   ├─ tabOrder.indexOf 比较     → setNavDir(-1|0|1)  决定转场方向
   ├─ tabRef.current = next
   ├─ setTab(next) → useDeferredValue → deferredTab   (App.tsx:63)
   └─ safeStorage.set('campus-mobile-tab', next)      持久化
                    │
                    ▼
        switch(deferredTab)                  (App.tsx:148-192)
                    │ 生成 panelContent
                    ▼
   <AnimatePresence mode="wait" custom={navDir}>      (App.tsx:281-295)
     └─ <m.div key={deferredTab} variants={panelVariants}>
          └─ <ErrorBoundary><Suspense>{panelContent}</Suspense></ErrorBoundary>

[用户切换玻璃档位]
   ThemeDialog / 设置页 → safeStorage.set('campus-glass-level', …)
   ▼ （下次启动，或运行时）
   root.setAttribute('data-glass', <off|standard|ultra>)   (main.tsx:42-43)
   ▼
   :root[data-glass=…] 只覆盖一组变量      (index.css:100-142, 189-199)
   ▼
   .glass-dock / .glass-nav 的 transition               (index.css:428-434)
      --glass-tint 260ms / --glass-shine 260ms / --glass-rim 260ms
   ▼
   ::before / ::after 的渐变 alpha 与 mask 重新求值（不触发 React 渲染）
```

`initTheme()` 的读取顺序是硬约定：先判 `campus-light-mode === '1'` 决定 `dark` class（`android/frontend/src/main.tsx:27-34`），再读主题名（`android/frontend/src/main.tsx:27-30`），最后读玻璃档位并**必须在首帧前**落到 `data-glass`（`android/frontend/src/main.tsx:27-29`），否则启动会闪一次默认档。

## Connections

- [[android-frontend-account]] — `AccountPanel`/`SelfServicePanel` 由本外壳在 `android/frontend/src/App.tsx:153-168` 分派渲染；账号增删改切换的 handler 来自 `useAccount()`（`android/frontend/src/App.tsx:97`）。
- [[android-frontend-settings]] — `ThemeDialog`、`OnboardingWizard` 为 lazy 引入（`android/frontend/src/App.tsx:49-50`）；`VALID_THEMES`/`VALID_GLASS_LEVELS` 与主题存储键来自 `@/settings/constants`（`android/frontend/src/main.tsx:10`）。
- [[android-frontend-monitor]] — `MonitorPanel`/`QualityPanel`/`NetworkQualityCapsule` 及 `useMonitor()` 的三个 handler（`android/frontend/src/App.tsx:96`、`180-187`）。
- [[android-frontend-hooks-stores]] — `useConfigStore`/`useAuthStore`/`useQualityStore`/`useThemeStore`/`useLogToastStore` 与 `tauriApiWithRetry`（`android/frontend/src/App.tsx:14-26`）。
- [[android-frontend-i18n-and-lib]] — `safeStorage`/`cn`、`lib/renderLiveness`、`lib/animations`、`lib/notificationPermission`（`android/frontend/src/main.tsx:9-11`、`android/frontend/src/App.tsx:21-22`、`45`）。
- [[android-tauri-shell-and-plugins]] — `tauriApi` 的 IPC 面向 Rust 命令；人脸录入弹窗 `FaceCaptureDialog` 由验证门命令式驱动（`android/frontend/src/App.tsx:369-370`）。
- [[android-rust-backend]] — 协议核心单点存在于 Rust crate，安卓经 Cargo path 依赖继承（双端共享约定），前端只经 IPC 触达。

## Known Issues

1. **GPU 崩溃时心跳定时器仍正常跳动，会掩盖渲染链失活**。历史缺陷已在 `android/frontend/src/main.tsx:107-113` 用 `isRenderLoopAlive()` 守卫修掉：rAF 停滞时跳过心跳刷新，让 `elapsed` 判定能触发恢复。
2. **心跳阈值不能收回到 5s**。`android/frontend/src/main.tsx:108-109` 注释记录：低端机长 GC 或长任务阻塞超过 5s 会误判为 GPU 崩溃而整页重载，故放宽到 10s；改动此值需同步 `lib/renderLiveness` 的语义。
3. **玻璃栏启动期曾整条发黑**：祖先层的 `opacity<1`、`transform`、`will-change` 会让 Chromium 把该层当作 backdrop root，`backdrop-filter` 采样不到页面内容被填黑，54% 白膜读出深灰栏。现行做法是就绪前整条 `opacity: 0` + `pointer-events: none`（`android/frontend/src/index.css:437-467`），就绪帧只走 240ms 淡入；兜底超时路径用 `.glass-ready:not(.glass-live)` 钉回实底（`android/frontend/src/index.css:454-457`）。不要改回「先白膜、后变透明」的两段式。
4. **`--edge-fade-x` 不能提到 `:root`**：`var()` 在声明元素上替换，放到 `:root` 会把 `--glass-r` 固化成 18px，药丸形 `::before` 的 computed mask 停在 `0.3r`/`r`/`1.53r`（5.4/18/27.54px），亮线在弧上出现断口（`android/frontend/src/index.css:486-494`）。
5. **`.anim-idle .animate-pulse` 必须独立成规则**：2026-09-13 曾与 `.scrollbar-none` 并进同一条逗号规则，`animation-play-state` 只落在末个选择器上，导致 idle 冻结整条失效、后台雷达脉冲与骨架脉冲持续驱动合成器（`android/frontend/src/index.css:1075-1084`）。
6. **`.panel-content` 必须 `content-visibility: visible`**：改回 `auto` 会跳过视口外或判为不可见的含动画元素，表现为「数据在但动画没播」（`android/frontend/src/index.css:911-917`）。
7. **页签恢复存在无高亮窗口**：`monitor` 只在质量检测关闭时才是可见页签；若跨会话恢复出 `monitor` 而配置已开启质量，底栏会无高亮项，故有归位 effect（`android/frontend/src/App.tsx:129-136`）。`tab` 白名单必须覆盖 `handleTabChange` 可写入的全部页签（`android/frontend/src/App.tsx:57-62`，KI#14），新增页签时两处同时改。
8. **首启向导标记写错位置就等于没有引导**：`campus-onboarding-done` 只能由向导在「跳过」或「登录成功」时写入（`android/frontend/src/App.tsx:138-146`）；不要在 `App` 挂载时直接写，否则用户不知道填什么、哪个密码。
9. **玻璃档位边缘参数已被三次校正收敛**：standard 与 ultra 的 `shine`/`rim` 必须完全一致，两档只差 `tint + blur`（`android/frontend/src/index.css:136-142`、`193-199`）；再给 ultra 单独加边框阴影会被读作「增强边框阴影」并被要求回退。
10. **`.force-light-dialog` 是存量缺陷的修补**：关于对话框为固定白底设计，暗色主题下继承的近白文字与暗卡变量对比度崩坏；容器重定义浅色令牌且必须显式 `color`（`color` 是继承属性，`android/frontend/src/index.css:216-237`）。新写固定亮底弹窗时复用该类，不要另起一套。