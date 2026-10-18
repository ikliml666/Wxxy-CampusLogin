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

`tauri-plugin-architecture` 定义了 Campus Login Assistant Android 应用的核心交互架构。该架构基于 Tauri v2 插件系统，采用“Rust 桥接 + Kotlin 原生实现”的双轨模式，通过 IPC (Inter-Process Communication) 命令面暴露功能。其核心职责是将复杂的 Android 系统级操作（如 Wi-Fi 绑定、前台保活、生物识别密钥存储）封装为标准化的异步 API，并通过前端的 `tauriApi` 统一入口进行调用，同时处理网络不稳定下的重试逻辑与安全门控。

## Key Components

### Frontend Hooks & Store (`android/frontend/src/hooks/tauriApi.ts`)
构建安卓端 Tauri IPC 适配层。将 Rust 后端命令映射为前端调用，并处理桌面端专属功能在安卓端的降级。提供带重试机制的 API 包装。

*   **核心常量与类型**：
    *   `desktopOnly<T>(name: string)` (L17-L18): 返回 `Promise.reject` 的辅助函数，用于标记安卓不支持的功能。
    *   `noopListener` (L21): 返回空函数的辅助函数，用于安卓端无对应事件的监听器。
    *   `TauriApi` 接口 (L36-L140): 定义所有可用的 IPC 方法签名。包含配置、网络、认证、账号、监控、更新等模块。
    *   `AvoidBadWifiStatus` / `RestoreWrittenSettingsResult` (L154-L169): 安卓特有的系统设置读写状态结构。
    *   `ALI_DNS` / `RECOMMENDED_DNS`: 推荐 DNS IP 集合（逻辑关联 `useDnsSetup`）。

*   **关键实现细节**：
    *   **事件监听封装 `createEventListener`** (L174-L195):
        *   解决 Tauri `listen` 异步注册问题。返回清理函数，支持在 `listen` Promise 完成前取消订阅（通过 `cancelled` 标志和 `.then` 链式清理）。
        *   错误处理：DEV 模式下打印失败日志，但不阻断主流程。
    *   **生物识别验证门 `verifyWindowsIdentity`** (L208-L240):
        *   **双链路回退逻辑**：优先检查 `shouldUseFaceFallback()`。若开启且系统生物识别不可用（国产平板 Class 1 限制），则调用 `useFaceDialogStore.openFaceDialog` 进行 2D 人脸验证。
        *   **统一上报**：无论成功（`biometric` 或 `face2d`）还是失败，均通过 `invoke('verify_biometric_identity', { success, method })` 上报后端，失败时归一化为 `{success: false}` 而非抛出异常。
        *   **异常隔离**：`biometricCheckStatus` 查询异常不阻断主流程，直接落回系统 BiometricPrompt 链路。
    *   **外部链接打开 `openExternal`** (L288-L300):
        *   严格校验 URL 协议（仅 http/https）、长度（<2048）及格式。
        *   调用 `@tauri-apps/plugin-opener`。
    *   **重试机制 `withRetry` & `tauriApiWithRetry`** (L303-L329):
        *   `isRetryableError`: 识别包含 'timeout', 'network', 'fetch', 'connection' 的错误。
        *   `withRetry`: 指数退避重试（基础延迟 500ms，最大 2 次重试）。
        *   **差异化重试策略**：仅 `saveConfig` 启用重试（一次性关键操作）。`checkPortalStatus` 和 `checkNetworkQuality` 因高频调用，由业务层自行兜底，避免放大流量。

### Network Module (`android/frontend/src/network/useDnsSetup.ts`)
DNS DoH 检测与优化的一键操作 Hook。

*   **关键常量**：
    *   `ALI_DNS`, `TENCENT_DNS`, `RECOMMENDED_DNS` (L12-L14): 定义推荐的公共 DNS IP 集合，用于判定“是否推荐 DNS”及“DoH 是否可用”。
*   **关键状态与方法**：
    *   `handleCheckDns` (L38-L58):
        *   调用 `ipc.checkDnsDohStatus()`。
        *   **逻辑判断**：检查是否有适配器使用推荐 DNS (`hasRecommendedDns`) 以及推荐 DNS 是否开启了 DoH (`dohNotEnabled`)。
        *   **Toast 反馈**：未使用推荐 DNS 提示 warning；使用了但未开 DoH 提示 warning。
    *   `handleSetupDnsDoh` (L60-L82):
        *   调用 `ipc.setupDnsDoh(dnsFamily)`。
        *   **刷新机制**：成功后立即重新调用 `checkDnsDohStatus` 以刷新 UI 状态（独立 try-catch，防止失败覆盖成功 Toast）。
        *   **挂载保护**：全程使用 `mountedRef` 防止组件卸载后 setState。

### Auth & Dashboard (`android/frontend/src/auth/DashboardPanel.tsx`)
校园网总览面板组件。支持卡片化布局、自定义卡片注入、拖拽排序、编辑模式。

*   **核心数据结构**：
    *   `CardDef` / `ExtraCardDef` (L36-L48): 定义内置卡与宿主注入卡的接口。`ExtraCardDef` 包含 `render` 函数用于动态渲染内容。
    *   `ALL_CARDS` (L50-L56): 内置卡片列表（quickActions, accountManage, selfOnline, selfLog, networkQuality）。
    *   `DEFAULT_LAYOUT` (L58): 默认卡片顺序。
    *   `EXTRA_REMOVED_KEY` (L67): 本地存储 Key，记录被用户显式删除的自定义卡片 ID（黑名单机制，防止迁移复活）。

*   **关键子组件与 Hooks**：
    *   **`QuickActionsCard`**: DHCP 续租 (`onDhcpRenew`) 和释放重获 (`onDhcpReleaseRenew`)。使用 `useAsyncLock` 防止重复点击。
    *   **`AccountManageCard`**: 账号切换。使用 `OnlineDeviceStack` 堆叠展示账号卡片。切换时使用 `switchTimerRef` 延迟清除 loading 状态以配合 StrictMode。
    *   **`NetworkQualityCard`**: 显示网关/外网延迟。使用 `resolveQualityDisplay` 解析质量等级。
    *   **`SelfOnlineCard` / `SelfLogCard`**:
        *   **隐私掩码**：共用 `useSelfCardReveal` Hook。需通过 `useHelloGate` (生物识别/密码验证) 才能查看明文信息（IP/MAC/时间）。
        *   **数据获取**：共用 `useSelfCardFetch` Hook。自动根据账号变化触发查询，支持手动刷新。
        *   **轮询**：`SelfOnlineCard` 在凭据齐备时启动 60s 轮询 (`setInterval`) 以自动下线过期设备。
        *   **别名管理**：`SelfOnlineCard` 内嵌设备别名修改功能，直接写入 `config.deviceAliases`。
    *   **`EditableCardItem`** (L350-L418):
        *   **自研长按拖拽**：不使用 framer-motion drag（WebView 兼容性问题）。
        *   **交互逻辑**：长按 350ms (`LONG_PRESS_MS`) 激活 -> 跟随手指移动 (`dragDy`) -> 越过阈值触发 `onSwap` -> 振动反馈。
        *   **滚动冲突处理**：激活前移动 >10px 取消；激活后 `preventDefault` 阻止浏览器滚动。
    *   **`DashboardPanel`** (主组件):
        *   **布局持久化**：`loadLayout` / `saveLayout` 使用 `safeStorage`。
        *   **合并空态**：移动端无自助凭据时，`selfOnline` 和 `selfLog` 合并为 `SelfServiceMergedEmptyCard`。
        *   **编辑模式**：允许添加/删除/排序卡片。删除自定义卡片时加入黑名单。

### Auth Hook (`android/frontend/src/auth/useAuth.ts`)
认证相关的 UI 辅助 Hook。

*   **关键方法**：
    *   `handleOpenPortal` (L18-L21): 打开门户登录页（默认 `http://10.1.99.100`，可配置）。
    *   `handleOpenSelfService` (L23-L25): 硬编码打开自助服务页面 (`http://10.1.80.200:8080/Self/login/?302=LI`)。

### Account Stack (`android/frontend/src/account/SelfOnlineStack.tsx`)
提供设备在线状态的可视化堆叠组件，分为手机端竖排（`OnlineDeviceStack`）和平板/桌面端横排（`OnlineDeviceStackH`），以及单张卡片内容渲染（`OnlineDeviceStackCard`）。核心交互为“整副牌连续滚动”，所有卡片常挂载，通过 `transform` 切换位置。

*   **常量与配置**：
    *   `PEEK_TOP = 44`, `PEEK_BOTTOM = 44`：竖排露出边高度。
    *   `DRAG_THRESHOLD = 60`：拖拽翻页阈值。
    *   `DRAG_MIN = -110`, `DRAG_MAX = 70`：竖排拖拽行程上限。
    *   `V_SIDE_SCALE = 0.75`：竖排侧卡缩放比例。
    *   `CF_X = [60, 85]`：横排相邻/次邻偏移百分比。
    *   `SIDE_SCALE = 0.6`, `SIDE_SCALE_2 = 0.45`：横排侧卡缩放。
    *   `NARROW_MIN_W = 213`：窄模式最小宽度。
    *   `SLIDE_MS = 500`, `EASE = 'cubic-bezier(0.22, 1, 0.36, 1)'`：动画参数。

*   **`OnlineDeviceStack` (竖排)**：
    *   **Props**：`cards: React.ReactNode[]`, `variant?: 'coverflow' | 'narrow'`, `focusIndex?: number`。
    *   **状态管理**：`index` (当前页), `dragY` (拖拽位移), `dragging` (是否拖拽中), `cardH` (舞台高度)。
    *   **关键逻辑**：
        *   `useEffect` 监听 `count` 变化，防止列表变短时 `index` 越界 (`Math.min(i, count-1)`)。
        *   `useEffect` 监听 `focusIndex`，外部触发时直接设 `index`。
        *   **量高逻辑**：使用 `ResizeObserver` 或 fallback 一次性查询 `stage.querySelectorAll` 计算最大高度，确保舞台不跳变。
        *   **手势处理**：`onPointerDown/Move/Up/Cancel`。`suppressClick` 防止拖拽后误触点击。
        *   **点击分区**：以中线划分，上半屏点击翻上一张，下半屏翻下一张。
        *   **姿势计算 (`rel`)**：基于 `index` 计算相对位置 `r`。
            *   `r=0`: 当前卡，`top=PEEK_TOP`, `scale=1`, `z=30`, `opacity=1`。
            *   `r=1`: 下一张，从顶部露出，`top=0`, `scale=V_SIDE_SCALE`, `opacity=0.5`。
            *   `r=count-1`: 上一张，从底部露出，`bottomTop` 动态计算对齐底边。
            *   更深的卡透明隐藏 (`opacity=0`)，仅 `count > 3/4` 时挂载。
        *   **拖拽样式**：当前卡拖拽时 `transition: none`，应用 `calc()` 合并初始位与拖拽值。

*   **`OnlineDeviceStackCard`**：
    *   **Props**：`loginTime`, `ip`, `mac`, `flowMb`, `durationMin`, `terminalType`, `alias?`, `onRename?`, `offlineDisabled`, `offlineLoading`, `onOffline?`。
    *   **UI结构**：头部（时间、别名标签、终端类型标签、编辑按钮）、中部（IP/MAC 垂直芯片）、底部（流量/时长统计、注销按钮）。
    *   **细节**：深色模式反色处理 (`dark:bg-muted dark:ring-white/[0.06]`)；别名截断显示；注销按钮带加载态。

*   **`OnlineDeviceStackH` (横排)**：
    *   **Props**：同竖排，但默认 `variant='coverflow'`。
    *   **差异**：
        *   拖拽方向为 X 轴 (`dragX`)，阈值 `DRAG_CLAMP = 80`。
        *   **姿势表 (`sd`)**：
            *   `s=0`: 中央，`x=0`, `scale=1`, `z=30`。
            *   `s=1`/`s=count-1`: 相邻，`x=±CF_X[0]`, `scale=0.6`, `opacity=0.7`。
            *   `s=2`/`s=count-2`: 次邻，`x=±CF_X[1]`, `scale=0.45`, `opacity=0.45`。
            *   `count >= 6` 时，`s=3`/`s=count-3` 透明隐藏。
        *   **定位差异**：`coverflow` 使用 `left/right` 百分比留白；`narrow` 使用 `left: 50%` + `translateX(-50% + offset)` 居中，最小宽 `NARROW_MIN_W`。
        *   **CSS Hack**：横排居中需将 `-50%` 与百分比偏移合并进 `calc()`，否则浏览器会丢弃裸求和。

### Self Service State (`android/frontend/src/account/selfServiceState.ts`)
管理生物识别验证门（Hello Gate）和自助服务会话门（Session Gate），控制敏感操作的权限校验。

*   **`biometricFailMessage(err, t)`**：
    *   根据错误对象中的 `code` 字段映射到 i18n key。
    *   映射表 `BIOMETRIC_ERROR_KEY` 包含：`userCancel`, `systemCancel`, `biometryNotEnrolled`, `noDeviceCredential`, `faceTimeout`, `faceChallenge` 等。
    *   未知错误回退 `extractErrorMessage` 或默认文案。

*   **`useHelloGate(options?)`**：
    *   **用途**：绑定运营商等操作前的验证门。
    *   **逻辑**：
        *   检查 `gateFresh(helloGateVerifiedAt)`：若 TTL 内且时钟未回拨，直接放行。
        *   检查总开关 `helloEnabled()`：若关闭则放行（除非 `ignoreToggle=true`）。
        *   调用 `tauriApiWithRetry.verifyWindowsIdentity`。成功则更新 `helloGateVerifiedAt = Date.now()`。
        *   失败则 Toast 提示并返回 `false`。
    *   **TTL**：`VERIFY_TTL_MS = 570_000` (570s)，对齐后端 600s 留余量。
    *   **时钟回拨保护**：`elapsed >= 0` 确保不因系统时间调整导致永真。

*   **`useSelfServiceVerify()`**：
    *   **用途**：自助服务面板内的操作验证。
    *   **逻辑**：
        *   独立于 Hello Gate，使用 `selfSessionVerifiedAt`。
        *   支持配置项 `config.selfReverifyEachAction`：若为 `true`，每次操作都强制验证；否则沿用 TTL 缓存。
        *   同样受 `helloEnabled()` 总开关控制。
    *   **`resetSelfSessionGate()`**：手动重置会话门（用于面板卸载时）。

### Settings Panel (`android/frontend/src/settings/SettingsPanel.tsx`)
设置面板 UI，包含启动、自动化、安全、网络质量等模块。安卓端差异化渲染（如保活设置、无退出应用选项）。

*   **结构**：两列布局 (`md:grid-cols-2`)，左侧启动/自动化/保活，右侧安全/网络。
*   **关键功能块**：
    *   **新手指引**：置顶卡片，引导用户进入 Onboarding。
    *   **启动与通知**：自动启动 (`autoLaunch`)、开机自动登录 (`autoLoginOnStart`)、通知开关 (`enableNotification`)。通知开启时请求权限。
    *   **自动化**：准备就绪自动登录 (`autoLoginOnPreparation`)、夜间运营商切换 (`enableNightOperatorSwitch`)。
    *   **保活设置**：仅在 Android 下渲染 (`KeepAliveSettingsCard`)。
    *   **安全设置**：
        *   **关闭保护需验证**：`selfHelloEnabled` 和 `selfReverifyEachAction` 从 True 切 False 时，调用 `handleSecurityDisable`，该函数内部先调 `verifyWindowsIdentity`，成功才执行 `apply(true)`。
        *   **2D 人脸**：开启时需确认风险弹窗 (`ConfirmDialog`)；关闭时清除模板 (`clearTemplate()`)。已录入时显示录入/重录按钮。
    *   **网络质量**：
        *   开关 `enableNetworkQuality`。关闭时联动清理：停止延迟测试 (`stopLatencyTest`)、重置 `defaultPanel` 避免空白、切换 `activePanel` 到 dashboard。
        *   **延迟计算选项**：弹窗 (`latencyModalOpen`)，包含跳过 TTFB (`skipTtfbInLatency`) 和跳过内容传输 (`skipContentInLatency`) 的开关及解释说明。
    *   **背景看板娘**：底部低透明度装饰图。

### Onboarding Flow (`android/frontend/src/settings/useOnboardingFlow.ts`)
新手指引向导的状态机 Hook，统一安卓手机全屏向导和平板 Dialog 向导的逻辑。

*   **状态机**：4 步 (`ONBOARDING_STEP_COUNT = 4`)：欢迎 -> 账号 -> 绑定运营商 -> 完成。
*   **关键 State**：
    *   `step`, `direction` (滑动方向)。
    *   账号信息：`username`, `password`, `operator`。
    *   绑定信息：`bindOperatorValue`, `phone`, `smsPassword`, `bindState` (`idle/loading/success/error`)。
    *   辅助：`showCloseConfirm`, `loginSuccess`。
*   **核心逻辑**：
    *   **初始化**：`open` 变为 `true` 时重置状态，读取 `config` 填充表单。
    *   **绑定验证**：`canBind` 检查用户名、密码（或掩码）、运营商、手机号格式、短信密码非空。
    *   **绑定操作**：`handleBind` 先调 `ensureBindHello` (Hello Gate)，成功后进入 `success` 状态，延迟 1.2s 进入下一步。
    *   **账号保存**：`handleNext` 在步骤 1 落盘 `user`, `operator`, `password` (仅当输入了新密码时)。
    *   **完成登录**：`handleLoginAndFinish` 最终校验账号，落盘配置，调用 `onLogin()`。成功则延迟 1.5s 标记 `campus-onboarding-done` 并关闭向导。
    *   **哨兵值**：`BIND_OPERATOR_NONE = '__none__'`, `DEFAULT_OPERATOR = '__default__'`。

### Keystore Plugin (`android/plugins/keystore/...`)
Android KeyStore AES-GCM 加解密插件，用于密码等敏感数据落盘加密。

*   **Kotlin (`KeystorePlugin`)**：
    *   **Key 管理**：`getOrCreateKey()` 使用 `KeyGenParameterSpec` 生成 256-bit AES Key，Alias `"campus_login_master"`。
    *   **Commands**：
        *   `encrypt(text)`: 返回 Base64(`IV + Ciphertext`)。
        *   `decrypt(data)`: 解析 Base64，提取 IV (前12字节) 和解密。
    *   **异常处理**：捕获异常并 Log，返回 JSObject 错误。

*   **Rust (`lib.rs`)**：
    *   **Struct**: `CampusKeystore<R>`。
    *   **Methods**: `encrypt(&str) -> Result<String>`, `decrypt(&str) -> Result<String>`。
    *   **Integration**: 通过 `run_mobile_plugin` 调用 Kotlin 插件。提供 `CampusKeystoreExt` trait 方便在 Tauri App 中获取实例。

### Monitor Service Plugin (`android/plugins/foreground-service/...`)
前台服务保活、探针窗口锁管理、电池优化/自启设置跳转、APK 安装。

*   **Kotlin (`MonitorServicePlugin`)**：
    *   **前台服务控制**：
        *   `startMonitor(title)`: 启动 `ForegroundService`，处理 `IllegalStateException` (部分 ROM 限制)。
        *   `stopMonitor()`: 发送 Action 让服务自杀。
        *   `updateNotification(text)`: 直接通过 `NotificationManager` 更新通知，与服务侧协议一致。
    *   **探针窗口**：`beginProbeWindow` / `endProbeWindow` 静态调用 `ForegroundService` 方法，持有 WifiLock/WakeLock 期间禁止 WiFi 省电。
    *   **电源状态**：`getPowerState` 返回 `screenOn` 和 `wifiConnected`，供 Rust 巡检分档。
    *   **厂商适配**：
        *   `vendorTargets`: 映射品牌 (Xiaomi, Huawei, OPPO, vivo, Samsung 等) 到具体的自启/省电页面 ComponentName。
        *   `openVendorBatterySettings`: 降级链：厂商专用页 -> 应用详情页 -> 通用设置。返回 `path` 和 `tried` 列表供前端提示。
        *   `openNotificationSettings`: 降级链：API 26+ Action -> 旧版 Action -> 应用详情 -> 通用设置。
    *   **电池优化**：`getBatteryOptimizationInfo` 返回是否忽略优化、品牌、是否有专用页。`requestIgnoreBatteryOptimizations` 弹出系统确认框。
    *   **APK 安装**：`installApk`：从 `filesDir/update/` 取文件名，构建 FileProvider URI，启动系统安装器。防路径逃逸。
    *   **开机自启**：`setBootAutostart` / `isBootAutostartEnabled`：操作 `BootReceiver` 组件启用状态及 SharedPreferences。
    *   **`BootReceiver`**：接收 `BOOT_COMPLETED`，若开启自启则尝试启动 FGS 和 MainActivity (best-effort)。

*   **Rust (`lib.rs`)**：
    *   **Struct**: `CampusMonitorService<R>`。
    *   **Methods**:
        *   `start_monitor`, `stop_monitor`, `update_notification`.
        *   `begin_probe_window`, `end_probe_window`.
        *   `get_power_state` -> `(bool, bool)`.
        *   `set_boot_autostart`, `is_boot_autostart_enabled`.
        *   `install_apk`.
        *   `get_battery_optimization_info`, `request_ignore_battery_optimizations`.
        *   `open_vendor_battery_settings`, `open_notification_settings`.
    *   **Integration**: 注册 `com.campuslogin.plugin.monitorservice.MonitorServicePlugin`。

### Network Bind Plugin (`android/plugins/network-bind/...`)
Android 原生网络绑定插件，核心解决校园网 Captive Portal（认证前无网）下的进程级流量劫持问题。提供 WiFi 绑定、SSID 读取、系统设置读写及 WiFi 状态监听能力。

*   **Kotlin (`NetworkBindPlugin`)**：
    *   **`bindToWifi(invoke: Invoke)`** (行 73):
        *   **逻辑**：先遍历 `allNetworks` 筛选 WiFi。优先选择带 `NET_CAPABILITY_INTERNET` 的网络；若为 Captive Portal（无 INTERNET），退而求其次选任意 WiFi。
        *   **直绑**：调用 `manager.bindProcessToNetwork(picked.first)`。成功则返回 `{bound: true, path: "allNetworks"}`。
        *   **回退**：若直绑失败（含 VPN 活跃导致 `false`），构造 `NetworkRequest`（显式 `.removeCapability(NET_CAPABILITY_INTERNET)` 以防 AOSP 拒绝），通过 `requestNetwork` 等待连接。
        *   **竞态保护**：使用 `AtomicBoolean` 确保单次调用只 resolve 一次。记录 `lastBoundNetwork` 避免重复清池。
        *   **返回值**：`{bound, path, reason}`。path 区分 `allNetworks` / `requestNetwork` / `already_bound`。
    *   **`unbind(invoke: Invoke)`** (行 136):
        *   调用 `bindProcessToNetwork(null)` 恢复默认路由，注销回调。
    *   **`acceptWifiNetwork(invoke: Invoke)`** (行 154):
        *   **目的**：静默接受无互联网能力的 WiFi（免用户点击“仍然连接”）。
        *   **路径 1**：探测 Hidden API `setAcceptUnvalidated` 是否可用（反射检查方法存在性，行 208）。
        *   **路径 2**：若网络已 `VALIDATED`，直接返回 `accepted=true`。
        *   **路径 3**：尝试反射调用 `ConnectivityManager.setAcceptUnvalidated(network, true, true)`（行 218）。成功则 `path="hidden_api"`。
        *   **路径 4**：失败则写 `Settings.Global`（`captive_portal_mode=0`, `network_avoid_bad_wifi=0`），需 `WRITE_SECURE_SETTINGS` 权限（行 240）。
        *   **快照机制**：写入前通过 `snapshotGlobalKey` 记录原值到 `SharedPreferences`（key: `prev_*`），用于后续还原。
    *   **`startWifiWatcher` / `stopWifiWatcher`** (行 306, 329):
        *   注册 `NetworkCallback` 监听 WiFi `available`/`lost` 及 `capabilitiesChanged`。
        *   仅在 `NET_CAPABILITY_VALIDATED` 状态翻转时触发 `trigger("wifiChanged", ...)`，避免高频信号变化噪音。
    *   **`getWifiSsid(invoke: Invoke)`** (行 348):
        *   API 33+ 依赖 `NEARBY_WIFI_DEVICES` 权限（不弹框，仅查状态）；API <33 恒返回 `granted=false`。
        *   底层通过 `WifiManager.connectionInfo?.ssid` 读取，去除两侧引号。
    *   **`reportWifiUnusable(invoke: Invoke)`** (行 377):
        *   调用 `manager.reportNetworkConnectivity(wifi, false)` 触发系统重检，促使系统可能切换至蜂窝。
    *   **内部辅助**：
        *   `describe(caps)` (行 398)：生成能力摘要字符串如 `net+val+cp`。
        *   `hasVpn(manager)` (行 404)：检测是否有 VPN Transport，解释直绑失败原因。
        *   `ensureAvoidBadWifi` / `restoreAvoidBadWifi` / `restoreWrittenSettings` (行 276-340)：程序化控制 `network_avoid_bad_wifi` 开关及还原系统设置，配合 `guardPrefs` 进行快照管理。

*   **Rust (`lib.rs`)**：
    *   **`CampusNetworkBind<R>` 结构体**：持有 `PluginHandle<R>`。
    *   **`bind_to_wifi(&self)`** (行 23)：调用 Kotlin `bindToWifi`，返回 `serde_json::Value`。
    *   **`accept_wifi_network(&self)`** (行 29)：调用 Kotlin `acceptWifiNetwork`。
    *   **`start_wifi_watcher` / `stop_wifi_watcher`** (行 35, 46)：
        *   注册 Listener Channel (`registerListener`) 并启动 Watcher。
        *   停止时先移除 Listener，再调用 Kotlin `stopWifiWatcher`。
    *   **`get_wifi_ssid` / `request_wifi_ssid_permission` / `report_wifi_unusable`** (行 58-76)：
        *   均为 JNI 阻塞调用，文档注释强调调用方须走 `spawn_blocking`。
    *   **`ensure_avoid_bad_wifi` / `restore_avoid_bad_wifi` / `restore_written_settings` / `get_secure_settings_status`** (行 82-108)：
        *   对应 Kotlin 的设置读写与状态查询命令。
    *   **`init<R>()`** (行 114)：
        *   注册 Android 插件 ID `com.campuslogin.plugin.networkbind` 映射到 `NetworkBindPlugin`。

### Config State (`android/src-tauri/src/config_state.rs`)
安卓端配置持久化、加密存储、版本迁移及 IO 锁管理。结构与桌面端 `Config` 对齐。

*   **`Settings` 结构体 (行 36)**：
    *   包含账号密码、夜间切换开关、巡检间隔、SSID 名单等所有业务配置。
    *   默认值定义在 `impl Default` (行 68)，注意 `background_check_interval` 默认 60s，`latency_test_interval` 默认 600s。
*   **`CryptoBridge` (行 133)**：
    *   抽象加密接口。移动端调用 `tauri_plugin_campus_keystore` (AndroidKeyStore AES-GCM)；Host 测试环境注入假加密。
*   **`load_file` / `save_file`** (行 156, 169)：
    *   **加密落盘**：密码字段单独提取为 `password_cipher`，明文置空后序列化。
    *   **原子写入**：使用带 PID 的临时文件 `.json.<pid>.tmp`，写完 rename，防止崩溃残留覆盖。
    *   **解密容错**：解密失败时密码置空，其他字段正常加载。
*   **`migrate_legacy_defaults`** (行 196)：
    *   基于 `config_schema_version` (当前 7) 执行一次性迁移。
    *   v0→v1: 后台检测 15s→60s。
    *   v2: 自动化登录/验证默认开启。
    *   v3: 质量检测默认关闭。
    *   v4: 补闲时巡检间隔 300s。
    *   v5: 检测终点默认 23:00。
    *   v6: 质量测试间隔 60s→600s。
    *   v7: 夜切默认开启，定时动作禁用哨兵 1440。
    *   **幂等性**：仅当值等于旧默认值或特定迁移条件满足时才修改，用户手动修改的值不被覆盖。
*   **`save_config`** (行 266)：
    *   全程持有 `CONFIG_IO_LOCK` (`tokio::sync::Mutex`)，防止与账号切换命令并发冲突。
    *   合并密码字段：若前端传掩码 `***` 或空串，保留旧密码；否则更新。
    *   保存后广播 `config-changed` 事件。
*   **`masked_for_display`** (行 253)：出站唯一出口，将非空密码替换为 `***`。
*   **`resolve_password_field`** (行 260)：处理密码回退逻辑。

### Campus Detect (`android/src-tauri/src/campus_detect.rs`)
安卓侧校园网连通性探针。替代桌面的 ICMP Ping，改用 TCP 连接探测 Portal 和网关，并结合 SSID 判定。

*   **`pick_campus_source_ip`** (行 23)：
    *   枚举网卡，排除蜂窝 (`rmnet*`, `ccmni*`)、Link-local、Loopback。
    *   优先选 `wlan0`，其次选其他合法 IPv4。
*   **`portal_reachable`** (行 37)：
    *   调用 `campus_login_lib::network::bound_socket::tcp_connect_bounded` 进行 TCP 连接测试（超时 3s）。
*   **`probe_campus`** (行 52)：
    *   **短路判断**：若无非蜂窝 IPv4 (`source.is_none()`)，直接返回 `wifi_off=true`。
    *   **子网判定**：复用桌面纯函数 `is_same_subnet_18`。
    *   **TCP 探测**：
        *   若在子网内：并行探测 Portal TCP。
        *   若不在子网内：并行探测 Portal TCP 和 Gateway TCP。
    *   **结果**：`on_campus = subnet || gateway_ok || portal_ok`。
*   **`probe_campus_with_ssid`** (行 142)：
    *   包装 `probe_campus`。若开启 `enable_network_name_check`，异步获取 SSID 并匹配。
    *   名称命中则强制 `on_campus=true`，跳过 TCP 探测优化。
*   **`check_campus_status`** (行 166)：
    *   Tauri Command。先调用 `protocol_cmds::ensure_wifi_bound` 确保绑定。
    *   返回 JSON：`onCampusNetwork`, `currentSsid`, `campusMessage` (WiFi未连接/已连接/未连接), `sourceIp` 等。
*   **`ssid_matches`** (行 115)：
    *   大小写不敏感匹配。特殊处理默认名 `i-wxxy` 兼容历史同源 `iwxxy-2/3`。

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