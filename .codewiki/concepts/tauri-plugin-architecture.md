---
title: tauri-plugin-architecture
type: concept
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
- **`desktopOnly<T>(name)`**: 返回被拒绝的 Promise，用于安卓端无对应物的桌面功能（如 DHCP、DNS、窗口管理）。
- **`noopListener()`**: 返回空操作清理函数，用于安卓端不支持的事件订阅。
- **`createEventListener<T>(eventName)`**: 事件监听工厂。处理 `listen` 异步初始化过程中的取消逻辑，防止内存泄漏或未触发回调。
- **`isRetryableError(e)`**: 判断错误是否可重试（包含 'timeout', 'network', 'fetch', 'connection'）。
- **`withRetry(fn, maxRetries=2, baseDelay=500)`**: 指数退避重试逻辑。仅 `saveConfig` 使用此包装器 (`tauriApiWithRetry`)，其他高频接口（如 `checkPortalStatus`）由业务层自行控制频率。
- **`export interface TauriApi`**: 定义所有 IPC 方法签名。
    - **配置类**: `getConfig`, `saveConfig`, `getInitData`.
    - **网络类**: `bindToWifi`, `checkPortalStatus`, `checkCampusStatus`, `dhcpRenewAll` (桌面专属), `checkNetworkQuality`.
    - **认证/生物识别**: `doLogin`, `doLogout`, `verifyWindowsIdentity`.
        - *关键实现*: `verifyWindowsIdentity`。统一出口：若启用人脸回退 (`shouldUseFaceFallback`) 且系统生物识别不可用，则调用 `useFaceDialogStore` 进行 2D 人脸验证；否则走系统 `biometricAuthenticate`。无论成功失败，均通过 `invoke('verify_biometric_identity')` 上报结果 `{success, method}`，失败归一化为 `{success: false}` 而非抛出异常。
    - **账号类**: `listAccounts`, `createAccount`, `switchAccount`, `deleteAccount`, `renameAccount`, `getActiveAccount`.
    - **后台检测/延迟测试**: `startBackgroundCheck`, `stopBackgroundCheck`, `triggerBackgroundCheck`, `startLatencyTest`, `stopLatencyTest`.
    - **更新类**: `checkUpdate`, `downloadUpdate`, `installUpdate`, `getMirrorUrls`.
    - **系统设置类**: `getAutoLaunch`, `setAutoLaunch`, `getBatteryOptimizationInfo`, `requestIgnoreBatteryOptimizations`, `openVendorBatterySettings`, `getAvoidBadWifiStatus`, `restoreWrittenSettings`.
- **`export const tauriApiWithRetry`**: 扩展自 `tauriApi`，仅重写 `saveConfig` 增加重试逻辑。
- **`export interface AvoidBadWifiStatus`**: 包含 `granted` (WRITE_SECURE_SETTINGS 权限), `avoidBadWifi` 值, `pendingRestoreAvoid/Captive` 快照状态。
- **`export interface RestoreWrittenSettingsResult`**: 包含 `restored` 记录及可选 `reason`。

### Network Module (`android/frontend/src/network/useDnsSetup.ts`)
- **`ALI_DNS`**: 阿里 DNS IP 集合。
- **`TENCENT_DNS`**: 腾讯 DNS Pod IP 集合。
- **`RECOMMENDED_DNS`**: 合并后的推荐 DNS 集合，用于判定“是否推荐 DNS”及“DoH 是否已启用”。
- **`export function useDnsSetup()`**:
    - 状态: `dohEnabling` (忙锁), `dnsFamily` ('both' | 'ipv4' | 'ipv6').
    - `handleCheckDns`: 调用 `ipc.checkDnsDohStatus()`。若未配置推荐 DNS，添加警告日志；若配置了但未启用 DoH，添加警告日志。
    - `handleSetupDnsDoh`: 调用 `ipc.setupDnsDoh(dnsFamily)`。成功后立即刷新状态并 toast 成功；失败或异常 toast 错误。使用 `mountedRef` 防止 StrictMode 下的状态更新丢弃。

### Auth & Dashboard (`android/frontend/src/auth/DashboardPanel.tsx`)
- **`type CardId`**: 内置卡 ID 或自定义卡 ID。
- **`interface ExtraCardDef`**: 宿主注入的自定义卡片定义，含 `render` 函数。
- **`ALL_CARDS`**: 内置卡片列表：`quickActions`, `accountManage`, `selfOnline`, `selfLog`, `networkQuality`.
- **`DEFAULT_LAYOUT`**: 默认布局顺序。
- **`EXTRA_REMOVED_KEY`**: localStorage key，记录用户显式删除的自定义卡 ID，防止迁移时复活。
- **`LONG_PRESS_MS`**: 编辑模式下激活拖拽的长按时间 (350ms)。
- **`DashboardPanelProps`**: 包含 `accounts`, `activeAccount`, `onSwitchAccount`, `onDhcpRenew`, `excludeCards` (移动端排除某些卡), `mergeSelfServiceEmpty` (移动端合并空态卡).
- **`QuickActionsCard`**: DHCP 续租与释放重获按钮。使用 `useAsyncLock` 防止重复点击。
- **`AccountManageCard`**: 账号切换列表。使用 `switchTimerRef` 和 `mountedRef` 处理切换状态显示。
- **`NetworkQualityCard`**: 展示网络质量指标与延迟对。
- **`useSelfCardReveal`**: 自助服务卡的数据揭示 Hook。依赖 `useHelloGate` 进行生物识别验证，验证通过后解锁敏感数据（IP/MAC等）。
- **`useSelfCardFetch`**: 自助服务卡数据获取 Hook。自动根据 account 变化发起请求，支持手动 refetch。
- **`SelfOnlineCard`**: 当前在线设备列表。
    - 数据源：`tauriApiWithRetry.querySelfDashboard`。
    - 轮询：凭据齐备时每 60s 自动刷新一次 (`useEffect`)。
    - 交互：支持设备别名修改 (`handleSaveAlias`) 和单设备注销 (`handleOffline`)。
    - 渲染：平板使用 `OnlineDeviceStackH`，手机使用 `OnlineDeviceStack`。
- **`SelfLogCard`**: 近期上网记录。限制显示最近 5 条 (`SELF_LOG_LIMIT = 5`)。
- **`SelfServiceMergedEmptyCard`**: 移动端无凭据时的合并空态提示卡。
- **`EditableCardItem`**: 编辑模式下的单个卡片容器。实现自定义长按拖拽手势（非 framer-motion drag），支持 `onSwap` (交换位置) 和 `onRemove` (删除)。
- **`DashboardPanel`**: 主组件。
    - 布局持久化：从 `safeStorage` 加载/保存布局。
    - 编辑模式：允许添加/删除/排序卡片。
    - 条件渲染：根据 `config.enableNetworkQuality` 隐藏质量卡；根据 `mergeSelfServiceEmpty` 合并自助服务空态卡。

### Auth Hook (`android/frontend/src/auth/useAuth.ts`)
- **`useAuth()`**:
    - 从 `useAuthStore` 获取登录/登出/检查在线状态。
    - `handleOpenPortal`: 打开门户 URL (默认 `http://10.1.99.100`)。
    - `handleOpenSelfService`: 打开自助服务页面 (`http://10.1.80.200:8080/Self/login/?302=LI`)。

### Account Stack (`android/frontend/src/account/SelfOnlineStack.tsx`)
- **`PEEK_TOP/BOTTOM`**: 露出边高度 (44px)。
- **`DRAG_THRESHOLD`**: 拖拽翻页阈值 (60px)。
- **`V_SIDE_SCALE`**: 竖排侧卡缩放比例 (0.75)。
- **`CF_X`**: 水平堆叠相邻/次邻偏移百分比。
- **`SIDE_SCALE/SIDE_SCALE_2`**: 水平堆叠侧卡缩放比例 (0.6 / 0.45)。
- **`NARROW_MIN_W`**: 窄版变体最小宽度 (213px)。
- **`OnlineDeviceStack`**: 垂直堆叠。
    - 手势：上下拖拽翻页，点击上下半区翻页。
    - 动画：整副牌连续滚动，当前卡居中清晰，侧卡缩小压暗。
    - 量高：使用 `ResizeObserver` 动态计算舞台高度。
- **`OnlineDeviceStackCard`**: 单张设备卡片内容。展示登录时间、IP、MAC、流量、时长、终端类型、别名。支持改名和注销按钮。
- **`OnlineDeviceStackH`**: 水平堆叠（平板/桌面）。
    - 手势：左右拖拽翻页，点击左右半区翻页。
    - 变体：支持 `coverflow` 和 `narrow` (窄版，用于账号管理场景)。
    - 定位：`narrow` 模式下使用 `left: 50%` + `translateX` 居中，配合 `min-width`。

### Self Service State (`android/frontend/src/account/selfServiceState.ts`)
- **`VERIFY_TTL_MS = 570_000`**：验证有效期 9.5 分钟，对齐后端 `IDENTITY_VERIFY_TTL_SECS=600s` 并留 30s 余量。
- **`BIOMETRIC_ERROR_KEY`**：映射错误码到 i18n key，涵盖 `userCancel`, `biometryNotEnrolled`, `faceTimeout` 等。
- **`helloEnabled()`**：读取 `config.selfHelloEnabled`，缺失视为 `true`（兼容旧配置）。
- **`biometricFailMessage(err, t)`**：解析错误对象中的 `code` 字段，返回翻译后的错误文案或原始消息。
- **`gateFresh(verifiedAt)`**：判定门是否新鲜。检查 `elapsed >= 0 && elapsed < VERIFY_TTL_MS`，防止时钟回拨导致死锁。
- **`useHelloGate(options?)`**：Hook。返回异步验证函数。
    - 若 `ignoreToggle: true`（如查看明文密码），强制绕过总开关进行真实验证。
    - 若门新鲜则直接返回 `true`。
    - 调用 `tauriApiWithRetry.verifyWindowsIdentity`，成功则更新 `helloGateVerifiedAt = Date.now()`。
- **`resetSelfSessionGate()`**：重置自助服务面板的验证状态（面板卸载时调用）。
- **`useSelfServiceVerify()`**：Hook。用于自助服务面板内操作前的验证。支持 `config.selfReverifyEachAction` 配置，若为 `true` 则每次操作都验证，否则复用 TTL 内的验证结果。

### Settings Panel (`android/frontend/src/settings/SettingsPanel.tsx`)
- **平台差异**：`isAndroid = import.meta.env.VITE_PLATFORM === 'android'`，仅安卓渲染 `KeepAliveSettingsCard` 及特定安全开关。
- **状态管理**：使用 `useConfigStore(useShallow(...))` 订阅配置，避免非相关字段变化导致的级联重渲染。
- **安全控制逻辑**：
    - `handleSecurityDisable(apply)`：关闭安全开关（如 `selfHelloEnabled`）时，强制调用 `verifyWindowsIdentity`。失败则保持原状态（Switch 受控回弹）。
    - `selfReverify` 开关：依赖 `selfHelloEnabled`，若 Hello 关闭则禁用该开关。
- **2D 人脸逻辑**：
    - 开启时需弹出 `ConfirmDialog` (`faceRiskConfirmOpen`) 告知风险。
    - 关闭时调用 `clearTemplate()` 清除本地人脸模板，并更新配置。
    - 显示录入状态按钮，调用 `openFaceDialog('enroll')`。
- **网络质量检测联动**：
    - 关闭 `enableNetworkQuality` 时，联动关闭 `enableLatencyTest`（若开启），停止后端测试任务 (`stopLatencyTest`)。
    - 若当前活跃面板为 `quality`，切换回 `dashboard`，并清空 `defaultPanel`。
- **UI 结构**：响应式两列布局（`md:grid-cols-2`），左侧为启动/自动化/保活，右侧为安全/引导。底部有看板娘装饰图。

### Onboarding Flow (`android/frontend/src/settings/useOnboardingFlow.ts`)
- **常量**：
    - `ONBOARDING_STEP_COUNT = 4`。
    - `BIND_OPERATOR_NONE = '__none__'`, `DEFAULT_OPERATOR = '__default__'`。
    - `BIND_SUCCESS_ADVANCE_MS = 1200`, `LOGIN_SUCCESS_ADVANCE_MS = 1500`。
- **状态机**：
    - `step`: 0 (欢迎) → 1 (账号) → 2 (绑定运营商) → 3 (完成)。
    - `direction`: 记录步骤前进/后退方向，用于动画。
- **关键 Hook/函数**：
    - `useOnboardingFlow(options)`：主 Hook。
    - `ensureBindHello`：绑定步骤专用的 Hello 验证门，复用 `useHelloGate`。
    - `handleBind`：执行运营商绑定。校验凭据完整性 (`canBind`)，调用 `tauriApiWithRetry.bindOperator`。成功后延迟进入下一步。
    - `handleNext`：账号步骤保存逻辑。仅当输入新密码时才更新 `password` 字段，避免覆盖已保存的掩码密码。
    - `handleLoginAndFinish`：最终登录。重新校验账号密码存在性，调用 `onLogin()`。成功后标记 `campus-onboarding-done` 并关闭向导。
- **边界条件**：
    - 密码处理：区分明文输入与后端掩码态 (`PASSWORD_MASK`)。
    - 定时器清理：在 `useEffect` cleanup 中清除 `bindTimerRef` 和 `finishTimerRef`。

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