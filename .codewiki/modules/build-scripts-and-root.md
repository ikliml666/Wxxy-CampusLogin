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

## Overview

本模块涵盖项目根目录基础设施、构建与发布脚本，以及桌面端前端基础结构。核心职责：

1. **项目规范与许可**：`AGENTS.md`（第 1-27 行）定义 AI 助手与贡献者行为准则；`LICENSE`（第 1-27 行）与 `LICENSE-ASSETS.md`（第 1-30 行）实施源代码 MIT + 美术素材保留版权双轨制。
2. **构建与发布自动化**：`clean-build-cache.ps1`（第 1-208 行）清理全平台编译缓存；`tauri-app/build.ps1`（第 1-212 行）执行桌面端 Tauri 构建；`make-release.ps1`（第 1-120 行）汇总安装包、校验文件与 APK 资产至 `release/`。
3. **前端基础架构**：基于 React 19 + Vite 6 的桌面端前端入口（`tauri-app/frontend/index.html` 第 1-118 行），含骨架屏、主题初始化及用于浏览器 UI 验证的 Mock Tauri 环境（`mock-tauri.js` 第 1-207 行、`mock-tauri-addon.js` 第 1-97 行）；当前版本号为 2.4.2（见 `README.md` 第 5 行、`package.json` 第 3 行、`about-preview.html` 第 227/251 行）。
4. **安全验证逻辑**：账户面板中实现 Windows Hello 验证门（Hello Gate），对应测试 `AccountPanel.helloGate.test.tsx`（第 1-150 行）验证“首次操作即验证，通过后共用”策略。

## Key Components

### 根目录脚本

#### `.gitignore`

- **职责**：定义版本控制忽略规则，隔离构建产物、本地配置及内部文档（第 1-112 行）。
- **关键条目**：
  - 构建输出：`target/`（第 5 行）、`dist/`（第 6 行）、`release/`（第 7 行）、`output/`（第 8 行）、`app-output/`（第 9 行）、`app-v/`（第 10 行）、`tauri-release/`（第 11 行）、`installer-build/`（第 14 行）、`release-build/`（第 15 行）、`archive/`（第 16 行）。
  - Tauri 生成：`tauri-app/src-tauri/gen/`（第 19 行）。
  - 环境配置：`.env`（第 22 行）、`.env.test`（第 23 行）。
  - 构建脚本本身被忽略：`tauri-app/build.ps1`（第 67 行）、`clean-build-cache.ps1`（第 68 行）。
  - 发布资产汇总脚本本地保留不入库：`make-release.ps1`（第 12-13 行注释与忽略项，第 13 行显式忽略）。
  - 安卓端：`android/*.apk`（第 85 行）、`android/*.apk.idsig`（第 86 行）、`android/build-apk.ps1`（第 89 行，硬编码本地 JDK 路径）。
  - 浏览器实测 mock：`mock-tauri*.js`（第 92 行，故意不入库）、`.zcode/`（第 93 行）。
  - 运行时数据：`config.json`（第 51 行）。
  - 内部文档：`docs/`（第 63 行）、`CODE_WIKI.md`（第 77 行）、`changelogs/`（第 59 行）、`CHANGELOG.md`（第 58 行）。
  - 前端增量编译缓存：`*.tsbuildinfo`（第 80 行）。

#### `AGENTS.md`

- **职责**：项目级工作约定，面向 AI 编码助手（ZCode 等）与贡献者（第 1-27 行）。
- **核心约束**：
  - **Changelog 纪律**：改动必须写入 `changelogs/CHANGELOG_v<版本>.md`（第 7 行），版本号权威源为 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 字段；目标文件 = 当前版本号 + 1；`changelogs/` 在 `.gitignore` 中仅本地留存。
  - **分支纪律**：改动全在主题分支，不直接改 main；合并放开但 push/删分支仍禁止（第 9 行，2026-10-12 修订）。
  - **双端同步**：通用改进必须在同一次提交内修改桌面（`tauri-app/`）和安卓（`android/`）两端（第 11 行）；协议实现单点存在于桌面 crate，安卓经 Cargo path 依赖自动继承。
  - **验证基线**：
    - 前端：`npx tsc --noEmit --incremental`（在 `tauri-app/frontend` 下；禁止 `tsc -b`，第 14 行）。
    - 后端：`cargo test`（在 `tauri-app/src-tauri` 下；全绿基线 397 用例（lib 396 + 集成 1，2026-09-22 实测），第 15 行）。
    - 安卓端：host `cargo check` 基线即失败，只认 `cargo check --target aarch64-linux-android --all-targets` 或 `pwsh android/build-apk.ps1`（第 16 行）。
    - 布局实测：向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock，测后还原（第 17 行）。
  - **文档同步**：新模块/决策/踩坑同步进 `.codewiki/`，收尾跑 `cw index` 与 `cw meta update`（第 19 行）。
  - **语言**：思考、回复、commit、文档一律中文（第 21 行）。
  - **项目速览**（第 23-27 行）：技术栈 Tauri 2 + React 19 + TypeScript，双端同构校园网助手。

#### `LICENSE` & `LICENSE-ASSETS.md`

- **职责**：定义软件许可（第 1-27 行 / 第 1-30 行）。
- **关键点**：
  - 源代码 MIT 许可（第 1-21 行），例外：美术素材不在 MIT 授予范围内（第 23-27 行）。
  - 美术素材（看板娘插画、应用图标等，清单见 `LICENSE-ASSETS.md` 第 6-10 行）**All Rights Reserved**，禁止单独提取、复制、再分发、改编、二次创作、商用、训练 ML/AI（第 18-23 行）。

#### `README.md`

- **职责**：项目概览、功能列表、技术栈、开发指南（第 1-226 行）。
- **关键信息**：
  - 版本徽章 `version-2.4.2`（第 5 行）。
  - **技术栈**：Tauri 2 + React 19 + TypeScript + Vite 6；TailwindCSS 3.4 + Framer Motion 12 + GSAP 3；Rust + Tokio（第 70-79 行表）。
  - **双端架构**：桌面（`tauri-app/`，第 86-118 行）+ 安卓（`android/`，第 119-131 行）；安卓前端为独立复刻树，后端 Cargo path 依赖桌面协议核心（第 179 行注释）。
  - **开发/构建**：
    - 安装依赖：`cd tauri-app/frontend && npm install` 与 `cd .. && npm install`（第 143-149 行）。
    - 开发模式：`cd tauri-app && npx tauri dev`（第 154-156 行）。
    - 构建发布：`pwsh tauri-app/build.ps1`（第 162 行）。
    - 安卓构建：`npx @tauri-apps/cli android build --target aarch64 --apk`（第 176 行），前端须先 `npm run build`（第 175 行）。
  - **测试基线**（与 AGENTS.md 第 15 行存在差异，须分别记录）：
    - 后端测试（432 个单元测试 + 4 个集成测试）`cargo test`（第 186-188 行）。
    - 前端测试（103 个测试）`npm test`（第 193-195 行）。
    - 前端类型检查 `npx tsc --noEmit --incremental`（第 198 行）。
  - **安全机制**：DPAPI (Win) / AndroidKeyStore AES-256-GCM (Android) 加密；敏感操作需 Windows Hello / BiometricPrompt 验证（600s TTL，第 204、208 行）；提权四级降级链（第 212 行）。

#### `clean-build-cache.ps1`

- **职责**：清理所有可重建的构建缓存与中间产物（第 1-208 行）。
- **关键逻辑**：
  - **清理范围** `$targets`（第 84-95 行）：
    - `<项目根>\target`（第 85 行）
    - `C:\cl-build\target` 旧短路径遗留（第 86 行）
    - `tauri-app\src-tauri\target`（第 87 行）
    - `android\src-tauri\target`（第 89 行）
    - `android\src-tauri\gen\android\app\build`（第 90 行，含 APK）
    - `android\src-tauri\gen\android\.gradle`（第 91 行）
    - `android\src-tauri\gen\android\build`（第 92 行）
    - `tauri-app\frontend\dist`（第 93 行）
    - `android\frontend\dist`（第 94 行）
    - 遗留 cargo target 同级目录（通过 `.fingerprint` 判定归属，第 73-82 行）
  - **glob 目标** `$globTargets`（第 97-100 行）：`tauri-app\frontend\*.tsbuildinfo`、`android\frontend\*.tsbuildinfo`。
  - **进程检测**：`$busy` 检测 cargo/rustc/tauri/campus-login/校园网登录助手 进程，存在则退出（第 102-106 行）。
  - **WebView2 缓存**：从 `tauri-app\src-tauri\tauri.conf.json` 动态读 `identifier`（第 132-134 行），删 `%LOCALAPPDATA%\<identifier>\EBWebView` 下 Cache/Code Cache/GPUCache/DawnGraphiteCache/DawnWebGPUCache/Service Worker/GrShaderCache/ShaderCache 子目录（第 137-155 行），保留用户数据。
  - **APK 唯一副本告警**：`$genApks` 与 `$releaseFiles` 比对，若 `gen/` 下有 `release/` 无则红色警告（第 168-175 行）。
  - **NSIS 安装包唯一副本告警**：按版本号+架构比对 `release\`（第 180-195 行）。
  - **交互确认**：`[CmdletBinding(SupportsShouldProcess, ConfirmImpact="High")]`（第 40 行），`ShouldProcess` 删除前确认（第 199-202 行）；支持 `-WhatIf`（第 36 行）、`-Confirm:$false`（第 38 行）。

#### `make-release.ps1`

- **职责**：执行完整发布构建并汇总 Release 资产到 `release/` 目录（第 1-120 行）。
- **关键逻辑**：
  - **参数**：`[switch]$SkipBuild`（第 15-17 行）。
  - **编码**：强制 UTF-8 输出避免 ✓ 乱码（第 21-23 行）。
  - **版本权威源**：从 `tauri-app\src-tauri\tauri.conf.json` 读 `version`（第 36-40 行）。
  - **构建流程**：若未 `-SkipBuild` 则 `Push-Location $tauriAppDir` 后 `& npx.cmd tauri build` 直通终端（第 42-54 行，具体调用第 48 行）。
  - **定位 cargo 产物**：读 `.cargo\config.toml` 中 `target-dir`（第 59-64 行）；候选目录顺序：`$cargoTarget` > `$env:CARGO_TARGET_DIR` > `root/target` > `src-tauri/target`（第 68-77 行），取最新 `*-setup.exe`（第 72-76 行）。
  - **资产处理**：
    - 架构提取 `$arch`（第 80 行），重命名为 `Wxxy-CampusLogin_${version}_${arch}-setup.exe`（第 81 行）。
    - 生成 `.sha256` 文件（同名，第 90-93 行）。
    - 收集 APK：递归找 `android/` 下 `Wxxy-CampusLogin_*.apk` 排除 `node_modules`（第 95-110 行）。
    - 复制 `RELEASE_NOTES_v<version>.md`（第 112-116 行）。

#### `tauri-app/build.ps1`

- **职责**：桌面端 Tauri 应用标准化构建脚本（第 1-212 行）。
- **关键逻辑**：
  - **参数**：`[switch]$SkipFrontend`、`[switch]$SkipCopy`、`[string]$TargetDir`（第 1-5 行）。
  - **路径定义**：`$releaseOutputDir = <root>\tauri-release`（第 20 行）。
  - **环境检查**：验证 `rustc`（第 34-40 行）、`node`（第 42-48 行）、`cargo`（第 50-54 行）可用性。
  - **依赖安装**：仅当 `node_modules` 不存在时 `npm install`（前端第 60-67 行、根第 69-76 行）。
  - **前端构建**：`npm run build`（第 84-96 行）；`--SkipFrontend` 时若 `dist\index.html` 不存在则报错退出（第 90-95 行）。
  - **Rust 构建**：
    - `CARGO_BUILD_JOBS = CPU 核心数`（第 101-103 行）。
    - 默认 `CARGO_TARGET_DIR = <root>\target`（第 111-113 行，2026-09-15 起废弃 `C:\cl-build`）；`-TargetDir` 可逃生路径过长（第 105-114 行）。
    - 直通调用 `node $tauriCliJs build` 或 `npx.cmd tauri build`（第 119-134 行，具体第 127 / 129 行），保留 tty 进度条。
  - **产物拷贝**：`[5/5]` 将 `bundle/` 递归拷至 `tauri-release/`（第 154-178 行），并拷 `campus-login.exe`（第 180-184 行）。
  - **SHA256 生成**：为所有 `*-setup.exe` 生成校验文件（第 189-193 行）。

### 前端基础结构

#### `tauri-app/frontend/about-preview.html`

- **职责**：静态 HTML 预览“关于”对话框 UI 设计稿（第 1-277 行）。
- **内容**：左侧栏（Logo/版本/检查更新按钮/GitHub 链接，第 223-238 行），右侧栏（状态圆环/特性卡片网格，第 241-271 行）。硬编码版本 `v2.4.2`（第 227、251 行）。非应用运行时文件，仅供视觉参考（CSS 内联，无 JS 框架依赖）。

#### `tauri-app/frontend/ANIMATION_GUIDE.md`

- **职责**：动效系统集成文档（第 1-293 行）。
- **关键组件**：
  - `AnimatedNumber`（`src/components/shared/AnimatedNumber.tsx`，第 8-26 行）：数字平滑滚动 + 弹性缩放 + 颜色高亮。
  - `ElasticBarChart`（第 30-53 行）：柱状图弹性生长 + 波浪错开。
  - `StatusPulseBadge`（第 57-82 行）：状态切换双次脉冲，级别 `excellent/good/moderate/poor/bad`。
  - `AnimatedCard`（`src/components/ui/animated-card.tsx`，第 88-104 行）：Framer Motion whileHover/whileTap/入场。
  - `DockNav`（第 106-115 行）：图标磁性微交互。
  - `Button`（第 117-125 行）：径向渐变光效。
  - `LogPanel`（第 127-135 行）：AnimatePresence 日志入场。
  - `FluidBackground`（第 140-147 行）：纯 CSS 背景渐变。
- **分工**：Framer Motion 负责基础交互/布局（第 245-248 行表）；Anime.js 负责复杂序列。

#### `tauri-app/frontend/index.html`

- **职责**：Vite 入口 HTML，骨架屏与主题初始化（第 1-118 行）。
- **关键逻辑**：
  - **主题初始化脚本**（第 9-26 行）：读 `localStorage` 的 `campus-light-mode`（置 `data-light`，第 12-16 行）、`campus-theme`（加 `theme-*` class，第 18-19 行）、`campus-glass-level`（置 `data-glass`，第 21-23 行）。
  - **CSS 变量**：`:root` 深色变量（第 28-51 行），`html[data-light]` 浅色变量（第 52-73 行）。
  - **骨架屏结构**（第 74-103 行）：`sk-tb`/`sk-sb`/`sk-main`/`sk-side`/`sk-content`/`sk-log` 占位，确保 JS 加载前不闪烁。
  - **语言兜底**（第 104-115 行）：按 `app-language === 'en'` 切换 `#sk-init-text` 为 `Initializing...`。
  - **入口**：`<script type="module" src="/src/main.tsx">`（第 116 行）。

#### `tauri-app/frontend/package.json`

- **职责**：前端依赖与脚本（第 1-51 行）。
- **版本**：`2.4.2`（第 3 行）。
- **脚本**：`dev` (vite)、`build` (vite build)、`preview`、`test` (vitest run)、`test:watch`、`clean`（清 dist/node_modules/.vite，第 12 行）。
- **关键依赖**：react ^19、@tauri-apps/api ^2、framer-motion ^12.38.0、gsap ^3.15.0、i18next ^26.3.1、react-i18next ^17.0.8、zustand ^5.0.13（第 14-36 行）。
- **devDependencies**：vite ^6.0.0、vitest ^4.1.10、tailwindcss ^3.4.10、typescript ^5.5.0（第 37-50 行）。

#### `tauri-app/frontend/postcss.config.js`

- **职责**：PostCSS 配置（第 1-6 行），启用 `tailwindcss` 与 `autoprefixer` 插件。

#### `tauri-app/frontend/public/mock-tauri-addon.js`

- **职责**：浏览器 UI 截图验证场景注入，叠加在 `mock-tauri.js` 之上（第 1-97 行，绝不入库，见 `.gitignore` 第 92 行）。
- **关键逻辑**：
  - **URL 参数**：`?op=@telecom&op2=@unicom&dual=1&night=1&offline=1&theme=dark&tab=network&emuw=504&probe=1`（第 2、5-11 行）。
  - **命令覆盖**（`window.__TAURI_INTERNALS__.invoke` 重写，第 14-48 行）：
    - `check_campus_status`：强制 `i-wxxy` 在线（第 15-21 行）。
    - `get_adapters`：按 `dual` 返回 WLAN / 以太网列表（第 22-28 行）。
    - `check_portal_status`：按 `offline` 模拟（第 29-31 行）。
    - `do_login`：演示不执行（第 32 行）。
    - `get_init_data`：注入 `operator`/`nightOperatorRestore`/`adapter1/2`/`dualAdapter`（第 34-46 行）。
  - **事件模拟**：`setTimeout`/`setInterval` 触发 `background-check-result`（第 70、77-80 行）。
  - **辅助**：`tab` 自动点击 Dock（第 71-76 行）、`emuw` CSS 钉宽（第 82-87 行）、`probe` 诊断超出视口写 `title`（第 88-95 行）。

#### `tauri-app/frontend/public/mock-tauri.js`

- **职责**：浏览器环境伪造 `__TAURI_INTERNALS__` 全局 Shim（第 1-207 行，绝不入库）。
- **关键逻辑**：
  - **初始化**：跳过新手引导、设默认主题（第 10-19 行）。
  - **transformCallback**（第 24-36 行）：定义 `window._cb{N}` 回调属性。
  - **Mock 数据常量**：
    - `mockConfig`（第 38-58 行）：字段含 `user`、`password`、`selfHelloEnabled`、`selfReverifyEachAction`、`allow2dFaceVerify`、`operator`、`enableNightOperatorSwitch`、`nightOperatorRestore`、`enableNightOutboundSwitch`、`outboundPriority`、`outboundMetricRestore`、`outboundDisabledAdapters`、`outboundStandbyRoute`、`nightOutboundRestore`、`dnsOptimizeAdapters`、`adapter1`、`adapter2`、`dualAdapter`、`autoLoginOnStart`、`autoExitAfterLogin`、`minimizeToTray`、`hiddenStart`、`autoLaunch`、`enableBackgroundCheck`、`backgroundCheckInterval`、`backgroundCheckIdleInterval`、`autoLoginOnPreparation`、`autoExitOnOnline`、`themeMode`、`enableNotification`、`activeAccount`、`enableLatencyTest`、`latencyTestInterval`、`customThemeColor`、`defaultPanel`、`enableNetworkQuality`、`skipTtfbInLatency`、`skipContentInLatency`、`portalUrl`、`fixedGateway`、`requiredNetworkName`、`enableNetworkNameCheck`、`campusGateway`、`updateSource`、`campusExitOnFail`、`campusCheckStartMinutes`、`campusCheckEndMinutes`、`scheduledLoginMinutes`、`scheduledLogoutMinutes`、`maxDisconnectReconnect`、`autoLoginCooldownSecs`、`logRetentionDays`、`configVersion`、`displayName`。
    - `campusStatus`（第 60-65 行）：`onCampusNetwork`/`currentSsid`/`campusMessage`/`enableNetworkNameCheck`/`requiredNetworkName`/`campusWifi`/`campusWired`。
    - `backgroundStatus`（第 67-73 行）：`isRunning`/`checkCount`/`serverAvailable`/`online`/`message`/`currentSsid` 等。
    - `mockAdapters`（第 75-79 行）：WLAN/以太网/以太网 2 三网卡对象数组（含 `name`/`ip`/`wireless`/`guid`/`mac`/`ifIndex`/`status`/`linkSpeed`）。
    - `mockAccounts`（第 83-87 行）：三个演示账号（`id`/`displayName`/`user`/`operator`/`passwordSaved`）。
    - `initData`（第 89-96 行）：聚合 `config`/`version`/`adapters`/`accounts`/`backgroundStatus` 等。
  - **命令处理器** `handlers`（第 98-164 行）：
    - 配置类：`get_config`(99)、`save_config`(100)、`get_init_data`(101)、`get_active_account`(102)。
    - 账号类：`list_accounts`(103)、`create_account`(106-117)、`save_current_as_account`(118-127)、`rename_account`(128-133)、`delete_account`(134-138)。
    - 网络类：`get_adapters`(139)、`get_adapter_details`(140)、`get_disabled_adapters`(141)、`get_current_outbound_name`(142)、`outbound_switch_now`(143)、`outbound_restore_now`(144)、`check_campus_status`(145)、`get_background_status`(146)、`check_campus_network`(156)、`check_network_quality`(157)、`do_login`(160)、`do_logout`(161)、`bind_to_wifi`(162)、`request_wifi_ssid_permission`(163)。
    - 其他：`get_soc_info`(147)、`get_notification_enabled`(148)、`set_notification_enabled`(149)、`get_boot_autostart`(150)、`get_battery_optimization_info`(151)、`get_debug_mode`(152)、`set_debug_mode`(153)、`get_logs`(154)、`get_log_retention_days`(155)、`check_update`(158)、`get_avoid_bad_wifi_status`(159)。
    - 未处理命令泛化兜底 `console.warn` 并返回 null（第 185-188 行）。
  - **事件系统**：`window.__TAURI_INTERNALS__.invoke` 处理 `plugin:event|listen/unlisten`（第 175-184 行）；`window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener`（第 194-198 行）；`window.__mockEmitEvent` 触发监听（第 199-206 行）。

### 测试文件

#### `tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx`

- **职责**：验证 `AccountPanel` 中“Hello 验证门”行为，模块级单例“首次操作即验证，通过后共用”（第 1-150 行）。
- **Mock 策略**：
  - `vi.mock('@/hooks/tauriApi')` 拦截 `verifyWindowsIdentity`/`getBindStatus`/`bindOperator`/`revealOperatorCredential`（第 7-18 行）。
  - `vi.mock('react-i18next')` 返回键名（第 19-21 行）。
  - `vi.mock('@/hooks/useLogToastStore')` 无操作（第 22-25 行）。
  - `storeState` 对象（第 26-32 行）字段：`configLoaded`、`passwordSaved`、`config`(含 `user`/`operator`/`adapter1`/`selfHelloEnabled`/`selfReverifyEachAction`)、`saveConfigDirect`、`syncPasswordSaved`；`vi.mock('@/hooks/useConfigStore')` 返回该状态（第 33-38 行）。
  - `vi.mock('@/components/ui/animated-card')` 简化为 div（第 39-41 行）。
- **辅助函数**：
  - `renderPanel()`：动态 `import('./AccountPanel')` 渲染（第 61-75 行）。
  - `openBindDialog(container)`：查找 `account.bindOperator` 按钮点击开弹窗（Radix Portal 挂 `document.body`，第 81-86 行）。
  - `findQueryBtn()`：从 `document.body` 查 `account.bindStatusQuery` 按钮（第 87-88 行）。
  - `beforeEach`：`vi.resetModules()` 重置模块图使 `helloGate` 回 `firstFree`（第 45-54 行，重置在第 53 行）。
- **核心用例**：
  - 用例 1：首次查询触发验证，后续复用（第 91-107 行）。
  - 用例 2：验证通过后查看明文不二次验证（第 109-130 行）。
  - 用例 3：验证失败不执行操作，下次仍需验证（第 132-149 行）。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     Root Directory                           │
│  ┌─────────────┐  ┌──────────────────┐  ┌────────────────┐  │
│  │ AGENTS.md   │  │ LICENSE /        │  │ .gitignore     │  │
│  │ (工作约定)   │  │ LICENSE-ASSETS   │  │ (忽略规则)      │  │
│  └─────────────┘  │ (双轨许可)        │  └────────────────┘  │
│                   └──────────────────┘                       │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                 Build Scripts                           │ │
│  │  clean-build-cache.ps1  ►  make-release.ps1            │ │
│  │         ▲                      │                        │ │
│  │         │                    ┌─▼──────────┐             │ │
│  │         └────────────────────│ build.ps1   │             │ │
│  │                              │ (Tauri Build)│             │ │
│  └──────────────────────────────┴──────────────┘             │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              tauri-app/frontend                         │ │
│  │  index.html (Skeleton/Theme)  package.json (Deps)       │ │
│  │  public/                                                    │
│  │    ├── mock-tauri.js (API Shim)                          │ │
│  │    └── mock-tauri-addon.js (Scenario Injector)           │ │
│  │  src/account/ (AccountPanel + HelloGate Test)            │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

### 构建与发布流

1. **触发**：开发者运行 `pwsh make-release.ps1`（第 1 行起）或 `pwsh tauri-app/build.ps1`。
2. **编译**：`make-release.ps1` 调用 `npx tauri build`（第 48 行），`build.ps1` 执行前端 `npm run build`（第 86 行）与后端 `tauri build`（第 127 或 129 行）。
3. **产物生成**：默认 `CARGO_TARGET_DIR=<root>\target`（build.ps1 第 111-113 行），`bundle\nsis\*-setup.exe` 产出（make-release.ps1 第 70-76 行）；`build.ps1` 拷至 `tauri-release/`（第 154-184 行）并生成 `.sha256`（第 189-193 行）。
4. **资产汇总**：`make-release.ps1` 读 `tauri.conf.json` 版本（第 36-40 行），重命名安装包为 `Wxxy-CampusLogin_<ver>_<arch>-setup.exe`（第 81 行），收集 APK（第 95-110 行）与 `RELEASE_NOTES_v<ver>.md`（第 112-116 行），放入 `release/`（第 26、83 行）。

### 前端初始化流

1. **加载**：浏览器加载 `index.html`（第 1 行起）。
2. **主题判断**：内联脚本读 `localStorage` 决定 `data-light`/`theme-*`/`data-glass`（第 9-26 行）。
3. **骨架屏显示**：CSS 渲染 `sk-*` 占位，JS 按 `app-language` 显初始化提示（第 74-115 行）。
4. **Mock 注入（仅开发）**：若未连 Tauri，`mock-tauri.js` 设 `window.__TAURI_INTERNALS__`（第 166 行），`mock-tauri-addon.js` 叠加场景（第 3 行）。
5. **React 挂载**：`/src/main.tsx` 启动（第 116 行），初始化 i18n 与 Zustand。

### Hello Gate 状态流

1. **模块级单例**：`AccountPanel` 内 `helloGate` 维护 `isVerified`（测试第 52 行注释）。
2. **首次操作**：敏感操作（查询绑定状态/查看明文）若未验证则先调 `verifyWindowsIdentity`（测试第 97-98 行）。
3. **验证成功**：置 `verified`，后续操作共用不再弹（测试第 102-106 行）。
4. **验证失败**：保持未验证，业务 API 调用 0 次，下次重验（测试第 138-147 行）。
5. **模块重置**：测试 `vi.resetModules()` 强制重载模块图，保证用例隔离（测试第 53 行）。

## Connections

- [[decisions/version-single-source-build-rs]]：版本号单一来源（`tauri.conf.json` 的 `version`，AGENTS.md 第 7 行、make-release.ps1 第 36-40 行）。
- [[decisions/release-asset-integrity]]：Release 资产完整性校验（`.sha256` 生成，build.ps1 第 189-193 行、make-release.ps1 第 90-93 行）。
- [[concepts/dual-platform-sharing]]：双端共享协议核心（AGENTS.md 第 11 行、README 第 179 行）。
- [[learnings/android-host-cargo-check-fails]]：安卓端 host cargo check 失败应对（AGENTS.md 第 16 行）。

## Known Issues

- **Radix Portal 定位陷阱**：绑定表单（弹窗 B）用 Radix Dialog/Portal，DOM 脱离 React 树挂 `document.body`。测试中不能直接在 `container` 查按钮，必须从 `document.body.querySelectorAll` 查找（测试第 81-88 行注释明确）。
- **凭据来源变更**：测试注释指出“凭据统一后绑定卡不再收集学号/自助服务密码...查询按钮直接可用”（测试第 77-80 行），UI 逻辑简化，直接从 Store 取信息。
- **模块重置必要性**：每个用例前须 `vi.resetModules()`（测试第 53 行），否则首个用例改的门状态污染后续用例。
- **验证失败不阻断门存在只阻断操作**：`verifyWindowsIdentity` 返回失败时业务 API 调用次数期望 0（测试第 138-141 行），门状态未变成功，下次仍须重验。
- **明文显示实现**：明文密码直渲在按钮附近 DOM 文本，测试用 `document.body.textContent` 断言（测试第 123-129 行），非 React 状态。
- **测试基线文档不一致**：AGENTS.md 第 15 行记 `cargo test` 基线 397 用例（2026-09-22），README.md 第 186-188 行记 432 单测 + 4 集成；前端 README 第 193-195 行记 103 测试。两文档未同步，实际以 CodeWiki `decisions/verification-baseline` 为准（AGENTS.md 第 15 行指向）。
- **构建脚本被 git 忽略**：`.gitignore` 第 67 行忽略 `tauri-app/build.ps1`、第 68 行忽略 `clean-build-cache.ps1`、第 13 行忽略 `make-release.ps1`（第 12-13 行注释说明为“仅本地保留不随仓库分发”），意味着这些脚本改动不进版本控制。