---
title: 配置文件并发写保护靠调用方持有锁（覆盖不全即互相覆盖）
type: learning
source_files:
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/account_cmds.rs
  - tauri-app/src-tauri/src/config/persist.rs
tags: [教训, 配置, 并发, 锁]
---

## 现象

多个写路径并发时可能互相覆盖：安卓 `config_io_lock` 只被部分命令持有，**`save_config`（`config_state.rs:312`，其 `save_to` 调用在 :332）完全没取锁**，正是注释里点名的并发读改写竞态对象之一；tmp 文件名固定（`.json.tmp`，:219，其后只有 write + rename，无 `sync_all` / rename 重试）；`load_from` 路径内的 schema 迁移落盘（`config_state.rs:257`、`:264`）同样在锁外——它与 `save_config` 并发时仍可能互相覆盖。

2026-09-13 复核：本轮新增的写路径（`monitor_loop.rs:331-339`、`:355-363`）都取了 `config_io_lock`，纪律在本轮被遵守；持锁命令清单见 `account_cmds.rs:127`、`:156`、`:179`。

## 根因

并发保护**完全依赖调用方记得持锁**，锁不是写路径内部的强制环节。

## 解决

新增写路径必须取锁（当前是靠纪律与注释约束）。

## 教训

① 碰配置写入时先确认是否持有 `config_io_lock` / `LOGIN_HISTORY_LOCK`；② 同类问题在桌面也存在——`LOGIN_HISTORY_LOCK` 只覆盖 `append_login_history` 一处（`persist.rs:10` 定义、`:100` 取锁），且桌面用 `Mutex<()>` 静态锁，多进程（如 `--helper` 提权子进程）并发写同一文件时锁无效。加写路径时把"锁在哪、覆盖哪些路径"一并写清。

## Connections

[[mask-placeholder-persisted-as-plaintext]]、[[config-missing-field-load-failure]]
