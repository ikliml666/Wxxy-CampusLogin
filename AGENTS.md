# AGENTS.md — 项目工作约定

面向 AI 编码助手（ZCode 等）与贡献者的项目级约定。架构速查见 `CODE_WIKI.md`；版本变更记录见 `CHANGELOG.md`（本地维护，不入 git）。

## 必守约定

1. **改动必须记入 CHANGELOG.md**：任何代码 / 配置 / 构建 / 文档改动，完成并验证后写入 `CHANGELOG.md` 的 `[Unreleased]` 小节（沿用既有分类：行为调整 / 缺陷修复 / 前端性能优化 / 架构改进 等），说清"改了什么 + 为什么 + 验证数据"。CHANGELOG.md 在 .gitignore 中（第 52 行），本地持续维护、不提交。

2. **分支纪律**：改动全在主题分支（`feat/xxx`、`fix/xxx`、`perf/xxx`、`chore/xxx`），不直接改 main；commit 说明用 `feat:` / `fix:` / `perf:` / `chore:` / `docs:` 前缀；不擅自 push、merge、删分支——由用户决定。

3. **验证以实际输出为准，不凭"应该没问题"**：
   - 前端：`npx tsc --noEmit --incremental`（在 `tauri-app/frontend` 下；**禁止 `tsc -b`**——tsconfig.node.json 为 composite 项目，会 emit 出 vite.config.js/.d.ts 污染文件）。涉及打包产物时 `npx vite build` 确认无警告。
   - 后端：`cargo test`（在 `tauri-app/src-tauri` 下；全绿基线 229 用例）。
   - 布局 / 交互类改动需浏览器实测：向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，**用后必须完整还原**（git diff 必须干净）。

4. **文档同步**：新模块、决策变更、踩坑记录同步更新 `CODE_WIKI.md`（增量记录，不写流水账）；用户可见的行为变化同步 CHANGELOG（见第 1 条）。

5. **语言**：思考、回复、commit 说明、文档一律中文；代码、命令、报错原文保持原样。

## 项目速览

- 技术栈：Tauri 2 + React 19 + TypeScript，Windows 平台校园网（Dr.COM/Portal）自动登录助手。
- 前端根：`tauri-app/frontend`；后端根：`tauri-app/src-tauri`。
- 发布流程、版本号五处同步、Release 资产检查清单见 `CODE_WIKI.md` 对应章节（版本号提交与 Release 发布必须同流程完成）。
