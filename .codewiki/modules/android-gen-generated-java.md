---
title: android-gen-generated-java
type: module
source_files:
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
tags: [android, tauri, bridge, webview, generated]
---

## Overview
本模块包含 Tauri CLI 自动生成的 Android Java/Kotlin 桥接代码。其核心职责是建立 Android 原生层（WebView、Activity 生命周期）与 Rust 后端之间的通信桥梁。它处理了 WebView 的配置、URL 拦截、JavaScript 交互、权限请求以及系统生命周期的同步。

所有文件均位于 `com.campuslogin.client` 包下，且标记为自动生成（AUTO-GENERATED），开发者不应直接修改这些文件中的逻辑，而应通过配置或插件扩展功能。

## Key Components

### Rust (Rust.kt)
**用途**：静态入口点，加载 Native Library 并提供所有与 Rust 后端通信的 JNI 外部方法。

- **方法**：
  - `init { System.loadLibrary("campus_login_android_lib") }` (Rust.kt:18-20)：初始化时加载名为 `campus_login_android_lib` 的 native 库。
  - `onActivityCreate(activity: WryActivity)` (Rust.kt:21)：通知 Rust Activity 创建。
  - `onActivityDestroy(activity: WryActivity)` (Rust.kt:22)：通知 Rust Activity 销毁。
  - `onActivitySaveInstanceState()` (Rust.kt:23)：通知 Rust 状态保存。
  - `onActivityLowMemory()` (Rust.kt:24)：通知 Rust 低内存警告。
  - `onWindowFocusChanged(activity: WryActivity, focus: Boolean)` (Rust.kt:25)：通知 Rust 窗口焦点变化。
  - `onNewIntent(intent: Intent)` (Rust.kt:26)：通知 Rust 新 Intent 到达。
  - `create()` (Rust.kt:28)：启动 Tauri 核心实例。
  - `start()` (Rust.kt:29)：启动 Tauri 应用。
  - `resume()` (Rust.kt:30)：恢复 Tauri 应用。
  - `pause()` (Rust.kt:31)：暂停 Tauri 应用。
  - `stop()` (Rust.kt:32)：停止 Tauri 应用。
  - `wryCreate()` (Rust.kt:34)：初始化 Wry WebView 相关资源。
  - `onWebviewDestroy(activity: WryActivity, webviewId: String)` (Rust.kt:35)：销毁特定 WebView ID 的资源。
  - `ipc(webviewId: String, url: String, message: String)` (Rust.kt:37)：**关键方法**。处理来自 JavaScript 的 IPC 消息。
  - `assetLoaderDomain(webviewId: String): String` (Rust.kt:39)：获取资产加载器的域名。
  - `withAssetLoader(webviewId: String): Boolean` (Rust.kt:40)：检查是否启用 AssetLoader。
  - `handleRequest(webviewId: String, request: WebResourceRequest, isDocumentStartScriptEnabled: Boolean): WebResourceResponse?` (Rust.kt:41)：处理网络请求拦截。
  - `shouldOverride(webviewId: String, url: String): Boolean` (Rust.kt:42)：判断是否由 App 处理 URL 加载。
  - `onPageLoading(webviewId: String, url: String)` (Rust.kt:43)：页面开始加载回调。
  - `onPageLoaded(webviewId: String, url: String)` (Rust.kt:44)：页面加载完成回调。
  - `onEval(webviewId: String, id: Int, result: String)` (Rust.kt:45)：接收 JavaScript 执行结果。
  - `handleReceivedTitle(webviewId: String, title: String)` (Rust.kt:47)：接收网页标题更新。

### Ipc (Ipc.kt)
**用途**：暴露给 JavaScript 的全局对象 `window.Tauri` 或类似接口，用于前端调用原生功能。

- **字段**：
  - `webView: RustWebView` (Ipc.kt:13)：关联的 WebView 实例。
  - `webViewClient: RustWebViewClient` (Ipc.kt:13)：关联的 WebViewClient 实例。
- **方法**：
  - `@JavascriptInterface fun postMessage(message: String?)` (Ipc.kt:20-25)：JS 调用此方法发送消息。内部调用 `Rust.ipc` 并将当前 URL 和消息传递给 Rust。注释指出避免使用 `getUrl()` 是因为线程问题，转而依赖 Client 跟踪的 `currentUrl`。

### Logger (Logger.kt)
**用途**：统一的日志记录工具，基于 Android `Log` 类，但增加了 Tag 管理和 DEBUG 模式控制。

- **常量**：
  - `LOG_TAG_CORE = "Tauri"` (Logger.kt:18)：核心日志标签。
- **方法**：
  - `fun tags(vararg subtags: String): String` (Logger.kt:20-24)：组合生成日志标签，如 `Tauri/SubTag`。
  - `verbose/debug/info/warn/error` (Logger.kt:26-83)：各等级的日志输出方法。均受 `shouldLog()` 控制。
  - `private fun shouldLog(): Boolean` (Logger.kt:86-88)：仅在 `BuildConfig.DEBUG` 为 true 时返回 true，发布版不输出日志。

### PermissionHelper (PermissionHelper.kt)
**用途**：辅助检查 Android 运行时权限和 Manifest 中声明的权限。

- **方法**：
  - `fun hasPermissions(context: Context?, permissions: Array<String>): Boolean` (PermissionHelper.kt:24-35)：检查用户是否已授予指定权限列表中的所有权限。
  - `fun hasDefinedPermission(context: Context, permission: String): Boolean` (PermissionHelper.kt:43-54)：检查权限是否在 `AndroidManifest.xml` 中声明。
  - `fun hasDefinedPermissions(context: Context, permissions: Array<String>): Boolean` (PermissionHelper.kt:62-69)：检查所有权限是否均在 Manifest 中声明。
  - `private fun getManifestPermissions(context: Context): Array<String>?` (PermissionHelper.kt:76-92)：从 PackageInfo 中提取 Manifest 声明的权限数组。兼容 Android 13+ (`PackageInfoFlags.of`)。
  - `fun getUndefinedPermissions(context: Context, neededPermissions: Array<String?>): Array<String?>` (PermissionHelper.kt:100-116)：找出所需权限中未在 Manifest 声明的部分。

### RustWebView (RustWebView.kt)
**用途**：自定义 WebView 子类，负责 JS 启用、文档起始脚本注入及基础操作。

- **字段**：
  - `initScripts: Array<String>` (RustWebView.kt:19)：需要在页面加载前执行的 JavaScript 脚本数组。
  - `id: String` (RustWebView.kt:19)：WebView 的唯一标识符，用于 Rust 侧路由。
  - `isDocumentStartScriptEnabled: Boolean` (RustWebView.kt:20)：是否支持文档起始脚本（API 26+）。
- **初始化块 (init)** (RustWebView.kt:22-40)：
  - 启用 JavaScript、DOM Storage、Geolocation、Database。
  - 禁用媒体播放需用户手势 (`mediaPlaybackRequiresUserGesture = false`)。
  - 允许自动打开窗口。
  - 若支持 `WebViewFeature.DOCUMENT_START_SCRIPT`，则通过 `WebViewCompat.addDocumentStartJavaScript` 注入 `initScripts`。
- **方法**：
  - `loadUrlMainThread(url: String)` (RustWebView.kt:42-46)：在主线程加载 URL。
  - `loadUrlMainThread(url: String, additionalHttpHeaders: Map<String, String>)` (RustWebView.kt:48-52)：带 Header 的主线程加载。
  - `override fun loadUrl(url: String)` (RustWebView.kt:54-58)：重载标准加载，先经 `Rust.shouldOverride` 检查。
  - `override fun loadUrl(url: String, additionalHttpHeaders: Map<String, String>)` (RustWebView.kt:60-64)：重载带 Header 的加载，先经 `Rust.shouldOverride` 检查。
  - `loadHTMLMainThread(html: String)` (RustWebView.kt:66-70)：在主线程加载 HTML 字符串。
  - `evalScript(id: Int, script: String)` (RustWebView.kt:72-78)：执行 JS 并异步回调结果至 `Rust.onEval`。
  - `clearAllBrowsingData()` (RustWebView.kt:80-90)：清除缓存、历史、表单数据等。注意：此处错误信息日志仍指向 "media capture file" (RustWebView.kt:80)，疑似复制遗留 bug。
  - `getCookies(url: String): String` (RustWebView.kt:92-95)：获取指定 URL 的 Cookie。

### RustWebViewClient (RustWebViewClient.kt)
**用途**：处理 WebView 导航、请求拦截、错误处理和页面状态回调。

- **字段**：
  - `interceptedState: mutableMapOf<String, Boolean>` (RustWebViewClient.kt:18)：记录 URL 是否被拦截处理。
  - `currentUrl: String` (RustWebViewClient.kt:19)：当前正在加载或已加载的 URL，供 `Ipc` 使用。
  - `lastInterceptedUrl: Uri?` (RustWebViewClient.kt:20)：最后被拦截的 URL。
  - `pendingUrlRedirect: String?` (RustWebViewClient.kt:21)：待重定向的 URL，用于解决竞态条件。
  - `assetLoader: WebViewAssetLoader` (RustWebViewClient.kt:23-26)：用于加载本地 Assets 的加载器。
- **方法**：
  - `override fun shouldInterceptRequest(...)` (RustWebViewClient.kt:41-68)：核心拦截逻辑。若存在 `pendingUrlRedirect` 则执行重定向；否则根据 `Rust.withAssetLoader` 决定使用 AssetLoader 还是 `Rust.handleRequest`。
  - `override fun shouldOverrideUrlLoading(...)` (RustWebViewClient.kt:57-62)：委托给 `Rust.shouldOverride` 决定是否由 App 处理 URL。
  - `override fun onPageStarted(...)` (RustWebViewClient.kt:68-77)：更新 `currentUrl`，若未被拦截则重新注入 `initScripts`，调用 `Rust.onPageLoading`。
  - `override fun onPageFinished(...)` (RustWebViewClient.kt:76-78)：调用 `Rust.onPageLoaded`。
  - `override fun onReceivedError(...)` (RustWebViewClient.kt:87-100)：特殊处理 `ERROR_CONNECT` 且为主框架加载时的竞态问题。通过 `stopLoading` -> `loadUrl` -> 设置 `pendingUrlRedirect` 重试，以触发自定义协议或拦截器。

### RustWebChromeClient (RustWebChromeClient.kt)
**用途**：处理浏览器级 UI 交互（弹窗、权限、文件选择、控制台日志）。

- **字段**：
  - `activity: WryActivity` (RustWebChromeClient.kt:45)：持有的 Activity 引用。
  - `permissionLauncher: ActivityResultLauncher<Array<String>>` (RustWebChromeClient.kt:46)：用于请求多权限。
  - `activityLauncher: ActivityResultLauncher<Intent>` (RustWebChromeClient.kt:47)：用于启动子 Activity（如相机、文件选择器）。
  - `permissionListener: PermissionListener?` (RustWebChromeClient.kt:48)：权限请求回调监听器。
  - `activityListener: ActivityResultListener?` (RustWebChromeClient.kt:49)：Activity 结果回调监听器。
- **方法**：
  - `override fun onShowCustomView(...)` / `onHideCustomView(...)` (RustWebChromeClient.kt:82-94)：全屏视频处理，默认隐藏自定义视图。
  - `override fun onPermissionRequest(request: PermissionRequest)` (RustWebChromeClient.kt:96-121)：处理 WebView 内部的权限请求（摄像头、麦克风）。映射到 Android 运行时权限并请求用户授权。
  - `override fun onJsAlert(...)` (RustWebChromeClient.kt:131-151)：显示 AlertDialog 模拟 JS alert。
  - `override fun onJsConfirm(...)` (RustWebChromeClient.kt:161-187)：显示带 OK/Cancel 的 AlertDialog 模拟 JS confirm。
  - `override fun onJsPrompt(...)` (RustWebChromeClient.kt:198-233)：显示带输入框的 AlertDialog 模拟 JS prompt。
  - `override fun onGeolocationPermissionsShowPrompt(...)` (RustWebChromeClient.kt:240-272)：处理地理位置权限请求。若未授权则请求 `ACCESS_COARSE/FINE_LOCATION`。
  - `override fun onShowFileChooser(...)` (RustWebChromeClient.kt:274-304)：处理文件选择。区分图片/视频捕获和常规文件选择。
  - `private val isMediaCaptureSupported` (RustWebChromeClient.kt:306-311)：检查是否支持媒体捕获（已授权或未在 Manifest 声明）。
  - `private fun showImageCapturePicker(...)` (RustWebChromeClient.kt:333-356)：启动相机拍照。
  - `private fun showVideoCapturePicker(...)` (RustWebChromeClient.kt:358-374)：启动相机录像。
  - `private fun showFilePicker(...)` (RustWebChromeClient.kt:376-415)：启动系统文件选择器，支持多选和 MIME 类型过滤。
  - `override fun onConsoleMessage(...)` (RustWebChromeClient.kt:438-459)：将 JS Console 日志转发到 Android Logger。过滤掉 Tauri 内部调试消息（如 `%cresult %c`）。
  - `override fun onReceivedTitle(...)` (RustWebChromeClient.kt:491-493)：捕获网页标题并通过 `Rust.handleReceivedTitle` 传递。

### WryActivity (WryActivity.kt)
**用途**：所有承载 WebView 的 Activity 的基类。管理 WebView 实例、Back 键处理、生命周期同步及 WebView 版本检测。

- **字段**：
  - `mWebView: RustWebView` (WryActivity.kt:52)：当前的 WebView 实例。
  - `id: Int` (WryActivity.kt:53)：Activity 的唯一 ID，用于 Rust 侧识别。
  - `handleBackNavigation: Boolean` (WryActivity.kt:54)：是否处理物理返回键（默认 true）。
- **方法**：
  - `open fun onWebViewCreate(webView: WebView)` (WryActivity.kt:56)：子类可重载，在 WebView 设置后调用。
  - `fun setWebView(webView: RustWebView)` (WryActivity.kt:58-79)：设置 WebView。若 `handleBackNavigation` 为真，注册 `OnBackPressedCallback` 实现 WebView 内后退逻辑。
  - `val version: String` (WryActivity.kt:92-123)：检测当前设备 WebView 的版本号。尝试多种包名 (`com.google.android.webview`, `com.android.chrome`, `com.android.webview`)。
  - `override fun onCreate(...)` (WryActivity.kt:118-123)：初始化 ID，添加 `WryLifecycleObserver`，调用 `Rust.onActivityCreate`。
  - `override fun onWindowFocusChanged(...)` (WryActivity.kt:121-124)：同步焦点变化到 Rust。
  - `override fun onSaveInstanceState(...)` (WryActivity.kt:126-130)：保存 Activity ID 到 Bundle，通知 Rust。
  - `override fun onPause()` / `onResume()` (WryActivity.kt:132-144)：同步 WebView 的 pause/resume 状态。
  - `override fun onDestroy()` (WryActivity.kt:146-150)：通知 Rust 销毁 Activity 和 WebView。
  - `override fun onLowMemory()` (WryActivity.kt:152-155)：通知 Rust 低内存。
  - `override fun onNewIntent(...)` (WryActivity.kt:157-160)：通知 Rust 新 Intent。
  - `fun getAppClass(name: String): Class<*>` (WryActivity.kt:162-164)：动态加载类。
  - `fun startActivity(cls: Class<*>): Int` (WryActivity.kt:166-172)：启动新的 Activity，生成唯一 ID 并放入 Intent Extras。

### TauriActivity (TauriActivity.kt)
**用途**：继承自 `WryActivity`，集成 `PluginManager`。

- **字段**：
  - `handleBackNavigation: Boolean = false` (TauriActivity.kt:35)：默认关闭原生 Back 键处理，交由插件或 JS 处理。
- **方法**：
  - `override fun onCreate(...)` (TauriActivity.kt:39-42)：调用 `PluginManager.onActivityCreate`。
  - `fun getPluginManager(): PluginManager` (TauriActivity.kt:42-44)：获取单例 PluginManager。
  - `override fun onNewIntent(...)` (TauriActivity.kt:46-49)：通知 PluginManager。
  - `override fun onRestart()` (TauriActivity.kt:51-54)：通知 PluginManager。
  - `override fun onDestroy()` (TauriActivity.kt:56-59)：通知 PluginManager。
  - `override fun onConfigurationChanged(...)` (TauriActivity.kt:61-64)：通知 PluginManager。
- **对象**：
  - `TauriLifecycleObserver` (TauriActivity.kt:17-32)：监听进程级生命周期（ProcessLifecycleOwner），同步 `PluginManager` 的 resume/pause/stop。

### MainActivity (MainActivity.kt)
**用途**：应用的入口 Activity，继承自 `TauriActivity`。

- **字段**：
  - `appWebView: WebView?` (MainActivity.kt:12)：应用主 WebView 实例引用。
- **方法**：
  - `override fun onCreate(...)` (MainActivity.kt:14-17)：调用 `enableEdgeToEdge()` 以支持全面屏/刘海屏边缘绘图，然后调用 `super.onCreate`。
  - `override fun onWebViewCreate(webView: WebView)` (MainActivity.kt:19-22)：缓存 WebView 实例到 `appWebView`。
  - `override fun onStart()` (MainActivity.kt:28-31)：调用 `appWebView?.resumeTimers()` 恢复 JS 定时器。
  - `override fun onStop()` (MainActivity.kt:33-36)：调用 `appWebView?.pauseTimers()` 暂停 JS 定时器以省电。
  - `override fun onTrimMemory(level: Int)` (MainActivity.kt:46-49)：转发系统内存压力至前端钩子，仅当 `level >= TRIM_MEMORY_RUNNING_LOW` 时通过 `evaluateJavascript` 注入 `window.__onTrimMemory`。
  - **注释说明** (MainActivity.kt:8-10)：特意未设置 renderer 优先级策略，因为 wry 0.55.1 缺乏对渲染进程崩溃的处理，降权会导致后台进程被杀从而引发前台白屏。

### ProGuard Rules (proguard-wry.pro)
**用途**：混淆规则，防止关键类和反射方法被移除。

- **保留**：
  - `com.campuslogin.client.*` 的 native 方法 (proguard-wry.pro:7-9)。
  - `WryActivity` 的构造函数、`setWebView`、`getAppClass`、`getId`、`getVersion`、`startActivity` (proguard-wry.pro:11-19)。
  - `Ipc` 的构造函数及带有 `@JavascriptInterface` 注解的方法 (proguard-wry.pro:21-25)。
  - `RustWebView` 的构造函数、`loadUrlMainThread`、`loadHTMLMainThread`、`evalScript` (proguard-wry.pro:27-33)。
  - `RustWebChromeClient` 和 `RustWebViewClient` 的构造函数 (proguard-wry.pro:35-37)。

## Architecture

```text
┌──────────────────────────────────────────────────┐
│ MainActivity (TauriActivity / WryActivity)         │
│ - WryLifecycleObserver (WryActivity.kt:23)         │
│ - setWebView(mWebView) (WryActivity.kt:58)         │
└───────────────────────┬──────────────────────────┘
                        │ contains
                        ▼
┌──────────────────────────────────────────────────┐
│ RustWebView (RustWebView.kt:19)                   │
│  ├─ Ipc (Ipc.kt:13, JS Bridge)                    │
│  ├─ RustWebViewClient (RustWebViewClient.kt:17)   │
│  └─ RustWebChromeClient (RustWebChromeClient.kt:36)│
└───────────────────────┬──────────────────────────┘
                        │ JNI (@JvmStatic)
                        ▼
┌──────────────────────────────────────────────────┐
│ Rust Object (Rust.kt:16)                          │
│  external fun ipc/shouldOverride/handleRequest... │
│  ▼ loads campus_login_android_lib.so              │
└──────────────────────────────────────────────────┘
```

## Data Flow

1.  **IPC Message Flow**:
    *   JS 调用 `window.Tauri.postMessage(msg)`。
    *   `Ipc.postMessage()` (Ipc.kt:15) 获取 `webView.id` 和 `webViewClient.currentUrl`。
    *   调用 `Rust.ipc(id, url, msg)` (Rust.kt:37)。
    *   Rust 后端接收消息并处理业务逻辑。

2.  **URL Navigation Flow**:
    *   `RustWebView.loadUrl()` (RustWebView.kt:54) 被调用。
    *   首先调用 `Rust.shouldOverride(id, url)` (Rust.kt:42) 判断是否由 App 处理。
    *   若否，调用 `super.loadUrl()`。
    *   `RustWebViewClient.shouldInterceptRequest()` (RustWebViewClient.kt:28) 拦截网络请求。
    *   若启用 AssetLoader，使用本地资源；否则调用 `Rust.handleRequest()` (Rust.kt:41) 返回 `WebResourceResponse`。
    *   页面加载过程中，`onPageStarted` (RustWebViewClient.kt:64) 和 `onPageFinished` (RustWebViewClient.kt:75) 分别调用 `Rust.onPageLoading` (Rust.kt:43) 和 `Rust.onPageLoaded` (Rust.kt:44)。

3.  **Lifecycle Synchronization**:
    *   `WryLifecycleObserver` 监听 Process Lifecycle (WryActivity.kt:23)。
    *   `onCreate` 调用 `Rust.create()` (Rust.kt:28) 和 `Rust.wryCreate()` (Rust.kt:34)。
    *   `onStart/onResume/onPause/onStop` 依次调用对应的 `Rust.start` (WryActivity.kt:32) / `Rust.resume` (WryActivity.kt:37) / `Rust.pause` (WryActivity.kt:42) / `Rust.stop` (WryActivity.kt:47)。
    *   `WryActivity` 自身生命周期方法（如 `onPause`）也会同步调用 `mWebView.onPause()` 和 `Rust.onActivity...` 系列方法。

4.  **Permission Request Flow**:
    *   WebView 触发 `onPermissionRequest` (RustWebChromeClient.kt:96) 或 `onGeolocationPermissionsShowPrompt` (RustWebChromeClient.kt:240)。
    *   `RustWebChromeClient` 解析资源类型（Camera/Mic/Geo）。
    *   使用 `permissionLauncher.launch()` 请求 Android 运行时权限。
    *   用户授权后，通过 `PermissionListener` 回调 `request.grant()` 或 `callback.invoke()`。

## Connections
*   [[android-exit-guard-renderer-policy]]：解释了为什么 `MainActivity` 不设置特定的渲染器优先级策略（MainActivity.kt:11-13）。
*   [[tauri-core-rust]]：`Rust` 类中的 JNI 方法（Rust.kt:16）最终连接到 Tauri 的 Rust 核心后端。
*   [[wry-webview-rs]]：`RustWebView` 和 `RustWebViewClient` 的实现细节依赖于 Wry crate 在 Android 上的具体行为。

## Known Issues
*   `RustWebView.clearAllBrowsingData()` (RustWebView.kt:80-90)：在捕获异常时，错误日志输出为 `Logger.error("Unable to create temporary media capture file: " + ex.message)` (RustWebView.kt:88)。这是从媒体捕获逻辑复制粘贴遗留的误导性文本，实际发生的是清除浏览数据失败。
*   `MainActivity` (MainActivity.kt:11-13)：有意不设置 renderer 优先级策略（wry 默认 IMPORTANT 常驻），因为 wry 0.55.1 无 `onRenderProcessGone` 处理，chromium 随即 SIGKILL 静默杀宿主进程（回前台白屏冻结）。
