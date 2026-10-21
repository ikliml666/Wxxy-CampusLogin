---
title: Android Frontend Architecture
type: architecture
source_files:
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/src/index.css
  - android/frontend/src/i18n/index.ts
tags: [architecture, overview, tauri, android, react, glassmorphism]
---

## Overview

本系统是一个基于 **Tauri + Android** 的跨平台校园网登录与监控客户端。架构采用经典的“前端 UI 层 - Rust 后端逻辑层 - 原生插件层”三层设计，旨在通过 Rust 提供高性能、高安全性的本地操作能力，同时利用 React/Vite 实现现代化的响应式用户界面。

核心设计理念包括：
1.  **安全隔离**：敏感数据（密码、Token）在内存中加密处理，落盘前加密，通过生物识别门控机制限制访问。
2.  **原子性持久化**：所有配置和历史记录采用“写临时文件 + rename”策略，防止进程崩溃导致的数据损坏。
3.  **零重渲染动画**：核心视觉特效（玻璃拟态、镜头效果）直接操作 DOM/CSS 变量，绕过 React 虚拟 DOM 开销。
4.  **防御性竞态处理**：状态管理使用串行锁和 TTL 时间戳机制，确保并发操作下的数据一致性。

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Client Application Layer                     │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐   ┌──────────────────┐   ┌────────────────┐  │
│  │  App Shell       │   │  Feature Modules │   │  Lib & Utils   │  │
│  │                  │   │                  │   │                │  │
│  │  • main.tsx      │   │  • Account       │   │  • i18n        │  │
│  │  • App.tsx       │   │    Panel/Hooks   │   │  • Glass/Lens  │  │
│  │  • Routing       │   │  • Settings      │   │  • Latency     │  │
│  │                  │   │    Wizard        │   │  • SafeStorage │  │
│  │                  │   │  • Monitor       │   │  • Animations  │  │
│  │                  │   │    Status/Timeline│   │                │  │
│  └────────┬─────────┘   └────────┬─────────┘   └────────┬───────┘  │
│           │                      │                      │          │
│           ▼                      ▼                      ▼          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              State Management & Infrastructure               │  │
│  │                                                              │  │
│  │  • Zustand Stores (Auth, Config, Quality, Theme)             │  │
│  │  • Tauri API Wrapper (tauriApi.ts)                           │  │
│  │  • Lifecycle Hooks (useAppInit, useHeartbeat, useEventList)  │  │
│  └──────────────────────────┬───────────────────────────────────┘  │
├──────────────────────────────┼────────────────────────────────────┤
│                              │ IPC (Invoke/Listen)                │
├──────────────────────────────▼────────────────────────────────────┤
│                       Rust Backend Layer                          │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  lib.rs (Entry Point)                                        │  │
│  │  ├─ config_state.rs (Atomic Write/Rename, Schema Migration)  │  │
│  │  ├─ protocol_cmds.rs (Login/Logout/Portal Status)            │  │
│  │  ├─ self_service_cmds.rs (Biometric Gate, Credential Mgmt)   │  │
│  │  ├─ quality_cmds.rs (Latency Testing, History)               │  │
│  │  └─ identity_gate.rs (TTL-based Biometric Verification)      │  │
│  └──────────────────────────┬───────────────────────────────────┘  │
├──────────────────────────────┼────────────────────────────────────┤
│                              │ JNI Bridge                         │
├──────────────────────────────▼────────────────────────────────────┤
│                     Android Native Plugins                        │
│  ┌──────────────────────┐ ┌──────────────────┐ ┌────────────────┐  │
│  │ Monitor Service      │ │ Keystore Plugin  │ │ Network Bind   │  │
│  │                      │ │                  │ │                │  │
│  │ • Foreground Service │ │ • AES-GCM Encrypt│ │ • WiFi Binding │  │
│  │ • Notification Upd   │ │ • Decrypt        │ │ • Captive Portal│  │
│  │ • Boot Autostart     │ │                  │ │ • SSID Perm    │  │
│  └──────────────────────┘ └──────────────────┘ └────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

数据流主要体现为从用户交互到后端执行，再到状态同步的闭环过程。

```text
┌──────────────┐       ┌──────────────────┐       ┌──────────────────┐
│   User Input │       │   Frontend Store │       │   Rust Backend   │
│              │       │                  │       │                  │
│ Click "Login"│──────►│ useAuthStore     │──────►│ invoke do_login  │
│              │       │  • Validate Form │       │                  │
│              │       │  • Call tauriApi │       │  • Check Config  │
│              │       │                  │       │  • JNI: Login    │
│              │◄──────│  • Update Status │◄──────│  • Save History  │
│              │       │  • Show Toast    │       │                  │
└──────────────┘       └──────────────────┘       └──────────────────┘
         ▲                       │                           │
         │                       ▼                           ▼
         │               ┌──────────────────┐       ┌──────────────────┐
         │               │   UI Components  │       │   Native Plugins │
         │               │                  │       │                  │
         │               │  • Dashboard     │       │  • MonitorSvc    │
         │               │  • StatusBar     │       │  • Keystore      │
         │               │                  │       │                  │
         └───────────────┴──────────────────┘       └──────────────────┘
                   ▲                               │
                   │                               │ JNI Callbacks
                   │                               ▼
                   │                       ┌──────────────────┐
                   │                       │ Android OS/API   │
                   │                       │                  │
                   │                       │ • WiFi Manager   │
                   │                       │ • KeyStore       │
                   │                       │ • Battery Opt    │
                   │                       └──────────────────┘
```

**关键流程说明：**
1.  **认证流程**：前端 `AccountPanel` 收集凭证 -> `useAuthStore` 校验格式 -> `tauriApi` 调用 `do_login` -> Rust 后端调用 JNI `campus_network_bind` -> 结果回传更新 Store -> UI 刷新。
2.  **监控流程**：`MonitorServicePlugin` 启动前台服务 -> 定期触发 `check_campus_status` -> Rust 后端通过 JNI 获取网络状态 -> 推送至 `qualityStore` -> `StatusBar` 组件响应更新。
3.  **安全操作**：用户请求查看运营商密码 -> `SelfServicePanel` 拦截 -> 调用 `verify_biometric_identity` -> Rust `identity_gate` 检查 TTL -> 若过期则唤起系统生物识别 -> 通过后解密并返回。

## Dependency Graph

依赖关系展示了模块间的耦合方向，箭头指向被依赖方。

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Dependency Direction                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  [Frontend UI]                                                      │
│  ├── app-shell (main.tsx, App.tsx)                                 │
│  │   ├──► frontend-account (AccountPanel, useAccount)              │
│  │   ├──► frontend-settings (SettingsPanel, useSettings)           │
│  │   ├──► frontend-monitor (MonitorPanel, useMonitor)              │
│  │   └──► frontend-hooks-stores (Zustand Stores, Tauri API)        │
│  │                                                                       │
│  ├── frontend-account                                                    │
│  │   ├──► hooks-stores (useConfigStore, tauriApi)                       │
│  │   └──► frontend-lib (utils, i18n)                                    │
│  │                                                                       │
│  ├── frontend-settings                                                   │
│  │   ├──► hooks-stores (useConfigStore, useThemeStore)                  │
│  │   └──► frontend-lib (notificationPermission)                          │
│  │                                                                       │
│  ├── frontend-monitor                                                    │
│  │   ├──► hooks-stores (authStore, qualityStore)                        │
│  │   └──► frontend-lib (latency, glass-dock)                            │
│  │                                                                       │
│  ├── frontend-hooks-stores                                               │
│  │   ├──► frontend-lib (safeStorage, utils)                             │
│  │   └──► backend-tauri (Rust Commands via IPC)                         │
│  │                                                                       │
│  ├── frontend-lib                                                        │
│  │   └──► (External: i18next, Hyalite, GSAP)                            │
│  │                                                                       │
│  [Backend Core]                                                         │
│  ├── android-rust-backend (lib.rs)                                       │
│  │   ├──► config_state.rs (Persistent Storage)                          │
│  │   ├──► protocol_cmds.rs (Login Logic)                                │
│  │   ├──► self_service_cmds.rs (Biometric Gate)                        │
│  │   ├──► quality_cmds.rs (Monitoring)                                  │
│  │   └──► JNI (Native Plugins)                                          │
│  │                                                                       │
│  ├── plugins                                                             │
│  │   ├── monitor-service (Foreground Service)                           │
│  │   ├── keystore (AES Encryption)                                      │
│  │   └── network-bind (WiFi/SSID Management)                            │
│  │                                                                       │
│  [Infrastructure]                                                       │
│  ├── workspace-docs-and-build (CI/CD, Changelog)                        │
│  └── tauri-node-compile-cache (Build Artifacts)                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Module Roles

各模块在系统中的具体职责如下：

### 1. Android Frontend App Shell (`android-frontend-app-shell`)
*   **角色**: 应用入口与布局容器。
*   **职责**: 初始化全局样式、GSAP 动画库、崩溃恢复逻辑；根据设备形态（手机/平板）渲染不同的导航结构（Tab vs Dock）。
*   **关键风险**: Mock 数据结构与后端不一致可能导致属性读取错误；玻璃栏启动期可能存在视觉跳变。

### 2. Account Module (`android-frontend-account`)
*   **角色**: 账号管理与自助服务 UI。
*   **职责**: 提供账号列表、切换、绑定运营商、修改密码等功能的界面；处理在线设备展示与上网记录查询。
*   **关键模式**: 使用 TTL 机制控制敏感操作的验证频率；采用数据归一化将不同来源的记录统一展示。

### 3. Settings Module (`android-frontend-settings`)
*   **角色**: 系统配置与引导。
*   **职责**: 管理主题、保活设置、新手引导向导；聚合配置状态并提供跨平台差异化的 UI。
*   **关键模式**: 平台差异化硬编码（如安卓特有的保活卡片）；异步竞态处理（组件卸载后清理回调）。

### 4. Monitor Module (`android-frontend-monitor`)
*   **角色**: 网络监控可视化。
*   **职责**: 实时显示网络质量、延迟测试、Portal 状态；提供测速入口与历史记录图表。
*   **关键模式**: 竞态与卸载保护（双重锁）；细粒度状态订阅以避免不必要的重渲染。

### 5. Hooks & Stores (`android-frontend-hooks-stores`)
*   **角色**: 核心状态管理与基础设施。
*   **职责**: 基于 Zustand 构建全局 Store（Auth, Config, Quality, Theme）；封装 Tauri API 调用；管理应用生命周期事件（心跳、快捷键、后台检测）。
*   **关键模式**: 脏标记与回退策略（防止配置丢失）；防御性竞态处理（串行锁、时间戳节流）。

### 6. Library & Utils (`android-frontend-i18n-and-lib`)
*   **角色**: 通用基础能力库。
*   **职责**: 国际化资源加载；高性能悬浮栏视觉效果（玻璃拟态、镜头物理仿真）；网络质量数据标准化；存储容错（内存回退）。
*   **关键模式**: 零 Re-render 动画（直接操作 DOM/CSS 变量）；Chromium Backdrop Filter 黑屏规避。

### 7. Android Rust Backend (`android-rust-backend`)
*   **角色**: 核心业务逻辑与安全门控。
*   **职责**: 封装账号管理、配置持久化、校园网连接、质量监控、生物识别验证等逻辑；通过 JNI 桥接 Android 原生能力。
*   **关键模式**: 原子性数据持久化（写临时文件+rename）；IO 串行化与并发控制（Mutex）；静默失败设计（兼容异构设备）。

### 8. Tauri Shell & Plugins (`android-tauri-shell-and-plugins`)
*   **角色**: 原生能力扩展与构建流程。
*   **职责**: 实现前台服务、密钥存储、网络绑定三大核心插件；管理 Gradle 构建与 APK 打包。
*   **关键风险**: 权限配置陷阱（capabilities.json 需包含自定义插件权限）；硬编码路径导致的构建环境耦合。