---
title: "SetIpInterfaceEntry 写 metric 的必踩点与字段对照"
type: learning
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/metric.rs
tags: [learning, metric, 跃点, iphlpapi, SetIpInterfaceEntry, GetIpInterfaceTable, 夜间出站, windows]

## Overview

在 CampusLogin Tauri 应用中，夜间出站切换（Outbound Switch）需要调整网络适配器的接口跃点（Metric），使特定网卡优先用于出站流量。该功能的核心在于对 Windows IP Helper API `SetIpInterfaceEntry` 的正确使用。

旧版实现中曾遇到两个典型失败：
1. 自行拼装仅包含部分字段的 `MIB_IPINTERFACE_ROW` 结构体，导致 `SetIpInterfaceEntry` 返回 `ERROR_INVALID_PARAMETER`。
2. 保留从系统读回的原行，仅修改 `Metric` 字段后回写，同样报 `ERROR_INVALID_PARAMETER`。

当前代码（基于 `tauri-app/src-tauri/src/helper/mod.rs` 和 `tauri-app/src-tauri/src/platform/metric.rs`）已修复这些问题：写路径通过 `interface_rows_for_guid` 拿到完整的当前生效行，改副本并显式将 `SitePrefixLength` 重置为 0，再调用 `SetIpInterfaceEntry`，确保写入成功。读路径无需提权，仅做快照与状态判定。

## Key Components

### 1. 读取模块 (`platform/metric.rs`)

负责枚举当前生效的接口跃点快照，无需管理员权限。

*   **`MetricRow` Struct** (行 16-20)
    *   **定义**：
        ```rust
        pub struct MetricRow {
            pub family: u16,     // AF_INET=2 / AF_INET6=23
            pub automatic: bool, // UseAutomaticMetric
            pub metric: u32,
        }
        ```
    *   **字段映射**（由 `read_interface_metrics` 填充）：
        *   `family`: 对应 `row.Family.0`（协议栈，2=IPv4 / 23=IPv6）。
        *   `automatic`: 对应 `row.UseAutomaticMetric.0 != 0`。
        *   `metric`: 对应 `row.Metric`。

*   **`interface_rows_for_guid(guid: &str)`** (行 25-53)
    *   **用途**：枚举接口表中与指定 GUID 匹配的所有原始 `MIB_IPINTERFACE_ROW` 行（读快照与 helper 写 metric 共用同一份枚举）。
    *   **逻辑**：调用 `GetIpInterfaceTable(AF_UNSPEC)` 获取全表（行 30），遍历每一行，使用 `ConvertInterfaceLuidToGuid` 将 LUID 转换为 GUID（行 42）。若转换成功且 GUID 匹配目标（行 45），则将该行加入结果集。最后调用 `FreeMibTable` 释放内存（行 50）。
    *   **注意**：LUID→GUID 转换失败的行会被跳过（行 43）；GUID 无匹配行时返回空 `Vec`。入参 GUID 解析复用 `crate::platform::elevation::parse_guid`（行 26，剥花括号、忽略大小写）。

*   **`read_interface_metrics(guid: &str)`** (行 56-65)
    *   **用途**：对外暴露的公开函数，将底层 `MIB_IPINTERFACE_ROW` 转换为应用层 `MetricRow` 列表。
    *   **返回值**：`Result<Vec<MetricRow>, String>`，直接映射 `interface_rows_for_guid` 的结果。

*   **测试用例** (`mod tests`, 行 67-84)
    *   `read_interface_metrics_smoke` (行 74-83)：真机冒烟测试（默认忽略），验证读路径无需提权即可正确枚举首个适配器的跃点，断言 `family` 只应是 2 或 23。

### 2. 写入模块 (`helper/mod.rs`)

负责在提权上下文（Helper Worker / `--helper` 提权副本）中执行实际的跃点设置。

*   **`HelperOp::SetMetric`** (行 149-150，位于 `enum HelperOp` 定义内)
    *   **定义**：
        ```rust
        #[cfg(target_os = "windows")]
        SetMetric { rows: Vec<String> },
        ```
    *   **用途**：定义支持的操作类型，`rows` 为编码后的条目列表（如 `"{guid}:{family}:{automatic}:{metric}"`）。该变体仅在 Windows 下编译。

*   **`decode_set_metric_row(row: &str)`** (行 610-640)
    *   **用途**：安全解码来自请求文件的字符串条目 `"{guid}:{family}:{automatic}:{metric}"`。条目来自 `ProgramData` 下任何本地用户可写的请求文件，且执行方是 SYSTEM，故每个字段严格校验后才交给 Win32。
    *   **校验**：按 `:` 拆分后必须恰为 4 段（行 612）；GUID 非空（行 617）；`family` 为 `"2"` 或 `"23"`（行 620-628）；`automatic` 为 `"0"` 或 `"1"`（行 629-635）；`metric` 为合法 `u32`（行 636-638）。
    *   **返回值**：`Result<(String, u16, bool, u32), String>`，分别为（原始带花括号 GUID、协议栈、是否自动跃点、静态跃点值）。

*   **`run_set_metric(rows: &[String], logs: &mut Vec<String>)`** (行 645-727，标注 `#[cfg(target_os = "windows")]`)
    *   **用途**：核心写入逻辑。逐条解码编码条目，定位接口行，修改副本，调用 `SetIpInterfaceEntry`。提权上下文内执行（主进程无接口写权限）。
    *   **关键步骤**：
        1.  **空条目检查**（行 658-660）：若 `rows` 为空，直接返回失败，防止误判切换已生效。
        2.  **枚举与定位**（行 674）：调用 `crate::platform::metric::interface_rows_for_guid(&guid)` 获取候选行；用 `find` 按 `r.Family.0 == family` 定位当前行（行 685）。
        3.  **副本修改**（行 691-694）：
            *   `target.UseAutomaticMetric = BOOLEAN(automatic as u8);`（行 691）
            *   `target.Metric = metric;`（行 692）
            *   **`target.SitePrefixLength = 0;`**（行 694）：**必踩点**。必须显式置 0，否则 `SetIpInterfaceEntry` 报 `ERROR_INVALID_PARAMETER`（等价 EasyTier/mullvad 同款实现）。
        4.  **执行写入**（行 695）：`let rc = unsafe { SetIpInterfaceEntry(&mut target) };`。
        5.  **结果汇总**（行 711-726）：统计 `applied` 与 `failed`，全成功则 `success: true`，否则失败并拼接明细。

*   **分发入口**（提权执行链路）
    *   `run_helper` 中匹配 `HelperOp::SetMetric` 并调用 `run_set_metric`（行 308-309）。
    *   计划任务 worker `process_request` 中同样分发（行 431-432）。

*   **测试用例** (`mod tests` 中 `set_metric_round_trip_and_reject`, 行 1476-1524)
    *   验证 `parse_helper_args` + `build_op_from_args` 构造出 `HelperOp::SetMetric { rows }`（行 1492-1508），并直接调用 `decode_set_metric_row` 检查合法 round-trip（行 1499-1505）与非法输入拒绝（行 1511-1523，覆盖字段数错误、空 GUID、非法 family/automatic/metric 等）。

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
    *   主进程或 Helper 调用 `read_interface_metrics(guid)`（行 56-65）。
    *   `interface_rows_for_guid`（行 25-53）调用 `GetIpInterfaceTable(AF_UNSPEC)` 获取运行时接口表。
    *   遍历表项，通过 `ConvertInterfaceLuidToGuid` 匹配目标 GUID。
    *   提取 `Family`、`Metric`、`UseAutomaticMetric` 构建 `MetricRow` 列表。
    *   **关键点**：读到的 `SitePrefixLength` 可能非 0，但读操作不受影响；写时才需显式清零。

2.  **写入阶段 (Write)**：
    *   主进程通过 Helper 通道发起 `SetMetric` 请求（如计划任务 worker 读 `TaskRequest` 后 `build_op_from_args` 构造 `HelperOp::SetMetric`），传入编码字符串如 `"{GUID}:2:0:1"`。
    *   Helper Worker (`run_set_metric`, 行 645-727) 接收请求。
    *   调用 `decode_set_metric_row`（行 610-640）解析参数，严格校验格式。
    *   再次调用 `interface_rows_for_guid`（行 674）获取当前生效的行（确保拿到最新的 `InterfaceLuid`、`InterfaceIndex` 等只读/校验字段）。
    *   **关键修正**：在修改副本时，强制设置 `target.SitePrefixLength = 0`（行 694）。
    *   调用 `SetIpInterfaceEntry`（行 695）提交更改。
    *   记录成功/失败明细到 `logs` 和 `message`（行 711-726），由 `write_result_file` 回写固定结果目录。

## Connections

- [[desktop-platform]] —— `platform/metric.rs` 的逐项清单与 cfg 门控。
- [[desktop-helper-update]] —— `HelperOp::SetMetric` 的编码条目与 `run_set_metric`。
- [[outbound-switch]] —— 判定纯函数层（何时 Switch / Restore）。
- [[windows-task-proxy-elevation]] —— 写操作走的提权通道（SYSTEM worker → `--helper-task`）。