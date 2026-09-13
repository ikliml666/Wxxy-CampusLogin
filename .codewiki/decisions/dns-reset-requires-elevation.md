---
title: "DNS 一键还原必须与一键设置一样走提权，否则功能只做一半"
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/dns_config.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
tags: [决策, DNS, 提权, helper, 可逆性]
---

## 背景

`platform/dns_config.rs::clear_adapter_dns_via_api`（清除适配器级 DNS、恢复 DHCP 自动获取）早已实现，但全仓只有 `network/dns_setup.rs` 在 WiFi 设置流程内部调用它。命令面把它暴露出来看似是"把已有函数接到一个 `#[tauri::command]` 上"的一行活。

## 决策

新增 `reset_dns` 命令时，**必须同时补 helper 的 `clear_dns` op 并走与 `setup_dns_doh` 相同的提权路径**，而不是直接内联调用 `clear_adapter_dns_via_api`。

## 理由

`SetInterfaceDnsSettings`（清除适配器 DNS 走的是这条 Win32 路径）**需要管理员权限**。而一键设置 DNS 的 `setup_dns_doh` 在非管理员下是通过 `--helper` 重启自身提权执行的。若 `reset_dns` 只做内联直调：

- 管理员运行 → 正常工作
- 非管理员运行 → 清除必然失败

结果是"能设不能撤"：用户借提权设置成功，想撤销时却失败——**功能只做了一半，而且是最容易让用户困惑的一半**（设置成功过，所以用户预期还原也能成功）。

这类"设置/撤销"配对功能的正确性判据是**能力对等**：撤销路径的权限要求、范围解析、失败提示都应与设置路径一致，否则就是半成品。

## 影响与约束

- helper op 的参数与结果格式应与既有 `dns` op 同构：`HelperOp::ClearDns { targets: Vec<String> }`（适配器 GUID 列表），结果写入 `details.restored` / `details.failed`（`"名字: 错误"`），便于主进程解析失败明细。
- 不存在"只改主进程、helper 以后再说"的分步方案——**helper 协议的两端必须在同一次提交内对齐**（主进程与子进程的参数/结果格式），否则提权路径必然不通。
- UAC 被取消时必须给出明确提示（「需要管理员权限」），不能静默失败——用户取消 UAC 是常见操作，静默失败会让用户以为"点了没反应"。
- 适配器操作范围沿用既有约定：`resolve_adapter_names` + `filter_operation_adapters` + 黑名单过滤，只作用于主/副适配器。

## Connections

[[helper-self-restart-elevation]]、[[adapter-operation-scope]]、[[desktop-network-dns]]、[[verification-gate-tiers]]
