---
title: Android Frontend & Native Bridge Patterns
type: module
source_files:
  - android/frontend/vite.config.ts
  - android/frontend/vite.mock.config.ts
  - android/frontend/src/main.tsx
  - android/frontend/src/App.tsx
  - android/frontend/tauri-mock.js
  - android/frontend/src/auth/index.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/useAccount.ts
tags: [patterns, conventions, architecture, android, tauri]
---

## Overview

本模块涵盖 Android 端 Tauri 应用的前端构建、核心 UI 逻辑、物理动画引擎及原生后端（Rust/Kotlin）交互模式。架构核心在于将复杂的玻璃拟态视觉与悬浮导航栏效果解耦为纯 JS/DOM 驱动，通过 `dock-spring` 和 `glass-dock` 实现零 React Re-render 的高性能动画；同时利用 `config_state.rs` 和 `NetworkBindPlugin.kt` 处理 Android 特有的网络绑定、Captive Portal 规避及配置加密存储。

## Key Components

### 1. Vite 构建配置 (`android/frontend/vite.config.ts`, `android/frontend/vite.mock.config.ts`)

*   **`vite.config.ts`**:
    *   `alias`: `@` -> `./src` (行 12)。
    *   `server.port`: `5174` (行 18)。
    *   `build.outDir`: `dist` (行 26)。
    *   `manualChunks`: 按库拆分 vendor chunk，包括 `react`, `framer-motion`, `gsap`, `@radix-ui` (行 36-40)。
*   **`vite.mock.config.ts`**:
    *   `tauriMockInject` 插件: `transformIndexHtml` 注入 `/tauri-mock.js` (行 16)；CSS 约束 `#root` 宽度 412px 居中 (行 22)。
    *   `cacheDir`: `os.tmpdir()/wxxy-vite-android` (行 41)。
    *   `server.port`: `5176` (行 43)。

### 2. 应用入口与崩溃恢复 (`android/frontend/src/main.tsx`)

*   **`initTheme`** (行 27-44): 读取 `campus-light-mode`, `campus-theme`, `campus-glass-level` 同步至 DOM。
*   **`setupCrashRecovery`** (行 50-113):
    *   `MAX_CRASH_RELOADS = 3` (行 50)。
    *   `visibilitychange`: 后台暂停 GSAP，前台恢复 (行 82-91)。
    *   心跳机制: 每 1s 更新 `lastHeartbeatTime`，需 `isRenderLoopAlive()` 检查 rAF (行 93-103)。
    *   超时判定: 每 2s 检查心跳间隔，若 >10s 触发重载 (行 105-113)。
    *   错误监听: 捕获 `GPU`, `WebGL`, `SharedArrayBuffer` 错误 (行 71-77)。
    *   空 DOM 检测: visible 且 root 无子节点时触发恢复 (行 62-69)。

### 3. 主容器与路由 (`android/frontend/src/App.tsx`)

*   **`useFormFactor`**: 判断 `tablet` 渲染 `TabletShell`，否则 `AppInner` (行 363-375)。
*   **`AppInner` 状态**:
    *   `tab`: 受控于 `safeStorage('campus-mobile-tab')`，白名单校验 (行 57-62)。
    *   `deferredTab`: `useDeferredValue` 优化转场 (行 63)。
    *   `tabOrder`: 第 4 位根据 `configEnableNetworkQuality` 切换 `quality`/`monitor` (行 70-76)。
    *   `navDir`: 计算前后页签索引差决定 `AnimatePresence` 方向 (行 77, 119-127)。
*   **UI 结构**:
    *   `header`: 绝对定位，包含状态点、IspMark、更新胶囊等 (行 202-260)。
    *   `main`: 滚动区域，padding 动态调整 (行 270-297)。
    *   `AnimatePresence`: `m.div` + `panelVariants` 实现带方向动画 (行 281-295)。
    *   `BottomNav`: 底部导航栏 (行 299)。
*   **全局弹窗**: `ToastContainer`, `SponsorCard`, `ConfirmDialog`, `AboutDialog` 等均使用 `lazy` + `Suspense` (行 301-356)。
*   **`NotificationPermissionGate`**: Android 13+ 引导通知权限 (行 381-404)。

### 4. Mock 数据与 IPC 拦截 (`android/frontend/tauri-mock.js`)

*   **初始化**: 跳过安卓首启引导，设置 `localStorage` 标记 (行 17-26)。支持 `?nomotion=1` 禁用动画 (行 9-15)。
*   **数据结构**: `config` (行 32-42), `mockAccounts` (行 45-49), `onlineList` (行 59-75)。
*   **命令分发器 (`handlers`)**:
    *   `get_init_data`: 返回配置、版本 `2.4.2-mock` (行 88-97)。
    *   `save_config`: 广播 `config-changed` 事件 (行 99-110)。
    *   `check_campus_status` / `check_portal_status` / `render_heartbeat`: **关键非显然行为**，直接返回平铺结构体，未使用 `ok()` 包裹 (行 144-146)。
    *   `invoke(cmd, args)`: 核心路由，维护 `eventListeners` 映射表 (行 216-235)。
    *   `emitEvent(event, payload)`: 遍历回调执行，异常隔离 (行 239-247)。
    *   **截图辅助**: `?dialog=create|edit|bind` 延时自动点击 (行 261-277)。

### 5. 认证与安全门控 (`android/frontend/src/auth/index.ts`, `selfServiceState.ts`)

*   **`auth/index.ts`**: 重导出 `DashboardPanel`, `AboutDialog`, `useAuth` (行 1-3)。
*   **`selfServiceState.ts`**:
    *   `BIOMETRIC_ERROR_KEY`: 错误码映射 i18n key (行 19-32)。
    *   `VERIFY_TTL_MS = 570_000` (行 54)。
    *   `gateFresh`: 判定 TTL 内且时钟未回拨 (行 58-62)。
    *   `useHelloGate`: 首次调用或过期时请求 `verifyWindowsIdentity` (行 65-87)。
    *   `useSelfServiceVerify`: 面板级验证，支持 `config.selfReverifyEachAction` 每次操作验证 (行 105-127)。

### 6. 账号管理 Hook (`android/frontend/src/account/useAccount.ts`)

*   **并发去重**: `switchInFlight` Promise 引用 (行 11)。
*   **`handleAddAccount`**: 调用 `saveCurrentAsAccount` (行 37-55)。
*   **`handleDeleteAccount`**: 检查 `result?.activeAccount !== undefined` 更新活跃账号，修复历史缺陷 (行 57-75)。
*   **`handleSwitchAccount`**: 改用 `!== undefined` 判断，防止空串跳过 (行 77-104)。
*   **`handleRenameAccount`**: 仅更新显示名 (行 108-126)。