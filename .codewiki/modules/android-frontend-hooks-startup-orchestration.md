---
title: 安卓前端启动编排
type: module
source_files:
  - android/frontend/src/hooks/useAppInit.ts
  - android/frontend/src/hooks/tauriApi.ts
tags: []
generated_by: fsoft-native
fsoft_page: android_frontend_hooks/android_frontend_hooks_startup_orchestration.md
---
# 安卓前端启动编排

## 简介

`android_frontend_hooks_startup_orchestration` 模块负责安卓端前端应用的启动编排与 Tauri 桥接层抽象。它由两个核心文件构成：

- `useAppInit`：应用级启动编排 hook，在 React 组件树挂载时一次性装配所有长期运行的子系统（事件监听、初始数据加载、心跳、全局快捷键）。
- `tauriApi`：统一的 Tauri 命令/事件桥接层，定义 `TauriApi` 接口并提供 `createEventListener`、`desktopOnly`、`noopListener`、`withRetry` 等通用辅助函数，处理安卓端与桌面端的平台差异。

该模块处于安卓前端 hook 体系的"地基"位置：它不直接渲染 UI，而是把后端命令通道、事件订阅通道与启动期数据流组织起来，供认证、账号、监控等上层模块消费。

## 架构总览

```mermaid
flowchart TD
    subgraph Entry["入口层"]
        Main["main.tsx<br/>setupCrashRecovery"]
        AppRoot["App 根组件"]
    end

    subgraph Orch["启动编排层（本模块）"]
        Init["useAppInit<br/>启动编排 hook"]
        Api["tauriApi<br/>TauriApi 接口 + 辅助函数"]
    end

    subgraph SubSystems["启动子系统"]
        EH["useEventListeners<br/>后端事件订阅装配"]
        IDL["useInitialDataLoad<br/>初始数据加载"]
        HB["useHeartbeat<br/>渲染心跳上报"]
        GS["useGlobalShortcut<br/>全局快捷键"]
    end

    subgraph Stores["状态存储层（兄弟子模块）"]
        CS["useConfigStore<br/>配置/账号"]
        AS["useAuthStore<br/>认证状态"]
        QS["useQualityStore<br/>网络质量/更新"]
        TS["useThemeStore<br/>主题"]
        LTS["useLogToastStore<br/>日志/提示"]
    end

    subgraph Backend["安卓后端（Rust）"]
        GetInit["get_init_data 命令"]
        Monitor["monitor_loop 事件发射"]
        Prot["protocol_cmds 命令"]
        Sys["system_cmds 命令"]
    end

    Main -->|"渲染异常恢复"| AppRoot
    AppRoot -->|"挂载时调用"| Init
    Init --> EH
    Init --> IDL
    Init --> HB
    Init --> GS

    EH -.->|"api.on* 事件订阅"| Api
    IDL -.->|"api.getInitData / checkOnline"| Api
    HB -.->|"api.renderHeartbeat"| Api
    GS -.->|"api.cancelAutoExit"| Api

    IDL --> CS
    IDL --> AS
    IDL --> QS
    IDL --> TS
    EH --> CS
    EH --> AS
    EH --> QS
    EH --> LTS

    Api -->|"invoke / listen"| Backend
```

## 核心组件

### useAppInit — 启动编排入口

```typescript
function useAppInit() {
  useEventListeners()
  useInitialDataLoad()
  useHeartbeat()
  useGlobalShortcut()
}
```

`useAppInit` 本身不包含业务逻辑，它在 App 根组件挂载时被调用，按顺序装配四个长期运行的子系统。每个子系统以 `useEffect` 形式自行管理生命周期与清理，因此 `useAppInit` 自身保持极薄的透传形态，便于按需组合与测试替换。

| 子系统 | 职责 | 安卓端特殊性 |
| --- | --- | --- |
| `useEventListeners` | 订阅后端事件（后台检测、自动登录、网络质量、更新、配置变更等），路由到对应 store | 跳过适配器相关的 noop 订阅；额外挂 `visibilitychange`/`pagehide` 兜底冲刷配置 |
| `useInitialDataLoad` | 调用 `get_init_data` 拉取配置、账号、后台检测状态，初始化主题与面板 | 删除了桌面遗留的 `getAdapters`/GPU/DNS DoH 补充请求（均为 `desktopOnly` 空转） |
| `useHeartbeat` | 每 5s 调用 `renderHeartbeat`，让后端感知前端存活 | 切后台时暂停；渲染链失活（`isRenderLoopAlive`）时跳过 |
| `useGlobalShortcut` | 注册 `Ctrl+Shift+C` 取消自动退出 | 忽略输入框可编辑元素，避免复制快捷键误触发 |

### tauriApi — Tauri 桥接层

`tauriApi.ts` 定义了贯穿整个安卓前端的 `TauriApi` 接口，将 Tauri `invoke` 命令与事件监听抽象为统一的 Promise/回调式 API。其核心设计意图是**平台差异收敛**：桌面专属能力在安卓端有明确的降级语义。

#### TauriApi 接口的平台分层

```mermaid
flowchart LR
    subgraph Common["两端共有能力"]
        C1["配置 getConfig / saveConfig"]
        C2["登录 doLogin / doLogout"]
        C3["账号 listAccounts / switchAccount"]
        C4["后台检测 start/stop/trigger"]
        C5["更新 check_update / download"]
        C6["自助服务 VPN 系列"]
    end

    subgraph AndroidOnly["安卓专属能力"]
        A1["bindToWifi 绑定 WiFi"]
        A2["requestWifiSsidPermission"]
        A3["电池优化 getBatteryOptimizationInfo<br/>requestIgnoreBatteryOptimizations<br/>openVendorBatterySettings"]
        A4["通知设置 openNotificationSettings"]
        A5["getAvoidBadWifiStatus<br/>restoreWrittenSettings"]
    end

    subgraph DesktopOnly["桌面专属（安卓拒绝）"]
        D1["exportConfig / importConfig"]
        D2["exportDiagnostics"]
        D3["getAdapters / enableAdapter<br/>getAdapterDetails"]
        D4["dhcpRenewAll / dhcpReleaseRenew"]
        D5["setupDnsDoh / resetDns"]
        D6["minimizeWindow / closeWindow<br/>showWindow / notifyWindowReady"]
    end

    Api["TauriApi 接口"] --> Common
    Api --> AndroidOnly
    Api --> DesktopOnly
```

- **桌面专属能力**：接口中带明确注释（如 `/** 桌面专属：配置导出/导入与诊断包导出依赖 DPAPI 密文、适配器/GPU 采集与本地文件系统，安卓端无对应实现 */`）。这些方法在安卓端由 `desktopOnly` 包装，调用即 reject，错误信息为 `桌面专属功能,安卓端不可用: <name>`。
- **安卓专属能力**：`bindToWifi`、`requestWifiSsidPermission`、电池优化系列、`getAvoidBadWifiStatus`/`restoreWrittenSettings`（Settings.Global 写通道）等，对应安卓后端 `protocol_cmds.rs`、`system_cmds.rs` 中的命令。

#### 辅助函数

| 函数 | 签名 | 作用 |
| --- | --- | --- |
| `createEventListener` | `<T>(eventName: string) => (cb) => () => void` | 包装 Tauri `listen`，返回可取消订阅函数；处理"listen Promise 未完成即被取消"的竞态（延迟 unlisten）；DEV 模式下注册失败仅打日志不抛错 |
| `desktopOnly` | `<T>(name: string) => Promise<T>` | 返回一个立即 reject 的 Promise，用于桌面专属命令在安卓端的占位实现 |
| `noopListener` | `() => () => {}` | 返回一个空操作的订阅取消函数，用于安卓端永不发射的桌面专属事件（如 `onAdaptersChanged`）的占位 |
| `withRetry` | `<T>(fn, maxRetries=2, baseDelay=500) => Promise<T>` | 对可重试错误执行指数退避重试（`baseDelay * 2^attempt + 随机 200ms`），不可重试错误立即抛出 |

### createEventListener 的竞态处理

```mermaid
sequenceDiagram
    participant U["调用方组件"]
    participant L["createEventListener"]
    participant T["Tauri listen()"]

    U->>L: 传入 cb，得到 unlisten
    L->>T: listen(eventName) [Promise]
    Note over L: cancelled = false
    alt 正常完成
        T-->>L: resolve(unlistenFn)
        L->>L: unlisten = fn
    else 提前取消
        U->>L: 调用返回的 unlisten()
        L->>L: cancelled = true
        Note over T: listen 仍在进行
        T-->>L: resolve(unlistenFn)
        L->>L: 因 cancelled 已 true#59 立即 fn()
    end
```

该设计保证了 React StrictMode 下 setup→cleanup→setup 的循环不会产生悬挂监听器。

## 启动数据流

### 初始数据加载流程

```mermaid
sequenceDiagram
    participant Root["App 根组件"]
    participant IDL["useInitialDataLoad"]
    participant Api["tauriApi"]
    participant BE["安卓后端 get_init_data"]
    participant CS["useConfigStore"]
    participant TS["useThemeStore"]
    participant AS["useAuthStore"]
    participant ADP["useAdapterStore"]

    Root->>IDL: 挂载触发 effect
    IDL->>Api: api.getInitData()
    Api->>BE: invoke("get_init_data")
    BE-->>Api: InitData{config, backgroundStatus, accounts, activeAccount, refreshRate}
    Api-->>IDL: initData

    alt 加载成功
        IDL->>CS: setState({config: 合并 DEFAULT_CONFIG})
        IDL->>CS: syncPasswordSaved(密码是否被 MASK)
        IDL->>TS: initTheme(cfg)
        IDL->>ADP: 恢复/设置 activePanel（质量禁用时跳过 quality 面板）
        IDL->>AS: setState({bgStatus: 合并 backgroundStatus})
        alt bgStatus.online === true
            IDL->>AS: setStatus({text: message, state: online})
            Note over IDL,AS: 修复"启动自动登录成功后状态点仍灰"的竞态丢失
        end
        IDL->>CS: setState({accounts, activeAccount})
        IDL->>AS: checkOnline(cfg)
        IDL->>QS: setState({refreshRate})
        IDL->>CS: setState({configLoaded: true})
    else 加载失败
        IDL->>CS: setState({config: DEFAULT_CONFIG, configLoaded: true})
        IDL->>LTS: addLog("初始化数据加载失败")
    end
```

关键设计点：

- **`configLoaded` 信号**：`useInitialDataLoad` 无论成功或降级都会在末尾把 `useConfigStore.configLoaded` 置为 `true`。依赖配置的启动逻辑（如自助服务面板自动验证/回显）应等待此信号，避免在加载窗口期提前消耗一次性流程。
- **修复启动竞态**：后台检测可能在 WebView 事件监听建立前跑完首拍并发射事件，导致前端错过"在线"状态。`useInitialDataLoad` 通过 `get_init_data` 返回的 `backgroundStatus` 同步补齐这一状态。
- **平台裁剪**：安卓端删除了桌面遗留的补充请求（`getAdapters`/GPU 信息/DNS DoH 状态），避免对 `desktopOnly` 命令的必 reject 空转白耗启动窗口。

### 事件订阅路由

```mermaid
flowchart TD
    subgraph Events["后端发射事件"]
        E1["background-check-result"]
        E2["auto-login-result"]
        E3["login-log"]
        E4["auto-exit-countdown / cancelled"]
        E5["campus-exit-countdown / cancelled"]
        E6["network-quality-result"]
        E7["update-available / update-unverified"]
        E8["config-changed"]
    end

    EH["useEventListeners"]
    EH --> E1
    EH --> E2
    EH --> E3
    EH --> E4
    EH --> E5
    EH --> E6
    EH --> E7
    EH --> E8

    E1 -->|"setBgStatus / setStatus（引用稳定优化）"| AS["useAuthStore"]
    E2 -->|"addToast/addLog + checkOnline"| LTS["useLogToastStore"]
    E2 --> AS
    E3 --> LTS
    E4 -->|"addToastWithAction（可取消）"| LTS
    E5 --> LTS
    E6 -->|"mergeNetworkQuality + bad 告警"| QS["useQualityStore"]
    E6 --> LTS
    E7 -->|"setUpdateAvailable 等"| QS
    E7 --> LTS
    E8 -->|"mergeConfigFromBackend（跳过脏字段）"| CS["useConfigStore"]
    E8 -->|"setActiveAccount + listAccounts 刷新"| CS
```

订阅层的关键防御措施：

- **`mountedRef` 门控**：StrictMode 下二次 setup 时恢复 `mountedRef = true`，防止 cleanup 置 false 后所有 handler 静默失效。
- **1s 去抖**：`background-check-result` 通过 `lastBgCheckTimeRef` 做 1s 节流，避免高频更新击穿渲染。
- **引用稳定**：后台检测结果中 `text`/`state` 未变化时跳过 `setStatus`，避免 StatusBar 整卡重渲染。
- **配置合并防回滚**：`config-changed` 使用 `mergeConfigFromBackend` 跳过本地未落盘的脏字段，避免后端旧快照覆盖本地刚设置的值（如 `enableLatencyTest`）与 MASK 覆盖密码。

## 进程退出与配置冲刷

安卓端进程被杀不会触发 Tauri 的 `onCloseRequested`，因此编排层在事件订阅中额外挂载了浏览器生命周期兜底：

```mermaid
flowchart LR
    subgraph Triggers["冲刷触发源"]
        T1["onCloseRequested（窗口关闭请求）"]
        T2["visibilitychange → hidden（切后台）"]
        T3["pagehide（页面卸载）"]
    end

    T1 -->|"preventDefault 拦截"| Flush["flushPendingConfig"]
    T2 --> Flush
    T3 --> Flush

    Flush -->|"hasPendingConfig? pending 或 in-flight 任一"| Check{"有待保存数据?"}
    Check -->|"否"| Direct["直接关闭/返回"]
    Check -->|"是"| Race["await Promise.race(in-flight, 2s 超时)"]
    Race --> Close["close() 或静默返回"]
```

`hasPendingConfig` 的判定同时覆盖 debounce 待存（`saveConfigPending`）与进行中保存（`saveConfigInFlight`），避免 in-flight 保存随窗口销毁丢失。

## 与入口层的关系

启动编排之上还有 [安卓前端入口与 Mock](../android_frontend_entrypoint_mocks.md) 中的 `setupCrashRecovery`，负责渲染异常恢复：

- 检测 GPU/WebGL 错误、rAF 心跳停滞（>10s 阈值，避免低端机长 GC 误判）、root 元素为空时自动重载，最多 3 次。
- 切页面可见性时暂停/恢复 GSAP 全局时间线。

`useHeartbeat` 的 `isRenderLoopAlive()` 检查与 `setupCrashRecovery` 的 rAF 停滞检测协同：渲染链失活时前端不上报心跳，后端按心跳丢失重载 WebView。

## 依赖关系

```mermaid
flowchart TD
    subgraph Orch["启动编排模块（本页）"]
        direction TB
        UA["useAppInit.ts::useAppInit"]
        TA["tauriApi.ts::TauriApi / createEventListener / desktopOnly / noopListener / withRetry"]
    end

    subgraph State["状态存储子模块"]
        direction TB
        linkState["android_frontend_hooks_state_stores.md"]
    end

    subgraph Parent["hooks 父模块"]
        direction TB
        linkParent["android_frontend_hooks.md"]
    end

    subgraph Upper["上层功能模块"]
        direction TB
        linkAuth["../android_frontend_auth.md"]
        linkMonitor["../android_frontend_monitoring_network.md"]
        linkSettings["../android_frontend_settings.md"]
        linkAccount["../android_frontend_account_management.md"]
    end

    subgraph Entry["入口层"]
        direction TB
        linkEntry["../android_frontend_entrypoint_mocks.md"]
    end

    subgraph Backend["安卓后端"]
        direction TB
        linkBackend["../android_backend_tauri.md"]
    end

    UA --> TA
    UA --> linkState
    TA -->|"store 读写"| linkState
    linkParent --> UA
    linkParent --> TA
    linkAuth --> TA
    linkMonitor --> TA
    linkSettings --> TA
    linkAccount --> TA
    linkEntry -->|"挂载调用"| UA
    TA -->|"invoke/listen"| linkBackend
```

## 相关模块参考

- 状态存储实现细节：[安卓前端 Hooks — 状态存储](android_frontend_hooks_state_stores.md)（`useConfigStore`、`useAuthStore`、`useQualityStore`、`useLogToastStore`、`useThemeStore`、`useAdapterStore`）
- 动画与表单因子运行时：[安卓前端 Hooks — 动画运行时](android_frontend_hooks_animation_runtime.md)
- Hooks 父模块总览：[安卓前端 Hooks](../android_frontend_hooks.md)
- 入口与崩溃恢复：[安卓前端入口与 Mock](../android_frontend_entrypoint_mocks.md)
- 后端命令实现：[安卓后端 Tauri](../android_backend_tauri.md)（`get_init_data`、`render_heartbeat`、`check_portal_status`、`do_login` 等命令）
- 构建配置：[构建与部署配置 — 安卓构建](../build_deployment_configuration/build_deployment_configuration_android_build.md)（`android/frontend/package.json` 的 `build`/`dev`/`typecheck` 脚本）
