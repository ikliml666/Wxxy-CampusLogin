---
title: android-gen-build-system
type: module
source_files:
  - android/src-tauri/gen/android/.editorconfig
  - android/src-tauri/gen/android/.gitignore
  - android/src-tauri/gen/android/app/.gitignore
  - android/src-tauri/gen/android/app/build.gradle.kts
  - android/src-tauri/gen/android/app/proguard-rules.pro
  - android/src-tauri/gen/android/app/proguard-tauri.pro
  - android/src-tauri/gen/android/app/tauri.build.gradle.kts
  - android/src-tauri/gen/android/app/tauri.properties
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
tags: [Tauri, Android, Gradle, Rust, WebView, Plugin]
---

## Overview
本模块包含 Tauri CLI 生成的 Android 原生工程代码、Gradle 构建脚本、ProGuard 混淆规则及资源文件。它负责将 Rust 核心逻辑编译为 `.so` 动态库，并桥接至 Android 平台的 `WebView` 中运行前端 UI。该模块还处理了自定义插件的集成、系统权限管理以及后台保活服务的配置。

## Key Components

### Build Configuration & Scripts

#### `android/src-tauri/gen/android/app/build.gradle.kts`
- **功能**: 定义应用构建配置、依赖项、签名策略及产物命名。
- **关键配置**:
    - **Namespace/ID**: `com.campuslogin.client` (L10-L11)。
    - **SDK Versions**: Compile/Target SDK 36, Min SDK 29 (L9, L15-L16)。
    - **Signing**: Release 模式默认使用本机 debug keystore (`~/.android/debug.keystore`)，密码均为 `android` (L22-L27)。注释指出此证书不可更换，否则需用户重装。
    - **Output Naming**: 产物命名为 `Wxxy-CampusLogin_<version>.apk` (L30-L33)。
    - **Dependencies**:
        - 官方依赖: `androidx.webkit`, `appcompat`, `activity-ktx`, `material`, `lifecycle-process` (L48-L52)。
        - 手写插件依赖: `tauri-plugin-campus-network-bind`, `tauri-plugin-campus-keystore`, `tauri-plugin-campus-monitor-service`, `tauri-plugin-biometric`, `tauri-plugin-opener` (L54-L58)。这些在 `settings.gradle` 中持久声明以弥补 CLI 生成遗漏。
    - **Rust Plugin**: 应用 `rust` 插件，根目录相对路径为 `../../../` (L45-L46)。

#### `android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt`
- **类**: `RustPlugin` (L10-L76)
    - **方法**:
        - `apply(project: Project)` (L12): 注册扩展 `rust`，配置 ABI 维度 (universal, arm64, arm, x86, x86_64)，创建对应的 `rustBuild` Task。
    - **常量**: `TASK_GROUP = "rust"` (L9)。

#### `android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt`
- **类**: `BuildTask` (L9-L72)
    - **属性**:
        - `rootDirRel`: String? (L11)
        - `target`: String? (L13)
        - `release`: Boolean? (L15)
    - **方法**:
        - `assemble()` (L18): 执行 Node.js 调用 Tauri CLI。硬编码了 Node 路径 `C:\Program Files\nodejs\node.exe` (L20) 和 Tauri CLI 路径 `E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\node_modules\@tauri-apps\cli\tauri.js` (L52)。包含 Windows 下的 fallback 逻辑 (L22-L35)。
        - `runTauriCli(executable: String)` (L39): 构造参数列表 `[tauri.js, android, android-studio-script, --target, <target>]` 并执行 (L41-L68)。

#### `android/src-tauri/gen/android/settings.gradle`
- **功能**: 项目设置与插件路径声明。
- **持久化插件**: 手动 include 了 `tauri-plugin-campus-*` 系列插件 (L6-L23)，路径指向本地开发环境或 Cargo registry。
- **自动引入**: `apply from: 'tauri.settings.gradle'` (L25)。

#### `android/src-tauri/gen/android/tauri.settings.gradle`
- **功能**: Tauri CLI 自动生成的插件依赖声明。
- **内容**: 包含 `tauri-android` 及所有已启用插件的路径映射 (L1-L16)。

#### `android/src-tauri/gen/android/app/tauri.properties`
- **内容**: 存储版本信息 `tauri.android.versionName=2.4.1`, `tauri.android.versionCode=2004001` (L1-L3)。

### Core Android Components

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt`
- **类**: `MainActivity` (L8-L13)
    - **继承**: `TauriActivity`
    - **方法**:
        - `onCreate(savedInstanceState: Bundle?)` (L10): 调用 `enableEdgeToEdge()` 并启动父类。
    - **注释说明**: 故意不设 renderer 优先级策略，避免后台渲染进程被杀导致白屏 (L5-L7)。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/TauriActivity.kt`
- **对象**: `TauriLifecycleObserver` (L10-L21)
    - **方法**: 实现 `DefaultLifecycleObserver`，转发 `onResume`, `onPause`, `onStop` 到 `PluginManager` (L12-L20)。
- **抽象类**: `TauriActivity` (L23-L52)
    - **继承**: `WryActivity`
    - **属性**: `handleBackNavigation: Boolean = false` (L25)
    - **方法**:
        - `getPluginManager()`: 返回全局 `PluginManager` (L30)。
        - 生命周期回调 (`onCreate`, `onNewIntent`, `onRestart`, `onDestroy`, `onConfigurationChanged`) 均转发给 `PluginManager` (L32-L51)。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/WryActivity.kt`
- **对象**: `WryLifecycleObserver` (L14-L33)
    - **方法**: 监听应用级生命周期，调用 `Rust.create()`, `Rust.start()`, `Rust.resume()` 等静态方法 (L16-L32)。
- **抽象类**: `WryActivity` (L36-L136)
    - **属性**:
        - `mWebView`: `RustWebView` (L38)
        - `id`: `Int` (L39)
        - `handleBackNavigation`: `Boolean` (L40)
    - **方法**:
        - `setWebView(webView: RustWebView)` (L43): 绑定 WebView，注册 BackPress 回调。若 `handleBackNavigation` 为 true，则优先返回上一页，否则退出 Activity (L45-L56)。
        - `version`: String (Getter, L59-L88): 获取当前 WebView 版本名。
        - `onCreate(savedInstanceState: Bundle?)` (L90): 初始化 ID，注册 `WryLifecycleObserver`，调用 `Rust.onActivityCreate`。
        - `onSaveInstanceState(outState: Bundle)` (L106): 保存 ID。
        - `onDestroy()` (L118): 调用 `Rust.onActivityDestroy` 和 `Rust.onWebviewDestroy`。
        - `startActivity(cls: Class<*>): Int` (L130): 启动新 Activity 并返回 ID。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Rust.kt`
- **对象**: `Rust` (L10-L45)
    - **Init**: 加载 native library `"campus_login_android_lib"` (L12)。
    - **Native Methods** (L15-L44):
        - Activity 生命周期: `onActivityCreate`, `onActivityDestroy`, `onActivitySaveInstanceState`, `onActivityLowMemory`, `onWindowFocusChanged`, `onNewIntent`。
        - App 状态: `create`, `start`, `resume`, `pause`, `stop`。
        - WebView 交互: `wryCreate`, `onWebviewDestroy`, `ipc`, `assetLoaderDomain`, `withAssetLoader`, `handleRequest`, `shouldOverride`, `onPageLoading`, `onPageLoaded`, `onEval`, `handleReceivedTitle`。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebView.kt`
- **类**: `RustWebView` (L14-L85)
    - **继承**: `WebView`
    - **属性**:
        - `initScripts`: `Array<String>` (L14)
        - `id`: `String` (L14)
        - `isDocumentStartScriptEnabled`: `Boolean` (L16)
    - **Init**: 启用 JS、DOM Storage、Geolocation、Database。若支持 `DOCUMENT_START_SCRIPT`，则注入初始脚本 (L19-L30)。
    - **方法**:
        - `loadUrlMainThread(url: String)` (L33): 在主线程加载 URL。
        - `loadHTMLMainThread(html: String)` (L45): 在主线程加载 HTML。
        - `evalScript(id: Int, script: String)` (L50): 执行 JS 并通过 `Rust.onEval` 回传结果。
        - `clearAllBrowsingData()` (L57): 清除缓存、数据库、历史。
        - `getCookies(url: String): String` (L68): 获取 Cookie。
        - `loadUrl(url: String)` / `loadUrl(url: String, headers: Map)` (L37-L43): 覆写，先通过 `Rust.shouldOverride` 检查是否拦截。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebViewClient.kt`
- **类**: `RustWebViewClient` (L12-L98)
    - **继承**: `WebViewClient`
    - **属性**:
        - `interceptedState`: `MutableMap<String, Boolean>` (L14)
        - `currentUrl`: `String` (L15)
        - `lastInterceptedUrl`: `Uri?` (L16)
        - `pendingUrlRedirect`: `String?` (L17)
        - `assetLoader`: `WebViewAssetLoader` (L19-L22): 基于 `Rust.assetLoaderDomain` 构建。
    - **方法**:
        - `shouldInterceptRequest(...)` (L24): 处理请求拦截。若启用 AssetLoader 则使用之；否则调用 `Rust.handleRequest`。强制添加 `Cache-Control: no-store` (L36-L39)。
        - `shouldOverrideUrlLoading(...)` (L51): 委托给 `Rust.shouldOverride`。
        - `onPageStarted(...)` (L56): 更新 `currentUrl`，若非拦截页面则注入 `initScripts`，调用 `Rust.onPageLoading`。
        - `onPageFinished(...)` (L63): 调用 `Rust.onPageLoaded`。
        - `onReceivedError(...)` (L67): 处理连接拒绝错误，尝试重定向加载以防 OAuth 流程中断 (L70-L78)。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebChromeClient.kt`
- **类**: `RustWebChromeClient` (L16-L302)
    - **继承**: `WebChromeClient`
    - **内部接口**: `PermissionListener`, `ActivityResultListener` (L18-L22)。
    - **属性**:
        - `activity`: `WryActivity` (L26)
        - `permissionLauncher`: `ActivityResultLauncher<Array<String>>` (L28)
        - `activityLauncher`: `ActivityResultLauncher<Intent>` (L29)
    - **方法**:
        - `onPermissionRequest(request: PermissionRequest)` (L65): 处理 WebView 权限请求（相机、麦克风），动态申请 Android 权限 (L67-L86)。
        - `onJsAlert/Confirm/Prompt(...)`: 显示原生 AlertDialog (L91-L163)。
        - `onGeolocationPermissionsShowPrompt(...)`: 处理定位权限请求 (L168-L191)。
        - `onShowFileChooser(...)`: 处理文件选择，区分拍照/录像和普通文件选择 (L196-L213)。
        - `showMediaCaptureOrFilePicker(...)`: 调用系统相机/录像 Intent (L220-L239)。
        - `showImageCapturePicker(...)`: 拍照 Intent，使用 FileProvider 获取 URI (L241-L260)。
        - `showVideoCapturePicker(...)`: 录像 Intent (L262-L276)。
        - `showFilePicker(...)`: 普通文件选择 Intent (L278-L300)。
        - `onConsoleMessage(consoleMessage: ConsoleMessage)`: 将 Web Console 消息转发至 Android Logger (L302-L321)。
        - `onReceivedTitle(...)`: 调用 `Rust.handleReceivedTitle` (L323-L327)。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Ipc.kt`
- **类**: `Ipc` (L10-L22)
    - **继承**: 无
    - **属性**: `webView: RustWebView`, `webViewClient: RustWebViewClient` (L12)。
    - **方法**:
        - `@JavascriptInterface fun postMessage(message: String?)` (L14): 接收 JS 消息，调用 `Rust.ipc(webView.id, webViewClient.currentUrl, m)` (L18)。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Logger.kt`
- **类**: `Logger` (L10-L95)
    - **伴生对象**:
        - `LOG_TAG_CORE = "Tauri"` (L13)
        - `tags(vararg subtags: String)`: 组合 Tag (L15-L19)
        - `verbose/debug/info/warn/error(...)`: 日志输出方法，仅在 `BuildConfig.DEBUG` 时打印 (L21-L93)。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/PermissionHelper.kt`
- **对象**: `PermissionHelper` (L10-L103)
    - **方法**:
        - `hasPermissions(context, permissions)`: 检查权限是否授予 (L18-L28)。
        - `hasDefinedPermission(context, permission)`: 检查 Manifest 是否声明权限 (L31-L43)。
        - `hasDefinedPermissions(context, permissions)`: 检查所有权限是否声明 (L46-L53)。
        - `getManifestPermissions(context)`: 读取 Manifest 中的权限数组 (L56-L70)。
        - `getUndefinedPermissions(context, neededPermissions)`: 找出未声明的权限 (L73-L92)。

### Resources & Assets

#### `android/src-tauri/gen/android/app/src/main/AndroidManifest.xml`
- **Permissions**:
    - `INTERNET`, `POST_NOTIFICATIONS`, `FOREGROUND_SERVICE`, `RECEIVE_BOOT_COMPLETED`, `WAKE_LOCK` (L4-L10)。
    - `REQUEST_INSTALL_PACKAGES`: APK 安装 (L13)。
    - `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`: 电池优化白名单 (L18)。
    - `CAMERA`: 人脸验证 (L21)。
    - `WRITE_SECURE_SETTINGS`: 夜间出站切换，需 `pm grant` 授予 (L26-L28)。
- **Application**:
    - `usesCleartextTraffic="${usesCleartextTraffic}"`: 由 build config 注入 (L31)。
- **Activities**:
    - `MainActivity`: Launcher, Leanback (L34-L43)。
- **Services**:
    - `ForegroundService`: `foregroundServiceType="specialUse"`, subtype `campus-network-monitor-keepalive` (L48-L52)。
- **Receivers**:
    - `BootReceiver`: Enabled=false, Exported=false (L57-L62)。
- **Providers**:
    - `FileProvider`: Authorities `${applicationId}.fileprovider`, Resource `@xml/file_paths` (L65-L72)。

#### `android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml`
- **Paths**:
    - `<external-files-path name="my_images" path="Pictures/" />`: 相机图片 (L10)。
    - `<files-path name="update" path="update/" />`: 更新 APK (L11)。

#### `android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json`
- **配置**:
    - `productName`: "CampusLogin", `version`: "2.4.1", `identifier`: "com.campuslogin.client" (L1)。
    - `app.windows`: 主窗口配置，宽度 400，高度 800 (L1)。
    - `app.security.csp`: 严格 CSP，允许 `self`, `ipc:`, `http://ipc.localhost`, `http://asset.localhost` (L1)。
    - `bundle.android.minSdkVersion`: 29 (L1)。

#### `android/src-tauri/gen/android/app/tauri.build.gradle.kts`
- **功能**: 自动生成的额外依赖声明。
- **依赖**: `tauri-android`, `tauri-plugin-biometric`, `tauri-plugin-campus-*`, `tauri-plugin-notification`, `tauri-plugin-opener` (L1-L11)。

### ProGuard Rules

#### `android/src-tauri/gen/android/app/proguard-tauri.pro`
- **规则**: 保留 `TauriActivity` 的 `getPluginManager` 方法 (L1-L5)。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/proguard-wry.pro`
- **规则**:
    - 保留 `com.campuslogin.client.*` 的 native 方法 (L1-L5)。
    - 保留 `WryActivity` 的关键方法和构造函数 (L7-L15)。
    - 保留 `Ipc` 类的 `@JavascriptInterface` 方法 (L17-L22)。
    - 保留 `RustWebView` 的关键方法 (L24-L30)。
    - 保留 `RustWebChromeClient` 和 `RustWebViewClient` 的构造函数 (L32-L34)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────────────┐
│                        Android Native Layer                         │
│                                                                      │
│  ┌─────────────────┐   ┌──────────────────┐   ┌──────────────────┐  │
│  │ MainActivity.kt │──▶│ TauriActivity.kt│──▶│ WryActivity.kt   │  │
│  │ (UI Entry)      │   │ (Plugin Manager) │   │ (WebView Bridge) │  │
│  └─────────────────┘   └──────────────────┘   └────────┬─────────┘  │
│                                                        │             │
│  ┌─────────────────┐   ┌──────────────────┐            │             │
│  │ ForegroundService│   │ BootReceiver     │◄───────────┘             │
│  │ (Monitor Keep)  │   │ (Auto Start)     │                          │
│  └─────────────────┘   └──────────────────┘                          │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                      RustBridge (Rust.kt)                      │  │
│  │  ◄──────────────────────────────────────────────────────────►  │  │
│  │  Calls Native Library ("campus_login_android_lib")             │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                     WebView Core                               │  │
│  │  ┌──────────────┐  ┌───────────────┐  ┌───────────────────┐   │  │
│  │  │ RustWebView  │  │WebViewClient  │  │ WebChromeClient   │   │  │
│  │  │ (JS/URL Mgmt)│  │ (Request Inter)│  │ (UI/Perm/File)    │   │  │
│  │  └──────┬───────┘  └───────┬───────┘  └────────┬──────────┘   │  │
│  └─────────┼──────────────────┼────────────────────┼─────────────┘  │
│            │                  │                    │                │
│            ▼                  ▼                    ▼                │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                   Frontend (index.html)                        │  │
│  │         via WebViewAssetLoader / IPC                           │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Initialization**:
    - `MainActivity.onCreate` calls `super.onCreate`.
    - `TauriActivity` and `WryActivity` lifecycle observers are registered.
    - `Rust.create()` and `Rust.wryCreate()` are called via `WryLifecycleObserver` upon app start.
    - `Rust.onActivityCreate(this)` is called in `WryActivity.onCreate`.

2.  **WebView Loading**:
    - `RustWebView.loadUrl` is invoked.
    - `RustWebViewClient.shouldInterceptRequest` checks if the request should be intercepted.
    - If intercepted, `Rust.handleRequest` is called to fetch resources from the Rust side or asset loader.
    - On page load completion, `Rust.onPageLoaded` is triggered.

3.  **IPC Communication**:
    - JavaScript calls `window.Tauri.postMessage`.
    - `Ipc.postMessage` receives the message.
    - `Rust.ipc(webViewId, url, message)` bridges the data to the Rust backend.

4.  **Background Service**:
    - `ForegroundService` runs a monitor loop.
    - It holds a WakeLock and WifiLock.
    - `BootReceiver` can enable the service on boot if configured.

## Connections

- [[frontend-ui]]: The web frontend loaded inside `RustWebView`.
- [[rust-core]]: The Rust backend accessed via JNI in `Rust.kt`.
- [[plugin-system]]: Plugins like `biometric`, `network-bind` integrated via `settings.gradle` and `build.gradle.kts`.