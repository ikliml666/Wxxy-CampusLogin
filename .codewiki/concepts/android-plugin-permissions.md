---
type: concept
title: android-plugin-permissions
source_files:
  - android/src-tauri/capabilities/default.json
  - android/plugins/keystore/permissions/default.toml
  - android/plugins/network-bind/permissions/default.toml
  - android/plugins/foreground-service/permissions/default.toml
tags:
  - tauri
  - acl
  - security
  - android
  - permissions
---

## Overview

`android-plugin-permissions` 是指 Android 端 Tauri 2 应用中，针对自研插件（如 `keystore`, `network-bind`, `foreground-service`）所定义的权限标识符（Permission Identifiers）及其在应用能力文件（Capabilities）中的授权状态。

该概念核心揭示了当前架构中一个关键的**“声明与实现分离”**模式：虽然各插件通过 `permissions/default.toml` 定义了严格的权限集，但在应用的运行时 ACL（Access Control List）入口 `capabilities/default.json` 中，这些插件的权限并未被显式授予。这是因为前端业务逻辑并未直接调用插件的 `invoke` API，而是通过统一的底层命令通道（`protocol_cmds.rs`）进行中转。这种设计解耦了权限声明与实际调用路径，但也引入了维护认知负担和潜在的运行时拒绝风险。

## Involved Modules

1.  **desktop-tauri-shell-config**:
    *   提供构建壳配置，定义 `tauri.conf.json` 和 `capabilities/default.json`。
    *   负责将前端产物接入 Tauri 运行时并收敛权限。
    *   包含对自研插件权限集的引用及 `protocol_cmds.rs` 命令通道的配置。
2.  **tauri-gen-schemas**:
    *   作为 ACL/能力生成物的快照源。
    *   提供 `acl-manifests.json`（插件命令面清单）和 `capabilities.json`（合并后的最终能力结果）。
    *   负责运行时权限校验的单一事实来源。
3.  **android-frontend-hooks-and-stores**:
    *   前端基础设施层，通过 `tauriApi` 发起 IPC 调用。
    *   实际消费权限的地方，但通过间接方式（调用后端命令而非直接 invoke 插件）。
4.  **android-frontend-monitor / android-frontend-account / android-frontend-settings**:
    *   具体业务模块，依赖上述 Hooks 执行敏感操作（如 WiFi 绑定、后台保活、Keystore 存储），其成功与否依赖于底层的权限放行。

## Flow

数据与控制流的流动体现了“间接授权”的特点：

1.  **权限定义阶段 (Build Time)**:
    *   各自研插件（`keystore`, `network-bind`, `foreground-service`）在各自的 `permissions/default.toml` 中定义所需的权限标识符。
    *   `desktop-tauri-shell-config` 中的 `build.rs` 触发构建，Tauri 编译器读取这些权限定义。

2.  **能力生成阶段 (Compile Time)**:
    *   `tauri-gen-schemas` 模块生成 `gen/schemas/capabilities.json`。
    *   **关键断点**: 尽管插件定义了权限，但 `capabilities/default.json`（唯一的手写能力文件入口）**未包含**对这些插件权限的 `allow` 或 `deny` 语句。这意味着在生成的 ACL 中，这些特定插件的命令默认处于未授权或受控状态。

3.  **运行时校验阶段 (Runtime)**:
    *   前端通过 `tauriApi` (来自 `android-frontend-hooks-and-stores`) 发送请求。
    *   请求被路由至 Rust 后端的 `protocol_cmds.rs` (由 `desktop-rust-core` 暴露)。
    *   Tauri 运行时 ACL 引擎根据 `acl-manifests.json` 和 `capabilities.json` 进行检查。
    *   **特殊路径**: 由于前端是通过应用命令通道（Application Commands）而非插件 Invoke 调用，ACL 引擎可能基于命令名称（Command Name）而非插件权限标识符进行放行，或者 `protocol_cmds.rs` 本身拥有更高的执行上下文权限。

4.  **潜在风险流**:
    *   如果未来前端改为直接 `invoke` 某个插件，而 `default.json` 仍未更新，则会导致 `Error: Permission denied`。
    *   如果 `protocol_cmds.rs` 的实现发生变更，导致调用了未在白名单中的新命令，且该命令未被 ACL 显式允许，同样会失败。

## Implementation Map

*   **权限定义源**:
    *   `android/plugins/keystore/permissions/default.toml`: Keystore 插件权限定义。
        *   包含权限: `allow-encrypt`, `allow-decrypt`。
    *   `android/plugins/network-bind/permissions/default.toml`: Network Bind 插件权限定义。
        *   包含权限: `allow-bind-to-wifi`, `allow-unbind`, `allow-accept-wifi-network`, `allow-ensure-avoid-bad-wifi`, `allow-restore-avoid-bad-wifi`, `allow-restore-written-settings`, `allow-get-secure-settings-status`。
    *   `android/plugins/foreground-service/permissions/default.toml`: Foreground Service 插件权限定义。
        *   包含权限: `allow-startMonitor`, `allow-stopMonitor`, `allow-updateNotification`, `allow-setBootAutostart`, `allow-isBootAutostartEnabled`, `allow-installApk`, `allow-beginProbeWindow`, `allow-endProbeWindow`, `allow-getPowerState`, `allow-getBatteryOptimizationInfo`, `allow-requestIgnoreBatteryOptimizations`, `allow-openVendorBatterySettings`。

*   **能力授权入口 (当前状态)**:
    *   `android/src-tauri/capabilities/default.json`: 应用唯一的能力声明文件。**注意**：此处仅显式包含了官方插件权限 (`core:default`, `biometric:default`, `opener:default`, `notification:default`)，**未包含**上述三个自研插件的权限 ID，体现了“隐式 Label 耦合”和“ACL 与命令通道分离”的模式。

*   **运行时校验与生成物**:
    *   `android/src-tauri/gen/schemas/acl-manifests.json`: 运行时读取的插件命令面清单，用于决定 IPC 命令放行或拦截。
    *   `android/src-tauri/gen/schemas/capabilities.json`: 编译合并后的最终能力结果，供运行时与 schema 校验使用。

*   **命令实现与路由 (绕过插件权限检查的关键)**:
    *   `android/src-tauri/src/protocol_cmds.rs`: 定义了大量 Tauri 命令（如 `bind_to_wifi`, `accept_wifi_network`, `set_boot_autostart` 等），这些命令由前端直接调用，从而绕过了插件层面的权限隔离，直接在后端 Rust 代码中执行。