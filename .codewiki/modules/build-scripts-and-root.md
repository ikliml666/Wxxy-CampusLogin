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

## Key Components

### 根目录脚本
#### `clean-build-cache.ps1`
清理项目内全部编译缓存与中间产物。
- **功能范围**：删除 `target/`、`dist/`、`*.tsbuildinfo` 及 WebView2 运行时缓存。
- **安全机制**：检测运行中的 `cargo/rustc/tauri` 进程并阻止；若 `gen/` 下存在 `release/` 中无的 APK 唯一副本，会红色警告。
- **参数支持**：`-WhatIf`（预览）、`-Confirm:$false`（静默）。
- **关键逻辑行号**：
  - 路径定义与进程检测：第 45-85 行。
  - WebView2 缓存清理：第 107-128 行（从 `tauri.conf.json` 动态读取 identifier）。
  - APK 唯一性告警：第 133-142 行。

#### `make-release.ps1`
一键构建并汇总 Release 资产。
- **流程**：读取版本号 -> 调用 `build.ps1` -> 重命名 NSIS 安装包为 `Wxxy-CampusLogin_<版本>_x64-setup.exe` -> 生成 `.sha256` 校验文件 -> 收集 APK 与 Release Notes。
- **关键逻辑行号**：
  - 环境变量修复（解决 MSVC 沙箱写入问题）：第 56-61 行。
  - 安装包定位与重命名：第 95-105 行。
  - SHA256 生成：第 108-110 行（格式 `<hash>  <filename>`，两空格无换行）。
  - APK 收集：第 118-128 行（优先精确版本名，否则取最新）。

#### `tauri-app/build.ps1`
桌面端 Tauri 发布构建脚本。
- **步骤**：环境检查 -> 依赖安装 -> 前端构建 (`npm run build`) -> Tauri 构建 (`npx tauri build`) -> 产物拷贝至 `tauri-release/`。
- **关键配置**：默认使用项目根 `target/` 作为 Cargo 输出目录；支持 `-SkipFrontend` 跳过前端编译。
- **关键逻辑行号**：
  - 环境变量设置：第 66-74 行（`CARGO_BUILD_JOBS` 与 `CARGO_TARGET_DIR`）。
  - 产物拷贝与 SHA256 生成：第 96-118 行。

### 前端基础结构
#### `tauri-app/frontend/index.html`
React 应用入口。
- **特性**：内联 CSS 实现深色/浅色模式切换（通过 `data-light` 属性）；包含骨架屏（Skeleton）样式以在 i18n 加载前展示占位符。
- **关键逻辑行号**：
  - 主题初始化脚本：第 14-23 行（读取 `campus-light-mode` 和 `campus-theme`）。
  - 骨架屏文案语言兜底：第 63-71 行（根据 `app-language` 显示英文或中文）。

#### `tauri-app/frontend/package.json`
前端依赖与脚本定义。
- **依赖**：React 19, Framer Motion 12, GSAP 3, Zustand 5, TailwindCSS 3.4。
- **脚本**：`dev`, `build`, `preview`, `test` (Vitest)。

#### `tauri-app/frontend/public/mock-tauri.js`
浏览器环境下的 Tauri API Mock（仅供开发/UI 验证，不入库）。
- **功能**：伪造 `__TAURI_INTERNALS__`，模拟 `invoke` 调用返回预设数据（如 `mockConfig`, `mockAdapters`）。
- **关键逻辑行号**：
  - 回调转换函数：第 22-36 行。
  - 命令处理器映射：第 66-88 行（处理 `get_config`, `do_login` 等）。
  - 事件监听模拟：第 102-112 行。

#### `tauri-app/frontend/public/mock-tauri-addon.js`
Mock 场景注入器（叠加在 mock-tauri.js 之上）。
- **功能**：通过 URL 参数（如 `?op=@telecom&dual=1`）动态修改 Mock 状态，用于截图验证特定网络场景。
- **关键逻辑行号**：
  - 参数解析与状态覆盖：第 8-25 行。
  - 定时事件发射：第 56-63 行。

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

## Connections
- [[decisions/version-single-source-build-rs]]：版本号单一来源与构建流程关联。
- [[decisions/release-asset-integrity]]：Release 资产完整性校验策略（SHA256）。
- [[concepts/dual-platform-sharing]]：双端共享协议核心的架构决策。
- [[learnings/android-host-cargo-check-fails]]：安卓端主机 cargo check 失败的应对方案。