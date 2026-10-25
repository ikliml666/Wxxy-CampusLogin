---
title: bin 与 lib 是两棵独立模块树，新增顶层模块必须两处声明
type: decision
source_files:
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/lib.rs
tags: [决策, 构建, 模块树, rust]
---

## 背景

同一份源码需要同时被 bin target 与 lib target 使用（安卓经 lib 复用桌面 crate）。在 `tauri-app/src-tauri/src/lib.rs` 第 1 行注释明确说明：`// 跨平台协议核心:安卓端(path 依赖)唯一可见面,保持无桌面依赖`，第 11 行注释说明：`// 桌面专属:托盘/窗口/命令/监控/提权/更新,安卓 target 不编译`。`tauri-app/src-tauri/src/main.rs` 作为桌面 bin 入口（见第 3-15 行）直接声明了全部模块且无 `#[cfg(desktop)]` 限制，因为 bin target 仅面向桌面构建。

## 决策

`main.rs`（第 3-15 行）与 `lib.rs`（第 2-21 行）各声明一份模块树，新增顶层模块**必须两处同时声明**。

- 若为跨平台核心模块：需在 `lib.rs` 第 2-9 行区域加入 `pub mod xxx;`，并在 `main.rs` 第 3-15 行区域加入 `mod xxx;`。
- 若为桌面专属模块：需在 `lib.rs` 第 12-21 行区域加入 `#[cfg(desktop)] pub mod xxx;`（参照第 12-13 行 `app`、第 14-15 行 `commands`、第 16-17 行 `helper`、第 18-19 行 `monitor`、第 20-21 行 `update` 的写法），并在 `main.rs` 第 3-15 行区域加入 `mod xxx;`（无 cfg 限制，如第 3 行 `mod app;`、第 4 行 `mod commands;`、第 13 行 `mod helper;`、第 8 行 `mod monitor;`、第 11 行 `mod update;`）。

## 理由

旧文档只写了违反后果（漏一处 bin target E0432），未记录为何不用 `main.rs` 复用 `campus_login_lib::`。根源在于 `lib.rs` 第 1 行的设计约束：安卓端通过 path 依赖仅引用 lib 的纯跨平台面（第 2-9 行），而 `main.rs` 作为 bin 入口需要引入桌面专属逻辑并完全控制启动流程。观察 `main.rs` 的 `main` 函数：第 21-25 行设置 `panic` hook 并直接调用 `crate::log_error!` 与 `crate::infra::logger::flush_quick()`；第 31-33 行在 Tauri 装配前拦截 `--helper-task` 参数并直接 `std::process::exit`；第 38-47 行解析 `--helper` 提权参数；第 52-53 行执行 RTSS 检测；第 57-69 行构建 WebView2 启动参数并设置 env；第 75-81 行构建 Tokio runtime 并启动 Tauri 应用。这些流程要求 `main.rs` 自身拥有一棵完整的模块树（第 3-15 行），无法直接通过 `mod lib;` 下沉到 lib 的跨平台约束中，因此必须两处独立声明模块。

## 备选方案

旧文档未记录。从当前代码看，曾考虑的备选包括：在 `lib.rs` 中全量声明所有模块并对安卓做条件裁剪（现状即如此，见 `lib.rs` 第 12-21 行的 `#[cfg(desktop)]`），或让 `main.rs` 完全代理 `lib.rs` 调用（被否决，因为 `main.rs` 的 `main` 函数包含大量 bin 专属流程，无法直接下放至 lib 的跨平台约束中）。

## 影响与约束

漏一处 → bin target E0432（例如在 `main.rs` 第 3-15 行未声明某模块，而 `main` 函数或 helper 逻辑中使用了该模块）。代价已知：同一份源码被编译两遍（构建时间翻倍），且 `#[macro_export]` 宏在 bin 与 lib 各有一份实例（`crate::log_error!` 见 `main.rs` 第 22 行，与 `campus_login_lib::log_error!` 是不同实例）；改动公共模块时两端都会重新编译，无法只改一端。此外，`main.rs` 第 21-25 行的 panic hook 依赖 `infra` 模块与 `log_error!` 宏，这要求 `infra` 模块在 `main.rs` 第 12 行及 `lib.rs` 第 5 行均被声明，否则 panic hook 无法编译。

## Connections

[[verification-baseline]]、[[android-generated-project-discipline]]