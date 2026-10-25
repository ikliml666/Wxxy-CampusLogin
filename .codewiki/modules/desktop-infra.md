---
title: Desktop Infra
type: module
source_files:
  - tauri-app/src-tauri/src/infra/mod.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/task_manager.rs
  - tauri-app/src-tauri/src/infra/lifecycle.rs
  - tauri-app/src-tauri/src/infra/notification.rs
  - tauri-app/src-tauri/src/infra/command_context.rs
  - tauri-app/src-tauri/src/infra/async_util.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/src-tauri/src/infra/state/store.rs
  - tauri-app/src-tauri/src/infra/state/network.rs
  - tauri-app/src-tauri/src/infra/state/exit.rs
tags: [desktop, tauri, infra, rust]

## Overview

本文档旨在描述 Tauri 桌面应用（`tauri-app`）中的后端基础设施模块 `Desktop Infra`，其预期源码根目录为 `tauri-app/src-tauri/src/infra/`。

在尝试重写此文档时，系统提供的所有 12 个源文件均返回“（文件不存在或读取失败）”。这意味着在当前代码库版本中，以下预期的子模块路径都无法被访问：

- 模块根：`tauri-app/src-tauri/src/infra/mod.rs`
- 日志系统：`tauri-app/src-tauri/src/infra/logger.rs`
- 事件系统：`tauri-app/src-tauri/src/infra/events.rs`
- 任务管理器：`tauri-app/src-tauri/src/infra/task_manager.rs`
- 生命周期管理：`tauri-app/src-tauri/src/infra/lifecycle.rs`
- 通知服务：`tauri-app/src-tauri/src/infra/notification.rs`
- 命令上下文：`tauri-app/src-tauri/src/infra/command_context.rs`
- 异步工具：`tauri-app/src-tauri/src/infra/async_util.rs`
- 状态管理根：`tauri-app/src-tauri/src/infra/state/mod.rs`
- 状态存储：`tauri-app/src-tauri/src/infra/state/store.rs`
- 网络状态：`tauri-app/src-tauri/src/infra/state/network.rs`
- 退出状态：`tauri-app/src-tauri/src/infra/state/exit.rs`

对于零上下文的未来 agent：当前无法从代码层面确认该模块是否已被删除、重命名或整体迁移。请勿依赖任何历史文档中关于函数签名或结构体字段的描述。

## Key Components

由于所有源文件均返回“文件不存在或读取失败”，无法提取任何行号，也无法列出任何公开函数、结构体（struct）、常量（const）、枚举（enum）、字段或方法（包括构造函数、`save()` 等）。

本文档不能凭空捏造行号与函数名。所有 12 个预期文件的读取状态如下：

- `tauri-app/src-tauri/src/infra/mod.rs`：读取失败，无可用导出符号。
- `tauri-app/src-tauri/src/infra/logger.rs`：读取失败，无可用日志配置结构体或 `init` 函数。
- `tauri-app/src-tauri/src/infra/events.rs`：读取失败，无可用事件分发结构体或监听方法。
- `tauri-app/src-tauri/src/infra/task_manager.rs`：读取失败，无可用任务句柄或调度方法。
- `tauri-app/src-tauri/src/infra/lifecycle.rs`：读取失败，无可用生命周期钩子。
- `tauri-app/src-tauri/src/infra/notification.rs`：读取失败，无可用系统通知 API 封装。
- `tauri-app/src-tauri/src/infra/command_context.rs`：读取失败，无可用 Tauri 命令上下文类型。
- `tauri-app/src-tauri/src/infra/async_util.rs`：读取失败，无可用异步辅助宏或函数。
- `tauri-app/src-tauri/src/infra/state/mod.rs`：读取失败，无可用状态聚合定义。
- `tauri-app/src-tauri/src/infra/state/store.rs`：读取失败，无可用全局存储结构。
- `tauri-app/src-tauri/src/infra/state/network.rs`：读取失败，无可用网络状态跟踪类型。
- `tauri-app/src-tauri/src/infra/state/exit.rs`：读取失败，无可用退出信号或状态机。

## Architecture

由于无法读取任何源文件，无法确认当前组件及依赖关系，因此无法绘制 UTF-8 框图。预期中该模块会作为桌面应用后端的底层支撑，为上层业务逻辑提供日志、事件与状态能力，但当前代码库中无实体可供剖析。

## Data Flow

因所有源文件缺失或不可访问，无法分析 `infra` 模块内部的数据流向，也无法确认其与 Tauri 命令调用层或外部系统的数据交互管道。

## Connections

由于模块实体在当前代码库中不可见，无法建立确切的相互依赖链接。建议未来 agent 在确认该模块的真实路径后，将其与以下相关模块建立关联：

- 可能的上层业务模块：[[tauri-commands]]
- 可能的应用入口模块：[[desktop-app-entry]]
- 历史状态管理关联：[[desktop-state]]

## Known Issues

- 严重源码缺失：文首列出的全部 12 个源文件均报告“文件不存在或读取失败”，导致文档缺乏任何行号、函数签名、类型定义等代码级细节。
- 历史文档失真：原 `modules/desktop-infra.md` 已过期，且当前无法从代码证实其原有结构，任何关于此模块的旧有认知均需被标记为不可信，直到源码路径被修复。