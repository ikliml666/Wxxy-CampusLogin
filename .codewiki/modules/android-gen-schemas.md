---
title: android-gen-schemas
type: module
source_files:
  - android/src-tauri/gen/schemas/acl-manifests.json
  - android/src-tauri/gen/schemas/android-schema.json
  - android/src-tauri/gen/schemas/capabilities.json
  - android/src-tauri/gen/schemas/desktop-schema.json
  - android/src-tauri/gen/schemas/mobile-schema.json
  - android/src-tauri/gen/schemas/windows-schema.json
tags: [tauri, schema, acl, capability, permission]
---

## Overview
`android-gen-schemas` 模块负责管理 Tauri 应用（特别是 Android 端）在构建时自动生成的 JSON Schema 文件。这些文件构成了 Tauri 权限控制系统（ACL）的底层基础，用于校验前端 IPC 调用的合法性。该模块包含针对特定平台（Android、Desktop、Mobile、Windows）的 Schema 定义，以及一个核心的能力（Capability）配置文件。通过严格的类型约束，确保只有经过授权的窗口和 WebView 才能访问特定的 Rust 后端命令。

## Key Components

### `acl-manifests.json`
**路径**: `android/src-tauri/gen/schemas/acl-manifests.json`
此文件定义了所有可用的权限标识符（Permission Identifiers）、默认权限集以及具体的允许/拒绝命令映射。它是权限系统的“字典”。

- **根对象结构**:
  - 包含多个键值对，每个键代表一个插件或功能模块（如 `biometric`, `campus-keystore`, `core` 等）。
  - 每个模块对象包含：
    - `default_permission`: 该模块的默认权限配置。
    - `permissions`: 该模块下所有细粒度权限的定义。
    - `permission_sets`: 权限集定义（当前为空对象 `{}`）。
    - `global_scope_schema`: 全局作用域 Schema 定义（仅在 `opener` 模块中有内容）。

- **关键模块定义**:
  - **`biometric`** (行 1): 生物识别权限。默认包含 `allow-authenticate` 和 `allow-status`。
  - **`campus-keystore`** (行 23): 校园登录密钥存储。使用 AndroidKeyStore AES-GCM。默认包含 `allow-encrypt` 和 `allow-decrypt`。
  - **`campus-monitor-service`** (行 45): 校园网后台监控保活服务。涉及前台服务、WifiLock、WakeLock 及开机自启。默认包含大量命令如 `startMonitor`, `stopMonitor`, `installApk` 等。
  - **`campus-network-bind`** (行 87): 网络绑定到 WLAN。强制流量走 WiFi。默认包含 `bindToWifi`, `unbind` 等。
  - **`core`** (行 129): 核心插件集合。默认启用 `core:path:default`, `core:event:default`, `core:window:default` 等 9 个子模块的默认权限。
  - **`core:app`** (行 136): 应用信息获取。包括版本、名称、Tauri 版本等。
  - **`core:event`** (行 160): 事件系统。包括 `listen`, `unlisten`, `emit`, `emit_to`。
  - **`core:image`** (行 184): 图像处理。包括 `new`, `from_bytes`, `from_path`, `rgba`, `size`。
  - **`core:menu`** (行 208): 菜单管理。包括创建、插入、移除、设置文本/图标等。
  - **`core:path`** (行 244): 路径处理。包括 `resolve`, `normalize`, `join`, `basename` 等。
  - **`core:resources`** (行 268): 资源管理。仅包含 `close` 命令。
  - **`core:tray`** (行 275): 系统托盘。包括创建、设置图标/菜单/标题、显示/隐藏等。
  - **`core:webview`** (行 307): WebView 管理。包括创建、关闭、位置/大小设置、调试工具切换等。
  - **`core:window`** (行 363): 窗口管理。包括创建、关闭、最大化/最小化、全屏、主题、位置/尺寸、光标控制等。
  - **`notification`** (行 519): 通知服务。包括发送、取消、通道管理、权限请求等。
  - **`opener`** (行 555): 链接打开器。支持 URL 和文件路径打开。
    - `global_scope_schema` (行 578): 定义了 `OpenerScopeEntry` 的 JSON Schema，支持通配符 URL 和系统路径变量（如 `$HOME`, `$DESKTOP`）。

### `android-schema.json`
**路径**: `android/src-tauri/gen/schemas/android-schema.json`
Android 平台专用的 Capability 文件格式验证 Schema。

- **`CapabilityFile`** (行 4): 入口定义。接受单个 Capability 对象、Capability 数组或包含 `capabilities` 键的对象。
- **`Capability`** (行 38): 核心能力定义。
  - `identifier` (行 55): 唯一标识符（字符串）。
  - `description` (行 63): 描述信息。
  - `remote` (行 71): 远程 URL 配置 (`CapabilityRemote`)。
  - `local` (行 85): 是否启用本地 URL 访问（布尔值，默认 true）。
  - `windows` (行 93): 关联的窗口列表（支持 glob 模式）。
  - `webviews` (行 107): 关联的 WebView 列表。
  - `permissions` (行 115): 权限列表 (`PermissionEntry` 数组)。
  - `platforms` (行 129): 适用的平台限制。
- **`PermissionEntry`** (行 141): 权限条目。可以是简单的标识符引用，也可以是带有 `allow`/`deny` 作用域扩展的对象。特别针对 `opener` 权限做了特殊的 `if/then` 条件校验。
- **`Identifier`** (行 173): 权限标识符枚举。列出了所有合法的权限字符串常量，如 `biometric:default`, `core:default`, `notification:allow-show` 等。这是防止非法权限注入的关键。
- **`Value`** / **`Number`** / **`Target`** / **`Application`** (行 463-523): 辅助类型定义，用于支持权限作用域中的数据结构和平台目标。

### `capabilities.json`
**路径**: `android/src-tauri/gen/schemas/capabilities.json`
实际生效的能力配置文件。它引用了上述 Schema 中定义的权限。

- **`default`** (行 1): 默认能力配置。
  - `identifier`: `"default"`
  - `description`: "安卓端主窗口能力:官方 biometric(验证门)/opener(外链,APK 安装)/notification(监控前台通知权限请求)前端 API"
  - `local`: `true`
  - `windows`: `["main"]`
  - `permissions`:
    - `core:default`: 启用所有核心插件默认权限。
    - `biometric:default`: 启用生物识别默认权限。
    - `opener:default`: 启用链接打开默认权限。
    - `notification:default`: 启用通知默认权限。

### `desktop-schema.json`, `mobile-schema.json`, `windows-schema.json`
**路径**: 
- `android/src-tauri/gen/schemas/desktop-schema.json`
- `android/src-tauri/gen/schemas/mobile-schema.json`
- `android/src-tauri/gen/schemas/windows-schema.json`

这些文件结构与 `android-schema.json` 高度相似，但 `Identifier` 枚举的内容略有不同，反映了不同平台支持的插件差异。

- **`desktop-schema.json`** (行 1): 桌面端 Schema。缺少 `biometric`, `campus-*` 等移动端特有插件的 Identifier。主要包含 `core` 系列和 `notification`。
- **`mobile-schema.json`** (行 1): 移动通用 Schema。与 `android-schema.json` 类似，包含完整的 `biometric`, `campus-*`, `opener` 等 Identifier。
- **`windows-schema.json`** (行 1): Windows 端 Schema。类似于 desktop，但可能包含 Windows 特有的核心行为（尽管从提供的源码看，其 Identifier 列表与 desktop 非常接近，主要区别在于生成时的上下文）。

## Architecture

```text
+-----------------------------+       +---------------------------+
|   capabilities.json         |       |   acl-manifests.json      |
| (Config File)               |       | (Permission Dictionary)   |
|                             |       |                           |
| - identifier: "default"     |       | - biometric: {...}        |
| - windows: ["main"]         |       | - campus-keystore: {...}  |
| - permissions:              |       | - core: {...}             |
|   - core:default            |       | - notification: {...}     |
|   - biometric:default       |       | - opener: {...}           |
|   - ...                     |       +---------------------------+
+--------------^--------------+                  ^
               |                                 |
               | References                      | Defines Valid IDs
               |                                 |
+--------------+---------------------------------+---------------+
|                   android-schema.json                         |
| (JSON Schema Validator for Android)                         |
|                                                               |
| - Validates Capability Structure                              |
| - Checks Identifier against Enum (from acl-manifests)         |
| - Enforces Scope Rules (e.g., Opener URLs)                    |
+---------------------------------------------------------------+
```

1. **Source of Truth**: `acl-manifests.json` 是权限定义的源头，由 Tauri 构建脚本根据 Rust 代码中的宏自动生成。它定义了哪些权限存在，以及它们的默认状态。
2. **Validation Schema**: `android-schema.json` (及其他平台 Schema) 定义了 Capability 文件的合法结构。它内部的 `Identifier` 枚举通常是从 `acl-manifests.json` 中提取出来的，用于静态验证。
3. **Configuration**: `capabilities.json` 是开发者编写或 Tauri CLI 生成的最终配置文件，它引用了 `acl-manifests.json` 中定义的权限 ID。
4. **Runtime Enforcement**: Tauri 运行时加载 `capabilities.json`，并使用对应的 Schema 进行解析和验证，确保没有未授权的权限被注入。

## Data Flow

1. **Build Time**:
   - Tauri 构建脚本扫描 Rust 源代码。
   - 提取所有注册的插件和命令。
   - 生成 `acl-manifests.json`，包含所有可能的权限 ID 和默认配置。
   - 基于 `acl-manifests.json` 和平台特性，生成对应的 `*-schema.json` 文件。
   - 如果存在 `capabilities.json`，则将其与 Schema 进行比对验证。

2. **Runtime (Android)**:
   - App 启动。
   - Tauri Core 读取 `gen/schemas/capabilities.json`。
   - 解析 `default` 能力，获取 `permissions` 列表。
   - 对于 `main` 窗口下的 WebView，应用这些权限。
   - 当 JS 调用 IPC 命令（如 `biometric.authenticate`）时，Tauri 检查当前 WebView 的权限列表中是否包含 `biometric:allow-authenticate`。
   - 如果匹配且未被 `deny` 覆盖，则执行 Rust 侧的实现；否则抛出权限错误。

## Connections

- [[tauri-core]]: Tauri 核心库，负责运行时权限检查和 IPC 路由。
- [[tauri-cli]]: Tauri 命令行界面，负责在 `tauri build` 或 `tauri dev` 时触发 Schema 生成。
- [[android-security]]: Android 平台的安全机制，如 `AndroidKeyStore` (对应 `campus-keystore`) 和 `ConnectivityManager` (对应 `campus-network-bind`)。

## Known Issues

- **Schema 冗余**: `acl-manifests.json` 包含了大量的 `deny-*` 条目，虽然它们在默认情况下不被使用，但增加了文件体积。
- **手动维护风险**: `capabilities.json` 需要手动更新以添加新的权限。如果忘记添加新插件的权限，即使 `acl-manifests.json` 中已定义，前端也无法调用。
- **平台差异**: `desktop-schema.json` 和 `android-schema.json` 的 `Identifier` 列表不同。如果在跨平台开发中混淆，可能导致在某一平台上编译通过但在另一平台上运行时因权限缺失而失败。例如，`biometric` 权限在 Desktop Schema 中不存在，如果在 Desktop 上尝试引用可能会报错。