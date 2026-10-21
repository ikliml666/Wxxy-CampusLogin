---
title: Repository Patterns and Conventions
type: pattern
source_files:
  - android/frontend/vite.config.ts
  - android/frontend/vite.mock.config.ts
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/src/index.css
  - android/frontend/tauri-mock.js
  - android/frontend/src/auth/index.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/useAccount.ts
  - android/frontend/src/settings/constants.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/lib/dock-lens.ts
  - android/frontend/src/lib/dock-spring.ts
  - android/frontend/src/lib/glass-dock.tsx
  - android/frontend/src/lib/utils.ts
  - android/frontend/src/monitor/useMonitor.ts
  - android/frontend/src/monitor/LatencyComponents.tsx
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/gen/android/app/build.gradle.kts
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
  - make-release.ps1
  - AGENTS.md
tags:
  - patterns
  - conventions
  - architecture
  - android
  - tauri
---

## Overview

本仓库采用 **Tauri + React (Vite)** 架构，支持 Windows (桌面) 与 Android 双端。代码库呈现出高度一致的模块职责划分、状态管理范式及跨平台适配策略。以下总结了在构建流程、目录结构、错误处理、状态管理及原生交互方面反复出现的模式。

## Patterns

### 1. 构建与发布自动化 (Build & Release Automation)

**描述**: 项目通过 PowerShell 脚本实现标准化的构建、清理与发布流程，强制要求产物校验（SHA256）和缓存清理。Android 端构建环境存在隔离性，需通过特定 Gradle 任务或脚本触发交叉编译。

*   **证据**:
    *   `make-release.ps1`: 作为主构建入口，执行 Tauri 前端构建、产物定位及 SHA256 校验 (`target-release-build.log`)。
    *   `clean-build-cache.ps1`: 负责清理 `target/`, `dist/` 及 WebView2 缓存，保留 Local Storage 以维持登录状态。
    *   `android/src-tauri/gen/android/app/build.gradle.kts`: Gradle 构建配置，包含 Rust 插件调用逻辑。
    *   `AGENTS.md`: 定义了开发者/AI Agent 的 Commit 前缀及 Changelog 同步纪律。

### 2. 目录结构与模块导出约定 (Directory Structure & Index Exports)

**描述**: 遵循“按功能域拆分组件/Hook”的目录结构，每个功能模块（如 `account`, `settings`, `monitor`）均提供统一的 `index.ts` 入口文件，集中导出公共接口。这降低了上游耦合，便于 Tree-shaking。

*   **证据**:
    *   `android/frontend/src/auth/index.ts`: 统一导出 `DashboardPanel`, `AboutDialog`, `useAuth` 等。
    *   `android/frontend/src/account/index.ts`: 统一导出 `AccountPanel`, `ChangePasswordDialog`, `useAccount`。
    *   `android/frontend/src/settings/index.ts`: 统一导出 `SettingsPanel`, `ThemeDialog`, `OnboardingWizard`。
    *   `android/frontend/src/monitor/index.ts`: 导出监控相关组件与 Hook。

### 3. 原子性数据持久化模式 (Atomic Data Persistence)

**描述**: Rust 后端在处理配置和历史记录时，严格采用“写入临时文件 + rename”的策略。这是防止进程崩溃导致 JSON 文件损坏的核心机制，确保了数据的一致性。

*   **证据**:
    *   `android/src-tauri/src/config_state.rs`: 配置文件保存逻辑中，先写入 `.tmp` 文件，成功后再重命名为正式文件名。
    *   `android/src-tauri/src/login_history.rs` / `quality_history.rs`: 历史记录追加同样遵循此原子写入模式。

### 4. 防御性竞态与异步安全处理 (Defensive Concurrency & Async Safety)

**描述**: 前端 Hooks 广泛使用双重锁机制（挂载检查 + 业务状态锁）来防止组件卸载后的无效 setState 或并发 API 调用。后端则通过 Mutex 保护共享 IO 资源。

*   **证据**:
    *   `android/frontend/src/monitor/useMonitor.ts`: 使用 `mountedRef` 和 `busyRef` 双重锁，防止组件卸载后执行 setState。
    *   `android/frontend/src/account/useAccount.ts`: 使用 `switchInFlight` Promise 机制合并重复请求；Dialog 保存期间禁用交互防竞态。
    *   `android/src-tauri/src/config_state.rs`: 使用全局 `tokio::sync::Mutex<()>` (`CONFIG_IO_LOCK`) 保护配置读写。
    *   `android/frontend/src/hooks/useAuthStore.ts`: 使用串行锁 `_checkOnlineLockFlag` 保证并发安全。

### 5. 零 Re-render 高性能动画模式 (Zero Re-render Animation)

**描述**: 针对复杂的视觉特效（如悬浮栏镜头效果），直接操作 DOM Style 和 CSS 变量，结合自定义物理弹簧内核，避免触发 React 渲染周期，显著降低主线程负担。

*   **证据**:
    *   `android/frontend/src/lib/dock-lens.ts`: 核心 Hook，实现镜头效果、果冻形变及零 React re-render 动画。
    *   `android/frontend/src/lib/dock-spring.ts`: 定义 `Motion` 类，提供标量物理弹簧仿真内核。
    *   `android/frontend/src/lib/glass-dock.tsx`: 封装 Hyalite 库，解决 Chromium 合成层黑屏问题。

### 6. 生物识别安全门控与 TTL 机制 (Biometric Security Gate & TTL)

**描述**: 敏感操作（如踢下线、查状态、解密密码）前强制进行身份验证。系统采用 TTL (Time-To-Live) 机制控制验证频率，并包含时钟回拨防护，防止因系统时间异常导致的安全绕过或死锁。

*   **证据**:
    *   `android/frontend/src/account/selfServiceState.ts`: 实现 TTL 机制，包含 `elapsed >= 0` 时钟回拨防护和 `ignoreToggle` 强制验证逻辑。
    *   `android/src-tauri/src/identity_gate.rs`: Rust 后端实现生物识别 TTL 校验（10分钟有效期）。
    *   `android/frontend/src/settings/SettingsPanel.tsx`: 敏感操作前强制调用 `ensureBindHello`。

### 7. 配置脏标记与回退策略 (Dirty Flag & Fallback Strategy)

**描述**: 配置 Store 引入 `dirtyFields` 和重试计数器。当保存失败时，累计重试次数，达到阈值后放弃脏标记以防止 UI 与后端永久脱节，并在应用关闭前兜底保存。

*   **证据**:
    *   `android/frontend/src/hooks/useConfigStore.ts`: `updateConfig` 和 `saveConfigDirect` 实现脏字段追踪；`flushPendingConfig` 用于关闭前的兜底保存。

### 8. Chromium Backdrop Filter 黑屏规避 (Chromium Backdrop Filter Black Screen Mitigation)

**描述**: 在 Chromium 内核下，`backdrop-filter` 在合成层构建期间可能导致黑屏。解决方案是主动检测祖先链变换状态，延迟挂载特效组件，并设置超时降级机制。

*   **证据**:
    *   `android/frontend/src/lib/glass-dock.tsx`: 主动检测 `backdropReady` 状态，延迟挂载 Hyalite，设计超时降级避免常驻黑膜。

### 9. 平台差异化硬编码与适配 (Platform-Specific Hardcoding)

**描述**: 为了快速适配 Android 特有需求，代码中存在多处基于 `isAndroid` 常量的渲染分支硬编码，以及针对 Android 后端字段名差异的特殊处理。这增加了多平台维护成本。

*   **证据**:
    *   `android/frontend/src/settings/SettingsPanel.tsx`: 使用 `isAndroid` 常量控制 `KeepAliveSettingsCard` 渲染。
    *   `android/frontend/src/hooks/useSettings.ts`: 针对安卓后端字段名差异（`enableBootAutostart` vs `autoLaunch`）做特殊处理。
    *   `android/frontend/src/hooks/tauriApi.ts`: `desktopOnly` 函数拒绝桌面专属功能（如 DHCP）。

### 10. Mock 数据一致性风险 (Mock Data Consistency Risk)

**描述**: 开发环境的 Mock 体系旨在模拟 Tauri 后端行为，但部分 Mock 函数未使用标准包装器，返回平铺结构体，若前端直读逻辑不匹配会导致属性为 `undefined`。

*   **证据**:
    *   `android/frontend/tauri-mock.js`: `check_campus_status` 和 `check_portal_status` 未使用标准 `ok()` 包装，返回平铺结构体。

### 11. WebView 缓存清理边界 (WebView Cache Cleanup Boundary)

**描述**: 构建清理脚本刻意保留 `Local Storage` 和 `Session Storage` 以维持登录状态，但清除其他缓存可能影响复杂页面状态。这是一个需要权衡的性能优化点。

*   **证据**:
    *   `clean-build-cache.ps1`: 清理脚本逻辑中明确区分了需保留的存储区域和需清除的缓存区域。

### 12. 网络质量增量合并策略 (Network Quality Incremental Merge)

**描述**: 针对后台推送的网络质量数据，采取保留旧 quality 等级的合并逻辑，防止在慢网络或 `busy` 状态下 UI 状态频繁震荡。

*   **证据**:
    *   `android/frontend/src/lib/latency.ts`: `mergeNetworkQuality` 函数处理增量推送，保留旧等级防止 UI 震荡。

### 13. 权限配置陷阱 (Permission Configuration Trap)

**描述**: Tauri 的 `capabilities.json` 仅绑定默认权限集，若未显式添加自定义插件权限，前端调用将直接被拦截。这是 Android 端特有的高风险配置项。

*   **证据**:
    *   `android/src-tauri/gen/android/app/build.gradle.kts` / `capabilities.json`: 指出 `campus-keystore`, `campus-monitor-service` 等插件权限未在默认能力集中，需手动配置。

### 14. 输入草稿模式 (Input Draft Mode)

**描述**: 关键数值输入（如检测间隔）采用本地暂存 + Blur/Enter 提交机制，并配合 Clamp 限制，防止无效值写入 Store 或引起 UI 闪烁。

*   **证据**:
    *   `android/frontend/src/monitor/LatencyComponents.tsx`: 关键数值输入采用本地暂存 + Blur/Enter 提交机制。