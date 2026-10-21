---
title: android-native-resources
type: module
source_files:
  - android/src-tauri/gen/android/app/src/main/AndroidManifest.xml
  - android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json
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
tags: [android, manifest, resources, fileprovider, tauri]
---

## Overview

本模块负责定义 Android 原生应用的底层资源配置，包括 `AndroidManifest.xml`、启动图标（Drawable）、Activity 布局、颜色、主题以及文件共享路径配置。这些资源构成了 Tauri 应用（CampusLogin）在 Android 平台上的外壳与基础运行环境。

核心功能涵盖：
1. **权限管理**：声明网络、通知、前台服务、电池优化白名单、摄像头及系统设置写入等权限。
2. **组件声明**：定义主 Activity、前台监控 Service、开机自启 Receiver 及 FileProvider。
3. **UI 资源**：提供自适应图标、深色/浅色主题支持及默认 Activity 布局。
4. **安全隔离**：通过 `file_paths.xml` 收窄 FileProvider 的共享范围，防止越权暴露。

## Key Components

### AndroidManifest.xml (android/src-tauri/gen/android/app/src/main/AndroidManifest.xml)

#### Permissions (权限声明)
- **INTERNET**: `3|` - 允许应用访问互联网。
- **POST_NOTIFICATIONS**: `4|` - Android 13+ 发送通知所需权限。
- **FOREGROUND_SERVICE**: `5|` - 启动前台服务所需权限。
- **FOREGROUND_SERVICE_SPECIAL_USE**: `6|` - 指定特殊用途的前台服务类型。
- **RECEIVE_BOOT_COMPLETED**: `7|` - 接收开机广播，用于实现开机自启。
- **WAKE_LOCK**: `8|` - 获取唤醒锁，防止屏幕休眠。
- **REQUEST_INSTALL_PACKAGES**: `10|` - 请求安装 APK 包的权限（需用户确认，非静默）。
- **REQUEST_IGNORE_BATTERY_OPTIMIZATIONS**: `14|` - 请求加入电池优化白名单。
    - *注意*：`11|-13|` 注释说明此权限需前端 UI (`KeepAliveSettingsCard`) 先弹框引导用户手动授权，不能直接静默调用。
- **CAMERA**: `16|` - 摄像头权限，用于 2D 人脸验证及 WebView getUserMedia。
- **WRITE_SECURE_SETTINGS**: `20|-22|` - 写入系统安全设置（如夜间模式切换、WiFi 策略）。
    - *限制*：此权限为 signature/privileged 级别，普通安装无法获得。仅在用户通过 `adb pm grant` 授予后生效。代码中已处理未授予时的降级逻辑（回落到“无权限”分支），无副作用。

#### Application & Activity (应用与主界面)
- **Application Attributes**: `27|-31|`
    - `icon`: `28|` - 引用 `@mipmap/ic_launcher`。
    - `label`: `29|` - 引用 `@string/app_name` ("CampusLogin")。
    - `theme`: `30|` - 引用 `@style/Theme.campus_login_android`。
    - `usesCleartextTraffic`: `31|` - 使用构建变量 `${usesCleartextTraffic}` 控制明文流量。
- **MainActivity**: `32|-44|`
    - `name`: `.MainActivity`
    - `launchMode`: `singleTask`
    - `exported`: `true`
    - `intent-filter`: `38|-43|`
        - `MAIN` + `LAUNCHER`: 标准桌面启动入口。
        - `LEANBACK_LAUNCHER`: `42|` - Android TV 启动入口。

#### Services & Receivers (后台服务与广播)
- **ForegroundService**: `47|-54|`
    - `name`: `com.campuslogin.plugin.monitorservice.ForegroundService`
    - `foregroundServiceType`: `specialUse`
    - `property`: `51|-53|` - 子类型标识为 `campus-network-monitor-keepalive`。
    - *职责*：持有常驻通知、WifiLock 和 WakeLock，业务逻辑由 Rust 循环执行。
- **BootReceiver**: `57|-64|`
    - `name`: `com.campuslogin.plugin.monitorservice.BootReceiver`
    - `enabled`: `false` (默认禁用，由设置页动态启停)。
    - `exported`: `false`
    - `intent-filter`: `61|-63|` - 监听 `BOOT_COMPLETED`。

#### Providers (文件提供者)
- **FileProvider**: `66|-74|`
    - `name`: `androidx.core.content.FileProvider`
    - `authorities`: `${applicationId}.fileprovider`
    - `grantUriPermissions`: `true`
    - `meta-data`: `71|-73|` - 引用 `@xml/file_paths` 定义共享路径。

### Tauri Configuration (android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json)

- **Bundle Info**: `1|`
    - `productName`: "CampusLogin"
    - `version`: "2.4.1"
    - `identifier`: "com.campuslogin.client"
- **App Windows**: `1|`
    - 单窗口 `main`，尺寸 400x800，可调整大小、最大化。
    - URL: `index.html`
- **Security**: `1|`
    - `csp`: 严格的内容安全策略，允许连接 `ipc:` 和本地资产。
    - `assetProtocol`: `1|` - 启用 `$APPDATA/files/face-models/**` 目录下的本地资源访问。
- **Build**: `1|`
    - `frontendDist`: `../frontend/dist`
    - `devUrl`: `http://localhost:5174/`
- **Android Specific**: `1|`
    - `minSdkVersion`: 29 (Android 10+)

### Drawables & Icons (启动图标资源)

#### ic_launcher_background.xml (android/src-tauri/gen/android/app/src/main/res/drawable/ic_launcher_background.xml)
- **Vector Drawable**: `2|-170|`
    - 尺寸: 108x108 dp。
    - 背景色: `#3DDC84` (绿色)。
    - 网格线: 绘制了多条垂直和水平参考线，颜色为半透明白色 (`#33FFFFFF`)，用于视觉对齐或调试。

#### ic_launcher_foreground.xml (android/src-tauri/gen/android/app/src/main/res/drawable-v24/ic_launcher_foreground.xml)
- **Vector Drawable**: `1|-30|`
    - 包含两个 `<path>`。
    - 第一个路径 `7|-23|`: 带有线性渐变填充 (`#44000000` 到 `#00000000`)，形状类似一个不规则的多边形或箭头尾部。
    - 第二个路径 `24|-29|`: 白色填充 (`#FFFFFF`)，形状复杂，可能代表具体的 Logo 图形或装饰性图案。

#### Adaptive Icon Manifest (android/src-tauri/gen/android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml)
- **Adaptive Icon**: `2|-5|`
    - `foreground`: `3|` - 引用 `@mipmap/ic_launcher_foreground` (通常指向 `drawable-v24` 中的前景图)。
    - `background`: `4|` - 引用 `@color/ic_launcher_background`。

### Colors & Themes (颜色与主题)

#### colors.xml (android/src-tauri/gen/android/app/src/main/res/values/colors.xml)
- 定义了标准的 Material Design 调色板：
    - `purple_200`, `purple_500`, `purple_700`
    - `teal_200`, `teal_700`
    - `black`, `white`

#### ic_launcher_background.xml (android/src-tauri/gen/android/app/src/main/res/values/ic_launcher_background.xml)
- `ic_launcher_background`: `3|` - 定义为白色 (`#fff`)。
    - *注意*：这与 `drawable/ic_launcher_background.xml` 中的绿色背景不同。这里定义的是颜色资源，供 `mipmap-anydpi-v26/ic_launcher.xml` 的 `background` 属性引用。

#### Strings (android/src-tauri/gen/android/app/src/main/res/values/strings.xml)
- `app_name`: `2|` - "CampusLogin"
- `main_activity_title`: `3|` - "CampusLogin"

#### Themes (android/src-tauri/gen/android/app/src/main/res/values/themes.xml & values-night/themes.xml)
- **Base Style**: `3|`
    - `parent`: `Theme.MaterialComponents.DayNight.NoActionBar`
    - 这意味着应用自动跟随系统日间/夜间模式，且没有操作栏 (ActionBar)。
    - 两个文件内容完全一致，均预留了自定义扩展点 `<!-- Customize your theme here. -->`。

### Layout (android/src-tauri/gen/android/app/src/main/res/layout/activity_main.xml)
- **ConstraintLayout**: `2|-7|`
    - 包含一个居中的 `TextView`，显示文本 "Hello World!"。
    - 这是一个默认的占位布局，实际内容主要由 Tauri WebView 渲染。

### File Paths (android/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml)

- **Security Restriction**: `2|-9|`
    - 注释明确指出：从原来的全量外部存储映射收窄为特定目录，以修复潜在的安全漏洞。
- **Defined Paths**: `10|-17|`
    - `external-files-path`: `11|-13|`
        - `name`: "my_images"
        - `path`: "Pictures/"
        - *用途*: 对应 `getExternalFilesDir(DIRECTORY_PICTURES)`，用于相机拍照回传图片给 WebView (`RustWebChromeClient.createImageFileUri`)。
    - `files-path`: `14|-16|`
        - `name`: "update"
        - `path`: "update/"
        - *用途*: 对应内部存储的 `files/update/` 目录，是 MonitorServicePlugin 安装 APK 的唯一来源 (`installApk`)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                   Android Native Resources                   │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐      ┌──────────────────────────────┐  │
│  │  AndroidManifest │      │   Tauri Config (JSON)        │  │
│  │  (manifest.xml)  │      │   (assets/tauri.conf.json)   │  │
│  │                  │      │                              │  │
│  │ - Permissions    │      │ - App ID / Version           │  │
│  │ - Activities     │      │ - Window Settings            │  │
│  │ - Services       │      │ - CSP / Security             │  │
│  │ - Providers      │      │ - Min SDK                    │  │
│  └────────┬─────────┘      └──────────────┬───────────────┘  │
│           │                               │                  │
│           ▼                               ▼                  │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                 Runtime Interaction                   │   │
│  │                                                      │   │
│  │  MainActivity (WebView) <───► Tauri Core (Rust)      │   │
│  │       ▲                           │                  │   │
│  │       │                           ▼                  │   │
│  │  ┌─────────────┐          ┌──────────────────┐       │   │
│  │  │ Foreground  │◄─────────│ MonitorService   │       │   │
│  │  │ Service     │          │ Plugin (Rust)    │       │   │
│  │  └─────────────┘          └──────────────────┘       │   │
│  │                                                      │   │
│  │  ┌─────────────┐          ┌──────────────────┐       │   │
│  │  │ BootReceiver│          │ FileProvider     │       │   │
│  │  └─────────────┘          └────────┬─────────┘       │   │
│  │                                    │                 │   │
│  │                            ┌───────▼─────────┐       │   │
│  │                            │ file_paths.xml  │       │   │
│  │                            │ (Images/APK)    │       │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **启动流程**:
   - 系统触发 `LAUNCHER` Intent。
   - `AndroidManifest.xml` `32|-44|` 中的 `MainActivity` 被实例化。
   - `activity_main.xml` `1|-18|` 加载初始布局（Hello World）。
   - Tauri 引擎初始化，加载 `tauri.conf.json` `1|` 中的配置。
   - WebView 加载 `index.html`，接管 UI 渲染。

2. **后台保活流程**:
   - `MonitorServicePlugin` (Rust) 请求启动前台服务。
   - `ForegroundService` `47|-54|` 启动，显示通知并持有 `WakeLock`/`WifiLock`。
   - 若设备重启，`BootReceiver` `57|-64|` 接收到 `BOOT_COMPLETED` 广播。
   - 由于 `enabled="false"` `59|`，默认不自动启动，需用户通过设置页显式开启。

3. **文件共享流程 (安全关键)**:
   - **场景 A (相机拍照)**:
     - Rust 端调用 `createImageFileUri`。
     - 图片保存在 `Context.getExternalFilesDir(DIRECTORY_PICTURES)`。
     - `FileProvider` 根据 `file_paths.xml` `11|-13|` 中的 `external-files-path` 生成 Content URI。
     - WebView 通过该 URI 访问图片。
   - **场景 B (APK 安装)**:
     - `MonitorServicePlugin.installApk` 下载 APK 至内部存储 `files/update/`。
     - `FileProvider` 根据 `file_paths.xml` `14|-16|` 中的 `files-path` 生成 Content URI。
     - 系统安装器通过该 URI 读取 APK 进行安装。

## Connections

- [[tauri-core]]: Tauri 核心 Rust 逻辑，通过 IPC 与 Android 原生层交互。
- [[monitor-service-plugin]]: 负责后台监控、电池优化白名单请求及 APK 更新安装。
- [[webview-integration]]: 处理 WebView 内的媒体访问（相机）及文件选择器行为。