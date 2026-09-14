---
title: 禁用网卡分类盲区——只认 NotPresent+ConfigFlags 漏掉「禁用报 Down」
type: learning
source_files:
  - tauri-app/src-tauri/src/network/discovery/windows.rs
  - tauri-app/src-tauri/src/monitor/adapter_watch.rs
tags: [教训, 适配器, 禁用分类, 自动启用, API语义, 历史缺陷]
---

## 现象

用户禁用手选网卡后，应用设计的「自动启用被禁用网卡」（adapter_watch 15s 巡检）从未触发——
日志中连一条「检测到手选适配器被禁用，尝试自动启用」都没有（2026-09-14 本机复现实锤）。

## 根因

`discovery/windows.rs` 四分类只把「`OperStatus == IfOperStatusNotPresent` **且** 注册表
`ConfigFlags` 含 `CONFIGFLAG_DISABLED (0x1)`」判为 Disabled，其余一切归为「未连接」。两个误区：

1. **微软文档明文**（MIB_IF_ROW2 / IP_ADAPTER_ADDRESSES_LH）：AdminStatus=Down（禁用）时
   OperStatus「Down **或** NotPresent 两者皆可能」——只认 NotPresent 有半个盲区；
   psutil 等业界实现判禁用的可靠依据是 **AdminStatus**（禁用→Down；拔线→保持 Up）。
2. **ConfigFlags 是未文档化行为**（DEVPKEY_Device_ConfigFlags "internal use only"），
   且开机重新枚举/驱动重装会洗掉该位（本机实测：USB 网卡开机时重装 rtump64 驱动）。

原注释「Down 在 Windows 上实际语义是接口未就绪，不是管理员禁用」把两个互斥场景
（媒体断开 vs 管理性关闭）混为一谈——区分二者的正是 AdminStatus，而不是 OperStatus。

## 解决

决策提取为纯函数 `classify_adapter_status`（8 用例决策表单测）：
`AdminStatus == Down` → Disabled（GetIfEntry2 查询失败返回 None 回退旧行为）；
NotPresent 时保留 ConfigFlags 位作回退（幽灵虚拟副本无接口行，查不到 AdminStatus）。

## 教训

- 「文档说 A 或 B」时只实现 A 就是半个 bug——枚举值可能值集都要覆盖，或找到区分二者的
  另一个字段（此处是 AdminStatus）。
- 判定禁用/连接状态这类「系统状态映射」，优先选**文档化语义**字段，未文档化的注册表
  位只能作回退佐证。
- 自动启用类「无事件依赖的巡检触发」必须配合分类正确性验证——分类漏判时它静默不触发，
  日志无任何痕迹，比报错更难查。

## Connections

[[adapter-visibility-cache-staleness]]、[[adapter-operation-scope]]
