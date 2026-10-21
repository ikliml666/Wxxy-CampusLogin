---
title: android-frontend-app-shell
type: module
source_files:
  - android/frontend/.env
  - android/frontend/index.html
  - android/frontend/package.json
  - android/frontend/pnpm-workspace.yaml
  - android/frontend/postcss.config.js
  - android/frontend/tailwind.config.js
  - android/frontend/tsconfig.json
  - android/frontend/tsconfig.tsbuildinfo
  - android/frontend/vite.config.ts
  - android/frontend/vite.mock.config.ts
  - android/frontend/tauri-mock.js
  - android/frontend/public/mock-tauri-addon.js
  - android/frontend/public/mock-tauri.js
  - android/frontend/src/main.tsx
  - android/frontend/src/vite-env.d.ts
tags: [android, frontend, tauri, vite, tailwind, responsive, glassmorphism]
---

## Overview

本模块是 Android 前端应用的入口与布局外壳，负责构建 React 应用的基础设施、响应式导航结构及主题系统。核心职责包括：
1.  **构建与配置**：通过 Vite + Tauri 集成，提供开发、Mock 测试及生产构建流程。
2.  **主题与样式引擎**：实现基于 CSS 变量的动态主题切换（深色/浅色）、多档位玻璃拟态（Glassmorphism）渲染引擎及无障碍动画控制。
3.  **响应式适配**：根据屏幕形态（手机/平板）自动切换 `AppInner` 或 `TabletShell` 布局，并管理底部导航栏（BottomNav）与侧边栏（DockNav）。
4.  **Mock 体系**：提供内存级 Tauri API Mock，支持场景注入（如双网卡、离线状态），用于无原生环境的独立开发与截图测试。

## Key Components

### 构建与配置层

#### `vite.config.ts` (行 1-40)
*   **职责**：Vite 主配置文件。
*   **关键实现**：
    *   插件：`@vitejs/plugin-react` (行 9)。
    *   别名：`@` 指向 `./src` (行 11-13)。
    *   基础路径：`base: './'` (行 15)，适配 Tauri 相对路径。
    *   服务器：端口 `5174`，忽略 `.*.tmpdir` 防 EBUSY (行 17-23)。
    *   构建：输出 `dist`，关闭 Source Map，目标 `es2020` (行 26-31)。
    *   分包：手动拆分 `react`, `framer-motion`, `gsap`, `radix-ui` 为独立 vendor chunk (行 35-40)。

#### `vite.mock.config.ts` (行 1-43)
*   **职责**：专为浏览器端 Mock Tauri API 设计的 Vite 配置。
*   **关键实现**：
    *   继承基础配置 (行 38)。
    *   自定义插件 `tauri-mock-inject`：
        *   `transformIndexHtml`：注入 `/tauri-mock.js` 脚本 (行 16)。
        *   注入样式限制 `#root` 最大宽度 412px 并居中，模拟手机竖屏 (行 19-24)。
    *   缓存目录：移至 `os.tmpdir()/wxxy-vite-android` 规避 Windows Defender 拦截 (行 41)。
    *   端口：`5176` (行 43)。

#### `tailwind.config.js` (行 1-106)
*   **职责**：Tailwind CSS 主题扩展配置。
*   **关键实现**：
    *   暗黑模式：`darkMode: 'class'` (行 3)。
    *   颜色扩展：基于 CSS 变量定义语义化颜色 (`primary`, `destructive` 等)，格式 `hsl(var(--xxx))` (行 7-52)。
    *   圆角：`lg`, `md`, `sm` 基于 `var(--radius)` (行 54-58)。
    *   动画关键帧：`bounceSoft`, `shimmer`, `refreshHover` 等 (行 63-103)。
    *   特效：`backdropBlur.xs: '2px'` (行 104-106)。

#### `index.html` (行 1-116)
*   **职责**：应用入口 HTML，负责首屏骨架屏、主题预加载及 i18n 兜底。
*   **关键实现**：
    *   **CSS 变量体系**：定义深色/浅色默认值及骨架屏类 `.sk-*` (行 28-86)。
    *   **主题恢复**：读取 `localStorage` 中的 `campus-light-mode`, `campus-theme`, `campus-glass-level`，设置 `data-light`, `dark`, `data-glass` 属性 (行 10-25)。包裹在 try-catch 中防止跨域存储异常 (行 11-24)。
    *   **i18n 兜底**：根据 `app-language` 修改初始化提示文案 (行 107-114)。
    *   **DOM 结构**：根节点 `#root`，内含骨架屏容器 `.sk` (行 90-94)。
    *   **脚本加载**：引入 `/src/main.tsx` (行 116)。

#### `.env` (行 1)
*   **职责**：定义 Vite 构建环境变量。
*   **关键实现**：`VITE_PLATFORM=android` (行 1)，用于条件编译。

#### `package.json` (行 1-50)
*   **职责**：项目元数据与依赖管理。
*   **关键实现**：
    *   名称：`"campus-login-android-frontend"` (行 2)。
    *   脚本：`dev` (行 7), `build` (行 8), `typecheck` (行 9)。
    *   依赖：React 19, Tauri API v2, Zustand, Radix UI, Framer Motion, GSAP, i18next, `@vladmandic/human` (行 12-34)。
    *   特殊配置：`allowScripts.esbuild@0.25.12: true` (行 48-50)。

#### `tsconfig.json` (行 1-24)
*   **职责**：TypeScript 编译器配置。
*   **关键实现**：
    *   基础选项：`target: ES2020`, `jsx: react-jsx`, `strict: true` (行 3, 6, 14-17)。
    *   路径别名：`@/*` -> `./src/*` (行 20-22)。
    *   类型引用：`types: ["vite/client"]` (行 13)。

### 样式与主题引擎

#### `src/index.css` (行 1-1202)
*   **职责**：全局样式入口，定义 CSS 变量、玻璃拟态引擎及动画库。
*   **关键实现**：
    *   **CSS `@property` 注册**：显式注册 `--glass-glow-alpha`, `--glass-glow-reach`, `--glass-tint`, `--glass-shine`, `--glass-rim` 以支持平滑过渡 (行 8-35)。
    *   **主题变量体系**：`:root` 中定义 HSL 语义化变量及实底颜色 (行 37-64)。
    *   **玻璃档位系统**：
        *   **标准档**：`--glass-blur: 8px`, `--glass-tint: 54%` (行 70-115)。
        *   **极致档**：`[data-glass="ultra"]`，`--glass-blur: 14px`, `--glass-tint: 26%` (行 134-142)。
        *   **关闭档**：`[data-glass="off"]`，零滤镜开销 (行 116-122)。
    *   **暗色模式适配**：重定义背景及玻璃基色，增强 rim/shine (行 144-203)。
    *   **强制亮色对话框**：`.force-light-dialog` 解决暗色下白底对话框对比度崩坏 (行 220-241)。
    *   **玻璃栏渲染引擎**：`.glass-dock` / `.glass-nav` 使用多层 background 和 backdrop-filter，处理 Hyalite 兼容及边缘光影 (行 386-580)。
    *   **启动期黑屏修复**：`.glass-dock:not(.glass-live)` 强制 `--glass-tint: 100%` (行 465-468)。
    *   **药丸形适配**：`.glass-nav` 覆盖 `--glass-r: 34px` 修正 mask 采样 (行 483-485)。
    *   **动画库**：定义 `windowReveal`, `refreshShake`, `cardEnter` 等 (行 865-1133)。
    *   **无障碍**：`.anim-idle` 暂停脉冲动画；`prefers-reduced-motion` 禁用所有过渡 (行 1083-1202)。

### 认证与仪表盘组件

#### `src/auth/DashboardPanel.tsx` (行 1-1005)
*   **职责**：移动端仪表盘主面板，卡片式布局，支持编辑与自定义注入。
*   **类型定义**：
    *   `CardId` (行 45)：内置 ID 或自定义字符串。
    *   `ExtraCardDef` (行 54-61)：宿主注入的自定义卡片接口。
    *   `DashboardPanelProps` (行 102-117)：核心 Props。
*   **布局持久化**：
    *   `loadLayout` / `saveLayout` (行 73-86)：读写 JSON 格式卡片 ID 数组。
    *   `EXTRA_REMOVED_KEY` (行 91)：额外卡片黑名单。
*   **内置卡片组件**：
    *   `QuickActionsCard` (行 119-191)：DHCP 续租/释放，低网络质量触发光晕动画。
    *   `AccountManageCard` (行 193-297)：复用 `OnlineDeviceStack` 展示账号。
    *   `NetworkQualityCard` (行 299-339)：展示网关/外网延迟。
    *   `SelfOnlineCard` (行 418-592)：在线设备管理，支持生物验证解锁隐私信息。
    *   `SelfLogCard` (行 599-677)：上网记录，限制显示最近 5 条。
    *   `SelfServiceMergedEmptyCard` (行 681-701)：移动端专用空态合并卡。
*   **编辑模式手势**：
    *   `EditableCardItem` (行 737-831)：长按拖拽交换，阈值 `LONG_PRESS_MS = 350` (行 735)。
*   **主渲染逻辑** (行 833-1005)：过滤卡片，处理空态合并。

#### `src/auth/AboutDialog.tsx` (行 1-657)
*   **职责**：“关于”对话框，集成更新检查、下载、安装及赞助面板。
*   **关键实现**：
    *   **工具函数**：`formatSize`, `formatSpeed`, `renderInlineMarkdown`, `renderNotesLines` (行 43-96)。
    *   **核心逻辑**：
        *   自动检查更新 (行 147-167)。
        *   下载流程：监听进度，校验 checksum (行 175-265)。
        *   安装：调用 `api.installUpdate` (行 217-226)。
    *   **UI 布局**：左侧应用信息，右侧更新日志/下载区 (行 268-657)。

#### `src/auth/AboutDialogMobile.tsx` (行 1-256)
*   **职责**：移动端专属“关于”弹窗，替代桌面双栏布局。
*   **关键实现**：
    *   **下载策略**：根据 `updateSource` 调整镜像/GitHub 重试顺序 (行 70-101)。
    *   **安装**：调用 `api.installUpdate` 通过 FileProvider 交系统安装器 (行 103-110)。
    *   **UI 渲染**：更新源切换按钮，无 APK 资产兜底跳转 GitHub (行 140-256)。

#### `src/auth/useAuth.ts` (行 1-27)
*   **职责**：封装认证业务逻辑 Hook。
*   **关键实现**：
    *   状态订阅：`isLoggingIn`, `doLogin`, `doLogout` 等 (行 8-15)。
    *   辅助操作：`handleOpenPortal` (行 20-23), `handleOpenSelfService` (行 25-27)。

#### `src/auth/types.ts` (行 1-14)
*   **职责**：认证相关数据结构。
*   **关键实现**：`PortalStatusResult`, `CommandResult`, `LoginResult`。

#### `src/auth/index.ts` (行 1-5)
*   **职责**：认证模块统一导出。
*   **关键实现**：导出 `DashboardPanel`, `AboutDialog`, `useAuth`, types。

### 布局与导航组件

#### `src/components/layout/BottomNav.tsx` (行 1-122)
*   **职责**：移动端底部磨砂玻璃胶囊导航栏。
*   **关键实现**：
    *   标签定义：`TABS` 常量 (行 20-25)。
    *   动态标签：`useNavTabs` 根据 `config.enableNetworkQuality` 插入 'quality' 或 'monitor' (行 30-35)。
    *   视觉效果：`useGlassRefraction`, `useDockLens` 启用镜头放大效果 (行 46-65)。

#### `src/components/layout/DockNav.tsx` (行 1-295)
*   **职责**：桌面/平板侧边或底部导航栏。
*   **关键实现**：
    *   图标映射：`ICON_MAP` (行 30-39)。
    *   子组件：`DockItem` (行 41-85), `ActionButton` (行 88-169)。
    *   主组件：可见性过滤，Lens 集成，固定定位布局 (行 176-295)。

#### `src/components/layout/TitleBar.tsx` (行 1-273)
*   **职责**：应用标题栏，提供窗口控制、主题/语言切换、通知开关。
*   **关键实现**：
    *   Props：`showWindowControls` (行 11-24)。
    *   交互：`handleTitleBarMouseDown` 拖动, `handleTitleBarDoubleClick` 最大化 (行 73-90)。
    *   窗口控制按钮：最小化/最大化/关闭 (行 232-273)。

#### `src/components/layout/RightPanel.tsx` (行 1-278)
*   **职责**：右侧运行日志面板。
*   **关键实现**：
    *   常量：`LOG_ICONS`, `RIGHT_PANEL_ANIM_THRESHOLD = 50` (行 20-52)。
    *   核心逻辑：`handleClearWithAnimation` 使用 GSAP stagger 动画，含失败保护 `setTimeout` (行 64-125)。
    *   数据分片：`staticLogs` 无动画, `animLogs` 有入场/出场动画 (行 148-157)。

### 响应式适配组件

#### `src/App.tsx` (行 1-405)
*   **职责**：安卓双外壳入口组件。
*   **关键实现**：
    *   **双外壳逻辑**：`useFormFactor()` 判断平板/手机，分别渲染 `<TabletShell />` 或 `<AppInner />` (行 363-375)。
    *   **AppInner 状态管理**：
        *   Tab 状态：从 localStorage 读取，白名单校验 (行 57-63)。
        *   导航方向：计算索引差决定转场方向 (行 69-79)。
        *   动态 Tab 顺序：第 4 位取决于 `configEnableNetworkQuality` (行 70-76)。
        *   Tab 归位：配置加载后若 quality 开启但处于 monitor 页，自动切换 (行 131-136)。
        *   新手引导：无账号时弹出 `OnboardingWizard` (行 138-146)。
    *   **面板内容渲染**：dashboard, account, selfservice, quality, monitor, more (行 148-192)。
    *   **UI 结构**：Header (状态点, IspMark, UpdateAvailableDialog), Main (AnimatePresence), BottomNav (行 194-358)。
    *   **通知权限门控**：`NotificationPermissionGate` 仅在未询问时显示 (行 381-405)。

#### `src/components/tablet/TabletShell.tsx` (行 1-291)
*   **职责**：平板设备外壳容器，适配横竖屏。
*   **关键实现**：
    *   布局策略：横屏左主右日志，竖屏主内容全宽居中 (行 1-10)。
    *   缩放：顶部栏放大 `1.3x`，内容区缩小 `0.9x` (行 1-10)。
    *   面板路由：dashboard 渲染 `MobileDashboard`，其他渲染裁剪版面板 (行 160-222)。
    *   动画：`AnimatePresence mode="wait"` 实现滑入/滑出 (行 261-291)。

#### `src/components/mobile/MobileDashboard.tsx` (行 1-167)
*   **职责**：移动端总览页，复用桌面 DashboardPanel 架构。
*   **关键实现**：
    *   子组件：`MobileStatusCard` (网络状态, IspMark), `MobileMonitorCard` (后台监控) (行 38-139)。
    *   主组件：排除 `quickActions` 卡片，注入 `mobileHero` 和 `mobileMonitor` (行 141-167)。

#### `src/components/mobile/MobileMore.tsx` (行 1-44)
*   **职责**：移动端“更多”聚合页，Chips 切换子面板。
*   **关键实现**：
    *   Tab 定义：settings, speedtest, log, monitor (行 21-26)。
    *   状态管理：`subRaw` / `sub`，降级回 settings 防止不一致 (行 33-44)。

#### `src/components/mobile/MobileQuickActions.tsx` (行 1-76)
*   **职责**：首页悬浮快捷操作栏。
*   **关键实现**：
    *   布局：绝对定位底部导航栏上方，右对齐 (行 35-76)。
    *   样式：登录按钮实心主色，注销按钮磨砂玻璃拟态 (行 35-76)。
    *   逻辑：`handleQuickLogin` 先绑定 WLAN 再登录，忙状态禁用 (行 24-32)。

### 入口与崩溃恢复

#### `src/main.tsx` (行 1-132)
*   **职责**：应用入口，负责全局样式初始化、GSAP 配置及崩溃恢复。
*   **关键实现**：
    *   **GSAP 配置**：全局 ease `expo.out`，检测 `prefers-reduced-motion` 强制 duration 0 (行 17-25)。
    *   **主题初始化**：`initTheme` 基于 safeStorage 设置根节点 class/data 属性 (行 27-46)。
    *   **崩溃恢复**：
        *   可见性监听：页面隐藏暂停 GSAP，显示恢复 (行 62-91)。
        *   心跳检测：每秒更新心跳，每 2s 检测 elapsed > 10s 判定 GPU 崩溃，触发 `tryRecover` (行 93-113)。
        *   最多重载 3 次 (行 109)。
    *   **Root 挂载**：Dev 环境包裹 StrictMode，ErrorBoundary, LazyMotion (行 118-132)。

### Mock 体系

#### `tauri-mock.js` (行 1-277)
*   **职责**：内存级 Tauri API Mock，用于 Vite 开发及 Headless 截图。
*   **关键实现**：
    *   环境初始化：检测重复注入，支持 `?nomotion=1`, `?panel=` (行 5-26)。
    *   数据模型：静态 config, mockAccounts, onlineList, loginHistory, billRows (行 32-76)。
    *   命令分发器 `handlers`：
        *   `get_init_data` (行 88-97)。
        *   `save_config`：掩码字段保留旧值，触发 `config-changed` 事件 (行 99-110)。
        *   `query_self_dashboard` (行 111)。
        *   `self_offline_session` (行 112-117)。
        *   `check_campus_status` / `check_portal_status`：**不包裹在 `ok()` 中**，直接返回平铺结构体 (行 144-145)。
        *   `create_account` / `switch_account` 等：维护 mockAccounts 数组，广播 `config-changed` (行 150-196)。
    *   核心接口 `invoke`：路由命令，处理 listen 回调 ID (行 216-235)。
    *   事件系统 `emitEvent`：查找回调 ID 执行 (行 238-247)。
    *   截图辅助：支持 `?dialog=create|edit|bind` 自动点击 (行 261-277)。

#### `public/mock-tauri-addon.js` (行 1-84)
*   **职责**：叠加在 `mock-tauri.js` 之上的场景注入脚本。
*   **关键实现**：
    *   场景参数解析：`op`, `dual`, `night`, `offline`, `tab` (行 5-10)。
    *   Invoke 劫持：
        *   `check_campus_status` 强制返回已连接校园网 `i-wxxy` (行 14-20)。
        *   `get_adapters` 追加以太网适配器 (行 21-27)。
        *   `check_portal_status` 控制在线/可达状态 (行 28-30)。
        *   `get_init_data` 修改配置 (行 33-42)。
    *   后台状态模拟：动态生成 bgStatus (行 46-56)。
    *   定时事件发射：模拟后台轮询 `background-check-result` (行 58-69)。
    *   UI 辅助：自动点击 Dock 按钮，CSS 强制宽度 `emuw`，DOM 探测 `probe` (行 60-84)。

#### `public/mock-tauri.js` (行 1-195)
*   **职责**：Android 环境轻量级 Tauri Shim。
*   **关键实现**：
    *   环境初始化：设置 onboarding-done 及主题偏好 (行 10-19)。
    *   数据模型：mockConfig, mockAccounts (行 38-88)。
    *   Handlers：`get_config`, `save_config`, `list_accounts`, `do_login` 等 (行 90-153)。
    *   核心接口：`transformCallback`, `invoke` (行 155-176)。
    *   插件内部模拟：`__TAURI_EVENT_PLUGIN_INTERNALS__`, `__TAURI_PATTERN__` (行 180-186)。
    *   手动事件发射：暴露 `window.__mockEmitEvent` (行 189-195)。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        index.html                           │
│  (Skeleton, Theme Init, i18n Fallback, #root mount)         │
└──────────────────────┬──────────────────────────────────────┘
                       │ Load ES Module
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                      main.tsx                               │
│  (GSAP Config, Crash Recovery, Root Mount)                  │
└──────────────────────┬──────────────────────────────────────┘
                       │ Render
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                        App.tsx                              │
│  (Form Factor Detection: TabletShell vs AppInner)           │
├──────────────────────┬──────────────────────────────────────┤
│       TabletShell    │              AppInner                │
│  (Left/Right Split,  │  (Mobile Layout, BottomNav, Header)  │
│   Zoom Factors)      │                                      │
├──────────────────────┼──────────────────────────────────────┤
│  MobileDashboard     │  MobileDashboard                     │
│  AccountPanel        │  AccountPanel                        │
│  SelfServicePanel    │  SelfServicePanel                    │
│  QualityPanel        │  QualityPanel                        │
│  MonitorPanel        │  MonitorPanel                        │
│  More (MobileMore)   │  More (MobileMore)                   │
└──────────────────────┴──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  Mock System (Dev/Test)                     │
│  taui-mock.js ──► window.__TAURI_INTERNALS__.invoke         │
│  mock-tauri-addon.js ──► Scene Injection (Dual NIC, etc.)   │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **启动与初始化**：
    *   `index.html` 加载 `main.tsx`。
    *   `main.tsx` 执行 `initTheme()` 读取 `localStorage` 设置 CSS 变量，配置 GSAP。
    *   `App.tsx` 挂载，通过 `useFormFactor()` 决定渲染 `TabletShell` 或 `AppInner`。
    *   `AppInner` 从 localStorage 读取 `campus-mobile-tab`，结合 `configEnableNetworkQuality` 确定 `tabOrder` 和 `navDir`。

2.  **Mock 数据流**：
    *   开发环境中，`vite.mock.config.ts` 注入 `tauri-mock.js` 到 `window.__TAURI_INTERNALS__`。
    *   组件调用 `tauriApiWithRetry.invoke('command')`。
    *   `tauri-mock.js` 的 `invoke` 分发至 `handlers`，返回 Promise 或内存数据。
    *   若加载 `mock-tauri-addon.js`，则劫持特定命令（如 `check_campus_status`）返回场景化数据。
    *   后端变更（如 `save_config`）通过 `emitEvent('config-changed', ...)` 触发前端 Store 更新。

3.  **导航与面板切换**：
    *   用户点击 `BottomNav` 或 `DockNav` 项。
    *   `App.tsx` 更新 `tab` state。
    *   计算 `navDir` (-1, 0, 1) 驱动 `AnimatePresence` 动画。
    *   对应面板组件（如 `MobileDashboard`, `AccountPanel`）渲染。

4.  **主题与玻璃效果**：
    *   `index.html` 或 `main.tsx` 设置 `data-glass` 属性。
    *   `src/index.css` 根据 `[data-glass="..."]` 选择器应用对应的 `--glass-blur`, `--glass-tint` 等变量。
    *   `.glass-dock` / `.glass-nav` 利用这些变量进行多层渐变和 backdrop-filter 渲染。

## Connections

*   [[modules/android-frontend-auth]]: 认证模块，包含 `DashboardPanel`, `AboutDialog` 等核心业务组件。
*   [[modules/android-frontend-store]]: 状态管理模块，包含 `useConfigStore`, `useAuthStore`, `useQualityStore`，被 `App.tsx` 及各类面板广泛引用。
*   [[modules/android-frontend-hooks]]: Hooks 模块，包含 `useAuth`, `useHelloGate`, `useDockLens` 等，被 UI 组件调用。
*   [[modules/android-backend-tauri]]: Tauri 后端模块，提供真实的 `invoke` 命令实现，前端 Mock 旨在模拟其行为。

## Known Issues

1.  **Mock 数据一致性风险**：`tauri-mock.js` 中 `check_campus_status` 和 `check_portal_status` 不使用 `ok()` 包装，必须与前端直读属性的逻辑严格匹配，否则会导致 `onCampusNetwork` 等值为 `undefined` (行 143-145)。
2.  **崩溃恢复阈值**：`main.tsx` 中将渲染心跳超时阈值设为 **10s** (行 109)，而非默认的 5s，以防止低端机 GC 导致误判重启。但若真发生 GPU 崩溃，恢复可能较慢。
3.  **Tab 动态性与归位**：`App.tsx` 中 `tabOrder` 是动态的。如果用户在 `monitor` 页（当 quality 关闭时），随后开启了 quality 开关，页面会自动跳回 `quality` 页，避免底栏无高亮 (行 131-136)。此行为可能导致用户意外离开当前查看的监控页面。
4.  **玻璃栏启动期黑屏**：`src/index.css` 中 `.glass-dock:not(.glass-live)` 强制 `--glass-tint: 100%` (行 465-468) 以防止 Chromium backdrop root 问题导致的黑屏，但这会在 JS 补间期间产生短暂的视觉跳跃。
5.  **药丸形玻璃断口**：`src/index.css` 中 `.glass-nav` 局部覆盖 `--glass-r: 34px` (行 483-485) 以修正横向 mask 采样点，若全局 `--glass-r` 变化需同步调整此值。
6.  **Markdown 解析顺序**：`AboutDialog.tsx` 中 `renderNotesLines` 判断标题必须从 `###` 到 `#` (行 74-77)，若顺序错误会截断错位。