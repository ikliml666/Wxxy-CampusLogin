---
title: 本机（UTF-8）正常、用户机（GBK 代码页）关键字匹配失效
type: learning
source_files:
  - tauri-app/src-tauri/src/network/subnet.rs
  - tauri-app/src-tauri/src/network/dhcp.rs
tags: [教训, 编码, windows, 本地化, netsh]
---

## 现象

开发机（UTF-8 模式）一切正常，用户机（GBK 代码页）上关键字匹配全部失效。

## 根因

`netsh` / `ipconfig` / Portal 响应的编码随系统代码页变化，硬编码按 UTF-8 解码必然失配。

## 解决

解码统一 **UTF-8 优先 → OEM 回退**（`decode_console_bytes` / `decode_charset_bytes`）。

## 教训

解析命令行工具输出（`netsh` / `ipconfig`）时**不得假定编码**；关键字匹配还要考虑本地化语言（已有 `profile` / `配置文件` / `設定檔` 三语兜底），未覆盖的语言会静默降级为 `Ok(None)`。开发机与用户机的代码页差异必须纳入验证清单。

## Connections

[[ipconfig-failure-exit-code-zero]]
