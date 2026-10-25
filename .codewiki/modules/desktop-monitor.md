---
title: Desktop Monitor
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/monitor/watcher.rs
  - tauri-app/src-tauri/src/monitor/background_task.rs
  - tauri-app/src-tauri/src/monitor/background_check.rs
  - tauri-app/src-tauri/src/monitor/portal_check.rs
  - tauri-app/src-tauri/src/monitor/auto_auth.rs
  - tauri-app/src-tauri/src/monitor/latency.rs
  - tauri-app/src-tauri/src/monitor/quality_scheduler.rs
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/campus_check.rs
  - tauri-app/src-tauri/src/monitor/background_emit.rs
  - tauri-app/src-tauri/src/monitor/outbound_night.rs
tags: [tauri, desktop, monitor, rust, network]

## Overview

`desktop-monitor` 模块对应于 Tauri 桌面端应用（`tauri-app`）中位于 `tauri-app/src-tauri/src/monitor/` 目录下的监控子系统。该模块预期负责桌面客户端的网络状态监听、后台轮询、校园网探测、认证 Portal 检测、延迟测量、出口调度及夜间策略等能力。

在本文生成时，所有相关的源文件均报告“（文件不存在或读取失败）”。因此，本文章仅能基于文件路径与命名约定描述预期结构，无法提供经过代码验证的具体实现、符号定义或控制流。

## Key Components

由于以下全部 13 个源文件在读取时均返回“（文件不存在或读取失败）”，无法提取任何公开函数、类、常量、结构体字段或方法，也没有任何有效的行号前缀可供引用：

- `tauri-app/src-tauri/src/monitor/mod.rs`
- `tauri-app/src-tauri/src/monitor/watcher.rs`
- `tauri-app/src-tauri/src/monitor/background_task.rs`
- `tauri-app/src-tauri/src/monitor/background_check.rs`
- `tauri-app/src-tauri/src/monitor/portal_check.rs`
- `tauri-app/src-tauri/src/monitor/auto_auth.rs`
- `tauri-app/src-tauri/src/monitor/latency.rs`
- `tauri-app/src-tauri/src/monitor/quality_scheduler.rs`
- `tauri-app/src-tauri/src/monitor/scheduled.rs`
- `tauri-app/src-tauri/src/monitor/outbound_switch.rs`
- `tauri-app/src-tauri/src/monitor/campus_check.rs`
- `tauri-app/src-tauri/src/monitor/background_emit.rs`
- `tauri-app/src-tauri/src/monitor/outbound_night.rs`

无任何可用行号、函数签名、数据类型或字段细节可供零上下文 agent 参考。

## Architecture

因模块内包含 13 个源文件（组件数量 ≥ 3），按要求提供框图。由于源码内容完全缺失，仅能基于文件系统层级展示组件（文件）的存在状态，无法推测或绘制运行时调用关系箭头：

```text
┌────────────────────────────────────────────────────┐
│ Module: tauri-app/.../monitor/ (全部读取失败)        │
│ ┌────────────┐ ┌────────────┐ ┌────────────┐       │
│ │ mod.rs     │ │ watcher.rs │ │ task.rs    │       │
│ └────────────┘ └────────────┘ └────────────┘       │
│ ┌────────────┐ ┌────────────┐ ┌────────────┐       │
│ │ check.rs   │ │ portal.rs  │ │ auth.rs    │       │
│ └────────────┘ └────────────┘ └────────────┘       │
│ ┌────────────┐ ┌────────────┐ ┌────────────┐       │
│ │ latency.rs │ │ quality.rs │ │ sched.rs   │       │
│ └────────────┘ └────────────┘ └────────────┘       │
│ ┌────────────┐ ┌────────────┐ ┌────────────┐       │
│ │ switch.rs  │ │ campus.rs  │ │ emit.rs    │       │
│ └────────────┘ └────────────┘ └────────────┘       │
│ ┌────────────┐                                      │
│ │ night.rs   │                                      │
│ └────────────┘                                      │
└────────────────────────────────────────────────────┘
```

## Data Flow

无源码与行号可供追踪实际数据流。基于文件名推测（非代码验证），未来的数据流可能如下：

1. `watcher.rs` 或 `scheduled.rs` 触发监控任务。
2. `background_task.rs` 与 `background_check.rs` 接管后台执行。
3. `portal_check.rs`、`campus_check.rs` 检测网络环境。
4. `latency.rs` 与 `quality_scheduler.rs` 评估链路质量。
5. `auto_auth.rs`、`outbound_switch.rs`、`outbound_night.rs` 执行对应网络动作。
6. `background_emit.rs` 向 Tauri 前端发送状态事件。

上述流程缺乏文件行号与函数调用佐证，仅供占位参考。

## Connections

- 主应用容器 [[tauri-app]]
- 可能的网络与认证逻辑 [[network-auth]]
- 定时任务基础设施 [[scheduler]]
- 前端状态同步 [[desktop-frontend]]

## Known Issues

| 文件 | 状态 | 具体问题 |
| --- | --- | --- |
| `tauri-app/src-tauri/src/monitor/mod.rs` | 读取失败 | 文件不存在或读取失败，无法分析模块导出 |
| `tauri-app/src-tauri/src/monitor/watcher.rs` | 读取失败 | 文件不存在或读取失败，无法分析监听逻辑 |
| `tauri-app/src-tauri/src/monitor/background_task.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/background_check.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/portal_check.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/auto_auth.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/latency.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/quality_scheduler.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/scheduled.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/outbound_switch.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/campus_check.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/background_emit.rs` | 读取失败 | 文件不存在或读取失败 |
| `tauri-app/src-tauri/src/monitor/outbound_night.rs` | 读取失败 | 文件不存在或读取失败 |

- 由于所有源文件均读取失败，无法确认是否存在被注释或禁用的功能（无法提供具体文件与行号）。
- 文档当前无法反映真实的 Rust 代码结构、Trait 实现或 Tauri Command 定义。