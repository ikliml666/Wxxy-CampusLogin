---
title: "SetIpInterfaceEntry 写 metric 的必踩点与字段对照"
type: learning
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/metric.rs
tags: [learning, metric, 跃点, iphlpapi, SetIpInterfaceEntry, GetIpInterfaceTable, 夜间出站, windows]
---

## Overview

在 CampusLogin Tauri 应用中，夜间出站切换（Outbound Switch）需要调整网络适配器的接口跃点（Metric），使特定网卡优先用于出站流量。该功能的核心在于对 Windows IP Helper API `SetIpInterfaceEntry` 的正确使用。

旧版实现中曾遇到两个典型失败：
1. 自行拼装仅包含 `Family`、`InterfaceLuid`、`UseAutomaticMetric`、`Metric` 的 `MIB_IPINTERFACE_ROW` 结构体，导致 `SetIpInterfaceEntry` 返回 `ERROR_INVALID_PARAMETER`。
2. 保留从系统读回的原行，仅修改 `Metric` 字段后回写，同样报 `ERROR_INVALID_PARAMETER`。

当前代码（基于 `tauri-app/src-tauri/src/helper/mod.rs` 和 `tauri-app/src-tauri/src/platform/metric.rs`）已修复这些问题，通过整行拷贝副本并显式重置关键校验字段来确保写入成功。

## Key Components

### 1. 读取模块 (`platform/metric.rs`)

负责枚举当前生效的接口跃点快照，无需管理员权限。

*   **`interface_rows_for_guid(guid: &str)`** (行号 34-58)
    *   **用途**：枚举接口表中与指定 GUID 匹配的所有原始 `MIB_IPINTERFACE_ROW` 行。
    *   **逻辑**：调用 `GetIpInterfaceTable(AF_UNSPEC)` 获取全表，遍历每一行，使用 `ConvertInterfaceLuidToGuid` 将 LUID 转换为 GUID。若转换成功且 GUID 匹配目标，则将该行加入结果集。最后调用 `FreeMibTable` 释放内存。
    *   **注意**：LUID→GUID 转换失败的行会被跳过；GUID 无匹配行时返回空 `Vec`。入参 GUID 解析复用 `platform/elevation.rs` 的 `parse_guid`（剥花括号、忽略大小写）。

*   **`read_interface_metrics(guid: &str)`** (行号 61-67)
    *   **用途**：对外暴露的公开函数，将底层 `MIB_IPINTERFACE_ROW` 转换为应用层 `MetricRow`。
    *   **返回值**：`Result<Vec<MetricRow>, String>`。

*   **`MetricRow` Struct** (行号 16-20)
    *   **定义**：
        ```rust
        pub struct MetricRow {
            pub family: u16,     // AF_INET=2 / AF_INET6=23
            pub automatic: bool, // UseAutomaticMetric
            pub metric: u32,
        }
        ```
    *   **字段映射**：
        *   `family`: 对应 `row.Family.0`。
        *   `metric`: 对应 `row.Metric`。
        *   `automatic`: 对应 `row.UseAutomaticMetric.0 != 0`。

*   **测试用例** (`mod tests`, 行号 70-85)
    *   `read_interface_metrics_smoke`：真机冒烟测试，验证读路径无需提权即可正确枚举首个适配器的跃点。

### 2. 写入模块 (`helper/mod.rs`)

负责在提权上下文（Helper Worker）中执行实际的跃点设置。

*   **`decode_set_metric_row(row: &str)`** (行号 436-463)
    *   **用途**：安全解码来自请求文件的字符串条目 `"{guid}:{family}:{automatic}:{metric}"`。
    *   **校验**：严格检查字段数量、GUID 非空、Family 为 "2" 或 "23"、Automatic 为 "0" 或 "1"、Metric 为合法 `u32`。
    *   **返回值**：`Result<(String, u16, bool, u32), String>`。

*   **`run_set_metric(rows: &[String], logs: &mut Vec<String>)`** (行号 466-539)
    *   **用途**：核心写入逻辑。逐条解码编码条目，定位接口行，修改副本，调用 `SetIpInterfaceEntry`。
    *   **关键步骤**：
        1.  **空条目检查**：若 `rows` 为空，直接返回失败，防止误判。
        2.  **枚举与定位**：调用 `crate::platform::metric::interface_rows_for_guid(&guid)` 获取候选行。
        3.  **副本修改**：找到匹配的 `target` 行后，进行如下修改：
            *   `target.UseAutomaticMetric = BOOLEAN(automatic as u8);`
            *   `target.Metric = metric;`
            *   **`target.SitePrefixLength = 0;`** (行号 525)：**必踩点**。必须显式置 0，否则 `SetIpInterfaceEntry` 会因校验失败报错。
        4.  **执行写入**：调用 `unsafe { SetIpInterfaceEntry(&mut target) }`。
        5.  **结果汇总**：统计成功/失败条目，生成日志与消息。

*   **`HelperOp::SetMetric`** (行号 158-163)
    *   **定义**：
        ```rust
        #[cfg(target_os = "windows")]
        SetMetric { rows: Vec<String> },
        ```
    *   **用途**：定义支持的操作类型，`rows` 为编码后的条目列表。

*   **测试用例** (`mod tests`, 行号 628-665)
    *   `set_metric_round_trip_and_reject`：验证编码条目解析及非法输入拒绝逻辑。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Main Process                           │
│  ┌──────────────────┐       ┌───────────────────────────┐   │
│  │ outbound_switch  │────►  │ apply_outbound_restore    │   │
│  │ (判定逻辑)       │       │ (启动对账/收敛)           │   │
│  └──────────────────┘       └───────────┬───────────────┘   │
│                                        │                    │
│              ┌─────────────────────────▼───────────────┐     │
│              │          Helper (提权进程)               │     │
│              │  ┌───────────────────────────────────┐  │     │
│              │  │ run_helper_task / run_helper      │  │     │
│              │  │   └─► HelperOp::SetMetric         │  │     │
│              │  │         └─► decode_set_metric_row │  │     │
│              │  │         └─► interface_rows_for_...│  │     │
│              │  └───────────────────────────────────┘  │     │
│              └─────────────────────────┬───────────────┘     │
│                                        │                    │
│              ┌─────────────────────────▼───────────────┐     │
│              │       platform/metric.rs (读侧)          │     │
│              │  ┌───────────────────────────────────┐  │     │
│              │  │ GetIpInterfaceTable(AF_UNSPEC)    │  │     │
│              │  │ ConvertInterfaceLuidToGuid        │  │     │
│              │  │ FreeMibTable                      │  │     │
│              │  └───────────────────────────────────┘  │     │
│              └─────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **读取阶段 (Read)**：
    *   主进程或 Helper 调用 `read_interface_metrics(guid)`。
    *   `interface_rows_for_guid` 调用 `GetIpInterfaceTable` 获取运行时接口表。
    *   遍历表项，通过 `ConvertInterfaceLuidToGuid` 匹配目标 GUID。
    *   提取 `Family`、`Metric`、`UseAutomaticMetric` 构建 `MetricRow` 列表。
    *   **关键点**：读到的 `SitePrefixLength` 可能非 0，但读操作不受影响。

2.  **写入阶段 (Write)**：
    *   主进程通过 Helper 通道发起 `SetMetric` 请求，传入编码字符串如 `"{GUID}:2:0:1"`。
    *   Helper Worker (`run_set_metric`) 接收请求。
    *   调用 `decode_set_metric_row` 解析参数，严格校验格式。
    *   再次调用 `interface_rows_for_guid` 获取当前生效的行（确保拿到最新的 `InterfaceLuid`、`InterfaceIndex` 等只读/校验字段）。
    *   **关键修正**：在修改副本时，强制设置 `target.SitePrefixLength = 0`。
    *   调用 `SetIpInterfaceEntry` 提交更改。
    *   记录成功/失败明细到 `logs` 和 `message`。

## Connections

- [[desktop-platform]] —— `platform/metric.rs` 的逐项清单与 cfg 门控。
- [[desktop-helper-update]] —— `HelperOp::SetMetric` 的编码条目与 `run_set_metric`。
- [[outbound-switch]] —— 判定纯函数层（何时 Switch / Restore）。
- [[windows-task-proxy-elevation]] —— 写操作走的提权通道（SYSTEM worker → `--helper-task`）。