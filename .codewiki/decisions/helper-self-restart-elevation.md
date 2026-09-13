---
title: 提权操作用 --helper 重启自身，弃用 PowerShell
type: decision
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
  - tauri-app/src-tauri/src/platform/elevation.rs
tags: [决策, 提权, helper, windows, dns, mac]
---

## 背景

DNS 设置、MAC 伪装等操作需要管理员权限。早期通过 PowerShell 提权执行。

## 决策

提权操作用 `--helper` **重启自身**子进程执行，弃用 PowerShell：Rust 直调 Win32/winreg。

## 理由

移除 shell 拼接注入面与 `-EncodedCommand` 编码坑，耗时 200-500ms → 1ms。

## 备选方案

PowerShell（含 `-EncodedCommand`）——因改为自身重启而被弃用，原因即上条注入面与编码坑。

## 影响与约束

新增提权操作要走 helper 分派（`HelperOp`），结果经 `--result` 文件回传。已知缺陷：`write_result_file` 忽略全部 IO 错误（`helper/mod.rs:190-197`），写失败时主进程只能等到 25s/30s 超时；helper 无自我超时、无重入保护；MAC 伪装值清除失败仍报成功（`helper/mod.rs:168-176`）。

## Connections

[[platform-com-elevation-undocumented]]、[[single-notification-channel]]
