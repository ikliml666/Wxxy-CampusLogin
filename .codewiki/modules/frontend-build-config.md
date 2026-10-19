---
title: frontend-build-config
type: module
source_files:
  - android/frontend/.env
  - android/frontend/index.html
  - android/frontend/package.json
  - android/frontend/postcss.config.js
  - android/frontend/public/mock-tauri-addon.js
  - android/frontend/public/mock-tauri.js
  - android/frontend/tailwind.config.js
  - android/frontend/tauri-mock.js
  - android/frontend/tsconfig.json
  - android/frontend/vite.config.ts
  - android/frontend/vite.mock.config.ts
  - android/frontend/src/index.css
  - android/frontend/src/vite-env.d.ts
tags: [frontend, build, vite, tailwind, tauri, mock, config]
---

## Overview
本模块负责 Android 前端应用（Campus Login Assistant）的构建配置、开发环境模拟及样式系统。核心基于 Vite 6.0 + React 19 技术栈，集成 Tailwind CSS 3.4 与 PostCSS。针对 Tauri 桌面端 API 在浏览器 DevTools 中不可用的痛点，提供了两套独立的 Mock 方案：`mock-tauri.js`（基础 UI 验证）和 `tauri-mock.js`（完整业务逻辑测试），通过自定义 Vite 插件注入到 HTML 中，确保前端代码在不依赖真实 WebView 的情况下可独立运行和截图验证。

## Key Components

### 构建工具链配置

#### `vite.config.ts` (Standard Config)
标准生产/开发配置，定义入口、别名、分包策略。
- **行 20-23**: `server.watch.ignored` - 忽略 IDE 临时目录 `**/.*.tmpdir/**`，防止 Windows 下 `fs.watch` EBUSY 错误导致 Vite 闪退。
- **行 35-40**: `build.rollupOptions.output.manualChunks` - 手动拆分 vendor chunks 以优化缓存：
  - `vendor-react`: 包含 `react/` 和 `react-dom/`。
  - `vendor-motion`: 包含 `framer-motion/`。
  - `vendor-gsap`: 包含 `gsap/`。
  - `vendor-radix`: 包含 `@radix-ui/`。
- **行 28**: `build.sourcemap: false` - 生产构建不生成 Source Map。
- **行 31**: `build.target: 'es2020'` - 编译目标为 ES2020。

#### `vite.mock.config.ts` (Mock Test Config)
专为浏览器本地测试设计的覆盖配置，不修改 git 跟踪文件。
- **行 10-28**: `tauriMockInject` 插件 (`transformIndexHtml`)：
  - **行 14**: 注入 `/tauri-mock.js` 到 `<head-prepend>`，优先于模块脚本执行，伪造 `window.__TAURI_INTERNALS__`。
  - **行 18-24**: 注入内联 CSS 限制 `#root` 最大宽度为 412px 并居中，模拟手机竖屏布局。
- **行 36**: `cacheDir` - 将 Vite 缓存移至系统临时目录 `os.tmpdir()/wxxy-vite-android`，绕过 Windows Defender 对 `node_modules/.vite` 的写入拦截。
- **行 40**: `server.port: 5176` - 占用端口 5176，避免与标准配置冲突。
- **行 32-33**: 注释说明 Windows 下 esbuild 二进制文件被杀软拦截的问题及环境变量修复方案。

#### `tailwind.config.js`
Tailwind CSS 配置，采用 HSL 变量驱动的主题系统。
- **行 4**: `darkMode: 'class'` - 通过 `<html class="dark">` 切换深色模式。
- **行 8-43**: `theme.extend.colors` - 扩展颜色系统，所有语义色（primary, secondary, destructive 等）均映射到 CSS 变量（如 `var(--primary)`）。
- **行 45-49**: `theme.extend.borderRadius` - 使用 `var(--radius)` 控制圆角大小。
- **行 51-55**: `theme.extend.fontFamily` - 定义 sans-serif 和 monospace 字体族。
- **行 57-98**: `theme.extend.animation` & `keyframes` - 定义自定义动画，包括 `bounceSoft`, `shimmer`, `iconHoverRotate`, `refreshSpin` 等。
- **行 100-102**: `theme.extend.backdropBlur` - 添加 `xs: '2px'` 模糊级别。

#### `postcss.config.js`
PostCSS 插件链，顺序为 `tailwindcss` -> `autoprefixer`。

#### `tsconfig.json`
TypeScript 编译器选项。
- **行 2**: `target: "ES2020"`。
- **行 4**: `module: "ESNext"`。
- **行 6**: `skipLibCheck: true`。
- **行 7**: `moduleResolution: "bundler"`。
- **行 18-20**: `paths: { "@/*": ["./src/*"] }` - 配置路径别名 `@` 指向 `src` 目录。
- **行 22**: `include: ["src"]`。

#### `package.json`
项目元数据与依赖。
- **行 1-3**: 名称 `campus-login-android-frontend`，私有包，版本 `2.4.1`。
- **行 6-8**: Scripts:
  - `dev`: `vite --host`。
  - `build`: `tsc --noEmit && vite build`。
  - `typecheck`: `tsc --noEmit`。
- **行 10-28**: Dependencies: 包含 Radix UI 组件库、Tauri Plugins v2、HUMAN (AI 人脸检测)、Framer Motion、GSAP、i18next、Zustand 等。
- **行 29-39**: DevDependencies: Vite 6, React 19, TypeScript 5, Tailwind CSS 3.4 等。
- **行 41-43**: `allowScripts.esbuild@0.25.12: true` - 允许 esbuild 安装脚本。

### 环境与环境变量

#### `.env`
- **行 1**: `VITE_PLATFORM=android` - 定义构建平台标识。

#### `index.html`
HTML 入口文件，包含骨架屏（Skeleton Screen）和内联初始化脚本。
- **行 12-20**: 内联脚本读取 `localStorage` 中的 `campus-light-mode` 和 `campus-theme`，设置 `data-light` 属性和 `theme-{name}` 类名，实现首屏主题同步。
- **行 21-66**: 内联 CSS 定义骨架屏样式 (`sk-*` 类)，使用 CSS 变量定义深浅色背景、文字颜色及强调色。
- **行 68-82**: DOM 结构：
  - `sk-tb`: 标题栏占位。
  - `sk-sb`: 状态栏，显示脉冲动画点 `sk-dot` 和初始化文本 `sk-init-text`。
  - `sk-main`: 主区域，包含侧边栏 `sk-side`、内容区 `sk-content`（含卡片和线条占位）和日志栏 `sk-log`。
- **行 84-92**: 内联脚本兜底语言：若 `app-language` 为 `en`，将初始化文本改为 "Initializing..."。
- **行 94**: 模块入口 `<script type="module" src="/src/main.tsx"></script>`。

### Mock 系统 (Browser Shim)

#### `public/mock-tauri.js` (Basic UI Mock)
轻量级 Tauri API 模拟，用于基础 UI 渲染验证。
- **行 12-18**: 初始化 `localStorage`：跳过新手引导 (`campus-onboarding-done`)，根据 URL 参数 `?theme=dark` 设置浅色/深色模式。
- **行 20-33**: `transformCallback` - 模拟 Tauri 回调 ID 生成机制，将 JS 函数绑定到 `window._cb{id}`。
- **行 35-66**: `mockConfig` - 默认配置对象，包含用户信息、网络检查间隔、主题模式等字段。
- **行 68-76**: `campusStatus` & `backgroundStatus` - 静态校园网状态和后台检测状态对象。
- **行 78-90**: `initData` - 初始化数据结构，包含配置、适配器列表、账户信息等。
- **行 92-126**: `handlers` 对象 - 命令分发器，映射 Tauri 命令名到模拟返回值：
  - `get_config`, `save_config`, `get_init_data`, `check_campus_status`, `do_login`, `do_logout` 等。
  - `check_portal_status`: 返回在线状态。
- **行 128-145**: `window.__TAURI_INTERNALS__` 注入：
  - `invoke`: 路由命令到 `handlers`，处理 `plugin:event|listen/unlisten` 事件监听注册。
  - `transformCallback`: 注册回调函数。
- **行 147-150**: `window.__TAURI_EVENT_PLUGIN_INTERNALS__` - 模拟事件注销接口。
- **行 152-154**: `window.__mockEmitEvent` - 测试用事件发射器。

#### `public/mock-tauri-addon.js` (Scenario Injection)
叠加在基础 Mock 之上的场景注入脚本，支持 URL 参数控制状态。
- **行 8-12**: 解析 URL 参数：
  - `op`: 运营商后缀 (默认 `@telecom`)。
  - `dual`: 是否双网卡模式 (`1`)。
  - `night`: 夜切模式 (`1`)。
  - `online`: 在线状态 (`offline=1` 表示离线)。
  - `tab`: 自动点击指定 Dock 页签按钮。
  - `emuw`: 模拟屏幕宽度。
  - `probe`: 诊断模式，输出溢出元素信息到 `<title>`。
- **行 14-15**: 检查 `mock-tauri.js` 是否已加载，未加载则退出。
- **行 17-18**: 保存原始 `invoke` 方法以便调用真实逻辑（如果存在）。
- **行 19-26**: 重写 `check_campus_status`：强制返回在线、SSID `i-wxxy` 等演示数据。
- **行 27-32**: 重写 `get_adapters`：返回 WLAN 适配器，若 `dual=1` 则追加以太网适配器。
- **行 33-35**: 重写 `check_portal_status`：根据 `online` 参数返回在线/离线状态。
- **行 36-37**: 重写 `do_login`：返回成功但提示“演示模式不执行登录”。
- **行 38-39**: 重写 `get_background_status`：调用内部 `bgStatus()` 函数。
- **行 40-48**: 重写 `get_init_data`：调用原始 invoke，然后修改返回的 `config.operator`（夜间模式置空）和 `nightOperatorRestore`。
- **行 50-62**: `bgStatus()` 函数：生成后台检测状态的动态数据，包含在线运营商、检查次数等。
- **行 64-65**: 延迟 600ms 发射一次 `background-check-result` 事件。
- **行 66-72**: 若指定 `tab` 参数，延迟 1.5s 后查找对应 `aria-label` 的按钮并触发点击。
- **行 73-76**: 每 3s 循环发射 `background-check-result` 事件，递增 `checkCount`。
- **行 77-82**: 若指定 `emuw` 参数，插入 CSS 强制 body 宽度，模拟窄屏布局。
- **行 83-92**: 若 `probe=1`，2.5s 后扫描超出视口的元素，将标签名和尺寸拼接到 `<title>` 供无头浏览器诊断。

#### `tauri-mock.js` (Full Business Logic Mock)
完整的业务逻辑 Mock，用于自助服务面板、账单查询等功能测试。
- **行 13-19**: 初始化 `localStorage`：跳过引导，设置默认面板为 `selfservice`。
- **行 25-30**: `config`：模拟配置，包含掩码密码 `***` 和设备别名 `deviceAliases`。
- **行 33-40**: `onlineList`：模拟 5 个在线会话，包含 IP、MAC、流量、时长等详细数据。
- **行 42-44**: `loginHistory`：模拟历史登录记录数组。
- **行 46-50**: `billRows` & `billSummary`：模拟账单明细和汇总数据。
- **行 52-53**: `ok()` 辅助函数：封装标准成功响应 `{success, message, data}`。
- **行 55-56**: `sleep()`：模拟异步延迟。
- **行 58-60**: `mockAdapters`：模拟单个以太网适配器。
- **行 64-74**: `handlers.get_init_data`：返回包含配置、适配器详情、空账户列表的初始化数据。
- **行 75-76**: `handlers.save_config`：更新内存配置，若字段非 `***` 则替换；延迟 0ms 发射 `config-changed` 事件。
- **行 77-78**: `handlers.query_self_dashboard`：返回 `onlineList` 和 `loginHistory`。
- **行 79-86**: `handlers.self_offline_session`：模拟注销，过滤掉指定 `sessionId` 的会话，延时 800ms。
- **行 87-88**: `handlers.query_self_online_log`：返回账单数据和汇总。
- **行 90-91**: `handlers.change_self_password`：模拟改密成功，延时 800ms。
- **行 92-93**: `handlers.verify_windows_identity` / `verify_biometric_identity`：模拟生物识别验证通过。
- **行 94-95**: `handlers.check_campus_status` / `check_portal_status`：直接返回平铺的状态对象（非 `ok()` 包裹），确保前端能正确读取 `onCampusNetwork` 和 `online` 字段。
- **行 96-100**: `handlers.render_heartbeat`, `get_debug_mode`, `get_logs`, `check_update` 等基础命令。
- **行 101-104**: `handlers.start_background_check` / `stop_background_check` / `get_background_status`。
- **行 105-110**: 窗口控制命令 (`show_window`, `minimize_window`, `close_window`, `notify_window_ready`, `cancel_auto_exit`) 和 `open_external`。
- **行 112-142**: `invoke(cmd, args)` 分发逻辑：
  - 处理 `plugin:event|listen`：维护 `eventListeners` 映射表，记录事件名到回调 ID 的关联。
  - 未知命令警告并返回 null。
- **行 144-153**: `emitEvent(event, payload)`：根据事件名遍历注册的回调 ID，从 `window['__cb_' + id]` 获取函数并执行。
- **行 155-163**: `window.__TAURI_INTERNALS__` 注入：
  - `invoke`: 上述分发逻辑。
  - `transformCallback`: 将回调存入 `window['__cb_' + id]` 并返回 ID。
  - `metadata`: 模拟当前窗口/Webview 标签为 'mock'。

### 样式系统

#### `src/index.css`
全局样式层，基于 Tailwind 和 CSS 变量。
- **行 4-31**: `@layer base :root` - 定义浅色模式下的 HSL 变量（background, foreground, primary, card 等）及表面色（surface-top/main/side）。
- **行 33-60**: `.dark` - 定义深色模式下的对应变量，调整对比度和饱和度。
- **行 62-83**: 主题变体 (`.theme-vibrant`, `.theme-forest`, `.theme-midnight`, `.theme-ocean`, `.theme-cherry`) - 分别覆盖 primary 和 accent 色调，并提供对应的 `.dark` 变体。
- **行 85-105**: `.force-light-dialog` - 强制亮色皮肤作用域，解决关于对话框在白底上暗色主题变量导致的对比度问题。显式声明 `color` 和所有相关变量。
- **行 107-113**: 全局重置 `border-color: transparent`，设置 html/body 字体平滑和背景色。
- **行 115-133**: 滚动条样式：全局细滚动条 (`thin`)，半透明拇指，隐藏轨道。
- **行 143-150**: `@layer components` - 卡片圆角增强，确保 `rounded-2xl.bg-white` 等组合具有特定阴影。
- **行 152-173**: 按钮和输入框圆角强制规则 (`rounded-xl`, `rounded-md`, `rounded-lg`)。
- **行 175-196**: `.btn-tonal-glass` - 磨砂玻璃按钮，利用 `color-mix` 混合主色调和背景色，暗色模式下提高主色调占比至 26%。
- **行 198-236**: `.glass-dock` - 玻璃质感导航栏，使用伪元素 `::before` 和 `::after` 实现渐变高光和边框效果，暗色模式适配。
- **行 238-246**: `.interval-input` - 隐藏数字输入框的原生 spinner 按钮。
- **行 248-266**: 应用容器圆角与最大化状态样式 (`app-outer-square`, `app-maximized`)。
- **行 268-276**: `.log-entry-flash` - 日志条目新出现时的闪光动画。
- **行 278-316**: `.btn-physical` / `.btn-press` - 物理按压感按钮，包含径向渐变光晕跟随鼠标位置 (`--mouse-x/y`)，缩放和阴影过渡。
- **行 318-356**: `.soft-btn` - Neumorphism (新拟态) 风格按钮，使用内外阴影模拟凸起/凹陷，暗色模式调整阴影方向。
- **行 358-376**: `.animated-card-interactive` - 交互卡片悬停上浮和边框显现。
- **行 378-386**: 窗口揭示动画 `.animate-window-reveal`。
- **行 388-394**: 刷新抖动动画 `.refresh-shake`。
- **行 396-408**: 日志条目悬停位移和高亮左侧栏。
- **行 410-414**: 面板内容可见性设置，禁用 `content-visibility: auto` 以保证动画渲染。
- **行 416-422**: 卡片入场动画 `.card-enter`，支持交错延迟 (`--stagger-i`)。
- **行 424-446**: 对话框遮罩和内容进入/退出动画 (`dialog-overlay-enter/exit`, `dialog-content-enter/exit`)。
- **行 448-453**: 输入框焦点光晕效果。
- **行 455-466**: 标题栏图标按钮动画 (`scale`, `rotate`)。
- **行 468-474**: 窗口控制按钮动画。
- **行 476-480**: 图标旋转入场 `.icon-spin-in`。
- **行 482-500**: 状态指示器动画 (`status-enter`, `status-offline-shake`)。
- **行 502-508**: 标签弹跳入场 `.label-bounce-in`。
- **行 510-514**: 数字缩放入场 `.num-scale-in`。
- **行 518-522**: `.anim-idle .animate-pulse` - 空闲时暂停脉冲动画，修复逗号选择器导致的失效问题。
- **行 524-526**: `.scrollbar-none` - 移动端隐藏滚动条。
- **行 528-530**: `.anim-idle .signal-glow-active` - 空闲时暂停信号光效。
- **行 532-536**: `.capsule-hover-wrap` - 胶囊悬停缩放。
- **行 538-546**: 信号条入场动画 `.signal-bar-enter`。
- **行 548-568**: 信号光效脉冲动画 `.signal-glow-active`，5s 循环。
- **行 570-586**: 列表项交互样式，悬停右移并显示左侧彩色条。
- **行 588-618**: `@media (prefers-reduced-motion: reduce)` - 尊重系统无障碍设置，禁用所有动画和过渡。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                    Browser / DevTools                        │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                   index.html                            │ │
│  │  ┌──────────────┐   ┌────────────────────────────────┐  │ │
│  │  │ Inline Init  │   │ <script src="/tauri-mock.js"> │  │ │
│  │  │ (Theme/Lang) │   │ (Defines __TAURI_INTERNALS__)  │  │ │
│  │  └──────────────┘   └────────────────────────────────┘  │ │
│  │                         ▼                               │ │
│  │  ┌────────────────────────────────────────────────────┐ │ │
│  │  │          React App (main.tsx)                      │ │ │
│  │  │  ┌─────────────┐  ┌──────────────┐  ┌───────────┐ │ │ │
│  │  │  │ Tauri API   │  │ Zustand Store│  │ UI Components│ │ │ │
│  │  │  │ (invoke)    │──▶│ (State Mgmt) │  │ (Tailwind) │ │ │ │
│  │  │  └─────────────┘  └──────────────┘  └───────────┘ │ │ │
│  │  └────────────────────────────────────────────────────┘ │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              Mock Layer (Memory)                        │ │
│  │  ┌───────────────────────────────────────────────────┐  │ │
│  │  │ tauri-mock.js / mock-tauri.js                     │  │ │
│  │  │ - Handlers: get_config, save_config, ...          │  │ │
│  │  │ - Event Bus: eventListeners map                   │  │ │
│  │  │ - Data Stores: onlineList, billRows, config       │  │ │
│  │  └───────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化阶段**:
    *   `index.html` 加载，执行内联脚本读取 `localStorage` 设置主题和语言。
    *   若使用 `vite.mock.config.ts` 启动，`tauriMockInject` 插件将 `tauri-mock.js` 注入头部。
    *   `tauri-mock.js` 执行，定义 `window.__TAURI_INTERNALS__`，初始化内存中的 `config`, `onlineList`, `eventListeners` 等数据结构。
    *   React 应用 (`main.tsx`) 挂载，发起首次 `invoke('get_init_data')`。
    *   Mock 层拦截请求，返回预设的 `initData` 对象。

2.  **状态管理**:
    *   React 组件接收数据，通过 Zustand store 更新全局状态。
    *   UI 组件根据 Zustand 状态和 Tailwind CSS 类名渲染界面。

3.  **用户交互**:
    *   用户操作（如点击登录、查看账单）触发 Tauri API 调用 (`invoke`)。
    *   Mock 层根据命令名查找 `handlers` 中的对应函数。
    *   例如 `self_offline_session`：过滤 `onlineList`，模拟网络延迟，返回成功消息。
    *   例如 `save_config`：更新内存 `config` 对象，并通过 `emitEvent('config-changed', ...)` 通知订阅者。

4.  **事件系统**:
    *   前端通过 `listen` 命令注册事件回调，Mock 层将回调 ID 存入 `eventListeners[eventName]`。
    *   后端（或 Mock 定时器，如 `setInterval` 在 `mock-tauri-addon.js` 中）调用 `emitEvent`。
    *   Mock 层遍历匹配的回调 ID，执行绑定的 JS 函数，传递 payload。

5.  **构建阶段**:
    *   `npm run build` 执行 `tsc --noEmit` 类型检查，随后 `vite build`。
    *   Vite 根据 `manualChunks` 配置将 React、Motion、GSAP、Radix 拆分为独立 chunk。
    *   产物输出至 `dist/` 目录。

## Connections
- [[frontend-app-logic]]: 前端业务逻辑层，依赖本模块提供的 Mock API 进行开发和测试。
- [[backend-tauri-plugins]]: 真实的 Tauri Rust 后端插件，本模块的 Mock 文件旨在模拟其行为。
- [[design-system-tailwind]]: 本模块定义的 Tailwind 配置和 CSS 变量是设计系统的核心部分。
- [[mobile-responsive-layout]]: `index.html` 和 `vite.mock.config.ts` 中的样式约束共同实现了移动端适配和截图验证。