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
本模块包含 Tauri CLI 生成的 Android 原生工程代码、Gradle 构建脚本、ProGuard 混淆规则及资源文件（位于 `android/src-tauri/gen/android/` 下）。它负责将 Rust 核心逻辑编译为 `.so` 动态库（`Rust.kt` 行 17-19 加载 `campus_login_android_lib`），并桥接至 Android 平台 `WebView` 运行前端 UI。模块处理了自定义插件集成（`settings.gradle` 行 9-19 手动 include 手写插件）、系统权限管理（`AndroidManifest.xml` 行 3-22）、后台保活服务（`ForegroundService` 行 47-54）以及构建产物命名（`app/build.gradle.kts` 行 38-44）。

## Key Components

### Build Configuration & Scripts

#### `android/src-tauri/gen/android/app/build.gradle.kts`
- **功能**: 定义应用构建配置、依赖项、签名策略及产物命名（行 1-97）。
- **插件声明**: `com.android.application`, `org.jetbrains.kotlin.android`, `rust`（行 3-7）。
- **tauriProperties 加载**: 从 `tauri.properties` 读版本（行 9-14）。
- **android 块**（行 16-73）:
  - `compileSdk = 36`（行 17），`namespace = "com.campuslogin.client"`（行 18）。
  - `defaultConfig`: `manifestPlaceholders["usesCleartextTraffic"] = "false"`（行 20），`applicationId = "com.campuslogin.client"`（行 21），`minSdk = 29`（行 22），`targetSdk = 36`（行 23），`versionCode`/`versionName` 从属性读（行 24-25）。
  - `signingConfigs` release 使用本机 debug.keystore（行 29-36，storeFile 行 31，密码 android 行 32-34）。
  - `applicationVariants` 产物命名 `Wxxy-CampusLogin_${variantVersionName}.apk`（行 38-44）。
  - `buildTypes`: debug（行 46-56）开启 debuggable 并保留 jni 调试符号；release（行 57-65）启用 minify 并配置 proguardFiles。
  - `kotlinOptions.jvmTarget = "1.8"`（行 67-69）；`buildFeatures.buildConfig = true`（行 70-72）。
- **rust 块**: `rootDirRel = "../../../"`（行 75-77）。
- **dependencies**（行 79-95）: androidx 库（行 80-84），手写/官方插件 project 依赖（行 87-91），test 依赖（行 92-94）。
- **apply from**: `tauri.build.gradle.kts`（行 97）。

#### `android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt`
- `const val TASK_GROUP = "rust"`（行 8）。
- `open class Config`（行 10-12）: `lateinit var rootDirRel: String`。
- `open class RustPlugin : Plugin<Project>`（行 14-85）:
  - `apply(project: Project)`（行 17-84）: 创建 `rust` 扩展；默认 abiList（行 20-21）、archList（行 23-24）、targetsList（行 26）；配置 `ApplicationExtension` 的 flavorDimensions "abi" 及 productFlavors universal/arm64/arm/x86/x86_64（行 28-47）；`afterEvaluate` 创建 `rustBuildUniversal*` 与每架构 `rustBuild*` Task（行 49-83）。

#### `android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt`
- `open class BuildTask : DefaultTask()`（行 9-69）:
  - `@Input var rootDirRel: String?`（行 10-11），`var target: String?`（行 12-13），`var release: Boolean?`（行 14-15）。
  - `@TaskAction fun assemble()`（行 17-45）: 硬编码 Node 路径 `C:\Program Files\nodejs\node.exe`（行 19），调用 `runTauriCli`，Windows 下 fallback 扩展名（行 22-44）。
  - `fun runTauriCli(executable: String)`（行 47-69）: 硬编码 tauri.js 路径 `E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\node_modules\@tauri-apps\cli\tauri.js`（行 52），参数 `[tauriJs, "android", "android-studio-script"]`（行 53），通过 `project.exec` 执行（行 55-68）。

#### `android/src-tauri/gen/android/settings.gradle`
- `include ':app'`（行 1）。
- 手动持久 include 手写插件及 biometric/opener（行 9-19），路径指向本地或 Cargo registry（如 `../../../plugins/network-bind/android` 行 10，`C:/Users/ik/.cargo/...` 行 16,19）。
- `apply from: 'tauri.settings.gradle'`（行 21）。

#### `android/src-tauri/gen/android/tauri.settings.gradle`
- 自动生成文件（行 1-15），声明 `tauri-android` 及各插件路径（含硬编码 registry 路径）。

#### `android/src-tauri/gen/android/app/tauri.properties`
- 自动生成（行 1 注释），`tauri.android.versionName=2.4.1`（行 2），`tauri.android.versionCode=2004001`（行 3）。

#### `android/src-tauri/gen/android/build.gradle.kts`（根 android）
- `buildscript` 依赖 gradle 8.11.0、kotlin 1.9.25（行 1-10）。
- `allprojects` 仓库（行 12-17）。
- `tasks.register("clean")`（行 19-21）。

#### `android/src-tauri/gen/android/buildSrc/build.gradle.kts`
- `kotlin-dsl` 插件（行 1-3），`gradlePlugin` 注册 id "rust" 对应 RustPlugin（行 5-12），依赖 gradle 8.11.0（行 19-22）。

#### `android/src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties`
- `distributionUrl=https\://services.gradle.org/distributions/gradle-8.14.3-bin.zip`（行 3）。

#### `android/src-tauri/gen/android/gradle.properties`
- JVM 参数 `-Xmx4096m -Dfile.encoding=UTF-8`（行 9），并行与缓存开启（行 13-14），`android.useAndroidX=true`（行 18），`kotlin.code.style=official`（行 20），`android.nonTransitiveRClass=true`（行 24），`android.nonFinalResIds=false`（行 25）。

#### `android/src-tauri/gen/android/local.properties`
- `sdk.dir=C:/Users/ik/AppData/Local/Android/Sdk`（行 1）。

### Core Android Components

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt`
- `class MainActivity : TauriActivity()`（行 11）:
  - `private var appWebView: WebView? = null`（行 12）。
  - `onCreate(savedInstanceState: Bundle?)`（行 14-17）: `enableEdgeToEdge()` 后 super。
  - `onWebViewCreate(webView: WebView)`（行 19-22）: 保存 `appWebView`。
  - `onStart()`（行 28-31）: `appWebView?.resumeTimers()`。
  - `onStop()`（行 33-36）: `appWebView?.pauseTimers()`。
  - `onTrimMemory(level: Int)`（行 42-48）: 仅当 level >= `TRIM_MEMORY_RUNNING_LOW` 时向前端注入 `window.__onTrimMemory`。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/TauriActivity.kt`
- `object TauriLifecycleObserver : DefaultLifecycleObserver`（行 17-32）: `onResume/onPause/onStop` 转发 PluginManager（行 18-31）。
- `abstract class TauriActivity : WryActivity()`（行 34-65）:
  - `override val handleBackNavigation: Boolean = false`（行 35）。
  - `onCreate`（行 37-40）: `PluginManager.onActivityCreate`。
  - `fun getPluginManager(): PluginManager`（行 42-44）。
  - `onNewIntent`（行 46-49），`onRestart`（行 51-54），`onDestroy`（行 56-59），`onConfigurationChanged`（行 61-64）均转发 PluginManager。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/WryActivity.kt`
- `private val ACTIVITY_ID_KEY = "__wryActivityId"`（行 21）。
- `object WryLifecycleObserver`（行 23-49）: `onCreate` 调用 `Rust.create()`/`Rust.wryCreate()`（行 26-27），`onStart/Resume/Pause/Stop` 对应 Rust 静态方法（行 30-48）。
- `abstract class WryActivity : AppCompatActivity()`（行 51-175）:
  - `private lateinit var mWebView: RustWebView`（行 52），`var id: Int = 0`（行 53），`open val handleBackNavigation: Boolean = true`（行 54）。
  - `open fun onWebViewCreate(webView: WebView)`（行 56）。
  - `fun setWebView(webView: RustWebView)`（行 58-79）: 绑定 WebView 并注册返回键回调。
  - `val version: String` getter（行 81-112）: 获取 WebView 版本。
  - `onCreate`（行 114-119），`onWindowFocusChanged`（行 121-124），`onSaveInstanceState`（行 126-130），`onPause`（行 132-137），`onResume`（行 139-144），`onDestroy`（行 146-150），`onLowMemory`（行 152-155），`onNewIntent`（行 157-160）。
  - `fun getAppClass(name: String): Class<*>`（行 162-164）。
  - `fun startActivity(cls: Class<*>): Int`（行 166-172）: 启动新 Activity 并返回随机 ID。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Rust.kt`
- `object Rust`（行 16-48）:
  - `init { System.loadLibrary("campus_login_android_lib") }`（行 17-19）。
  - 外部函数（行 21-47）: `onActivityCreate`, `onActivityDestroy`, `onActivitySaveInstanceState`, `onActivityLowMemory`, `onWindowFocusChanged`, `onNewIntent`, `create`, `start`, `resume`, `pause`, `stop`, `wryCreate`, `onWebviewDestroy`, `ipc`, `assetLoaderDomain`, `withAssetLoader`, `handleRequest`, `shouldOverride`, `onPageLoading`, `onPageLoaded`, `onEval`, `handleReceivedTitle`。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebView.kt`
- `class RustWebView(context: Context, val initScripts: Array<String>, val id: String) : WebView(context)`（行 19-98）:
  - `val isDocumentStartScriptEnabled: Boolean`（行 20，在 init 中赋值行 22-40）。
  - `init` 块启用 JS、DOM、Geolocation 等（行 22-40）。
  - `fun loadUrlMainThread(url: String)`（行 42-46）及重载（行 48-52）。
  - `override fun loadUrl(url: String)`（行 54-58）与重载（行 60-64）: 经 `Rust.shouldOverride` 检查。
  - `fun loadHTMLMainThread(html: String)`（行 66-70）。
  - `fun evalScript(id: Int, script: String)`（行 72-78）。
  - `fun clearAllBrowsingData()`（行 80-90）。
  - `fun getCookies(url: String): String`（行 92-95）。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebViewClient.kt`
- `class RustWebViewClient(webView: RustWebView, context: Context) : WebViewClient()`（行 17-100）:
  - `private val interceptedState = mutableMapOf<String, Boolean>()`（行 18），`var currentUrl: String = "about:blank"`（行 19），`private var lastInterceptedUrl: Uri? = null`（行 20），`private var pendingUrlRedirect: String? = null`（行 21）。
  - `private val assetLoader = WebViewAssetLoader.Builder()...`（行 23-26）。
  - `shouldInterceptRequest`（行 28-55）: 使用 assetLoader 或 `Rust.handleRequest`，强制 `Cache-Control: no-store`。
  - `shouldOverrideUrlLoading`（行 57-62）。
  - `onPageStarted`（行 64-73）: 注入 initScripts（若未拦截）并调用 `Rust.onPageLoading`。
  - `onPageFinished`（行 75-77）: `Rust.onPageLoaded`。
  - `onReceivedError`（行 79-97）: 处理 CONNECT 错误重试。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/RustWebChromeClient.kt`
- `class RustWebChromeClient(appActivity: WryActivity) : WebChromeClient()`（行 36-493）:
  - 内部接口 `PermissionListener`（行 37-39）、`ActivityResultListener`（行 41-43）。
  - 属性 `activity`（行 45）、`permissionLauncher`（行 46）、`activityLauncher`（行 47）、`permissionListener`（行 48）、`activityListener`（行 49）。
  - `init` 注册 launcher（行 51-72）。
  - `onShowCustomView`/`onHideCustomView`（行 82-85, 92-94）。
  - `onPermissionRequest`（行 96-121）: 处理相机/麦克风权限。
  - `onJsAlert`（行 131-151）、`onJsConfirm`（行 161-187）、`onJsPrompt`（行 198-233）: 原生对话框。
  - `onGeolocationPermissionsShowPrompt`（行 240-272）。
  - `onShowFileChooser`（行 274-304）: 文件/媒体选择。
  - `private val isMediaCaptureSupported`（行 306-311）。
  - `showMediaCaptureOrFilePicker`（行 313-331）、`showImageCapturePicker`（行 333-356）、`showVideoCapturePicker`（行 358-374）、`showFilePicker`（行 376-415）。
  - `getValidTypes`（行 417-436）。
  - `onConsoleMessage`（行 438-459）转发日志。
  - `isValidMsg`（行 461-466）。
  - `createImageFileUri`（行 468-476）、`createImageFile`（行 478-485）。
  - `onReceivedTitle`（行 487-492）调用 `Rust.handleReceivedTitle`。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Ipc.kt`
- `class Ipc(val webView: RustWebView, val webViewClient: RustWebViewClient)`（行 13-25）:
  - `@JavascriptInterface fun postMessage(message: String?)`（行 14-22）: 调用 `Rust.ipc`。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/Logger.kt`
- `class Logger`（行 16-89）伴生对象:
  - `LOG_TAG_CORE = "Tauri"`（行 18）。
  - `tags(vararg subtags: String)`（行 20-24）。
  - `verbose/debug/info/warn/error` 系列（行 26-83），均受 `shouldLog()`（行 85-87，仅 DEBUG）控制。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/PermissionHelper.kt`
- `object PermissionHelper`（行 17-117）:
  - `hasPermissions(context, permissions)`（行 24-35）。
  - `hasDefinedPermission(context, permission)`（行 43-54）。
  - `hasDefinedPermissions(context, permissions)`（行 62-69）。
  - `getManifestPermissions(context)`（行 76-92）。
  - `getUndefinedPermissions(context, neededPermissions)`（行 100-116）。

### Resources & Assets

#### `android/src-tauri/gen/android/app/src/main/AndroidManifest.xml`
- 权限声明（行 3-22）: INTERNET, POST_NOTIFICATIONS, FOREGROUND_SERVICE, FOREGROUND_SERVICE_SPECIAL_USE, RECEIVE_BOOT_COMPLETED, WAKE_LOCK, REQUEST_INSTALL_PACKAGES, REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, CAMERA, WRITE_SECURE_SETTINGS（行 20-22，tools:ignore）。
- Application（行 27-75）: icon, label, theme, `usesCleartextTraffic="${usesCleartextTraffic}"`（行 31）。
- Activity `MainActivity`（行 32-44）: launchMode singleTask, intent-filter MAIN/LAUNCHER/LEANBACK_LAUNCHER。
- Service `ForegroundService`（行 47-54）: `foregroundServiceType="specialUse"` 及 property subtype。
- Receiver `BootReceiver`（行 57-64）: enabled=false。
- Provider `FileProvider`（行 66-74）: authorities `${applicationId}.fileprovider`, `@xml/file_paths`。

#### `android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml`
- `<external-files-path name="my_images" path="Pictures/" />`（行 11），`<files-path name="update" path="update/" />`（行 14-16），收窄共享路径。

#### `android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json`
- 单行 JSON（行 1）: `productName":"CampusLogin"`, `version":"2.4.1"`, `identifier":"com.campuslogin.client"`, 窗口 400x800, CSP 严格, `bundle.android.minSdkVersion":29` 等。

#### `android/src-tauri/gen/android/app/tauri.build.gradle.kts`
- 自动生成依赖（行 1-12）: `tauri-android`, `tauri-plugin-biometric`, `tauri-plugin-campus-*`, `tauri-plugin-notification`, `tauri-plugin-opener`。

### ProGuard Rules

#### `android/src-tauri/gen/android/app/proguard-tauri.pro`
- 行 1-5: 保留 `com.campuslogin.client.TauriActivity.getPluginManager()`。

#### `android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/generated/proguard-wry.pro`
- 行 1-37: 保留 `com.campuslogin.client.*` native 方法（行 7-9），`WryActivity` 构造与关键方法（行 11-19），`Ipc` @JavascriptInterface（行 21-25），`RustWebView` 方法（行 27-33），`RustWebChromeClient`/`RustWebViewClient` 构造（行 35-37）。

#### `android/src-tauri/gen/android/app/proguard-rules.pro`
- 行 1-21: 默认注释模板，无实际规则。

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

1. **Initialization**:
   - `MainActivity.onCreate`（行 14-17）调用 `super.onCreate` → `TauriActivity.onCreate`（行 37-40）→ `WryActivity.onCreate`（行 114-119）注册 `WryLifecycleObserver` 并调 `Rust.onActivityCreate`。
   - 应用启动 `WryLifecycleObserver.onCreate` 调 `Rust.create()`/`Rust.wryCreate()`（行 26-27）。

2. **WebView Loading**:
   - `RustWebView.loadUrl`（行 54-58）经 `Rust.shouldOverride` 检查。
   - `RustWebViewClient.shouldInterceptRequest`（行 28-55）调用 `Rust.handleRequest` 或 assetLoader。
   - 页面完成 `onPageFinished` → `Rust.onPageLoaded`（行 75-77）。

3. **IPC Communication**:
   - JS 调用 `Ipc.postMessage`（行 14-22）→ `Rust.ipc(webView.id, webViewClient.currentUrl, m)`（行 20）。

4. **Background Service**:
   - `AndroidManifest.xml` 声明 `ForegroundService`（行 47-54）与 `BootReceiver`（行 57-64，默认 disabled）。
   - Rust 侧循环持锁，Java 层仅提供常驻通知与锁。

## Connections

- [[frontend-ui]]: The web frontend loaded inside `RustWebView` (see `tauri.conf.json` line 1).
- [[rust-core]]: The Rust backend accessed via JNI in `Rust.kt` (lines 16-48).
- [[plugin-system]]: Plugins like `biometric`, `network-bind` integrated via `settings.gradle` (lines 9-19) and `app/build.gradle.kts` (lines 87-91).

## Known Issues

- **CLI 插件遗漏**: `settings.gradle` 行 3-8 注释指出 Tauri CLI 自动生成的 `tauri.settings.gradle` 会漏掉 path 依赖的手写插件，导致 `ClassNotFoundException`；已在 `settings.gradle` 手动 include 兜底。
- **硬编码本地路径**: `BuildTask.kt` 行 19、52 硬编码 Node 及 tauri.js 路径（`C:\Program Files\nodejs\node.exe`、`E:\ik\...`），`tauri.settings.gradle` 行 3-13 及 `settings.gradle` 行 16、19 硬编码 Cargo registry 绝对路径，换机构建失效。
- **渲染进程策略**: `MainActivity.kt` 行 8-10 注释说明故意不设 renderer 优先级策略，避免 wry 0.55.1 无 `onRenderProcessGone` 处理导致白屏。
- **受限权限**: `AndroidManifest.xml` 行 20-22 声明 `WRITE_SECURE_SETTINGS`，需 `adb pm grant` 才生效，普通安装不授予。
- **签名不可更换**: `app/build.gradle.kts` 行 27-28 注释说明 release 使用 debug.keystore，证书一经发布不可更换。
