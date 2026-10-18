---
title: System Architecture Overview
type: architecture
source_files:
  - android/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/account/BindOperatorDialog.tsx
  - android/frontend/src/account/ChangePasswordDialog.tsx
  - android/frontend/src/account/DeviceAliasDialog.tsx
  - android/frontend/src/account/index.ts
  - android/frontend/src/account/LoginInfoDialog.tsx
  - android/frontend/src/account/operatorChip.ts
  - android/frontend/src/account/SelfOnlineRecords.tsx
  - android/frontend/src/account/selfServiceTypes.ts
  - android/frontend/.env
  - android/frontend/pnpm-workspace.yaml
  - modules/design-prototypes/online-history-redesign.html
tags: [architecture, system-overview, frontend, backend, tauri, rust, react]
---

## Overview

Wxxy-CampusLogin 是一个基于 Tauri 构建的跨平台校园网管理应用，核心聚焦于 Android 端，同时支持桌面端。系统采用前后端分离架构：前端基于 React + TypeScript + Tailwind CSS，通过 Zustand 进行状态管理；后端基于 Rust (Tauri Commands)，负责原生能力调用、网络协议处理及数据持久化。

本系统总览旨在为未来 Agent 提供全局视角，阐明模块间的职责边界、数据流向及依赖关系。

## System Overview

系统由四个主要层级构成：构建与Mock层、前端UI层、状态管理层、以及Rust后端核心层。

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Wxxy-CampusLogin System                            │
├─────────────────────────────────────────────────────────────────────────────┤
│  [Build & Mock Layer]                                                       │
│  ┌──────────────────┐   ┌──────────────────┐   ┌────────────────────────┐  │
│  │ .env / package   │   │ mock-tauri.js    │   │ tauri-mock.js          │  │
│  │ pnpm-lock        │   │ (Basic Shim)     │   │ (Production Mock)      │  │
│  └──────────────────┘   └──────────────────┘   └────────────────────────┘  │
│           │                      │                       │                 │
│           ▼                      ▼                       ▼                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  [Frontend UI Layer (React/Tailwind)]                                       │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐  │
│  │ Account Panel│ │ Monitor Panel│ │ Quality Panel│ │ Self Service     │  │
│  │ (Lifecycle)  │ │ (Latency)    │ │ (Speed/QoS)  │ │ (Biometric)      │  │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────────┘  │
│           │                      │                       │                 │
│           ▼                      ▼                       ▼                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  [State Management & Hooks (Zustand)]                                       │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐  │
│  │ useAuthStore │ │ useConfigSto │ │ useQualitySt │ │ useLogToastStore │  │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────────┘  │
│           │                      │                       │                 │
│           ▼                      ▼                       ▼                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  [Backend Core (Rust/Tauri Commands)]                                       │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐  │
│  │ config_state │ │ monitor_loop │ │ protocol_cmds│ │ self_service_cmds│  │
│  │ (Persistence)│ │ (Background) │ │ (Auth/Net)   │ │ (Gate/Bio)       │  │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────────┘  │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                     │
│  │ account_cmds │ │ quality_cmds │ │ update_cmds  │                     │
│  │ (Accounts)   │ │ (History)    │ │ (AutoUpdate) │                     │
│  └──────────────┘ └──────────────┘ └──────────────┘                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

数据流遵循单向数据流原则：用户交互触发前端 Action，通过 `tauriApi` 桥接调用 Rust 命令，Rust 处理业务逻辑后返回结果，更新 Store，进而驱动 UI 重渲染。

```text
┌──────────────┐      ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│   User Input │      │   Frontend   │      │   Tauri API  │      │   Rust Cmds  │
│   (Click/Nav)│      │   (React)    │      │   Bridge     │      │   (Rust)     │
└──────┬───────┘      └──────┬───────┘      └──────┬───────┘      └──────┬───────┘
       │                    │                      │                      │
       │ 1. Dispatch Action │                      │                      │
       ├───────────────────►│                      │                      │
       │                    │ 2. Invoke Command    │                      │
       │                    ├─────────────────────►│                      │
       │                    │                      │ 3. Execute Logic     │
       │                    │                      ├─────────────────────►│
       │                    │                      │                      │
       │                    │                      │ 4. Return Result     │
       │                    │                      ◄──────────────────────┤
       │                    │ 5. Update Store      │                      │
       │                    ◄─────────────────────┤                      │
       │                    │                      │                      │
       │ 6. Re-render UI    │                      │                      │
       ◄───────────────────┤                      │                      │
       │                    │                      │                      │
       │ 7. Visual Feedback │                      │                      │
       └────────────────────┴──────────────────────┴──────────────────────┘
```

关键路径示例：
1. **账号切换**: `AccountPanel` -> `useAccount.switch_account` -> `tauriApiWithRetry(switch_account)` -> `account_cmds.rs` -> 更新 `useAuthStore` -> UI 刷新。
2. **后台监控**: `MonitorPanel` toggle -> `useMonitor.handleToggleBackgroundCheck` -> `start_background_check` -> `monitor_loop.rs` (异步循环) -> 推送状态至 `useAuthStore`/`useQualityStore`。
3. **生物识别门禁**: `SelfServicePanel` 请求明文密码 -> `useHelloGate` -> `verify_biometric_identity` -> `identity_gate.rs` (TTL校验) -> 返回凭证。

## Dependency Graph

依赖方向清晰，Rust 后端作为信任根，前端无状态或弱状态，中间层通过 Store 解耦。

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Dependency Direction                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [Frontend Components]                                                      │
│  AccountPanel, MonitorPanel, QualityPanel, SelfServicePanel                 │
│       │                                                                     │
│       ▼ (Consumes)                                                          │
│  [Hooks & Stores]                                                           │
│  useAccount, useMonitor, useAuthStore, useConfigStore, useQualityStore      │
│       │                                                                     │
│       ▼ (Invokes via Tauri Bridge)                                          │
│  [Tauri API Layer]                                                          │
│  tauriApi, tauriApiWithRetry, createEventListener                           │
│       │                                                                     │
│       ▼ (IPC Calls)                                                         │
│  [Rust Backend Commands]                                                    │
│  account_cmds, protocol_cmds, monitor_loop, quality_cmds,                   │
│  self_service_cmds, config_state, identity_gate                             │
│       │                                                                     │
│       ▼ (Shared State & Utils)                                              │
│  config_state (JSON Persistence), login_history, infra::logger              │
│                                                                             │
│  [External Dependencies]                                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────────────────┐  │
│  │ React/GSAP  │  │ Tailwind    │  │ Zustand     │  │ Tauri Native APIs│  │
│  └─────────────┘  └─────────────┘  └─────────────┘  └──────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Module Roles

### 1. Frontend Account (`frontend-account`)
**职责**: 账户全生命周期管理（创建/切换/删除）、运营商绑定、密码管理及 Windows Hello 生物识别门控。
**关键点**: 
- 实现带有 TTL 的生物识别缓存验证，防止死锁。
- 严格处理“空密码”语义（保留旧密码 vs 清除），需显式标志位。
- 使用 Promise 锁防止账号切换竞态。

### 2. Frontend Build Assets (`frontend-build-assets`)
**职责**: 构建环境配置、依赖锁定及开发阶段的原生 API Mock 模拟。
**关键点**:
- 分层 Mock 架构：基础 Shim + 场景增强 + 生产级 Mock。
- 通过 URL 参数注入测试场景（如夜切态）。
- 严格锁定依赖版本确保 CI/CD 一致性。

### 3. Frontend Monitoring (`frontend-monitoring`)
**职责**: 网络质量后台自动检测、延迟测试、实时可视化及第三方测速入口。
**关键点**:
- 配置持久化优先模式：先存配置再调 API，防竞态。
- 草稿输入防抖机制，解决高频按键光标丢失。
- 降级可用设计：权限获取失败不阻断流程。

### 4. Rust Backend Commands (`rust-backend-commands`)
**职责**: Tauri Android 后端核心业务实现，暴露 `#[tauri::command]` 接口。
**关键点**:
- 原子性写入：写临时文件 + rename，防数据损坏。
- 代际 Token (Generation) 防护后台循环状态污染。
- Android 适配：双网路由漂移 WiFi 强制绑定、小核调度降功耗。
- 安全约束：密码加密存储、出站脱敏、SHA256 更新校验。

### 5. Docs Changelogs (`docs-changelogs`)
**职责**: 版本变更、架构审计修复记录、夜间出站切换设计及账号体系增强技术档案。
**关键点**:
- 双端同构但未抽象，存在代码重复。
- 巨型函数风险：`apply_outbound_switch` 过长，需栈溢出预防。
- 清理死代码与假接口，消除混淆。

### 6. Frontend Hooks Store (`frontend-hooks-store`)
**职责**: 核心状态管理 (Zustand) 及基础设施层，桥接 UI 与后端。
**关键点**:
- 安卓端功能降级：桌面专属功能显式拒绝而非静默失败。
- 配置防抖与脏字段保护：500ms 防抖保存，连续失败放弃脏标记。
- 自适应帧率：动态调整 GSAP ticker 帧率以优化功耗。
- StrictMode 双重挂载处理：防止异步操作被丢弃。

### 7. Design Prototypes (`design-prototypes`)
**职责**: 前端 UI/UX 设计探索资产库及 E2E 测试脚本。
**关键点**:
- React 受控组件注入模式：绕过 React 状态管理进行 Headless 测试。
- Headless 渲染时序风险：需固定长延时避免虚拟时间预算导致渲染跳过。
- 零值与在线会话特殊处理：固化边界条件显示逻辑。

### 8. Frontend Core Lib (`frontend-core-lib`)
**职责**: 应用启动入口、双端自适应布局外壳、国际化、主题样式、动画引擎及网络质量解析。
**关键点**:
- 按需探测的渲染存活检测：平衡低功耗与崩溃检测。
- 网络质量状态合并策略：处理 `busy` 状态，防止 UI 停滞。
- 存储容错机制：`localStorage` 异常时回退到内存 Map。
- Windows 杀软兼容性：缓存目录移至 `os.tmpdir()` 规避 Defender 拦截。