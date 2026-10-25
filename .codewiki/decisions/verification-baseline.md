---
title: 统一验证基线（测试与类型检查命令）
type: decision
source_files:
  - AGENTS.md
tags: [决策, 验证, 测试基线]
---

## 背景

项目需要一套固定的、"跑过才算完"的验证命令，避免每次凭"应该没问题"交差。AGENTS.md 第 13 行起（`13| 4. **验证以实际输出为准，不凭"应该没问题"**：`）已将验证基线列为项目必守约定，本决策文档以此为准进行重写与说明。

## 决策

验证基线以 AGENTS.md 第 13-17 行的约定为准：

- **前端验证**：在 `tauri-app/frontend` 下执行 `npx tsc --noEmit --incremental`（AGENTS.md:14）。**严禁使用 `tsc -b`**，因为 `tsconfig.node.json` 为 composite 项目，会 emit 出 `vite.config.js` / `.d.ts` 污染文件。涉及打包产物时需用 `npx vite build` 确认无警告。安卓前端为独立复刻代码库，须在 `android/frontend` 下执行同法验证（AGENTS.md:14，结合 AGENTS.md:14 的双端同步约定）。
- **后端验证**：在 `tauri-app/src-tauri` 下执行 `cargo test`，全绿基线为 **548 用例（lib 547 + 集成 1，2026-10-25 实测）**（AGENTS.md:15）。
- **安卓端 Rust 验证**：由于 host 环境执行 `cargo check` / `cargo test` 在 `android/src-tauri` 基线即失败（mobile-only 插件门控），Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量）或一键出包 `pwsh android/build-apk.ps1`（AGENTS.md:16）。
- **布局 / 交互类改动验证**：需向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务进行浏览器实测，**用后必须完整还原**（`git diff` 必须干净）（AGENTS.md:17）。

### `cargo test` 用例数统计口径

依据 AGENTS.md:15 的 `cargo test` 要求，在 `tauri-app/src-tauri` 下执行 `cargo test`，独立用例数 = lib 547 + 集成 1 = 548（2026-10-25 实测，WebVPN 模块接入后）。bin 目标与 lib 是同一套测试经双模块树重复链接，把多行 `test result` 直接相加会把 lib 那套数两遍，不是独立用例数。**后续新增/删除测试时按同口径更新此数**。

## 理由

根据 AGENTS.md:13 的强制约定，验证必须以实际输出为准，不能凭"应该没问题"。将具体的命令、基线数据与例外情况固化为决策文档，可为 AI 编码助手与贡献者提供明确的"完成"定义，并便于 CodeWiki 索引关联（AGENTS.md:19）。

## 备选方案

无（AGENTS.md 直接以"实际输出为准"作为顶层必守约定，未记录其他备选；旧文档亦未记录）。

## 影响与约束

以下约束均提取自 AGENTS.md 第 13-17 行，适用于所有改动：

- **前端**（AGENTS.md:14）：`npx tsc --noEmit --incremental`（在 `tauri-app/frontend` 下；**禁止 `tsc -b`**——tsconfig.node.json 为 composite 项目，会 emit 出 vite.config.js/.d.ts 污染文件）。涉及打包产物时 `npx vite build` 确认无警告。安卓前端同法（在 `android/frontend` 下执行）。
- **后端**（AGENTS.md:15）：`cargo test`（在 `tauri-app/src-tauri` 下；全绿基线 548 用例（lib 547 + 集成 1，2026-10-25 实测），以 CodeWiki 的 `decisions/verification-baseline` 为准）。
- **安卓端**（AGENTS.md:16）：host `cargo check` / `cargo test` 在 `android/src-tauri` 基线即失败（mobile-only 插件门控）——Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量）或一键出包 `pwsh android/build-apk.ps1`（详见 CodeWiki 的 `decisions/verification-baseline` 与 `learnings/android-host-cargo-check-fails`）。
- **布局 / 交互类改动**（AGENTS.md:17）：需向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，**用后必须完整还原**（git diff 必须干净）。

## Connections

[[tsc-b-emits-contaminated-files]]、[[android-host-cargo-check-fails]]
