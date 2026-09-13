---
title: MASK 占位符被直接落盘会变明文密码 "***"
type: learning
source_files:
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
tags: [教训, 安全, 配置, 密码, 落盘]
---

## 现象

任何把 `PASSWORD_MASK` 占位符直接落盘的调用方，重启后会得到**明文密码 `"***"`**。

## 根因

`save_config_to_disk_encrypted`（`persist.rs:153-168`）直接落盘传入的 `Config`，**不做校验**；且 `persist.rs:159-164` 明确**不排除** `PASSWORD_MASK`——注释说明理由：若真实密码恰为 `"***"`，排除判断会让它明文落盘。

## 解决

当前调用方 `commands/config_cmd.rs:107-123` 已在落盘前还原真值——属**依赖调用方正确性**。

## 教训

把 MASK 还原真值是**写盘方的责任**，落盘函数不做保护。新增任何配置写盘路径时，必须先经"空串/MASK = 回退已存值、显式清除走 clear 标志"的语义还原，再落盘。

## Connections

[[config-mask-single-exit]]、[[config-write-lock-coverage]]、[[android-identifier-change-breaks-keystore]]
