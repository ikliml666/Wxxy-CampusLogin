---
title: 模式与约定总结 (Patterns)
type: pattern
source_files:
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/monitor/LatencyTimeline.tsx
  - android/frontend/src/monitor/useMonitor.ts
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/frontend/index.html
tags:
  - architecture
  - coding-standards
  - error-handling
  - performance
  - security
---

## Overview

本仓库遵循“双端独立复刻但逻辑同构”的架构哲学。Android 前端与桌面 Tauri 前端在代码库中完全分离，但通过共享的 Rust 后端 crate (`campus-login`) 和高度一致的 React 组件命名/结构保持业务逻辑的一致性。整体风格强调移动端性能优化（防泄漏、动画降级）、严格的状态管理（Zustand + 脏数据检测）以及基于 Tauri ACL 的安全门控。开发流程受 `AGENTS.md` 约束，要求 AI 辅助下的高纪律性提交与文档同步。

## Patterns

### 1. 响应式双壳与平台差异化适配 (Responsive Shell & Platform Abstraction)

**描述**：应用根据设备形态（手机 vs 平板/桌面）动态切换外壳布局。Android 端通过 `useFormFactor()` 区分 `AppInner` (Mobile) 和 `TabletShell`，桌面端则直接运行于标准窗口。UI 层通过条件渲染处理平台差异（如 Android 无原生导航栏需自定义 Dock/BottomNav），而逻辑层通过抽象层屏蔽底层 API 差异。

**证据**：
*   **Android 双壳路由**：`android/frontend/src/App.tsx` 实现 `AppInner` 与 `TabletShell` 的动态切换，支持底部 Tab 导航与侧边栏多栏布局。
*   **平台常量隔离**：`android/frontend/src/settings/useOnboardingFlow.ts` 使用 `isAndroid` 常量区分字段名（如 `enableBootAutostart` vs `autoLaunch`）。
*   **桌面端入口一致性**：`tauri-app/frontend/index.html` 作为桌面端唯一入口，结构与 Android `index.html` 保持高度同构。

### 2. 状态管理的脏数据冲突解决 (Dirty Data Conflict Resolution)

**描述**：在配置持久化场景中，采用 `dirtyFields` 标记本地修改，并结合 `dirtyFailureCounts` 失败计数器。当后端旧快照可能覆盖本地未落盘的新值时，优先保留本地最新状态；若连续同步失败，则放弃自动同步以避免永久脱节。这种模式防止了竞态条件下的数据丢失。

**证据**：
*   **脏字段追踪**：`android/frontend/src/hooks/useConfigStore.ts` 实现 `dirtyFields` 机制，仅在字段变更时参与合并。
*   **失败熔断**：同一文件中的 `mergeConfigFromBackend` 逻辑包含失败计数重置与同步放弃策略。

### 3. 串行化 IO 与代际锁保护 (Serialized IO & Generation Locks)

**描述**：后端 Rust 核心通过全局异步锁强制配置读写串行化，防止并发写入导致的数据损坏。同时，监控循环与网络测试使用 `AtomicU64` 代际标记（Generation ID）而非简单的布尔标志，确保旧任务不会意外清除新任务的运行状态，解决了高并发下的生命周期竞态问题。

**证据**：
*   **配置串行锁**：`android/src-tauri/src/config_state.rs` 定义 `CONFIG_IO_LOCK`，所有 `get_config`/`save_config` 均受其保护。
*   **代际标记**：`android/src-tauri/src/monitor_loop.rs` 使用 `MONITOR_GENERATION` 原子变量管理后台监控循环的生命周期。
*   **原子文件落盘**：`lib.rs` 及相关模块采用 "Write to `.tmp` -> Rename" 模式保证配置文件原子性。

### 4. 生物识别门控与安全隐私掩码 (Biometric Gate & Privacy Masking)

**描述**：敏感操作（如运营商凭据揭示、自助服务）强制通过双层 TTL 验证门控。第一层为全局会话级验证 (`useHelloGate`)，第二层为操作级即时验证 (`useSelfServiceVerify`)。UI 层对敏感信息默认展示掩码，且存在时钟回拨防护机制以防验证永真。

**证据**：
*   **双层门控**：`android/frontend/src/account/selfServiceState.ts` 定义 `useHelloGate` 和 `useSelfServiceVerify`。
*   **时钟回拨防护**：`selfServiceState.ts` 行 58-62 显式检查系统时间倒退，防止死锁。
*   **明文揭示强制验证**：`android/frontend/src/account/useOperatorBind.ts` 即使总开关关闭，揭示密码也需二次生物验证。
*   **后端掩码出口**：`config_state.rs` 提供 `masked_for_display` 统一处理敏感字段序列化。

### 5. 移动端高性能动画与手势隔离 (High-Performance Animation & Gesture Isolation)

**描述**：针对 WebView 环境限制，采用直接 DOM Style 操作替代 React State 驱动高频交互（如 Dock 镜头效果），结合 `ResizeObserver` 和 `rAF` 循环维持 60fps+。通过 CSS `touch-action: none` 隔离滚动冲突，并实施 GSAP 实例清理与超时 Failsafe 机制，防止后台挂起导致的内存泄漏或卡死。

**证据**：
*   **DOM 直操优化**：`android/frontend/src/lib/dock-lens.ts` 绕过 React 渲染瓶颈，直接操作 DOM 样式实现果冻形变。
*   **手势隔离**：`android/frontend/src/account/SelfOnlineStack.tsx` 设置 `touch-action: none` 配合 `ResizeObserver` 实现堆叠视图。
*   **GSAP 超时保护**：`android/frontend/src/components/layout/RightPanel.tsx` 实施 GSAP 超时重置与 `kill()` 清理，应对 WebView 后台休眠。

### 6. 错误处理的静默降级与非阻断式请求 (Silent Degradation & Non-blocking Requests)

**描述**：前端在网络波动或权限缺失时采取“静默降级”策略。关键路径失败不中断主流程，非关键路径（如 WiFi SSID 获取、Dashboard 自动刷新）失败仅记录日志或忽略，避免弹出干扰性 Toast。弹窗组件吞没提交错误以保持 UI 上下文连续性，允许用户重试。

**证据**：
*   **静默刷新失败**：`android/frontend/src/account/useAccount.ts` 中 `fetchDashboard` 自动刷新失败不报 Toast。
*   **非阻断权限**：`android/frontend/src/monitor/useMonitor.ts` 将 WiFi SSID 权限请求包裹在 try-catch 中，失败仅降级显示。
*   **错误吞没重试**：`android/frontend/src/account/DeviceAliasDialog.tsx` 捕获提交错误并保持弹窗打开，由父组件统一提示。

### 7. 首帧主题一致性与防闪烁保障 (First-Frame Theme Consistency)

**描述**：为防止 React 挂载前的样式跳变（FOUC），在 HTML 入口文件中通过内联同步 IIFE 脚本，在 CSS 解析前立即写入 `data-light`、`theme` 和 `glass-level` 属性。此逻辑在 Android 和桌面端 `index.html` 中严格保持一致，确保首屏视觉瞬间稳定。

**证据**：
*   **Android 首帧脚本**：`android/frontend/index.html` 行 9-26 执行同步 IIFE 设置 CSS 变量。
*   **桌面端首帧脚本**：`tauri-app/frontend/index.html` 行 9-26 执行相同逻辑，确保跨端视觉一致。
*   **CSS 变量驱动**：`android/frontend/src/index.css` 依赖上述属性切换 `--glass-blur` 等变量。

### 8. 数据归一化与多源融合 (Data Normalization & Multi-source Fusion)

**描述**：面对异构数据源（如 Dashboard 历史数组与账单对象数组），组件层采用“多源转一态”策略，将其统一转换为标准化的内部结构（如 `SessionRow`），从而简化视图层渲染逻辑，降低维护成本。

**证据**：
*   **记录融合**：`tauri-app/frontend/src/account/SelfOnlineRecords.tsx` 将 `SelfHistoryRow` 和 `SelfLogRow` 统一映射为标准化的 `SessionRow`。
*   **延迟归一化**：`android/frontend/src/monitor/LatencyTimeline.tsx` 实现严格的宽度归一化算法，确保百分比总和不超过 100%。

### 9. Tauri ACL 与命令通道解耦 (ACL Decoupling from Command Channels)

**描述**：自研插件定义了独立的权限集，但在实际调用中，前端通过应用命令通道 (`protocol_cmds.rs`) 而非插件 invoke 进行通信。这导致能力文件 (`capabilities/default.json`) 中未显式授权这些插件命令，而是依赖通配符或底层 IPC 机制。这种模式降低了耦合，但增加了 ACL 审查的认知负担，需警惕 `deny-<command>` 优先原则带来的潜在拦截风险。

**证据**：
*   **命令通道复用**：`android/src-tauri/src/protocol_cmds.rs` 集中处理登录、监控等核心业务命令。
*   **ACL 隐式耦合**：`desktop-tauri-shell-config` 模块指出 `capabilities/default.json` 硬编码匹配 `"main"` 窗口，且自研插件权限未在 capability 中授予，依赖运行时 ACL 的宽泛配置或通配符。
*   **生成物不可手动修改**：`tauri-gen-schemas` 模块强调 `gen/schemas/capabilities.json` 为自动生成，权限变更需修正源码或声明文件。