# AGENTS.md — 项目工作约定

面向 AI 编码助手（ZCode 等）与贡献者的项目级约定。架构速查见 `CODE_WIKI.md`；版本变更记录见 `CHANGELOG.md`（本地维护，不入 git）。

## 必守约定

1. **改动必须记入 CHANGELOG.md**：任何代码 / 配置 / 构建 / 文档改动，完成并验证后写入 `CHANGELOG.md` 的 `[Unreleased]` 小节（沿用既有分类：行为调整 / 缺陷修复 / 前端性能优化 / 架构改进 等），说清"改了什么 + 为什么 + 验证数据"。CHANGELOG.md 在 .gitignore 中，本地持续维护、不提交。
   **CHANGELOG 按版本归档**：发布版本（打 tag 或跑 `make-release.ps1`）时把当时整份 `CHANGELOG.md` 快照存为本地 `changelogs/CHANGELOG_v<版本>.md`（该目录在 .gitignore，仅本地留存），防止单文件无限膨胀；`make-release.ps1` 汇总发布资产时自动执行快照，快照后可酌情清理 CHANGELOG.md 中已发布版本的旧条目保持轻量。

2. **分支纪律**：改动全在主题分支（`feat/xxx`、`fix/xxx`、`perf/xxx`、`chore/xxx`），不直接改 main；commit 说明用 `feat:` / `fix:` / `perf:` / `chore:` / `docs:` 前缀；不擅自 push、merge、删分支——由用户决定。

3. **通用改进双端同步**：本项目是 Windows 桌面端 + 安卓端的双端应用，安卓前端（`android/frontend`）是独立复刻代码库，**桌面改动不会自动同步到安卓**。凡通用改进——前端 UI / 交互行为、文案与 i18n、图标与看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等——必须在**同一次提交内双端各改一份**（桌面 `tauri-app/` + 安卓 `android/`），不允许只改一端留待"下次同步"；提交前用 `git diff --stat` 自检是否同时触及两端。两类例外：协议实现单点存在于桌面 crate（安卓经 Cargo path 依赖自动继承，禁止复制，见 CODE_WIKI §三-1）；平台专属能力（DPAPI/Keystore、托盘/前台服务等）各端自理。

4. **验证以实际输出为准，不凭"应该没问题"**：
   - 前端：`npx tsc --noEmit --incremental`（在 `tauri-app/frontend` 下；**禁止 `tsc -b`**——tsconfig.node.json 为 composite 项目，会 emit 出 vite.config.js/.d.ts 污染文件）。涉及打包产物时 `npx vite build` 确认无警告。安卓前端同法（在 `android/frontend` 下执行）。
   - 后端：`cargo test`（在 `tauri-app/src-tauri` 下；全绿基线 493 用例，以 CODE_WIKI §1.4 为准）。
   - 安卓端：host `cargo check` / `cargo test` 在 `android/src-tauri` 基线即失败（mobile-only 插件门控）——Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量）或一键出包 `pwsh android/build-apk.ps1`（详见 CODE_WIKI §1.4 / §4.3.1）。
   - 布局 / 交互类改动需浏览器实测：向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，**用后必须完整还原**（git diff 必须干净）。

5. **文档同步**：新模块、决策变更、踩坑记录同步更新 `CODE_WIKI.md`（增量记录，不写流水账）；用户可见的行为变化同步 CHANGELOG（见第 1 条）。

6. **语言**：思考、回复、commit 说明、文档一律中文；代码、命令、报错原文保持原样。

## 项目速览

- 技术栈：Tauri 2 + React 19 + TypeScript，Windows + 安卓双端同构的校园网（Dr.COM/Portal）自动登录助手。
- 桌面端：前端根 `tauri-app/frontend`、后端根 `tauri-app/src-tauri`；安卓端根 `android/`（后端 path 依赖桌面协议核心 crate，前端为独立复刻树）。
- 发布流程、版本号同步清单、Release 资产检查清单见 `CODE_WIKI.md` 对应章节（版本号提交与 Release 发布必须同流程完成）。
