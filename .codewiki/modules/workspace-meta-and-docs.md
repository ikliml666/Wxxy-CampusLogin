---
title: workspace-meta-and-docs
type: module
source_files:
  - docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md
  - docs/research/2026-09-27-windows-outbound-switch-research.md
  - docs/superpowers/plans/2026-09-14-account-adapter-binding.md
  - .gitignore
  - AGENTS.md
  - CodeWiki.md
  - LICENSE
  - LICENSE-ASSETS.md
  - README.md
  - clean-build-cache.ps1
  - make-release.ps1
  - build-release.log
  - docs/changelog-entry-rtss.md
  - docs/insert-rtss-changelog.js
tags: [meta, docs, build, release, gitignore, changelog, night-switch, outbound-switch, account-adapter]
---

## Overview
模块 `workspace-meta-and-docs` 负责维护仓库根级元数据、工程规范、构建脚本及核心设计文档。它不直接包含业务逻辑代码，而是定义项目的“宪法”与基础设施。该模块涵盖了从 AI 助手工作约定 (`AGENTS.md`)、CI/CD 流程 (`make-release.ps1`, `clean-build-cache.ps1`)、版本控制策略 (`.gitignore`) 到关键功能的技术规格书（夜间出站切换、账号适配器绑定）及事故复盘（RTSS 崩溃）。

## Key Components

### 配置文件与规范

#### `.gitignore`
- **职责**：定义 Git 忽略规则，隔离构建产物、本地配置及内部文档（行 1-112）。
- **关键规则**：
    - **构建产物忽略**：行 5-11（`target/`, `dist/`, `release/`, `output/`, `app-output/`, `app-v/`, `tauri-release/`）；行 13 忽略本地 `make-release.ps1`。
    - **Tauri 生成文件**：行 18-19（`tauri-app/src-tauri/gen/`）。
    - **环境文件**：行 21-23（`.env`, `.env.test`）。
    - **IDE 配置**：行 25-28（`.trae/`, `.vscode/`, `.idea/`）。
    - **日志与临时文件**：行 30-32（`*.log`, `logs/`）；行 39-44（`*.tmp`, `*.pdb`, `temp/`, `asar_check2/`, `asar_restore/`）。
    - **会话临时产物**：行 46-48（`preview-shots/`, `target-tmp/`，注释 2026-10-02 确认）。
    - **运行时数据**：行 50-51（`config.json`）。
    - **发布资产与文档**：行 53-59（`RELEASE_*.md`, `v*.*.*-更新日志.md`, `CHANGELOG.md`, `changelogs/`）；行 61-63（`ersikDocumentstrae_projects1`, `docs/`）。
    - **构建脚本**：行 66-68（`tauri-app/build.ps1`, `clean-build-cache.ps1`）。
    - **辅助二进制**：行 70-71（`tauri-app/src-tauri/campus-helper.exe`）。
    - **内部文档**：行 73-77（`ANIMATION_GUIDE.md`, `CODE_WIKI.md`）。
    - **TypeScript 构建信息**：行 79-80（`*.tsbuildinfo`）。
    - **Android 相关**：行 82-93（`.android-git-backup/`, `android/*.apk`, `android/*.apk.idsig`, `android/build-apk.ps1`, `mock-tauri*.js`, `.zcode/`）。
    - **本地实验残留**：行 95-99（`.tmp-esbuild/`, `.probe/`, `design-prototypes/`, `archx_*/`）；行 102-103（`shots/`）；行 105-106（`design/`）；行 108-110（`pnpm-lock.yaml`, `pnpm-workspace.yaml`）。

#### `AGENTS.md`
- **职责**：定义面向 AI 编码助手与贡献者的项目级工作约定（行 1-27）。
- **关键约定**：
    - **版本记录**：改动必须记入 `changelogs/CHANGELOG_v<版本>.md`，版本号权威源为 `tauri.conf.json`，目标文件=当前版本+1（行 7）。
    - **分支纪律**：主题分支开发，commit 前缀规范，合并放开但禁 push/删分支（行 9）。
    - **双端同步**：通用改进同一次提交双端各改一份，`git diff --stat` 自检；协议单点桌面 crate、平台专属自理（行 11）。
    - **验证流程**：前端 `npx tsc --noEmit --incremental`（禁 `tsc -b`，行 14）；后端 `cargo test` 全绿基线 397 用例（行 15）；安卓 `cargo check --target aarch64-linux-android`（行 16）；布局 mock 注入后须还原（行 17）。
    - **文档同步**：CodeWiki 收尾跑 `cw index` 与 `cw meta update`（行 19）。
    - **语言**：思考/回复/commit/文档一律中文（行 21）。

#### `CodeWiki.md`
- **职责**：CodeWiki 入口速查（行 1-33）。
- **架构速查**：协议单点+平台外壳，安卓 path 依赖（`android/src-tauri/Cargo.toml:34`，行 11）；前端两套独立复刻树（桌面 65 条/安卓 53 条命令，行 12）；双端同步（行 13）。
- **文章导航**：modules 19 篇、concepts 5 篇、decisions 40+ 篇、learnings 60+ 篇（行 19-27）。
- **维护约定**：`cw status` 判定三态，`cw index`/`cw meta update` 随代码提交（行 31-32）。

#### `LICENSE` & `LICENSE-ASSETS.md`
- **LICENSE**：MIT 覆盖源码与构建脚本（行 1-21）；美术素材例外见 LICENSE-ASSETS（行 25-27）；第三方见 THIRD-PARTY-NOTICES（行 28）。
- **LICENSE-ASSETS.md**：资产清单含 `assets/ui-girl/`、`public/girl/`、应用图标等（行 6-10）；权利范围禁止提取/改编/商用/训练 AI（行 14-23）；授权需书面请求（行 26-27）。

#### `README.md`
- **职责**：项目介绍与开发指南（行 1-226）。
- **功能特性**：自动切换运营商（周日至四 23:00 切、次日 7:30 恢复，行 40）；夜间出站切换（断网时段切非校园网卡，次日 7:30 还原，长按拖拽指定，行 41）；安卓端同源对齐（行 62 写 7:30 重登）。
- **技术栈**：Tauri 2 + React 19 + Vite 6 + Rust + Tokio（行 70-79）。
- **测试声明**：后端 432 单测 + 4 集成（行 186-187）；前端 103 测试（行 193-194）。
- **安全说明**：提权四级降级链（管理员→SYSTEM 计划任务→COM 静默→runas，行 212）。

### 构建与发布脚本

#### `clean-build-cache.ps1`
- **职责**：清理可重建的编译缓存与构建中间产物（行 1-134）。
- **参数**：`-WhatIf`（行 26），`-Confirm:$false`（行 28）。
- **逻辑**：进程检查（行 51-55）；计算待删大小（行 57-76）；读 `tauri.conf.json` 定位 WebView2 缓存（行 81-106）；APK 唯一副本告警（行 113-121）；确认后删除（行 123-133）。

#### `make-release.ps1`
- **职责**：构建并汇总 Release 资产到 `release/`（行 1-122）。
- **参数**：`-SkipBuild`（行 15-17）。
- **函数**：`Fail([string]$msg)` 退出报错（行 30-33）。
- **逻辑**：设 UTF-8（行 22-23）；读 `tauri.conf.json` version（行 36-39）；`npx tauri build`（行 42-54）；读 `.cargo/config.toml` target-dir（行 58-64）；重命名 setup.exe + sha256（行 67-93）；收集 APK/NOTES（行 96-116）。

#### `build-release.log`
- **职责**：记录一次 Tauri v2.0.0 构建日志（行 1-73）。
- **内容**：环境 rustc 1.99.1/node v24.19.0（行 16-17）；vite 构建成功（行 23-44）；Tauri beforeBuildCommand 成功（行 46-71）；Rust 编译 `campus-login v2.4.1` 失败（行 72-73）。

### 变更日志与工具

#### `docs/changelog-entry-rtss.md`
- **职责**：RTSS 崩溃根因 changelog 条目（行 1）。
- **根因**：RTSS 注入 `RTSSHooks64.dll` 致访问违规（行 1）。
- **方案**：启动早期检测 RTSS 并留痕（行 1）；证伪自动写 profile（行 1）。
- **验证**：`cargo test` 328 passed（行 1）。

#### `docs/insert-rtss-changelog.js`
- **职责**：一次性插入 RTSS 条目到 `CHANGELOG.md`（行 1-18）。
- **逻辑**：路径常量（行 4-5）；跳过已存在（行 8-11）；查找 `### 缺陷修复` 锚点插入（行 13-16）；写回（行 17-18）。

### 设计文档与研究

#### `docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md`
- **职责**：夜间出站自动切换+网卡排序规格（行 1-117）。
- **配置字段（行 24-32）**：
    - `enableNightOutboundSwitch` (行 28)：默认 `false`，总开关。
    - `outboundPriority` (行 29)：默认 `[]`，桌面网卡有序列表。
    - `outboundMetricRestore` (行 30)：默认 `''`，JSON 快照 `[{guid, family, automatic, metric}]`。
    - `nightOutboundRestore` (行 31)：默认 `''`，安卓切换态标记。
- **纯函数**：`config/outbound_switch.rs` 的 `switch_minute_for(weekday)` (行 37)，`evaluate_night_outbound(...)` (行 38)。
- **桌面动作**：`campus_check.rs` 判定（行 43）；Iphlpapi `SetIpInterfaceEntry` 设 Metric=1（行 45）；`HelperOp::SetMetric` 变体（行 46）；快照时序（行 47）。
- **还原三重保障**：重启还原/06:30 主动还原/启动对账（行 49-53，恢复窗口 06:30~23:00 见行 14）。
- **UI**：NetworkPanel 新卡，↑/↓ 排序，出站徽标（行 61-65）。
- **安卓动作**：`full_logout` + `report_wifi_unusable`（行 73-74）；`network_avoid_bad_wifi` 引导（行 83）；已知风险（行 94-96）。

#### `docs/research/2026-09-27-windows-outbound-switch-research.md`
- **职责**：Windows 出站切换底层调研（行 1-424）。
- **结论**：同类项目多用 Disable-NetAdapter（行 17）；改 metric 对 TUN 无效（行 18, 121-134）；静默提权无长期 API（行 19, 262）；CM_Disable_DevNode 默认不跨重启（行 21, 318-320）。
- **实证项目**：NMAR（行 27-37）、PSActiveNetworkSwitcher（行 39-62，依赖 ifIndex 有隐患）、nadap-switch（行 64-72）、windows-sleep-network-fix（行 74-82）。
- **TUN 抢占**：mihomo Issue #3186（RouteMetric=0 压制，行 129-131）；WireGuard commit 66ad537f 相加证实（行 132）。
- **可恢复性**：三层兜底（主动还原/启动对账/看门狗，行 328-332）；先恢复受限再恢复全网（行 336）。
- **启示**：禁用网卡路线经得起论证（行 407）。

#### `docs/superpowers/plans/2026-09-14-account-adapter-binding.md`
- **职责**：账号体系增强实施计划 R1-R4（行 1-241）。
- **数据契约（行 35-78）**：
    - `display_name` (行 41)：默认 `""`，显示名。
    - `adapter1_account` (行 42)：默认 `""`，主适配器账号 id。
    - `adapter2_account` (行 43)：默认 `""`，副适配器账号 id。
    - `list_accounts` 返回 `Vec<AccountItem>`（行 52）；`rename_account` 新增（行 53）；`switch_account` 补 `activeAccount`（行 54）。
    - `AccountItem` struct（行 58-64）：`id: String`, `display_name: String`。
- **行为规则**：账号 id sanitize（行 86）；`save_config` 后自动建号（行 91-100）；R4 前端守卫改 `!== undefined`（行 115）；适配器指定账号优先级（行 122-129）。
- **双端同步清单**：适配器指定账号仅桌面（行 214）。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Workspace Meta & Docs                   │
├─────────────────────────────────────────────────────────────┤
│  [AGENTS.md] ──► [AI Assistants / Contributors]             │
│       ▲                                                     │
│       │ 指引                                                  │
│  [CodeWiki.md] ──► [Module Index / Arch Overview]           │
│       ▲                                                     │
│       │ 入口                                                  │
├─────────────────────────────────────────────────────────────┤
│  [Build Scripts]                                              │
│  ├─ make-release.ps1 ──► [Release Assets]                    │
│  └─ clean-build-cache.ps1 ──► [Clean Targets]                │
├─────────────────────────────────────────────────────────────┤
│  [Design Specs & Research]                                    │
│  ├─ night-outbound-switch-design.md ──► [Desktop/Android]   │
│  ├─ windows-outbound-switch-research.md ──► [Win Low-Level] │
│  └─ account-adapter-binding.md ──► [Account System]         │
├─────────────────────────────────────────────────────────────┤
│  [Changelog Tools]                                            │
│  ├─ changelog-entry-rtss.md                                  │
│  └─ insert-rtss-changelog.js ──► [CHANGELOG.md]             │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **构建与发布流**：
    * `make-release.ps1` 读取 `tauri.conf.json` 获取版本号（行 36-39）。
    * 执行 `npx tauri build`（行 42-54），触发 Vite 与 Rust 编译。
    * 重命名 `*-setup.exe` 为 `Wxxy-CampusLogin_<版本>_<架构>-setup.exe` 并生成 `.sha256`（行 67-93），汇总至 `release/`（行 118-122）。
    * `clean-build-cache.ps1` 独立扫描 `target/`、`dist/`、WebView2 缓存（行 8-20, 81-106），检测构建进程则安全退出（行 51-55）。

2. **变更日志流**：
    * 编写 `docs/changelog-entry-rtss.md`（行 1）。
    * 运行 `docs/insert-rtss-changelog.js`，查找 `### 缺陷修复` 锚点插入（行 13-16），写回 `CHANGELOG.md`（行 17-18）。

3. **设计决策流**：
    * `windows-outbound-switch-research.md` 提供底层约束（metric 陷阱行 125-134，提权路径行 262），影响 `night-outbound-switch-design.md` 实现（如使用 GUID 行 46、Iphlpapi 行 45）。
    * `account-adapter-binding.md` 定义 `Config` 扩展字段（行 41-43），与出站切换配置共存于双端配置结构。

## Connections
* [[modules/workspace-config]]：`night-outbound-switch-design.md` 中 `config/outbound_switch.rs`、`config/night_switch.rs` 属配置模块；`account-adapter-binding.md` 中 `display_name` 等字段增于 `Config`。
* [[modules/workspace-desktop-monitor]]：桌面端 `monitor/scheduled.rs` 30s 循环调用 `evaluate_night_outbound`（设计文档行 39）。
* [[modules/workspace-android-monitor]]：安卓端 `monitor_loop.rs::run_scheduled_actions` 调用 `evaluate_night_outbound`（设计文档行 39）。
* [[modules/workspace-auth-service]]：`account-adapter-binding.md` 中 `auth/service.rs::full_login` 凭据副本准备（行 129）。
* [[modules/workspace-ui-network-panel]]：设计文档 `NetworkPanel` 新卡（行 61-65）与计划文档 `NetworkPanel.tsx` 适配器下拉（行 168）。
* [[modules/workspace-protocol-core]]：`CodeWiki.md` 指出协议实现单点桌面 crate，安卓 path 依赖（行 11）。

## Known Issues

1. **`.gitignore` 与 `docs/` 目录一致性**：`.gitignore` 行 63 忽略 `docs/`，但 `docs/changelog-entry-rtss.md` 与 `docs/insert-rtss-changelog.js` 在提供文件列表中，可能为本地未跟踪或历史遗留，需注意文档入库策略。
2. **测试基线文档不一致**：`AGENTS.md` 行 15 记 `cargo test` 基线 397 用例（2026-09-22）；`README.md` 行 186-187 声明 432 单测+4 集成；`docs/changelog-entry-rtss.md` 行 1 记 328 passed。各文件基线数值矛盾，应以 CodeWiki `decisions/verification-baseline` 为权威。
3. **恢复时间窗文档分歧**：设计文档 `night-outbound-switch-design.md` 行 14 定义恢复窗口 06:30~23:00；`README.md` 行 40-41、62 描述次日 7:30 起还原/重登。双端文档未对齐。
4. **出站切换方案潜在缺陷**：设计文档行 45 采用 Iphlpapi 改接口 metric（Metric=1），但调研报告行 125-149 证明 TUN/VPN 存在时合成 metric 被 RouteMetric=0 压制、仅禁用网卡可靠（行 407 结论）。设计文档尚未根据调研更新为禁用路线。
5. **安卓端切换非确定性**：安卓依赖系统 `MultinetworkPolicyTracker` 探测周期（设计文档行 95），非精确 23:00 整点切换。
6. **账号 ID 与 Name 分离复杂度**：`account-adapter-binding.md` 行 107-108 改名不改文件名与 `active_account`，前端须正确处理显示名回退与事件同步。
