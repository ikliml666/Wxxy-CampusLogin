---
title: rust-tauri-project
type: module
source_files:
  - android/src-tauri/build.rs
  - android/src-tauri/capabilities/default.json
  - android/src-tauri/Cargo.toml
  - android/src-tauri/gen/android/.editorconfig
  - android/src-tauri/gen/android/.gitignore
  - android/src-tauri/gen/android/app/.gitignore
  - android/src-tauri/gen/android/app/build.gradle.kts
  - android/src-tauri/gen/android/app/proguard-rules.pro
  - android/src-tauri/gen/android/app/proguard-tauri.pro
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Ipc.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Logger.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/PermissionHelper.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/proguard-wry.pro
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Rust.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebChromeClient.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebView.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebViewClient.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/TauriActivity.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/WryActivity.kt
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt
  - android/src-tauri/gen/android/app/src/main/res/drawable/ic_launcher_background.xml
  - android/src-tauri/gen/android/app/src/main/res/drawable-v24/ic_launcher_foreground.xml
  - android/src-tauri/gen/android/app/src/main/res/layout/activity_main.xml
  - android/src-tauri/gen/android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml
  - android/src-tauri/gen/android/app/src/main/res/values/colors.xml
  - android/src-tauri/gen/android/app/src/main/res/values/ic_launcher_background.xml
  - android/src-tauri/gen/android/app/src/main/res/values/strings.xml
  - android/src-tauri/gen/android/app/src/main/res/values/themes.xml
  - android/src-tauri/gen/android/app/src/main/res/values-night/themes.xml
  - android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml
  - android/src-tauri/gen/android/app/tauri.build.gradle.kts
  - android/src-tauri/gen/android/app/tauri.properties
  - android/src-tauri/gen/android/build.gradle.kts
  - android/src-tauri/gen/android/buildSrc/build.gradle.kts
  - android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt
  - android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt
  - android/src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties
  - android/src-tauri/gen/android/gradle.properties
  - android/src-tauri/gen/android/gradlew
  - android/src-tauri/gen/android/gradlew.bat
  - android/src-tauri/gen/android/local.properties
  - android/src-tauri/gen/android/settings.gradle
  - android/src-tauri/gen/android/tauri.settings.gradle
  - android/src-tauri/gen/schemas/capabilities.json
  - android/src-tauri/icons/android/mipmap-anydpi-v26/ic_launcher.xml
  - android/src-tauri/icons/android/values/ic_launcher_background.xml
  - android/src-tauri/tauri.conf.json
tags: [Tauri, Android, Rust, Build System, Metadata]
---

## Overview

本模块 `rust-tauri-project` 定义了基于 Tauri v2 的 Android 客户端项目元数据。它涵盖了从 Rust 核心依赖、构建脚本到 Android 原生工程生成的完整配置集。主要功能包括：定义应用版本与标识、配置 WebView 安全策略（CSP）、管理插件依赖关系、以及生成用于构建 APK 的 Gradle 文件和 Kotlin 胶水代码。

该模块是跨平台桌面端 (`tauri-app`) 在 Android 平台的映射，保持了核心业务逻辑的一致性，同时针对 Android 特性（如前台服务、生物识别、网络绑定）进行了适配。

## Key Components

### 1. Rust 构建层 (Cargo)

**文件**: `android/src-tauri/Cargo.toml`
*   **Package**: `campus-login-android` (v2.4.1)
    *   **Lib Crate**: `campus_login_android_lib` (cdylib, rlib)
*   **Dependencies**:
    *   `tauri = "2"`: 核心框架，启用 `protocol-asset`。
    *   `campus-login`: 路径依赖 `../../tauri-app/src-tauri`，复用桌面端核心逻辑。
    *   **Plugins**:
        *   `tauri-plugin-campus-network-bind`: 网络绑定。
        *   `tauri-plugin-campus-keystore`: AndroidKeyStore AES-GCM 加密。
        *   `tauri-plugin-campus-monitor-service`: 前台服务保活。
        *   `tauri-plugin-biometric`: 官方生物识别。
        *   `tauri-plugin-opener`: 官方外链/APK 打开。
        *   `tauri-plugin-notification`: 通知监控。
    *   **Profile.release**:
        *   `lto = "fat"`, `codegen-units = 1`, `strip = true`。
        *   **注意**: 未设置 `panic="abort"`，以保留异步任务的 unwind 捕获能力，避免进程直接崩溃。

**文件**: `android/src-tauri/build.rs`
*   **Function `main()`**: 调用 `tauri_build::build()`，触发 Tauri 构建流程。

### 2. Tauri 配置与权限

**文件**: `android/src-tauri/tauri.conf.json`
*   **App Config**:
    *   `identifier`: `com.campuslogin.client`
    *   `windows`: 单窗口 `CampusLogin` (400x800)。
    *   `security.csp`: 自定义 CSP，允许 `ipc:` 和 `http://asset.localhost`。禁用了默认的 asset CSP 修改 (`dangerousDisableAssetCspModification: ["style-src"]`)。
    *   `bundle.android.minSdkVersion`: 29。

**文件**: `android/src-tauri/capabilities/default.json`
*   **Identifier**: `default`
*   **Permissions**:
    *   `core:default`
    *   `biometric:default`
    *   `opener:default`
    *   `notification:default`
*   **Windows**: `["main"]`

### 3. Android 原生工程结构

#### 3.1 Gradle 构建系统

**文件**: `android/src-tauri/gen/android/build.gradle.kts`
*   **Classpath**: AGP 8.11.0, Kotlin 1.9.25。

**文件**: `android/src-tauri/gen/android/app/build.gradle.kts`
*   **Namespace**: `com.campuslogin.client`
*   **SDK**: compileSdk 36, targetSdk 36, minSdk 29。
*   **SigningConfig**:
    *   **Release**: 使用本地 debug.keystore (`~/.android/debug.keystore`)，密码均为 `android`。
    *   **目的**: 个人分发，免 zipalign/apksigner 后处理。
*   **Output Naming**: `Wxxy-CampusLogin_<version>.apk`。
*   **Dependencies**:
    *   手写插件: `tauri-plugin-campus-network-bind`, `tauri-plugin-campus-keystore`, `tauri-plugin-campus-monitor-service`。
    *   官方插件: `tauri-plugin-biometric`, `tauri-plugin-opener`。
*   **Rust Plugin**: 调用 `rust { rootDirRel = "../../../" }`。

**文件**: `android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt`
*   **Class `RustPlugin`**:
    *   定义 ABI Flavors: `universal`, `arm64`, `arm`, `x86`, `x86_64`。
    *   创建 Task: `rustBuildUniversal{Debug/Release}` 及其子任务。
    *   依赖 `BuildTask` 执行 CLI。

**文件**: `android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt`
*   **Class `BuildTask`**:
    *   **Method `runTauriCli()`**: 硬编码 Node.js 路径 (`C:\Program Files\nodejs\node.exe`) 和 CLI 路径 (`E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\node_modules\@tauri-apps\cli\tauri.js`)。
    *   **Fallback**: Windows 下尝试 `.exe`, `.cmd`, `.bat` 后缀。

**文件**: `android/src-tauri/gen/android/settings.gradle`
*   **Includes**:
    *   手写插件: `tauri-plugin-campus-network-bind`, `tauri-plugin-campus-keystore`, `tauri-plugin-campus-monitor-service`。
    *   官方插件: `tauri-plugin-biometric`, `tauri-plugin-opener` (硬编码 Cargo registry 路径)。
    *   **注释**: 说明此处声明是为了防止 CLI 重生成覆盖导致 `ClassNotFoundException`。

#### 3.2 Kotlin 胶水代码 (Generated)

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/WryActivity.kt`
*   **Class `WryActivity`**:
    *   **Field `id`**: Activity 唯一标识。
    *   **Method `setWebView()`**: 注册 BackPress 回调，处理 WebView 返回键逻辑。
    *   **Method `onCreate()`:** 注册 `WryLifecycleObserver`，调用 `Rust.onActivityCreate()`。
    *   **Method `onDestroy()`:** 调用 `Rust.onActivityDestroy()` 和 `Rust.onWebviewDestroy()`。
    *   **Property `version`**: 获取当前 WebView 版本名称。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/TauriActivity.kt`
*   **Object `TauriLifecycleObserver`**: 监听 `onResume`, `onPause`, `onStop`，转发给 `PluginManager`。
*   **Class `TauriActivity`**: 继承 `WryActivity`，重写 `handleBackNavigation = false`。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt`
*   **Class `MainActivity`**: 继承 `TauriActivity`。
*   **Comment**: 明确禁止设置 renderer 优先级策略，以防止 Chromium SIGKILL 宿主进程。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Rust.kt`
*   **Object `Rust`**:
    *   **Static Init**: `System.loadLibrary("campus_login_android_lib")`。
    *   **External Functions**:
        *   Lifecycle: `onActivityCreate`, `onActivityDestroy`, `onNewIntent`, `onWindowFocusChanged`, `onActivityLowMemory`.
        *   Core: `create`, `start`, `resume`, `pause`, `stop`, `wryCreate`.
        *   WebView: `ipc`, `handleRequest`, `shouldOverride`, `onPageLoading`, `onPageLoaded`, `onEval`, `handleReceivedTitle`.
        *   Asset: `assetLoaderDomain`, `withAssetLoader`.

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebView.kt`
*   **Class `RustWebView`**:
    *   **Init**: 启用 JS, DOM Storage, Geolocation, Database。
    *   **Document Start Script**: 若支持 `WebViewFeature.DOCUMENT_START_SCRIPT`，则注入初始化脚本。
    *   **Method `loadUrl()`**: 拦截并调用 `Rust.shouldOverride()`。
    *   **Method `evalScript()`**: 异步执行 JS 并通过 `Rust.onEval()` 回传结果。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebViewClient.kt`
*   **Class `RustWebViewClient`**:
    *   **Field `currentUrl`**: 跟踪当前 URL。
    *   **Method `shouldInterceptRequest()`**:
        *   若启用 Asset Loader，使用 `WebViewAssetLoader`。
        *   否则调用 `Rust.handleRequest()`，强制添加 `Cache-Control: no-store`。
    *   **Method `onPageStarted()`**: 注入 init scripts (若非拦截请求)。
    *   **Method `onReceivedError()`**: 处理 `ERR_CONNECTION_REFUSED` 导致的重定向重试。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebChromeClient.kt`
*   **Class `RustWebChromeClient`**:
    *   **Permission Handling**: 使用 `ActivityResultLauncher` 处理相机/录音权限。
    *   **JS Dialogs**: 实现 `onJsAlert`, `onJsConfirm`, `onJsPrompt`。
    *   **FileChooser**: 支持媒体拍摄和图片选择。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Ipc.kt`
*   **Class `Ipc`**:
    *   **Method `postMessage()`**: JavaScript 接口，调用 `Rust.ipc(webViewId, url, message)`。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Logger.kt`
*   **Class `Logger`**: 封装 `android.util.Log`，TAG 为 `Tauri`。仅在 `BuildConfig.DEBUG` 时输出。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/PermissionHelper.kt`
*   **Object `PermissionHelper`**:
    *   **Method `hasPermissions()`**: 检查运行时权限。
    *   **Method `hasDefinedPermission()`**: 检查 Manifest 中是否声明。

#### 3.3 AndroidManifest & Resources

**文件**: `android/src-tauri/gen/android/app/src/main/AndroidManifest.xml`
*   **Permissions**:
    *   `INTERNET`, `POST_NOTIFICATIONS`, `FOREGROUND_SERVICE`, `RECEIVE_BOOT_COMPLETED`, `WAKE_LOCK`, `REQUEST_INSTALL_PACKAGES`, `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`, `CAMERA`, `WRITE_SECURE_SETTINGS` (Protected, 需 adb grant)。
*   **Activities**:
    *   `MainActivity`: `singleTask`, `exported=true`。
*   **Services**:
    *   `ForegroundService`: `specialUse`, subtype `campus-network-monitor-keepalive`。
*   **Receivers**:
    *   `BootReceiver`: `enabled=false` (默认禁用，由设置页动态开启)。
*   **Providers**:
    *   `FileProvider`: `authorities="${applicationId}.fileprovider"`。

**文件**: `android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml`
*   **Paths**:
    *   `<external-files-path name="my_images" path="Pictures/" />`: 相机拍照。
    *   `<files-path name="update" path="update/" />`: APK 更新安装。
    *   **Note**: 移除了原 `external-path/cache-path` 的全局暴露。

**文件**: `android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json`
*   **Content**: 复制自根目录 `tauri.conf.json`，包含前端距离 `../frontend/dist` 和 devUrl `http://localhost:5174/`。

### 4. ProGuard 规则

**文件**: `android/src-tauri/gen/android/app/proguard-tauri.pro`
*   保留 `TauriActivity.getPluginManager()`。

**文件**: `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/proguard-wry.pro`
*   保留 `RustWebView`, `RustWebChromeClient`, `Ipc`, `WryActivity` 的 native 方法和关键方法。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                      Tauri Android Project                          │
├─────────────────────────────────────────────────────────────────────┤
│  [Rust Layer]                                                       │
│  src-tauri/Cargo.toml ──► campus-login (Shared Logic)               │
│           │                                                         │
│           ▼                                                         │
│  src-tauri/build.rs ──► tauri_build::build()                        │
├─────────────────────────────────────────────────────────────────────┤
│  [Configuration]                                                    │
│  tauri.conf.json ──► App ID, Window, CSP, Bundle                    │
│  capabilities/default.json ──► Permissions (Biometric, Opener...)   │
├─────────────────────────────────────────────────────────────────────┤
│  [Android Native Layer (Gen)]                                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  build.gradle.kts / settings.gradle                           │  │
│  │  ├── RustPlugin (ABI Flavors)                                  │  │
│  │  ├── BuildTask (Node CLI Invocation)                           │  │
│  │  └── Dependencies (Plugins + Tauri Android SDK)                │  │
│  └───────────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  Kotlin Activities & WebViews                                 │  │
│  │  MainActivity ──► TauriActivity ──► WryActivity               │  │
│  │       │              │                  │                      │  │
│  │       │              ▼                  ▼                      │  │
│  │       │         PluginManager      RustWebView                  │  │
│  │       │                              │                          │  │
│  │       └──────────────► Rust (JNI) ◄──┘                          │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Build Time**:
    *   `RustPlugin` 触发 `BuildTask`。
    *   `BuildTask` 调用 Node.js CLI (`@tauri-apps/cli`) 生成 Android 原生代码和资源。
    *   CLI 读取 `tauri.conf.json` 和 `capabilities`，生成 `gen/` 目录下的所有文件。

2.  **Runtime - Initialization**:
    *   `MainActivity.onCreate()` 启动。
    *   `WryActivity.onCreate()` 注册 `WryLifecycleObserver` 并调用 `Rust.onActivityCreate()`。
    *   `Rust` 对象静态块加载 `libcampus_login_android_lib.so`。

3.  **Runtime - WebView Interaction**:
    *   HTML/JS 通过 `window.__TAURI__` 或 IPC 发送消息。
    *   `Ipc.postMessage()` 被调用。
    *   `Rust.ipc()` JNI 函数接收消息。
    *   Rust 侧处理业务逻辑，可能调用 Plugin API (如 Biometric, Notification)。
    *   结果通过 `RustWebView.evalScript()` 回调或事件通道返回前端。

4.  **Runtime - Background Service**:
    *   `ForegroundService` 运行 Rust 循环。
    *   持有 `WifiLock` 和 `WakeLock`。
    *   通过 `Notification` 插件监控前台通知。

## Connections

*   [[rust-tauri-core]]: 共享核心业务逻辑 (`campus-login` crate)。
*   [[android-plugins]]: 自定义插件 (`network-bind`, `keystore`, `foreground-service`) 的实现细节。
*   [[frontend-app]]: 前端资源 (`../frontend/dist`) 和 JS API 交互。