---
title: android_backend_tauri 模块总览
type: module
source_files:
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/MainActivity.kt
  - android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt
  - android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/android_state.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/face_model_cmds.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/login_history.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/src-tauri/src/quality_cmds.rs
  - android/src-tauri/src/quality_history.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/system_cmds.rs
  - android/src-tauri/src/update_cmds.rs
  - android/src-tauri/src/vpn_cmds.rs
  - android/src-tauri/src/battery_cmds.rs
  - android/src-tauri/src/cpu_affinity.rs
  - tauri-app/src-tauri/src/bin/campus-login-core.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/commands/background.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/login.rs
  - tauri-app/src-tauri/src/commands/mod.rs
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/updater.rs
  - tauri-app/src-tauri/src/commands/vpn.rs
  - tauri-app/src-tauri/src/platform/autostart.rs
  - tauri-app/src-tauri/src/platform/best_route.rs
  - tauri-app/src-tauri/src/platform/console_output.rs
  - tauri-app/src-tauri/src/platform/core_autostart.rs
  - tauri-app/src-tauri/src/platform/dns_config.rs
  - tauri-app/src-tauri/src/platform/ecoqos.rs
  - tauri-app/src-tauri/src/platform/elevation.rs
  - tauri-app/src-tauri/src/platform/gpu.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
  - tauri-app/src-tauri/src/platform/icmp_probe.rs
  - tauri-app/src-tauri/src/platform/identity.rs
  - tauri-app/src-tauri/src/platform/metric.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/platform/route_table.rs
  - tauri-app/src-tauri/src/platform/rtss_compat.rs
  - tauri-app/src-tauri/src/platform/task_proxy.rs
  - tauri-app/src-tauri/src/platform/toast.rs
  - tauri-app/src-tauri/src/self_service/mod.rs
  - android/core-logic/Cargo.toml
  - android/core-logic/src/boot.rs
  - android/core-logic/src/config_tmp.rs
  - android/core-logic/src/file_lock.rs
  - android/core-logic/src/lib.rs
  - android/core-logic/src/notify.rs
  - android/core-logic/src/platform_sig.rs
  - android/core-logic/src/status.rs
  - android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/CampusLoginApplication.kt
  - android/src-tauri/src/core/jni_platform.rs
  - android/src-tauri/src/core/login.rs
  - android/src-tauri/src/core/mod.rs
  - android/src-tauri/src/core/monitor.rs
  - android/src-tauri/src/core/net_state.rs
  - android/src-tauri/src/core/platform.rs
  - android/src-tauri/src/core/runner.rs
  - android/src-tauri/src/core/status_file.rs
  - android/src-tauri/src/core/ui_bridge.rs
  - android/src-tauri/src/core_bridge_cmds.rs
  - tauri-app/src-tauri/src/platform/elevated_daemon.rs
  - tauri-app/src-tauri/src/platform/if_stats.rs
  - tauri-app/src-tauri/src/platform/memory_trim.rs
  - tauri-app/src-tauri/src/platform/tcp_probe.rs
tags: []
generated_by: fsoft-native
fsoft_page: android_backend_tauri.md
---
# android_backend_tauri 模块总览

`android_backend_tauri` 是校园网登录助手**安卓端 Tauri 后端**（crate `campus-login-android`，lib `campus_login_android_lib`，源码位于 `android/src-tauri/src`）的总入口模块。它不实现任何登录协议本身——协议、VPN、自助服务、网络质量等能力全部单点复用跨平台核心 `campus-login` crate——而是负责三件事：

1. **命令面**：由 `lib.rs::run` 通过 `tauri::generate_handler!` 注册一组 `#[tauri::command]`，把 WebView 的 `invoke("xxx")` 翻译为对协议核心、安卓原生插件、后台核心（`CoreService`）与本地文件系统的操作，命令名/参数/返回结构与桌面端 `commands/*.rs` 保持同构，前端近零适配。
2. **后台巡检**：巡检、自动重登、定时登录/注销、夜间出站与运营商切换、WiFi 就寝守则全部运行在**主进程**的 `CoreService` 前台服务里（经 JNI 拉起 `core::runner`），界面被销毁后服务仍存活（force-stop 是整包杀，分进程并不额外保活）；配套提供电池优化白名单引导、厂商自启页跳转与大小核绑核省电。
3. **持久化底座**：`config.json`（Keystore AES-GCM 加密、原子写、跨进程锁、schema 迁移）、`accounts/*.json` 多账号档案、登录历史与质量历史、生物识别验证门时间戳，UI 与后台核心共用同一份磁盘契约。

因此该模块的核心特征是**主进程前台服务 + 三条协作通道**：Binder 命令转发（同步阻塞，包 `spawn_blocking`）、`core_status.json` 状态文件（原子写 + 存活/过期判定）、广播事件回投（核心侧无 `AppHandle` 可直发，经 Kotlin `CoreEvents.nativeEmitToUi` 回到 Tauri 侧再 `emit` 给 WebView）。

## 架构总览

```mermaid
flowchart TB
    subgraph FE["WebView 前端 android/frontend"]
        UI["React 组件 / hooks / tauriApi.ts"]
    end

    subgraph UIPROC["主进程 campus_login_android_lib"]
        HANDLER["invoke_handler<br/>lib.rs::run"]
        CMDS["命令面<br/>protocol / account / self_service / vpn / update"]
        CFG["配置与持久化<br/>config_state / account_cmds / history"]
        BRIDGE["core_bridge_cmds<br/>Binder 转发"]
        UIB["core::ui_bridge<br/>事件回投"]
        APP["CampusLoginApplication<br/>Shizuku 多进程支持"]
    end

    subgraph COREPROC["CoreService 前台服务（与 UI 同进程）"]
        SVC["CoreService"]
        MON["core::monitor<br/>巡检 / 定时 / 夜切"]
        NETSTATE["core::net_state<br/>Portal 探测单点"]
    end

    subgraph PLUGINS["安卓原生插件"]
        NB["network-bind<br/>bindProcessToNetwork / SSID"]
        KS["keystore<br/>AndroidKeyStore AES-GCM"]
        MS["foreground-service<br/>通知 / installApk / core 转发"]
    end

    subgraph SHARED["跨平台共享 crate"]
        LIB["campus-login<br/>auth / vpn / self_service / quality"]
        LOGIC["campus-core-logic<br/>状态文件 / 跨进程锁 / 纯逻辑"]
    end

    DISK[("应用私有目录<br/>config.json + config.json.lock<br/>accounts / login-history / quality_history")]

    UI -->|invoke| HANDLER
    HANDLER --> CMDS
    HANDLER --> CFG
    CMDS --> LIB
    CMDS --> NB
    CFG --> KS
    CMDS --> BRIDGE
    BRIDGE -->|Binder 命令| SVC
    SVC --> MON
    MON --> NETSTATE
    MON --> LIB
    MON --> LOGIC
    MON -->|原子写状态文件| DISK
    CFG -->|进程内锁 + 跨进程锁 + tmp rename| DISK
    MS --> SVC
    APP -.->|"enableMultiProcessSupport(true)"| COREPROC
    SVC -->|事件广播| UIB
    UIB --> UI
```

平台专属能力统一以 `#[cfg(mobile)]` / `#[cfg(not(mobile))]` 双分支实现：移动端调插件，非移动端返回 `Err("仅安卓端支持")` 或空实现，使掩码、迁移、退避、白名单、tag 提取等纯逻辑可在 host 上直接单测。

## 子模块构成

```mermaid
flowchart LR
    CMD["android_backend_tauri_commands<br/>命令面"]
    MON["android_backend_tauri_monitoring<br/>后台巡检与保活"]
    ST["android_backend_tauri_state_storage<br/>状态与持久化"]

    CMD -->|"core_bridge_cmds::call 转发 start / stop / trigger / login"| MON
    MON -->|"core_status.json 供 get_background_status 读取"| CMD
    CMD -->|"current_settings / save_config"| ST
    MON -->|"update_at_dir 事务写配置"| ST
```

| 子模块 | 职责要点 |
| --- | --- |
| [android_backend_tauri_commands](android-backend-tauri-commands.md) | 命令面全集：网络探测与通道（`protocol_cmds` / `campus_detect` / `quality_cmds`）、配置与多账号（`config_state` / `account_cmds`）、自助服务与 WebVPN（`self_service_cmds` / `vpn_cmds`）、系统聚合与更新（`system_cmds` / `update_cmds` / `face_model_cmds`）；`ensure_wifi_bound` 等全链路前置不变式。 |
| [android_backend_tauri_monitoring](android-backend-tauri-monitoring.md) | 主进程前台服务里的巡检循环（`core::monitor`）、命令转发与状态读取（`core_bridge_cmds`）、电池优化与厂商设置页（`battery_cmds`）、小核绑核（`cpu_affinity`）；三态裁决、静默期闸、注销保护期、失败熔断等护栏，以及在线稳定时按 `adaptive_interval_ms` 拉长兜底巡检周期的自适应退避。 |
| [android_backend_tauri_state_storage](android-backend-tauri-state-storage.md) | `AndroidState` 内存态、`CryptoBridge` 加解密、`config.json` 原子写与 schema 迁移（当前 v8）、`accounts/*.json` 与自动建号、登录/质量历史、`identity_gate` 验证门，以及桌面端同契约参照实现。 |

## 关键设计约束

- **协议单点**：登录/注销/自助服务/WebVPN/质量检测只在 `campus-login` crate 实现，安卓侧仅做参数校验、平台能力适配与命令形状对齐；定时判定、静默期、夜切/出站切换等纯函数同样复用共享 crate，避免双端语义漂移。
- **明文只在内存**：磁盘上 `settings.password` 恒为空串，真实密码存于 `passwordCipher`；加密失败即保存失败，绝不降级为明文；任何出站配置快照都经 `masked_for_display()` 把 `password` 与 `vpn_password_enc` 替换为 `***`。
- **跨进程写必须双锁**：进程内 `tokio::sync::Mutex` + `config.json.lock` 文件锁，`update_at_dir` 的闭包在锁内对刚读出的文件内容执行；落盘成功后才刷新内存缓存与 `emit config-changed`。
- **平台能力经 trait 注入**：后台核心不启动 tauri，`CorePlatform` 中刻意不出现 tauri 类型，通知 / WifiLock / WiFi 绑定 / 就寝守则由 `JniPlatform` 实现，保证核心可脱离 Activity 运行。
- **UI 启动按配置补拉起核心**：安装更新/强制停止后 `START_STICKY` 与开机广播都不保证及时恢复核心，因此 `lib.rs::ui_startup` 读到配置后仅当 `enable_background_check` 为真才经 `core_bridge_cmds::call(&app, "start", "{}")` 转发一次 `start`；关闭时打开 UI 不得凭空出现常驻通知。核心侧的 `start` 亦再查一次该配置，为假时直接返回 `{"isRunning": false}`，外部关闭监测时启动命令不能把用户开关重新打开。
- **release profile 有硬约束**：`lto = "fat"`、`codegen-units = 1`、`strip = true`，且**不设 `panic = "abort"`**——async 任务 panic 需 unwind 捕获为任务失败，abort 会连带杀死前台服务与保活。
- **核心与 UI 必须同进程**：`CoreService` 承载后台核心，Manifest 中不写 `android:process`。与 UI 同进程即可在界面销毁后存活（force-stop 是整包杀，分进程并不额外保活），更关键的是 Shizuku 的 binder 只在 provider 进程（主进程）可靠注入——拆到独立进程会让核心永久拿不到 binder，`cmd wifi` 与 SSID 读取随之静默失效。`CampusLoginApplication` 仅保留 `ShizukuProvider.enableMultiProcessSupport(true)`，为日后引入独立进程预留基础 binder 分发能力。

## 安卓宿主入口：`CampusLoginApplication`

`android/src-tauri/gen/android/app/src/main/java/com/campuslogin/client/CampusLoginApplication.kt` 是应用级 `Application` 子类，只做一件事：在 `onCreate` 中调用 `ShizukuProvider.enableMultiProcessSupport(true)`，让 Shizuku 的 binder 在**每个进程**都可用（多进程支持）。

- 后台核心（`CoreService`）运行在**主进程**——前台服务与 UI 同进程即可在界面销毁后存活，而 Shizuku 的 binder 只在 provider 进程可靠注入，拆到独立进程会让核心永久拿不到 binder；
- `enableMultiProcessSupport(true)` 仍保留：若日后再引入独立进程，它能提供基础 binder 分发（主进程始终是最可靠的宿主）；
- 因此 `CampusLoginApplication` 不再需要判定主进程、也不再向本包补播 binder 广播——`pingBinder()` 在主进程天然可用，`cmd wifi` 与 SSID 读取不再依赖任何跨进程补播或自注入兜底（见 [android_plugins_foreground_service](android-plugins-foreground-service.md) 的 9.2 节）。

## 相关模块

- 原生插件：[android_plugins_keystore](android-plugins-keystore.md)、[android_plugins_network_bind](android-plugins-network-bind.md)、[android_plugins_foreground_service](android-plugins-foreground-service.md)。
- 前端消费方：[android_frontend_monitoring_network](android-frontend-monitoring-network.md)、[android_frontend_settings](android-frontend-settings.md)、[android_frontend_hooks_state_stores](android-frontend-hooks-state-stores.md)、[android_frontend_account_management](android-frontend-account-management.md)。
- 桌面端同构参照：[desktop_backend_monitoring](desktop-backend-monitoring.md)、[desktop_backend_configuration](desktop-backend-configuration.md)、[desktop_backend_commands](desktop-backend-commands.md)。
- 构建与打包：[build_deployment_configuration_android_build](build-deployment-configuration-android-build.md)。