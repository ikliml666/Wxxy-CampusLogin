---
title: bin 与 lib 是两棵独立模块树，新增顶层模块必须两处声明
type: decision
source_files:
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/lib.rs
tags: [决策, 构建, 模块树, rust]
---

## 背景

同一份源码需要同时被 bin target 与 lib target 使用（安卓经 lib 复用桌面 crate）。

## 决策

`main.rs` 与 `lib.rs` 各声明一份模块树，新增顶层模块**必须两处同时声明**。

## 理由

旧文档只写了违反后果（漏一处 bin target E0432），未记录为何不用 `main.rs` 复用 `campus_login_lib::`。

## 备选方案

旧文档未记录。

## 影响与约束

漏一处 → bin target E0432。代价已知：同一份源码被编译两遍（构建时间翻倍），且 `#[macro_export]` 宏在 bin 与 lib 各有一份实例（`crate::log_info!` 与 `campus_login_lib::log_info!` 是不同实例）；改动公共模块时两端都会重新编译，无法只改一端。

## Connections

[[verification-baseline]]、[[android-generated-project-discipline]]
