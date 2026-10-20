---
title: build-scripts-and-root
type: module
source_files:
  - .gitignore
  - AGENTS.md
  - LICENSE
  - LICENSE-ASSETS.md
  - README.md
  - clean-build-cache.ps1
  - make-release.ps1
  - tauri-app/build.ps1
  - tauri-app/frontend/about-preview.html
  - tauri-app/frontend/ANIMATION_GUIDE.md
  - tauri-app/frontend/index.html
  - tauri-app/frontend/package.json
  - tauri-app/frontend/postcss.config.js
  - tauri-app/frontend/public/mock-tauri-addon.js
  - tauri-app/frontend/public/mock-tauri.js
  - tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx
tags: [构建, 发布, 根目录, Tauri, 前端结构]
---

## Overview
本模块涵盖项目的根目录基础设施、构建与发布脚本，以及桌面端前端的基础结构。核心职责包括：
1. **项目规范与许可**：通过 `AGENTS.md` 定义 AI 助手与贡献者的行为准则（Changelog 管理、分支纪律、双端同步、验证基线）；通过 `LICENSE` 和 `LICENSE-ASSETS.md` 实施源代码 MIT 授权与美术素材保留版权的双轨制。
2. **构建与发布自动化**：提供 `clean-build-cache.ps1` 清理全平台编译缓存；`tauri-app/build.ps1` 执行桌面端 Tauri 构建；`make-release.ps1` 汇总安装包、校验文件与 APK 资产以准备 GitHub Release。
3. **前端基础架构**：基于 React 19 + Vite 6 的桌面端前端入口，包含骨架屏、主题初始化及用于浏览器 UI 验证的 Mock Tauri 环境。
4. **安全验证逻辑**：在账户面板中实现 Windows Hello 验证门（Hello Gate），遵循“首次操作即验证，通过后共用”的策略，避免频繁触发系统级生物识别弹窗。

## Key Components

### 根目录脚本

#### `.gitignore`
- **职责**：定义版本控制忽略规则，核心策略是隔离构建产物、本地配置及内部文档。
- **关键条目**：
    - `target/`, `dist/`, `release/`：构建输出。
    - `tauri-app/src-tauri/gen/`：Tauri 生成的原生代码。
    - `.env*`：环境配置。
    - `tauri-app/build.ps1`, `clean-build-cache.ps1`：构建脚本本身被忽略（第 67-68 行）。
    - `android/*.apk`, `android/build-apk.ps1`：安卓端产物与硬编码 JDK 路径的脚本。
    - `mock-tauri*.js`：前端浏览器调试用的 Mock 注入脚本，故意不入库（第 92 行）。
    - `config.json`：运行时数据（第 51 行）。

#### `AGENTS.md`
- **职责**：项目级工作约定，面向 AI 助手与贡献者。
- **核心约束**：
    - **Changelog 纪律**：改动必须写入 `changelogs/CHANGELOG_v<版本>.md`（第 7 行），默认版本号 = 当前 + 1；该目录在 `.gitignore` 中，仅本地留存。
    - **双端同步**：通用改进（UI/i18n/IPC/配置）必须在同一次提交内修改桌面 (`tauri-app/`) 和安卓 (`android/`) 两端（第 11 行）。例外：协议核心 crate 通过 Cargo path 依赖自动继承。
    - **验证基线**：
        - 前端：`npx tsc --noEmit --incremental`（禁止 `tsc -b`，防止污染 composite 项目文件，第 14 行）。
        - 后端：`cargo test`（桌面 397 用例，第 15 行）；安卓端需交叉编译 `cargo check --target aarch64-linux-android` 或 `pwsh android/build-apk.ps1`（第 16 行）。
    - **布局实测**：需向 `index.html` 临时注入 `__TAURI_INTERNALS__` mock，测后还原（第 17 行）。

#### `LICENSE` & `LICENSE-ASSETS.md`
- **职责**：定义软件许可。
- **关键点**：
    - 源代码 MIT 许可。
    - 美术素材（看板娘、图标等）**All Rights Reserved**，禁止单独提取、二次创作、商用及 AI 训练（`LICENSE-ASSETS.md` 第 18-23 行）。

#### `README.md`
- **职责**：项目概览、功能列表、技术栈、开发指南。
- **关键信息**：
    - **技术栈**：Tauri 2 + React 19 + TS + Vite 6；后端 Rust + Tokio。
    - **双端架构**：桌面 (`tauri-app/`) + 安卓 (`android/`)。安卓前端为独立复刻树，后端通过 Cargo path 依赖桌面协议核心（第 179 行注释）。
    - **安全机制**：DPAPI (Win) / AndroidKeyStore AES-256-GCM (Android) 加密存储密码；敏感操作需 Windows Hello / BiometricPrompt 验证（600s TTL）。
    - **提权降级链**：管理员直跑 → SYSTEM 计划任务代理 → COM 静默提权 → runas UAC（第 212 行）。

#### `clean-build-cache.ps1`
- **职责**：清理所有可重建的构建缓存与中间产物。
- **关键逻辑**：
    - **清理范围**（第 35-49 行）：
        - 桌面/安卓 `target/` 目录。
        - 安卓 Gradle 构建产物 (`gen/android/app/build`, `.gradle`, `build`)。
        - 前后端 `dist/` 目录。
        - `*.tsbuildinfo` 文件。
        - WebView2 用户数据缓存（`%LOCALAPPDATA%/<identifier>/EBWebView/...`），仅删 Cache/GPU/Dawn 子目录，保留 Local Storage（第 86-94 行）。
    - **安全检查**：
        - 检测运行中的 `cargo/rustc/tauri/campus-login` 进程，存在则退出（第 51-55 行）。
        - **APK 唯一副本告警**：若 `gen/` 下有 APK 但 `release/` 中没有，红色警告并提示需重跑构建（第 113-121 行）。
    - **交互确认**：使用 `ShouldProcess` 高影响操作确认（第 30, 125 行）。

#### `make-release.ps1`
- **职责**：执行完整发布构建并汇总 Release 资产到 `release/` 目录。
- **关键逻辑**：
    - **版本权威源**：从 `tauri.conf.json` 读取 `version`（第 38 行）。
    - **构建流程**：调用 `npx tauri build`（若未 `-SkipBuild`，第 42-54 行）。保持子进程直通终端以显示进度条（第 45-46 行）。
    - **定位安装包**：
        - 优先读取 `.cargo/config.toml` 中的 `target-dir`（第 60-64 行）。
        - 候选目录顺序：`$cargoTarget` > `$env:CARGO_TARGET_DIR` > `root/target` > `src-tauri/target`（第 68 行）。
        - 查找最新修改时间的 `*-setup.exe`（第 72-76 行）。
    - **资产处理**：
        - 重命名为 `Wxxy-CampusLogin_<ver>_<arch>-setup.exe` 并生成 `.sha256`（第 81-93 行）。
        - 收集已构建的 APK（递归查找，排除 node_modules）（第 96-110 行）。
        - 复制 `RELEASE_NOTES_v<ver>.md`（第 112-116 行）。

#### `tauri-app/build.ps1`
- **职责**：桌面端 Tauri 应用的标准化构建脚本。
- **关键逻辑**：
    - **参数**：`-SkipFrontend`, `-SkipCopy`, `-TargetDir`（第 1-5 行）。
    - **环境检查**：验证 `rustc`, `node`, `cargo` 可用性（第 34-54 行）。
    - **依赖安装**：仅在 `node_modules` 不存在时安装（第 59-79 行）。
    - **前端构建**：执行 `npm run build`（第 84-96 行）。
    - **Rust 构建**：
        - 设置 `CARGO_BUILD_JOBS` 为 CPU 核心数（第 101-103 行）。
        - 默认 `CARGO_TARGET_DIR` 指向项目根 `target/`（第 111-113 行），支持自定义 `-TargetDir` 逃生路径过长问题。
        - 直接调用 `tauri.js build` 或 `npx.cmd tauri build`，保持 tty 直通（第 125-134 行）。
    - **产物拷贝**：将 `bundle/nsis` 下的安装包及 `campus-login.exe` 拷贝至 `tauri-release/`（第 156-184 行）。
    - **SHA256 生成**：为所有 `-setup.exe` 生成校验文件（第 189-193 行）。

### 前端基础结构

#### `tauri-app/frontend/about-preview.html`
- **职责**：静态 HTML 文件，用于预览“关于”对话框的 UI 设计稿。
- **内容**：包含左侧栏（Logo/版本/检查更新按钮/GitHub 链接）和右侧栏（状态圆环/特性卡片网格）的 CSS 样式与结构。硬编码版本 `v2.4.1`（第 227 行）。非应用运行时文件，仅供视觉参考。

#### `tauri-app/frontend/ANIMATION_GUIDE.md`
- **职责**：动效系统集成文档，记录新增组件的使用方法与性能优化建议。
- **关键组件**：
    - `AnimatedNumber`：数字平滑滚动 + 弹性缩放 + 颜色高亮（Anime.js）。
    - `ElasticBarChart`：柱状图弹性生长 + 波浪式错开启动（Anime.js）。
    - `StatusPulseBadge`：状态切换脉冲动画（Anime.js）。
    - `AnimatedCard` / `DockNav` / `Button`：Framer Motion 微交互（hover/tap/spring）。
    - `LogPanel`：日志条目入场/出场动画（Framer Motion AnimatePresence）。
    - `FluidBackground`：纯 CSS 背景渐变动画。
- **分工**：Framer Motion 负责基础交互/布局动画；Anime.js 负责复杂序列/物理模拟。

#### `tauri-app/frontend/index.html`
- **职责**：Vite 入口 HTML，负责骨架屏渲染与主题初始化。
- **关键逻辑**：
    - **主题初始化脚本**（第 9-22 行）：读取 `localStorage` 中的 `campus-light-mode` 和 `campus-theme`，设置 `data-light` 属性或添加 `theme-*` class。
    - **CSS 变量**：定义深色/浅色模式下的语义化 CSS 变量（如 `--bg-primary`, `--sk-accent` 等，第 24-69 行）。
    - **骨架屏结构**：提供 `sk-tb` (标题栏), `sk-sb` (状态栏), `sk-side` (侧边栏), `sk-content` (内容区) 占位符，确保 JS 加载前界面不闪烁（第 86-98 行）。
    - **语言兜底**：根据 `app-language` localStorage 项切换骨架屏文案（中文/英文，第 103-110 行）。

#### `tauri-app/frontend/package.json`
- **职责**：前端项目依赖与脚本定义。
- **关键依赖**：React 19, Tauri API v2, Framer Motion 12, GSAP 3, i18next, Zustand, TailwindCSS 3.4。
- **脚本**：`dev`, `build`, `test` (vitest), `clean` (清理 dist/.vite)。

#### `tauri-app/frontend/postcss.config.js`
- **职责**：PostCSS 配置，启用 TailwindCSS 和 Autoprefixer。

#### `tauri-app/frontend/public/mock-tauri-addon.js`
- **职责**：浏览器 UI 截图验证用场景注入脚本，叠加在 `mock-tauri.js` 之上。
- **关键逻辑**：
    - **URL 参数解析**：支持 `?op=@telecom&op2=@unicom&dual=1&night=1&offline=1&theme=dark&tab=network&emuw=504&probe=1` 等参数（第 5-11 行）。
    - **命令覆盖**：
        - `check_campus_status`：强制返回在线状态 `i-wxxy`。
        - `get_adapters`：根据 `dual` 参数返回单/双适配器列表。
        - `check_portal_status` / `do_login`：根据 `offline` 参数模拟在线/离线。
        - `get_init_data`：拦截并修改返回的 config，注入 `operator`, `nightOperatorRestore`, `adapter1/2`, `dualAdapter` 等字段（第 34-46 行）。
        - `get_background_status`：返回模拟后台巡检状态，包括运营商徽标、SSID、校园网状态等。
    - **事件模拟**：定时触发 `background-check-result` 事件（第 70, 77-80 行）。
    - **辅助功能**：
        - `tab` 参数：自动点击对应 Dock 按钮。
        - `emuw` 参数：通过 CSS 强制 body 宽度，模拟窄屏布局。
        - `probe` 参数：诊断超出视口元素，写入 `<title>`。

#### `tauri-app/frontend/public/mock-tauri.js`
- **职责**：浏览器环境下伪造 `__TAURI_INTERNALS__` 的全局 Shim，供前端在 DevTools 中独立测试 UI。
- **关键逻辑**：
    - **初始化**：跳过新手引导，设置默认主题（第 10-19 行）。
    - **Mock 数据**：
        - `mockConfig`：包含所有配置字段默认值（第 38-58 行）。
        - `mockAdapters`：预定义 WLAN/以太网/以太网 2 三张网卡（第 75-79 行）。
        - `mockAccounts`：预定义三个演示账号（第 83-87 行）。
    - **命令处理器** (`handlers` 对象，第 98-164 行)：
        - 实现 `get_config`, `save_config`, `get_init_data`, `list_accounts`, `create_account`, `save_current_as_account`, `rename_account`, `delete_account` 等账号管理命令。
        - 实现 `get_adapters`, `check_campus_status`, `get_background_status`, `do_login`, `do_logout` 等核心业务命令。
        - 未实现命令泛化兜底返回 null 或 warn。
    - **事件系统**：实现 `plugin:event|listen/unlisten` 及 `__mockEmitEvent` 全局函数，支持前端事件监听器回调（第 175-205 行）。

### 测试文件

#### `tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx`
- **职责**：验证 `AccountPanel` 中“Hello 验证门”的行为模式。该门是一个模块级单例，遵循“首次操作即验证，通过后共用”的策略。
- **Mock 策略**：
    - **Tauri API Mock**: 使用 `vi.mock('@/hooks/tauriApi')` 拦截底层 Tauri 调用（第 7-18 行）。
    - **Store Mock**: `useConfigStore` 返回固定状态，`useLogToastStore` 无操作（第 26-38 行）。
    - **UI 组件 Mock**: `AnimatedCard` 简化为 div，`react-i18next` 返回键名（第 40-42 行）。
- **辅助函数**：
    - `renderPanel()`: 异步导入并渲染 `<AccountPanel />`（第 61-75 行）。
    - `openBindDialog(container)`: 查找绑定运营商按钮并点击，注意 Radix Portal 机制，弹窗内容挂载在 `document.body`（第 81-86 行）。
    - `findQueryBtn()`: 从 `document.body` 中查找查询按钮（第 87-88 行）。
    - `beforeEach`: 重置所有 mock 状态，调用 `vi.resetModules()` 强制重新加载模块图，确保 `helloGate` 的单例状态回到初始状态（第 45-54 行）。
- **核心测试用例**：
    - **用例 1：首次查询触发验证，后续复用**：第一次点击触发 `verifyWindowsIdentity`，成功后后续操作不再重复验证（第 91-107 行）。
    - **用例 2：验证通过后，查看明文密码不二次验证**：查询步骤触发验证后，查看明文操作共享同一验证状态（第 109-130 行）。
    - **用例 3：验证失败的处理逻辑**：验证失败不会清除门状态，下次操作仍需重新验证（第 132-147 行）。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     Root Directory                           │
│  ┌─────────────┐  ┌──────────────────┐  ┌────────────────┐  │
│  │ AGENTS.md   │  │ LICENSE /        │  │ .gitignore     │  │
│  │ (工作约定)   │  │ LICENSE-ASSETS   │  │ (忽略规则)      │  │
│  └─────────────┘  │ (双轨许可)        │  └────────────────┘  │
│                   └──────────────────┘                       │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                 Build Scripts                           │  │
│  │  clean-build-cache.ps1  ►  make-release.ps1            │  │
│  │         ▲                      │                        │  │
│  │         │                    ┌─▼──────────┐             │  │
│  │         └────────────────────│ build.ps1   │             │  │
│  │                              │ (Tauri Build)│             │  │
│  └──────────────────────────────┴──────────────┘             │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │              tauri-app/frontend                         │  │
│  │  index.html (Skeleton/Theme)  package.json (Deps)       │  │
│  │  public/                                                    │  │
│  │    ├── mock-tauri.js (API Shim)                          │  │
│  │    └── mock-tauri-addon.js (Scenario Injector)           │  │
│  │  src/                                                     │  │
│  │    ├── account/ (Types, DeviceAliasDialog, HelloGate)    │  │
│  │    └── auth/ (Dashboard, AboutDialog, Types)             │  │
│  └─────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

### 构建与发布流
1. **触发**：开发者运行 `make-release.ps1`。
2. **编译**：脚本设置临时目录修复 MSVC 沙箱问题，调用 `tauri-app/build.ps1`。
3. **产物生成**：
   - `build.ps1` 执行前端 `vite build` 和后端 `tauri build`。
   - 产物存入 `target/release/bundle`。
   - `build.ps1` 将产物拷贝至 `tauri-release/` 并生成 `.sha256`。
4. **资产汇总**：`make-release.ps1` 获取最新版本号，重命名安装包，收集 APK（如有）和 Release Notes，最终放入 `release/` 目录供 `gh release create` 上传。

### 前端初始化流
1. **加载**：浏览器加载 `index.html`。
2. **主题判断**：内联脚本读取 `localStorage` 决定应用深色/浅色模式及自定义主题类。
3. **骨架屏显示**：CSS 渲染骨架屏，JS 根据 `app-language` 显示对应语言的初始化提示。
4. **Mock 注入（仅开发）**：若未连接 Tauri，加载 `mock-tauri.js` 和可选的 `mock-tauri-addon.js` 拦截 `__TAURI_INTERNALS__.invoke`，返回静态数据。
5. **React 挂载**：`main.tsx` 启动应用，初始化 i18n 和 Zustand stores。

### Hello Gate 状态流
1. **模块级单例**：`AccountPanel` 内部维护 `isVerified` 标志。
2. **首次操作**：当用户执行敏感操作（如查询绑定状态、查看明文密码）时，若 `isVerified` 为 false，则先调用 `verifyWindowsIdentity`。
3. **验证成功**：`verifyWindowsIdentity` 返回 success -> `isVerified = true`。后续操作直接执行业务 API，不再触发生物识别。
4. **验证失败**：`verifyWindowsIdentity` 返回失败 -> `isVerified` 保持 false 或重置，下次操作需重新验证。
5. **模块重置**：测试环境中通过 `vi.resetModules()` 强制重新加载模块图，确保 `helloGate` 的单例状态回到初始的 `firstFree` 状态，保证用例隔离。

## Connections
- [[decisions/version-single-source-build-rs]]：版本号单一来源与构建流程关联。
- [[decisions/release-asset-integrity]]：Release 资产完整性校验策略（SHA256）。
- [[concepts/dual-platform-sharing]]：双端共享协议核心的架构决策。
- [[learnings/android-host-cargo-check-fails]]：安卓端主机 cargo check 失败的应对方案。

## Known Issues
- **Radix Portal 定位陷阱**：绑定表单（弹窗 B）使用 Radix Dialog/Portal，其 DOM 节点脱离 React 树，直接挂载在 `document.body`。测试中不能直接在 `container`（React 根容器）中查找弹窗内的按钮，必须使用 `document.body.querySelectorAll`。这是前端测试中常见的陷阱，源码注释明确指出了这一点。
- **凭据来源变更**：注释指出：“凭据统一后绑定卡不再收集学号/自助服务密码...查询按钮直接可用”。这意味着 UI 逻辑简化，不再需要额外的输入框来收集自助服务密码，而是直接从 Store 获取信息。
- **模块重置的重要性**：每个测试用例前必须调用 `vi.resetModules()`。如果不重置，第一个测试用例修改的门状态可能会污染后续用例，导致它们误以为已通过验证。
- **验证失败不阻断门的存在，只阻断操作**：当 `verifyWindowsIdentity` 返回失败时，测试期望业务 API 调用次数为 0。同时，门的验证状态并未变为“成功”，因此下一次尝试仍需重新进行生物识别。这体现了最小权限原则。
- **明文显示的实现**：明文密码直接渲染在按钮附近的 DOM 文本中。测试通过 `document.body.textContent` 断言其存在，而非检查特定的 React 状态或组件属性。