---
title: workspace-docs-and-build
type: module
source_files:
  - docs/research/2026-09-27-windows-outbound-switch-research.md
  - docs/superpowers/plans/2026-09-14-account-adapter-binding.md
  - docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md
  - LICENSE
  - LICENSE-ASSETS.md
  - make-release.ps1
  - README.md
  - changelogs/CHANGELOG_v2.3.6.md
  - changelogs/CHANGELOG_v2.3.8.md
  - changelogs/CHANGELOG_v2.3.9.md
  - changelogs/CHANGELOG_v2.4.1.md
  - .gitignore
  - AGENTS.md
  - build-release.log
  - clean-build-cache.ps1
  - CodeWiki.md
  - docs/changelog-entry-rtss.md
  - docs/insert-rtss-changelog.js
  - release/RELEASE_NOTES_v2.3.6.md
  - release/RELEASE_NOTES_v2.3.7.md
  - release/RELEASE_NOTES_v2.3.8.md
  - release/RELEASE_NOTES_v2.3.9.md
  - release/RELEASE_NOTES_v2.4.0.md
  - release/RELEASE_NOTES_v2.4.1.md
  - release/Wxxy-CampusLogin_2.4.1_x64-setup.exe.sha256
  - target-release-build.log
  - changelogs/_audit-fix-plan.md
  - changelogs/_audit-fix-report-A.md
  - changelogs/_audit-fix-report-B.md
  - changelogs/_audit-fix-report-C.md
  - changelogs/_audit-fix-report-D.md
  - release/RELEASE_NOTES_v2.3.3.md
  - release/RELEASE_NOTES_v2.3.4.md
  - release/RELEASE_NOTES_v2.3.5.md
tags: [构建, 文档, 发布, PowerShell, Tauri, CI/CD]
---

## Overview

本模块「workspace-docs-and-build」并非传统意义上的业务逻辑代码模块，而是负责管理项目**文档资产、版本变更日志、发布说明、审计修复记录**以及**自动化构建与清理脚本**的基础设施层。它定义了从代码提交到最终产物发布的完整生命周期规范，确保 Windows 桌面端（Tauri + Rust）与 Android 端（Kotlin + Rust）的双端同步、构建一致性以及可追溯性。

核心职责包括：
1.  **文档治理**：通过 `AGENTS.md` 和 `CodeWiki.md` 规范 AI 助手与人类的开发纪律，强制 Changelog 本地化与 Wiki 同步。
2.  **构建自动化**：提供 `make-release.ps1` 和 `clean-build-cache.ps1` 处理跨平台（Windows/Android）的编译、打包、SHA256 校验及缓存清理。
3.  **版本追踪**：维护 `changelogs/` 目录下的详细变更记录（如 `_audit-fix-plan.md`）及 `release/` 目录下的用户级发布说明。
4.  **合规与安全**：管理 `LICENSE` 与 `LICENSE-ASSETS.md`，明确代码 MIT 许可与美术素材私有版权的边界。

## Key Components

### 1. 构建与清理工具 (PowerShell)

#### `make-release.ps1`
*   **职责**：执行最终的 Release 构建流程，汇总 GitHub Release 所需的所有资产。
*   **关键实现细节**：
    *   **版本源读取** (Line 38)：解析 `tauri.conf.json` 获取当前版本号。
    *   **前端构建** (Line 42-54)：调用 `npx tauri build`，保留 tty 以显示 Vite/Rust 编译进度条。
    *   **产物定位** (Line 56-77)：动态解析 `.cargo/config.toml` 中的 `target-dir`，在多个候选目录中查找最新的 `*-setup.exe`。
    *   **完整性校验** (Line 91-93)：为安装包生成同名的 `.sha256` 文件，格式 `<hash>  <filename>`。
    *   **APK 收集** (Line 96-110)：递归扫描 `android/` 目录，提取符合版本号的 APK，排除 `node_modules`。
    *   **输出清单** (Line 118-122)：打印 `release/` 目录下所有文件及其大小，供人工核对。

#### `clean-build-cache.ps1`
*   **职责**：安全清理项目中所有可重建的编译缓存、中间产物及 WebView2 运行时缓存。
*   **关键实现细节**：
    *   **清理目标列表** (Line 35-49)：
        *   桌面端：`target/`, `C:\cl-build\target`, `tauri-app\src-tauri\target`, `tauri-app\frontend\dist`, `*.tsbuildinfo`。
        *   安卓端：`android\src-tauri\target`, `android\src-tauri\gen\android\app\build`, `android\frontend\dist` 等。
        *   WebView2 缓存：动态读取 `tauri.conf.json` 中的 `identifier`，定位 `%LOCALAPPDATA%\<identifier>\EBWebView`，删除 `Cache`, `Code Cache`, `GPUCache`, `DawnGraphiteCache`, `DawnWebGPUCache`, `Service Worker`, `GrShaderCache`, `ShaderCache` (Line 78-106)。**注意**：保留 `Local Storage` 和 `Session Storage` 以避免清除用户登录状态。
    *   **运行安全检查** (Line 51-55)：检测是否正在运行 `cargo`, `rustc`, `tauri`, `campus-login` 等进程，若存在则拒绝执行以防止数据损坏。
    *   **预扫描与警告** (Line 57-121)：计算待删文件大小。特别检查 `android\src-tauri\gen\android\app\build\outputs\apk` 下是否存在未在 `release\` 目录中的 APK，若存在则发出红色警告，提示删除后需重跑耗时构建 (Line 113-121)。
    *   **执行模式** (Line 125-133)：支持 `-WhatIf` 预览，`-Confirm:$false` 静默删除，否则交互确认。

### 2. 文档与 Wiki 基础设施

#### `AGENTS.md`
*   **职责**：定义面向 AI 编码助手（ZCode 等）和人类贡献者的项目开发约定、验证基线与工作流纪律。
*   **核心规则**：
    *   **Changelog 管理** (Line 7)：所有改动必须记录在 `changelogs/CHANGELOG_v<版本>.md`。`changelogs/` 在 `.gitignore` 中，仅本地留存。废弃统一 `CHANGELOG.md`。
    *   **分支纪律** (Line 9)：主题分支开发 (`feat/xxx`)，禁止直接改 main。Commit 前缀规范。合并放开 (`--ff-only`)，但禁止 push/删分支。
    *   **双端同步** (Line 11)：Windows 桌面 (`tauri-app/`) 与 Android (`android/`) 前端为独立复刻树。通用改进必须在**同一次提交内**双端各改一份。
    *   **验证基线** (Line 14-17)：
        *   前端：`npx tsc --noEmit --incremental` (禁止 `tsc -b`，避免污染 vite 配置)。涉及打包需 `npx vite build`。
        *   后端 (桌面)：`cargo test` (基线 397 用例)。
        *   后端 (Android)：Host `cargo check/test` 因 mobile-only 插件门控而失败。必须使用 `cargo check --target aarch64-linux-android --all-targets` 或 `pwsh android/build-apk.ps1`。
    *   **文档同步** (Line 19)：新模块/决策/踩坑需更新 CodeWiki。修 bug 写 `learnings/<slug>.md`，做决策写 `decisions/<slug>.md`。收尾跑 `cw index` 和 `cw meta update`。

#### `CodeWiki.md`
*   **职责**：仓库级 CodeWiki 入口文件，提供架构速查、文章导航与维护约定。
*   **关键内容**：
    *   **架构速查** (Line 10-15)：协议单点（桌面 `campus-login` crate 为唯一源，安卓依赖复用）、前端双树（`tauri-app/frontend` 与 `android/frontend` 独立复刻）。
    *   **文章导航** (Line 17-27)：列出 `_architecture.md`, `_index.md`, `modules/` (19篇), `concepts/` (5篇), `decisions/` (40+篇), `learnings/` (60+篇)。
    *   **维护约定** (Line 29-33)：会话开始跑 `cw status`。修 bug/做决策需写对应 wiki 文件。收尾跑 `cw index` 和 `cw meta update`。全量刷新走 `codewiki_generate`。

#### `.gitignore`
*   **职责**：定义版本控制忽略规则，重点排除构建产物、本地配置、IDE 设置及 Android/iOS 专属缓存。
*   **关键项**：
    *   构建输出：`target/`, `dist/`, `release/`, `output/`, `app-output/`, `app-v/`, `tauri-release/` (Line 5-11)。
    *   Tauri 生成代码：`tauri-app/src-tauri/gen/` (Line 19)。
    *   环境文件：`.env`, `.env.test` (Line 22-23)。
    *   IDE 配置：`.trae/`, `.vscode/`, `.idea/` (Line 26-28)。
    *   日志与临时文件：`*.log`, `logs/`, `*.tmp`, `*.pdb`, `temp/`, `asar_check2/`, `asar_restore/` (Line 31-44)。
    *   **本地文档与资产**：`RELEASE_*.md`, `v*.*.*-更新日志.md`, `CHANGELOG.md`, `changelogs/`, `ersikDocumentstrae_projects1`, `docs/` (Line 54-63)。**注意**：`changelogs/` 仅本地留存，不入 git (Line 57-58)。
    *   特定构建脚本与二进制：`tauri-app/build.ps1`, `clean-build-cache.ps1`, `tauri-app/src-tauri/campus-helper.exe` (Line 67-71)。
    *   Android 相关：APK 备份 (`android/*.apk`), 构建脚本 (`android/build-apk.ps1`), Android Git 备份 (`/.android-git-backup/`) (Line 83-89)。
    *   Mock 与探针：`mock-tauri*.js`, `.zcode/`, `.tmp-esbuild/`, `.probe/` (Line 92-98)。
    *   pnpm 残留：`pnpm-lock.yaml`, `pnpm-workspace.yaml` (Line 109-110)，项目使用 `package-lock.json`。

### 3. 发布说明与审计记录 (Markdown)

#### `release/` 目录
包含 v2.3.3 至 v2.4.1 的详细发布说明，记录功能演进、缺陷修复及性能优化。
*   **`RELEASE_NOTES_v2.3.3.md`**：安卓端正式发布，安全体系对齐 (KeyStore AES-GCM, 生物识别验证门)。
*   **`RELEASE_NOTES_v2.3.4.md`**：安卓平板适配，短边 ≥600dp 自动启用桌面 Dock 布局。
*   **`RELEASE_NOTES_v2.3.5.md`**：注销协议重构 (Radius 先行, MAC 解绑, 在线复核, MAC 重置自愈)，WebView2 白屏自动恢复。
*   **`RELEASE_NOTES_v2.3.6.md`**：安卓联网全链路绑定，WiFi 变化秒级响应，2D 人脸验证（可选），非校园网自动退出生效时段。
*   **`RELEASE_NOTES_v2.3.7.md`**：安卓更新链路修复（snake_case/camelCase 解析错误），VPN 场景直连 (SO_BINDTODEVICE)，安卓省电组合拳。
*   **`RELEASE_NOTES_v2.3.8.md`**：桌面轻量化模式（默认开启，点 X 销毁窗口保留后端），晚间断网自动切换运营商。
*   **`RELEASE_NOTES_v2.3.9.md`**：安卓校园网识别增强 (SSID 读取)，修复应用内更新闪退 (栈溢出改为堆分配)。
*   **`RELEASE_NOTES_v2.4.0.md`**：夜间出站自动切换（双端），状态栏在线运营商徽标，安卓安装包减半，桌面零 UAC 提权代理。
*   **`RELEASE_NOTES_v2.4.1.md`**：双端悬浮导航栏 FlClash 形态，桌面适配器下拉显示异常根治，iOS 端开发终止。
*   **`Wxxy-CampusLogin_2.4.1_x64-setup.exe.sha256`**：提供 v2.4.1 Windows 安装包的 SHA256 校验值 `dd75e3988b111b7c9b9ce5c4e966cb1506d130e3dbaa82287573ab9d818fa911`。

#### `changelogs/` 目录 (本地留存)
包含详细的内部变更日志、审计修复计划与报告。
*   **`_audit-fix-plan.md`**：全量审计修复计划，定义 A/B/C/D 四组任务。
    *   **A组 (桌面 Rust)**：删除 `panic = "abort"`，账号操作互斥锁，同步命令移出主线程，下载可取消，DPAPI 加密加 optional_entropy 兼容旧密文。
    *   **B组 (安卓 Rust)**：删除 `panic = "abort"`，延迟巡检三修，配置 IO 锁统一，后台监控八项，digest 语义区分。
    *   **C组 (桌面前端)**：揭示密码 30s 自动清除，假成功簇修复，switch_account 去重，monitor busy 锁，新事件接线。
    *   **D组 (安卓 Kotlin + 前端)**：FGS 启动 try/catch，验证门重写，同构修复，watcherFailed 降级提示。
*   **`_audit-fix-report-A.md`**：A 组落实情况，`cargo test` 全绿 (lib 453 passed)。偏离：A10 测试修错，A17 编译修正。
*   **`_audit-fix-report-B.md`**：B 组落实情况，`cargo check --target aarch64-linux-android` exit=0。偏离：B4a FGS 存活降级，B4g watcher_failed 命名。
*   **`_audit-fix-report-C.md`**：C 组落实情况，`npx tsc --noEmit --incremental` exit 0。偏离：C2 SelfServicePanel 无需改动，C4 失败 toast 超出计划。
*   **`_audit-fix-report-D.md`**：D 组落实情况，Kotlin 无本机编译环境。偏离：update-unverified payload 键名，watcherFailed 驼峰命名。
*   **`CHANGELOG_v2.3.6.md`**：记录 WebView 黑屏自愈，安卓新手引导缺失，安卓 WiFi+流量同开登录失败修复，安卓校园网绑定失败修复。
*   **`CHANGELOG_v2.3.8.md`**：科教中心 SSID 匹配，晚间断网自动切换运营商，夜切后自动验证，桌面 `config-changed` payload 缺包裹修复，后台留存优化。
*   **`CHANGELOG_v2.3.9.md`**：(注：观察批2中提及此文件，但未提供详细内容，主要关联 v2.3.9 的栈溢出修复)。
*   **`CHANGELOG_v2.4.1.md`**：(注：观察批2中提及此文件，主要关联最小化节流修复)。

### 4. 辅助脚本与资源

#### `docs/insert-rtss-changelog.js`
*   **职责**：一次性 Node.js 脚本，将 RTSS 问题条目插入主目录 `CHANGELOG.md` 的 `[Unreleased]` -> `缺陷修复` 段。
*   **关键逻辑**：
    *   **路径硬编码** (Line 4-5)：脚本中使用了绝对路径 `E:/ik/Documents/trae_projects/1/Wxxy-CampusLogin/` 指向 `CHANGELOG.md` 和 `docs/changelog-entry-rtss.md`。这意味着该脚本只能在特定的本地开发环境中运行，不具备跨机器通用性。
    *   **重复性检查** (Line 8-11)：读取目标文件，若已包含“定位 WebView 白屏崩溃根因”字样则跳过。
    *   **锚点插入** (Line 13-16)：定位 `### 缺陷修复` 标题后的第一个换行符，在其后插入条目内容。

#### `docs/changelog-entry-rtss.md`
*   **职责**：RTSS (RivaTuner Statistics Server) 导致 WebView2 白屏崩溃问题的 Changelog 条目草稿。
*   **关键内容**：根因为 RTSS 注入 `RTSSHooks64.dll` 导致 GPU 初始化访问违规。解决方案为启动早期检测 RTSS 并告知用户手动添加排除 profile。

#### `LICENSE` & `LICENSE-ASSETS.md`
*   **职责**：定义代码与美术素材的许可范围。
*   **关键约束**：
    *   **代码**：MIT License。
    *   **美术素材**：版权所有 (All Rights Reserved)。禁止单独提取、再分发、改编、商用及 AI 训练。仅限本项目内部使用。

## Architecture

本模块采用“文档驱动 + 脚本自动化”的架构。核心在于通过 `AGENTS.md` 确立规范，利用 PowerShell 脚本处理异构构建环境（Windows Desktop / Android），并通过 Markdown 文件链式记录从设计调研到最终发布的完整上下文。

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Workspace Docs & Build                       │
├─────────────────────────────────────────────────────────────────────┤
│  [AGENTS.md] ──(定义规范)──► [开发者/AI Agent]                      │
│       │                                                         │
│       ├─(Changelog 本地化)──► [changelogs/*.md]                   │
│       │                                                        │
│       ├─(Wiki 同步)──► [CodeWiki.md] / [learnings/] / [decisions/]│
│       │                                                        │
│       └─(构建指令)──► [make-release.ps1]                         │
│              │                                                   │
│              ├─(读取版本)──► [tauri.conf.json]                    │
│              ├─(执行构建)──► [npx tauri build] / [build-apk.ps1]  │
│              ├─(清理缓存)──► [clean-build-cache.ps1]              │
│              │       │                                           │
│              │       └─(检查进程)──► [cargo/rustc/tauri]          │
│              │       └─(定位 WebView2 缓存)──► [tauri.conf.json]  │
│              │                                                   │
│              └─(产出资产)──► [release/]*                          │
│                      │                                          │
│                      ├─ [RELEASE_NOTES_v*.md]                     │
│                      ├─ [*-setup.exe]                             │
│                      ├─ [*.sha256]                                │
│                      └─ [*.apk]                                   │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **开发阶段**：
    *   开发者/AI Agent 遵循 `AGENTS.md` 规范进行代码修改。
    *   修改涉及双端同步时，需在 `tauri-app/` 和 `android/` 中同时提交变更。
    *   每次提交需更新 `changelogs/CHANGELOG_v<ver>.md` 或 `learnings/` / `decisions/` 目录。
    *   本地运行 `cw index` 和 `cw meta update` 保持 CodeWiki 同步。

2.  **构建阶段**：
    *   触发 `make-release.ps1`。
    *   脚本读取 `tauri.conf.json` 获取版本号。
    *   执行 `npx tauri build` 构建桌面端，或调用 `android/build-apk.ps1` 构建移动端。
    *   脚本解析 `.cargo/config.toml` 定位产物，生成 SHA256 校验文件。
    *   收集所有产物至 `release/` 目录。

3.  **清理阶段**：
    *   触发 `clean-build-cache.ps1`。
    *   脚本检查是否有构建进程运行，若有则中止。
    *   计算待删除缓存大小，检查是否存在未归档的 APK 并发出警告。
    *   执行 `Remove-Item` 清理 `target/`, `dist/`, WebView2 缓存等目录。

## Connections

*   [[campus-login]]: 桌面端核心 Rust 协议库，其构建产物被 `make-release.ps1` 收集。
*   [[android-campus-login]]: 安卓端核心 Rust/Kotlin 实现，其构建产物被 `make-release.ps1` 收集。
*   [[night-outbound-switch]]: 夜间出站切换功能的设计规格 (`specs/...-design.md`) 和实现细节 (`research/...-research.md`) 均位于本模块管理的文档体系中。
*   [[account-adapter-binding]]: 账号适配器绑定功能的实施计划 (`plans/...-binding.md`) 位于本模块管理的文档体系中。
*   [[rtss-compat]]: RTSS 兼容性问题的修复记录 (`changelog-entry-rtss.md`) 位于本模块。

## Known Issues

1.  **Changelog 本地化导致的历史丢失**：`changelogs/` 目录被 `.gitignore` 忽略 (Line 57-58 of `.gitignore`)，意味着详细的内部变更日志仅在开发者本地存在，不随 Git 仓库分发。发布时可能需要特殊流程汇总或仅在本地查看。这可能导致后续审查历史变更时缺乏细粒度记录。
2.  **Android Host Cargo Check 失败**：在主机上运行 `cargo check` 或 `cargo test` 会失败，因为存在 mobile-only 插件的门控逻辑 (Line 16 of `AGENTS.md`)。必须指定 `--target aarch64-linux-android` 或使用 `build-apk.ps1` 脚本。这增加了 CI/CD 配置的复杂性。
3.  **WebView2 缓存清理的用户数据风险**：`clean-build-cache.ps1` 虽然保留了 `Local Storage` 和 `Session Storage` (Line 78-106)，但删除了 `Code Cache` 和 `ShaderCache` 等。如果用户有复杂的 WebView2 状态依赖，清理后可能需要重新加载页面或重新登录，尽管通常影响较小。
4.  **RTSS 注入无法自动修复**：尝试自动写入 RTSS Profile 配置文件以排除应用被注入的方案被证伪 (Line 8-14 of `docs/changelog-entry-rtss.md`)。RTSS 7.3.7 忽略手动放入 Profiles 目录的 cfg 文件。因此，只能检测并告知用户手动操作，用户体验较差。
5.  **构建脚本路径依赖**：`make-release.ps1` 依赖 `.cargo/config.toml` 中的 `target-dir` 来定位产物 (Line 56-77)。如果用户自定义了构建路径，脚本可能无法正确找到安装包，需要手动调整或报告错误。