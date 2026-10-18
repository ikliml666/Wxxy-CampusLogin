---
title: "DNS 重置必须走提权路径，确保设置/撤销能力对等"
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
tags: [决策, DNS, 提权, helper, 可逆性]
---

## 背景

`platform/dns_config.rs::clear_adapter_dns_via_api`（清除适配器级 DNS、恢复 DHCP 自动获取）早已实现，但全仓只有 `network/dns_setup.rs` 在 WiFi 设置流程内部调用它。命令面把它暴露出来看似是“把已有函数接到一个 `#[tauri::command]` 上”的一行活。

然而，Windows 的 `SetInterfaceDnsSettings` API 需要管理员权限。如果 `reset_dns` 仅在内联路径中直接调用，非管理员用户将无法执行该操作，导致“能设不能撤”的功能残缺状态。

## 决策

新增 `reset_dns` 命令时，**必须同时补全 helper 的 `ClearDns` op 并走与 `setup_dns_doh` 相同的提权路径**，而不是直接内联调用 `clear_adapter_dns_via_api`。

具体实现要求：
1. **主进程侧**：在 `commands/network_cmd.rs` 中实现 `reset_dns` 命令，校验目标适配器列表。若当前进程已具备管理员权限，则直接调用 `dns_config::clear_adapter_dns_via_api`；否则，通过 `helper_spawn::spawn_elevated_helper` 触发提权子进程。
2. **Helper 侧**：在 `helper/mod.rs` 中新增 `HelperOp::ClearDns { targets: Vec<String> }` 变体，并实现对应的 `run_clear_dns` 函数。该函数需遍历 GUID 列表，逐个调用 `clear_adapter_dns_via_api`，并收集成功/失败明细。
3. **协议对齐**：Helper 返回的结果 JSON 必须包含 `details.restored` 和 `details.failed` 字段，格式为 `"适配器名: 错误信息"`，以便主进程解析并向用户反馈精确结果。

## 理由

### 1. 权限一致性
`SetInterfaceDnsSettings` 需要管理员权限。`setup_dns_doh` 在非管理员下是通过 `--helper` 重启自身提权执行的。若 `reset_dns` 只做内联直调：
- 管理员运行 → 正常工作
- 非管理员运行 → 清除必然失败

结果是“能设不能撤”：用户借提权设置成功，想撤销时却失败——**功能只做了一半，而且是最容易让用户困惑的一半**（设置成功过，所以用户预期还原也能成功）。

### 2. 能力对等原则
这类“设置/撤销”配对功能的正确性判据是**能力对等**：撤销路径的权限要求、范围解析、失败提示都应与设置路径一致，否则就是半成品。

### 3. 用户体验
UAC 被取消时必须给出明确提示（「需要管理员权限」），不能静默失败。用户取消 UAC 是常见操作，静默失败会让用户以为“点了没反应”。

## 影响与约束

### Helper Op 设计
- **参数结构**：`HelperOp::ClearDns { targets: Vec<String> }`，其中 `targets` 为适配器 GUID 列表。
- **结果格式**：写入 `details.restored` / `details.failed`（`"名字: 错误"`），便于主进程解析失败明细。
- **三态汇总文案**：由 helper 侧生成，与管理员路径保持一致：
    - 全部成功：“已恢复DNS自动获取（N 个适配器）”
    - 部分失败：“部分适配器恢复失败: 失败明细”
    - 全部失败：“恢复DNS自动获取失败: 失败明细”

### 实施约束
- **不存在分步方案**：不存在“只改主进程、helper 以后再说”的分步方案——**helper 协议的两端必须在同一次提交内对齐**（主进程与子进程的参数/结果格式），否则提权路径必然不通。
- **适配器操作范围**：沿用既有约定，只作用于 DNS 优化卡片内显式选择的适配器名单，并经过黑名单过滤。
- **错误处理**：非管理员运行且 UAC 被取消时，应返回明确的错误信息，而非静默失败。

## Connections

[[helper-self-restart-elevation]]、[[adapter-operation-scope]]、[[desktop-network-dns]]、[[verification-gate-tiers]]