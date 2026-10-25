---
title: zustand-global-state
type: concept
source_files:
  - android/frontend/src/hooks/useQualityStore.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/hooks/tauriApi.ts
  - android/frontend/src/hooks/useAdapterStore.ts
  - android/frontend/src/hooks/useAppStore.ts
  - android/frontend/src/hooks/useAuthStore.ts
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useLogToastStore.ts
tags:
  - state-management
  - zustand
  - android-frontend
  - hooks
---

## Overview

`zustand-global-state` 是 Android 前端应用的核心状态管理范式，基于 `zustand` 库构建，由 `android/frontend/src/hooks/` 目录下一系列原子化领域 Store 及其配套 Hook 组成。该概念定义了数据在后端 Tauri IPC、本地持久化（`safeStorage`）与 React UI 组件之间的流动方式，是连接底层 Rust 命令与上层业务（认证、配置、网络质量、主题、日志通知）的基础设施。

当前代码核心特征：
1. **细粒度订阅**：各 Store 通过 `create<Interface>()` 暴露状态切片，UI 使用选择器订阅避免全局重渲染。
2. **脏数据冲突解决**：`useConfigStore` 引入 `dirtyFields` 集合（L20）与 `DIRTY_FAILURE_LIMIT=3` 连续失败计数（L24-25），防止后端旧快照覆盖本地未确认修改（L103-119, L151-162）。
3. **平台适配隔离**：`tauriApi.ts` 中以 `desktopOnly` 拦截器（L14-15）和 `noopListener`（L18）屏蔽桌面专属能力（适配器/DHCP/窗口/托盘等），确保安卓端 Store 稳定。
4. **生命周期保护**：异步锁（`_checkOnlineLockFlag` L16、`_qualityLockFlag` L11）串行化执行；`saveConfigInFlight` Promise 引用跟踪（L17, L168-175）保证关窗时等待保存完成；登录/注销 `withTimeout`（L69-78）防止按钮永久禁用。
5. **质量探测节流**：模块级 `lastQualityResultTime`（L16）与 `QUALITY_MANUAL_THROTTLE_MS=60_000`（L21）避免登录后与后端 latency loop 重复全量检测（历史缺陷 P2-F5）。

## Key Components

### android/frontend/src/hooks/useQualityStore.ts

- 模块级变量：
  - `_qualityLockFlag` (L11): `boolean`，刷新质量时的并发锁。
  - `lastQualityResultTime` (L16): `number`，最近一次质量结果到达时间戳（0 表示从未），用于登录后手动探测节流。
- 函数 `getLastQualityResultTime` (L18-20): 返回 `lastQualityResultTime`。
- 接口 `QualityStore` (L22-41) 全部字段与方法：
  - `networkQuality: NetworkQuality | null`
  - `dnsDohStatus: DnsDohStatus | null`
  - `dnsChecking: boolean`
  - `isRefreshingQuality: boolean`
  - `updateAvailable: boolean`
  - `latestVersion: string`
  - `releaseNotes: string`
  - `updatePromptOpen: boolean`
  - `refreshRate: number`
  - `refreshQuality: () => Promise<void>`
  - `setNetworkQuality: (q: NetworkQuality | null | ((prev: NetworkQuality | null) => NetworkQuality | null)) => void`
  - `setDnsDohStatus: (s: DnsDohStatus | null) => void`
  - `setDnsChecking: (v: boolean) => void`
  - `setUpdateAvailable: (v: boolean) => void`
  - `setLatestVersion: (v: string) => void`
  - `setReleaseNotes: (v: string) => void`
  - `setUpdatePromptOpen: (v: boolean) => void`
- `useQualityStore` (L43-89): `create` 实例。
  - `refreshQuality` 实现 (L54-74)：检查 `config.enableNetworkQuality`，加锁 500ms 后释放，调用 `api.checkNetworkQuality?.()` 并走 `setNetworkQuality` 合并。
  - `setNetworkQuality` (L78-82)：函数式更新，若结果非 null 则写 `lastQualityResultTime = Date.now()`。
  - 其余 setter (L83-88) 直接 `set` 对应字段。

### android/frontend/src/hooks/useThemeStore.ts

- 接口 `ThemeStore` (L9-19) 全部字段与方法：
  - `themeName: ThemeName`
  - `isLightMode: boolean`
  - `customThemeColor: string`
  - `glassLevel: GlassLevel`
  - `setThemeName: (name: ThemeName) => void`
  - `setIsLightMode: (v: boolean) => void`
  - `setGlassLevel: (v: GlassLevel) => void`
  - `initTheme: (cfg: Partial<Config>) => void`
  - `setCustomThemeColor: (color: string) => void`
- `useThemeStore` (L21-57)：初始状态从 `safeStorage` 读取（L23-28）。
  - `initTheme` (L34-54)：依据 `safeStorage` 与 `cfg.themeMode` 设定主题/浅色模式并持久化。
  - `setCustomThemeColor` (L56)。
- 副作用订阅 (L60-98)：监听 `glassLevel`/`isLightMode`/`themeName`/`customThemeColor` 变化，操作 `document.documentElement` 的 class、data 属性与 CSS 变量（使用 `setProperty` 覆盖，修复累积缺陷 L82-87）。

### android/frontend/src/hooks/tauriApi.ts

- 类型定义：
  - `ConnectionCampusStatus` (L20-24): `onCampus: boolean`, `name: string | null`, `message: string`
  - `BatteryOptimizationInfo` (L26-30): `ignoring: boolean`, `brand: string`, `hasVendorTarget: boolean`
  - `VendorSettingsResult` (L32-36): `path: 'vendor' | 'vendor_action' | 'app_details' | 'settings' | 'none'`, `target: string`, `tried: string[]`
  - `CampusStatusResult` (L38-46): `onCampusNetwork`, `currentSsid`, `campusMessage`, `enableNetworkNameCheck`, `requiredNetworkName`, `campusWifi`, `campusWired`
  - `AvoidBadWifiStatus` (L153-162): `granted`, `avoidBadWifi`, `pendingRestoreAvoid`, `pendingRestoreCaptive`
  - `RestoreWrittenSettingsResult` (L164-167): `restored: Record<string, boolean>`, `reason?: string`
  - 接口 `TauriApi` (L48-151): 所有后端命令方法签名（含安卓可用与桌面专属）。
- 辅助函数/常量：
  - `desktopOnly<T>` (L14-15)：返回 rejected Promise 拦截桌面专属调用。
  - `noopListener` (L18)：返回 `() => {}` 空清理函数。
  - `createEventListener<T>` (L169-198)：安全封装 Tauri `listen`，支持取消与错误处理。
  - `isRetryableError` (L341-351)：判断错误是否可重试。
  - `withRetry` (L353-369)：指数退避重试封装。
- `tauriApi` 对象 (L200-339)：实现 `TauriApi`。桌面专属方法（如 `exportConfig` L203, `getAdapters` L206, `minimizeWindow` L265, `dhcpRenewAll` L284 等）指向 `desktopOnly` 或 `noopListener`；安卓可用方法用 `invoke`。
- `tauriApiWithRetry` (L371-377)：展开 `tauriApi`，仅对 `saveConfig` 包装 `withRetry`（注释明确定义原因 L373-376）。

### android/frontend/src/hooks/useAdapterStore.ts

- 接口 `AdapterStore` (L6-9)：`activePanel: PanelName`, `setActivePanel: (p: PanelName) => void`。
- `useAdapterStore` (L11-14)：安卓端网卡数据链已删除，仅保留面板导航状态（默认 `'dashboard'`）。

### android/frontend/src/hooks/useAppStore.ts

- 仅 re-export (L1-3)：
  - `export { useAppInit } from './useAppInit'`
  - `export { hasPendingConfig, flushPendingConfig } from './useConfigStore'`

### android/frontend/src/hooks/useAuthStore.ts

- 模块级：
  - `_checkOnlineLockFlag` (L16): `boolean` 串行锁。
  - `QUALITY_MANUAL_THROTTLE_MS` (L21): `60_000` 登录后质量探测节流阈值。
  - 类型 `CampusStatus` (L23), `PortalStatus` (L24) 推导自 api。
- 辅助函数：
  - `detectCampusNetwork` (L29-35)：调 `checkCampusStatus` 失败返 null。
  - `buildCampusBgStatusPatch` (L38-49)：纯函数构造 bgStatus 补丁。
  - `queryPortalStatus` (L56-65)：传空 `adapterIp` 调 `checkPortalStatus`，失败返 `{ok:false}`。
  - `withTimeout` (L69-78)：Promise.race 超时包装，清理定时器。
  - `setStatusStable` (L82-86)：引用稳定更新 status。
- 接口 `AuthStore` (L88-98) 全部字段与方法：
  - `isLoggingIn: boolean`
  - `isLoggingOut: boolean`
  - `status: { text: string; state: StatusState }`
  - `bgStatus: BackgroundStatus`
  - `doLogin: () => Promise<boolean>`
  - `doLogout: () => Promise<void>`
  - `checkOnline: (cfg?: Partial<Config>) => Promise<void>`
  - `setStatus: (s: { text: string; state: StatusState }) => void`
  - `setBgStatus: (s: BackgroundStatus | ((prev: BackgroundStatus) => BackgroundStatus)) => void`
- `useAuthStore` (L100-247)：
  - `doLogin` (L106-164)：带锁、超时登录，成功后按节流条件手动质量探测（L141-147），失败才复查（L159-161）。
  - `doLogout` (L166-194)：类似带超时。
  - `checkOnline` (L196-243)：campus 检测（若启用）、portal 查询，锁在 finally 真正释放（L241）。
  - `setStatus` (L245), `setBgStatus` (L246)。

### android/frontend/src/hooks/useConfigStore.ts

- 模块级：
  - `saveConfigTimer` (L14), `saveConfigPending` (L15), `saveConfigInFlight` (L17), `dirtyFields` (L20), `DIRTY_FAILURE_LIMIT = 3` (L24), `dirtyFailureCounts` (L25)
- 接口 `ConfigStore` (L27-46) 全部字段与方法：
  - `config: Config`
  - `configLoaded: boolean`
  - `passwordSaved: boolean`
  - `accounts: AccountItem[]`
  - `activeAccount: string`
  - `language: string`
  - `updateConfig: (partial: Partial<Config>) => void`
  - `updateConfigLocal: (partial: Partial<Config>) => void`
  - `mergeConfigFromBackend: (incoming: Partial<Config>) => void`
  - `clearDirtyFields: () => void`
  - `syncPasswordSaved: (saved: boolean) => void`
  - `saveConfigDirect: (cfg: Partial<Config>, clearPassword?: boolean) => Promise<void>`
  - `setAccounts: (a: AccountItem[]) => void`
  - `setActiveAccount: (a: string) => void`
  - `setLanguage: (lang: string) => void`
- `useConfigStore` (L48-186)：
  - `updateConfig` (L56-88)：防抖 500ms 保存，标记 dirtyFields，联动主题色。
  - `updateConfigLocal` (L90-99)：仅本地。
  - `mergeConfigFromBackend` (L103-119)：跳过 dirtyFields，同步 passwordSaved。
  - `saveConfigDirect` (L128-176)：in-flight 跟踪，成功清脏标，失败处理 passwordSaved 与失败计数。
  - `hasPendingConfig` (L188-193)：导出函数，检查 pending 或 in-flight。
  - `flushPendingConfig` (L195-224)：导出函数，关窗前刷新待存。

### android/frontend/src/hooks/useLogToastStore.ts

- 模块级：`toastTimers` (L5), `toastIdCounter` (L6), `logIdCounter` (L7), `MAX_TOASTS = 4` (L9), `isDuplicateTitle` (L13)
- 接口 `LogToastStore` (L15-26) 全部字段与方法：
  - `logs: LogEntry[]`
  - `toasts: ToastMessage[]`
  - `addLog: (message: string, type?: LogType) => void`
  - `addToast: (title: string, type?: LogType, description?: string, duration?: number, mascot?: ToastMessage['mascot']) => void`
  - `addToastWithAction: (toast: ToastMessage) => void`
  - `removeToast: (id: string) => void`
  - `removeToastsByPrefix: (prefix: string) => void`
  - `setLogs: (logs: LogEntry[]) => void`
  - `cleanupToasts: () => void`
- `useLogToastStore` (L28-135)：
  - `addLog` (L32-50)：重复内容计数。
  - `addToast` (L52-82)：非前台跳过，同题去重，超限淘汰。
  - `addToastWithAction` (L84-108)：带操作按钮 toast 豁免前台限制。
  - `removeToast` (L110-114), `removeToastsByPrefix` (L116-126), `setLogs` (L128), `cleanupToasts` (L130-134)。

## Architecture

```text
┌──────────────┐   ┌────────────────────┐   ┌─────────────────┐
│ React UI     │──►│ Zustand Stores     │──►│ tauriApi        │
│ Components   │◄──│ (Auth/Config/      │◄──│ (invoke/desktop│
└──────────────┘   │ Quality/Theme/     │   │ Only/noop)      │
                   │ LogToast/Adapter)  │   └────────┬────────┘
                   └────────────────────┘            │
                                                    ▼
                                            ┌─────────────────┐
                                            │ Rust Backend    │
                                            │ (Tauri commands)│
                                            └─────────────────┘
Stores 文件: useAuthStore, useConfigStore, useQualityStore,
useThemeStore, useLogToastStore, useAdapterStore, useAppStore(re-export)
平台隔离: tauriApi.desktopOnly (L14-15) 拒绝桌面调用, noopListener (L18) 空事件
```

## Data Flow

1. **初始化与加载**：`useAppInit`（经 `useAppStore` 重导出 L2）触发；`useThemeStore.initTheme` (L34-54) 从 `safeStorage` 与配置初始化主题 DOM；`useConfigStore` 通过 `tauriApi.getInitData`/`getConfig` 载入配置（代码中由 `configLoaded` 标志 L32 跟踪）。
2. **状态读取**：组件用选择器订阅（如 `useAuthStore(s => s.status)`），Zustand 确保精准重渲染。
3. **状态写入与 IPC**：交互调用 Action（如 `doLogin` L106），Action 内通过 `tauriApiWithRetry` 调后端；`saveConfigDirect` (L128) 经 `tauriApi.saveConfig` 带重试 (L376)。
4. **配置冲突解决**：`updateConfig` (L56) 标记 `dirtyFields` 并防抖保存；后端 `config-changed` 事件经 `mergeConfigFromBackend` (L103) 跳过脏字段；连续失败超 `DIRTY_FAILURE_LIMIT` (L155) 放弃脏标记。
5. **质量探测节流**：登录成功或手动刷新走 `setNetworkQuality` 更新 `lastQualityResultTime` (L80)；登录后探测前检查 `Date.now() - getLastQualityResultTime() > 60_000` (L141)。
6. **日志与通知**：`addToast` 在非前台跳过 (L56)、同题去重 (L61)、`MAX_TOASTS` 淘汰 (L64)；`addLog` 重复计数 (L40)。
7. **生命周期保护**：`checkOnline` 锁在 promise settle 释放 (L241)；`flushPendingConfig` (L195) 等待 in-flight 保存；登录/注销 `withTimeout` (L125, L175) 防死锁。

## Connections

- [[android-frontend-hooks-and-stores]]：本概念核心定义层（所有 Store 与 `tauriApi`）。
- [[android-frontend-app-shell]]：消费 Store 驱动全局 UI（DockNav/TabletShell）。
- [[android-frontend-auth]]：依赖 `useAuthStore` 认证状态与 `useQualityStore` 质量数据。
- [[android-frontend-settings]]：`SettingsPanel` 依赖 `useConfigStore`/`useThemeStore`。
- [[android-frontend-account]]：`AccountPanel` 依赖 `useConfigStore` 账号列表。
- [[android-frontend-monitor]]：`useMonitor` 依赖 `useQualityStore` 与 `useAuthStore`。
- [[android-frontend-entry-config]]：`main.tsx` 初始化 `initTheme` 与配置加载。

## Known Issues

代码中以注释形式记录了历史缺陷及修复（均已在当前代码修复，此处汇总以备追溯）：

- **P2-F5**（重复全量质量探测）：登录后无条件手动探测，与后端 latency loop 重复。修复：引入 `lastQualityResultTime` (L16) 与 `QUALITY_MANUAL_THROTTLE_MS=60s` (L21) 节流（useQualityStore L13-20, L78-82；useAuthStore L18-21, L141-147）。
- **P2-34**（登录/注销按钮永久禁用）：后端阻塞致 `isLoggingIn/Out` 永真。修复：`withTimeout` 60s 包装（useAuthStore L69-78, L125, L175）。
- **主题 inline style 无界增长**：旧版 `cssText +=` 累积。修复：改用 `style.setProperty` 覆盖（useThemeStore L82-87）。
- **checkOnline 锁过早释放**：原 `setTimeout(500)` 释放导致并发冗余。修复：finally 中 promise settle 释放（useAuthStore L236-241）。
- **登录成功状态闪烁**：成功立即 `checkOnline` 被 Portal 会话未传播覆盖。修复：仅失败复查（useAuthStore L156-161）。
- **配置脏字段永久脱节**：保存持续失败致 `dirtyFields` 永久跳过。修复：`DIRTY_FAILURE_LIMIT=3` 失败计数重置（useConfigStore L21-25, L151-162）。
- **密码已保存标记不同步**：保存密码后 `passwordSaved` 不置 true / 失败仍残留。修复：`saveConfigDirect` 成功/失败分支显式同步（useConfigStore L138-150）。
- **Toast 无限累积**：无上限致内存增长。修复：`MAX_TOASTS=4` 淘汰最旧（useLogToastStore L8-9, L64-68）。
- **同题 Toast 重复刷屏**：多通道同事件各弹一次。修复：`isDuplicateTitle` 跳过（useLogToastStore L11-13, L60-61）。
- **非前台 Toast 堆积**：用户看不到却入队。修复：`document.visibilityState` 判断跳过（带操作按钮除外）（useLogToastStore L53-56）。
- **关窗丢失 in-flight 保存**：`hasPendingConfig` 仅查 pending。修复：纳入 `saveConfigInFlight` 跟踪与 `flushPendingConfig` 等待（useConfigStore L17, L168-175, L188-224）。
- **桌面专属能力安卓调用崩溃**：统一以 `desktopOnly` 拒绝并 `noopListener` 空监听（tauriApi L14-15, L18, L203-206, L265-266 等）。
- **Portal 状态误判离线**：旧链路解析失败判无网络。修复：安卓恒传空 `adapterIp` 让后端回退缓存源 IP（useAuthStore L52-56, L224-225）。