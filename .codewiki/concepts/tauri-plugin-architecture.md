---
type: concept
title: tauri-plugin-architecture
source_files:
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/network/useDnsSetup.ts
  - android/frontend/src/auth/DashboardPanel.tsx
  - android/frontend/src/auth/useAuth.ts
  - android/frontend/src/account/SelfOnlineStack.tsx
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/settings/SettingsPanel.tsx
  - android/frontend/src/settings/useOnboardingFlow.ts
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
  - android/plugins/keystore/src/lib.rs
  - android/plugins/foreground-service/android/src/main/java/com/campuslogin/plugin/monitorservice/MonitorServicePlugin.kt
  - android/plugins/foreground-service/src/lib.rs
  - android/plugins/network-bind/android/src/main/java/com/campuslogin/plugin/networkbind/NetworkBindPlugin.kt
  - android/plugins/network-bind/src/lib.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/campus_detect.rs
tags: [Tauri, IPC, Plugin, Android, Architecture]
---

## Overview

`tauri-plugin-architecture` 是 Campus Login Assistant Android 应用的核心交互架构模式，定义了前端 React UI 层与后端 Rust/Kotlin 原生能力之间的通信机制。该架构基于 Tauri v2 的插件系统，采用“Rust 桥接 + Kotlin 原生实现”的双轨模式，通过 IPC (Inter-Process Communication) 命令面暴露功能。其核心职责是将复杂的 Android 系统级操作（如 Wi-Fi 绑定、前台保活、生物识别密钥存储）封装为标准化的异步 API，并通过前端的 `tauriApi` 统一入口进行调用，同时处理网络不稳定下的重试逻辑与安全门控。

## Key Components

### Frontend Hooks & Store (`android/frontend/src/hooks/tauriApi.ts`)
- **`desktopOnly<T>(name)`**: 返回 `Promise.reject`，用于适配器/DHCP/DNS/窗口等安卓无对应物的功能。
- **`noopListener`**: 返回 no-op 清理函数，用于安卓无对应的事件订阅（如 `onAdaptersChanged`）。
- **`TauriApi` 接口**: 定义全部 IPC 方法签名。
- **`verifyWindowsIdentity`**: **核心验证门逻辑**。统一出口：先检查 `shouldUseFaceFallback()`，若开启且系统生物识别不可用（国产平板 Class 1），则走应用内 2D 人脸比对 (`useFaceDialogStore`)；否则走系统 `biometricAuthenticate`。无论哪条链路，最终都调用后端 `verify_biometric_identity` 上报 `{success, method}`。失败归一化为 `CommandResult` 而非抛出 IPC 异常。
- **`openExternal`**: URL 安全校验（协议白名单、长度<2048、`new URL` 解析），通过 `openerOpenUrl` 打开。
- **`isRetryableError`**: 判断错误是否可重试（包含 'timeout/network/fetch/connection'）。
- **`withRetry`**: 指数退避重试（基础 500ms，最大 2 次）。
- **`tauriApiWithRetry`**: 仅对 `saveConfig` 启用重试（一次性关键操作）；其他高频接口由业务层或后端事件流兜底。

### Network Module (`android/frontend/src/network/useDnsSetup.ts`)
- **`ALI_DNS`**: 阿里公共 DNS (`223.5.5.5`, `223.6.6.6` 等)。
- **`RECOMMENDED_DNS`**: 阿里 + 腾讯 DNSPod 集合，用于判定“是否推荐 DNS / DoH 是否已启用”。
- **`handleCheckDns`**: 调用 `checkDnsDohStatus`，若未使用推荐 DNS 或推荐 DNS 但未启用 DoH，记录 warning toast。
- **`handleSetupDnsDoh`**: 调用 `setupDnsDoh(dnsFamily)`，成功后独立捕获刷新状态（避免外层 catch 重复报错），失败记录 error toast。

### Auth & Dashboard (`android/frontend/src/auth/DashboardPanel.tsx`)
- **`DashboardPanelProps`**: 含 `excludeCards`（排除 Windows 专属卡）、`mergeSelfServiceEmpty`（无凭据时合并 selfOnline/selfLog 为空态卡）、`extraCards`（宿主自定义卡片）。
- **`ExtraCardDef`**: 自定义卡片定义，含 `render` 函数。
- **`ALL_CARDS`**: 内置卡片列表 (`quickActions`, `accountManage`, `selfOnline`, `selfLog`, `networkQuality`)。
- **`DEFAULT_LAYOUT`**: 默认顺序。
- **`QuickActionsCard`**: DHCP 续租与换 IP 按钮，网络质量差时触发危险光晕动画。
- **`AccountManageCard`**: 账号切换列表，当前账号高亮，切换时显示 loading 状态。
- **`NetworkQualityCard`**: 显示网关/外网延迟，支持手动刷新。
- **`SelfOnlineCard`**: 在线设备概览。数据每 60s 轮询以处理设备踢下线。支持掩码/查看切换（需通过 `useHelloGate` 验证门）。支持设备别名编辑与单设备注销。
- **`SelfLogCard`**: 近期上网记录，最多展示 5 条。同样支持掩码/查看切换。
- **`SelfServiceMergedEmptyCard`**: 移动端无凭据时的合并空态卡。
- **`EditableCardItem`**: 编辑模式下的单卡容器，实现自研长按拖动交换（350ms 激活，touchmove 防滚动接管，vibrate 反馈）。
- **`DashboardPanel`**: 主容器。管理卡片布局持久化（`safeStorage`）、黑名单（`EXTRA_REMOVED_KEY`）、可用卡片过滤、渲染逻辑。

### Auth Hook (`android/frontend/src/auth/useAuth.ts`)
- **`handleOpenPortal`**: 打开 Portal 页（默认 `http://10.1.99.100`）。
- **`handleOpenSelfService`**: 打开自助服务页（`http://10.1.80.200:8080/Self/login/?302=LI`）。

### Account Stack (`android/frontend/src/account/SelfOnlineStack.tsx`)
- **`PEEK_TOP/BOTTOM`**: 露出边高度 44px。
- **`DRAG_THRESHOLD`**: 60px 翻页阈值。
- **`V_SIDE_SCALE`**: 竖排侧卡缩放 0.75。
- **`SIDE_SCALE/SIDE_SCALE_2`**: 水平侧卡缩放 0.6/0.45。
- **`CF_X`**: 相邻/次邻横向偏移 60%/85%。
- **`NARROW_MIN_W`**: 窄屏最小宽度 213px。
- **`OnlineDeviceStack`**: 垂直堆叠。整副牌连续滚动，当前卡居中，上下露出边缩小压暗。支持指针拖拽与点击分区换页。
- **`OnlineDeviceStackCard`**: 单张设备卡内容。显示登录时间、IP/MAC 芯片、流量/时长、别名标签、注销按钮。
- **`OnlineDeviceStackH`**: 水平堆叠。机制同垂直，但方向为左右。支持 `coverflow` 与 `narrow` 变体。

### Self Service State (`android/frontend/src/account/selfServiceState.ts`)
- **`helloEnabled()`**: 读取 `config.selfHelloEnabled !== false`，缺失视为开启。
- **`BIOMETRIC_ERROR_KEY`**: 映射错误码到 i18n key，包括 `userCancel`, `systemCancel`, `biometryNotEnrolled`, `faceTimeout` 等。
- **`biometricFailMessage(err, t)`**: 根据错误码返回翻译后的失败消息，否则回退原始消息或默认文案。
- **`VERIFY_TTL_MS = 570_000`**: 验证有效期 570s，对齐后端 600s TTL，留 30s 余量。
- **`gateFresh(verifiedAt)`**: 判定门是否新鲜（TTL 内且时钟未回拨 `elapsed >= 0`）。
- **`useHelloGate(options?)`**: Hook，返回验证函数。支持 `ignoreToggle: true`（强制验证，无视总开关）。首次操作验证，成功后缓存 `helloGateVerifiedAt`。
- **`resetSelfSessionGate()`**: 重置自助服务会话门。
- **`useSelfServiceVerify()`**: 自助服务面板专用验证门。支持 `selfReverifyEachAction` 配置（每次操作都验证）。

### Settings Panel (`android/frontend/src/settings/SettingsPanel.tsx`)
- **`isAndroid`**: 区分平台，仅安卓渲染保活卡片。
- **`handleSecurityDisable`**: 关闭安全开关前需通过 Windows Hello 验证。使用 `tauriApiWithRetry.verifyWindowsIdentity`。
- **安全设置**:
    - `selfHelloEnabled`: 切换需验证（关闭时）。
    - `selfReverifyEachAction`: 依赖 `selfHelloEnabled`，关闭时需验证。
    - `allow2dFaceVerify`: 开启前弹风险确认弹窗 (`ConfirmDialog`)；关闭时清除人脸模板 (`clearTemplate()`)。
- **网络质量检测**:
    - 关闭 `enableNetworkQuality` 时联动关闭 `enableLatencyTest` 并清理面板引用。
    - 打开延迟计算选项 Modal (`latencyModalOpen`)，包含 `skipTtfbInLatency` 和 `skipContentInLatency` 开关。
- **自订阅 config**: 使用 `useConfigStore(useShallow(...))` 避免不必要的重渲染。

### Onboarding Flow (`android/frontend/src/settings/useOnboardingFlow.ts`)
- **`ONBOARDING_STEP_COUNT = 4`**
- **`BIND_OPERATOR_NONE = '__none__'`**
- **`DEFAULT_OPERATOR = '__default__'`**
- **`BIND_SUCCESS_ADVANCE_MS = 1200`**, **`LOGIN_SUCCESS_ADVANCE_MS = 1500`**
- **`handleNext(step=1)`**: 落盘账号信息（用户名、运营商、密码），避免空密码覆盖已保存值。
- **`handleBind`**: 绑定运营商账号。先通过 `ensureBindHello()` 验证，再调用 `tauriApiWithRetry.bindOperator`。凭据复用账号步输入。
- **`handleLoginAndFinish`**: 最终校验账号完整性，执行登录，成功后设置标记并关闭向导。
- **`canBind`**: 校验绑定表单完整性（用户名、密码/掩码、运营商、手机号格式、短信密码）。
- **`passwordSaved`**: 判断密码是否已保存（用于显示掩码或占位符）。

### Keystore Plugin (`android/plugins/keystore/...`)
- **`KEY_ALIAS = "campus_login_master"`** (`KeystorePlugin.kt`): 密钥别名。
- **`getOrCreateKey()`** (`KeystorePlugin.kt`): 获取或生成 256-bit AES Key，使用 GCM 模式。
- **`encrypt(invoke)`** (`KeystorePlugin.kt`): 加密明文，返回 base64(iv + ciphertext)。
- **`decrypt(invoke)`** (`KeystorePlugin.kt`): 解密 base64 数据，提取 IV 和密文，返回明文。
- **`CampusKeystore<R>`** (`lib.rs`): Rust 侧封装，调用 Android 插件。
- **`encrypt(text) -> Result<String>`** (`lib.rs`): 调用 `run_mobile_plugin("encrypt", ...)`。
- **`decrypt(data) -> Result<String>`** (`lib.rs`): 调用 `run_mobile_plugin("decrypt", ...)`。
- **`init()`** (`lib.rs`): 注册插件并管理状态。

### Monitor Service Plugin (`android/plugins/foreground-service/...`)
- **`startMonitor(invoke)`** (`MonitorServicePlugin.kt`): 启动前台服务，处理 SDK >= O 的 FGS 限制。
- **`stopMonitor(invoke)`** (`MonitorServicePlugin.kt`): 停止前台服务。
- **`updateNotification(invoke)`** (`MonitorServicePlugin.kt`): 更新通知内容。
- **`installApk(invoke)`** (`MonitorServicePlugin.kt`): 通过 FileProvider 安装 APK，防路径逃逸。
- **`beginProbeWindow(invoke)`** / **`endProbeWindow(invoke)`** (`MonitorServicePlugin.kt`): 探针窗口锁管理（WifiLock + WakeLock）。
- **`getPowerState(invoke)`** (`MonitorServicePlugin.kt`): 返回屏幕状态和 WiFi 连接状态。
- **`vendorTargets`** (`MonitorServicePlugin.kt`): 映射各品牌（小米、华为、OPPO、vivo 等）的自启/省电页组件名。
- **`openVendorBatterySettings(invoke)`** (`MonitorServicePlugin.kt`): 跳厂商设置页，降级链：厂商专用页 → 应用详情页 → 通用设置。
- **`openNotificationSettings(invoke)`** (`MonitorServicePlugin.kt`): 跳通知设置页，降级链：API 26+ 标准页 → 旧版 action → 应用详情页 → 通用设置。
- **`setBootAutostart(invoke)`** / **`isBootAutostartEnabled(invoke)`** (`MonitorServicePlugin.kt`): 管理开机自启 Receiver 的启用状态和 SharedPreferences 记忆。
- **`BootReceiver`** (`MonitorServicePlugin.kt`): 处理 `BOOT_COMPLETED`，检查 SharedPreferences 决定是否拉起前台服务和 MainActivity。
- **`CampusMonitorService<R>`** (`lib.rs`): Rust 侧封装，提供 `start_monitor`, `stop_monitor`, `update_notification`, `begin_probe_window`, `end_probe_window`, `get_power_state`, `set_boot_autostart`, `is_boot_autostart_enabled`, `install_apk`, `get_battery_optimization_info`, `request_ignore_battery_optimizations`, `open_vendor_battery_settings`, `open_notification_settings` 等方法。
- **`init()`** (`lib.rs`): 注册插件。

### Network Bind Plugin (`android/plugins/network-bind/...`)
- **`class NetworkBindPlugin`** (`NetworkBindPlugin.kt`): Tauri 插件主类。
- **`bindToWifi(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 核心绑定命令。先查 `allNetworks` 优先选有 `NET_CAPABILITY_INTERNET` 的 WiFi；若无或直绑失败，退回到 `requestNetwork`（显式 remove `NET_CAPABILITY_INTERNET` 以防 captive portal 被拒）。返回 `{bound, path, reason}`。
- **`unbind(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 解绑，恢复系统默认路由。
- **`acceptWifiNetwork(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 处理 captive portal。依次尝试：已验证 (`already_validated`) -> Hidden API `setAcceptUnvalidated` (反射调用，需 CONNECTIVITY_INTERNAL) -> Settings.Global 写入 (`settings_global`, 需 WRITE_SECURE_SETTINGS)。
- **`startWifiWatcher(invoke: Invoke)`** / **`stopWifiWatcher(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 注册/注销 `NetworkCallback`，监听 `available`, `lost`, `validated`/`unvalidated` 事件，通过 `trigger("wifiChanged", ...)` 发给 Rust。
- **`getWifiSsid(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 获取 SSID。API 33+ 用 `NEARBY_WIFI_DEVICES`，低版本恒返回空。
- **`requestWifiSsidPermission(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 请求运行时权限。
- **`reportWifiUnusable(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 调用 `reportNetworkConnectivity(wifi, false)` 触发系统重检。
- **`ensureAvoidBadWifi(invoke: Invoke)`** / **`restoreAvoidBadWifi(invoke: Invoke)`** / **`restoreWrittenSettings(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 管理 `Settings.Global.network_avoid_bad_wifi` 和 `captive_portal_mode`。使用 `SharedPreferences` ("network_settings_guard") 做快照还原，防止重启后设置回滚。
- **`getSecureSettingsStatus(invoke: Invoke)`** (`NetworkBindPlugin.kt`): 查询 WRITE_SECURE_SETTINGS 授权状态及待还原标记。
- **常量/细节**: `REQUEST_TIMEOUT_MS = 3000`; `WIFI_SSID_PERMISSION_ALIAS = "wifiSsid"`; `KEY_AVOID_BAD_WIFI`, `KEY_CAPTIVE_PORTAL_MODE`; `PREF_PREFIX = "prev_"`.
- **`pub struct CampusNetworkBind<R: Runtime>`** (`lib.rs`): 持有 `PluginHandle`，包装所有 Kotlin 命令。
- **`impl CampusNetworkBind`** (`lib.rs`):
    - `bind_to_wifi(&self) -> Result<serde_json::Value>`
    - `accept_wifi_network(&self) -> Result<serde_json::Value>`
    - `start_wifi_watcher(&self, channel: tauri::ipc::Channel<...>) -> Result<...>`: 先注册 Listener，再启动 Watcher。
    - `stop_wifi_watcher(&self, channel_id: u32) -> Result<...>`: 移除 Listener 并停止 Watcher。
    - `get_wifi_ssid(&self)`, `request_wifi_ssid_permission(&self)`, `report_wifi_unusable(&self)`, `ensure_avoid_bad_wifi(&self)`, `restore_avoid_bad_wifi(&self)`, `restore_written_settings(&self)`, `get_secure_settings_status(&self)`: 均为 `run_mobile_plugin` 调用。
- **`pub trait CampusNetworkBindExt<R: Runtime>`** (`lib.rs`): 扩展 `Manager`，提供 `campus_network_bind()` 访问器。
- **`pub fn init<R: Runtime>() -> TauriPlugin<R>`** (`lib.rs`): 插件初始化，注册 Android 插件 ID `"com.campuslogin.plugin.networkbind"`。

### Config State (`android/src-tauri/src/config_state.rs`)
- **`pub const PASSWORD_MASK: &str = "***"`**
- **`lazy_static! CONFIG_IO_LOCK`**: 全局串行锁，防止 save_config / switch / delete 并发竞态。
- **`pub async fn config_io_lock() -> MutexGuard<'static, ()>`**
- **`pub struct Settings`**: 配置结构，字段对齐桌面 Config，包含账号、夜切开关、巡检间隔、SSID 名单等。
- **`impl Default for Settings`**: 默认值，如 `background_check_interval: 60_000`, `background_check_idle_interval: 300_000`, `latency_test_interval: 600_000`, `config_schema_version: 7`.
- **`pub struct CryptoBridge`**: 加解密桥，移动端走 `CampusKeystore`，host 测试走假实现。
- **`pub async fn load_file/save_file(...)`**: 密码经 AES-GCM 加密后存 `password_cipher` 字段；落盘用原子写（tmp + rename）。
- **`pub async fn load_from/save_to(...)`**: 加载时执行 `migrate_legacy_defaults`。
- **`async fn migrate_legacy_defaults(...)`**: v0→v7 逐步迁移，如 v5→v6 质量间隔 60s→600s，v6→v7 夜切默认开、定时动作禁用哨兵 1440。
- **`pub fn masked_for_display(s: &Settings) -> serde_json::Value`**: 密码掩码出口。
- **`pub fn resolve_password_field(...)`**: 处理空串/掩码占位符回退已存值语义。
- **`#[tauri::command] get_config/save_config(...)`**: 配置读写命令，`save_config` 持 `CONFIG_IO_LOCK`。
- **`pub async fn current_settings(...)`**: 缓存优先读取。

### Campus Detect (`android/src-tauri/src/campus_detect.rs`)
- **`pub fn pick_campus_source_ip(interfaces: &[(String, IpAddr)]) -> Option<Ipv4Addr>`**: 排除蜂窝 (`rmnet*`, `ccmni*`)，优先 `wlan0`。
- **`pub async fn portal_reachable(host: &str, port: u16, timeout: Duration) -> bool`**: 调用 `bound_socket::tcp_connect_bounded`。
- **`pub struct CampusProbe`**: 包含 `source`, `portal_ok`, `on_campus`, `wifi_off`.
- **`pub async fn probe_campus(campus_gateway: &str, portal_url: &str) -> Result<CampusProbe, String>`**: 核心探测逻辑。若子网不命中，并行探测网关和 Portal TCP。
- **`pub fn campus_status_message(wifi_off: bool, on_campus: bool) -> &'static str`**: 返回 "WiFi 未连接"/"已连接校园网"/"未连接校园网"。
- **`pub fn ssid_matches(ssid: &str, required: &str) -> bool`**: 大小写不敏感匹配，特判 `i-wxxy` 兼容 `iwxxy-2/3`。
- **`pub(crate) async fn probe_campus_with_ssid(...) -> (Result<CampusProbe, String>, Option<String>)`**: 开启名称检查时取 SSID 参与判定。
- **`#[tauri::command] check_campus_status(...)`**: 调用 `probe_campus_with_ssid`，返回 JSON 包含 `onCampusNetwork`, `currentSsid`, `campusMessage` 等。
- **`#[tauri::command] request_wifi_ssid_permission(...)`**: 转发给插件。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Frontend Layer (React)                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │ Dashboard    │  │ Settings     │  │ Onboarding   │  │ Self Online  │   │
│  │ Panel        │  │ Panel        │  │ Flow         │  │ Stack        │   │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘   │
│         │                 │                 │                 │           │
│         ▼                 ▼                 ▼                 ▼           │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                    tauriApi / tauriApiWithRetry                     │  │
│  │              (Unified IPC Entry & Retry Logic)                      │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                              │ Tauri IPC
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Rust Backend Core                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │ config_state │  │ campus_detect│  │ account_cmds │  │ emit_events  │   │
│  │ (CryptoBridge│  │ (TCP Probe  │  │ (Auto Create)│  │ (Config Changed│   │
│  │  IO Lock)    │  │  SSID Match)│  │              │  │  Events)      │   │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────────────┘   │
│         │                 │                 │                               │
│         ▼                 ▼                 ▼                               │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                        Rust Plugins Wrapper                         │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐             │  │
│  │  │ Keystore     │  │ Foreground   │  │ Network Bind │             │  │
│  │  │ (AES-GCM)    │  │ Service      │  │ (Wi-Fi Bind) │             │  │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘             │  │
│  └─────────┼─────────────────┼─────────────────┼──────────────────────┘  │
└────────────┼─────────────────┼─────────────────┼─────────────────────────┘
             │ JNI             │ JNI             │ JNI
             ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Kotlin Native Implementation                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                     │
│  │ Keystore     │  │ Monitor      │  │ Network      │                     │
│  │ Plugin       │  │ Service      │  │ Bind         │                     │
│  │ (AndroidKey) │  │ Plugin       │  │ Plugin       │                     │
│  └──────────────┘  └──────────────┘  └──────────────┘                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

数据与控制流在模块间按以下路径流动：

1.  **初始化与桥接建立**：
    *   应用启动时，`frontend-hooks-store` 中的 `useAppInit` 加载初始配置。
    *   Rust 后端 (`rust-backend-core`) 注册 Commands，Kotlin 插件 (`rust-plugins-*`) 通过 JNI 暴露原生方法。
    *   前端通过 `tauriApi` 建立与后端的 IPC 通道。

2.  **UI 触发与指令转发**：
    *   用户在 `DashboardPanel` (frontend-auth-module) 或 `SettingsPanel` (frontend-settings-module) 执行操作。
    *   控制流进入 `useAuth()` 或相关 Settings Hooks，调用 `tauriApi.invoke()`。
    *   若操作涉及网络质量或 DNS 配置，`frontend-network-module` 的 `useDnsSetup()` 介入，强制使用 `tauriApiWithRetry` 封装调用，以应对网络抖动。

3.  **原生执行与安全门控**：
    *   **普通操作**：IPC 消息直达 `rust-backend-core` 或对应 Plugin 的 Rust Wrapper。
        *   例如：Wi-Fi 绑定请求由 `rust-plugins-network-bind` 接收，转发至 Kotlin 的 `NetworkBindPlugin.kt` 执行 JNI 调用。
        *   例如：保活服务启动由 `rust-plugins-foreground-service` 的 `MonitorServicePlugin` 处理，管理 WakeLock。
    *   **敏感操作**：
        *   **密钥访问**：涉及密码读写时，流程指向 `rust-plugins-keystore`。Rust 层调用 Kotlin 的 `KeystorePlugin.kt`，利用 `AndroidKeyStore` 进行 AES-GCM 加解密，明文不落地。
        *   **生物识别门控**：在 `frontend-account-module` 中，`SelfOnlineStack` 或账户切换前，先调用 `useHelloGate`。若未通过 Hello 验证，则拦截 IPC 请求，防止敏感数据泄露。

4.  **结果返回与状态同步**：
    *   原生层执行完成后，通过 Tauri Event Listener 或 Command Response 将结果返回前端。
    *   `frontend-hooks-store` 中的 Store (如 `useAuthStore`, `useQualityStore`) 接收事件并更新状态。
    *   UI 组件（如 `StatusBar`, `NetworkQualityCapsule`）响应 Store 变化，重新渲染可视化界面。

## Connections

- [[concepts/tauri-plugin-architecture]]