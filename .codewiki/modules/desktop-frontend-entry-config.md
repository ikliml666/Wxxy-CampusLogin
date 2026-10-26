---
title: desktop-frontend-entry-config
type: module
source_files:
  - tauri-app/frontend/index.html
  - tauri-app/frontend/about-preview.html
  - tauri-app/frontend/ANIMATION_GUIDE.md
  - tauri-app/frontend/package.json
  - tauri-app/frontend/postcss.config.js
  - tauri-app/build.ps1
tags: [桌面端, 前端, 构建配置, 入口, 骨架屏, 动效]
---

## Overview

本模块是桌面端前端的「工程配置 + HTML 入口」层，不含任何 React 业务逻辑，但决定了应用启动后到 React 挂载之前的全部可见行为。它由四类文件组成：

| 文件 | 角色 |
| --- | --- |
| `tauri-app/frontend/index.html` | 唯一 HTML 入口；内联首帧主题脚本、骨架屏样式与 DOM、语言兜底脚本，最后加载 `/src/main.tsx` |
| `tauri-app/frontend/package.json` | 前端依赖与脚本清单（dev/build/preview/test/test:watch/clean），版本 `2.4.2` |
| `tauri-app/frontend/postcss.config.js` | PostCSS 管线：仅 `tailwindcss` + `autoprefixer` 两个插件 |
| `tauri-app/build.ps1` | 桌面端全链路打包脚本：npm install → `npm run build` → `tauri build` → 产物拷贝与 SHA256 校验文件生成 |

另有两个辅助产物：`tauri-app/frontend/about-preview.html` 是「关于」对话框的纯静态设计预览（无 JS，无 React），`tauri-app/frontend/ANIMATION_GUIDE.md` 是动画组件分工与效果说明文档。

本模块的核心价值在于**首帧一致性**：`index.html` 用一段同步内联脚本在 CSS 解析前就把主题、玻璃档位、语言写进 `<html>`，避免启动时先按默认主题渲染一次再跳变。

## Key Components

### index.html：首帧主题脚本（`index.html:9-26`）

一段立即执行的 IIFE，依次读取三个 localStorage 键并落到 `documentElement`：

- `index.html:16-21`：读 `campus-light-mode`，为 `'1'` 时设 `data-light="1"`（行14），否则加 `dark` 类（行16）。
- `index.html:19-20`：读 `campus-theme`，非 `default` 时追加 `theme-<值>` 类（多主题色支持）。
- `index.html:23-26`：读 `campus-glass-level`，`off`/`ultra` 原样写入 `data-glass`，其余一律归为 `standard`；注释（`index.html:23`）明确说明这是为了「玻璃档位在首帧前定下，避免启动时先按默认档渲染一次再跳变」。
- `index.html:24`：`catch(e){}` 静默吞掉 localStorage 不可用/异常，保证脚本永不阻塞后续解析。

### index.html：骨架屏（`index.html:27-87`）

- `index.html:28-51`：`:root` 定义暗色默认变量集（`--bg-primary: #0f172a`、`--bg-secondary: #1e293b`、`--sk-accent: #3b82f6`、`--sidebar-bg: rgba(15,23,42,0.7)` 等约 20 个变量）。
- `index.html:52-73`：`html[data-light]` 覆盖同一批变量为亮色（`--bg-primary: #ffffff` 等），即主题切换只换变量、不改选择器。
- `index.html:74-87`：骨架屏结构类：`.sk`（纵向铺满 100vh）、`.sk-tb`（40px 顶栏）、`.sk-sb`（32px 状态栏，含 `.sk-dot` 脉冲圆点，`index.html:79-80` 定义 `sk-pulse` 1.5s 无限动画）、`.sk-main` / `.sk-side`（180px 侧栏）/ `.sk-content` / `.sk-card` / `.sk-line`（占位线，12px 高）/ `.sk-log`（220px 日志栏）。
- `index.html:90-103`：`#root` 内的骨架屏 DOM，与后续 React 布局同构；状态栏文案节点为 `#sk-init-text`，初值「正在初始化...」（`index.html:93`）。

### index.html：骨架屏幕文案语言兜底（`index.html:104-115`）

- `index.html:105-111`：读 `app-language`，为 `'en'` 时把 `#sk-init-text` 改为 `Initializing...`；注释（`index.html:105-106`）说明该键与 i18n 的 `lookupLocalStorage` 同名，保证 i18n 尚未初始化时首屏文案语言正确。

### index.html：入口与页面元信息

- `index.html:2`：`<html lang="zh-CN">`。
- `index.html:4-5`：charset 与 viewport。
- `index.html:6-7`：favicon 两个尺寸 `/32x32.png`、`/128x128.png`（由 Vite `public/` 直出）。
- `index.html:8`：窗口标题「校园网登录助手」。
- `index.html:116`：`<script type="module" src="/src/main.tsx"></script>`——React 挂载的唯一入口。

### package.json：脚本与依赖

- `package.json:2-5`：包名 `campus-login-frontend`、版本 `2.4.2`、`private: true`、`"type": "module"`（因此配置文件用 ESM 语法）。
- `package.json:8-15` 脚本：`dev`=`vite`、`build`=`vite build`、`preview`=`vite preview`、`test`=`vitest run`、`test:watch`=`vitest`、`clean`=内联 Node 脚本删除 `dist` 与 `node_modules/.vite`。
- `package.json:25-47` 运行时依赖：Radix UI 七件套（dialog/label/select/separator/slot/switch/tooltip，行15-21）、`@tauri-apps/api` 与 `@tauri-apps/plugin-shell`（均 `^2`，行22-23）、`class-variance-authority`、`clsx`、`framer-motion ^12.38.0`（行26）、`gsap ^3.15.0`（行27）、`i18next ^26.3.1` + `i18next-browser-languagedetector` + `react-i18next`（行28-29,33）、`lucide-react ^0.446.0`（行30）、`react`/`react-dom` `^19.0.0`（行31-32）、`tailwind-merge`（行34）、`zustand ^5.0.13`（行35）。
- `package.json:43-51` 开发依赖：`@testing-library/jest-dom`、`@testing-library/react`、`@types/react`、`@types/react-dom`、`@vitejs/plugin-react ^4.3.0`、`autoprefixer`、`jsdom`、`postcss`、`tailwindcss ^3.4.10`、`typescript ^5.5.0`、`vite ^6.0.0`、`vitest ^4.1.10`。

### postcss.config.js（`postcss.config.js:1-6`）

`export default { plugins: { tailwindcss: {}, autoprefixer: {} } }`——无额外 PostCSS 插件，样式能力全部由 Tailwind + 项目 CSS 变量承担。

### build.ps1：桌面打包脚本

- `build.ps1:1-5` 参数：`-SkipFrontend`（跳前端构建）、`-SkipCopy`（跳产物拷贝）、`-TargetDir`（自定义 `CARGO_TARGET_DIR`）。
- `build.ps1:7`：`$ErrorActionPreference = "Stop"`。
- `build.ps1:15-20` 路径推导：`$scriptDir` = 脚本所在目录，`$tauriAppDir` = 当前目录，`$srcTauriDir` = `src-tauri`，`$frontendDir` = `frontend`，`$campusLoginDir` = 上一级仓库根，`$releaseOutputDir` = `<根>/tauri-release`。
- `build.ps1:32-54` 环境检查：`rustc`、`node`、`cargo` 缺任一即 `exit 1`。
- `build.ps1:59-79`：frontend 与 tauri-app 两级 `node_modules` 各自缺则 `npm install`，已存在则跳过。
- `build.ps1:84-96`：`Push-Location $frontendDir; npm run build`；`-SkipFrontend` 时校验 `dist\index.html` 存在，否则报错退出。
- `build.ps1:102-115`：`CARGO_BUILD_JOBS` 设为 CPU 核数；默认 `CARGO_TARGET_DIR` = `<仓库根>/target`（`build.ps1:102-104`，注释说明 2026-09-15 起废弃 `C:\cl-build` 短路径，路径过长时用 `-TargetDir` 逃生）。
- `build.ps1:119-134`：优先直接用 `node node_modules\@tauri-apps\cli\tauri.js build`，缺失则回退 `npx.cmd tauri build`；注释（`build.ps1:116-118`）解释必须前台直通，重定向会破坏子进程 tty 导致进度输出消失。
- `build.ps1:139-143`：非零退出码透传 `exit $buildResult`；`-SkipCopy` 时提前 `exit 0`（`build.ps1:147-151`）。
- `build.ps1:156-178`：从 `<target>/release/bundle` 递归拷贝到 `tauri-release/`，先删除旧目录（`build.ps1:164-167`）。
- `build.ps1:180-184`：额外拷贝 `release/campus-login.exe`。
- `build.ps1:186-193`：对每个 `*-setup.exe` 生成 `.sha256` 文件，格式为 shasum 兼容的 `<hash>  <filename>`、`-NoNewline`；注释（`build.ps1:186-188`）强调应用内更新器强制 SHA256 校验，缺这个文件用户无法安装。

### about-preview.html：关于对话框静态预览

纯静态设计稿，用于在没有 React 的环境里校对「关于」对话框视觉：

- `about-preview.html:1-6`：独立 HTML5 文档，标题「关于 - 校园网登录助手」。
- `about-preview.html:7-214`：内联 `<style>`，定义 `.dialog-overlay`（`rgba(0,0,0,0.5)` + `backdrop-filter: blur(4px)`）、`.dialog-wrap`、`.dialog-body`（780×520、16px 圆角）、`.left-panel`（35%、最小 273px）、`.app-icon`、`.right-panel`、`.features-grid`（三列）、`.feature-card` 等。
- `about-preview.html:218-274`：DOM 结构与真实对话框一一对应；`about-preview.html:225` 用 data-URI 内联 SVG 做应用图标占位，`about-preview.html:227`、`251` 硬编码版本号 `v2.4.2`，`about-preview.html:230-233` 为「已是最新版本」按钮，`about-preview.html:254-270` 为三张特性卡片（双适配器支持 / 多账号管理 / 自动重连）。
- 该文件无 `<script>`（全篇无 script 标签），所有状态（检查中/已最新/有新版）均为写死的静态样本。

### ANIMATION_GUIDE.md：动效分工文档

- `ANIMATION_GUIDE.md:245-248` 分工表：Framer Motion 负责基础交互、布局动画、高频更新（`whileHover`/`whileTap`/`AnimatePresence`）；Anime.js 负责复杂序列、精细控制、物理模拟（数字滚动、柱状图生长、状态脉冲）。
- `ANIMATION_GUIDE.md:254-265` 文件清单：新增 `src/components/shared/AnimatedNumber.tsx`、`ElasticBarChart.tsx`、`StatusPulseBadge.tsx`；修改 `animated-card.tsx`、`DockNav.tsx`、`button.tsx`、`LogPanel.tsx`、`FluidBackground.tsx`、`src/index.css`。
- `ANIMATION_GUIDE.md:272-289` 效果约定：启动入场 0.4s / `staggerChildren` 0.1s 且动画期间不可点击；卡片悬停上浮 4px、按钮缩到 0.92、Dock 图标放大 1.15 + 上浮 6px；数据更新 0.6s、柱状图每根错开 50ms、状态切换 1s；日志新条目右侧滑入 spring、旧条目左侧滑出。

## Architecture

```text
┌─────────────────────────── tauri-app/build.ps1 ───────────────────────────┐
│ [1/5] 检查 rustc/node/cargo ► [2/5] 两级 npm install                        │
│ [3/5] frontend: npm run build ──► dist/index.html + assets                  │
│ [4/5] node @tauri-apps/cli tauri.js build (CARGO_TARGET_DIR=<root>/target)  │
│ [5/5] release/bundle ──► tauri-release/  + *-setup.exe.sha256               │
└───────────────────────────────────┬────────────────────────────────────────┘
                                    ▼
┌──────────────────────── frontend/index.html ────────────────────────────┐
│ :9-26   同步 IIFE：campus-light-mode / campus-theme / campus-glass-level  │
│         ──► documentElement[data-light | class=dark | theme-<x>]         │
│ :27-87  骨架屏 CSS（:root 暗色变量 ◄► html[data-light] 亮色变量）          │
│ :90-103 骨架屏 DOM（#root 内，#sk-init-text="正在初始化..."）             │
│ :104-115 app-language==='en' ► #sk-init-text="Initializing..."           │
│ :116    <script type="module" src="/src/main.tsx"> ──► React 挂载         │
└───────────────────────────────────┬──────────────────────────────────────┘
                                    ▼
┌──────────── 配置层 ─────────────┐   ┌──────── 文档/预览 ────────┐
│ package.json  脚本+deps         │   │ about-preview.html 静态稿  │
│ postcss.config.js tailwind+auto │   │ ANIMATION_GUIDE.md 动效说明│
└─────────────────────────────────┘   └───────────────────────────┘
```

## Data Flow

**启动（首帧 → React 接管）**

1. WebView 解析 `index.html`，执行 `index.html:9-26` 的内联脚本，从 localStorage 取 `campus-light-mode`、`campus-theme`、`campus-glass-level`，重型写入 `<html>` 的属性/类。
2. 内联 `<style>`（`index.html:27-87`）按 `:root` 或 `html[data-light]` 的变量集渲染骨架屏，此时无网络、无 IPC、无 React。
3. `index.html:104-115` 脚本按 `app-language` 本地化 `#sk-init-text`。
4. `index.html:116` 的 module script 加载 `/src/main.tsx`，React 挂载后替换骨架屏。

**构建（源码 → 安装包）**

1. `build.ps1:32-54` 校验工具链 → `build.ps1:59-79` 装依赖 → `build.ps1:84-96` 前端 `vite build` 产出 `dist/`。
2. `build.ps1:101-114` 交由 tauri CLI 把 `dist/` 打进二进制，Rust 侧编译产物落 `<仓库根>/target`。
3. `build.ps1:156-193` 把 `target/release/bundle` 与 `campus-login.exe` 拷入 `tauri-release/`，并为安装器生成 SHA256 校验文件。

**样式（CSS 变量链）**

`postcss.config.js:1-6` 先跑 Tailwind 再跑 autoprefixer；主题值本身不来自 Tailwind 配置，而来自 `index.html:28-51`（暗）与 `52-73`（亮）这两组 CSS 变量，运行时由 `:9-26` 脚本切换。

## Connections

- [[desktop-tauri-shell-config]]：`tauri.conf.json` 的 `build.frontendDist` / `beforeDevCommand` 指向本模块产出的 `frontend/dist` 与 `npm run dev`。
- [[desktop-tauri-shell]]：Tauri 窗口加载 `index.html`，骨架屏即该窗口的首帧。
- [[frontend-build-config]]：前端构建配置的通用版本，与本文的桌面端参数对照。
- [[frontend-ui-shell]]：React 挂载后的真实布局，骨架屏（`.sk-*`）是它的同构占位。
- [[desktop-frontend-shared]]：`AnimatedNumber.tsx`、`ElasticBarChart.tsx`、`StatusPulseBadge.tsx` 等被 `ANIMATION_GUIDE.md` 点名的组件归属。
- [[desktop-frontend-panels]]：`LogPanel`、`QualityPanel` 是动效接入点。
- [[desktop-app-lifecycle]]：启动阶段与首帧主题的时序。
- [[build-scripts-and-root]]：仓库级构建脚本总览。
- [[design-prototypes]]：`about-preview.html` 与同类静态设计稿的目录归属。

## Known Issues

1. **动画指南与依赖清单不一致**：`ANIMATION_GUIDE.md:245-248` 与 `254-256` 反复声明使用 Anime.js，但 `package.json:14-36` 的运行时依赖里只有 `framer-motion ^12.38.0`（第 26 行）与 `gsap ^3.15.0`（第 27 行），没有任何 anime.js 条目；同时指南未提及 gsap。若文档描述仍然有效，说明动画已改由 gsap 承担而文档未更新；二者需对齐其一。
2. **`about-preview.html` 版本号硬编码但当前一致**：`about-preview.html:227` 与 `251` 写死 `v2.4.2`，与 `package.json:3` 的版本号当前一致，但仍是两份独立真相，版本升级时会静默过期（本项目版本号另有单一来源约定）。
3. **`about-preview.html` 无任何交互**：文件内没有 `<script>`（全篇无 script 标签），「已是最新版本」按钮（`about-preview.html:230-233`）只是静态样本，不能当作更新检查逻辑的参考实现。
4. **`build.ps1` 标题版本号硬编码**：`build.ps1:141` 打印 `CampusLogin Tauri v2.0.0 Build`，与前端 `2.4.2` 不一致，仅影响控制台输出，易误导打包者判断当前版本。
5. **路径长度风险**：`build.ps1:111-113` 默认 `CARGO_TARGET_DIR=<仓库根>/target`，仓库路径较深时 Windows 会触发长路径错误，脚本仅在失败提示里建议 `-TargetDir 'C:\short\target'`（`build.ps1:141`），无自动降级。
6. **`-SkipFrontend` 的隐式依赖**：`build.ps1:90-95` 只校验 `dist\index.html` 是否存在，不校验其新鲜度，跳过前端构建可能把旧产物打进包内。
