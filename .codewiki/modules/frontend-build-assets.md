---
title: frontend-build-assets
type: module
source_files:
  - android/frontend/.env
  - android/frontend/index.html
  - android/frontend/package.json
  - android/frontend/pnpm-lock.yaml
  - android/frontend/pnpm-workspace.yaml
  - android/frontend/postcss.config.js
  - android/frontend/public/mock-tauri-addon.js
  - android/frontend/public/mock-tauri.js
  - android/frontend/tauri-mock.js
tags: [frontend, build-config, mock, tauri, dependency-management]
---

## Overview
本模块负责 Android Tauri 应用前端构建环境的配置、依赖锁定及开发阶段的原生 API Mock 模拟。核心职责包括定义构建常量（`.env`）、管理项目元数据与依赖版本（`package.json`, `pnpm-lock.yaml`）、配置 CSS 预处理管线（`postcss.config.js`），以及在开发环境中通过多层 Mock 脚本模拟 Tauri 原生能力（`mock-tauri.js`, `mock-tauri-addon.js`, `tauri-mock.js`）。该模块确保前端代码在无真实原生后端或需要特定测试场景（如夜切态、离线状态、双网卡）时能够独立运行和调试。入口 HTML（`index.html`）还负责首屏主题/语言预加载与骨架屏样式。

## Key Components

### Configuration Files

#### `android/frontend/.env`
- **职责**：定义构建环境常量，供 Vite 注入全局变量。
- **关键实现**：
  - 第 1 行：`VITE_PLATFORM=android`，标识当前运行平台为 Android，对应全局变量 `import.meta.env.VITE_PLATFORM`。

#### `android/frontend/package.json`
- **职责**：项目元数据、依赖管理与构建脚本配置。
- **关键实现**：
  - 第 2 行：`name`: `"campus-login-android-frontend"`。
  - 第 4 行：`version`: `"2.4.2"`（当前前端版本）。
  - 第 5 行：`type`: `"module"`（启用 ES Modules）。
  - Scripts：
    - 第 7 行：`dev`: `"vite --host"`，启动开发服务器并允许外部访问。
    - 第 8 行：`build`: `"tsc --noEmit && vite build"`，先 TypeScript 类型检查再生产构建。
    - 第 9 行：`typecheck`: `"tsc --noEmit"`。
  - Dependencies（部分）：`@tauri-apps/api` (^2, 锁定 2.12.3)、`@tauri-apps/plugin-biometric` (~2, 锁定 2.4.2)、`@tauri-apps/plugin-notification` (^2.4.0, 锁定 2.6.0)、`@tauri-apps/plugin-opener` (^2, 锁定 2.7.1)、`react` (^19.0.0, 锁定 19.3.0)、`react-dom` (^19.0.0, 锁定 19.3.0)、`zustand` (^5.0.13, 锁定 5.0.15)、`@radix-ui/react-dialog` 等、`framer-motion` (^12.38.0, 锁定 12.43.0)、`gsap` (^3.15.0)、`i18next` (^26.3.1, 锁定 26.4.2)、`react-i18next` (^17.0.8, 锁定 17.0.15)、`i18next-browser-languagedetector` (^8.2.1)、`@vladmandic/human` (^3.3.6)。
  - DevDependencies：`typescript` (^5.5.0, 锁定 5.9.3)、`vite` (^6.0.0, 锁定 6.4.4)、`@vitejs/plugin-react` (^4.3.0, 锁定 4.7.0)、`tailwindcss` (^3.4.10, 锁定 3.4.19)、`autoprefixer`、`postcss` 等。
  - 第 48-50 行：`allowScripts` 字段声明 `"esbuild@0.25.12": true`，允许 esbuild 构建脚本执行。

#### `android/frontend/pnpm-lock.yaml`
- **职责**：pnpm 依赖锁定文件（lockfileVersion 9.0），确立确定性依赖快照。
- **关键实现**：
  - 第 3-5 行：`settings` 中 `autoInstallPeers: true`、`excludeLinksFromLockfile: false`。
  - `importers` 段（第 9-110 行）列出直接依赖精确版本，如 `react@19.3.0`、`vite@6.4.4(@types/node@22.20.5)(jiti@1.21.7)`。
  - `packages` 段（第 112 行起）及 `snapshots` 段（第 1505 行起）记录所有传递依赖与安装图谱，含 `@esbuild/*` 多平台原生二进制（第 205-359 行可选包）、`@rollup/rollup-*` 原生包（第 735-871 行可选包）。

#### `android/frontend/pnpm-workspace.yaml`
- **职责**：pnpm workspace 级别全局设置（允许/禁止原生构建）。
- **关键实现**：
  - 第 1-2 行：`allowBuilds:` 下 `esbuild: set this to true or false`——此为占位提示文本，未生效为确定布尔值，需手动改为 `true` 或 `false` 才能控制 esbuild 构建。

#### `android/frontend/postcss.config.js`
- **职责**：配置 PostCSS 插件链，服务于 Tailwind CSS。
- **关键实现**：
  - 第 1-6 行：默认导出 ES Module 对象，插件按顺序为 `tailwindcss: {}`（核心引擎）与 `autoprefixer: {}`（浏览器前缀）。

### HTML Entry Point

#### `android/frontend/index.html`
- **职责**：应用入口 HTML，初始化 CSS 变量、骨架屏、主题/语言预加载及 React 挂载点。
- **关键实现**：
  - **头部与图标**（第 1-9 行）：`<meta>` 视口、`/32x32.png` 与 `/128x128.png` 图标、标题“校园网登录助手”。
  - **主题初始化脚本**（第 10-26 行）：IIFE 读取 `localStorage`：
    - 第 12 行：`campus-light-mode` 为 `'1'` 则设 `data-light='1'`（第 13-14 行），否则加 `dark` class（第 16 行）。
    - 第 18-19 行：`campus-theme` 非 `'default'` 则加 `theme-{th}` class。
    - 第 21-23 行：`campus-glass-level` 设 `data-glass` 属性（值 `off`/`ultra`/默认 `standard`），首帧前定档防跳变。
  - **CSS 变量系统**（第 27-87 行）：
    - `:root`（第 28-51 行）深色默认值，如 `--bg-primary: #0f172a`、`--sk-accent: #3b82f6`。
    - `html[data-light]`（第 52-73 行）浅色覆盖，如 `--bg-primary: #ffffff`。
    - 第 74-86 行：重置样式、布局约束（`overflow: hidden`）、骨架屏类（`.sk-tb`、`.sk-sb`、`.sk-main`、`.sk-side`、`.sk-content`、`.sk-card`、`.sk-log`）。
  - **骨架屏 DOM**（第 90-103 行）：`#root` 内 `.sk` 结构，状态栏含 `id="sk-init-text"` 文本“正在初始化...”（第 93 行）。
  - **骨架屏文案国际化兜底**（第 104-115 行）：若 `localStorage.app-language === 'en'`（第 109 行），将 `sk-init-text` 改为 `"Initializing..."`。
  - **React 挂载**（第 116 行）：`<script type="module" src="/src/main.tsx"></script>`。

### Mock Scripts

#### `android/frontend/public/mock-tauri.js`
- **职责**：基础 Tauri API Shim（仅供浏览器实测，绝不入库），提供 `__TAURI_INTERNALS__` 模拟。
- **关键实现**：
  - 第 4-19 行：IIFE，若已有 `__TAURI_INTERNALS__` 则退让；设 `localStorage` 跳过引导（`campus-onboarding-done`、`campus-notification-prompt`），`?theme=dark` 控深色。
  - 第 21-36 行：`transformCallback(callback, once)` 分配 `_cb{id}` 全局函数。
  - 第 38-57 行：`mockConfig` 对象字段：`user:'23331150'`、`operator:'cmcc'`、`themeMode:'dark'`、`requiredNetworkName:'i-wxxy'`、`configSchemaVersion:7`、`displayName:'演示账号'` 等。
  - 第 59-72 行：`campusStatus`、`backgroundStatus` 静态对象。
  - 第 76-80 行：`mockAccounts` 数组（3 项，含 `id/displayName/user/operator/passwordSaved`）。
  - 第 82-88 行：`initData` 组合对象（`version:'2.3.9-mock'`）。
  - 第 90-153 行：`handlers` 命令表：
    - `get_config`/`save_config`（第 91-92 行）、`get_init_data`（第 93 行）、`list_accounts`（第 95 行）。
    - `create_account`（第 98-109 行）：不切换激活账号；`save_current_as_account`（第 110-119 行）；`rename_account`（第 120-125 行）；`delete_account`（第 126-130 行）。
    - `get_adapters`（第 131 行）等网络/系统命令、 `do_login`（第 149 行）返回演示消息。
  - 第 155-197 行：挂载 `window.__TAURI_INTERNALS__`（含 `invoke` 分发，第 162-176 行处理 `plugin:event|listen`）、`__TAURI_EVENT_PLUGIN_INTERNALS__`、`__TAURI_PATTERN__`、`__mockEmitEvent`（第 189-195 行）。

#### `android/frontend/public/mock-tauri-addon.js`
- **职责**：叠加在 `mock-tauri.js` 之上的场景注入层（绝不入库），通过 URL 参数驱动。
- **关键实现**：
  - 第 1 行注释：声明 `?op=@telecom&dual=1&night=1&offline=1&theme=dark` 参数协议。
  - 第 4 行：若无 `__TAURI_INTERNALS__` 则警告退出。
  - 第 5-10 行：解析 `op`(默认 `'@telecom'`)、`dual`、`night`、`online`(由 `offline` 反推)、`tab`。
  - 第 12-44 行：劫持 `invoke`，存 `origInvoke`；覆写 `check_campus_status`（第 14-20 行）、`get_adapters`（第 21-27 行，双网卡追加）、`check_portal_status`（第 28-30 行）、`do_login`（第 31 行）、`get_background_status`（第 32 行）、`get_init_data`（第 33-42 行，改 `operator` 与 `nightOperatorRestore`）。
  - 第 46-56 行：`bgStatus()` 构造后台状态（`interval:30000`）。
  - 第 58-69 行：定时发射 `background-check-result` 事件（600ms 首发射，3s 周期）。
  - 第 60-65 行：`tab` 辅助点击 Dock 按钮（`aria-label` 末匹配）。
  - 第 71-76 行：`emuw` 参数注入 CSS 钉宽；第 77-84 行：`probe=1` 诊断写 `<title>`。

#### `android/frontend/tauri-mock.js`
- **职责**：更贴近生产的 Tauri Mock（vite.mock.config.ts 注入，不进构建产物），含动态业务数据。
- **关键实现**：
  - 第 8-15 行：`?nomotion=1` 注入 CSS 禁用动画/过渡。
  - 第 19-26 行：`?panel=` 覆盖启动面板并设 `localStorage`。
  - 第 32-42 行：`config` 对象（`user:'24385214'`、`operator:'@cmcc'`、`deviceAliases:{B025AA6CA580:'实验室台式机'}`、`themeMode` 由 `?theme=` 控）。
  - 第 45-49 行：`mockAccounts`（主/宿舍/实验室三账号）；第 51-56 行：`activeAccountName` 可变，支持 `?active=` 覆盖。
  - 第 59-65 行：`onlineList`（5 会话）、第 67-70 行 `loginHistory`、第 72-76 行 `billRows`/`billSummary`。
  - 第 78 行：`ok(data, message)` 标准响应构造；第 80 行 `sleep(ms)`。
  - 第 87-214 行：`handlers`：
    - `get_init_data`（第 88-97 行）含 `version:'2.4.2-mock'`。
    - `save_config`（第 99-110 行）：`'***'` 掩码保留，发 `config-changed` 事件。
    - `query_self_dashboard`（第 111 行）、`self_offline_session`（第 112-118 行，延时移除）、`query_self_online_log`（第 119 行）、`change_self_password`（第 121 行）。
    - `query_bind_status`（第 126-136 行，`?bound=` 场景）、`bind_operator`（第 138-141 行，`?bindfail=1` 失败）。
    - `check_campus_status`（第 144 行）与 `check_portal_status`（第 145 行）**直接返回平铺结构**，禁 `ok()` 包裹。
    - 账号：`create_account`（第 150-163 行）、`save_current_as_account`（第 165-175 行）、`rename_account`（第 176-181 行）、`switch_account`（第 182-190 行，广播 `config-changed`）、`delete_account`（第 191-196 行）。
  - 第 216-235 行：`invoke` 分发；第 238-247 行：`eventListeners` 与 `emitEvent` 按名路由。
  - 第 249-257 行：挂载 `__TAURI_INTERNALS__`（`transformCallback` 用 `__cb_{id}`）。
  - 第 263-277 行：`?dialog=create|edit|bind` 延时点击入口按钮。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│              Frontend Build Assets                    │
├──────────────────────────────────────────────────────┤
│ Config: .env, package.json, pnpm-*, postcss.config   │
│ Entry: index.html (theme/lang preload + skeleton)    │
└───────────────┬──────────────────────────────────────┘
                ▼
┌──────────────────────────────────────────────────────┐
│ Mock Layer (public/*.js, injected pre-React)         │
│ ┌──────────────┐   ┌──────────────────────────────┐  │
│ │ mock-tauri.js│   │ tauri-mock.js                │  │
│ │ base shim    │   │ dynamic data + Vite mock     │  │
│ └──────┬───────┘   └──────────────┬───────────────┘  │
│        └───────────┬───────────────┘                 │
│            ┌───────▼──────────┐                      │
│            │ mock-tauri-addon │ (URL scenario wrap) │
│            └──────────────────┘                      │
└───────────────┬──────────────────────────────────────┘
                ▼
┌──────────────────────────────────────────────────────┐
│ React App (main.tsx): Zustand + i18next + Radix UI   │
└──────────────────────────────────────────────────────┘
```

## Data Flow

1. **Dependency Resolution**：`pnpm install` 读 `pnpm-lock.yaml`（第 1 行起）与 `pnpm-workspace.yaml`（第 1-2 行），按 `package.json`（第 11-36 行）装包；`build` 脚本（第 8 行）先 `tsc --noEmit` 后 `vite build`。
2. **CSS Processing**：Vite 经 `postcss.config.js`（第 1-6 行）链：`tailwindcss` → `autoprefixer`。
3. **App Init**：`index.html` 加载（第 90 行 `#root`），内联脚本（第 10-26 行、第 104-115 行）从 `localStorage` 设主题/语言；第 116 行载 `main.tsx`。
4. **Mock Invocation**：应用调 `window.__TAURI_INTERNALS__.invoke(cmd,args)`；若存在 `mock-tauri-addon.js` 则劫持（第 12-44 行）改写 `op/dual/night` 场景；否则落 `tauri-mock.js` 或 `mock-tauri.js` 的 `handlers`（如 `switch_account` 第 182-190 行发 `config-changed`）；事件由 `emitEvent`（tauri-mock.js 第 239-247 行）或 `__mockEmitEvent`（mock-tauri.js 第 189-195 行）派发。

## Connections
- [[frontend-core]]：React 应用逻辑、Zustand 状态、Radix UI 组件。
- [[tauri-backend]]：Rust/Kotlin 原生后端，Mock 脚本模拟其 `invoke` 契约。
- [[i18n-config]]：i18next 国际化，index.html 第 104-115 行做首屏文案兜底。

## Known Issues
- **pnpm-workspace 占位**：`android/frontend/pnpm-workspace.yaml` 第 2 行 `esbuild: set this to true or false` 为提示文本，未置确定布尔，需手动改。
- **Peer Dep 严格**：Radix UI 要求 `@types/react`/`@types/react-dom` 锁 19.3.0（见 pnpm-lock.yaml 第 87-92 行），否则 TS 推断失败。
- **Esbuild 多平台**：lock 含大量 `@esbuild/*`（第 205-359 行）原生二进制，CI 需对应架构。
- **Mock 平铺结构陷阱**：`tauri-mock.js` 第 144、145 行 `check_campus_status`/`check_portal_status` 必须直返平铺对象，用 `ok()` 裹会导致前端读 `undefined` 判离线。
- **密码掩码**：`tauri-mock.js` 第 99-110 行 `save_config` 识别 `'***'` 保留后端旧密码。
- **动画禁用**：`tauri-mock.js` 第 8-15 行 `?nomotion=1` 禁动画供 headless 截图，但影响视觉调试。
- **Mock 入库风险**：`mock-tauri.js` 第 2 行、`mock-tauri-addon.js` 第 1 行注明“绝不入库”，`tauri-mock.js` 第 1 行注明“不进入构建产物”，提交时需排除。
