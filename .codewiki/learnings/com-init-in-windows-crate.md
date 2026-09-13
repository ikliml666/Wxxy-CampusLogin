---
title: 不要手动 COM init（会破坏 windows crate 的 factory cache 自愈）
type: learning
source_files:
  - tauri-app/src-tauri/src/platform/dns_config.rs
tags: [教训, windows, com, win32]
---

## 现象

手动做 COM 初始化 / 反初始化后，相关 COM 调用出现异常。

## 根因

`windows` crate 的 factory cache 自带 `CoIncrementMTAUsage` 自愈机制，手动 init/uninit 反而破坏它。

## 解决

**不要手动 COM init**，交给 crate 自行处理。

## 教训

使用 `windows` crate 时不要自行调 `CoInitializeEx` / `CoUninitialize`；这类"补初始化"的直觉动作在本项目已被证实有害。

## Connections

[[platform-com-elevation-undocumented]]
