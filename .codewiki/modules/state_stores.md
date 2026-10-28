---
title: state_stores
type: module
source_files:
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useAuthStore.ts
  - tauri-app/frontend/src/hooks/useAdapterStore.ts
  - tauri-app/frontend/src/hooks/useQualityStore.ts
  - tauri-app/frontend/src/hooks/useThemeStore.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.ts
  - tauri-app/frontend/src/hooks/useAppStore.ts
  - tauri-app/frontend/src/hooks/useAppInit.ts
  - tauri-app/frontend/src/hooks/useInitialDataLoad.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
  - tauri-app/frontend/src/hooks/useHeartbeat.ts
  - tauri-app/frontend/src/hooks/useAsyncLock.ts
  - tauri-app/frontend/src/hooks/useLogToastStore.test.ts
  - tauri-app/frontend/src/hooks/useAsyncLock.test.tsx
  - tauri-app/frontend/src/hooks/useEventListeners.notify.test.tsx
  - tauri-app/frontend/src/App.tsx
  - tauri-app/frontend/src/shared/ui-types.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - tauri-app/frontend/src/monitor/types.ts
  - tauri-app/frontend/src/network/types.ts
  - tauri-app/frontend/src/network/adapters.ts
  - tauri-app/frontend/src/shared/types.ts
tags: [state-management, zustand, hooks, frontend, tauri, global-store]
---

## Overview

`state_stores` 是 Tauri 桌面前端（位于 `tauri-app/frontend/src/hooks/`）的**全局状态管理中枢**：一组基于 [Zustand](https://github.com/pmndrs/zustand) 的领域 store（`create` 工厂，来自 `zustand`），加上把它们串联起来、并在应用启动时把数据灌进 store 的 React hooks。每个 store 负责一个明确业务域，组件通过 `useXxxStore(selector)` 细粒度订阅、通过 `useXxxStore.getState()` 在非 React 上下文（事件回调、定时任务）里读写。

本模块覆盖的状态域：

- **配置域** `useConfigStore`（`useConfigStore.ts:51`）：整个应用唯一的 `Config` 来源，含 500ms 防抖落盘、脏字段（dirty fields）避免后端旧快照覆盖、关窗 flush、账号列表、语言。
- **认证域** `useAuthStore`（`useAuthStore.ts:149`）：登录/注销、在线状态检测（`checkOnline`）、后台检测状态 `bgStatus`、登录/注销中锁。
- **适配器域** `useAdapterStore`（`useAdapterStore.ts:45`）：网卡列表、详情、当前激活面板、刷新动作。
- **网络质量域** `useQualityStore`（`useQualityStore.ts:40`）：网络质量、DNS DoH 状态、版本更新信息、手动刷新。
- **主题域** `useThemeStore`（`useThemeStore.ts:21`）：主题名、浅色模式、自定义主色、玻璃档位，并监听自身变化写 DOM。
- **日志/Toast 域** `useLogToastStore`（`useLogToastStore.ts:28`）：应用级日志流与 toast 队列，含重复折叠、上限淘汰、前后台去重。

编排层：`useAppStore.ts` 仅为历史兼容的 re-export（真实逻辑已拆分进领域 store，见 `useAppStore.ts:1-4`）；`useAppInit.ts` 把 `useEventListeners` + `useInitialDataLoad` + `useHeartbeat` + `useGlobalShortcut` 一并挂起；`useInitialDataLoad.ts` 在挂载时调用后端 `getInitData` 把初始快照灌入各 store；`useEventListeners.ts` 订阅所有后端事件并把结果写回对应 store。`useAsyncLock.ts` 是带 StrictMode 保护的通用异步锁 hook（组件级，非 Zustand）。

store 间是单向依赖（领域 store 互不持有对方实例，均通过 `getState()` 在运行时互读），无循环依赖；所有写回后端都集中在 `useConfigStore.saveConfigDirect` 与各处 `api.*` 调用。

## Key Components

所有 store 均为模块级单例（`create<Store>((set, get) => ({...}))`），通过命名导出 `useXxxStore` 与若干顶层辅助函数/常量。下面按文件列出公开符号、行号与职责。

### `useConfigStore.ts` — 配置域

`interface ConfigStore`（`useConfigStore.ts:27-49`）字段/方法：

| 符号 | 行号 | 类型/签名 | 用途 |
| --- | --- | --- | --- |
| `config` | `:52` | `Config` | 全应用唯一配置对象（初值 `DEFAULT_CONFIG`） |
| `configLoaded` | `:53` | `boolean` | 初始配置是否从后端加载完；依赖 config 的启动逻辑需等此信号（见 `useConfigStore.ts:29-32`、`useInitialDataLoad.ts:152`） |
| `passwordSaved` | `:54` | `boolean` | 会话内是否已保存密码（`credReady` 据此判定凭证就绪） |
| `initDebugMode` | `:35` | `boolean?` | `get_init_data` 携带的 debug 开关真实值，`InitData` 未到前 `undefined` |
| `initLogRetentionDays` | `:36` | `number?` | 同上，日志保留天数真实值 |
| `accounts` | `:37` | `AccountItem[]` | 账号列表 |
| `activeAccount` | `:38` | `string` | 当前激活账号 |
| `language` | `:39` | `string` | 当前语言（初值 `safeStorage.get('app-language') \|\| 'zh'`） |
| `updateConfig` | `:59` | `(partial: Partial<Config>) => void` | 更新 config 并 500ms 防抖落盘；标记 dirty 字段、联动主题色 |
| `updateConfigLocal` | `:93` | `(partial: Partial<Config>) => void` | 仅本地更新（不落盘，但仍标 dirty） |
| `mergeConfigFromBackend` | `:106` | `(incoming: Partial<Config>) => void` | 合并后端 `config-changed` 回传，**跳过本地 dirty 字段**防覆盖 |
| `clearDirtyFields` | `:115` | `() => void` | 清空 dirty 标记与失败计数 |
| `syncPasswordSaved` | `:120` | `(saved: boolean) => void` | 设置 `passwordSaved` |
| `saveConfigDirect` | `:122` | `(cfg, clearPassword?) => Promise<void>` | 立即落盘（绕过防抖）；跟踪 in-flight Promise，失败联动 `passwordSaved=false` |
| `setAccounts` | `:170` | `(a: AccountItem[]) => void` | 设置账号列表 |
| `setActiveAccount` | `:171` | `(a: string) => void` | 设置激活账号 |
| `setLanguage` | `:173` | `(lang: string) => void` | 切语言并写 `safeStorage` + `i18next.changeLanguage` |

模块级辅助/导出：

- `saveConfigTimer` / `saveConfigPending` / `saveConfigInFlight` / `dirtyFields` / `dirtyFailureCounts`（`useConfigStore.ts:14-25`）：防抖定时器、待保存快照、在飞 Promise、脏字段集合与连续失败计数。`DIRTY_FAILURE_LIMIT = 3`（`:24`）。
- `hasPendingConfig()`（`useConfigStore.ts:180-185`）：pending 或 in-flight 任一存在即视为有待保存。
- `flushPendingConfig()`（`useConfigStore.ts:187-216`）：清防抖定时器、把 pending（丢弃 `PASSWORD_MASK`）连同 in-flight 一并发回后端并等待，用于关窗收尾（`App.tsx:295-314`、`useEventListeners.ts:55`）。

### `useAuthStore.ts` — 认证域

`interface AuthStore`（`useAuthStore.ts:137-147`）：

| 符号 | 行号 | 类型/签名 | 用途 |
| --- | --- | --- | --- |
| `isLoggingIn` | `:138` | `boolean` | 登录进行中（按钮禁用） |
| `isLoggingOut` | `:139` | `boolean` | 注销进行中 |
| `status` | `:140` | `{ text: string; state: StatusState }` | 在线状态文本与状态机（`loading`/`online`/`offline`/`error`） |
| `bgStatus` | `:141` | `BackgroundStatus` | 后台检测状态（`monitor/types.ts:25` 全字段） |
| `doLogin` | `:155` | `(adapterName?) => Promise<boolean>` | 登录（带 60s `withTimeout`、成功后节流质量探测、失败才复查） |
| `doLogout` | `:216` | `(adapterName?) => Promise<void>` | 注销（带 60s `withTimeout`、事后复查） |
| `checkOnline` | `:247` | `(cfg?, adps?) => Promise<void>` | 在线检测核心：campus 检测→适配器 IP 解析→portal 状态查询；持 `_checkOnlineLockFlag` 串行锁，promise 真正 settle 才释放 |
| `setStatus` | `:317` | `(s) => void` | 直接设置 status |
| `setBgStatus` | `:318` | `(s | (prev)=>s) => void` | 设置/函数式更新 `bgStatus` |

模块级符号：`_checkOnlineLockFlag`（`useAuthStore.ts:19`）、`QUALITY_MANUAL_THROTTLE_MS = 60_000`（`useAuthStore.ts:32`）、类型别名 `CampusStatus`/`PortalStatus`（`:26-27`）、纯函数 `detectCampusNetwork`/`buildCampusBgStatusPatch`/`pickAdapterIp`/`refreshAdaptersForIp`/`queryPortalStatus`/`withTimeout`/`setStatusStable`（`:32-135`）。其中 `withTimeout`（`useAuthStore.ts:118`）用 `Promise.race` 给登录/注销 invoke 加 60s 超时，避免按钮永久 disabled；`setStatusStable`（`useAuthStore.ts:131`）在 text/state 未变时跳过 set，保持引用稳定（StatusBar 按引用订阅）。

### `useAdapterStore.ts` — 适配器域

`interface AdapterStore`（`useAdapterStore.ts:34-43`）：

| 符号 | 行号 | 类型/签名 | 用途 |
| --- | --- | --- | --- |
| `adapters` | `:35` | `Adapter[]` | 网卡列表（`network/types.ts:3`） |
| `adapterDetails` | `:36` | `AdapterDetail[]` | 网卡详情（`network/types.ts:15`） |
| `isRefreshingAdapters` | `:37` | `boolean` | 刷新中标志 |
| `activePanel` | `:38` | `PanelName` | 当前激活面板（初值 `'dashboard'`，见 `:49`） |
| `refreshAdapters` | `:51` | `() => Promise<void>` | 强制重探 + 触发后台检测，持 `_adapterLockFlag` 锁 |
| `setAdapters` | `:68` | `(a: Adapter[]) => void` | 写列表 |
| `setAdapterDetails` | `:69` | `(d: AdapterDetail[]) => void` | 写详情 |
| `setActivePanel` | `:70` | `(p: PanelName) => void` | 切面板 |

模块级辅助：`refreshAdapterData`（`useAdapterStore.ts:16-32`）并行 `getAdapters` + `getAdapterDetails` 后写回 store（收敛了三处重复实现，见 `useAdapterStore.ts:11-13`）。

### `useQualityStore.ts` — 网络质量域

`interface QualityStore`（`useQualityStore.ts:22-38`）：

| 符号 | 行号 | 类型/签名 | 用途 |
| --- | --- | --- | --- |
| `networkQuality` | `:23` | `NetworkQuality \| null` | 网络质量（`monitor/types.ts:83`） |
| `dnsDohStatus` | `:24` | `DnsDohStatus \| null` | DNS DoH 状态（`network/types.ts:44`） |
| `dnsChecking` | `:25` | `boolean` | DNS 检查中 |
| `isRefreshingQuality` | `:26` | `boolean` | 质量刷新中 |
| `updateAvailable` | `:27` | `boolean` | 是否有更新 |
| `latestVersion` | `:28` | `string` | 最新版本 |
| `releaseNotes` | `:29` | `string` | 更新说明 |
| `refreshRate` | `:30` | `number` | 后台检测刷新率（来自 `initData.refreshRate`） |
| `refreshQuality` | `:50` | `() => Promise<void>` | 手动刷新（受 `enableNetworkQuality===false` 短路、`_qualityLockFlag` 锁控制） |
| `setNetworkQuality` | `:74` | `(q | (prev)=>q) => void` | 写质量并刷新 `lastQualityResultTime` |
| `setDnsDohStatus` | `:79` | `(s) => void` | 写 DNS 状态 |
| `setDnsChecking` | `:80` | `(v) => void` | — |
| `setUpdateAvailable` | `:81` | `(v) => void` | — |
| `setLatestVersion` | `:82` | `(v) => void` | — |
| `setReleaseNotes` | `:83` | `(v) => void` | — |

模块级辅助：`_qualityLockFlag`（`useQualityStore.ts:11`）、`lastQualityResultTime`（`useQualityStore.ts:16`）、`getLastQualityResultTime()`（`useQualityStore.ts:18-20`，登录后节流探测判定用）。

### `useThemeStore.ts` — 主题域

`interface ThemeStore`（`useThemeStore.ts:9-19`）：

| 符号 | 行号 | 类型/签名 | 用途 |
| --- | --- | --- | --- |
| `themeName` | `:22` | `ThemeName` | 主题名（初值 `'default'`，`ui-types.ts:3`） |
| `isLightMode` | `:23` | `boolean` | 浅色模式（初值读 `safeStorage`） |
| `customThemeColor` | `:24` | `string` | 自定义主色（初值 `'#6366f1'`） |
| `glassLevel` | `:25` | `GlassLevel` | 玻璃档位（初值 `'ultra'`，`ui-types.ts:5`） |
| `setThemeName` | `:32` | `(name) => void` | — |
| `setIsLightMode` | `:33` | `(v) => void` | — |
| `setGlassLevel` | `:34` | `(v) => void` | — |
| `initTheme` | `:36` | `(cfg: Partial<Config>) => void` | 启动时据 `safeStorage` + `cfg.themeMode` 初始化主题/浅色 |
| `setCustomThemeColor` | `:58` | `(color) => void` | 即时写主色（预览） |

模块级副作用：`useThemeStore.subscribe(...)`（`useThemeStore.ts:62-100`）监听 `glassLevel`/`isLightMode`/`themeName`/`customThemeColor` 变化并切 DOM class、CSS 变量（`--primary`/`--ring`/`--accent` 等），自定义色经 `hexToHsl` 转换后 `setProperty`（修复 `cssText +=` 无界增长，见 `:84-88`）。

### `useLogToastStore.ts` — 日志/Toast 域

`interface LogToastStore`（`useLogToastStore.ts:15-26`）：

| 符号 | 行号 | 类型/签名 | 用途 |
| --- | --- | --- | --- |
| `logs` | `:16` | `LogEntry[]` | 应用日志流（`ui-types.ts:8`，含 `count` 折叠计数） |
| `toasts` | `:17` | `ToastMessage[]` | toast 队列（`ui-types.ts:17`） |
| `addLog` | `:32` | `(message, type?) => void` | 追加日志；连续相同（内容+类型）折叠为一条并 `count++`；上限 `MAX_LOG_ENTRIES=300` 滚旧 |
| `addToast` | `:53` | `(title, type?, description?, duration?, mascot?) => void` | 加 toast；非前台不入队（`visibilityState!=='visible'` 跳过），同 title 去重、超 `MAX_TOASTS=4` 淘汰最旧 |
| `addToastWithAction` | `:85` | `(toast) => void` | 带操作按钮的 toast（如"取消退出"），非前台仍入队 |
| `removeToast` | `:111` | `(id) => void` | 移除并清定时器 |
| `removeToastsByPrefix` | `:117` | `(prefix) => void` | 按 id 前缀批量移除（自动退出/校园网退出计数 toast 用） |
| `setLogs` | `:129` | `(logs) => void` | 直接设日志（超过上限截断） |
| `cleanupToasts` | `:131` | `() => void` | 清所有定时器与 toast（App 卸载兜底，见 `App.tsx:233`） |

模块级辅助：`toastTimers` Map、`toastIdCounter`/`logIdCounter`、`MAX_TOASTS = 4`（`useLogToastStore.ts:5-9`）、`isDuplicateTitle`（`useLogToastStore.ts:13`，双通道去重）。

### 编排层

- `useAppStore.ts:2-3`：空壳 re-export，仅兼容旧引用（`useAppInit` / `hasPendingConfig` / `flushPendingConfig`）。
- `useAppInit.ts:6-11`：`useEventListeners()` + `useInitialDataLoad()` + `useHeartbeat()` + `useGlobalShortcut()`。
- `useInitialDataLoad.ts:18-168`：`useEffect` 内 `api.getInitData()` 把 config / adapters / bgStatus / accounts / dns 等初始快照写入各 store，并据 `savedPanel`/`defaultPanel` 恢复 `activePanel`，最后 `checkOnline` 并置 `configLoaded=true`（失败降级 `DEFAULT_CONFIG`）。
- `useEventListeners.ts:18-384`：`useEffect` 订阅全部后端事件（`onBackgroundCheckResult`、`onAutoLoginResult`、`onAdaptersChanged`、`onAdapterDetailsChanged`、`onAdapterDisabledWarning`、`onLoginLog`、`onAutoExitCountdown`、`onAutoExitCancelled`、`onCampusExitCountdown`、`onCampusExitCancelled`、`onNetworkQualityResult`、`onUpdateAvailable`、`onConfigChanged`、`onScheduledActionSkipped`、`onMacResetCycle`），把结果写回对应 store / 触发 toast / 调用 `checkOnline`。失焦兜底 flush（`useEventListeners.ts:368`）。
- `useHeartbeat.ts:5-20`：5s `renderHeartbeat`，渲染链失活时跳过（让后端重载 WebView）。
- `useAsyncLock.ts:3-27`：通用异步锁 hook（组件级），`[isRunning, execute]`，带 StrictMode `mountedRef` 保护与 `cooldownMs` 冷却，避免按钮永久转圈（测试见 `useAsyncLock.test.tsx`）。

## Architecture

六个领域 store 由 Zustand 单例承载，相互**不互相持有引用**，运行时仅通过 `useXxxStore.getState()` 互读（单向、无环）。编排层在应用挂载期把后端数据灌入 store，并持续把后端事件写回 store。下方框图标注组件名与文件：

```text
┌─────────────────────────────── useAppInit (useAppInit.ts:6) ───────────────────────────────┐
│  mount: useEventListeners() │ useInitialDataLoad() │ useHeartbeat() │ useGlobalShortcut()  │
└───────┬───────────────────────────────┬───────────────────────────────┬───────────────────┘
        │ subscribe(后端事件)             │ getInitData() 初始快照          │ 5s renderHeartbeat
        ▼                                ▼                                │
┌─────────────── useEventListeners.ts:18 ───────────┐   ┌── useInitialDataLoad.ts:18 ──┐    │
│ onBackgroundCheckResult / onAutoLoginResult        │   │ config / adapters /          │    │
│ onAdaptersChanged / onConfigChanged / ...          │   │ bgStatus / accounts /        │    ▼
│ → 写回各 store + addLog/addToast                   │   │ dns + setActivePanel +       (后端 Rust)
└───────┬───────────────┬───────────────┬──────────┘   │ checkOnline + configLoaded    │
        │               │               │              └────────────┬───────────────────┘
        ▼               ▼               ▼                            │
┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌──────────────┐
│ useAuth    │  │ useAdapter │  │ useQuality │  │ useConfig  │  │ useTheme   │  │ useLogToast  │
│ Store      │  │ Store      │  │ Store      │  │ Store      │  │ Store      │  │ Store        │
│ :149       │  │ :45        │  │ :40        │  │ :51        │  │ :21        │  │ :28          │
│ status/    │  │ adapters/  │  │ quality/   │  │ config/    │  │ theme/     │  │ logs/toasts │
│ bgStatus/  │  │ activePanel│  │ dnsDoh/    │  │ accounts/  │  │ glassLevel │  │              │
│ doLogin    │  │ details    │  │ update     │  │ language   │  │            │  │              │
└─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘  └─────┬────────┘
      │ getState()    │ getState()    │ getState()    │ getState()     │ subscribe()    │ 渲染
      └──────────────►│──────────────►│──────────────►│ 互读(单向无环) │ ► DOM class/  │ ToastContainer
       运行时互读      └──────────────►│──────────────►│               │   CSS 变量     │ / LogPanel
                                      └──────────────►│               │  (:62-100)    │ (App.tsx:174,522)
                                                     │ saveConfigDirect (:122) ─► api.saveConfig
                                                     │ flushPendingConfig (:187) ─► 关窗收尾
```

要点：

- **单一数据源**：`useConfigStore.config` 是所有配置的唯一真源；输入框走 `updateConfig`（防抖落盘），后端回传走 `mergeConfigFromBackend`（跳过 dirty）。
- **主题副作用内聚**：`useThemeStore` 通过 `subscribe`（`useThemeStore.ts:62`）自管 DOM 副作用，组件无需关心 CSS 变量切换。
- **锁模式一致**：`checkOnline`/`refreshAdapters`/`refreshQuality` 均采用「锁标志在 promise 真正 settle 后释放」模式（`useAuthStore.ts:308-314`、`useAdapterStore.ts:62-64`、`useQualityStore.ts:64-68`），修复了早期 `setTimeout(500)` 提前释放导致的重入/冗余 invoke。
- **引用稳定性**：`setStatusStable`（`useAuthStore.ts:131`）与 `setBgStatus` 函数式更新、事件处理里 `JSON.stringify` 比较（`useEventListeners.ts:235`）等，均为了避免订阅方（如 `StatusBar`）无效整卡重渲染。

## Data Flow

### 启动流程（挂载 → 初始数据 → 事件订阅）

1. `App.tsx` 渲染 `AppInner`，首行调用 `useAppInit()`（`App.tsx:115` → `useAppInit.ts:6`）。
2. `useInitialDataLoad` 的 `useEffect` 调 `api.getInitData()`（`useInitialDataLoad.ts:32`），把结果落盘到各 store：
   - `useConfigStore.setState({ config })`（`:41`），并据 `PASSWORD_MASK` 设 `passwordSaved`（`:36-40`）、写 `initDebugMode`/`initLogRetentionDays`（`:44-47`）。
   - `useThemeStore.getState().initTheme(cfg)`（`:49`）。
   - 恢复 `activePanel`（据 `savedPanel` / `defaultPanel`，`:51-60`）。
   - 写 `adapters`（`:70`）、`bgStatus`（`:81-92`）、`adapterDetails`（`:95`）、`accounts`/`activeAccount`（`:98-101`）、`refreshRate`（`:106`）。
   - 最后 `useAuthStore.getState().checkOnline(cfg, adps)`（`:103`）并置 `configLoaded=true`（`:152`）；失败时降级 `DEFAULT_CONFIG`（`:160`）。
3. 同时 `useEventListeners` 注册所有后端事件监听（`useEventListeners.ts:18` 起），后续后端推送直接写回 store。

### 配置写入与落盘（用户输入 → 后端）

- 组件调 `useConfigStore.getState().updateConfig(partial)`（`useConfigStore.ts:51`）：先 `set` 到内存 config，标记 `dirtyFields`，再 500ms 防抖后调 `saveConfigDirect`（`useConfigStore.ts:80-89`）。
- `saveConfigDirect`（`useConfigStore.ts:122`）：`api.saveConfig` 落盘；成功后清 dirty、置 `passwordSaved=true`；失败保留 dirty 并累加 `dirtyFailureCounts`，达 `DIRTY_FAILURE_LIMIT=3` 放弃 dirty 让后端回传重同步（`:144-155`）。
- 后端任何修改经 `onConfigChanged` → `mergeConfigFromBackend`（`useEventListeners.ts:336-342`）回流，跳过本地 dirty 字段防覆盖。

### 在线状态检测（后台巡检 + 手动触发）

- 后台事件 `onBackgroundCheckResult`（`useEventListeners.ts:59`）：节流 1s（`lastBgCheckTimeRef`），通过 `useAuthStore.getState().setBgStatus(...)`（`:118`）更新 `bgStatus`（含逐卡 `adapterStatuses` 构建）、并在 text/state 变化时才 `setStatus`（`:178-181`）。
- 手动 `checkOnline`（`useAuthStore.ts:247`）：campus 检测 → 适配器 IP 解析（`pickAdapterIp`/`refreshAdaptersForIp`）→ portal 状态查询（`queryPortalStatus`），全程 `_checkOnlineLockFlag` 串行互斥。
- 登录 `doLogin`（`useAuthStore.ts:155`）：60s `withTimeout` 包装，成功后按 `QUALITY_MANUAL_THROTTLE_MS=60_000` 节流做质量探测（`:191`），仅失败时复查（`:209-211`），修复了登录瞬间状态闪烁。

### 关窗收尾（flush）

- `App.tsx:handleCloseWindow`（`:295`）调 `flushPendingConfig()`（`useConfigStore.ts:187`）：清防抖定时器，把 pending + in-flight 保存一并发回后端并 `Promise.allSettled` 等待（最多 2s race，`:302-308`），失败不阻塞关窗。
- `useEventListeners.ts:55` 的 `window blur` 也兜底 flush，覆盖 Alt+F4 等不经前端按钮的关闭时序。
- **注意**：前端**不注册** `onCloseRequested`（见 `useEventListeners.ts:49-54` 注释与 `App.tsx:289-294`），销毁控制权回归 Rust，避免 Tauri core 因检测到 JS 监听而 `prevent_close` 导致窗口滞留。

### 日志 / Toast 数据流

- 任意 store / 事件处理通过 `useLogToastStore.getState().addLog / addToast` 写日志与通知。
- `addLog` 连续相同（内容+类型）折叠为一条并 `count++`（`:39-43`），上限 `MAX_LOG_ENTRIES=300`（`ui-constants.ts:1`）。
- `addToast` 非前台跳过（`:57`），同 title 去重（`isDuplicateTitle`），超 `MAX_TOASTS=4` 淘汰最旧（`:65-69`）；`addToastWithAction` 用于"取消退出"等带操作入口的 toast（非前台仍入队）。
- `App.tsx:174-181` 用 `useShallow` 订阅 `logs/toasts` 传给 `ToastContainer` / `RightPanel`。

## Connections

本模块（前端全局状态）与以下模块/组件通过 import 与 `getState()` 互连：

- [[tauri-api]] / `tauriApi.ts`：`tauriApiWithRetry` 是各 store 写回后端的唯一通道（`doLogin`/`doLogout`/`checkOnline`/`saveConfigDirect`/`refreshAdapterData`/`refreshQuality`/`getInitData` 等），所有 `onX` 后端事件监听也在此注册（`useEventListeners.ts:10`）。
- [[settings]]：`Config` / `AccountItem`（来自 `@/settings`）、`DEFAULT_CONFIG` / `VALID_THEMES` / `VALID_GLASS_LEVELS`（来自 `@/settings/constants`），`useConfigStore` 与 `useThemeStore` 直接依赖。
- [[network]]：`Adapter` / `AdapterDetail` / `DnsDohStatus` / `DnsAdapterInfo`（`network/types.ts`）、`AUTO_DETECT_ADAPTER`（`network/adapters.ts:3`），被 `useAdapterStore` / `useQualityStore` / `useAuthStore` 引用。
- [[monitor]]：`BackgroundStatus` / `NetworkQuality` / `AdapterOnlineStatus`（`monitor/types.ts`），被 `useAuthStore` / `useQualityStore` / `useEventListeners` 引用；`StatusBar`（`monitor/StatusBar.tsx`）按引用订阅 `status`/`bgStatus`。
- [[shared]]：`StatusState` / `PanelName` / `ThemeName` / `GlassLevel` / `LogType` / `LogEntry` / `ToastMessage`（`shared/ui-types.ts`）、`MAX_LOG_ENTRIES` / `PASSWORD_MASK` / `NAV_ITEMS`（`shared/ui-constants.ts`）、`MAX_TOASTS`/计数常量（模块内）。
- [[lib]]：`safeStorage` / `extractErrorMessage` / `hexToHsl`（`@/lib/utils`、`@/lib/color`）、`mergeNetworkQuality`（`@/lib/latency`）。
- [[auth]] / [[account]] / [[network]] / [[settings]] 的容器 hooks（`useAuth`/`useNetwork`/`useAccount`/`useSettings`/`useMonitor`）：在 `App.tsx` 中被调用，其 `handleXxx` 回调多直接调 `useConfigStore.getState().updateConfig` 等（`App.tsx:169,168-172`）。
- `App.tsx`：顶层消费各 store（`activePanel`/`accounts`/`isLoggingIn`/`config.*`/`toasts`/`logs` 等），并持有 `handleCloseWindow` 关窗 flush（`App.tsx:295`）。
- `useAppInit` 还依赖 [[global-shortcut]]（`useGlobalShortcut`，见 `useAppInit.ts:4`）。

相关测试：`useLogToastStore.test.ts`（日志折叠/toast 去重/上限/前后台）、`useAsyncLock.test.tsx`（StrictMode 锁释放）、`useEventListeners.notify.test.tsx`（C11 事件接线）。

## Known Issues

代码中大量以「历史缺陷」注释形式记录，均为已修复但需后人警惕的坑：

- **关窗丢配置（脏字段永久脱节）**：原 `saveConfigPending` 检查遗漏 in-flight 保存，关窗直接跳过 flush。已修复（`useConfigStore.ts:180-185`、`flushPendingConfig` 跟踪 `saveConfigInFlight` 并 `Promise.allSettled`）。`dirtyFields` 失败超 `DIRTY_FAILURE_LIMIT=3` 才放弃脏标记，避免后端旧快照永久覆盖本地新值（`:21-25,144-155`）。
- **`onCloseRequested` 致窗口滞留**：前端曾注册该监听，Tauri core 检测到就 `prevent_close`（tauri-2.11.0 `window.rs:170-174`），窗口销毁转嫁 JS 异步收尾链，6 连击关闭无效。现约定前端不注册，flush 由关闭发起方（`App.tsx:290`）与 `blur` 兜底（`useEventListeners.ts:50`）就地完成（`:49-54`、`:289-294`）。
- **锁提前释放导致重入**：`checkOnline`/`refreshAdapters` 早期用 `setTimeout(500)` 释放锁，执行超 500ms 时锁提前释放产生冗余 invoke。现改为 promise 真正 settle 即释放（对齐 `refreshQuality` 的 500ms 最短展示但锁随工作完成释放）（`useAuthStore.ts:308-314`、`useAdapterStore.ts:61-65`）。
- **登录后状态闪烁 / 重复全量探测**：登录成功即 `checkOnline()` 会把"在线"瞬间覆盖成"离线"；登录后无条件全量质量探测与后端 latency loop 重复。修复：成功后不复查（仅失败复查）、质量探测按 60s 节流（`:186-198,206-211`）。
- **`passwordSaved` 永不正确**：会话内保存密码后 `config-changed` 回写 `PASSWORD_MASK` 但 `passwordSaved` 从不置 true。修复：`saveConfigDirect` 成功即置 true、失败置 false（`:132-143`）。
- **引用不稳定触发无效重渲染**：`setStatus` 无条件 set 新对象使 `StatusBar` 整卡重渲染；`onAdapterDetailsChanged` 每次推送新数组对象。修复：`setStatusStable`（`:131`）、JSON 比较跳过未变详情（`:233-236`）、`EMPTY_ADAPTER_STATUSES` 空数组复用（`:13-15,156`）。
- **toast/log 无界累积**：原无上限。`MAX_TOASTS=4`（`:9`）、`MAX_LOG_ENTRIES=300`（`ui-constants.ts:9`）、连续日志折叠 `count`（`useLogToastStore.ts:39-43`）修复。
- **非前台 toast 刷屏**：原窗口隐藏时仍弹 toast。修复：`addToast` 在 `visibilityState!=='visible'` 时跳过（保留带 action 的 toast），信息已由日志兜底（`:54-57,76-95`）。
- **主题切换 inline style 无界增长**：原 `cssText +=` 累积。改用 `style.setProperty` 覆盖同名变量（`:84-88`）。
- **StrictMode 下锁/初始化失效**：`useInitialDataLoad` 早期用 `initDoneRef` 短路导致二次 setup 丢弃异步初始化；`useAsyncLock` 的 `mountedRef` cleanup 置 false 后锁永不释放。修复：二次 setup 恢复 `mountedRef=true`（`useInitialDataLoad.ts:26`、`useAsyncLock.ts:12-17`）。
- **功能注释/约束（非禁用但需注意）**：
  - 网络质量检测由后端 latency loop 统一管理，前端 `useInitialDataLoad` **不再主动**调用 `checkNetworkQuality`（`:139-140` 注释）；`refreshQuality` 仍保留供手动刷新（`useQualityStore.ts:50`）。
  - `quality` 面板在 `enableNetworkQuality===false` 时**不渲染**（`App.tsx:384`），恢复 `activePanel` 会跳过该面板避免主区空白（`useInitialDataLoad.ts:54-60`）。
  - `useAppStore.ts` 已废弃，仅保留 re-export 兼容旧引用（`:1-4`）。
