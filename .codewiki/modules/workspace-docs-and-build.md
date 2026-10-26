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
  - changelogs/_audit-fix-plan.md
  - changelogs/_audit-fix-report-A.md
  - changelogs/_audit-fix-report-B.md
  - changelogs/_audit-fix-report-C.md
  - changelogs/_audit-fix-report-D.md
  - .gitignore
  - AGENTS.md
  - build-release.log
  - clean-build-cache.ps1
  - CodeWiki.md
  - docs/changelog-entry-rtss.md
  - docs/insert-rtss-changelog.js
  - release/RELEASE_NOTES_v2.3.3.md
  - release/RELEASE_NOTES_v2.3.4.md
  - release/RELEASE_NOTES_v2.3.5.md
  - release/RELEASE_NOTES_v2.3.6.md
  - release/RELEASE_NOTES_v2.3.7.md
  - release/RELEASE_NOTES_v2.3.8.md
  - release/RELEASE_NOTES_v2.3.9.md
  - release/RELEASE_NOTES_v2.4.0.md
  - release/RELEASE_NOTES_v2.4.1.md
  - release/Wxxy-CampusLogin_2.4.1_x64-setup.exe.sha256
  - target-release-build.log
  - README.md
tags: [构建, 文档, 发布, PowerShell, Tauri, CI/CD]
---

## Overview

本模块「workspace-docs-and-build」不是业务逻辑模块，而是管理项目**文档资产、版本变更日志、发布说明、审计修复记录**与**自动化构建/清理脚本**的基础设施层。它定义了从代码提交到最终产物发布的完整生命周期规范，确保 Windows 桌面端（Tauri 2 + Rust，根 `tauri-app/`）与 Android 端（Kotlin + Rust，根 `android/`）的双端同步、构建一致性与可追溯性。

核心职责：

1. **文档治理**：`AGENTS.md`（6 条必守约定）与 `CodeWiki.md` 规范 AI 助手与人类的开发纪律，强制 changelog 本地化（`changelogs/` 不入 git）与 CodeWiki（`.codewiki/`）同步。
2. **构建自动化**：`make-release.ps1` 读取 `tauri.conf.json` 版本号、执行 `npx tauri build`、定位并重命名 NSIS 安装包、生成 `.sha256`、收集 Release Notes（明确不收集安卓 APK，仅负责 Windows 桌面端）；`clean-build-cache.ps1` 安全清理全部编译缓存与 WebView2 运行时缓存。
3. **版本追踪**：`changelogs/` 维护按版本文件的变更日志与审计修复计划/报告 `_audit-fix-*.md`，`release/` 维护 v2.3.3–v2.4.1 共 9 份用户级发布说明与 v2.4.1 校验文件。
4. **设计调研沉淀**：`docs/` 下三份长文档——夜间出站切换调研报告、账号适配器绑定实施计划、夜间出站切换设计规格。
5. **合规与安全**：`LICENSE`（源码 MIT + 美术素材例外）与 `LICENSE-ASSETS.md`（素材 All Rights Reserved）划定双轨许可边界。

## Key Components

### 1. 构建与清理工具（PowerShell）

#### `make-release.ps1`

* **参数**：`[switch]$SkipBuild`（23|–25|）——只汇总不编译。
* **编码设置**（29|–31|）：`[Console]::OutputEncoding`/`$OutputEncoding` 置 UTF-8，避免 GBK 终端把 `✓` 显示成 `?`。
* **版本源读取**（43|–48|）：`$confPath = Join-Path $srcTauriDir "tauri.conf.json"` → `Get-Content -Raw | ConvertFrom-Json` 取 `.version`；失败分别 `Fail`。注释明确「版本号权威源：tauri.conf.json（Cargo.toml 的 version 由 build.rs 校验同值）」（43|）。
* **前端+桌面构建**（50|–62|）：`Push-Location $tauriAppDir` 后 `& npx.cmd tauri build`，**不重定向不接管道**（53| 注释：重定向使子进程失去 tty，cargo 进度条会消失）；`$LASTEXITCODE -ne 0` 则 `Fail`。
* **cargo target-dir 解析**（64|–72|）：正则 `^\s*target-dir\s*=\s*['"](.+?)['"]` 扫 `tauri-app\src-tauri\.cargo\config.toml`，可指向项目外（本项目指到 `E:\ik\Documents\trae_projects\cargo-target\campus-login`，注释见 64|）。
* **产物定位**（74|–98|）：候选目录 = cargo 权威目录 / `$env:CARGO_TARGET_DIR` / `<root>/target` / `src-tauri/target`，逐个看 `release\bundle\nsis` 下 `*-setup.exe`，取 `LastWriteTime` 最新者；找不到则 `Fail`（若仅有旧版本残留则报错提醒勿改名发布）。
* **资产命名**（100|–101|）：从文件名正则 `_([A-Za-z0-9]+)-setup$` 抽架构（默认 `x64`），拼 `Wxxy-CampusLogin_${version}_${arch}-setup.exe`。
* **SHA256 校验**（110|–112|）：`Get-FileHash -Algorithm SHA256` → 小写 hash，写 `"$destExe.sha256"`，内容格式 `"<hash>  <filename>"`（应用内更新按「资产URL+.sha256」请求，文件名必须与资产同名，注释见 110|）。
* **APK 不收集**（115|–116|）：脚本头部注释（11|–13|）与此处注释明确——本脚本仅负责 Windows 桌面端，不编译也不收集安卓 APK（安卓出包走 `android/build-apk.ps1`，两端产物分属不同 Release 渠道）。
* **Release Notes 收集**（118|–122|）：项目根存在 `RELEASE_NOTES_v$version.md` 则拷入 `release/`（注：`.gitignore:54` 忽略 `RELEASE_*.md`，仅本地）。
* **输出清单**（124|–139|）：打印 `release/` 全部文件与 MB 大小，并提示目录内历史遗留文件。

#### `clean-build-cache.ps1`

* **声明**（40|）：`[CmdletBinding(SupportsShouldProcess, ConfirmImpact = "High")]`，支持 `-WhatIf` / `-Confirm:$false`。
* **cargo target-dir 解析**（49|–55|）：与 `make-release.ps1` 同款正则读 `tauri-app\src-tauri\.cargo\config.toml`，注释点明 2026-10-26 前曾漏掉项目外这块最大缓存（47|）。
* **遗留 target 归属判定**（60|–82|）：`Test-OwnedTargetDir` 函数按 `.fingerprint` 中出现 `^(campus-login|campus_login|tauri-plugin-campus)` 才认定属本项目（60|），防止误删别的项目的 `debug\`、`<triple>\` 目录。
* **清理目标**（84|–96| `targets`、97|–100| `globTargets`）：
  * 桌面：`<root>/target`（85|）、`C:\cl-build\target`（86|，旧短路径遗留）、`tauri-app\src-tauri\target`（87|）、`tauri-app\frontend\dist`（93|）。
  * cargo 权威目录 `$cargoTarget`（88|）与同级遗留目录 `$legacyTargets`（80|–82|）。
  * 安卓：`android\src-tauri\target`（89|）、`android\src-tauri\gen\android\app\build`（90|）、`gen\android\.gradle`（91|）、`gen\android\build`（92|）、`android\frontend\dist`（94|）。
  * tsc 增量缓存：两个前端根的 `*.tsbuildinfo`（98|–99|）。
  * 刻意**不删**：`node_modules`、`release\`、`changelogs\`、源码（6|–7| 注释）。
* **运行中检查**（102|–106|）：`Get-Process -Name cargo, rustc, tauri, campus-login, "校园网登录助手"` 非空则红字拒绝退出。
* **预扫描计量**（108|–127|）：逐目标 `Measure-Object` 求 GB/KB，累积 `$total` 与 `$pending`；不存在的打印「跳过（不存在）」。
* **WebView2 缓存定位**（129|–157|）：动态读 `tauri.conf.json` 的 `identifier` → `%LOCALAPPDATA%\<identifier>\EBWebView`，删除 8 个缓存子目录（`Default\Cache`、`Default\Code Cache`、`Default\GPUCache`、`Default\DawnGraphiteCache`、`Default\DawnWebGPUCache`、`Default\Service Worker`、`GrShaderCache`、`ShaderCache`）；**保留 Local Storage / Session Storage**（注释见 129|–131|：资产按普通 HTTP 缓存，清缓存解决界面残留旧资产，用户数据不动）。
* **空目标短路**（159|–162|）。
* **APK 唯一副本告警**（164|–175|）：`gen\android\app\build\outputs\apk` 下存在而 `release\` 没有的 APK → 红字警告需重跑 `android\build-apk.ps1`。
* **NSIS 安装包唯一副本告警**（177|–195|）：按「版本号 + 架构」比对（因 `make-release.ps1` 重命名资产，不能按文件名比对，178|–179|），认不出版本号就保守报警。
* **确认与执行**（197|–208|）：打印合计 GB → `ShouldProcess` 拒绝则「未执行删除」→ `Remove-Item -Recurse -Force` 逐个删除。

### 2. 文档与 Wiki 基础设施

#### `AGENTS.md`

6 条必守约定（5| 标题下 7|–21|）：

| 约定 | 行 | 要点 |
|---|---|---|
| 1 · Changelog | 7| | 改动写 `changelogs/CHANGELOG_v<版本>.md`；版本号唯一权威源 = `tauri-app/src-tauri/tauri.conf.json` 的 `version`；**目标文件 = 当前版本 + 1**（如当前 2.4.1 → `CHANGELOG_v2.4.2.md`）；已有未发布文件则并入最早那个，绝不顺延开新文件；`changelogs/` 在 .gitignore、仅本地；2026-09-15 起废弃统一 `CHANGELOG.md` |
| 2 · 分支纪律 | 9| | 主题分支 `feat/fix/perf/chore/xxx`，commit 前缀规范；收尾可自行 `--ff-only` 合回 main；**禁止 push、删分支**（2026-10-12 用户修订：合并放开） |
| 3 · 双端同步 | 11| | 通用改进必须**同一次提交内**双端各改一份（`tauri-app/` + `android/`），`git diff --stat` 自检；例外：协议核心单点（安卓 path 依赖继承）、平台专属能力各端自理 |
| 4 · 验证基线 | 13|–17| | 见下表 |
| 5 · 文档同步 | 19| | 修 bug 写 `learnings/<slug>.md`、决策写 `decisions/<slug>.md`；收尾 `cw index` + `cw meta update` 随代码提交 |
| 6 · 语言 | 21| | 思考/回复/commit/文档一律中文，代码与报错原文保持原样 |

验证基线明细（13|–17|）：

| 端 | 命令 | 备注 |
|---|---|---|
| 前端 | `npx tsc --noEmit --incremental` | **禁止 `tsc -b`**（tsconfig.node.json 为 composite，会 emit `vite.config.js`/`.d.ts` 污染文件，14|）；打包用 `npx vite build` |
| 桌面后端 | `cd tauri-app/src-tauri && cargo test` | 基线 **397 用例（lib 396 + 集成 1，2026-09-22 实测）**，以 CodeWiki `decisions/verification-baseline` 为准（15|） |
| 安卓后端 | `cargo check --target aarch64-linux-android --all-targets` 或 `pwsh android/build-apk.ps1` | host `cargo check/test` 基线即失败（mobile-only 插件门控，16|） |
| 布局/交互 | 浏览器实测 | 注入 `__TAURI_INTERNALS__` mock + vite dev，**用后 git diff 必须干净**（17|） |

`项目速览`（23|–27|）：技术栈 Tauri 2 + React 19 + TS；桌面根 `tauri-app/frontend` 与 `tauri-app/src-tauri`，安卓根 `android/`；发布流程见 CodeWiki `decisions/release-asset-integrity` 与 `decisions/version-single-source-build-rs`（27|）。

#### `CodeWiki.md`

* **是什么**（1|–7|）：`.codewiki/` 随仓库提交；Wxxy-CampusLogin 是无锡学院 Dr.COM/ePortal 自动登录助手。
* **架构速查**（9|–15|）：协议单点 + 平台外壳（安卓 `android/src-tauri/Cargo.toml:34` path 依赖）；前端两套独立复刻树（桌面 65 条命令、安卓 53 条，三处刻意异名由转调抹平，12|）；通用改动双端同步（`git diff --stat` 唯一守门，13|）。
* **文章导航**（17|–27|）：`_architecture.md`、`_index.md`、`modules/`（19 篇）、`concepts/`（5 篇）、`decisions/`（40+ 篇）、`learnings/`（60+ 篇）、`_patterns.md`。
* **维护约定**（29|–33|）：会话开始 `cw status` 判三态（未编译/过期/最新）；收尾 `cw index` + `cw meta update`；全量刷新走 `codewiki_generate`。

#### `.gitignore`

按用途分组（行号）：

| 类别 | 行 | 关键项 |
|---|---|---|
| 构建输出 | 5|–16| | `target/`、`dist/`、`release/`、`output/`、`app-output/`、`app-v/`、`tauri-release/`、`make-release.ps1`（12|–13|，用户要求不随仓库分发）、`installer-build/`、`release-build/`、`archive/` |
| Tauri 生成 | 19| | `tauri-app/src-tauri/gen/` |
| 环境/IDE/日志/临时 | 22|–44| | `.env`、`.trae/`、`*.log`、`*.tmp`、`*.pdb`、`temp/`、`preview-shots/`、`target-tmp/`（46|–48|，2026-10-02 用户确认） |
| 运行数据 | 51| | `config.json` |
| 发布说明 | 54|–55| | `RELEASE_*.md`、`v*.*.*-更新日志.md`（不入 repo） |
| Changelog | 57|–59| | `CHANGELOG.md`、`changelogs/`（**仅本地留存**） |
| 内部文档 | 61|–64| | `ersikDocumentstrae_projects1`、`docs/`（**`docs/` 整目录不入 git**） |
| 构建脚本 | 67|–68| | `tauri-app/build.ps1`、`clean-build-cache.ps1` |
| Helper 二进制 | 71| | `tauri-app/src-tauri/campus-helper.exe` |
| tsbuildinfo | 80| | `*.tsbuildinfo` |
| Android | 83|–89| | `/.android-git-backup/`、`android/*.apk`、`android/build-apk.ps1`（硬编码本地 JDK 路径） |
| Mock/探针/本地产物 | 92|–106| | `mock-tauri*.js`、`.zcode/`、`.tmp-esbuild/`、`.probe/`、`design-prototypes/`、`archx_*/`、`shots/`、`design/` |
| pnpm 残留 | 108|–110| | `pnpm-lock.yaml`、`pnpm-workspace.yaml`（项目仍用 `package-lock.json`） |

### 3. 变更日志与审计记录（`changelogs/`，本地留存）

#### `changelogs/CHANGELOG_v2.3.6.md`（v2.3.6 时代 4 条缺陷修复）

* 2026-09-11 前端黑屏自愈：`app/webview_recovery.rs` 按崩溃种类分流，`BROWSER_PROCESS_EXITED` 走 `attempt_app_restart` + 10 分钟窗口最多 2 次限流（8|–27|）。
* 安卓新手引导缺失：抽 `useOnboardingFlow` + `OnboardingWizardMobile`（30|–47|）。
* 安卓 WiFi+流量同开登录失败全链路修复：`ensure_wifi_bound` 补齐、`clear_client_pool()`、`requestNetwork` 回退（50|–79|）。
* captive portal + VPN 绑定失败真机修复（82|–110|）。

#### `changelogs/CHANGELOG_v2.3.8.md`（v2.3.8 + 四批追加）

* 行为调整：科教中心 SSID `iwxxy-2/3` 匹配（PR #2，7|）；晚间断网自动切换运营商（11|–13|）；夜切后 chkstatus 自动核验（16|–18|）。
* 缺陷修复：`config-changed` payload 缺 `{config}` 包裹历史回归（26|–28|）。
* 后台留存：轻量化模式 + Windows 效率模式（38|–41|）、安卓退出护栏/稳态退避/renderer 放行（42|–44|）。
* 第二批（50|–72|）：夜切默认开、定时动作哨兵 1440、时间框截断修复、固定值只读展示。
* 第三批（76|–88|）：账号面板移除两开关、夜切文案对齐。
* **第四批（92|–101|，2026-10-03）**：夜间出站目标默认「无线网卡优先」——`monitor/outbound_switch.rs::effective_outbound_priority` 纯函数，`NetworkPanel`/`outboundOrder.ts::buildOutboundOrder` 第三参 `wirelessNames`；验证 `cargo test` **447 用例全绿**（lib 442 + 集成 4 + 其他 1）。

#### `changelogs/CHANGELOG_v2.3.9.md`

* 安卓 WiFi SSID 通道打通（`network-bind` 插件 `getWifiSsid` / `requestWifiSsidPermission`，`NEARBY_WIFI_DEVICES` + `neverForLocation`，7|–12|）。
* 修复「一键下载更新」必现闪退：`verify_file_sha256` 的 64KB 栈数组改 `vec![0u8; 65536]` 堆分配（16|–20|）；约束写入 CodeWiki `learnings/android-async-command-large-future-stack-overflow`。
* 修复「APK 文件不存在」：下载/安装目录统一 `app_data_dir()/files/update`（21|–23|）。

#### `changelogs/CHANGELOG_v2.4.1.md`（开发中，当前活跃文件）

* 行为调整：双端悬浮导航栏对齐 FlClash（7|–12|，药丸零变形、spring 250/28、桌面 `.glass-dock` 毛玻璃）。
* 缺陷修复：最小化期间夜间切换后出站面板陈旧（`syncOnRestore` 5s 节流 + `visibilitychange`/`focus`，16|–19|）；适配器下拉未翻译 key / 双勾选 / 文本拼接三根治（`DisabledAdapter.status: String → AdapterStatus`、轮询 `first_round` 首轮无条件推送、下拉禁用段从 adapters 单通道派生，21|–24|）。
* 文档：iOS 迁移规格评审与 v2 修订（28|–32|）；iOS M0 骨架落地（36|–40|）；iOS 全部移除（44|–47|，2026-10-08 终止开发）。

#### 审计修复计划与四组报告

* **`changelogs/_audit-fix-plan.md`**：54 条发现中 49 条成立、4 条撤销，另有 10 条复审新增（4|），分支 `feat/sess-session--fix-audit-findings`（基线 main@ca58a92，5|）。通用约定（7|–18|）：禁 `git add -A`、禁新增依赖、`async fn` 内禁 ≥4KB 栈数组（12|）。接口约定（20|–26|）：新命令 `cancel_download_update`；新事件 `scheduled-action-skipped`、`mac-reset-cycle`；安卓事件 `update-unverified`；`watcher_failed` 驼峰命名（25|）；`verify_biometric_identity(success, method)` 签名（26|）。分组：A 组桌面 Rust A1–A18（28|–48|）、B 组安卓 Rust B1–B9（50|–76|）、C 组桌面前端 C1–C11（78|–91|）、D 组安卓 Kotlin+前端 D1–D5（93|–102|）、文档收尾（104|–112|，目标 `changelogs/CHANGELOG_v2.4.2.md`）。
* **`_audit-fix-report-A.md`**：A1–A18 全落实；`cargo test` **lib 453 passed / 5 ignored，集成 4 passed / 1 ignored**（7|），`cargo check` 0 warning（8|）；偏离 4 条（112|–117|：A10 测试双重加密错误、A17 `TrySendError::into_inner` 不存在于 stable std、A2/A3 实现形态、测试基线数字差额）；「未做」节（119|–122|）记录 A17/A6 无独立单测的取舍。
* **`_audit-fix-report-B.md`**：`cargo check --target aarch64-linux-android --all-targets` exit=0 / 0 warning（5|）；B1–B9 逐条落实（9|–74|）；偏离 5 条（83|–87|：B4a 降级为保活重发、`watcherFailed` 驼峰命名、B4f continue 语义、B9 未加 app 参数、B8 `cache-path` 整条删除）；未做 4 条（91|–94|）；文件清单 12 个（96|–111|）。
* **`_audit-fix-report-C.md`**：C1–C11 全落实，`npx tsc --noEmit --incremental` exit 0（4|、45|–49|）；15 文件逐项表格（9|–29|）；偏离 4 条（32|–37|：SelfServicePanel 无需改、revealedOp 联动、C4 失败 toast、mountedRef 代替 seq）。
* **`_audit-fix-report-D.md`**：D1–D5 + 补充项（update-unverified 监听，56|–60|），`android/frontend` tsc exit 0（5|、85|–88|）；偏离 5 条（72|–77|：payload 键 `file`、`watcherFailed` 驼峰、不传 consentMessage、catch `IllegalStateException`、失败归一化）；Kotlin 未本机编译（81|）。

### 4. 发布说明（`release/`）

| 文件 | 版本要点 |
|---|---|
| `RELEASE_NOTES_v2.3.3.md` | 安卓端正式发布：KeyStore AES-GCM、生物识别验证门、前台服务保活（5|–11|）；包名变 `com.campuslogin.client`（52|） |
| `RELEASE_NOTES_v2.3.4.md` | 安卓平板 ≥600dp 桌面 Dock 布局（7|）；6 项自动化默认开（8|） |
| `RELEASE_NOTES_v2.3.5.md` | 注销协议重构（Radius 先行、MAC 重置自愈，7|）；WebView2 渲染崩溃事件秒级重载（8|）；看板娘 v2（19|） |
| `RELEASE_NOTES_v2.3.6.md` | 安卓全链路 WiFi 绑定（7|–12|）、2D 人脸回退（16|）、看板娘进驻通知（30|） |
| `RELEASE_NOTES_v2.3.7.md` | 更新检测 snake_case 解析修复（7|）、VPN SO_BINDTODEVICE 旁路（14|）、安卓省电组合拳（21|–24|）、每日定时登录/注销（28|） |
| `RELEASE_NOTES_v2.3.8.md` | 轻量化模式 + EcoQoS（7|–11|）、晚间断网自动切换（15|–19|）、定时动作启用开关（23|）、划任务保活修复（31|） |
| `RELEASE_NOTES_v2.3.9.md` | 安卓 SSID 免定位读取（12|–15|）、闪退与 APK 路径修复（19|–21|） |
| `RELEASE_NOTES_v2.4.0.md` | 夜间出站切换三件套（12|–18|）、状态栏运营商徽标（22|）、APK 43MB→约 20MB（29|）、零 UAC 四级提权链（45|–46|）、恢复窗口 6:30→7:30（63|） |
| `RELEASE_NOTES_v2.4.1.md` | FlClash 导航栏（12|–16|）、适配器下拉三刀根治（20|）、最小化恢复重探（21|）、iOS 终止（25|） |
| `Wxxy-CampusLogin_2.4.1_x64-setup.exe.sha256` | 内容为单行 SHA256 哈希 `8851bfddf5239a853c9eb8227a009ecf645d4312c2a5e9578bf6e2f20e4c09e0  Wxxy-CampusLogin_2.4.1_x64-setup.exe`（1|） |

### 5. 构建日志（诊断用）

* **`target-release-build.log`**（成功构建全链）：版本读取 `version: 2.4.1`（7|）→ vite 两次构建（34|–55|、64|–85|，2187 modules）→ cargo release 3m16s（144|）→ NSIS 产出 `校园网登录助手_2.4.1_x64-setup.exe` 10.92 MB（148|–150|、167|）→ 汇总 `Wxxy-CampusLogin_2.4.1_x64-setup.exe` + `.sha256`（178|–179|）+ `Wxxy-CampusLogin_2.4.1.apk` 24.09 MB（182|、199|）→ 尾部给出 `gh release create/upload` 命令模板（201|–203|）。rustc 为 `1.98.0`（25|）。
* **`build-release.log`**（失败案例）：frontend 构建成功（43|、71|）后 `Compiling campus-login v2.4.1`（72|）即 `failed to build app:`（73|）——Tauri 构建阶段中断，无错误细节被记录，是「日志未捕获 cargo 真实错误」的现场样本。注意该日志 rustc 为 `1.99.1`（16|），与成功日志 `1.98.0`（25|）来自不同工具链时点。

### 6. 设计与调研长文档（`docs/`，本地不入 git）

#### `docs/research/2026-09-27-windows-outbound-switch-research.md`

按证据分级（【项目实证】/【官方文档】/【推论】/【推测】/【否定性结论】，4|–9|）组织的 Windows 出站切换调研。结构：§0 结论速览（13|–21|）、§1 同类开源项目（NMAR 网关可达性兜底 27|–37|、PSActiveNetworkSwitcher 事件触发无还原 39|–62|、nadap-switch 64|–72|、windows-sleep-network-fix 74|–82|、否定性结论：无路由表级定时切网卡项目 92|–100|）、§2 出站切换姿势（合成 metric = 路由 + 接口 106|–114|、四做法对比 116|–123|、TUN metric=0 压制物理卡 125|–141|、禁用网卡是唯一可靠动作 + `CM_Disable_DevNode` 默认不跨重启 143|–150|、两层状态机 152|–167|、host model 与 `DisableDefaultRoutes` 169|–182|、route 命令坑 184|–191|、路由 API 193|–201|、TUN 绕行 203|–208|）、§3 静默提权对比（计划任务默认值陷阱表 222|–232|、错误码表 234|–244|、服务 Session 0 247|–254|、`autoElevate` 仅限微软签名 259|、三形态推荐计划任务代理 272|–283|）、§4 可恢复性（快照四对象 300|–307|、三层兜底 325|–339|、路由侧天然兜底 341|–345|、事件驱动 347|–356|）、§5 缺陷模式六类（360|–401|）、§6 对本项目启示（405|–412|）、附录证据等级汇总（416|–424|）。

#### `docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md`

夜间出站切换设计规格：时间表周日~四 23:00 / 周五六 23:30 / 恢复窗口（14|）；配置字段 `enableNightOutboundSwitch`/`outboundPriority`/`outboundMetricRestore`/`nightOutboundRestore`（26|–31|）；判定纯函数 `config/outbound_switch.rs::evaluate_night_outbound`（35|–39|）；桌面切换动作 = 逐卡校园网判定 + metric=1 + `HelperOp::SetMetric` + 快照时序（41|–47|）；三重还原（49|–53|）；安卓「注销 + `report_wifi_unusable` 触发系统重检」（71|–77|）；`network_avoid_bad_wifi` 引导（81|–84|）；已排除路线 4 条（103|–108|）；验证计划（110|–117|）。

#### `docs/superpowers/plans/2026-09-14-account-adapter-binding.md`

账号体系增强实施计划（分支 `feat/sess-8604de73-account-adapter-binding`，基线 `aeee616`，6|–7|）：R1–R5 需求表（14|–20|，R5 明确不实施 22|）；数据契约 `display_name`/`adapter1_account`/`adapter2_account` 与 `AccountItem`（39|–74|）；行为规则 §4.1–§4.5（84|–129|，含自动建号幂等短路、显示名重名校验、切换三路径修复、按适配器取凭据）；工作单元 A–D（135|–184|）；验证命令（190|–199|，禁 `tsc -b` 202|）；双端同步清单（208|–216|，适配器指定账号为桌面专属例外 214|）；风险 8 条（223|–230|）；验收标准（234|–241|）。

### 7. Changelog 插入脚本（一次性工具）

#### `docs/insert-rtss-changelog.js`

* **硬编码绝对路径**（4|–5|）：`E:/ik/Documents/trae_projects/1/Wxxy-CampusLogin/CHANGELOG.md` 与 `docs/changelog-entry-rtss.md`——仅本机可用。
* **幂等检查**（8|–11|）：目标已含「定位 WebView 白屏崩溃根因」则 `SKIP`。
* **锚点插入**（13|–16|）：定位 `### 缺陷修复`，在其行尾换行后插入条目。
* **状态**：该脚本针对的统一 `CHANGELOG.md` 已于 2026-09-15 废弃（`AGENTS.md:7`、`.gitignore:58`），脚本与目标文件均已失效，仅作历史工具保留。

#### `docs/changelog-entry-rtss.md`

RTSS 注入导致 WebView2 白屏的 changelog 条目草稿（1|–16|）：根因为 RTSS 向 msedgewebview2.exe 注入 `RTSSHooks64.dll` 触发 0xc0000005（4|–6|）；「自动写排除 profile」方案被**证伪**——RTSS 7.3.7 忽略手放的 cfg，只能在 RTSS 界面手动排除（8|–14|）；`cargo test` 328 passed（15|–16|）。

### 8. 许可与自述

#### `LICENSE`（1|–28|）

MIT License（Copyright (c) 2026 ik），1|–21|；尾部**资产例外**（25|–28|）：许可仅覆盖源码与构建脚本，美术素材清单见 `LICENSE-ASSETS.md`，第三方组件声明见 `THIRD-PARTY-NOTICES.md`。

#### `LICENSE-ASSETS.md`（1|–30|）

* 素材清单（6|–10|）：`assets/ui-girl/`、双端 `public/girl/`、应用图标与收款码、双端 `icons/`（含 `tray-icon.png`）。
* **允许**（14|–16|）：仅本项目源码副本与官方发行版内部原样使用。
* **禁止**（18|–23|）：单独提取/复制/再分发、改编二创、商业用途、**用于训练机器学习/AI 生成模型**。
* 授权请求（27|）：GitHub Issues 或 `ikliml6666@gmail.com`。

#### `README.md`

* 版本徽标 `2.4.3`（5|）、平台 Windows x64 | Android（6|）、MIT（7|）。
* 下载渠道：GitHub Releases + 夸克网盘（28|）。
* 功能特性（35|–66|）：含「夜间出站切换」描述（41|，周日至周四 23:00、周五六 23:30、次日 7:30 还原、长按拖拽排序）、安卓平台差异清单（66|）。
* 技术栈表（68|–79|）与项目结构树（81|–131|）。
* 测试命令（183|–199|）：`cargo test`（注明 432 单元 + 4 集成，186|）、`npm test`（103 个前端测试，193|）、`npx tsc --noEmit --incremental`（198|）。
* 安全说明（201|–214|）：DPAPI/KeyStore、`bindProcessToNetwork`、更新白名单 + SHA256、四级提权降级链（212|：管理员直跑 → SYSTEM 计划任务代理 → COM 静默 → runas）、账号名防路径遍历（213|）。
* 双轨许可说明（220|–226|）。

## Architecture

本模块采用「规范驱动 + 脚本自动化 + Markdown 链式沉淀」架构：`AGENTS.md` 确立纪律 → 开发者/AI 执行并写 `changelogs/` → `make-release.ps1` 组装发布资产 → `release/` 归档用户说明；`clean-build-cache.ps1` 独立处理缓存；`docs/` 与 `.codewiki/` 承载设计与知识。

```text
┌──────────────────────────────────────────────────────────────────┐
│               Workspace Docs & Build（本模块）                    │
├──────────────────────────────────────────────────────────────────┤
│ [AGENTS.md] ──规范──► [开发者 / AI Agent]                         │
│    │  ├─changelog(约定1)─► [changelogs/CHANGELOG_v*.md] (本地)     │
│    │  ├─双端同步(约定3)──► [tauri-app/] + [android/]              │
│    │  └─wiki 同步(约定5)─► [CodeWiki.md] ► [.codewiki/]           │
│    ▼                                                             │
│ [make-release.ps1] ─读版本─► [tauri.conf.json]                    │
│    │  ├─构建─► npx tauri build ─► target/…/nsis/*-setup.exe      │
│    │  ├─定位─► [.cargo/config.toml] target-dir                    │
│    │  ├─校验─► release/*.sha256                                   │
│    │  └─收集─► [release/*.md] (APK 不收集，仅 Windows 桌面端)     │
│ [clean-build-cache.ps1] ─进程检查─► cargo/rustc/tauri             │
│    └─清缓存─► target/ dist/ %LOCALAPPDATA%\<id>\EBWebView         │
└──────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **开发阶段**：
   * 开发者/AI 按 `AGENTS.md:7` 确认 `tauri.conf.json` 版本号，把改动写入 `changelogs/CHANGELOG_v<版本+1>.md`（审计轮目标 `CHANGELOG_v2.4.2.md`，`_audit-fix-plan.md:106`）。
   * 通用改动 `git diff --stat` 自检双端同时触及（`AGENTS.md:11`）。
   * 设计/调研落 `docs/superpowers/{specs,plans}/` 与 `docs/research/`；bug 写 `learnings/`、决策写 `decisions/`，收尾 `cw index` + `cw meta update`（`AGENTS.md:19`）。
   * 验证：双端 `npx tsc --noEmit --incremental`、桌面 `cargo test`、安卓 `cargo check --target aarch64-linux-android --all-targets`。

2. **构建发布阶段**：
   * `pwsh -File make-release.ps1`（或 `-SkipBuild`）→ 读版本（`make-release.ps1:43`–`48`）→ `npx.cmd tauri build`（50|–62|）→ 解析 target-dir 找最新 `*-setup.exe`（74|–98|）→ 重命名为 `Wxxy-CampusLogin_<ver>_<arch>-setup.exe`（100|–101|）→ 生成 `.sha256`（110|–112|）→ 收集 `RELEASE_NOTES_v<ver>.md`（118|–122|，APK 不在本脚本收集范围见 115|–116| 注释）→ 打印清单（124|–139|）。
   * 发布用 `target-release-build.log:201`–`203` 的 `gh release create/upload` 模板上传资产。

3. **清理阶段**：
   * `pwsh -File clean-build-cache.ps1 [-WhatIf|-Confirm:$false]` → 进程检查（102|–106|）→ 预扫描计量（108|–127|）→ WebView2 缓存按 identifier 定位（129|–157|）→ APK/NSIS 唯一副本告警（164|–195|）→ `ShouldProcess` 确认（199|–202|）→ 删除（204|–207|）。

## Connections

* [[campus-login]]：桌面协议核心 crate，`make-release.ps1` 收集其 NSIS 产物；其验证基线（397 用例）写在 `AGENTS.md:15`。
* [[night-outbound-switch]]：夜间出站切换，设计规格见 `docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md`，调研报告见 `docs/research/2026-09-27-windows-outbound-switch-research.md`，变更见 `CHANGELOG_v2.3.8.md:92`–`101` 与 `RELEASE_NOTES_v2.4.0.md:12`–`18`。
* [[account-adapter-binding]]：实施计划见 `docs/superpowers/plans/2026-09-14-account-adapter-binding.md`，其双端同步清单与验收标准是该功能的验收依据。
* [[rtss-compat]]：RTSS 注入白屏问题，条目草稿 `docs/changelog-entry-rtss.md` 与一次性插入脚本 `docs/insert-rtss-changelog.js` 均在本模块。
* [[audit-fix-findings]]：审计修复轮的计划（`_audit-fix-plan.md`）与 A/B/C/D 四份报告，接口约定跨组固定不可改名。
* [[release-asset-integrity]] / [[version-single-source-build-rs]]：`make-release.ps1:43` 版本权威源与 `.sha256` 同名约定的决策依据（`AGENTS.md:27` 引用）。
* [[verification-baseline]]：桌面/安卓验证基线的权威记录，`AGENTS.md:15`–`16` 与其保持一致。

## Known Issues

1. **`changelogs/` 与 `docs/` 不入 git**：`.gitignore:57`–`59`（changelogs）与 `61`–`64`（docs）整体忽略，本模块大部分源文件（调研、计划、设计、审计报告、changelog）仅存开发者本地，仓库审查者看不到细粒度变更史；发布与协作需依赖 `release/RELEASE_NOTES_*.md`（但其也被 `RELEASE_*.md` 忽略，`.gitignore:54`）。
2. **README 版本徽标与构建日志不一致**：`README.md:5` 徽标为 `2.4.3`，而 `target-release-build.log:7` 与 `build-release.log` 构建的均为 `2.4.1`。AGENTS.md 约定版本唯一权威源是 `tauri.conf.json`（`AGENTS.md:7`），徽标需按版本同步清单（`AGENTS.md:27` 指向 `decisions/version-single-source-build-rs`）随版本提交一并更新，否则 README 撒谎。
3. **README 测试用例数与实际基线漂移**：`README.md:186` 写「432 个单元测试 + 4 个集成测试」、`193` 写「103 个测试」，而 `AGENTS.md:15` 权威基线为 397（lib 396 + 集成 1，2026-09-22），审计后实测已达 453 lib + 4 集成（`_audit-fix-report-A.md:7`），`CHANGELOG_v2.3.8.md:69` 又记 719、`CHANGELOG_v2.3.8.md:99` 记 447——四处数字互相冲突，README 的数字已过期。
4. **Android host `cargo check/test` 必败**：mobile-only 插件门控（`AGENTS.md:16`），CI 必须注入 NDK 跑 `--target aarch64-linux-android --all-targets` 或用 `android/build-apk.ps1`；单测执行无 host 通道（`_audit-fix-report-B.md:94`）。
5. **`make-release.ps1` 对自定义 target-dir 的隐式依赖**：产物定位依赖 `.cargo/config.toml` 正则解析与四个候选目录（`make-release.ps1:64`–`98`）；若 `target-dir` 写法超出正则 `['"]…['"]` 匹配（如未加引号），脚本会静默回退到默认目录并可能打包旧产物（该风险在 `64` 注释中被点名）。
6. **`docs/insert-rtss-changelog.js` 已失效**：目标 `CHANGELOG.md` 已废弃删除（`AGENTS.md:7`、`.gitignore:58`），且脚本硬编码 `E:/ik/...` 绝对路径（`docs/insert-rtss-changelog.js:4`–`5`），在其他机器运行必然报错；应视为历史一次性工具。
7. **RTSS 注入无法自动修复**：自动写排除 profile 的方案被证伪——RTSS 7.3.7 忽略手放入 Profiles 目录的 cfg，只能检测后 `log_warn` 引导用户在 RTSS 界面手动排除（`docs/changelog-entry-rtss.md:14`）。
8. **构建日志可能丢失 cargo 真实错误**：`build-release.log:72`–`73` 仅有 `Compiling campus-login v2.4.1` 与 `failed to build app:`，无 rustc 错误细节；诊断构建失败时不能只依赖该类汇总日志，需重跑直通输出（`make-release.ps1:53` 注释正是为此不重定向）。
9. **WebView2 缓存清理的边界**：`clean-build-cache.ps1` 删除 `Code Cache`/`ShaderCache` 等但保留 `Local Storage`/`Session Storage`（`clean-build-cache.ps1:129`–`157`）——用户数据不丢，但清理后首启需重新拉取资产、可能触发一次完整重新渲染。
10. **审计修复的跨组协调残留**：`_audit-fix-report-B.md:74` 记录 B9 签名已改而前端调用在 D 组未完成前会缺参报错；`_audit-fix-report-D.md:81` 记录 Kotlin 侧无本机编译验证——这类「接口已改、消费端待接」窗口依赖 `_audit-fix-plan.md:104`–`112` 的主会话统一验证（`build-apk.ps1` 兜底）收口，合并前必须跑完。
