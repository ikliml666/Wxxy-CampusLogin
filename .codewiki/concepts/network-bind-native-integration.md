---
type: concept
title: network-bind-native-integration
source_files:
- android/frontend/src/lib/notificationPermission.ts
- android/frontend/src/monitor/useMonitor.ts
- android/frontend/src/monitor/types.ts
- android/src-tauri/src/campus_detect.rs
- android/src-tauri/src/self_service_cmds.rs
- android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
tags: [android, native-binding, tauri-plugin, campus-network]
---

## Overview

`network-bind-native-integration` 是 Android 端 Tauri 应用中负责将前端业务逻辑与底层 Android 原生网络能力（特别是 WiFi 绑定、SSID 权限获取及 Captive Portal 绕过）进行桥接的核心概念。该概念横跨了前端 UI 交互层、Tauri IPC 通信层以及 Kotlin 原生插件层，旨在解决校园网环境下设备必须固定连接到特定 WiFi SSID 才能正常上网的技术约束。

其核心职责包括：
1.  **权限引导**：在应用启动或首次需要时，向用户请求并处理 Android 的 `NEARBY_WIFI_DEVICES` (WiFi SSID) 权限。
2.  **状态探测**：通过 Rust 后端调用原生插件，检测当前连接的网络是否符合“已绑定”要求。
3.  **原生绑定执行**：利用 Kotlin 插件直接操作 Android 系统设置（如 Secure Settings），强制将默认数据路由限制为指定的 WiFi，并在必要时自动接受 Captive Portal（ captive portal 认证页面）。
4.  **安全门控集成**：将网络绑定状态作为自助服务（Self Service）等敏感操作的前置条件之一，确保只有处于正确网络环境下的用户才能执行高权限操作。

## Involved Modules

该概念涉及以下关键模块及其协作关系：

*   **`android-frontend-i18n-and-lib`**: 提供权限请求的统一入口 (`requestNotificationPermission` 模式扩展至 WiFi 权限) 和渲染存活检测，确保在网络切换期间 UI 不会因 WebView 重建而崩溃。
*   **`android-frontend-monitor`**: 作为网络状态的消费者，通过 `useMonitor` Hook 订阅后端推送的网络质量与绑定状态，并触发前端的权限请求逻辑。
*   **`android-rust-backend`**: 作为 Tauri Commands 的中枢，暴露 `check_campus_status` 和 `request_wifi_ssid_permission` 接口，协调前端请求与后端逻辑。
*   **`android-tauri-shell-and-plugins`**: 包含核心的 `NetworkBindPlugin` (Kotlin)，这是实际执行 WiFi 绑定、解绑、Secure Settings 修改的系统级实现者。

## Flow

数据与控制流遵循“前端触发 -> Rust 路由 -> 原生执行 -> 状态回传”的闭环：

1.  **初始化与权限申请**:
    *   应用启动或进入监控模块时，`android-frontend-monitor` 中的 `useMonitor` Hook 检查当前配置。
    *   若未获得 WiFi SSID 读取权限，调用 `android-frontend-i18n-and-lib` 中导出的权限请求逻辑（参考 `notificationPermission.ts` 的模式），向 Android 系统申请 `NEARBY_WIFI_DEVICES` 权限。

2.  **状态探测 (Check Status)**:
    *   前端组件（如 `StatusBar` 或 `QualityPanel`）调用 `tauriApiWithRetry` 发起 `check_campus_status` 命令。
    *   该命令映射到 `android-rust-backend` 的 `campus_detect.rs`。
    *   `campus_detect.rs` 通过 JNI 调用 `NetworkBindPlugin.kt` 中的 `getCurrentWifiSsid()` 或类似方法，获取当前连接的 WiFi 名称。
    *   Rust 层将获取到的 SSID 与本地配置的“允许列表”或“绑定规则”进行比对，返回布尔值或详细状态给前端。

3.  **执行绑定 (Bind Network)**:
    *   当检测到未绑定或需要重新绑定时，前端触发绑定流程。
    *   前端调用 `bindToWifi` Tauri Command。
    *   命令路由至 `android-tauri-shell-and-plugins` 中的 `NetworkBindPlugin.kt`。
    *   **核心执行**: `NetworkBindPlugin` 使用反射调用 Android 隐藏 API 或通过 `Settings.Global.putString` 修改 `wifi_configured_networks` 或相关 Secure Settings，确保数据流量仅通过指定 SSID 传输。同时，它可能注册广播接收器以监听 WiFi 断开事件。

4.  **Captive Portal 绕过**:
    *   在绑定过程中，若检测到 Captive Portal 特征，`NetworkBindPlugin` 会自动模拟 HTTP 请求或通过 JavaScript Interface 注入脚本，自动点击认证页面上的“连接”按钮，无需用户干预。

5.  **安全门控集成**:
    *   在 `self_service_cmds.rs` 中，执行如 `query_self_dashboard` 或 `change_self_password` 等敏感操作前，会隐式或显式依赖网络绑定状态。如果 `NetworkBindPlugin` 报告当前未绑定或网络异常，Rust 层可拒绝执行或提示用户先完成网络绑定。

6.  **结果反馈**:
    *   原生插件执行完毕后，通过 Tauri 的 `invoke` 响应机制将结果（成功/失败/错误码）返回给前端。
    *   前端 `useMonitor` Hook 更新 Zustand Store 中的 `networkQuality` 或 `bindingStatus`，触发 UI 刷新（如状态栏颜色变化、Toast 提示）。

## Implementation Map

具体实现位置如下：

*   **前端权限与工具**:
    *   `android/frontend/src/lib/notificationPermission.ts`: 虽然名为通知权限，但其中封装的权限请求模式被复用于 WiFi 权限管理。
    *   `android/frontend/src/monitor/useMonitor.ts`: 监控模块的核心 Hook，负责协调网络状态轮询和权限检查逻辑。
    *   `android/frontend/src/monitor/types.ts`: 定义了 `BackgroundStatus` 和 `NetworkQuality` 类型，包含与绑定状态相关的字段。

*   **Rust 后端接口**:
    *   `android/src-tauri/src/campus_detect.rs`: 定义 `check_campus_status` 和 `request_wifi_ssid_permission` Tauri Commands。行 L216 和 L244 附近是主要实现点。
    *   `android/src-tauri/src/self_service_cmds.rs`: 在敏感操作中集成网络环境校验，行 L67 附近的 `verify_biometric_identity` 可能间接关联网络上下文。

*   **Android 原生插件 (JNI/Kotlin)**:
    *   `android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt`: **核心实现文件**。行 L69-656 实现了 `bindToWifi`, `unbind`, `ensureAvoidBadWifi`, `restoreWrittenSettings` 等关键方法。这是所有网络绑定逻辑的物理落地处。
    *   `android/plugins/network-bind/build.rs`: 构建脚本，确保原生库正确编译。