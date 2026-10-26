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
  - tauri-app/frontend/package.json
tags: [构建, 发布, 许可证, 文档, 约定]
---

## Overview

本模块涵盖项目根目录下的核心基础设施文件，包括版本控制忽略配置 (`.gitignore`)、AI 助手与贡献者工作约定 (`AGENTS.md`)、双端构建脚本 (`tauri-app/build.ps1`, `android/build-apk.ps1`)、发布汇总脚本 (`make-release.ps1`) 以及项目级元数据 (`README.md`, `LICENSE*`)。这些文件定义了项目的开发流程、环境隔离规则、产物生成逻辑及法律合规声明。当前项目版本为 2.4.3（见 `README.md` 行5、`tauri-app/frontend/package.json` 行3、`tauri-app/frontend/about-preview.html` 行227；`target-release-build.log` 行7 记录的是历史 v2.4.1 构建）。

## Key Components

### `.gitignore`

定义全局 Git 忽略规则，确保敏感文件、构建产物及 IDE 配置不入库（行1-112）。

- **依赖与构建输出**：忽略 `node_modules/`（行2）、`target/`, `dist/`, `release/`, `output/`, `app-output/`, `app-v/`, `tauri-release/`（行5-11）。
- **发布资产汇总脚本与旧资产**：忽略 `make-release.ps1`（行13，仅本地保留）、`installer-build/`, `release-build/`, `archive/`（行14-16）。
- **Tauri 生成物**：忽略 `tauri-app/src-tauri/gen/`（行19）。
- **环境与日志**：忽略 `.env`, `.env.test`（行22-23）、`*.log`, `logs/`（行31-32）。
- **IDE 与 OS**：忽略 `.trae/`, `.vscode/`, `.idea/`（行26-28）、`.DS_Store`, `Thumbs.db`, `Desktop.ini`（行35-37）。
- **临时文件**：忽略 `*.tmp`, `*.pdb`, `temp/`, `asar_check2/`, `asar_restore/`（行40-44）。
- **会话临时产物**：忽略 `preview-shots/`, `target-tmp/`（行47-48，2026-10-02 用户确认）。
- **Runtime data**：忽略 `config.json`（行51）。
- **Release notes**：忽略 `RELEASE_*.md`, `v*.*.*-更新日志.md`（行54-55）。
- **Changelog**：忽略 `CHANGELOG.md`, `changelogs/`（行58-59，仅本地留存）。
- **内部文档**：忽略 `ersikDocumentstrae_projects1`, `docs/`（行62-63）。
- **Build scripts**：忽略 `tauri-app/build.ps1`, `clean-build-cache.ps1`（行67-68）。
- **Helper binary**：忽略 `tauri-app/src-tauri/campus-helper.exe`（行71）。
- **Animation guide / Code wiki**：忽略 `tauri-app/frontend/ANIMATION_GUIDE.md`（行74）、`CODE_WIKI.md`（行77）。
- **tsbuildinfo**：忽略 `*.tsbuildinfo`（行80）。
- **Android 专属**：忽略 `/.android-git-backup/`, `android/*.apk`, `android/*.apk.idsig`, `android/build-apk.ps1`（行83-89）。
- **浏览器实测 mock**：忽略 `mock-tauri*.js`, `.zcode/`（行92-93）。
- **本地产物/设计稿**：忽略 `.tmp-esbuild/`, `.probe/`, `design-prototypes/`, `archx_*/`, `shots/`, `design/`（行97-106）。
- **pnpm 残留**：忽略 `pnpm-lock.yaml`, `pnpm-workspace.yaml`（行109-110）。
- **iOS 壳 crate 构建产物**：注释说明（行112）。

### `AGENTS.md`

面向 AI 编码助手（ZCode 等）与贡献者的项目级约定（行1-27）。架构与模块详解见仓库内 `.codewiki/`；版本变更记录见 `changelogs/`。

- **必守约定 1：Changelog 规范**（行7）：任何改动写入 `changelogs/CHANGELOG_v<版本>.md`，版本号权威源为 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 字段；目标文件 = 当前版本号 + 1，并入最早未发布累积文件，绝不顺延开新版本文件；`changelogs/` 在 `.gitignore` 中仅本地留存。
- **必守约定 2：分支纪律**（行9）：主题分支（`feat/xxx` 等），不直接改 main；commit 前缀 `feat:`/`fix:`/`perf:`/`chore:`/`docs:`；可自行 merge 回 main（`--ff-only`），不擅自 push/删分支。
- **必守约定 3：通用改进双端同步**（行11）：桌面 `tauri-app/` 与安卓 `android/` 前端独立复刻，通用改进必须同一次提交双端各改一份；协议核心单点存在于桌面 crate（Cargo path 依赖），平台专属能力各端自理。
- **必守约定 4：验证以实际输出为准**（行13-17）：
  - 前端：`npx tsc --noEmit --incremental`（禁 `tsc -b`），`npx vite build` 确认无警告（行14）。
  - 后端：`cargo test` 在 `tauri-app/src-tauri`，全绿基线 397 用例（lib 396 + 集成 1，2026-09-22 实测）（行15）。
  - 安卓端：host `cargo check` 失败，只认 `cargo check --target aarch64-linux-android --all-targets` 或 `pwsh android/build-apk.ps1`（行16）。
  - 布局/交互改动需浏览器实测注入 mock，用后还原（行17）。
- **必守约定 5：文档同步**（行19）：同步进 `.codewiki/`，跑 `cw index` 与 `cw meta update`。
- **必守约定 6：语言**（行21）：中文思考/回复/commit/文档，代码命令保持原样。
- **项目速览**（行23-27）：技术栈 Tauri 2 + React 19 + TypeScript，双端校园网助手；发布流程见 CodeWiki `decisions/release-asset-integrity` 与 `decisions/version-single-source-build-rs`。

### `android/.gitignore`

Android 子目录特定忽略规则（行1-17）：

- 忽略 `node_modules/`（行2）、`frontend/dist/`, `src-tauri/target/`（行5-6）。
- 忽略 `*.local`, `build/`, `.tauri/`, `local.properties`, `*.tsbuildinfo`（行9-17）。

### `android/build-apk.ps1`

安卓端一键打包脚本（行1-89）。

- **参数**：`param([switch]$SkipFrontend)`（行6），跳过前端构建。
- **编码设置**：强制 UTF-8 输出（行11-12）。
- **路径**：`$root` 为脚本目录，`$frontendDir`（行15），`$outDir` = `src-tauri\gen\android\app\build\outputs\apk\universal\release`（行16）。
- **JAVA_HOME**：未设置时默认 `C:\Users\ik\AppData\Local\Android\jdk\jdk-17.0.20.1+1`（行19-21），校验有效性（行22-24）。
- **前端构建**：若缺 `node_modules` 则 `npm install`（行29-34）；否则 `npx vite build`（行37-41）。
- **Rust 编译与 APK**：删旧 APK（行45-47），调用 `node "..\tauri-app\node_modules\@tauri-apps\cli\tauri.js" android build --target aarch64 --apk`（行50）。
- **签名校验**：用 `apksigner` 验证（行63-72），未签名则 `zipalign` + `apksigner sign`（行73-87）。

### `make-release.ps1`

一键构建并汇总 GitHub Release 资产脚本（行1-139）。

- **参数**：`param([switch]$SkipBuild)`（行23），跳过编译仅汇总。
- **版本读取**：从 `tauri-app\src-tauri\tauri.conf.json` 的 `version` 字段读取（行44-46）。
- **构建**：若非 SkipBuild，直接 `npx tauri build`（行50-62），不调用 `tauri-app/build.ps1`。
- **cargo target 定位**：从 `.cargo\config.toml` 读取 `target-dir`（行64-72），候选目录取版本匹配的最新 `*-setup.exe`（行80-91）。
- **重命名**：`Wxxy-CampusLogin_${version}_${arch}-setup.exe`（行100-101），生成 `.sha256`（行110-112）。
- **APK 不收集**：脚本明确注释不收集安卓 APK（行115-116），安卓出包走 `android/build-apk.ps1` 单独上传。
- **Release Notes**：复制 `RELEASE_NOTES_v$version.md`（行118-122）。
- **输出**：汇总到 `release/` 目录（行34, 103）。

### `tauri-app/build.ps1`

桌面端 Tauri 构建脚本（行1-212）。

- **参数**：`param([switch]$SkipFrontend, [switch]$SkipCopy, [string]$TargetDir = "")`（行1-5）。
- **环境检查**：`rustc`, `node`, `cargo` 存在性校验（行34-54）。
- **依赖安装**：前端与根 `node_modules` 缺失时 `npm install`（行59-79）。
- **前端构建**：`npm run build`（行84-96），或复用现有 dist（行90-95）。
- **Rust 构建**：设置 `CARGO_BUILD_JOBS` 为 CPU 数（行101-103），默认 `CARGO_TARGET_DIR` 为项目根 `target/`（行105-114），调用 `node $tauriCliJs build` 或 `npx tauri build`（行119-134）。
- **产物复制**：复制到 `tauri-release/`（行153-178），生成 `campus-login.exe` 拷贝（行180-184），为 `*-setup.exe` 生成 `.sha256`（行186-193）。

### `LICENSE` & `LICENSE-ASSETS.md`

- **LICENSE**（行1-27）：MIT License 覆盖源代码与构建脚本；含 Asset Exception（行23-27）声明美术素材不在 MIT 范围内。
- **LICENSE-ASSETS.md**（行1-29）：看板娘插画、应用图标等美术素材“版权所有 (All Rights Reserved)”，禁止单独提取、二次分发、改编、商用、AI 训练（行12-23）；允许项目内部原样使用（行14-16）。

### `README.md`

项目主文档（行1-226）。

- 版本徽章 2.4.3（行5），双端支持（行6）。
- 功能特性：桌面与安卓端详细列表（行37-66）。
- 技术栈表（行70-79）：Tauri 2, React 19, Vite 6, TailwindCSS 3.4, Rust + Tokio 等。
- 项目结构树（行83-131）。
- 开发环境搭建与测试命令（行133-199）：提及后端 `cargo test` 432 个单元测试 + 4 集成测试（行186-187），前端 `npm test` 103 个测试（行193-195）。
- 安全说明（行201-214）。

### `tauri-app/frontend/package.json`

前端配置（行1-51）。

- 名称 `campus-login-frontend`，版本 `2.4.3`（行2-3）。
- 脚本：`dev`, `build`, `preview`, `test`(vitest run), `test:watch`, `clean`（行6-13）。
- 依赖：React 19, Tauri API ^2, Radix UI, Framer Motion ^12.38.0, GSAP ^3.15.0, i18next ^26.3.1, Zustand ^5.0.13 等（行14-36）。
- devDependencies：Vite ^6.0.0, Vitest ^4.1.10, TypeScript ^5.5.0, TailwindCSS ^3.4.10 等（行37-50）。

### `tauri-app/frontend/index.html`

前端入口（行1-118）。

- 骨架屏 CSS 样式 `.sk-*`（行74-87），初始化显示（行90-103）。
- 主题加载：JS 读 `localStorage` 的 `campus-light-mode`, `campus-theme`, `campus-glass-level`（行10-26）。
- 语言兜底：根据 `app-language` 设置骨架屏文案（行104-115）。

### `tauri-app/frontend/about-preview.html`

关于对话框预览（行1-277）。

- 左侧栏图标/标题/版本 v2.4.3（行224-238），检查更新按钮（行230-233）。
- 右侧栏状态圆与特性卡片（行240-271）。

### `tauri-app/frontend/ANIMATION_GUIDE.md`

动效指南（行1-293）。

- Anime.js 组件：`AnimatedNumber`(行7-26), `ElasticBarChart`(行30-53), `StatusPulseBadge`(行57-82)。
- Framer Motion：`AnimatedCard`(行88-104), `DockNav`(行106-115), `Button`(行117-124), `LogPanel`(行127-134)。
- 背景 `FluidBackground`(行140-148)，Dock 毛玻璃(行151-158)。

### `android/README.md`

安卓端文档（行1-27）。

- 结构：`frontend/`, `src-tauri/`, `gen/android/`（行8-10）。
- 构建命令（行14-19）。
- 平台决策：强制 WLAN 绑定、协议核心共享、校园网检测、Keystore（行23-26）。

### `android/CODE_WIKI.md`

安卓端笔记（行1）：自动登录成功通知改应用内 emit `auto-login-result`，删除系统通知，预置 `MONITOR.was_online=true`。

### `target-release-build.log`

历史构建日志（行1-203），展示 v2.4.1 构建过程，调用 `tauri-app/build.ps1`（行9），产物 `校园网登录助手_2.4.1_x64-setup.exe`（行148）重命名为 `Wxxy-CampusLogin_2.4.1_x64-setup.exe`（行178）。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                   Project Root (CampusLogin/)                 │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │ .gitignore   │  │ AGENTS.md    │  │ README.md / LICENSE│  │
│  │ (忽略规则)    │  │ (工作约定)    │  │ (文档/许可)         │  │
│  └──────────────┘  └──────────────┘  └────────────────────┘  │
│                                                              │
│  ┌──────────────────┐  ┌──────────────────┐ ┌──────────────┐ │
│  │ tauri-app/       │  │ android/         │ │ make-release │ │
│  │ build.ps1 (行1)  │  │ build-apk.ps1(行1)│ │ .ps1 (行23)  │ │
│  │ frontend/        │  │ frontend/        │ │ 汇总资产      │ │
│  │  package.json(行1)│  │ src-tauri/       │ │              │ │
│  │ src-tauri/       │  │ gen/android/     │ │              │ │
│  └──────────────────┘  └──────────────────┘ └──────────────┘ │
│        │                      │                   ▲         │
│        └──────► 产物 target/ ──┘── APK ────────────┘         │
│               tauri-release/ ◄── make-release 收集           │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

### 构建与发布流程

1. **开发者触发**：
   - 桌面端：运行 `pwsh tauri-app/build.ps1`（参数 `-SkipFrontend`/`-SkipCopy`/`-TargetDir`，行1-5）。
   - 安卓端：运行 `pwsh android/build-apk.ps1`（参数 `-SkipFrontend`，行6）。
   - 完整发布：运行 `pwsh make-release.ps1`（参数 `-SkipBuild`，行23）。

2. **`tauri-app/build.ps1` 流程**（行1-212）：
   - 检查 rustc/node/cargo（行34-54）。
   - 安装依赖（行59-79）。
   - 构建前端 `npm run build`（行84-96）。
   - 设 `CARGO_TARGET_DIR` 为项目根 `target/`（行105-114）。
   - 执行 `npx tauri build`（行119-134）。
   - 复制 `bundle/` 到 `tauri-release/`（行153-178），生成 `.sha256`（行186-193）。

3. **`android/build-apk.ps1` 流程**（行1-89）：
   - 设 `JAVA_HOME`（行19-24）。
   - 构建前端 `npx vite build`（行37-41）。
   - 执行 `node ..\tauri-app\node_modules\@tauri-apps\cli\tauri.js android build --target aarch64 --apk`（行50）。
   - 输出 APK 至 `src-tauri/gen/android/app/build/outputs/apk/universal/release/`（行16）。

4. **`make-release.ps1` 流程**（行1-139）：
   - 读 `tauri.conf.json` 版本（行44-46）。
   - 若非 `-SkipBuild` 则 `npx tauri build`（行50-62）。
   - 定位 `*-setup.exe` 重命名为 `Wxxy-CampusLogin_<版本>_x64-setup.exe`（行100-101）。
   - 生成 `.sha256`（行110-112）。
   - 不收集 APK（行115-116），复制 `RELEASE_NOTES_v<版本>.md`（行118-122）到 `release/`。

## Connections

- [[root-scripts-config]] -> [[tauri-app-build]] (通过 `tauri-app/build.ps1` 引用)
- [[root-scripts-config]] -> [[android-build]] (通过 `android/build-apk.ps1` 引用)
- [[root-scripts-config]] -> [[release-asset-integrity]] (通过 `make-release.ps1` 和 `LICENSE-ASSETS.md` 引用)
- [[root-scripts-config]] -> [[dual-platform-sharing]] (通过 `AGENTS.md` 双端同步约定引用)

## Known Issues

- `android/build-apk.ps1` 硬编码本地 JDK 路径（行20），换机器需提前设置 `JAVA_HOME` 环境变量。
- `make-release.ps1` 仅支持 Windows 风格路径与 PowerShell，且假设 `tauri.conf.json` 版本与 `Cargo.toml` 一致（由 build.rs 校验）。
- `README.md` 中测试基线（后端 432 单元测试 + 4 集成，前端 103 测试，行186-195）与 `AGENTS.md` 提到的 397 用例基线（行15）不一致，应以 AGENTS.md 与 CodeWiki `decisions/verification-baseline` 为准。
- `.gitignore` 忽略了 `make-release.ps1` 自身（行13），该脚本仅本地保留不入库，团队协作时需额外分发。
