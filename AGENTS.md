# AGENTS.md — 项目工作约定

面向 AI 编码助手（ZCode 等）与贡献者的项目级约定。架构与模块详解见仓库内 `.codewiki/`（由 `cw` 工具维护，随仓库进 git、可评审可共享；入口是 `_architecture.md` 与 `_index.md`）；版本变更记录见 `changelogs/`（按版本文件维护，随仓库进 git）。

## 必守约定

1. **改动必须记入版本化 changelog（changelogs/）**：任何代码 / 配置 / 构建 / 文档改动，完成并验证后写入 `changelogs/CHANGELOG_v<版本>.md`（沿用既有分类：行为调整 / 缺陷修复 / 前端性能优化 / 架构改进 等），说清"改了什么 + 为什么 + 验证数据"。**写之前必须先确认当前版本号**：以 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 字段为唯一权威源（见 CodeWiki 的 `decisions/version-single-source-build-rs`），不凭会话记忆或既有文件名顺延猜测。**目标文件 = 当前版本号 + 1**（如当前 2.4.1 → 写 `changelogs/CHANGELOG_v2.4.2.md`）；该文件即下一版发布时的更新日志——已存在就把新条目并入该文件（changelogs/ 里可能有多个未发布版本累积文件时，并入最早的那个），**绝不顺延开新的版本文件**，发布后封版，新内容再从新的"当前版本号 + 1"起步。`changelogs/` 随代码一起提交（2026-11-13 起从 .gitignore 移除，避免整文件重写后无从恢复）；不再维护统一的 CHANGELOG.md（2026-09-15 起废弃），发布流程也没有快照步骤。

2. **分支纪律**：改动全在主题分支（`feat/xxx`、`fix/xxx`、`perf/xxx`、`chore/xxx`），不直接改 main；commit 说明用 `feat:` / `fix:` / `perf:` / `chore:` / `docs:` 前缀；任务收尾可自行将本会话主题分支 merge 回 main（优先 `--ff-only`，遇冲突即停），但**不擅自 push、删分支**——由用户决定。

3. **通用改进双端同步**：本项目是 Windows 桌面端 + 安卓端的双端应用，安卓前端（`android/frontend`）是独立复刻代码库，**桌面改动不会自动同步到安卓**。凡通用改进——前端 UI / 交互行为、文案与 i18n、图标与看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等——必须在**同一次提交内双端各改一份**（桌面 `tauri-app/` + 安卓 `android/`），不允许只改一端留待"下次同步"；提交前用 `git diff --stat` 自检是否同时触及两端。两类例外：协议实现单点存在于桌面 crate（安卓经 Cargo path 依赖自动继承，禁止复制，见 CodeWiki 的 `decisions/protocol-core-single-source` 与 `concepts/dual-platform-sharing`）；平台专属能力（DPAPI/Keystore、托盘/前台服务等）各端自理。

4. **验证以实际输出为准，不凭"应该没问题"**：
   - 前端：`npx tsc --noEmit --incremental`（在 `tauri-app/frontend` 下；**禁止 `tsc -b`**——tsconfig.node.json 为 composite 项目，会 emit 出 vite.config.js/.d.ts 污染文件）。涉及打包产物时 `npx vite build` 确认无警告。安卓前端同法（在 `android/frontend` 下执行）。
   - 后端：`cargo test`（在 `tauri-app/src-tauri` 下；全绿基线与各套件用例数以 CodeWiki 的 `decisions/verification-baseline` 为准，不在本文件硬编码数字）。
   - 安卓端：host `cargo check` / `cargo test` 在 `android/src-tauri` 基线即失败（mobile-only 插件门控）——Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量）或一键出包 `pwsh android/build-apk.ps1`（详见 CodeWiki 的 `decisions/verification-baseline` 与 `learnings/android-host-cargo-check-fails`）。
   - 布局 / 交互类改动需浏览器实测：向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，**用后必须完整还原**（git diff 必须干净）。

5. **版本号升级按固定清单全量同步（发布前）**：版本号散落在 20 个文件里，漏一处就会在构建或发布环节炸——以 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 为唯一权威源，一次性改完下列全部位置：
   - **权威源与 Rust**：两端 `tauri.conf.json`、`tauri-app/src-tauri/Cargo.toml`、`android/src-tauri/Cargo.toml`、`android/core-logic/Cargo.toml`。
   - **前端包与常量**：两端 `package.json`、两端 `src/shared/ui-constants.ts` 的 `APP_VERSION`。
   - **锁文件**：三份 `package-lock.json` 各 2 处（根 `version` 与 `packages[""]`）；三份 `Cargo.lock` **只改自有包**（`campus-login`、`campus-login-android`、`campus-core-logic`），第三方包的同名版本号（如 `@bramus/specificity@2.4.3`）一律不动。
   - **展示位**：`README.md` 徽章、`tauri-app/frontend/about-preview.html` 两处（`app-version` / `status-version`）。
   - **`version.json`**：`version` 带 `v` 前缀；`notes` 保持精简——固定下载提醒头 + GitHub / 夸克两条渠道 + 「### 本版本最重要的变化」3~4 条要点 + 末尾指引，不贴整份 Release Notes。
   **不动的旧版本残留**（改了就是错）：注释里的历史版本引用（如 `monitor/outbound_night.rs` 的「见 changelog v2.4.3」）、`changelogs/` 与 `release/` 下的历史存档与上一版发布资产、构建产物目录（`android/src-tauri/gen/`、`target-tmp/`）。
   **验证**：桌面 `cargo check --lib` 输出 `Compiling campus-login v<新版本>`（证明版本号已进编译单元）；双端 `npx tsc --noEmit --incremental`；最后全仓 `grep -rIn "<旧版本>"`（排除 node_modules / target / build / gen / changelogs / release），剩余每一处都要能说清为什么保留。安装包编译与 Release 发布属另一环节，不在本清单内（见 CodeWiki 的 `decisions/release-asset-integrity`）。

6. **语言**：思考、回复、commit 说明、文档一律中文；代码、命令、报错原文保持原样。

## 项目速览

- 技术栈：Tauri 2 + React 19 + TypeScript，Windows + 安卓双端同构的校园网（Dr.COM/Portal）自动登录助手。
- 桌面端：前端根 `tauri-app/frontend`、后端根 `tauri-app/src-tauri`；安卓端根 `android/`（后端 path 依赖桌面协议核心 crate，前端为独立复刻树）。
- 发布流程与 Release 资产检查清单见 CodeWiki 的 `decisions/release-asset-integrity` 与 `decisions/version-single-source-build-rs`（版本号提交与 Release 发布必须同流程完成）。
