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
- **职责**：定义 Git 忽略规则，隔离构建产物、本地配置及内部文档。
- **关键实现**：
    - **构建产物忽略**：`target/`, `dist/`, `release/`, `output/`, `app-output/`, `app-v/`, `tauri-release/` (行 5-11)。
    - **Tauri 生成文件**：`tauri-app/src-tauri/gen/` (行 19)。
    - **环境文件**：`.env`, `.env.test` (行 22-23)。
    - **IDE 配置**：`.trae/`, `.vscode/`, `.idea/` (行 26-28)。
    - **日志与临时文件**：`*.log`, `logs/`, `*.tmp`, `*.pdb`, `temp/`, `asar_check2/`, `asar_restore/` (行 31-44)。
    - **会话临时产物**：`preview-shots/`, `target-tmp/` (行 47-48)，注释说明 2026-10-02 用户确认清理并忽略。
    - **运行时数据**：`config.json` (行 51)。
    - **发布资产与文档**：`RELEASE_*.md`, `v*.*.*-更新日志.md`, `CHANGELOG.md`, `changelogs/`, `ersikDocumentstrae_projects1`, `docs/` (行 54-63)，均为本地留存，不随仓库分发。
    - **构建脚本**：`tauri-app/build.ps1`, `clean-build-cache.ps1` (行 67-68)。
    - **辅助二进制**：`tauri-app/src-tauri/campus-helper.exe` (行 71)。
    - **内部文档**：`ANIMATION_GUIDE.md`, `CODE_WIKI.md` (行 74-77)。
    - **TypeScript 构建信息**：`*.tsbuildinfo` (行 80)。
    - **Android 相关**：`.android-git-backup/`, `android/*.apk`, `android/*.apk.idsig`, `android/build-apk.ps1`, `mock-tauri*.js`, `.zcode/` (行 83-93)。
    - **本地实验残留**：`.tmp-esbuild/`, `.probe/`, `design-prototypes/`, `archx_*`, `shots/`, `design/`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `ios/src-tauri/.cargo/config.toml` 指定 iOS 壳 crate 构建产物 (行 97-111)。

#### `AGENTS.md`
- **职责**：定义面向 AI 编码助手（ZCode 等）与贡献者的项目级工作约定。
- **关键实现**：
    - **版本记录**：改动必须记入 `changelogs/CHANGELOG_v<版本>.md`，默认内容存放版本号 = 当前版本号 + 1（行 7）。`changelogs/` 在 `.gitignore` 中，仅本地留存。
    - **分支纪律**：全在主题分支（`feat/xxx` 等），不直接改 main；commit 前缀规范；合并放开但禁止 push/删分支（行 9）。
    - **双端同步**：通用改进必须在同一次提交内双端各改一份（桌面 `tauri-app/` + 安卓 `android/`），`git diff --stat` 自检（行 11）。例外：协议实现单点存在于桌面 crate，安卓经 Cargo path 依赖自动继承；平台专属能力各端自理。
    - **验证流程**：
        - 前端：`npx tsc --noEmit --incremental`（禁止 `tsc -b`），`npx vite build`（行 14）。
        - 后端：`cargo test`（全绿基线 397 用例，2026-09-22 实测）（行 15）。
        - 安卓端：host `cargo check` 失败，需 `cargo check --target aarch64-linux-android --all-targets` 或 `pwsh android/build-apk.ps1`（行 16）。
        - 布局/交互：向 `index.html` 注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，用后必须完整还原（行 17）。
    - **文档同步**：新模块、决策变更、踩坑记录同步进 CodeWiki（`.codewiki/`），收尾跑 `cw index` 和 `cw meta update`（行 19）。
    - **语言**：思考、回复、commit 说明、文档一律中文（行 21）。

#### `CodeWiki.md`
- **职责**：CodeWiki 入口速查，介绍架构速查、文章导航与维护约定。
- **关键实现**：
    - **架构速查**：
        - 协议单点 + 平台外壳：登录/注销/Portal 探测/自助服务/网络质量等协议实现只存在于桌面协议核心 crate（`tauri-app/src-tauri`，package `campus-login`），安卓后端以 Cargo path 依赖复用（`android/src-tauri/Cargo.toml:34`），禁止复制；平台专属模块用 `#[cfg(desktop)]` 门控（行 11）。
        - 前端是两套独立复刻树（`tauri-app/frontend` 与 `android/frontend`），靠同形的 `tauriApi.ts` 接口与同名的 IPC 命令保持行为一致（桌面 65 条命令、安卓 53 条，三处刻意异名由转调抹平）（行 12）。
        - 通用改进双端同步（行 13）。
    - **文章导航**：总览 `_architecture.md`，索引 `_index.md`，模块 `modules/`（19 篇），概念 `concepts/`（5 篇），决策 `decisions/`（40+ 篇），踩坑 `learnings/`（60+ 篇），模式 `_patterns.md`（行 19-27）。
    - **维护约定**：会话开始跑 `cw status`；修 bug 写 `learnings/<slug>.md`，做决策写 `decisions/<slug>.md`；收尾 `cw index` 和 `cw meta update`（行 31-32）。编译/刷新走 `codewiki_generate`（行 33）。

#### `LICENSE` & `LICENSE-ASSETS.md`
- **职责**：定义代码与美术素材的许可范围。
- **关键实现**：
    - **LICENSE**：MIT License，覆盖源代码与构建脚本（行 1-21）。例外声明：美术素材不在本许可授予范围内，详见 `LICENSE-ASSETS.md`（行 25-27）。第三方组件许可见 `THIRD-PARTY-NOTICES.md`（行 28）。
    - **LICENSE-ASSETS.md**：
        - **资产清单**：`assets/ui-girl/`（含 `original/`），`tauri-app/frontend/public/girl/`，`android/frontend/public/girl/`，应用图标（`32x32.png`, `128x128.png` 等），收款码图片，`tauri-app/src-tauri/icons/`，`android/src-tauri/icons/`（含托盘图标 `tray-icon.png`）（行 6-10）。
        - **权利范围**：允许在本项目源码副本与官方发行版内部原样使用；禁止单独提取、复制再分发、改编、商用、训练机器学习/AI 生成模型（行 14-23）。
        - **授权请求**：联系作者获取书面许可（行 26-27）。

#### `README.md`
- **职责**：项目介绍、功能特性、技术栈、项目结构、开发环境搭建、安全说明。
- **关键实现**：
    - **功能特性**：一键登录/注销、自动重连、自动切换运营商、夜间出站切换、校园网检测、DNS 智能解析、网络质量监测、双适配器支持、在线运营商显示、多账号管理、运营商账号绑定、Windows Hello 验证、自助服务查询、总览自定义卡片、主题定制、系统托盘、开机自启、日志管理、用户自助服务、中英语言切换（行 37-58）。
    - **安卓端功能**：与桌面端共享同一 Rust 协议核心，功能同名对齐，平台差异：系统托盘、Windows Hello、DNS 优化、双适配器/有线管理、出站徽标与手动切换为桌面专属，人脸录入为安卓专属；安卓端登录流量强制绑定 WLAN（`bindProcessToNetwork`，不走蜂窝数据）（行 60-66）。
    - **技术栈**：Tauri 2, React 19, TypeScript, Vite 6, TailwindCSS 3.4, Framer Motion 12, GSAP 3, Rust, Tokio, reqwest 0.12, tokio-rustls 0.26, hickory-resolver 0.24, Windows DPAPI / AndroidKeyStore AES-256-GCM, react-i18next + i18next-browser-languagedetector（行 70-79）。
    - **项目结构**：详细列出 `tauri-app/` 和 `android/` 下的目录结构，包括前端组件、后端模块、插件等（行 83-131）。
    - **开发环境搭建**：前置要求 Node.js >= 18, Rust (stable), Tauri 2 CLI；安装依赖、开发模式、构建发布、安卓构建、测试（行 133-199）。
    - **安全说明**：密码加密存储、登录/注销流量绑定 WLAN、应用内更新校验、敏感操作生物识别验证、HTTP TLS 1.3、DoH RFC 8484、更新安装包 SHA256 校验、提权四级降级链、账号名校验防止路径遍历、外部链接白名单（行 202-214）。

### 构建与发布脚本

#### `clean-build-cache.ps1`
- **职责**：清理构建缓存与中间产物，不碰源码、发布资产、版本归档与依赖。
- **关键实现**：
    - **清理目标**：`target/`, `C:\cl-build\target`, `tauri-app\src-tauri\target`, `android\src-tauri\target`, `android\src-tauri\gen\android\app\build`, `android\src-tauri\gen\android\.gradle`, `android\src-tauri\gen\android\build`, `tauri-app\frontend\dist`, `android\frontend\dist`, `*.tsbuildinfo`, WebView2 前端运行时缓存（只删缓存子目录，保留 Local Storage 等用户数据）（行 8-20）。
    - **参数**：`-WhatIf`（只看会删什么）、`-Confirm:$false`（不确认直接删）（行 26-28）。
    - **执行逻辑**：
        - 检查构建进程或本应用在运行，如有则退出（行 51-55）。
        - 计算待删除文件大小，输出预览（行 57-76）。
        - 读取 `tauri.conf.json` 中的 `identifier`，定位 WebView2 缓存目录，计算大小（行 81-106）。
        - 检查 APK 唯一副本告警：`gen` 下有、`release\` 没有的 APK，警告后继续（行 113-121）。
        - 输出合计大小，交互确认后删除（行 123-133）。

#### `make-release.ps1`
- **职责**：构建并汇总 GitHub Release 资产到 `release/`。
- **关键实现**：
    - **参数**：`-SkipBuild`（不编译，仅从已有产物汇总）（行 16）。
    - **执行逻辑**：
        - 设置 UTF-8 编码（行 22-23）。
        - 读取 `tauri.conf.json` 中的 `version`（行 36-39）。
        - 如果未跳过构建，执行 `npx tauri build`（行 42-54）。
        - 读取 `.cargo/config.toml` 中的 `target-dir`，确定 cargo 实际输出目录（行 58-64）。
        - 查找最新的 `*-setup.exe`，重命名为 `Wxxy-CampusLogin_<版本>_<架构>-setup.exe`，生成 `.sha256` 文件（行 67-93）。
        - 收集已构建的 APK 与 `RELEASE_NOTES_v<版本>.md`（行 96-116）。
        - 输出 Release assets ready 信息（行 118-122）。

#### `build-release.log`
- **职责**：记录一次完整的 Tauri v2.0.0 构建过程日志。
- **关键实现**：
    - **环境检查**：rustc 1.99.1, node v24.19.0（行 16-17）。
    - **依赖安装**：跳过（行 20-21）。
    - **前端构建**：vite v6.4.2 构建成功，耗时 4.41s（行 23-44）。
    - **Tauri 构建**：使用 24 build jobs，custom target dir，执行 `beforeBuildCommand` 前端构建，耗时 4.24s（行 46-71）。
    - **Rust 编译**：`Compiling campus-login v2.4.1`，随后失败（行 72-73）。

### 变更日志与工具

#### `docs/changelog-entry-rtss.md`
- **职责**：记录 RTSS 崩溃根因定位及解决方案的 changelog 条目。
- **关键实现**：
    - **根因**：RivaTuner Statistics Server（MSI Afterburner 组件）向 WebView2 子进程注入 RTSSHooks64.dll，导致访问违规（行 4-6）。
    - **解决方案**：启动极早期检测 RTSS 安装并暂存结果，logger 就绪后在 startup 留痕（行 7-8）。
    - **证伪尝试**：自动写排除 profile 并证伪，RTSS 7.3.7 忽略手放入 Profiles 目录的 cfg 文件，排除只能在 RTSS 界面手动完成（行 9-14）。
    - **验证**：`cargo test` 328 passed / 0 failed（含 rtss_compat 3 个新用例）（行 15-16）。

#### `docs/insert-rtss-changelog.js`
- **职责**：一次性脚本，将 RTSS 崩溃根因条目插入主目录 CHANGELOG.md 的 [Unreleased] → 缺陷修复段。
- **关键实现**：
    - 读取目标文件和条目文件（行 4-5, 7-12）。
    - 检查条目是否已存在，若存在则跳过（行 8-11）。
    - 查找 `### 缺陷修复` 锚点，插入条目（行 13-16）。
    - 写入文件并输出 OK（行 17-18）。

### 设计文档与研究

#### `docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md`
- **职责**：定义「夜间出站自动切换 + 出站网卡排序」功能的完整技术规格。
- **核心背景与目标**：解决无锡学院校园网夜间断网问题，新增出站层自动切换功能（行 6-15）。
    - **桌面端**：通过修改网卡跃点数（metric），将非校园网网卡设为系统出站网卡；次日清晨自动还原（行 6-15）。
    - **安卓端**：到点注销校园网认证，触发系统 WiFi 连通性重检，利用`network_avoid_bad_wifi`机制切至蜂窝网络（行 6-15）。
- **配置字段设计**：
    - `enableNightOutboundSwitch` (`false`)：总开关（行 24-32）。
    - `outboundPriority` (`[]`)：桌面端专用，网卡友好名有序列表（行 24-32）。
    - `outboundMetricRestore` (`''`)：桌面端专用，JSON快照 `[ {guid, family, automatic, metric} ]`（行 24-32）。
    - `nightOutboundRestore` (`''`)：安卓端专用，暂存账号名（行 24-32）。
- **判定逻辑与跨平台模块**：
    - 新模块：`config/outbound_switch.rs`（Rust）（行 33-40）。
    - 复用逻辑：从 `config/night_switch.rs` 抽出共用时间表函数 `switch_minute_for(weekday)`（行 33-40）。
    - 评估函数：`evaluate_night_outbound(enabled, weekday, now_minutes, restore_snapshot) -> Switch | Restore | None`（行 33-40）。
- **桌面端详细实现**：
    - 校园网判定：`campus_check.rs`（行 41-66）。
    - Metric修改：使用Iphlpapi `GetIpInterfaceEntry`/`SetIpInterfaceEntry`，`SitePrefixLength` 须置0以防 `ERROR_INVALID_PARAMETER` (行 45)（行 41-66）。
    - 提权机制：`HelperOp` 新增 `SetMetric` 变体，走 `spawn_elevated_helper`（行 41-66）。
    - 三重还原保障：系统重启自动还原、06:30恢复窗口主动还原、启动对账（行 41-66）。
- **安卓端详细实现**：
    - 机制：复用 `full_logout` 链路，新Kotlin插件方法 `report_wifi_unusable`（行 67-97）。
    - 风险：Captive Portal白名单误判、切换时机非秒级（行 67-97）。

#### `docs/research/2026-09-27-windows-outbound-switch-research.md`
- **职责**：针对“夜间出站切换”功能的 Windows 底层机制调研。
- **关键事实**：
    - **合成 Metric 公式**：有效跃点 = RouteMetric + InterfaceMetric。同前缀间最小者胜。若 TUN 的 RouteMetric=0，改物理卡无效（行 138-141）。
    - **禁用网卡的唯一性**：产生路由事件迫使 TUN 重选；`CM_Disable_DevNode` 默认不跨重启（天然兜底），但 `SetupDiCallClassInstaller` 默认跨重启（危险）（行 145-150, 318-320）。
    - **ifIndex 漂移**：禁用再启用后 ifIndex 会变，脚本不可缓存 ifIndex，必须用 GUID/LUID/PnPInstanceId（行 150, 380-382）。
    - **提权路径**：UAC 无长期复用 API；`autoElevate` 仅限微软内部签名；计划任务 `/ru SYSTEM` 或 `HighestAvailable` 是最佳路径（行 216-283）。SYSTEM 进程写回用户可见路径需显式 ACL（行 245）。
    - **同类项目缺陷**：NMAR 全量开关易误伤；PSActiveNetworkSwitcher 依赖 ifIndex 且无还原看门狗（行 30-37, 46-62）。
    - **TUN 干扰实证**：mihomo Issue #3186 显示 TUN 自建明细路由 RouteMetric=0 压制物理卡；`IP_UNICAST_IF` 绑定在 strong host model 下可能失效（行 129-131, 206-208）。
    - **恢复设计**：三层兜底——主动还原、启动对账（三快照+失败计数）、看门狗（重启自愈）（行 328-332）。还原必须幂等，且顺序为“先恢复受限状态，再恢复全网路由”（行 335-336）。

#### `docs/superpowers/plans/2026-09-14-account-adapter-binding.md`
- **职责**：账号体系增强实施计划（R1-R4），定义双端数据契约、行为规则及工作单元分解。
- **关键实现**：
    - **数据契约**：`Config` 新增字段：`display_name` (String, default ""), `adapter1_account` (String, default ""), `adapter2_account` (String, default "")（行 39-46）。
    - **IPC 命令变更**：`list_accounts` 返回类型变为 `Vec<AccountItem>`；`rename_account` 新增；`switch_account` 补 `activeAccount` 字段（行 50-55）。
    - **账号 ID 生成与自动建号**：ID 由用户名 sanitize 而来（正则外字符换 `_`，截断 32 位）；触发点：`save_config` 落盘成功后调用内部 helper（行 86-100）。
    - **适配器指定账号逻辑 R1**：优先级：解析适配器角色 -> 查 `adapter1/2_account` -> 读对应账号文件凭据 -> 失败回退当前配置（行 122-129）。
    - **切换刷新修复 R4**：后端返回 `activeAccount`；前端守卫 `handleSwitchAccount` 判断改为 `!== undefined`（行 113-118）。

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

1.  **构建与发布流**：
    *   `make-release.ps1` 被调用时，首先读取 `tauri.conf.json` 获取版本号（行 36-39）。
    *   若未跳过构建，执行 `npx tauri build`（行 42-54），触发前端 Vite 构建和 Rust 编译。
    *   构建产物（如 `*-setup.exe`）被重命名并生成 `.sha256` 校验文件，最终汇总至 `release/` 目录（行 67-93）。
    *   `clean-build-cache.ps1` 可独立调用，扫描 `target/`、`dist/` 及 WebView2 缓存目录，并在检测到构建进程运行时安全退出（行 51-55）。

2.  **变更日志流**：
    *   开发者编写 `docs/changelog-entry-rtss.md`（行 4-6）。
    *   运行 `docs/insert-rtss-changelog.js`，该脚本读取条目文件，查找 `CHANGELOG.md` 中的 `### 缺陷修复` 锚点（行 13-16），并将内容插入其中（行 17-18）。

3.  **设计决策流**：
    *   `windows-outbound-switch-research.md` 提供了底层机制约束（如 Metric 陷阱、提权路径），直接影响 `night-outbound-switch-design.md` 的具体实现方案（如选择 `SetIpInterfaceEntry` 而非禁用网卡，使用 `GUID` 而非 `ifIndex`）。
    *   `account-adapter-binding.md` 定义了 `Config` 结构的扩展，这些字段将被 `night-outbound-switch-design.md` 中的 `outboundPriority` 等配置所引用或共存。

## Connections

*   [[modules/workspace-config]]：`night-outbound-switch-design.md` 中提到的 `config/outbound_switch.rs` 和 `config/night_switch.rs` 属于配置模块。
*   [[modules/workspace-desktop-monitor]]：桌面端监控逻辑 `monitor/scheduled.rs` 调用 `evaluate_night_outbound`。
*   [[modules/workspace-android-monitor]]：安卓端监控逻辑 `monitor_loop.rs` 调用 `evaluate_night_outbound`。
*   [[modules/workspace-auth-service]]：`account-adapter-binding.md` 中提到的 `auth/service.rs::full_login` 和 `load_account_config` 属于认证服务模块。
*   [[modules/workspace-ui-network-panel]]：`night-outbound-switch-design.md` 中提到的 `NetworkPanel` UI 组件以及 `account-adapter-binding.md` 中提到的 `NetworkPanel.tsx` 属于 UI 模块。
*   [[modules/workspace-protocol-core]]：`CodeWiki.md` 指出协议实现单点存在于桌面协议核心 crate，安卓后端以 Cargo path 依赖复用。

## Known Issues

1.  **`.gitignore` 与 `docs/` 目录一致性**：
    *   `.gitignore` 第 63 行忽略了 `docs/` 目录，这意味着整个 `docs/` 目录及其内容都不应被 Git 跟踪。然而，`docs/changelog-entry-rtss.md` 和 `docs/insert-rtss-changelog.js` 却在提供的文件列表中。这可能是一个不一致之处，需要进一步调查这两个文件是否是通过其他方式管理的，或者 `.gitignore` 规则生效前的遗留问题。
2.  **Windows Metric 修改陷阱**：
    *   根据 `windows-outbound-switch-research.md`，修改物理卡 Metric 无法压制 TUN（RouteMetric=0）（行 121-123, 129-131）。虽然 `night-outbound-switch-design.md` 选择了 Metric 方案，但需警惕 TUN 干扰。
3.  **安卓端切换非确定性**：
    *   安卓端切换依赖于系统 `MultinetworkPolicyTracker` 的探测周期，因此切换不是精确的 23:00 整点，而是有一定延迟（行 95）。
4.  **提权路径复杂性**：
    *   Windows 提权推荐计划任务代理，但 SYSTEM 进程写回用户可见路径需显式 ACL（行 245），增加了实现复杂度。
5.  **账号 ID 与 Name 分离**：
    *   `account-adapter-binding.md` 指出改名不改文件名，不改 `active_account` 指向（行 107-108），需确保前端正确处理此逻辑。