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
| `tauri-app/frontend/package.json` | 前端依赖与脚本清单（dev/build/preview/test/test:watch/clean），版本 `2.4.3` |
| `tauri-app/frontend/postcss.config.js` | PostCSS 管线：仅 `tailwindcss` + `autoprefixer` 两个插件 |
| `tauri-app/build.ps1` | 桌面端全链路打包脚本：npm install → `npm run build` → `tauri build` → 产物拷贝与 SHA256 校验文件生成 |

另有两个辅助产物：`tauri-app/frontend/about-preview.html` 是「关于」对话框的纯静态设计预览（无 JS，无 React），`tauri-app/frontend/ANIMATION_GUIDE.md` 是动画组件分工与效果说明文档。

本模块的核心价值在于**首帧一致性**：`index.html` 用一段同步内联脚本在 CSS 解析前就把主题、玻璃档位、语言写进 `<html>`，避免启动时先按默认主题渲染一次再跳变。

## Key Components

### index.html：页面元信息与 React 入口

- `index.html:2`：`<html lang="zh-CN">` 根元素语言声明。
- `index.html:4-5`：`<meta charset="UTF-8" />` 与 viewport 设置。
- `index.html:6-7`：两个尺寸 favicon 链接（`/32x32.png`、`/128x128.png`），由 Vite `public/` 直出。
- `index.html:8`：`<title>校园网登录助手</title>` 窗口标题。
- `index.html:116`：`<script type="module" src="/src/main.tsx"></script>` —— React 挂载的唯一入口标签。

### index.html：首帧主题脚本（`index.html:9-26`）

一段立即执行的 IIFE（函数体 `index.html:10-25`），同步读取 localStorage 并修改 `document.documentElement`：

- `index.html:16-21`：读取 `campus-light-mode`；为 `'1'` 时 `index.html:16` 设 `data-light="1"`，否则 `index.html:16` 添加 `dark` 类。
- `index.html:19-20`：读取 `campus-theme`，非 `default` 时追加 `theme-<值>` 类（多主题色支持）。
- `index.html:20`：注释说明「玻璃档位在首帧前定下，避免启动时先按默认档渲染一次再跳变」。
- `index.html:23-25`：读取 `campus-glass-level`，`index.html:23-24` 将其写入 `data-glass` 属性，值为 `off`/`ultra` 时原样保留，其余一律归为 `standard`。
- `index.html:24`：`catch(e){}` 静默吞掉 localStorage 不可用/异常，保证脚本永不阻塞后续解析。

### index.html：骨架屏样式（`index.html:27-87`）

- `index.html:28-51`：`:root` 选择器内定义暗色默认 CSS 变量集（常量），含 `--bg-primary: #0f172a`(行29)、`--bg-secondary: #1e293b`(行30)、`--text-primary: #f1f5f9`(行31)、`--text-secondary: #94a3b8`(行32)、`--text-muted: #64748b`(行33)、`--sk-accent: #3b82f6`(行34)、`--sk-border: #1e293b`(行35)、`--sidebar-bg: rgba(15,23,42,0.7)`(行36)、`--status-bar-bg: rgba(30,41,59,0.5)`(行37)、`--content-bg: rgba(15,23,42,0.3)`(行38)、`--hover-bg`(行39)、`--hover-bg-strong`(行40)、`--active-bg`(行41)、`--active-color`(行42)、`--active-border`(行43)、`--badge-bg`(行44)、`--badge-color`(行45)、`--badge-border`(行46)、`--icon-accent-bg`(行47)、`--icon-accent-color`(行48)、`--logo-from`(行49)、`--logo-to`(行50) 等。
- `index.html:52-73`：`html[data-light]` 选择器覆盖同一批变量为亮色（如 `--bg-primary: #ffffff` 行53、`--bg-secondary: #f8fafc` 行54 等），主题切换只换变量不改选择器。
- `index.html:74-87`：骨架屏结构类（常量样式）：`* { margin:0; padding:0; box-sizing:border-box }`(行74)、`html, body, #root`(行75)、`.sk`(行76)、`.sk-tb`(行77)、`.sk-sb`(行78)、`.sk-dot`(行79) 与 `@keyframes sk-pulse`(行80)、`.sk-main`(行81)、`.sk-side`(行82)、`.sk-content`(行83)、`.sk-card`(行84)、`.sk-line`(行85)、`.sk-log`(行86)。

### index.html：骨架屏 DOM（`index.html:90-103`）

- `index.html:90-103`：`#root` 内的骨架屏 HTML 结构，与后续 React 布局同构；状态栏文案节点 `index.html:93` 为 `<span id="sk-init-text">正在初始化...</span>`。

### index.html：骨架屏文案语言兜底脚本（`index.html:104-115`）

另一段 IIFE（`index.html:107-114`）：

- `index.html:105-106`：注释说明使用 `app-language` 键，与 i18n 的 `lookupLocalStorage` 同名键保持一致。
- `index.html:109`：判断 `localStorage.getItem('app-language')==='en'`。
- `index.html:111-112`：取得 `#sk-init-text` 元素并将其 `textContent` 改为 `'Initializing...'`。

### package.json：包元数据、脚本与依赖（`package.json:1-51`）

- `package.json:2-5`：常量 `name: "campus-login-frontend"`、`version: "2.4.3"`、`private: true`、`type: "module"`（决定配置文件用 ESM 语法）。
- `package.json:8-15`：scripts 对象（公开脚本常量）：`dev`(行7 `vite`)、`build`(行8 `vite build`)、`preview`(行9 `vite preview`)、`test`(行10 `vitest run`)、`test:watch`(行11 `vitest`)、`clean`(行12-13 内联 Node 脚本删除 `dist` 与 `node_modules/.vite`)。
- `package.json:25-47`：dependencies 常量集：`@radix-ui/react-dialog`(行15)、`@radix-ui/react-label`(行16)、`@radix-ui/react-select`(行17)、`@radix-ui/react-separator`(行18)、`@radix-ui/react-slot`(行19)、`@radix-ui/react-switch`(行20)、`@radix-ui/react-tooltip`(行21)、`@tauri-apps/api`(行22)、`@tauri-apps/plugin-shell`(行23)、`class-variance-authority`(行24)、`clsx`(行25)、`framer-motion ^12.38.0`(行26)、`gsap ^3.15.0`(行27)、`i18next ^26.3.1`(行28)、`i18next-browser-languagedetector`(行29)、`lucide-react ^0.446.0`(行30)、`react ^19.0.0`(行31)、`react-dom ^19.0.0`(行32)、`react-i18next`(行33)、`tailwind-merge`(行34)、`zustand ^5.0.13`(行35)。
- `package.json:43-51`：devDependencies 常量集：`@testing-library/jest-dom`(行38)、`@testing-library/react`(行39)、`@types/react`(行40)、`@types/react-dom`(行41)、`@vitejs/plugin-react ^4.3.0`(行42)、`autoprefixer`(行43)、`jsdom`(行44)、`postcss`(行45)、`tailwindcss ^3.4.10`(行46)、`typescript ^5.5.0`(行47)、`vite ^6.0.0`(行48)、`vitest ^4.1.10`(行49)。

### postcss.config.js（`postcss.config.js:1-6`）

- `postcss.config.js:1-6`：导出默认对象，plugins 仅含 `tailwindcss: {}`(行3) 与 `autoprefixer: {}`(行4)，无额外 PostCSS 插件。

### build.ps1：参数与路径常量（`build.ps1:1-20`）

- `build.ps1:1-5`：参数常量 `-SkipFrontend`(行2)、`-SkipCopy`(行3)、`-TargetDir`(行4)。
- `build.ps1:7`：`$ErrorActionPreference = "Stop"` 全局错误处理偏好。
- `build.ps1:15-20`：路径推导变量 `$scriptDir`(行15)、`$tauriAppDir`(行16)、`$srcTauriDir`(行17)、`$frontendDir`(行18)、`$campusLoginDir`(行19)、`$releaseOutputDir`(行20)。

### build.ps1：环境检查与依赖安装（`build.ps1:32-79`）

- `build.ps1:32-40`：检查 `rustc` 是否存在，缺失则红字报错 `exit 1`(行35-38)。
- `build.ps1:42-48`：检查 `node`，缺失则退出。
- `build.ps1:50-54`：检查 `cargo`，缺失则退出。
- `build.ps1:59-79`：若非 `-SkipFrontend`，对 `$frontendDir`(行60-67) 与 `$tauriAppDir`(行69-76) 各自判断 `node_modules` 缺失则 `npm install`。

### build.ps1：前端与 Tauri 构建（`build.ps1:81-151`）

- `build.ps1:84-96`：`Push-Location $frontendDir; npm run build`(行85-87)；`-SkipFrontend` 时校验 `dist\index.html` 存在(行90-95)，否则报错退出。
- `build.ps1:101-114`：设置 `$env:CARGO_BUILD_JOBS` 为 CPU 核数(行102-103)；`$TargetDir` 非空则用自定义(行105-107)，否则 `build.ps1:111-113` 设默认 `$env:CARGO_TARGET_DIR = <仓库根>/target`，注释(行109-110)说明 2026-09-15 起废弃 `C:\cl-build` 短路径。
- `build.ps1:116-118`：注释解释必须前台直通调用 CLI 以避免破坏 tty 进度条。
- `build.ps1:119-134`：优先 `node $tauriCliJs build`(行126-127)，否则 `npx.cmd tauri build`(行129)；记录 `$buildResult`(行131)。
- `build.ps1:139-143`：非零退出码透传 `exit $buildResult`；`build.ps1:147-151` 若 `-SkipCopy` 则提前 `exit 0`。

### build.ps1：产物拷贝与校验（`build.ps1:153-212`）

- `build.ps1:156-178`：从 `<target>/release/bundle` 递归拷贝到 `$releaseOutputDir`，先删旧目录(行164-167)。
- `build.ps1:180-184`：额外拷贝 `release/campus-login.exe` 到输出根。
- `build.ps1:186-193`：对每个 `*-setup.exe` 生成 `<hash>  <filename>` 格式、`-NoNewline` 的 `.sha256` 文件；注释(行186-188)强调应用内更新器强制 SHA256 校验。
- `build.ps1:208-212`：打印最终目录结构提示。

### about-preview.html：关于对话框静态预览（`about-preview.html:1-277`）

- `about-preview.html:1-6`：独立 HTML5 文档头，标题「关于 - 校园网登录助手」。
- `about-preview.html:7-214`：内联 `<style>` 定义 `.dialog-overlay`(行18-24)、`.dialog-wrap`(行26-34)、`.dialog-body`(行36-45, 780×520)、`.left-panel`(行48-57)、`.app-icon`(行59-65)、`.app-title`(行67-73)、`.app-version`(行75-80)、`.app-desc`(行82-88)、`.check-btn`(行90-107)、`.github-link`(行109-123)、`.right-panel`(行126-137)、`.close-btn`(行139-155)、`.status-circle`(行157-166)、`.features-grid`(行180-187)、`.feature-card`(行189-197) 等。
- `about-preview.html:218-274`：DOM 结构与真实对话框对应；`about-preview.html:225` 用 data-URI 内联 SVG 图标；`about-preview.html:227` 与 `251` 硬编码版本 `v2.4.3`；`about-preview.html:230-233` 为「已是最新版本」按钮；`about-preview.html:254-270` 为三张特性卡片（双适配器支持/多账号管理/自动重连）。
- 全文件 `about-preview.html:1-277` 无任何 `<script>` 标签，所有状态均为写死静态样本。

### ANIMATION_GUIDE.md：动效分工文档（`ANIMATION_GUIDE.md:1-293`）

- `ANIMATION_GUIDE.md:245-248`：分工表声明 Framer Motion 负责基础交互/布局动画/高频更新，Anime.js 负责复杂序列/精细控制/物理模拟。
- `ANIMATION_GUIDE.md:254-256`：新增文件清单点名 `AnimatedNumber.tsx`、`ElasticBarChart.tsx`、`StatusPulseBadge.tsx`。
- `ANIMATION_GUIDE.md:258-265`：修改文件清单含 `animated-card.tsx`、`DockNav.tsx`、`button.tsx`、`LogPanel.tsx`、`FluidBackground.tsx`、`src/index.css`。
- `ANIMATION_GUIDE.md:272-289`：效果预览约定（启动入场 0.4s、卡片悬停上浮 4px、按钮缩 0.92、Dock 图标放大 1.15、数据更新 0.6s、柱状图错开 50ms、状态切换 1s、日志滑入 spring 等）。

## Architecture

```text
┌─────────────────────────── tauri-app/build.ps1 ───────────────────────────┐
│ [1/5] 检查 rustc/node/cargo (行32-54) ► [2/5] 两级 npm install (行59-79)    │
│ [3/5] frontend: npm run build (行84-96) ──► dist/index.html + assets        │
│ [4/5] node @tauri-apps/cli tauri.js build (行119-134, CARGO_TARGET_DIR 行111-113) │
│ [5/5] release/bundle ──► tauri-release/ (行156-178) + *-setup.exe.sha256 (行186-193) │
└───────────────────────────────────┬────────────────────────────────────────┘
                                    ▼
┌──────────────────────── frontend/index.html ────────────────────────────┐
│ 行9-26   同步 IIFE：campus-light-mode / campus-theme / campus-glass-level  │
│         ──► documentElement[data-light | class=dark | theme-<x>]         │
│ 行27-87  骨架屏 CSS（:root 暗色变量 行28-51 ◄► html[data-light] 亮色变量 行52-73） │
│ 行90-103 骨架屏 DOM（#root 内，#sk-init-text="正在初始化..." 行93）       │
│ 行104-115 app-language==='en' ► #sk-init-text="Initializing..." (行109-111) │
│ 行116    <script type="module" src="/src/main.tsx"> ──► React 挂载         │
└───────────────────────────────────┬──────────────────────────────────────┘
                                    ▼
┌──────────── 配置层 ─────────────┐   ┌──────── 文档/预览 ────────┐
│ package.json (行1-51) 脚本+deps │   │ about-preview.html (行1-277)│
│ postcss.config.js (行1-6)       │   │ ANIMATION_GUIDE.md (行1-293)│
└─────────────────────────────────┘   └───────────────────────────┘
```

## Data Flow

**启动（首帧 → React 接管）**

1. WebView 解析 `index.html`，执行 `index.html:9-26` 的内联 IIFE，从 localStorage 取 `campus-light-mode`、`campus-theme`、`campus-glass-level` 并写入 `<html>` 属性/类。
2. 内联 `<style>`（`index.html:27-87`）按 `:root` 或 `html[data-light]` 的变量集渲染骨架屏，此时无网络、无 IPC、无 React。
3. `index.html:104-115` 脚本按 `app-language` 本地化 `#sk-init-text`（行109-111）。
4. `index.html:116` 的 module script 加载 `/src/main.tsx`，React 挂载后替换骨架屏。

**构建（源码 → 安装包）**

1. `build.ps1:32-54` 校验工具链 → `build.ps1:59-79` 装依赖 → `build.ps1:84-96` 前端 `vite build` 产出 `dist/`。
2. `build.ps1:101-114` 交由 tauri CLI 把 `dist/` 打进二进制，Rust 侧编译产物落 `<仓库根>/target`。
3. `build.ps1:156-193` 把 `target/release/bundle` 与 `campus-login.exe` 拷入 `tauri-release/`，并为安装器生成 SHA256 校验文件。

**样式（CSS 变量链）**

`postcss.config.js:1-6` 先跑 Tailwind 再跑 autoprefixer；主题值本身不来自 Tailwind 配置，而来自 `index.html:28-51`（暗）与 `52-73`（亮）这两组 CSS 变量，运行时由 `index.html:9-26` 脚本切换。

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

1. **动画指南与依赖清单不一致**：`ANIMATION_GUIDE.md:245-248` 与 `254-256` 反复声明使用 Anime.js，但 `package.json:14-36` 的运行时依赖里只有 `framer-motion ^12.38.0`（行26）与 `gsap ^3.15.0`（行27），没有任何 anime.js 条目；同时指南未提及 gsap。若文档描述仍然有效，说明动画已改由 gsap 承担而文档未更新；二者需对齐其一。
2. **`about-preview.html` 版本号硬编码但当前一致**：`about-preview.html:227` 与 `251` 写死 `v2.4.3`，与 `package.json:3` 的版本号当前一致，但仍是两份独立真相，版本升级时会静默过期（本项目版本号另有单一来源约定）。
3. **`about-preview.html` 无任何交互**：文件内没有 `<script>` 标签（全篇 `about-preview.html:1-277` 无 script 字样），「已是最新版本」按钮（`about-preview.html:230-233`）只是静态样本，不能当作更新检查逻辑的参考实现。
4. **`build.ps1` 标题版本号硬编码**：`build.ps1:141` 打印 `CampusLogin Tauri v2.0.0 Build`，与前端 `2.4.3`（`package.json:8`）不一致，仅影响控制台输出，易误导打包者判断当前版本。
5. **路径长度风险**：`build.ps1:111-113` 默认 `CARGO_TARGET_DIR=<仓库根>/target`，仓库路径较深时 Windows 会触发长路径错误，脚本仅在失败提示里建议 `-TargetDir 'C:\short\target'`（`build.ps1:141`），无自动降级。
6. **`-SkipFrontend` 的隐式依赖**：`build.ps1:90-95` 只校验 `dist\index.html` 是否存在，不校验其新鲜度，跳过前端构建可能把旧产物打进包内。
