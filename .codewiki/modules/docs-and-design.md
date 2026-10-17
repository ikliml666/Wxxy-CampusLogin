---
title: docs-and-design
type: module
source_files:
  - archx_f5aw7l0f/arch_final.md
  - archx_f5aw7l0f/arch_receipt.md
  - archx_f5aw7l0f/pony_final.md
  - docs/changelog-entry-rtss.md
tags: [documentation, design, research, superpowers, changelog, audit]
---

## Overview

`docs-and-design` 模块负责维护项目的文档体系、设计原型、研究论文以及超级能力（Superpowers）规划与规格说明。该模块是 CodeWiki 知识沉淀的核心载体，涵盖了从架构审计（Architecture Audit）、深化机会分析（Deepening Opportunities）到发布日志（Changelogs）和版本发布说明（Release Notes）的全生命周期记录。

当前上下文中的关键文档包括：
1. **架构分析与深化机会**：针对 `Wxxy-CampusLogin` 代码库的源码审计，识别出 6 个深化机会候选，重点关注“浅模块”拆分、“双端真缝”收敛及死代码删除。
2. **RTSS 白屏崩溃根因定位**：记录了 2026-09-14 发现的 RivaTuner Statistics Server (RTSS) 注入导致 WebView2 白屏的根因分析及解决方案。
3. **设计规范与研究**：包含出站切换设计、账户绑定计划等 Superpowers 规格说明。

## Key Components

### 1. 架构深化机会报告 (Arch Receipt)
基于对源码的深度阅读与分析，生成了结构化的深化机会回执。主要组件包括以下 6 个候选项：

*   **候选 1：夜间出站切换实现外移**
    *   **文件**：`tauri-app/src-tauri/src/monitor/scheduled.rs`
    *   **行号**：`1-1959` (现状), `116` (入口), `239-1401` (出站实现区段)
    *   **用途**：将 `scheduled.rs` 中约 1100 行的出站逻辑搬移至新文件 `outbound_night.rs`，保留 30s 循环与定时编排。
    *   **强度**：Strong

*   **候选 2：IPC 命令面收敛**
    *   **文件**：`tauri-app/src-tauri/src/app/startup.rs`, `android/src-tauri/src/lib.rs`, `android/frontend/src/hooks/tauriApi.ts`
    *   **行号**：`startup.rs:66-134` (桌面 65 条), `lib.rs:66-122` (安卓 53 条), `tauriApi.ts:203-332` (17 处假接口)
    *   **用途**：聚合只读查询至 `get_init_data`，删除安卓侧 `desktopOnly` 假接口，减少 IPC 往返。
    *   **强度**：Worth exploring

*   **候选 3：删除恒零字段与死命令**
    *   **文件**：`tauri-app/src-tauri/src/auth/portal.rs`, `android/src-tauri/src/campus_detect.rs`
    *   **行号**：`portal.rs:90` (`data_length`), `campus_detect.rs:217` (`detect_campus`)
    *   **用途**：删除恒为 0 的 `data_length` 字段及无前端调用的 `detect_campus` 死命令。
    *   **强度**：Strong

*   **候选 4：网卡角色解析深模块**
    *   **文件**：`tauri-app/frontend/src/network/NetworkPanel.tsx`, `tauri-app/src-tauri/src/network/adapter.rs`
    *   **行号**：`NetworkPanel.tsx:195-215`, `adapter.rs:66-160`
    *   **用途**：收敛主/副/出站语义，删除 `disabledAdapters` 双通道。
    *   **强度**：Worth exploring

*   **候选 5：定时编排下沉共享 Crate**
    *   **文件**：`tauri-app/src-tauri/src/monitor/scheduled.rs`, `android/src-tauri/src/monitor_loop.rs`
    *   **行号**：`scheduled.rs:116-235`, `monitor_loop.rs:835-1038`
    *   **用途**：将双端同构的互斥门控逻辑下沉至共享 crate，避免隐性分叉。
    *   **强度**：Worth exploring

*   **候选 6：前端热点文件拆分**
    *   **文件**：`tauri-app/frontend/src/network/NetworkPanel.tsx`, `tauri-app/frontend/src/account/AccountPanel.tsx`
    *   **行号**：`NetworkPanel.tsx:195` (1072 行), `AccountPanel.tsx:58` (855 行)
    *   **用途**：按子流程（DNS、DHCP、出站、运营商绑定）拆分大文件，提升局部性 (locality)。
    *   **强度**：Worth exploring

### 2. RTSS 崩溃修复记录
*   **文件**：`docs/changelog-entry-rtss.md`
*   **行号**：全文
*   **用途**：记录 WebView2 白屏崩溃的根因（RTSSHooks64.dll 注入）及自动检测告警方案的实现。
*   **关键发现**：RTSS 7.3.7 忽略手放入 Profiles 目录的 cfg 文件，排除需手动完成。

### 3. 审计发现 (Pony Final)
*   **文件**：`archx_f5aw7l0f/pony_final.md`
*   **行号**：引用了多个热点文件，如 `scheduled.rs:565-797` (巨型函数), `NetworkPanel.tsx:731-799` (重复 Select 构建), `SelfOnlineStack.tsx:44-501` (手势重复)。
*   **用途**：提供按可删行数降序的发现清单，用于指导代码重构。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                   docs-and-design Module                     │
├──────────────────────────┬───────────────────────────────────┤
│       Documentation      │          Design & Research        │
├──────────────────────────┼───────────────────────────────────┤
│ • Changelogs             │ • Superpowers Plans/Specs         │
│   (changelogs/*.md)      │   (docs/superpowers/plans/)       │
│ • Release Notes          │ • Design Prototypes               │
│   (release/RELEASE_*.md) │   (design-prototypes/*.mjs/.html) │
│ • Audit Reports          │                                   │
│   (archx_*/receipt.md)   │ • Research Papers                 │
│                          │   (docs/research/*.md)            │
├──────────────────────────┴───────────────────────────────────┤
│                  CodeWiki Knowledge Base                     │
│   (modules/docs-and-design.md - This Article)                │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **源码审计输入**：通过并行读取热点文件（如 `scheduled.rs`, `NetworkPanel.tsx`），提取函数签名、行号、调用关系及死代码证据。
2.  **深化机会生成**：基于约束文档（如协议逻辑红线、双端同步纪律），判定候选项的强度（Strong/Worth exploring/Speculative）。
3.  **文档沉淀**：
    *   **结构化回执**：输出包含 Files, Problem, Solution, Benefits, 删除测试的 Markdown 表格或列表。
    *   **Mermaid 图**：生成 Before/After 流程图以可视化重构效果。
    *   **Changelog 条目**：将技术决策（如 RTSS 处理）转化为用户可见的更新日志。

## Connections

*   [[modules/architecture]]：本模块输出的深化机会直接关联架构改进任务。
*   [[modules/frontend-network]]：`NetworkPanel.tsx` 的拆分建议影响前端网络模块的结构。
*   [[modules/backend-monitor]]：`scheduled.rs` 的外移建议影响后端监控模块的职责划分。
*   [[modules/platform-rtss]]：RTSS 兼容性逻辑的文档化关联到底层平台适配模块。

## Known Issues

1.  **双端同步成本**：多个深化机会（候选 2, 4, 5, 6）涉及双端同步改动，需严格遵守“通用改进双端同步”纪律，否则易产生隐性分叉。
2.  **RTSS 手动排除限制**：自动写入 RTSS Profile 方案已证伪，用户需手动操作，增加了技术支持复杂度。
3.  **历史注释过时**：部分源码注释（如 `campus_detect.rs:215`）与实际调用链不符，需在清理死代码时同步修正。