---
type: architecture
source_files: []
tags: [architecture, overview, tauri, android, zustand]
---

# System Overview

## Overview

本系统是一个基于 **Tauri v2** 构建的 Android 校园网登录助手应用。它采用 **Rust + Kotlin + React** 的混合架构，通过 Tauri IPC 机制实现前端 UI 与后端 Rust/Kotlin 原生能力的交互。

系统核心目标包括：
1.  **自动化认证**：处理校园网 Captive Portal、Wi-Fi 绑定及状态探测。
2.  **安全存储**：利用 Android Keystore 硬件级加密存储敏感凭据。
3.  **后台保活**：通过前台服务、WakeLock 及电池优化白名单确保监控任务持续运行。
4.  **高性能 UI**：基于 Zustand 的状态管理与 Framer Motion/GSAP 动画引擎，适配移动端与平板双形态。

## System Overview

系统整体分为三个主要层级：**前端表现层 (Frontend)**、**基础设施与状态层 (Infrastructure & State)**、**原生后端层 (Native Backend)**。

```text
┌──────────────────────────────────────────────────────────────────────┐
│                        Frontend Layer (React/TSX)                    │
│                                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────┐ │
│  │  Auth Module │  │ Network Mod. │  │ Monitor Mod. │  │AccountMod│ │
│  │(Dashboard,  │  │(DNS, DoH,   │  │(Latency,     │  │(Records, │ │
│  │ AboutDialog) │  │  Wi-Fi Bind)│  │  Quality,    │  │  Settings)│ │
│  └──────┬──────┘  └──────┬───────┘  └──────┬───────┘  └────┬─────┘ │
│         │                 │                  │              │       │
│  ┌──────▼─────────────────▼──────────────────▼──────────────▼─────┐ │
│  │                   Shared UI & Shell Layer                      │ │
│  │  (AppShell, AnimatedCard, ToastContainer, ErrorBoundary, etc.) │ │
│  └───────────────────────────┬────────────────────────────────────┘ │
│                              │                                      │
│  ┌───────────────────────────▼────────────────────────────────────┐ │
│  │               Infrastructure & State Layer                     │ │
│  │  (Zustand Stores: useAuthStore, useConfigStore, useQualityStore)│ │
│  │  (Hooks: useTauriApi, useAnimationProfile, useFormFactor)      │ │
│  └───────────────────────────┬────────────────────────────────────┘ │
└──────────────────────────────┼──────────────────────────────────────┘
                               │ IPC / Invoke
                               ▼
┌──────────────────────────────────────────────────────────────────────┐
│                       Native Backend Layer                           │
│                                                                      │
│  ┌──────────────────────┐   ┌──────────────────────────────────┐   │
│  │  Rust Core (src-tauri)│   │   Tauri Plugins (Kotlin/Rust)    │   │
│  │                      │   │                                  │   │
│  │ • Config/Acct Cmds   │◄──►│ • Foreground Service Plugin     │   │
│  │ • Campus Detect      │   │   (MonitorService, WakeLock)      │   │
│  │ • Crypto Bridge      │   │ • Network Bind Plugin             │   │
│  │                      │   │   (WifiBind, SSID, VPN Handle)    │   │
│  └──────────┬───────────┘   │ • Keystore Plugin                 │   │
│             │               │   (AndroidKeyStore AES-GCM)       │   │
│             │               └──────────────────────────────────┘   │
│             │                                                      │
│             └──────────────────────┬───────────────────────────────┘
│                                    │ JNI / System APIs
│                                    ▼
│                          Android OS Services
│                    (ConnectivityManager, PowerManager,
│                     NotificationManager, Keystore)
└──────────────────────────────────────────────────────────────────────┘
```

## Data Flow

数据流遵循 **单向数据流 (One-Way Data Flow)** 原则，结合异步事件驱动。

1.  **UI 触发 -> Store 更新**: 用户操作（如点击登录）触发 React 组件中的 Handler。Handler 调用 `useAuthStore` 或 `useConfigStore` 更新局部状态。
2.  **Store -> IPC Call**: Store 中的 Action（如 `doLogin`）封装了 `tauriApi.invoke()` 调用，向 Rust 后端发送命令。
3.  **IPC -> Native Logic**: Rust 后端接收 Command，执行原生逻辑（如调用 Kotlin 插件进行网络绑定或 Keystore 加解密）。
4.  **Native -> Event Bus**: 原生层完成操作后，通过 Tauri Event Listeners (`listen`) 向前端推送结果或状态变更（如 `onlineStatusChanged`, `networkQualityUpdate`）。
5.  **Event -> Store Sync**: 前端监听器捕获事件，更新对应的 Zustand Store（如 `useQualityStore.setNetworkQuality()`）。
6.  **Store -> UI Re-render**: Store 变化触发依赖该状态的 React 组件重新渲染，展示最新数据。

```text
[User Action]
      │
      ▼
┌─────────────┐     invoke()      ┌──────────────────┐
│  React UI   │ ────────────────► │  Rust Backend    │
│  Component  │ ◄─────────────── │  (Commands/State)│
└──────┬──────┘   Event Listen    └───────┬──────────┘
       │                                 │
       │        Update Store             │ JNI / System API
       ▼                                 ▼
┌─────────────┐               ┌──────────────────┐
│  Zustand    │ ◄──────────── │  Android Native  │
│  Store      │               │  Services        │
└──────┬──────┘               └──────────────────┘
       │
       ▼
┌─────────────┐
│  React UI   │ (Re-render with new data)
└─────────────┘
```

## Dependency Graph

模块间依赖方向清晰，旨在降低耦合度。

*   **Frontend Modules** 依赖 **Shared UI** 和 **Hooks/Stores**。
*   **Hooks/Stores** 依赖 **Tauri API** (IPC) 和 **Shared Libs** (i18n, Latency Utils)。
*   **Rust Backend** 依赖 **Core Logic** 和 **Plugins**。
*   **Plugins** 依赖 **Android SDK** 和 **Tauri Plugin Infra**。

```text
┌──────────────────────────────────────────────────────────────────────┐
│                         Dependency Direction                         │
│                                                                      │
│  [frontend-auth] ──► [frontend-hooks-store] ──► [rust-backend-core] │
│       │                     │                        ▲               │
│       │                     │                        │               │
│       ▼                     ▼                        │               │
│  [frontend-account] [frontend-shared-ui]             │               │
│       │                     │                        │               │
│       ▼                     ▼                        │               │
│  [frontend-network] [frontend-monitor]               │               │
│       │                     │                        │               │
│       └─────────────────────┴────────────────────────┘               │
│                                                                       │
│  [rust-backend-core] ──► [rust-plugins-keystore]                      │
│       │                                ▲                              │
│       │                                │                             │
│       ▼                                │                             │
│  [rust-plugins-foreground-service]     │                             │
│       │                                │                             │
│       └────────────────────────────────┘                             │
│                                                                       │
│  [rust-plugins-network-bind] ────────────────────────────────────────┘│
│                                                                       │
│  (All Rust Modules) ──► [android-system-services]                     │
└──────────────────────────────────────────────────────────────────────┘
```

## Module Roles

### 1. Frontend Modules (表现层)

*   **`frontend-ui-shell`**: 应用入口与布局容器。负责响应式外壳（手机/平板切换）、全局主题初始化、崩溃恢复机制及导航栏管理。
*   **`frontend-auth`**: 认证生命周期管理。包含登录/登出逻辑、主仪表盘 (`DashboardPanel`) 及关于对话框。提供敏感数据的门控显示模式。
*   **`frontend-network`**: 网络配置与优化。负责 DNS 设置、DoH 检测、Wi-Fi 绑定逻辑及夜间出站开关。提供权限引导 UI。
*   **`frontend-monitor`**: 实时监控与可视化。包含状态栏、质量评估面板、延迟时间线及网速测试。负责实时网络数据的采集与展示。
*   **`frontend-account`**: 账户与自助服务。管理多账号切换、凭据存储、运营商绑定、上网记录及生物识别门控验证。
*   **`frontend-settings`**: 全局配置与引导。处理主题、通知权限、开机自启、Android 保活设置及新用户向导 (Onboarding Wizard)。

### 2. Infrastructure & State (基础设施层)

*   **`frontend-hooks-store`**: 核心状态管理与 IPC 桥接。
    *   **Zustand Stores**: `useAuthStore` (认证), `useConfigStore` (配置), `useQualityStore` (质量), `useLogToastStore` (日志)。
    *   **Hooks**: `tauriApi` (统一 RPC), `useFormFactor` (设备形态), `useAnimationProfile` (动画性能), `useHeartbeat` (存活检测)。
*   **`frontend-shared-ui`**: 原子组件库。提供跨模块复用的 UI 元素，如 `AnimatedNumber`, `ConfirmDialog`, `LogPanel`, `SegmentTabs` 及错误边界处理。
*   **`frontend-lib-i18n`**: 国际化与工具函数。负责语言包加载、颜色转换 (`hexToHsl`)、延迟等级计算及增量数据合并策略。
*   **`frontend-build-config`**: 构建与 Mock 环境。定义 Vite/Tailwind 配置，提供分层 Mock API (`mock-tauri.js`) 以支持无后端开发调试。

### 3. Native Backend (原生后端层)

*   **`rust-backend-core`**: Rust 核心逻辑。管理配置持久化、多账号 CRUD、校园网状态探测 (`campus_detect`) 及 CPU 亲和性绑定。通过 IPC Commands 暴露接口。
*   **`rust-plugins-keystore`**: 安全存储插件。基于 Android KeyStore 提供透明的 AES-GCM 加解密服务，保护用户密码等敏感信息。
*   **`rust-plugins-foreground-service`**: 前台服务插件。维持应用后台运行，管理 WakeLock/WifiLock，处理通知更新及电池优化白名单引导。
*   **`rust-plugins-network-bind`**: 网络绑定插件。处理进程级 Wi-Fi 绑定、Captive Portal 自动接受、SSID 获取及系统设置快照还原。
*   **`rust-tauri-project`**: 项目元数据与生成代码。定义 Tauri 构建流程、Android 工程结构及生成的 Kotlin 胶水代码。