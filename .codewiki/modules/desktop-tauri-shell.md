---
title: desktop-tauri-shell
type: module
source_files:
  - tauri-app/build.ps1
  - tauri-app/frontend/about-preview.html
  - tauri-app/frontend/ANIMATION_GUIDE.md
  - tauri-app/frontend/index.html
  - tauri-app/frontend/package.json
  - tauri-app/frontend/pnpm-lock.yaml
  - tauri-app/frontend/postcss.config.js
  - tauri-app/frontend/public/mock-tauri-addon.js
  - tauri-app/frontend/public/mock-tauri.js
tags: [Tauri, Build, Frontend, Mock, Animation]
---

## Overview
该模块涵盖 CampusLogin 桌面端应用（基于 Tauri v2，构建脚本自称 `CampusLogin Tauri v2.0.0 Build`，见 `build.ps1` 第 11 行）的构建脚本、前端资源、动画集成指南及开发环境模拟层。核心功能包括：
- **自动化构建**：`tauri-app/build.ps1` 负责环境检查、依赖安装、前端打包、Rust/Cargo 编译及产物分发到 `tauri-release/`。
- **UI 与动效**：基于 React 19（`package.json` 第 31-32 行）、Framer Motion（`package.json` 第 26 行）与 Anime.js（`ANIMATION_GUIDE.md` 提及）的高性能动画系统，包含骨架屏（`index.html`）、关于页预览（`about-preview.html` 显示版本 v2.4.3，第 227、251 行）及全局毛玻璃效果。
- **开发仿真**：通过 `public/mock-tauri.js` 与 `public/mock-tauri-addon.js` 在浏览器环境中伪造 Tauri API，支持多场景截图验证（文件头均注明“绝不入库”，见 `mock-tauri.js` 第 1-2 行、`mock-tauri-addon.js` 第 1 行）。
- **工程配置**：基于 Vite 6（`package.json` 第 48 行）、Tailwind CSS 3.4（`package.json` 第 46 行）及 PostCSS（`postcss.config.js`）的前端构建栈，依赖锁定于 `pnpm-lock.yaml`（lockfileVersion 9.0，第 1 行）。

## Key Components

### 1. Build System (PowerShell)
文件：`tauri-app/build.ps1`

| 组件/变量 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| `$SkipFrontend` | 2 | 跳过前端构建开关（参数） |
| `$SkipCopy` | 3 | 跳过产物复制开关（参数） |
| `$TargetDir` | 4 | 自定义 Cargo 目标目录，解决 Windows 路径过长问题 |
| 环境检查 | 32-55 | 检查 `rustc`（34-40）、`node`（42-48）、`cargo`（50-54）是否可用 |
| 依赖安装 | 57-79 | 检查 `frontend/node_modules`（60-67）与根 `node_modules`（69-76），缺失则 `npm install` |
| 前端构建 | 82-96 | 执行 `npm run build`（84-88）；若跳过且无 `dist/index.html` 则报错（90-95） |
| Tauri 编译 | 99-145 | 设置 `CARGO_BUILD_JOBS` 为 CPU 核心数（101-103）；默认目标目录为项目根 `target`（109-114）；前台调用 `@tauri-apps/cli`（119-130）；计时（122-137） |
| 产物复制 | 153-193 | 将 `release/bundle` 复制到 `tauri-release/`（164-178），复制 `campus-login.exe`（180-184） |
| 校验和生成 | 186-193 | 对 `*-setup.exe` 生成 SHA256 文件（格式 `<hash>  <filename>`），供更新器验证 |

### 2. Frontend Assets & Configuration

#### A. Skeleton Screen (`index.html`)
文件：`tauri-app/frontend/index.html`

| 组件/逻辑 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| IIFE 主题初始化 | 9-26 | 读取 `localStorage` 设置 `data-light`、`theme-*`、`data-glass` 属性，避免首屏闪烁 |
| CSS 变量 | 28-73 | 定义深/浅色模式变量（如 `--bg-primary`、`--sk-accent`、`--sidebar-bg`） |
| 骨架 DOM | 90-103 | 渲染加载中结构（`.sk-tb`、`.sk-sb`、`.sk-main`、`.sk-side`、`.sk-content`、`.sk-log`） |
| i18n 兜底 | 104-115 | 根据 `app-language` 本地存储切换骨架屏文案（中文/英文 `Initializing...`） |

#### B. About Preview (`about-preview.html`)
文件：`tauri-app/frontend/about-preview.html`

| 组件/样式 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| 对话框遮罩 | 18-24 | 半透明模糊遮罩 `.dialog-overlay` |
| 左侧栏 | 223-238 | 应用图标（SVG data URI）、标题“校园网登录助手”、版本 `v2.4.3`（227）、描述、检查更新按钮、GitHub 链接 |
| 右侧栏 | 241-271 | 关闭按钮（242-244）、状态圆圈（247-249）、已是最新版本文案（250-251）、特性卡片网格 |
| 特性卡片 | 254-270 | 三列网格：双适配器支持、多账号管理、自动重连（对应图标与说明） |

#### C. Package Configuration
文件：`tauri-app/frontend/package.json` 与 `pnpm-lock.yaml`

| 依赖项 | 版本（package.json 行） | 锁定版本（pnpm-lock.yaml） | 用途 |
| :--- | :--- | :--- | :--- |
| `react` / `react-dom` | ^19.0.0（31-32） | 19.3.0（59-64） | UI 框架 |
| `@tauri-apps/api` | ^2（22） | 2.12.1（943-944） | Tauri 客户端 API |
| `@tauri-apps/plugin-shell` | ^2（23） | 2.4.0（946-947） | Shell 插件 |
| `framer-motion` | ^12.38.0（26） | 12.43.0（1235-1236） | 声明式动画 |
| `gsap` | ^3.15.0（27） | 3.15.0（1273-1274） | 复杂序列动画 |
| `i18next` / `react-i18next` | ^26.3.1 / ^17.0.8（28-33） | 26.4.2 / 17.0.15（1290-1291, 1516-1517） | 国际化 |
| `tailwindcss` | ^3.4.10（46） | 3.4.19（1648-1649） | 原子化 CSS |
| `zustand` | ^5.0.13（35） | 5.0.15（1855-1856） | 状态管理 |
| `vite` | ^6.0.0（48） | 6.4.3（1743-1744） | 构建工具 |
| `vitest` | ^4.1.10（49） | 4.1.11（1783-1784） | 测试运行器 |

#### D. PostCSS Config
文件：`tauri-app/frontend/postcss.config.js`

| 插件 | 行号 | 用途 |
| :--- | :--- | :--- |
| `tailwindcss` | 3 | 处理 Tailwind 指令 |
| `autoprefixer` | 4 | 自动添加浏览器前缀 |

### 3. Animation Guide (`ANIMATION_GUIDE.md`)
文件：`tauri-app/frontend/ANIMATION_GUIDE.md`

| 组件/策略 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| AnimatedNumber | 7-27 | 数字平滑滚动（easeOutQuad, 600ms），颜色高亮闪烁，卸载清理 |
| ElasticBarChart | 30-54 | 柱状图弹性生长（easeOutElastic），波浪式错开启动 |
| StatusPulseBadge | 57-83 | 状态切换双次脉冲，背景闪烁，支持 excellent~bad 五级 |
| AnimatedCard | 88-104 | 卡片 hover 上浮、tap 缩放、磁性倾斜 |
| DockNav 微交互 | 106-115 | 图标 hover 放大 1.15、弹簧回弹 |
| Button 动效 | 117-125 | whileTap 缩放、径向光效 |
| LogPanel 入场 | 127-135 | AnimatePresence 新日志滑入、旧日志滑出 |
| FluidBackground | 140-148 | 极慢渐变与光球，纯 CSS |
| 性能与内存 | 222-240 | 优先 transform/opacity，Anime.js 实例卸载暂停（237-239） |

### 4. Tauri Mock Layer (Browser Shim)

#### A. Core Mock (`mock-tauri.js`)
文件：`tauri-app/frontend/public/mock-tauri.js`

| 函数/对象 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| `transformCallback` | 24-36 | 将回调注册到 `window._cb{id}`，返回 ID |
| `mockConfig` | 38-58 | 默认配置对象，字段含：`user`('23331150'), `password`(''), `selfHelloEnabled`(false), `selfReverifyEachAction`(false), `allow2dFaceVerify`(false), `operator`('cmcc'), `enableNightOperatorSwitch`(false), `nightOperatorRestore`(''), `enableNightOutboundSwitch`(true), `outboundPriority`(['WLAN','以太网','以太网 2']), `outboundMetricRestore`(''), `outboundDisabledAdapters`(''), `outboundStandbyRoute`(''), `nightOutboundRestore`(''), `dnsOptimizeAdapters`([]), `adapter1`('以太网'), `adapter2`('以太网 2'), `dualAdapter`(true), `autoLoginOnStart`(false), `autoExitAfterLogin`(false), `minimizeToTray`(false), `hiddenStart`(false), `autoLaunch`(false), `enableBackgroundCheck`(true), `backgroundCheckInterval`(30000), `backgroundCheckIdleInterval`(300000), `autoLoginOnPreparation`(false), `autoExitOnOnline`(false), `themeMode`('dark'), `enableNotification`(true), `activeAccount`('mock-001'), `enableLatencyTest`(false), `latencyTestInterval`(30000), `customThemeColor`(''), `defaultPanel`('account'), `enableNetworkQuality`(true), `skipTtfbInLatency`(true), `skipContentInLatency`(true), `portalUrl`(''), `fixedGateway`(''), `requiredNetworkName`('i-wxxy'), `enableNetworkNameCheck`(true), `campusGateway`(''), `updateSource`('mirror'), `campusExitOnFail`(false), `campusCheckStartMinutes`(460), `campusCheckEndMinutes`(1380), `scheduledLoginMinutes`(0), `scheduledLogoutMinutes`(0), `maxDisconnectReconnect`(3), `autoLoginCooldownSecs`(60), `logRetentionDays`(7), `configVersion`(1), `displayName`('演示账号') |
| `campusStatus` | 60-65 | 校园网状态模拟：`onCampusNetwork`(true), `currentSsid`('i-wxxy'), `campusMessage`(''), `enableNetworkNameCheck`(true), `requiredNetworkName`('i-wxxy'), `campusWifi`({onCampus:true,name:'i-wxxy',message:''}), `campusWired`(null) |
| `backgroundStatus` | 67-73 | 后台检测状态模拟：`isRunning`(false), `checkCount`(0), `serverAvailable`(true), `online`(false), `message`('演示模式:未运行检测'), `currentSsid`('i-wxxy'), `onCampusNetwork`(true), `enableNetworkNameCheck`(true), `requiredNetworkName`('i-wxxy'), `campusWifi`(campusStatus.campusWifi), `campusWired`(null), `interval`(30000), `enabled`(true) |
| `mockAdapters` | 75-79 | 三网卡数组：WLAN(192.168.6.108,wireless:true,guid,mac,ifIndex:12,status:'connected',linkSpeed:287000000), 以太网(10.2.69.22,wireless:false,ifIndex:16,linkSpeed:1000000000), 以太网 2(192.168.6.109,wireless:false,ifIndex:4,linkSpeed:2500000000) |
| `mockAccounts` | 83-87 | 账号卡演示数据数组：mock-001(演示账号,user:'23331150',operator:'cmcc',passwordSaved:true), mock-002(室友的号,user:'23330999',operator:'telecom',passwordSaved:true), mock-003(备用账号,user:'',operator:'',passwordSaved:false) |
| `initData` | 89-96 | 聚合初始化数据：`config`(mockConfig), `version`('2.3.9-mock'), `adapters`(mockAdapters), `adapterDetails`([]), `disabledAdapters`([]), `accounts`(mockAccounts), `activeAccount`('mock-001'), `backgroundStatus`(backgroundStatus), `isAutoStart`(false), `autoLaunch`(false), `notificationEnabled`(true), `logRetentionDays`(7), `debugMode`(false) |
| `handlers` | 98-164 | 命令映射对象，方法含 `get_config`(99), `save_config`(100), `get_init_data`(101), `get_active_account`(102), `list_accounts`(103), `create_account`(106-117), `save_current_as_account`(118-127), `rename_account`(128-133), `delete_account`(134-138), `get_adapters`(139), `get_adapter_details`(140), `get_disabled_adapters`(141), `get_current_outbound_name`(142), `outbound_switch_now`(143), `outbound_restore_now`(144), `check_campus_status`(145), `get_background_status`(146), `get_soc_info`(147), `get_notification_enabled`(148), `set_notification_enabled`(149), `get_boot_autostart`(150), `get_battery_optimization_info`(151), `get_debug_mode`(152), `set_debug_mode`(153), `get_logs`(154), `get_log_retention_days`(155), `check_campus_network`(156), `check_network_quality`(157), `check_update`(158), `get_avoid_bad_wifi_status`(159), `do_login`(160), `do_logout`(161), `bind_to_wifi`(162), `request_wifi_ssid_permission`(163) |
| `__TAURI_INTERNALS__` | 166-190 | 伪造 invoke，路由事件监听与命令；`metadata`(167-170), `__TAURI_PATTERN__`(172), `transformCallback`(173), `invoke`(174-189) |
| `__TAURI_EVENT_PLUGIN_INTERNALS__` | 194-198 | 事件反注册 `unregisterListener`(195-197) |
| `__mockEmitEvent` | 199-205 | 手动触发事件回调，遍历 `eventHandlers` 调用 `window['_cb'+cbId]` |

#### B. Scenario Addon (`mock-tauri-addon.js`)
文件：`tauri-app/frontend/public/mock-tauri-addon.js`

| 参数/逻辑 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| URL 参数解析 | 5-11, 82, 88 | `op`(5-6), `dual`(7), `night`(8), `online`(9, 由 `offline` 取反), `op2`(10), `tab`(11); `emuw`(82); `probe`(88) |
| `invoke` 覆盖 | 13-48 | 动态修改 `check_campus_status`(15-21), `get_adapters`(22-28), `check_portal_status`(29-31), `do_login`(32), `get_background_status`(33), `get_init_data`(34-46) 等返回 |
| `bgStatus()` | 50-67 | 生成场景后台状态函数，返回对象含 `isRunning`, `checkCount`, `serverAvailable`, `online`, `onlineOperator`, `secondaryOnlineOperator`, `message`, `adapterStatuses`, `currentSsid`, `onCampusNetwork`, `enableNetworkNameCheck`, `requiredNetworkName`, `campusWifi`, `campusWired`, `interval`, `enabled`, `a1CampusMessage`, `a2CampusMessage`, `a1OnCampus`, `a2OnCampus` |
| 事件发射 | 69-80 | `setTimeout` 发送 `background-check-result`(70)，`setInterval` 每 3 秒发送(77-80) |
| 标签页切换 | 71-76 | 自动点击 Dock 指定 `aria-label` 按钮（取最后一个匹配） |
| 宽度仿真 | 82-87 | CSS 强制 `body` 宽度模拟窄屏（`emuw` 参数） |
| 诊断探针 | 88-95 | `probe=1` 时将溢出元素信息写入 `document.title` 供 `--dump-dom` 读取 |

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   Browser / Dev Console                     │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────┐  │
│  │ index.html   │  │ mock-tauri.js    │  │ mock-addon.js│  │
│  │ (Skeleton)   │─►│(__TAURI_INTERNALS│─►│(Scenario URL)│  │
│  └──────┬───────┘  └────────┬─────────┘  └──────┬───────┘  │
│         │                   │                    │          │
│         ▼                   ▼                    ▼          │
│  ┌──────────────────────────────────────────────────────┐  │
│  │        React App (Vite 6 + Tailwind 3.4)            │  │
│  │  Animations(Framer/Anime) │ Zustand │ Tauri invoke  │  │
│  └───────────────────────────────┬──────────────────────┘  │
└───────────────────────────────────┼────────────────────────┘
                                    │ Build Output
                                    ▼
┌─────────────────────────────────────────────────────────────┐
│                   build.ps1 (PowerShell)                    │
│  npm install ─► vite build ─► cargo tauri build ─► copy     │
│  (frontend/dist)   (src-tauri)        └─► tauri-release/    │
│  + *.sha256 for *-setup.exe                                │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **开发环境启动**：
   - `index.html` 加载，IIFE 设置主题与玻璃档位（第 9-26 行），随后加载骨架屏 DOM（90-103）。
   - `mock-tauri.js` 注入 `window.__TAURI_INTERNALS__`（第 166 行起），接管所有 Tauri 命令。
   - `mock-tauri-addon.js` 解析 URL 参数（第 5-11 行），覆盖 `invoke`（第 13-48 行）返回场景数据（双网卡、夜间、离线）。
   - React 应用通过 `@tauri-apps/api` 调用，由 Mock 层 Promise 返回。

2. **生产构建流程**：
   - 运行 `.\build.ps1`（可加 `-SkipFrontend`、`-SkipCopy`、`-TargetDir`）。
   - 检查 Rust/Node/Cargo（第 32-55 行）。
   - 安装前端与根依赖（第 57-79 行），执行 `vite build`（第 84-88 行）。
   - 调用 `node node_modules/@tauri-apps/cli/tauri.js build`（第 119-130 行）触发 Cargo 编译。
   - 复制 `release/bundle` 与 `campus-login.exe` 到 `tauri-release/`（第 153-184 行）。
   - 为 `-setup.exe` 生成 `.sha256`（第 186-193 行），更新器需此文件校验。

## Connections
* [[desktop-electron-migration]]：`build.ps1` 第 22-29 行打印旧 Electron 目录（`archive/`、`installer-build/`、`release-build/`、`renderer-app/`）布局，标明 Tauri 与 Electron 产物分离。
* [[tauri-backend-rust]]：`build.ps1` 第 119-130 行调用的 `tauri build` 依赖 `src-tauri` 下 Rust 代码（未在此模块列出）。
* [[frontend-react-core]]：前端动画指南、Mock 脚本及 `index.html` 骨架屏服务于 React 应用用户体验。

## Known Issues
1. **Cargo 静默期**：编译单个大 crate 时终端可能数分钟无输出（`build.ps1` 第 116-118 行注释），属正常行为，切勿中断。
2. **Windows 路径长度**：如遇路径过长，需用 `-TargetDir 'C:\short\target'`（`build.ps1` 第 109-114 行设定项目内化 `target`，第 141 行提示逃生参数）。
3. **Mock 隔离**：`mock-tauri.js` 第 1-2 行、`mock-tauri-addon.js` 第 1 行明确标注“绝不入库”，提交时须排除（CI/`.gitignore` 处理）；`index.html` 中无注入行但旧版提及，以当前代码为准。
4. **Anime.js 内存泄漏风险**：`ANIMATION_GUIDE.md` 第 237-239 行要求组件卸载时暂停动画实例，否则可能泄漏。
5. **锁定文件一致性**：`pnpm-lock.yaml` 中 `@testing-library/jest-dom` 被标为 deprecated（第 956 行），建议后续升级到 7.0.0 线，但当前 `package.json` 仍约束 `^6.9.1`（第 38 行 devDependencies）。
