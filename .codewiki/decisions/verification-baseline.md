---
title: 统一验证基线（测试与类型检查命令）
type: decision
source_files:
  - AGENTS.md
tags: [决策, 验证, 测试基线]
---

## 背景

需要一套固定的、"跑过才算完"的验证命令，避免每次凭"应该没问题"交差。

## 决策

验证基线为：`cargo test` 全绿（**基线 397 用例（lib 396 + 集成 1，2026-09-22 实测），以 CodeWiki 的 `decisions/verification-baseline` 为准**）；`npx tsc --noEmit --incremental`；安卓端 Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets` 或一键出包 `pwsh android/build-apk.ps1`。布局 / 交互类改动需浏览器实测，且用后 `git diff` 必须干净。

### `cargo test` 用例数统计口径

在 `tauri-app/src-tauri` 下执行 `cargo test`，独立用例数 = lib 396 + 集成 1 = 397。bin 目标与 lib 是同一套测试经双模块树重复链接，把多行 `test result` 直接相加会把 lib 那套数两遍，不是独立用例数。**后续新增/删除测试时按同口径更新此数**。

## 理由

旧文档未记录（只给出命令清单与"以实际输出为准"的要求）。

## 备选方案

旧文档未记录。

## 影响与约束

- **前端**：`npx tsc --noEmit --incremental`（在 `tauri-app/frontend` 下；**禁止 `tsc -b`**——tsconfig.node.json 为 composite 项目，会 emit 出 vite.config.js/.d.ts 污染文件）。涉及打包产物时 `npx vite build` 确认无警告。安卓前端同法（在 `android/frontend` 下执行）。
- **后端**：`cargo test`（在 `tauri-app/src-tauri` 下；全绿基线 397 用例（lib 396 + 集成 1，2026-09-22 实测），以 CodeWiki 的 `decisions/verification-baseline` 为准）。
- **安卓端**：host `cargo check` / `cargo test` 在 `android/src-tauri` 基线即失败（mobile-only 插件门控）——Rust 改动只认 `cargo check --target aarch64-linux-android --all-targets`（需注入 NDK 工具链环境变量）或一键出包 `pwsh android/build-apk.ps1`（详见 CodeWiki 的 `decisions/verification-baseline` 与 `learnings/android-host-cargo-check-fails`）。
- **布局 / 交互类改动**：需向 `tauri-app/frontend/index.html` 临时注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，**用后必须完整还原**（git diff 必须干净）。

## Connections

[[tsc-b-emits-contaminated-files]]、[[android-host-cargo-check-fails]]