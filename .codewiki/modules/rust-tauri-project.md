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

本模块 `rust-tauri-project` 定义了基于 Tauri v2 的 Android 客户端项目元数据与构建配置。涵盖 Rust 核心依赖（`android/src-tauri/Cargo.toml`）、构建脚本（`android/src-tauri/build.rs`）、Tauri 配置（`android/src-tauri/tauri.conf.json` 与 `capabilities/default.json`），以及由 Tauri CLI 生成的 Android 原生工程（`android/src-tauri/gen/android/...`）。主要功能：定义应用标识 `com.campuslogin.client`、版本 `2.4.1`；配置 WebView CSP；管理插件依赖（手写网络绑定/密钥库/前台服务 + 官方 biometric/opener/notification）；生成 Gradle 构建文件与 Kotlin 胶水代码（Activity、WebView、JNI 桥接）。该模块复用桌面端 `campus-login` crate（路径依赖 `../../tauri-app/src-tauri`，见 Cargo.toml 第31行）。

## Key Components

### 1. Rust 构建层

**`android/src-tauri/build.rs`**
- `fn main()`（第1-3行）：调用 `tauri_build::build()` 触发 Tauri 构建流程。

**`android/src-tauri/Cargo.toml`**
- `[package]`（第1-5行）：`name = "campus-login-android"`（第2行），`version = "2.4.1"`（第3行），`edition = "2021"`（第5行）。
- `[lib]`（第7-9行）：`name = "campus_login_android_lib"`（第8行），`crate-type = ["cdylib", "rlib"]`（第9行）。
- `[build-dependencies]`（第11-12行）：`tauri-build = { version = "2", features = [] }`（第12行）。
- `[dependencies]`（第14-48行）：
  - `tauri = { version = "2", features = ["protocol-asset"] }`（第15行）。
  - `tauri-plugin-notification = "2"`（第16行）。
  - `campus-login = { path = "../../tauri-app/src-tauri" }`（第31行）。
  - `tauri-plugin-campus-network-bind = { path = "../plugins/network-bind" }`（第34行）。
  - `tauri-plugin-campus-keystore = { path = "../plugins/keystore" }`（第37行）。
  - `tauri-plugin-campus-monitor-service = { path = "../plugins/foreground-service" }`（第40行）。
  - `tauri-plugin-biometric = "2"`（第43行），`tauri-plugin-opener = "2"`（第44行）。
  - `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "stream"] }`（第47行）。
- `[profile.release]`（第55-58行）：`lto = "fat"`（第56行），`codegen-units = 1`（第57行），`strip = true`（第58行）；第53-54行注释明确未设 `panic="abort"` 以保留异步任务 unwind。

### 2. Tauri 配置与权限

**`android/src-tauri/tauri.conf.json`**
- 根字段：`productName`（第3行），`version`（第4行），`identifier`（第5行 `com.campuslogin.client`）。
- `build`（第6-9行）：`frontendDist`（第7行），`devUrl`（第8行）。
- `app.windows`（第11-17行）：单窗口 `title`（第13行）、`width=400`（第14行）、`height=800`（第15行）。
- `app.security`（第18-25行）：`csp`（第19行），`assetProtocol`（第20-23行，启用且 scope `$APPDATA/files/face-models/**`），`dangerousDisableAssetCspModification`（第24行 `["style-src"]`）。
- `bundle.android.minSdkVersion`（第31行）：29。

**`android/src-tauri/capabilities/default.json`**
- `$schema`（第2行），`identifier`（第3行 `default`），`description`（第4行），`windows`（第5-7行 `["main"]`），`permissions`（第8-13行：`core:default`, `biometric:default`, `opener:default`, `notification:default`）。

**`android/src-tauri/gen/schemas/capabilities.json`**（第1行）：编译后 JSON，内容与 default.json 对应。

### 3. Android 原生工程

#### 3.1 Gradle 构建系统

**`android/src-tauri/gen/android/build.gradle.kts`**（第1-22行）：`classpath("com.android.tools.build:gradle:8.11.0")`（第7行），`classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.25")`（第8行）。

**`android/src-tauri/gen/android/app/build.gradle.kts`**
- `android.compileSdk = 36`（第17行），`namespace = "com.campuslogin.client"`（第18行）。
- `defaultConfig`：`minSdk = 29`（第22行），`targetSdk = 36`（第23行），`applicationId`（第21行）。
- `signingConfigs.release`（第29-36行）：使用 `System.getProperty("user.home") + "/.android/debug.keystore"`（第31行），密码 `android`。
- `applicationVariants.all`（第38-44行）：输出文件名 `Wxxy-CampusLogin_${variantVersionName}.apk`（第42行）。
- `buildTypes.debug`（第46-56行）：`isMinifyEnabled = false`，保留 so 调试符号。
- `buildTypes.release`（第57-65行）：`isMinifyEnabled = true`，应用 proguard。
- `rust { rootDirRel = "../../../" }`（第75-77行）。
- `dependencies`（第79-95行）：`implementation(project(":tauri-plugin-campus-network-bind"))` 等手写插件（第87-89行），官方插件（第90-91行），androidx 依赖（第80-84行）。

**`android/src-tauri/gen/android/settings.gradle`**（第1-21行）：`include ':app'`（第1行）；持久声明手写插件（第9-14行）与官方插件（第15-19行，路径硬编码 cargo registry 如第16行）；`apply from: 'tauri.settings.gradle'`（第21行）。

**`android/src-tauri/gen/android/tauri.settings.gradle`**（第1-15行）：自动生成 include，绝对路径指向 cargo registry 或本地（如第3行 tauri-android，第5行 biometric）。

**`android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt`**
- `open class Config`（第10-12行）：`lateinit var rootDirRel: String`。
- `open class RustPlugin : Plugin<Project>`（第14-85行）：`apply` 中定义 flavorDimensions "abi"（第30行），productFlavors `universal`/`arm64`/`arm`/`x86`/`x86_64`（第31-46行）；`afterEvaluate` 创建 `rustBuildUniversal{Debug,Release}` 等 Task（第49-83行），依赖 `BuildTask`。

**`android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt`**
- `open class BuildTask : DefaultTask()`（第9-45行）：`@Input` 属性 `rootDirRel`（第10行）、`target`（第12行）、`release`（第14行）；`@TaskAction fun assemble()`（第17-45行）。
- `fun runTauriCli(executable: String)`（第47-69行）：硬编码 `val executable = """C:\Program Files\nodejs\node.exe"""`（第19行），`val tauriJs = """E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\node_modules\@tauri-apps\cli\tauri.js"""`（第52行）；执行 `node tauri.js android android-studio-script`（第53行）。

#### 3.2 Kotlin 胶水代码（Generated/手写）

**`WryActivity.kt`**（生成）
- `object WryLifecycleObserver : DefaultLifecycleObserver`（第23-49行）：`onCreate` 调 `Rust.create()`/`Rust.wryCreate()`（第26-27行），`onStart`→`Rust.start()`（第32行）等。
- `abstract class WryActivity : AppCompatActivity()`（第51-175行）：
  - 字段 `private lateinit var mWebView`（第52行），`var id: Int = 0`（第53行），`open val handleBackNavigation: Boolean = true`（第54行）。
  - `fun setWebView(webView: RustWebView)`（第58-79行）：注册返回键回调。
  - `val version: String`（第81-112行）：获取 WebView 版本。
  - `onCreate`（第114-119行）：注册 `WryLifecycleObserver`，调 `Rust.onActivityCreate(this)`。
  - `onWindowFocusChanged`（第121-124行），`onSaveInstanceState`（第126-130行），`onPause/onResume`（第132-144行），`onDestroy`（第146-150行）调 `Rust.onActivityDestroy` 与 `Rust.onWebviewDestroy`，`onLowMemory`（第152-155行），`onNewIntent`（第157-160行），`getAppClass`（第162-164行），`startActivity`（第166-172行）。

**`TauriActivity.kt`**（生成）
- `object TauriLifecycleObserver : DefaultLifecycleObserver`（第17-32行）：`onResume/onPause/onStop` 转发 PluginManager。
- `abstract class TauriActivity : WryActivity()`（第34-65行）：`override val handleBackNavigation: Boolean = false`（第35行）；`onCreate` 调 `PluginManager.onActivityCreate`（第39行）；`fun getPluginManager()`（第42-44行）；`onNewIntent`（第46-49行），`onRestart`（第51-54行），`onDestroy`（第56-59行），`onConfigurationChanged`（第61-64行）。

**`MainActivity.kt`**（手写）
- `class MainActivity : TauriActivity()`（第11-49行）：
  - 第8-10行注释：不设 renderer 优先级策略避免 Chromium SIGKILL。
  - `onCreate`（第14-17行）：`enableEdgeToEdge()` 后 `super.onCreate`。
  - `onWebViewCreate`（第19-22行）：保存 `appWebView`。
  - `onStart/onStop`（第28-36行）：`appWebView?.resumeTimers()` / `pauseTimers()`。
  - `onTrimMemory`（第42-48行）：级别 ≥ `TRIM_MEMORY_RUNNING_LOW` 时 `evaluateJavascript("window.__onTrimMemory && window.__onTrimMemory($level)")`。

**`Rust.kt`**（生成）
- `object Rust`（第16-48行）：
  - `init { System.loadLibrary("campus_login_android_lib") }`（第17-19行）。
  - 外部函数：`onActivityCreate`(21), `onActivityDestroy`(22), `onActivitySaveInstanceState`(23), `onActivityLowMemory`(24), `onWindowFocusChanged`(25), `onNewIntent`(26), `create`(28), `start`(29), `resume`(30), `pause`(31), `stop`(32), `wryCreate`(34), `onWebviewDestroy`(35), `ipc`(37), `assetLoaderDomain`(39), `withAssetLoader`(40), `handleRequest`(41), `shouldOverride`(42), `onPageLoading`(43), `onPageLoaded`(44), `onEval`(45), `handleReceivedTitle`(47)。

**`RustWebView.kt`**（生成）
- `class RustWebView(context: Context, val initScripts: Array<String>, val id: String): WebView(context)`（第19-98行）：
  - `val isDocumentStartScriptEnabled`（第20行）。
  - `init`（第22-40行）：启用 JS、DOM、Geolocation、Database；若支持 DOCUMENT_START_SCRIPT 则注入。
  - `fun loadUrlMainThread(url: String)`（第42-46行）及重载（第48-52行）。
  - 重写 `loadUrl`（第54-64行）调 `Rust.shouldOverride`。
  - `fun loadHTMLMainThread(html: String)`（第66-70行），`fun evalScript(id: Int, script: String)`（第72-78行），`fun clearAllBrowsingData()`（第80-90行），`fun getCookies(url: String)`（第92-95行）。

**`RustWebViewClient.kt`**（生成）
- `class RustWebViewClient(webView: RustWebView, context: Context): WebViewClient()`（第17-100行）：
  - 字段 `currentUrl`（第19行），`lastInterceptedUrl`（第20行），`pendingUrlRedirect`（第21行），`assetLoader`（第23-26行）。
  - `shouldInterceptRequest`（第28-55行）：用 `WebViewAssetLoader` 或 `Rust.handleRequest`，加 `Cache-Control: no-store`（第47-49行）。
  - `shouldOverrideUrlLoading`（第57-62行），`onPageStarted`（第64-73行），`onPageFinished`（第75-77行），`onReceivedError`（第79-97行）处理 `ERROR_CONNECT` 重试。

**`RustWebChromeClient.kt`**（生成）
- `class RustWebChromeClient(appActivity: WryActivity) : WebChromeClient()`（第36-493行）：
  - 内部 `PermissionListener`（第37-39行）、`ActivityResultListener`（第41-43行）。
  - `init` 注册 `permissionLauncher`/`activityLauncher`（第51-72行）。
  - 重写 `onShowCustomView`(82-85), `onHideCustomView`(92-94), `onPermissionRequest`(96-121), `onJsAlert`(131-151), `onJsConfirm`(161-187), `onJsPrompt`(198-233), `onGeolocationPermissionsShowPrompt`(240-272), `onShowFileChooser`(274-304), 私有媒体捕获方法(306-376), `onConsoleMessage`(438-459), `onReceivedTitle`(487-492)。

**`Ipc.kt`**（生成）
- `class Ipc(val webView: RustWebView, val webViewClient: RustWebViewClient)`（第13-25行）：`@JavascriptInterface fun postMessage(message: String?)`（第15-22行）调 `Rust.ipc`。

**`Logger.kt`**（生成）
- `class Logger`（第16-89行）：companion object 提供 `tags`(20-24), `verbose`(26-35), `debug`(37-46), `info`(48-57), `warn`(59-68), `error`(70-83)；`shouldLog()` 仅 `BuildConfig.DEBUG`（第85-87行）。

**`PermissionHelper.kt`**（生成）
- `object PermissionHelper`（第17-117行）：`hasPermissions`(24-35), `hasDefinedPermission`(43-54), `hasDefinedPermissions`(62-69), `getManifestPermissions`(76-92), `getUndefinedPermissions`(100-116)。

#### 3.3 AndroidManifest 与资源

**`AndroidManifest.xml`**（第1-76行）
- 权限：`INTERNET`(3), `POST_NOTIFICATIONS`(4), `FOREGROUND_SERVICE`(5), `FOREGROUND_SERVICE_SPECIAL_USE`(6), `RECEIVE_BOOT_COMPLETED`(7), `WAKE_LOCK`(8), `REQUEST_INSTALL_PACKAGES`(10), `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`(14), `CAMERA`(16), `WRITE_SECURE_SETTINGS`(20-22, protected)。
- `application`（第27-75行）：`MainActivity`(32-44) `launchMode="singleTask"`；`ForegroundService`(47-54) `foregroundServiceType="specialUse"`；`BootReceiver`(57-64) `android:enabled="false"`；`FileProvider`(66-74) `authorities="${applicationId}.fileprovider"`。

**`file_paths.xml`**（第1-17行）：`external-files-path name="my_images" path="Pictures/"`（第11-13行），`files-path name="update" path="update/"`（第14-16行）。

**`proguard-tauri.pro`**（第1-4行）：keep `com.campuslogin.client.TauriActivity.getPluginManager()`。

**`proguard-wry.pro`**（第1-36行）：keep `RustWebView`, `RustWebChromeClient`, `Ipc`, `WryActivity` 的 native 方法及关键方法。

**其他资源**：`strings.xml`（第2-3行 app_name/main_activity_title）、`themes.xml`（Theme.MaterialComponents.DayNight.NoActionBar）、`colors.xml`、`ic_launcher_*` 等（略）。

### 4. 其他生成/配置

- `tauri.properties`（第1-3行）：`tauri.android.versionName=2.4.1`, `versionCode=2004001`。
- `gradle-wrapper.properties`（第3行）：`distributionUrl=https\://services.gradle.org/distributions/gradle-8.14.3-bin.zip`。
- `local.properties`（第1行）：`sdk.dir=C:/Users/ik/AppData/Local/Android/Sdk`（硬编码）。
- `gradlew`/`gradlew.bat`：标准 Gradle 包装脚本。

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│ Tauri Android Project (com.campuslogin.client v2.4.1)     │
├──────────────────────────────────────────────────────────┤
│ Rust Layer: Cargo.toml (line2) ── campus-login (path)    │
│   build.rs (line1) ── tauri_build::build()               │
├──────────────────────────────────────────────────────────┤
│ Config: tauri.conf.json (line5) capabilities (line3)     │
├──────────────────────────────────────────────────────────┤
│ Android Native (gen/)                                    │
│ ┌────────────────────────────────────────────────────┐   │
│ │ Gradle: build.gradle.kts (line17) settings (line9) │   │
│ │   RustPlugin (line14) → BuildTask (line9) node CLI │   │
│ └────────────────────────────────────────────────────┘   │
│ ┌────────────────────────────────────────────────────┐   │
│ │ Kotlin: MainActivity (line11) → TauriActivity      │   │
│ │   (line34) → WryActivity (line51) → Rust JNI (16)  │   │
│ │   RustWebView (19) Client (17) ChromeClient (36)   │   │
│ └────────────────────────────────────────────────────┘   │
│ Manifest (line27) ForegroundService, BootReceiver        │
└──────────────────────────────────────────────────────────┘
```

## Data Flow

1. **构建时**：`RustPlugin`（`RustPlugin.kt` 第14行）创建 Task → `BuildTask.assemble`（`BuildTask.kt` 第17行）调用 Node CLI（第52行 `tauri.js`）生成 `gen/` 内 Android 工程；读取 `tauri.conf.json` 与 `capabilities/default.json`。
2. **运行时初始化**：`MainActivity.onCreate`（`MainActivity.kt` 第14行）→ `WryActivity.onCreate`（`WryActivity.kt` 第114行）注册 `WryLifecycleObserver` 并调 `Rust.onActivityCreate`（`Rust.kt` 第21行）；`Rust` 静态块加载 `libcampus_login_android_lib.so`（第18行）。
3. **WebView 交互**：前端 JS → `Ipc.postMessage`（`Ipc.kt` 第15行）→ `Rust.ipc`（`Rust.kt` 第37行）→ Rust 业务逻辑/Plugin → 结果经 `RustWebView.evalScript`（`RustWebView.kt` 第72行）回传。
4. **后台保活**：`ForegroundService`（`AndroidManifest.xml` 第47行）运行 Rust 循环，持 WifiLock/WakeLock，通过 notification 插件请求权限。

## Connections

* [[rust-tauri-core]]：共享核心逻辑 `campus-login` crate（Cargo.toml 第31行）。
* [[android-plugins]]：手写插件 network-bind/keystore/foreground-service 实现（Cargo.toml 第34-40行）。
* [[frontend-app]]：前端资源 `../frontend/dist`（tauri.conf.json 第7行）。

## Known Issues

- **硬编码绝对路径**：`BuildTask.kt` 第19行 Node 路径、第52行 tauri.js 路径；`settings.gradle` 第16、19行 cargo registry 插件路径；`local.properties` 第1行 sdk.dir；`tauri.settings.gradle` 第3-14行绝对路径。切换机器需修改。
- **Renderer 优先级规避**：`MainActivity.kt` 第8-10行注释明确不设置 renderer 优先级，避免 wry 0.55.1 无 `onRenderProcessGone` 导致白屏冻结。
- **WRITE_SECURE_SETTINGS 权限**：`AndroidManifest.xml` 第20-22行声明但普通安装不授予，仅 adb `pm grant` 后生效（注释说明）。
- **未设 panic=abort**：Cargo.toml 第53-54行注释解释保留 unwind 以保活前台服务。
- **release 签名用 debug.keystore**：`build.gradle.kts` 第29-36行，证书一经发布不可更换。
