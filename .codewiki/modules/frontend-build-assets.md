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
本模块负责 Android Tauri 应用前端构建环境的配置、依赖锁定及开发阶段的原生 API Mock 模拟。核心职责包括定义构建常量（`.env`）、管理项目元数据与依赖版本（`package.json`, `pnpm-lock.yaml`）、配置 CSS 预处理管线（`postcss.config.js`），以及在开发环境中通过多层 Mock 脚本模拟 Tauri 原生能力（`mock-tauri*.js`, `tauri-mock.js`）。该模块确保了前端代码在无真实原生后端或需要特定测试场景（如夜切态、离线状态）时能够独立运行和调试。

## Key Components

### Configuration Files

#### `android/frontend/.env`
- **职责**：定义构建环境常量，供 Vite 注入全局变量。
- **关键实现**：
    - 第 1 行：`VITE_PLATFORM=android`，标识当前运行平台为 Android，对应全局变量 `import.meta.env.VITE_PLATFORM`。

#### `android/frontend/package.json`
- **职责**：项目元数据、依赖管理与构建脚本配置。
- **关键实现**：
    - 元数据：
        - 第 2 行：`name`: "campus-login-android-frontend"。
        - 第 4 行：`version`: "2.4.1"。
        - 第 5 行：`type`: "module" (启用 ES Modules)。
    - Scripts：
        - 第 7 行：`dev`: `"vite --host"`，启动开发服务器并允许外部访问。
        - 第 8 行：`build`: `"tsc --noEmit && vite build"`，先进行 TypeScript 类型检查（不输出文件），再执行 Vite 生产构建。
        - 第 9 行：`typecheck`: `"tsc --noEmit"`，仅执行类型检查。
    - Dependencies：
        - Tauri 生态：`@tauri-apps/api` (^2), `@tauri-apps/plugin-biometric` (~2), `@tauri-apps/plugin-notification` (^2.4.0), `@tauri-apps/plugin-opener` (^2)。
        - UI/状态：`react` (^19.0.0), `react-dom` (^19.0.0), `zustand` (^5.0.13), `@radix-ui/*` 系列。
        - 动画/交互：`framer-motion` (^12.38.0), `gsap` (^3.15.0)。
        - 国际化：`i18next` (^26.3.1), `react-i18next` (^17.0.8), `i18next-browser-languagedetector` (^8.2.1)。
        - AI/视觉：`@vladmandic/human` (^3.3.6)。
    - DevDependencies：
        - `typescript` (^5.5.0), `vite` (^6.0.0), `@vitejs/plugin-react` (^4.3.0), `tailwindcss` (^3.4.10) 及其插件。
    - 安全配置：
        - 第 48-50 行：`allowScripts.esbuild@0.25.12: true`，允许 esbuild 在构建过程中执行脚本。

#### `android/frontend/pnpm-lock.yaml`
- **职责**：pnpm 依赖锁定文件，确立前端构建环境的确定性快照。
- **关键实现**：
    - 锁定 `react@19.3.0` 和 `react-dom@19.3.0`。
    - 引入 `zustand@5.0.15`，配合 `use-sync-external-store@1.7.0`。
    - UI 组件库：`@radix-ui/*` (v1.x), `tailwindcss@3.4.19`, `lucide-react@0.446.0`。
    - 动画：`framer-motion@12.43.0`, `gsap@3.15.0`。
    - 国际化：`i18next@26.4.2`, `react-i18next@17.0.15`。
    - Tauri 集成：`@tauri-apps/api@2.12.3` 及插件 (`plugin-biometric@2.4.2`, `plugin-notification@2.6.0`, `plugin-opener@2.7.1`)。
    - AI/ML：`@vladmandic/human@3.3.6`。
    - 构建工具链：`vite@6.4.4`, `esbuild@0.25.12`, `@vitejs/plugin-react@4.7.0`, `typescript@5.9.3`。

#### `android/frontend/pnpm-workspace.yaml`
- **职责**：定义 pnpm workspace 级别的全局设置。
- **关键实现**：
    - 第 1-2 行：`allowBuilds.esbuild`，控制是否允许 esbuild 的原生构建步骤，防止 CI/CD 因缺少系统依赖失败。

#### `android/frontend/postcss.config.js`
- **职责**：配置 PostCSS 插件链，服务于 Tailwind CSS。
- **关键实现**：
    - 导出 ES Module 格式配置对象。
    - 插件顺序：`tailwindcss` (核心引擎) -> `autoprefixer` (浏览器前缀处理)。

### HTML Entry Point

#### `android/frontend/index.html`
- **职责**：应用入口 HTML，初始化 CSS 变量、骨架屏、主题/语言预加载及 React 挂载点。
- **关键实现**：
    - **CSS 变量系统** (`<style>` 块)：
        - 深色模式默认值 (`:root`)：`--bg-primary: #0f172a`。
        - 浅色模式覆盖值 (`html[data-light]`)：`--bg-primary: #ffffff` (第 49 行)。
        - 重置样式：`* { margin: 0; padding: 0; box-sizing: border-box; }` (第 70 行)。
        - 布局约束：`html, body, #root` 高度宽度 100%，`overflow: hidden` (第 71 行)。
        - 骨架屏类名：`.sk-tb`, `.sk-sb`, `.sk-main`, `.sk-side`, `.sk-content`, `.sk-card`, `.sk-log` 等 (第 72-82 行)。
    - **主题初始化脚本** (第 10-22 行)：
        - 读取 `localStorage.getItem('campus-light-mode')`，若为 `'1'` 则设置 `data-light='1'`，否则添加 `dark` class (第 16-18 行)。
        - 读取 `localStorage.getItem('campus-theme')`，若非 `'default'` 则添加 `theme-{th}` class (第 21 行)。
        - 包含 `try...catch` 防御性编程。
    - **骨架屏文案国际化兜底** (第 103-110 行)：
        - 检查 `localStorage.getItem('app-language')`，若为 `'en'` 则将 ID 为 `sk-init-text` 的元素文本改为 `'Initializing...'` (第 107 行)。
    - **React 挂载** (第 112 行)：
        - `<script type="module" src="/src/main.tsx"></script>`。

### Mock Scripts

#### `android/frontend/public/mock-tauri.js`
- **职责**：基础 Tauri API Shim，提供 `__TAURI_INTERNALS__` 的全局模拟环境。
- **关键实现**：
    - **环境初始化 (L13-22)**：跳过新手引导 (`campus-onboarding-done`)，处理主题 (`theme=dark`) 和通知弹窗状态。
    - **回调转换 (L26-38)**：`transformCallback` 为 JS 回调分配唯一 ID (`_cb{id}`)，支持一次性 (`once`) 或持久监听。
    - **Mock 数据定义 (L40-98)**：
        - `mockConfig` (L40)：默认配置，含 `operator: 'cmcc'`, `themeMode: 'dark'`, `requiredNetworkName: 'i-wxxy'` 等。
        - `campusStatus` / `backgroundStatus`：静态状态对象。
        - `mockAccounts` (L61)：预设三个账号 (`mock-001` ~ `mock-003`)。
        - `initData` (L69)：组合上述数据作为 `get_init_data` 返回值。
    - **命令处理器 (handlers) (L76-130)**：
        - **账号 CRUD**：`create_account` (L86), `save_current_as_account` (L97), `rename_account` (L105), `delete_account` (L111)。注意 `create_account` 不切换激活账号。
        - **网络/系统**：`get_adapters`, `check_campus_status`, `get_soc_info`, `get_logs` 等返回静态或空数据。
        - **通用兜底 (L126)**：未匹配命令打印警告并返回 `null`。
    - **全局挂载 (L132-148)**：
        - `window.__TAURI_INTERNALS__.invoke`：分发命令，处理 `plugin:event|listen/unlisten`。
        - `window.__TAURI_EVENT_PLUGIN_INTERNALS__`：模拟 `_unlisten` 行为。
        - `window.__mockEmitEvent`：手动触发事件广播。

#### `android/frontend/public/mock-tauri-addon.js`
- **职责**：基于 `mock-tauri.js` 的增强层，通过 URL 参数注入特定场景。
- **关键实现**：
    - **前置检查 (L4)**：检测 `window.__TAURI_INTERNALS__` 是否存在，若不存在则警告并退出。
    - **URL 解析 (L5-9)**：解析 `op` (运营商后缀, 默认 `'@telecom'`), `dual` (双网卡模式), `night` (夜切态), `online` (在线状态), `tab` (Dock页签)。
    - **invoke 劫持 (L12-38)**：
        - 保存原始 `origInvoke`。
        - `check_campus_status` (L13)：硬编码返回校园网连接状态 (`onCampusNetwork: true`, `currentSsid: 'i-wxxy'`)。
        - `get_adapters` (L18)：返回 WLAN 适配器；若 `dual=1` 追加以太网适配器。
        - `check_portal_status` (L26)：根据 `online` 变量返回在线/离线状态。
        - `do_login` (L30)：返回成功但提示“演示模式不执行登录”。
        - `get_background_status` (L31)：调用内部 `bgStatus()`。
        - `get_init_data` (L32)：**关键逻辑**。调用 `origInvoke` 获取真实初始化数据后，修改 `data.config.operator`（夜切态置空）及 `data.config.nightOperatorRestore`（设置恢复目标）。
    - **后台状态生成 (L36-47)**：`bgStatus()` 函数构建后台检查状态对象，包含 `interval: 30000` 等字段。
    - **定时事件发射 (L49-54)**：
        - 600ms 后首次发射 `background-check-result`。
        - 3s 间隔循环发射，递增 `checkCount`。
    - **UI 辅助功能 (L55-70)**：
        - `tab` 辅助 (L56-62)：查找对应 `aria-label` 的按钮并触发点击。
        - `emuw` 辅助 (L64-67)：注入 CSS 强制 `body` 宽度，模拟窄屏布局。
        - `probe` 诊断 (L68-74)：查找超出视口元素，将信息写入 `<title>` 供无头浏览器读取。

#### `android/frontend/tauri-mock.js`
- **职责**：更贴近生产环境的 Tauri Mock，包含动态业务数据及 Vite 开发服务器专用配置。
- **关键实现**：
    - **环境控制 (L13-26)**：
        - `?nomotion=1`：注入 CSS 禁用所有动画/过渡，用于 headless 截图一致性。
        - `?panel=`：覆盖启动面板，跳过引导。
    - **动态状态管理 (L30-66)**：
        - `config`：含 `deviceAliases` (MAC 到别名映射)。
        - `mockAccounts`：预设主/宿舍/实验室账号。
        - `activeAccountName`：可变的当前激活账号 ID，支持 `?active=<id>` URL 覆盖。
        - `onlineList` / `loginHistory` / `billRows`：动态业务数据，支持注销操作移除条目。
    - **命令分发 (handlers) (L78-165)**：
        - **异步模拟**：`self_offline_session` (L90), `change_self_password` (L97) 使用 `sleep(800)` 模拟网络延迟。
        - **账号交互**：
            - `switch_account` (L130)：更新 `activeAccountName` 和 `config`，并广播 `config-changed` 事件。
            - `save_current_as_account` (L120)：同步更新账户档案并广播 `config-changed`。
        - **特殊结构返回**：
            - `check_campus_status` (L108) & `check_portal_status` (L112)：**严禁使用 `ok()` 包裹**，直接返回平铺结构体，避免前端判读 `undefined`。
            - `query_self_dashboard` / `query_self_online_log`：使用 `ok(data)` 包裹返回标准响应格式。
    - **事件系统 (L170-188)**：
        - `eventListeners` 字典存储事件名到回调 ID 列表的映射。
        - `emitEvent` 按名路由派发 payload。
        - `invoke` 中处理 `plugin:event|listen` 注册回调。
    - **截图辅助 (L195-207)**：`?dialog=create|edit|bind` 延时自动点击对应 UI 按钮。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   Frontend Build Assets                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │ Config Layer │   │ Dependency   │   │ CSS Pipeline   │  │
│  │              │   │ Management   │   │                │  │
│  │ .env         │   │              │   │ postcss.config │  │
│  │ index.html   │   │ package.json │   │ + tailwindcss  │  │
│  │              │   │ pnpm-lock    │   │ + autoprefixer │  │
│  └──────┬───────┘   │ pnpm-work    │   └───────┬────────┘  │
│         │           └──────┬───────┘           │          │
│         ▼                  ▼                    ▼          │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 Application Startup                  │  │
│  │                                                      │  │
│  │  index.html loads scripts                          │  │
│  │       │                                              │  │
│  │       ▼                                              │  │
│  │  ┌──────────────────────────────────────────────┐    │  │
│  │  │           Mock Environment Layer             │    │  │
│  │  │                                              │    │  │
│  │  │  1. tauri-mock.js / mock-tauri.js            │    │  │
│  │  │     ├── Defines __TAURI_INTERNALS__          │    │  │
│  │  │     ├── Sets up base handlers (accounts,     │    │  │
│  │  │     │          adapters, status)             │    │  │
│  │  │     └── Manages event listeners              │    │  │
│  │  │                                              │    │  │
│  │  │  2. mock-tauri-addon.js (Optional Overlay)   │    │  │
│  │  │     ├── Wraps origInvoke                     │    │  │
│  │  │     ├── Overrides specific cmds (login,      │    │  │
│  │  │     │          campus status)                │    │  │
│  │  │     └── Injects URL-based scenarios          │    │  │
│  │  └──────────────────────────────────────────────┘    │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 React App (main.tsx)                 │  │
│  │  Uses Zustand for state, i18next for lang,           │  │
│  │  Radix UI for components, Tailwind for styles.       │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Dependency Resolution**:
    - `pnpm install` reads `pnpm-lock.yaml` and `pnpm-workspace.yaml` to install dependencies into `node_modules`.
    - `package.json` defines the versions and scripts used for building (`vite build`) and development (`vite`).

2.  **CSS Processing**:
    - Vite processes CSS files in the source code.
    - `postcss.config.js` applies `tailwindcss` to generate utility classes, followed by `autoprefixer` to add browser-specific prefixes.

3.  **Application Initialization**:
    - `index.html` is loaded.
    - Inline scripts read from `localStorage` to set initial theme and language states.
    - Skeleton screen classes are applied via CSS.
    - `main.tsx` is loaded as an ES module.

4.  **Mock API Invocation**:
    - The application calls `window.__TAURI_INTERNALS__.invoke(cmd, args)`.
    - If `mock-tauri-addon.js` is present, it intercepts the call:
        - Parses URL parameters (`op`, `dual`, `night`, etc.).
        - Checks if the command matches overridden ones (`check_campus_status`, `get_init_data`, etc.).
        - If matched, returns modified Mock data or calls `origInvoke` with modifications.
        - If not matched, delegates to the underlying shim.
    - If `mock-tauri-addon.js` is not present, the call goes directly to `tauri-mock.js` or `mock-tauri.js`:
        - Looks up the command in the `handlers` object.
        - Executes the handler logic (e.g., updating `mockAccounts`, returning static data).
        - For commands like `switch_account`, emits events via `emitEvent`.
    - Events are dispatched to registered listeners stored in `eventListeners`.

## Connections

- [[frontend-core]]: React application logic, state management (Zustand), and UI components.
- [[tauri-backend]]: Native Android backend implemented in Rust/Kotlin, which the Mock scripts simulate.
- [[i18n-config]]: Internationalization setup using `i18next`.

## Known Issues

- **Peer Dependencies**: Radix UI components require `@types/react` and `@types/react-dom` to strictly match version `19.3.0`; otherwise, TypeScript type inference may fail.
- **Esbuild Multi-platform**: The lock file includes many `@esbuild/*` native binaries for different platforms. Ensure CI/CD environments have correct architecture support to avoid build failures.
- **Zustand Version**: Zustand v5 is used for React 19 compatibility. Older versions may cause warnings or performance issues.
- **Mock Data Structure Trap**: In `tauri-mock.js`, `check_campus_status` and `check_portal_status` must return flat objects directly. Wrapping them with `ok()` will cause the frontend to read `res.data.onCampusNetwork` as `undefined`, leading to incorrect offline detection.
- **Password Masking**: `tauri-mock.js` recognizes `'***'` in password fields during `save_config` to preserve existing passwords, preventing accidental clearing.
- **Animation Disabling**: The `nomotion` parameter in `tauri-mock.js` disables animations for consistent headless screenshots but may affect visual debugging.