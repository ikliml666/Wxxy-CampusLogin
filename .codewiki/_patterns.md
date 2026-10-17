---
title: 反复出现的模式 (Patterns)
type: pattern
source_files:
  - android/frontend/src/network/NetworkPanel.tsx
  - android/frontend/src/network/useDnsSetup.ts
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/monitor/LatencyTimeline.tsx
  - android/frontend/src/main.tsx
  - android/frontend/src/shared/ConfirmDialog.tsx
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
  - android/frontend/vite.mock.config.ts
tags:
  - architecture
  - conventions
  - error-handling
  - state-management
---

## Overview

本仓库在 Android Tauri 应用开发中形成了一套高度一致的工程化模式。这些模式主要围绕 **Tauri IPC 通信的可靠性**、**Zustand 状态管理的精细化控制**、**Android 系统级资源的降级与适配** 以及 **前端构建环境的 Mock 隔离** 展开。通过将这些模式标准化，项目能够在一个复杂的跨平台（Rust/Kotlin/JS）栈中保持代码的可维护性和稳定性。以下总结了仓库中反复出现的核心模式及其具体实现证据。

## Patterns

### 1. IPC 重试与容错封装模式

在所有涉及底层原生能力（Rust Backend 或 Android System APIs）的调用中，前端强制使用统一的包装器来处理网络不稳定或原生进程无响应导致的失败。这种模式确保了 UI 层不会因为偶发的 IPC 错误而崩溃或显示不可用的状态。

*   **统一包装器**: 所有底层 IPC 通信必须经过 `tauriApiWithRetry`。该函数在 `tauriApi.ts` 中定义，为所有 Tauri invoke 调用添加了自动重试逻辑，专门处理 Android WebView 后台挂起或前台唤醒时的连接抖动。
    *   **证据**: `android/frontend/src/hooks/tauriApi.ts` (`tauriApiWithRetry`, L217-L221)。
*   **业务层重试**: 在具体的业务 Hook 中，如 `useSelfCardFetch`，内置了针对自助服务数据获取的自动重试机制，以应对校园网认证后的短暂延迟。
    *   **证据**: `android/frontend/src/auth/DashboardPanel.tsx` (L403-446, `useSelfCardFetch` 逻辑及 `fetchNow` 中的锁保护)。
*   **原子性写入保护**: 在后端 Rust 侧，配置保存不直接覆盖文件，而是采用“临时文件 + 原子重命名”策略，配合互斥锁防止并发读写导致的数据损坏。
    *   **证据**: `android/src-tauri/src/config_state.rs` (L345-L370, `save_config` 命令中的 `tmp + rename` 逻辑)。

### 2. Zustand 状态分离与防抖提交模式

前端广泛使用 Zustand 进行全局状态管理，但为了避免高频操作（如输入框打字、滚动监听）导致的过度渲染和 Store 脏数据，形成了一套严格的“本地草稿 + 异步提交”模式。

*   **精确订阅优化**: 使用 `useShallow` 或精确字段订阅来避免整个 Store 变更引发的级联重渲染，特别是在聚合了大量数据的账户面板中。
    *   **证据**: `android/frontend/src/auth/DashboardPanel.tsx` (L8, `useShallow` 订阅配置状态); `android/frontend/src/network/NetworkPanel.tsx` (L16, `useShallow` 订阅配置)。
*   **竞态锁控制**: 对于关键的状态切换操作（如账号切换），引入异步互斥锁 (`useAsyncLock`) 或内部状态标志 (`switchingAccount`)，确保请求串行化，防止并发请求导致的状态不一致。
    *   **证据**: `android/frontend/src/auth/DashboardPanel.tsx` (L106-129, `AccountManageCard` 中的 `switchingAccount` 状态锁); `android/frontend/src/auth/useAuth.ts` (L14, `authStore` 中的 `doLogin/doLogout` 状态)。

### 3. Android 系统资源降级与门控模式

由于 Android 碎片化和系统权限限制，前端无法保证所有功能在所有设备上可用。因此，项目采用了“优雅降级”和“权限门控”的模式，而非直接拦截或报错。

*   **权限引导降级**: 当应用需要高级系统权限（如 Secure Settings、WiFi SSID 读取）时，如果未获得授权，UI 不会直接禁用功能，而是提供 ADB 命令复制、手动跳转设置页等降级交互方案。
    *   **证据**: `android/frontend/src/network/NetworkPanel.tsx` (L48-103, `secureStatus` 状态管理与 ADB 命令复制 UI); `android/frontend/src/network/useDnsSetup.ts` (L18-82, DNS 检测与优化逻辑)。
*   **生物识别 TTL 对齐**: 前端的安全门控（Hello Gate）TTL 刻意略低于后端，以防止因时钟偏差或处理延迟导致的验证死锁，这是一种防御性的时间同步约定。
    *   **证据**: `android/frontend/src/account/selfServiceState.ts` (L61-62, `useHelloGate` TTL 570s vs 后端 600s)。
*   **厂商 ROM 兼容性链**: 在保活和自启设置中，尝试特定品牌（如小米、华为）的设置页跳转，失败后逐级降级至通用应用详情页，接受部分功能可能失效的风险。
    *   **证据**: `android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt` (L234-286, `openVendorBatterySettings` 的硬编码 ComponentName 降级链)。

### 4. 动画性能分级与上下文感知模式

为了在低端 Android 设备上保持流畅度，同时在高配设备上提供丰富动效，项目建立了基于设备性能和页面活跃度的双重性能优化模式。

*   **自适应帧率与动画降级**: 通过 `useAdaptiveFramePace` 和 `useAnimationProfile` 钩子，根据设备性能分级（旗舰/中端/低端）动态调整动画参数。在 Economy 模式下，将复杂的 Spring 动画降级为简单的 Duration/Ease 动画。
    *   **证据**: `android/frontend/src/hooks/useAdaptiveFramePace.ts` (L33-L78); `android/frontend/src/shared/AnimatedNumber.tsx` (引用 `useAnimationProfile`)。
*   **页面活跃度上下文**: 使用 `AnimationActiveProvider` 和 `usePageIdle` 检测页面是否处于后台或不可见状态。当页面不可见时，暂停 GSAP ticker 或降低刷新率，防止 rAF 停摆导致的动画卡死或资源浪费。
    *   **证据**: `android/frontend/src/hooks/usePageIdle.ts` (L3-L83); `android/frontend/src/main.tsx` (L75-93, `setupCrashRecovery` 中的 visibilitychange 监听以恢复/暂停 GSAP timeline)。
*   **Portal 挂载与样式隔离**: 悬浮组件（如 Tooltip、Capsule）使用 `createPortal` 挂载至 `document.body`，以减少 DOM 层级对重排的影响，但需注意样式隔离和滚动监听带来的潜在性能开销。
    *   **证据**: `android/frontend/src/monitor/NetworkQualityCapsule.tsx` (L38-218, `createPortal` 使用)。

### 5. 构建环境 Mock 分层与隔离模式

为了支持前端开发者在无真实 Android 环境（如浏览器中）进行开发和测试，项目建立了一套分层 Mock 体系，模拟 Tauri API 和后端行为。

*   **分层 Mock 注入**: 采用 `mock-tauri.js` (基础 UI 接口) -> `mock-tauri-addon.js` (场景注入) -> `tauri-mock.js` (完整业务逻辑) 的分层结构。通过 URL 参数动态切换 Mock 行为，加速 UI 验证迭代。
    *   **证据**: `android/frontend/public/mock-tauri.js` (L22-36, 注入 `window.__TAURI_INTERNALS__`); `android/frontend/public/mock-tauri-addon.js` (L8-25, 动态注入)。
*   **内存状态模拟**: Mock 层完全在内存中维护 `onlineList`, `billRows` 等数据结构，并通过 `emitEvent` 机制模拟异步事件通知，确保前端状态管理的完整性，无需连接真实后端。
    *   **证据**: `android/frontend/tauri-mock.js` (L13-163, 内存状态管理与事件总线模拟)。
*   **Windows 兼容性规避**: 针对 Windows Defender 拦截 Vite 缓存写入的问题，Mock 配置文件中将缓存路径移至临时目录，并忽略 IDE 临时文件，确保跨平台开发体验一致。
    *   **证据**: `android/frontend/vite.mock.config.ts` (L36, `cacheDir` 配置为 `os.tmpdir()` 以绕过 Defender 拦截)。

### 6. 安全敏感信息门控与脱敏模式

在处理用户敏感数据（如在线状态、密码、个人信息）时，采用显式的门控（Gate）和按需解密模式，防止数据在未授权情况下被意外暴露。

*   **Hello 门控验证**: `SelfOnlineCard` 使用 `useSelfCardReveal` 结合生物识别（Hello）门控来显示敏感数据。只有在验证通过后，数据才会被解密并渲染，这是一种安全的数据脱敏模式。
    *   **证据**: `android/frontend/src/auth/DashboardPanel.tsx` (L346-358, `useSelfCardReveal` 钩子定义及 `SelfOnlineCard` 的使用)。
*   **硬件级密钥存储**: 敏感凭据（如密码）不直接存储在明文文件或 SharedPrefs 中，而是通过 Tauri Plugin 桥接 Android Keystore 进行 AES-GCM 加密存储。
    *   **证据**: `android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt` (L50-63, `encrypt` 命令使用 AES/GCM/NoPadding)。

### 7. React StrictMode 与生命周期兼容模式

为了兼容 React 18+ 的 StrictMode 特性（Effect 双重执行）以及 Android WebView 的生命周期特点，多个核心 Hooks 实现了特定的兼容逻辑。

*   **StrictMode 双重挂载防护**: 在 `useInitialDataLoad`、`useEventListeners` 等 Hook 中，使用 `mountedRef` 或类似机制确保在 StrictMode 下 Effect 的双重调用不会导致异步操作丢失或重复注册。
    *   **证据**: `android/frontend/src/auth/DashboardPanel.tsx` (L106-112, `AccountManageCard` 中的 `mountedRef` 处理); `android/frontend/src/network/NetworkPanel.tsx` (L22-26, `mountedRef` 处理)。
*   **WebView 可见性恢复**: 依赖 `visibilitychange` 事件监听来恢复后台冻结的 GSAP ticker 或动画循环，部分解决 Android ROM 后台暂停 rAF 导致的应用假死问题。
    *   **证据**: `android/frontend/src/main.tsx` (L75-93, `setupCrashRecovery` 中的 visibilitychange 监听以恢复/暂停 GSAP timeline)。