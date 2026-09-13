---
title: 两端配置字段集不是子集关系，而是交集 + 双向差集
type: learning
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - android/src-tauri/src/config_state.rs
tags: [教训, 双端, 配置, 同步]
---

## 现象

新增配置字段时无法照抄某一端——按"双端同步"的直觉去抄会抄错。

## 根因

两端字段集不是子集关系：交集 31 个字段，**桌面独有 13 个**（双适配器三件、托盘/隐藏启动、退出策略、`campus_exit_*` 三件、`skip_sha256_when_missing`、`config_version`），**安卓独有 4 个**（`allow_2d_face_verify`、`background_check_idle_interval`、`enable_boot_autostart`、`config_schema_version`）。桌面 44、安卓 35。

另有两套易混编号：`config_version`（桌面 2）与 `config_schema_version`（安卓 4）语义不同却名字相近；两端 `default_panel` 默认值也不同（安卓 `"dashboard"`、桌面空串）。

## 解决

"双端同步"按**双向增量同步**执行：先判断字段属于两端通用还是平台专属，再决定是否两端各加一份。

## 教训

① 新增配置字段时对照 `config/model.rs` 与 `config_state.rs` 两份定义，别假设一端是另一端的超集；② 改默认值要走后端 `Settings::default` +（安卓）schema 迁移；③ 不要混淆 `config_version` 与 `config_schema_version`。

## Connections

[[dual-tree-sync-human-discipline]]、[[android-interval-default-schema-migration]]、[[config-missing-field-load-failure]]
