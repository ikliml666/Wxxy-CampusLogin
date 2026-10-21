--- frontmatter
type: concept
title: monitor-loop-and-quality-history
source_files:
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/monitor/useMonitor.ts
  - android/frontend/src/monitor/LatencyTimeline.tsx
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/quality_history.rs
tags: [monitor, quality, history, state-management, rust-backend]

## Overview

「monitor-loop-and-quality-history」描述了系统在网络质量监控闭环中的核心运作机制。它涵盖了从用户交互触发（或后台自动巡检）到后端执行网络探测，再到前端状态更新、历史记录持久化及可视化展示的完整数据流。该概念将“实时质量反馈”与“历史趋势分析”解耦又关联：实时监控关注当前的 `NetworkQuality` 等级与延迟数值，而历史模块则负责通过时间序列数据呈现网络波动的长期模式。这一机制依赖于 Rust 后端的原子性存储策略和前端 Zustand Store 的防竞态处理，确保在弱网环境或频繁切换场景下的数据一致性。

## Involved Modules

*   **android-frontend-hooks-stores**: 提供 `useQualityStore`，作为前端状态中枢，管理质量检测结果的时间戳节流与 UI 状态同步。
*   **android-frontend-monitor**: 包含 `useMonitor` Hook 和 `LatencyTimeline` 组件，负责发起探测请求、处理结果回调及渲染历史曲线。
*   **android-rust-backend**: 
    *   `quality_cmds`: 暴露 `check_network_quality`, `start_latency_test` 等 Tauri 命令，执行实际的 ICMP/TCP 探测。
    *   `quality_history`: 负责将探测结果追加写入本地历史文件，支持按时间段查询。
*   **android-tauri-shell-and-plugins**: 提供底层的 `MonitorServicePlugin`，在前台服务中维持监控循环的生命周期，确保应用在后台时仍能执行周期性检测。

## Flow

1.  **触发与调度**:
    *   用户手动点击刷新或配置变更时，`useMonitor` (L45-48) 先保存配置，随后调用 `refreshQuality`。
    *   后台定时任务由 `MonitorServicePlugin` 触发，通过 Tauri IPC 唤醒后端。
2.  **后端执行**:
    *   `quality_cmds.rs` 接收命令，调用 JNI 或原生网络库进行探测。
    *   探测成功后，结果同时写入 `quality_history.rs` 维护的历史记录文件（采用临时文件重命名策略保证原子性）。
3.  **状态同步**:
    *   后端返回最新的质量对象（含延迟、丢包率、等级）。
    *   `useQualityStore` 接收到数据，利用 `lastQualityResultTime` 进行节流判断，防止高频重复渲染。
    *   若为增量推送（如 `busy` 状态），`latency.ts` 中的 `mergeNetworkQuality` 会合并数据以平滑 UI 震荡。
4.  **可视化展示**:
    *   `LatencyTimeline` 组件订阅 `useQualityStore` 的最新状态。
    *   对于历史数据，组件向后端请求特定时间窗口的历史记录，并在 Canvas/SVG 上绘制折线图，展示延迟波动趋势。
5.  **异常处理**:
    *   若探测失败，`useMonitor` 中的 `mountedRef` 确保不会在组件卸载后更新状态，避免内存泄漏。
    *   后端静默忽略非关键性网络错误，保持主流程稳定。

## Implementation Map

*   **前端状态管理**: `android/frontend/src/hooks/useQualityStore.ts` (行 54-74, 18)，定义 `refreshQuality` 逻辑与 `getLastQualityResultTime` 节流键。
*   **监控业务逻辑**: `android/frontend/src/monitor/useMonitor.ts` (行 31-37, 45-48)，实现竞态锁、配置预保存及 API 调用封装。
*   **历史数据可视化**: `android/frontend/src/monitor/LatencyTimeline.tsx` (行 42-160)，负责渲染时间轴上的质量点。
*   **后端探测入口**: `android/src-tauri/src/quality_cmds.rs` (行 52-77)，注册 `check_network_quality` 和测速相关命令。
*   **后端历史存储**: `android/src-tauri/src/quality_history.rs`，实现记录的追加、读取与清理逻辑。