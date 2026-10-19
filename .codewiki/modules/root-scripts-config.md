---
title: root-scripts-config
type: module
source_files:
  - .gitignore
  - AGENTS.md
  - android/.gitignore
  - android/build-apk.ps1
  - android/CODE_WIKI.md
  - android/README.md
  - LICENSE
  - LICENSE-ASSETS.md
  - make-release.ps1
  - README.md
  - target-release-build.log
  - tauri-app/build.ps1
  - tauri-app/frontend/about-preview.html
  - tauri-app/frontend/ANIMATION_GUIDE.md
  - tauri-app/frontend/index.html
  - tauri-app/frontend/package-lock.json
  - tauri-app/frontend/package.json
tags: [构建, 发布, 许可证, 文档, 约定]
---

## Overview

本模块涵盖项目根目录下的核心基础设施文件，包括版本控制忽略配置 (`.gitignore`)、AI 助手与贡献者工作约定 (`AGENTS.md`)、双端构建脚本 (`tauri-app/build.ps1`, `android/build-apk.ps1`)、发布汇总脚本 (`make-release.ps1`) 以及项目级元数据 (`README.md`, `LICENSE*`)。这些文件定义了项目的开发流程、环境隔离规则、产物生成逻辑及法律合规声明。

## Key Components

### `.gitignore`
定义全局 Git 忽略规则，确保敏感文件、构建产物及 IDE 配置不入库。
- **依赖与构建**：忽略 `node_modules/`, `target/`, `dist/`, `release/`, `output/`, `app-output/`, `app-v/`, `tauri-release/` 等目录（第 2-9 行）。
- **本地资产**：忽略 `make-release.ps1` 和 `installer-build/`, `release-build/`, `archive/` 等仅本地保留的发布资产（第 11-13 行）。
- **Tauri 生成物**：忽略 `tauri-app/src-tauri/gen/`（第 16 行）。
- **环境与日志**：忽略 `.env`, `.env.test`, `*.log`, `logs/`（第 19-21 行）。
- **IDE 配置**：忽略 `.trae/`, `.vscode/`, `.idea/`（第 24-26 行）。
- **临时文件**：忽略 `*.tmp`, `*.pdb`, `temp/`, `asar_check2/`, `asar_restore/`（第 29-31 行）。
- **会话临时产物**：忽略 `preview-shots/`, `target-tmp/`（第 35-36 行，2026-10-02 用户确认清理并忽略）。
- **Runtime data**：忽略 `config.json`（第 39 行）。
- **Release notes**：忽略 `RELEASE_*.md`, `v*.*.*-更新日志.md`（第 42-43 行）。
- **Changelog**：忽略 `CHANGELOG.md`, `changelogs/`（第 46-47 行，2026-09-15 起废弃统一 CHANGELOG.md，按版本文件维护）。
- **内部文档**：忽略 `ersikDocumentstrae_projects1`, `docs/`（第 50-51 行）。
- **Build scripts**：忽略 `tauri-app/build.ps1`, `clean-build-cache.ps1`（第 54-55 行）。
- **Helper binary**：忽略 `tauri-app/src-tauri/campus-helper.exe`（第 58 行）。
- **Animation guide**：忽略 `tauri-app/frontend/ANIMATION_GUIDE.md`（第 61 行）。
- **Code wiki**：忽略 `CODE_WIKI.md`（第 64 行）。
- **tsbuildinfo**：忽略 `*.tsbuildinfo`（第 67 行）。
- **Android 专属**：忽略 `.android-git-backup/`, `android/*.apk`, `android/*.apk.idsig`, `android/build-apk.ps1`（第 70-74 行）。
- **浏览器实测用 mock**：忽略 `mock-tauri*.js`, `.zcode/`（第 77-78 行）。
- **iOS 壳 crate 构建产物**：忽略（第 81 行注释）。

### `AGENTS.md`
面向 AI 编码助手（ZCode 等）与贡献者的项目级约定。架构与模块详解见仓库内 `.codewiki/`；版本变更记录见 `changelogs/`。
- **必守约定 1：Changelog 规范**：任何代码/配置/构建/文档改动，完成并验证后写入 `changelogs/CHANGELOG_v<版本>.md`（沿用既有分类：行为调整/缺陷修复/前端性能优化/架构改进 等），说清"改了什么 + 为什么 + 验证数据"。**默认内容存放的版本号 = 当前版本号 + 1**（当前版本 v2.3.7 → 新内容写 `changelogs/CHANGELOG_v2.3.8.md`；该文件即 v2.3.8 发布时的更新日志，发布后封版，新内容顺延写下一版文件）。`changelogs/` 在 `.gitignore` 中，仅本地留存、不提交；不再维护统一的 `CHANGELOG.md`（2026-09-15 起废弃），发布流程也没有快照步骤。
- **必守约定 2：分支纪律**：改动全在主题分支（`feat/xxx`, `fix/xxx`, `perf/xxx`, `chore/xxx`），不直接改 main；commit 说明用 `feat:` / `fix:` / `perf:` / `chore:` / `docs:` 前缀；任务收尾可自行将本会话主题分支 merge 回 main（优先 `--ff-only`，遇冲突即停），但**不擅自 push、删分支**——由用户决定（2026-10-12 用户修订：合并放开，push/删分支仍禁止）。
- **必守约定 3：通用改进双端同步**：本项目是 Windows 桌面端 + 安卓端的双端应用，安卓前端（`android/frontend`）是独立复刻代码库，**桌面改动不会自动同步到安卓**。凡通用改进——前端 UI/交互行为、文案与 i18n、图标与看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等——必须在**同一次提交内双端各改一份**（桌面 `tauri-app/` + 安卓 `android/`），不允许只改一端留待"下次同步"；提交前用 `git diff --stat` 自检是否同时触及两端。两类例外：协议实现单点存在于桌面 crate（安卓经 Cargo path 依赖自动继承，禁止复制，见 CodeWiki 的 `decisions/protocol-core-single-source` 与 `concepts/dual-platform-sharing`）；平台专属能力（DPAPI/Keystore、托盘/前台服务等）各端自理。
- **必守约定 4：验证以实际输出为准，不凭"应该没问题"**：
    - 前端：`npx tsc --noEmit --incremental`（在 `tauri-app/frontend` 下；**禁止 `tsc -b`**——`tsconfig.node.json` 为 composite 项目，会 emit 出 `vite.config.js/.d.ts` 污染文件）。涉及打包产物时 `npx vite build` 确认无警告。安卓前端同法（在 `android/frontend` 下执行）。
    - 后端：`cargo test`（在 `tauri-app/src-tauri` 下；全绿基线 397 用例（lib 396 + 集成 1，2026-09-22 实测），以 CodeWiki 的 `decisions/verification-baseline` 为准）。
    - 安卓端：host `cargo check` / `cargo test` 在 `android/src-tauri` 基线即失败（mobile-only 插件门控）——Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量）或一键出包 `pwsh android/build-apk.ps1`（详见 CodeWiki 的 `decisions/verification-baseline` 与 `learnings/android-host-cargo-check-fails`）。
    - 布局/交互类改动需浏览器实测：向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，**用后必须完整还原**（git diff 必须干净）。
- **必守约定 5：文档同步**：新模块、决策变更、踩坑记录同步进 CodeWiki（仓库内 `.codewiki/`）——修了 bug 写 `learnings/<slug>.md`、做了决策写 `decisions/<slug>.md`、并更新被改动代码影响到的文章；收尾跑 `cw index` 重建索引与 `cw meta update` 记录 commit，两者对 `.codewiki/` 的改动随代码一起提交。用户可见的行为变化同步 changelog（见第 1 条）。
- **必守约定 6：语言**：思考、回复、commit 说明、文档一律中文；代码、命令、报错原文保持原样。
- **项目速览**：技术栈：Tauri 2 + React 19 + TypeScript，Windows + 安卓双端同构的校园网（Dr.COM/Portal）自动登录助手。桌面端：前端根 `tauri-app/frontend`、后端根 `tauri-app/src-tauri`；安卓端根 `android/`（后端 path 依赖桌面协议核心 crate，前端为独立复刻树）。发布流程、版本号同步清单、Release 资产检查清单见 CodeWiki 的 `decisions/release-asset-integrity` 与 `decisions/version-single-source-build-rs`（版本号提交与 Release 发布必须同流程完成）。

### `android/.gitignore`
Android 子模块特定的忽略规则。
- 忽略 `frontend/dist/`, `src-tauri/target/`, `build/`, `.tauri/`, `local.properties`, `*.tsbuildinfo`（第 3-13 行）。

### `android/build-apk.ps1`
安卓端一键打包脚本。
- **功能**：执行前端构建（Vite）和 Tauri Android 交叉编译，产出已签名/对齐的 Release APK。
- **参数**：`-SkipFrontend` 跳过前端构建（第 4 行）。
- **路径处理**：设置 `JAVA_HOME` 为 `C:\Users\ik\AppData\Local\Android\jdk\jdk-17.0.20.1+1`（第 10-11 行）。
- **产物位置**：`gen/android/app/build/outputs/apk/universal/release/`（第 12 行）。
- **命令**：调用 `npx @tauri-apps/cli tauri.js android build --target aarch64 --apk`（第 27 行）。

### `make-release.ps1`
一键构建并汇总 GitHub Release 资产的脚本。
- **功能**：包装 `tauri-app/build.ps1`，收集 Windows 安装包、APK 和 Release Notes，重命名安装包以符合 updater 要求，生成 `.sha256` 校验文件。
- **参数**：`-SkipBuild`（跳过编译，仅汇总已有产物）, `-SkipFrontend`（透传 build.ps1）, `-TargetDir`（自定义目标目录）（第 20-23 行）。
- **版本号读取**：从 `tauri-app\src-tauri\tauri.conf.json` 读取 version 字段（第 34-36 行）。
- **环境变量修复**：将 `TMP` 和 `TEMP` 重定向到项目内 `target-tmp` 目录，避免 MSVC 工具链因沙箱限制写入失败（第 44-46 行）。
- **产物收集**：
    - Windows 安装包：查找 `*-setup.exe`，重命名为 `Wxxy-CampusLogin_<版本>_x64-setup.exe`（第 68-73 行）。
    - SHA256：生成 `<hash>  <文件名>` 格式的文件（第 76-77 行）。
    - APK：查找 `Wxxy-CampusLogin_<版本>.apk`，若未找到则取最新 APK 并警告（第 82-91 行）。
    - Release Notes：复制 `RELEASE_NOTES_v<版本>.md`（第 94-96 行）。
- **输出示例**：见 `target-release-build.log`，展示了 v2.4.1 的完整构建与汇总过程（第 1-83 行）。

### `tauri-app/build.ps1`
桌面端 Tauri 应用构建脚本。
- **功能**：检查环境、安装依赖、构建前端、运行 `npx tauri build`、复制产物到 `tauri-release/` 并生成 `.sha256`。
- **参数**：`-SkipFrontend`, `-SkipCopy`, `-TargetDir`（第 1-3 行）。
- **路径结构**：定义 `CampusLogin/` 根目录下的 `archive/`, `installer-build/`, `release-build/`, `renderer-app/`, `tauri-app/`, `tauri-release/` 布局（第 18-24 行）。
- **前端构建**：在 `frontend/` 目录下执行 `npm run build`（第 47-51 行）。
- **Rust 构建**：设置 `CARGO_BUILD_JOBS` 为 CPU 核心数，默认 `CARGO_TARGET_DIR` 为项目根 `target/`（第 59-66 行）。
- **产物复制**：将 `bundle/` 下的 NSIS 安装包复制到 `tauri-release/`，并生成对应的 `.sha256` 文件（第 76-93 行）。

### `LICENSE` & `LICENSE-ASSETS.md`
- **LICENSE**：MIT License，覆盖源代码与构建脚本。包含 Asset Exception 声明，指出美术素材不在 MIT 授权范围内（第 1-32 行）。
- **LICENSE-ASSETS.md**：规定看板娘插画、应用图标等美术素材为“版权所有 (All Rights Reserved)”，禁止单独提取、二次分发、改编、商用及用于 AI 训练。允许在项目内部原样使用（第 1-30 行）。

### `README.md`
项目主文档。
- **简介**：无锡学院校园网登录助手，基于 Tauri 2 + React 19，Windows + Android 双端（第 1-5 行）。
- **功能特性**：详细列出登录、注销、自动重连、运营商切换、DNS 优化、网络监测等功能（第 20-50 行）。
- **技术栈**：Tauri 2, React 19, TypeScript, Rust, TailwindCSS 等（第 53-62 行）。
- **项目结构**：展示目录树，区分桌面端 (`tauri-app/`) 和安卓端 (`android/`)（第 65-110 行）。
- **开发指南**：提供前置要求、依赖安装、开发模式、构建发布及测试命令（第 113-150 行）。
- **安全说明**：DPAPI/AndroidKeyStore 加密存储密码，生物识别验证敏感操作，TLS 1.3，SHA256 校验等（第 153-175 行）。

### `tauri-app/frontend/package.json`
前端项目配置。
- **名称**：`campus-login-frontend`，版本 `2.4.1`（第 2-3 行）。
- **脚本**：`dev`, `build`, `preview`, `test`, `test:watch`, `clean`（第 5-11 行）。
- **依赖**：React 19, Tauri API, Radix UI, Framer Motion, GSAP, i18next, Zustand 等（第 13-28 行）。
- **开发依赖**：Vite 6, Vitest, TypeScript, TailwindCSS 等（第 29-39 行）。

### `tauri-app/frontend/index.html`
前端入口 HTML。
- **骨架屏**：包含 CSS 定义的骨架屏样式（`.sk-*` 类），用于初始化阶段显示（第 22-42 行）。
- **主题加载**：通过 JS 读取 `localStorage` 中的 `campus-light-mode` 和 `campus-theme`，动态添加 class（第 13-20 行）。
- **语言兜底**：根据 `app-language` 设置骨架屏提示文案（第 45-51 行）。

### `tauri-app/frontend/about-preview.html`
关于对话框预览页面。
- **结构**：左侧栏（图标、标题、版本、检查更新按钮、GitHub 链接），右侧栏（状态圆形图标、特性卡片网格）（第 30-130 行）。
- **样式**：内嵌 CSS，定义暗色/亮色模式变量及组件样式（第 10-90 行）。

### `tauri-app/frontend/ANIMATION_GUIDE.md`
动效系统集成指南。
- **Anime.js 组件**：`AnimatedNumber`（数字平滑更新）, `ElasticBarChart`（柱状图弹性生长）, `StatusPulseBadge`（状态脉冲）（第 10-60 行）。
- **Framer Motion 组件**：`AnimatedCard`（卡片交互）, `DockNav`（图标微交互）, `Button`（按钮动效）, `LogPanel`（日志入场）（第 65-100 行）。
- **性能优化**：建议使用 transform/opacity，避免 layout 重排，清理 Anime.js 实例（第 105-120 行）。

### `android/README.md`
安卓端说明文档。
- **结构**：`frontend/` (React), `src-tauri/` (Rust), `gen/android/` (Tauri 生成的 Gradle 工程)（第 5-8 行）。
- **构建命令**：提供前端构建、Tauri CLI 调试与构建命令（第 11-17 行）。
- **平台决策**：强制 WLAN 绑定、协议核心共享、校园网检测方案（SSID + /18 子网 + Portal TCP）、密码存储策略（Keystore）（第 20-28 行）。

### `android/CODE_WIKI.md`
安卓端 CodeWiki 片段。
- **自动登录通知优化**：删除系统通知，改用应用内 emit `auto-login-result` 接活前端死监听，消除重复轰炸；失败通知保留；预置 `MONITOR.was_online=true`（第 2-5 行）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Project Root (CampusLogin/)                  │
│                                                                     │
│  ┌──────────────┐   ┌──────────────────┐   ┌────────────────────┐  │
│  │ .gitignore   │   │ AGENTS.md        │   │ README.md          │  │
│  │ (忽略规则)    │   │ (工作约定)        │   │ (项目文档)          │  │
│  └──────────────┘   └──────────────────┘   └────────────────────┘  │
│                                                                     │
│  ┌──────────────────┐   ┌──────────────────┐   ┌────────────────┐  │
│  │ tauri-app/       │   │ android/         │   │ make-release   │  │
│  │ ├─ build.ps1     │   │ ├─ build-apk.ps1 │   │ .ps1           │  │
│  │ ├─ frontend/     │   │ ├─ frontend/     │   │ (发布汇总)      │  │
│  │ │ ├─ package.json│   │ │ ├─ package.json│   │                │  │
│  │ │ ├─ index.html  │   │ │ ├─ index.html  │   │                │  │
│  │ │ └─ ...         │   │ │ └─ ...         │   │                │  │
│  │ └─ src-tauri/    │   │ └─ src-tauri/    │   │                │  │
│  └──────────────────┘   └──────────────────┘   └────────────────┘  │
│                                                                     │
│  ┌──────────────────┐   ┌──────────────────┐                       │
│  │ LICENSE          │   │ LICENSE-ASSETS   │                       │
│  │ (MIT)            │   │ (Assets Reserved)│                       │
│  └──────────────────┘   └──────────────────┘                       │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 构建与发布流程

1. **开发者触发**：
   - 桌面端：运行 `pwsh tauri-app/build.ps1`。
   - 安卓端：运行 `pwsh android/build-apk.ps1`。
   - 完整发布：运行 `pwsh make-release.ps1`。

2. **`tauri-app/build.ps1` 流程**：
   - 检查 Node.js/Rust 环境。
   - 安装依赖（若缺失）。
   - 构建前端 (`npm run build`)。
   - 设置 `CARGO_TARGET_DIR` 为 `./target`。
   - 执行 `npx tauri build`。
   - 将产物从 `target/release/bundle` 复制到 `tauri-release/`。
   - 生成 `.sha256` 校验文件。

3. **`android/build-apk.ps1` 流程**：
   - 设置 `JAVA_HOME`。
   - 构建前端 (`npx vite build`)。
   - 执行 `npx @tauri-apps/cli tauri.js android build --target aarch64 --apk`。
   - 输出 APK 至 `android/src-tauri/gen/android/app/build/outputs/apk/universal/release/`。

4. **`make-release.ps1` 流程**：
   - 读取 `tauri.conf.json` 获取版本号。
   - 若未 `-SkipBuild`，调用 `tauri-app/build.ps1`。
   - 定位 bundle 目录，查找最新的 `*-setup.exe`。
   - 重命名安装包为 `Wxxy-CampusLogin_<版本>_x64-setup.exe`。
   - 生成对应 `.sha256` 文件。
   - 查找 APK（精确匹配或最新），复制到 release 目录。
   - 查找 `RELEASE_NOTES_v<版本>.md`，复制到 release 目录。
   - 输出最终 release 文件夹，准备上传 GitHub Release。

## Connections

- [[root-scripts-config]] -> [[tauri-app-build]] (通过 `tauri-app/build.ps1` 引用)
- [[root-scripts-config]] -> [[android-build]] (通过 `android/build-apk.ps1` 引用)
- [[root-scripts-config]] -> [[release-asset-integrity]] (通过 `make-release.ps1` 和 `LICENSE-ASSETS.md` 引用)
- [[root-scripts-config]] -> [[dual-platform-sharing]] (通过 `AGENTS.md` 双端同步约定引用)