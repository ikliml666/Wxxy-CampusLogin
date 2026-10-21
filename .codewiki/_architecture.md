---
title: System Architecture Overview
type: architecture
source_files: []
tags: [architecture, system-overview, tauri, android, frontend, backend]
---

## Overview

本系统是一个基于 **Tauri 2** 构建的跨平台校园网登录与网络管理应用，主要运行于 Android 移动端（APK）及 Windows 桌面端。系统采用经典的“前端 UI + 后端 Rust”架构，通过 IPC (Inter-Process Communication) 进行通信。

核心设计原则包括：
1.  **双端同构与差异适配**：Android 端与桌面端共享协议核心 (`campus_login_lib`)，但 UI 层完全独立复刻，针对各自平台的交互范式（Bottom Nav vs Dock/Tab）和硬件能力（生物识别、后台服务）进行差异化实现。
2.  **安全优先**：敏感操作（如运营商凭据揭示、自助服务）强制集成生物识别门控（Windows Hello / Android Biometric），配置数据采用原子文件落盘与加密存储。
3.  **高性能与稳定性**：UI 层广泛使用 Zustand 细粒度状态管理、CSS 变量驱动的主题系统以及针对 WebView 环境优化的动画策略；后端采用串行化 IO 和代际标记防止竞态条件。

## System Overview

系统由三大层级组成：**前端展示层**、**后端逻辑层** 和 **基础设施层**。

```text
┌──────────────────────────────────────────────────────────────────────┐
│                       Frontend Layer (React/Vite)                    │
├──────────────────────┬───────────────────────────────────────────────┤
│   Entry & Shell      │   Feature Modules                             │
│                      │                                               │
│ • Entry Config       │ • Auth (Dashboard, About)                     │
│   (index.html,       │ • Account (CRUD, Bind Operator)               │
│    main.tsx)         │ • Monitor (Quality, Latency, Status Bar)      │
│                      │ • Settings (Onboarding, Theme, KeepAlive)     │
│                      │                                               │
│ • App Shell          │ ───────────────────────────────────────────── │
│   (App.tsx,          │   Shared Infrastructure                       │
│    Tablet/Mobile     │ • Hooks & Stores (Zustand, Tauri API Wrap)    │
│    Layouts)          │ • Lib & i18n (Animations, Utils, Translations)│
└──────────┼───────────┴───────────────────────────────────────────────┘
           │ IPC Invoke / Tauri Commands
           ▼
┌──────────────────────────────────────────────────────────────────────┐
│                       Backend Layer (Rust/Tauri)                     │
├──────────────────────┬───────────────────────────────────────────────┤
│   Core Logic         │   Protocol & Network                          │
│                      │                                               │
│ • Rust Core          │ • Campus Detect (Probe Logic)                 │
│   (lib.rs)           │ • Login History                               │
│   - Config State     │ • Quality History                             │
│   - Account Cmds     │ • Campus Login Lib (Shared Crate)             │
│   - Protocol Cmds    │   (Auth, Portal, Night Switch)                │
│   - Monitor Loop     │                                               │
│   - Quality Cmds     │ ───────────────────────────────────────────── │
│   - Update Cmds      │   Platform Specific                           │
│                      │ • Android Plugins (Keystore, Foreground Svc)  │
│ • Desktop Shell      │ • Desktop APIs (IP Helper, Windows Hello)     │
│   (build.rs,         │                                               │
│    tauri.conf.json)  │ ───────────────────────────────────────────── │
│                      │   Security & Schema                           │
│                      │ • Tauri Gen Schemas (ACL, Capabilities)       │
└──────────────────────┴───────────────────────────────────────────────┘
```

## Data Flow

数据流向遵循单向依赖模式：前端发起请求 -> 后端处理业务/系统调用 -> 返回结果 -> 前端更新 Store -> UI 重渲染。

```text
[User Action]
      │
      ▼
┌──────────────────────────────────────────────────────────────────────┐
│                        Frontend (React)                              │
│                                                                      │
│  ┌─────────────┐     ┌──────────────────┐     ┌──────────────────┐  │
│  │ UI Component │────▶│  Hook/Store      │────▶│  Tauri API Wrap  │  │
│  │ (e.g.,       │     │ (useAccount,     │     │ (tauriApiWith    │  │
│  │  AccountPanel)│     │   useMonitor)    │     │   Retry)         │  │
│  └─────────────┘     └──────────────────┘     └────────┬─────────┘  │
│                                                       │              │
│  ┌──────────────────┐                                │              │
│  │  Zustand Store   │◀───────────────────────────────┘              │
│  │ (Auth, Config,   │  (Update State on Response)                   │
│  │  Quality)        │                                               │
│  └──────────────────┘                                               │
└──────────────────────────────────────────────────────────────────────┘
      │ IPC Invoke (JSON Payload)
      ▼
┌──────────────────────────────────────────────────────────────────────┐
│                        Backend (Rust)                                │
│                                                                      │
│  ┌──────────────────┐     ┌──────────────────┐     ┌──────────────┐ │
│  │ Tauri Command    │────▶│  Business Logic  │────▶│  System/API  │ │
│  │ Handler          │     │  (Config State,  │     │  Calls       │ │
│  │ (account_cmds.rs)│     │   Monitor Loop)  │     │ (Network,    │ │
│  └──────────────────┘     └──────────────────┘     │  File IO,    │ │
│                                                    │  Biometrics) │ │
│  ┌──────────────────┐                             └──────────────┘ │
│  │  ACL/Permission  │◀─────────────────────────────────────────────┘
│  │  Check (Gen S.)  │
│  └──────────────────┘
└──────────────────────────────────────────────────────────────────────┘
```

## Dependency Graph

模块间的依赖方向严格分层，禁止循环依赖。

```text
┌──────────────────────────────────────────────────────────────────────┐
│                         Dependency Direction                         │
│                                                                      │
│  (High Level)                                                        │
│  ◄── Workspace Meta & Docs (Rules, Specs)                           │
│  ◄── Release & Changelog Archive (Audit Trail)                      │
│                                                                      │
│  (Backend Core)                                                      │
│  ◄── Desktop Rust Core (Commands, State)                            │
│  ◄── Campus Login Lib (Protocol, Network)                           │
│  ◄── Tauri Gen Schemas (ACL, Permissions)                           │
│  ◄── Desktop Tauri Shell Config (Build, Assets)                     │
│                                                                      │
│  (Frontend Infra)                                                    │
│  ◄── Android Frontend Hooks & Stores (Zustand, API Wrap)            │
│  ◄── Android Frontend Lib & I18n (Utils, Animations)                │
│  ◄── Android Frontend Entry Config (HTML, CSS, Vite)                │
│                                                                      │
│  (Frontend Features)                                                 │
│  ◄── Android Frontend App Shell (Layouts, Navigation)               │
│  ◄── Android Frontend Auth (Dashboard, About)                       │
│  ◄── Android Frontend Account (CRUD, Self Service)                  │
│  ◄── Android Frontend Monitor (Quality, Latency)                    │
│  ◄── Android Frontend Settings (Onboarding, Theme)                  │
│                                                                      │
│  (Desktop Specific)                                                  │
│  ◄── Desktop Frontend Account (Similar to Android Account)          │
│  ◄── Desktop Frontend Entry Config                                  │
│                                                                      │
│  ▲                                                                   │
│  │ Dependencies Point Upwards                                         │
│  │ Consumers Depend on Providers                                      │
└──────────────────────────────────────────────────────────────────────┘
```

## Module Roles

### 1. 基础设施与配置层 (Infrastructure & Config)

*   **`workspace-meta-and-docs`**: 定义项目宪法、AI 助手纪律、构建规范及核心设计文档。不直接参与运行时，但约束所有其他模块的开发行为。
*   **`android-frontend-entry-config`**: Android 前端的工程入口。负责 `index.html` 首帧主题初始化、Vite 构建配置及全局样式。确保启动视觉一致性。
*   **`desktop-frontend-entry-config`**: 桌面端前端的工程入口。类似 Android 端，负责 HTML 骨架屏、首帧脚本及构建流程。
*   **`desktop-tauri-shell-config`**: Android 端 Tauri 应用的构建壳。负责 `Cargo.toml`、`tauri.conf.json`、ACL 权限声明及构建钩子 (`build.rs`)。将前端产物接入 Tauri 运行时。
*   **`tauri-gen-schemas`**: Tauri 2 ACL/能力生成物快照。提供运行时权限校验依据及开发时 JSON Schema 支持。不可手动修改，由构建流程自动生成。
*   **`release-and-changelog-archive`**: 纯文档归档层。记录版本变更、审计回执及发布资产校验和，服务于追溯与合规。

### 2. 后端核心层 (Backend Core)

*   **`desktop-rust-core`**: Tauri 后端的命令面与状态面骨架。
    *   **职责**: 注册所有 Tauri 命令 (Config, Account, Protocol, Monitor, Quality, Update)，管理配置锁，协调后台监控循环。
    *   **关键模式**: 使用 `AtomicU64` 代际标记防止竞态，串行化配置 IO，原子文件落盘。
    *   **依赖**: `campus_login_lib` (协议核心), `campus-detect` (探针), `login-history`, `quality-history`.

### 3. 前端基础设施层 (Frontend Infrastructure)

*   **`android-frontend-hooks-and-stores`**: Android 前端的核心状态管理与 API 适配层。
    *   **职责**: 基于 Zustand 管理 `Auth`, `Config`, `Quality`, `Theme` 等全局状态；封装 `tauriApi` 处理 IPC 调用与重试；编排应用启动流程 (`useAppInit`)。
    *   **关键模式**: 脏数据冲突解决 (`dirtyFields`)，Android 特定降级策略 (`desktopOnly` 拦截)，动画性能自适应。
*   **`android-frontend-lib-and-i18n`**: 底层工具库与国际化。
    *   **职责**: 提供 `i18next` 国际化支持，封装玻璃拟态/Dock 栏交互效果 (`glass-dock`, `dock-lens`)，提供颜色转换、延迟解析等通用工具。
    *   **关键模式**: 直接 DOM Style 操作保证 60fps+ 性能，容错降级约定。

### 4. 前端功能模块层 (Feature Modules)

*   **`android-frontend-app-shell`**: Android 前端的应用外壳与布局系统。
    *   **职责**: 响应式双壳架构 (Mobile/Tablet)，全局状态聚合，导航组件 (BottomNav, DockNav)，日志面板。
    *   **关键模式**: 基于 `useFormFactor` 的路由区分，高性能分片渲染，玻璃拟态视觉约定。
*   **`android-frontend-auth`**: 认证交互与仪表盘。
    *   **职责**: Dashboard 展示，About 对话框，认证状态 Hook (`useAuth`)。
    *   **关键模式**: 自定义拖拽实现 (解决 WebView pointer capture 限制)，Extra 卡生命周期管理 (黑名单机制)。
*   **`android-frontend-account`**: 账号面板与自助服务。
    *   **职责**: 账号 CRUD，运营商绑定，设备在线管理，生物识别验证门控。
    *   **关键模式**: 双层 TTL 生物识别门控 (`useHelloGate`), 并发去重 (`switchInFlight`), 错误静默降级。
*   **`android-frontend-monitor`**: 网络监控核心。
    *   **职责**: 从底层状态感知到可视化展示 (StatusBar, Capsule, Panel) 的完整闭环。
    *   **关键模式**: 竞态与生命周期保护 (`mountedRef` + `busyRef`), 配置持久化优先策略，数据归一化算法。
*   **`android-frontend-settings`**: 设置面板与新手引导。
    *   **职责**: 设置面板，Onboarding Wizard，常亮保活卡片，主题管理。
    *   **关键模式**: 平台差异化抽象 (`isAndroid` 条件渲染)，状态机与动画解耦，密码掩码态处理。

### 5. 桌面端特有模块 (Desktop Specific)

*   **`desktop-frontend-account`**: 桌面端账号管理。
    *   **职责**: 与 Android 端 `android-frontend-account` 功能类似，但集成 Windows Hello 验证。
    *   **关键模式**: 数据归一化 (多源转一态), 安全门控机制, StrictMode 兼容性。