# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.3.5 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
> **目标平台**: Windows (x64) + Android
> **通信方式**: Tauri IPC (`invoke` / `listen`)
> **文档分层**: 一~五章为手册层（概览 / 架构速查 / 关键约定 / 踩坑记录 / 决策记录），新人从这里读起；附录 A~H 为参考层（模块逐文件详解 / IPC 全表 / 依赖 / 安全 / 性能 / 版本号流程），内部沿用原 4.x/5.x/6.x/7.x 编号，正文的"见 4.x"类引用均指向附录。

---

## 一、项目概览

### 1.1 定位

CampusLogin 是无锡学院校园网（Dr.COM ePortal）自动登录助手，**Tauri 2 双端同构应用**：Windows 桌面端（`tauri-app/`）与安卓端（`android/`，2026-09-10 并入本仓库）共用同一协议核心（安卓以 Cargo path 依赖桌面 crate），登录/注销/Portal 探测/自助服务/网络质量协议零复制，命令面同名对齐，UI 各自适配（桌面 Dock 布局 / 安卓双外壳：平板复用 Dock 布局 + 手机移动底部导航）。提供一键登录/注销、自动重连、校园网智能检测、DNS 智能解析与优化、网络质量监测、多账号管理、运营商绑定与自助服务查询。

### 1.2 技术栈与核心特性

| 层 | 桌面端 | 安卓端 |
|----|--------|--------|
| 框架 | Tauri 2（WebView2） | Tauri 2（minSdk 29 / Android 10+） |
| 前端 | React 19 + TypeScript + Vite 6 + TailwindCSS 3.4 + Framer Motion 12 + GSAP 3 | 同左（独立复刻代码库，`VITE_PLATFORM=android` 分支） |
| 状态 | zustand ^5（按领域拆分 store） | 同左 |
| 后端 | Rust + Tokio + reqwest 0.12 + hickory-resolver + tokio-rustls | Rust + Tokio（`campus-login` path 依赖桌面协议核心） |
| 密码加密 | Windows DPAPI | AndroidKeyStore AES-256-GCM（手写插件） |
| 敏感操作验证 | Windows Hello（UserConsentVerifier） | BiometricPrompt（`@tauri-apps/plugin-biometric`） |
| 平台集成 | Win32/WinRT（适配器/注册表/DXGI/UAC 提权/托盘） | 前台服务 + WifiLock/WakeLock、`bindProcessToNetwork`、FileProvider 装 APK |

核心特性：一键登录 / 两步注销（Radius 注销先行、成功即止 + MAC 解绑收尾）、后台巡检断线自动重连、校园网三级检测、DNS 动态评分 + 应用级 DoH（RFC 8484）+ 一键 DNS/DoH 优化、网络质量并发检测与增量推送、多账号管理、运营商账号绑定、自助服务查询（在线信息/上网记录/踢下线）、敏感操作验证门、更新检查（镜像降级 + SHA256）、主题系统、中英双语、开机自启（桌面注册表 / 安卓 BootReceiver + 前台服务）。

---

### 1.3 目录结构

```
Wxxy-CampusLogin/
├── assets/                          # 截图与素材归档（ui-girl/ 看板娘原图）
├── tauri-app/
│   ├── build.ps1                    # 发布构建脚本 (构建后自动生成 <安装包>.sha256)
│   ├── frontend/                    # React 前端（逐模块详解见附录 B）
│   │   ├── index.html               # HTML 入口（布局验证时可临时注入 __TAURI_INTERNALS__ mock）
│   │   ├── vite.config.ts / vitest.config.ts / tailwind.config.js / tsconfig*.json
│   │   ├── public/                  # 静态资源 (图标 / 赞助码 / girl/ 看板娘场景图 WebP)
│   │   └── src/
│   │       ├── main.tsx / App.tsx   # 入口与主组件（面板路由/deferredPanel 转场/启动加速/崩溃恢复）
│   │       ├── hooks/               # 领域 store ×6 (config/auth/adapter/quality/theme/logToast)
│   │       │                        #   + tauriApi.ts (IPC 封装) + useAppInit 编排 (事件/首载/心跳/快捷键)
│   │       │                        #   + 动画与工具 hooks (useAnimationProfile/useAsyncLock/...)
│   │       ├── lib/                 # utils(safeStorage)/color/latency/animations/easing + *.test.ts
│   │       ├── i18n/locales/        # zh.json / en.json
│   │       ├── auth/ account/ monitor/ network/ settings/   # 业务域（面板组件 + useXxx hook + types）
│   │       ├── shared/              # 共享组件 (LogPanel/ErrorBoundary/ConfirmDialog/SponsorCard/MascotFigure/Toast...)
│   │       └── components/          # layout/ (DockNav/RightPanel/TitleBar) + ui/ (shadcn 风格 12 件)
│   └── src-tauri/                   # Rust 后端（逐模块详解见附录 A）
│       ├── tauri.conf.json          # 版本号权威源（build.rs 注入 APP_VERSION，见附录 H）
│       ├── build.rs / Cargo.toml / capabilities/default.json / icons/
│       └── src/
│           ├── main.rs / lib.rs     # 双模块树入口（panic hook / --helper 拦截 / runtime）
│           ├── app/                 # 应用生命周期: startup/tray/window/shortcut/heartbeat/webview_recovery/shutdown
│           ├── commands/            # 56 个 Tauri 命令，按领域 8 文件 (config/login/background/network/system/account/self_service/updater)
│           ├── auth/                # 认证协议核心: portal/protocol/session/service/failure_tracker/dual_adapter_executor
│           ├── network/             # client/adapter/discovery/adapter_cache/dhcp/subnet/dns/timing/quality/dns_setup
│           ├── monitor/             # 后台巡检: watcher 门面 + background_check/background_task/auto_auth/latency/adapter_watch 等 10 子模块
│           ├── config/              # model(40字段)/persist(原子写+账号+历史)/validate(校验+迁移)
│           ├── infra/               # state/(ConfigStore/NetworkState/ExitStateStore) + logger/events/task_manager/lifecycle/notification/async_util
│           ├── platform/            # Windows 交互: dns_config/elevation/gpu/identity/autostart/helper_spawn/console_output/toast(更新提醒 WinRT toast)
│           ├── account/             # crypto.rs (Windows DPAPI)
│           ├── self_service/        # Dr.COM Self 自助服务协议
│           └── helper/ update/      # --helper 提权子进程；更新检查/下载/SHA256 校验
├── android/                         # 安卓端（2026-09-10 由独立仓库并入；详解见附录 A §4.16 / B §5.14）
│   ├── frontend/                    # React 前端（桌面复刻+移动裁剪；VITE_PLATFORM=android 平台分支；平板/手机双外壳——useFormFactor 短边≥600dp 判平板渲染 TabletShell 桌面布局）
│   ├── src-tauri/                   # 安卓 Rust 后端（14 模块；campus-login path 依赖桌面协议核心）
│   │   └── gen/android/             # tauri CLI 生成的 gradle 工程（版本化提交，build 产物不入库）
│   └── plugins/                     # 手写 Tauri 插件: keystore / foreground-service(MonitorService+installApk) / network-bind
├── CODE_WIKI.md                     # 本文档
├── AGENTS.md                        # AI 编码助手项目约定
├── README.md                        # 项目说明
├── version.json                     # 版本号配置（推送与 Release 发布绑同一次操作）
└── .gitignore
```

### 1.4 启动 / 构建 / 测试命令

```bash
# ── 桌面端 ────────────────────────────────────────────────
cd tauri-app/frontend && npm install     # 前端依赖
cd .. && npm install                     # 根层依赖（含 tauri CLI）

cd tauri-app && npx tauri dev            # 开发模式

# 测试与检查（改动后基线：cargo test 493 全绿 = lib 246 + bin 同套 246 + 回归 1）
cd tauri-app/src-tauri && cargo test
cargo clippy --all-targets -- -D warnings
cd ../frontend && npm test               # vitest 74 用例
npx tsc --noEmit --incremental           # 类型检查（禁止 tsc -b，见 §四 踩坑）

# 发布构建（前端打包 + Tauri 打包 + 生成 .sha256）
pwsh tauri-app/build.ps1

# ── 安卓端 ────────────────────────────────────────────────
# 前置：ANDROID_HOME/NDK_HOME/JAVA_HOME 已配置；rustup target add aarch64-linux-android；
#       Windows 开发者模式（允许符号链接）
cd android/frontend && npm install
npm run build                            # tauri CLI 不跑 beforeBuildCommand，必须先单独构建
cd ../src-tauri
npx @tauri-apps/cli android dev                       # 真机/模拟器调试
npx @tauri-apps/cli android build --target aarch64 --apk   # 产出已签名 APK
# ⚠ host cargo check 在 android/src-tauri 基线即失败（mobile-only 插件），验证只认交叉编译
```

> 布局/交互类改动的验证方法：临时向 `tauri-app/frontend/index.html` 注入 `__TAURI_INTERNALS__` mock + vite dev 起本地服务，**用后完整还原（git diff 必须干净）**。分支纪律、CHANGELOG 记录等流程约定见 `AGENTS.md`；版本号五处同步与 Release 检查清单见附录 H。

---

## 二、架构速查

### 2.1 分层架构（桌面端）

```
┌─────────────────────────────────────────────────────┐
│                   用户界面 (UI Layer)                 │
│  React 19 + TypeScript + Tailwind CSS + Radix UI    │
│  Framer Motion 动画 | GSAP 3 | shadcn/ui | zustand  │
├─────────────────────────────────────────────────────┤
│            领域状态管理层 (State Layer)              │
│  useConfigStore / useAuthStore / useAdapterStore     │
│  useQualityStore / useThemeStore (zustand 按领域拆分) │
│  useLogToastStore — 日志/Toast                       │
│  useAppStore — 兼容壳 (3行 re-export)                │
│  tauriApi — Tauri IPC 通信封装 (纯模块，非 hook)     │
├─────────────────────────────────────────────────────┤
│                IPC 通信层 (Bridge Layer)              │
│  Tauri invoke (请求-响应) | Tauri listen (事件推送)   │
│  前端 ←→ Rust 后端                                   │
├─────────────────────────────────────────────────────┤
│                 业务逻辑层 (Logic Layer)              │
│  ┌─────────────────────────────────────────────────┐ │
│  │  monitor/watcher.rs (门面+启动聚合, 54行, re-export background_check/background_task) │ │
│  │    └─→ run_startup_tasks (启动期聚合 3 个 task_manager.spawn) │ │
│  │  monitor/background_check.rs (后台检测主体, run_background_check_blocking) │ │
│  │    ├─→ auth/failure_tracker.rs (认证+Portal请求失败计数, 阈值5) │ │
│  │    ├─→ monitor/auto_auth.rs (自动登录/断线重连)  │ │
│  │    ├─→ infra/lifecycle.rs  (自动退出倒计时+校园网退出) │ │
│  │    ├─→ monitor/latency.rs  (质量通知/延迟循环)   │ │
│  │    └─→ monitor/adapter_watch.rs (适配器监控,可取消) │ │
│  │  app/ — 应用生命周期 (startup/tray/window/shortcut/heartbeat/shutdown) │ │
│  │  auth/ — 认证模块 (6个子模块: session/protocol/portal/service/failure_tracker/dual_adapter_executor) │ │
│  │  auth/portal.rs — Portal 检测 (block_on_http 同步-异步桥接) │ │
│  │  network/ — 网络检测/延迟测试/质量检测 (9个业务子模块 + discovery/ 子目录含 registry/windows) │ │
│  │  network/timing.rs — DNS智能解析/DoH/评分系统     │ │
│  │  platform/dns_config.rs — DNS/DoH 检测与设置      │ │
│  │  account/ — 多账号管理 + DPAPI加密                │ │
│  │  config/ — 配置管理 (model/persist/validate)     │ │
│  └─────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────┤
│                 系统交互层 (System Layer)             │
│  Win32 API — 适配器查询(GetAdaptersAddresses)        │
│  ShellExecuteW — UAC 提权 (替代 PowerShell)          │
│  WebView2 COM — 内存管理 (ICoreWebView2_19.SetMemoryUsageTargetLevel) │
│  DXGI — GPU 信息检测 (CreateDXGIFactory1/IDXGIAdapter1) │
│  Win32 GDI — 显示器刷新率检测 (EnumDisplaySettingsW)   │
│  winreg — 注册表读写 (DNS/DoH 配置)                  │
│  reqwest — HTTP 请求 (TLS 1.3强制+1.2回退)          │
│  hickory-resolver — 传统 DNS 解析                    │
│  tokio-rustls — DoH TLS 连接 (RFC 8484)             │
│  tokio — 异步运行时                                  │
│  Windows Registry — 开机自启/DNS配置                 │
└─────────────────────────────────────────────────────┘
```

### 2.2 Commands 模块依赖关系

```
// [架构说明] 模块间耦合关系
//
//  依赖链（箭头表示 "调用/依赖"）：
//
//  monitor/watcher (门面) ──re-export──→ monitor/background_check (检测主体)
//                                            │
//                                            ├──→ auth/failure_tracker (认证+Portal请求失败计数, 阈值5)
//                                            ├──→ monitor/auto_auth ──→ infra/lifecycle
//                                            │         │                    │
//                                            │         └──→ infra/notification (emit_notification)
//                                            └──→ infra/lifecycle
//                              (质量检测已收敛至 monitor/latency 定时循环独占驱动，见 4.11)
//
//  monitor/background_task (调度层) ──→ task_manager.spawn("background_check") ──→ background_check
//
//  commands/login ──→ auth/service (full_login/full_logout 统一入口 + post_login_handler)
//                  │     └──→ auth/session ──→ auth/protocol (两步注销)
//                  │                          └──→ auth/portal (Portal 检测)
//                  │     post_login_handler 内部：infra/lifecycle (start_auto_exit)
//                  │                             + monitor/watcher (run_background_check，仅 enable_background_check 时)
//                  └──→ infra/events (EventBus emit_login_log)
//
//  infra/lifecycle ──→ infra/notification (emit_notification)
//
//  monitor/adapter_watch ──→ infra/events (EventBus 适配器变更事件)
//                          + 依赖 state + network, CancellationToken 可退出
//
//  commands/network_cmd ──→ network (适配器/DHCP/质量检测)
//                       ├──→ monitor/watcher (check_campus_network，经 re-export)
//                       ├──→ monitor/latency (spawn_latency_test_loop)
//                       ├──→ auth/portal (check_portal_full)
//                       ├──→ platform/dns_config (DNS/DoH 读写)
//                       └──→ platform/elevation (UAC 提权)
//
// 所有模块通过 AppState 共享状态（见 infra/state/ 子目录），状态一致性依赖原子操作和 ArcSwap 保证
// 后台任务通过 BackgroundTaskManager 统一管理 cancel token，响应退出信号避免退出挂起
```

### 2.3 关键数据流

```
用户操作 → React组件 → 领域 store (useConfigStore/useAuthStore/useAdapterStore/useQualityStore) → tauriApi.invoke()
                                                                                                       ↓
                                                                                                  Tauri IPC
                                                                                                       ↓
                                                                                            #[tauri::command] Rust函数
                                                                                                       ↓
                              AppState (ConfigStore + TaskFlags + BackgroundTaskManager + NetworkState + ExitStateStore + UpdateStats[含5个原子字段: last_update_check_epoch_ms/update_notified(AtomicBool)/last_disabled_notification_ms/last_render_heartbeat_ms/last_network_change_notification_ms])
                                                                                                       ↓
                                                                                              Win32 API / HTTP / 注册表 / 文件系统
                                                                                                       ↓
                                                                                              结果返回 / 事件推送 (emit)
                                                                                                       ↓
                                                                                              tauriApi.onXxx() → useEventListeners → 领域 store → UI更新
```

### 2.4 模块职责速查

**桌面后端** `tauri-app/src-tauri/src/`（逐文件详解见附录 A）:

| 模块 | 职责 |
|------|------|
| `main.rs` + `app/` | 入口（panic hook / `--helper` 拦截 / runtime）与应用生命周期（启动装配/托盘/窗口/快捷键/心跳/退出分流） |
| `commands/` | 56 个 Tauri 命令，按领域 8 文件（config/login/background/network/system/account/self_service/updater） |
| `auth/` | 认证协议核心：Portal 检测（portal）、登录/两步注销请求（protocol）、会话封装（session）、服务编排（service）、失败计数（failure_tracker）、双适配器并行执行器（dual_adapter_executor） |
| `network/` | 网络基础设施：适配器发现与缓存（discovery/adapter_cache）、DHCP 与 MAC（dhcp）、/18 子网与网关（subnet）、DNS 评分与 DoH 智能解析（dns）、HTTP 计时（timing）、质量并发检测（quality）、DNS 一键设置（dns_setup） |
| `monitor/` | 后台巡检：watcher 门面 + background_check 主体 + background_task 调度、自动登录（auto_auth）、延迟测试循环（latency）、适配器监控（adapter_watch） |
| `config/` | 配置模型（40 字段）/ 原子持久化 / 校验与迁移 |
| `account/` | DPAPI 加密（crypto.rs）；多账号命令逻辑在 commands/account.rs |
| `infra/` | 全局状态（state/ 三 Store CAS 快照）、日志、事件总线（15 emit）、后台任务管理、退出生命周期、系统通知 |
| `platform/` | Windows 交互：DNS/DoH 设置、UAC 提权、GPU 检测、Windows Hello、开机自启、helper 子进程启动 |
| `self_service/` | Dr.COM Self 自助服务协议（登录/绑定/dashboard/上网记录/踢下线/明文揭示） |
| `helper/` + `update/` | `--helper` 提权子进程（改 MAC/设 DNS）；更新检查/下载/SHA256 校验/安装 |

**桌面/安卓共用前端形态**（桌面 `tauri-app/frontend/src/` 详解见附录 B，安卓差异见附录 B §5.14）:

| 模块 | 职责 |
|------|------|
| `hooks/` | 领域 store ×7（zustand：config/auth/adapter/quality/theme/logToast）+ `tauriApi.ts` IPC 封装（56 API + 15 事件）+ 初始化编排（useAppInit → 事件/首载/心跳/快捷键） |
| `auth/ account/ monitor/ network/ settings/` | 业务域面板（组件 + useXxx hook + types） |
| `shared/ components/` | 共享组件（日志/Toast/错误边界/看板娘/赞助浮层）与 shadcn 风格基础 UI |
| `lib/ i18n/` | 工具（latency/color/safeStorage）与中英文案 |

**安卓端** `android/`（后端详解见附录 A §4.16，前端差异见附录 B §5.14）:

| 模块 | 职责 |
|------|------|
| `src-tauri/src/`（14 模块） | 命令面包装（protocol/self_service/account/system/quality/update_cmds）、校园网探针（campus_detect）、Keystore 加密配置（config_state）、监控状态机（monitor_loop）、验证门 TTL（identity_gate）、绑核（cpu_affinity） |
| `plugins/` | 手写 Tauri 插件三件：keystore（AES-256-GCM）/ foreground-service（前台服务保活+自启+装 APK）/ network-bind（进程绑定 WLAN） |
| `frontend/` | 独立复刻 React 前端（移动裁剪 + `VITE_PLATFORM=android` 分支 + BottomNav） |

### 2.5 核心文件索引

| 要找什么 | 去哪 |
|----------|------|
| 桌面应用入口 / 命令注册点 | `tauri-app/src-tauri/src/main.rs` → `app/startup.rs`（`run()` 的 `generate_handler!`，新增命令必须在此注册） |
| 登录/注销协议实现 | `auth/protocol.rs`（请求模板/两步注销/JSONP 解析）、`auth/portal.rs`（状态检测，80 页面探测 vs :801 协议） |
| 配置模型 / 持久化 / 校验 | `config/model.rs`（40 字段+掩码出口）、`config/persist.rs`（atomic_write/账号/历史）、`config/validate.rs` |
| 全局状态 | `infra/state/`（AppState / ConfigStore / NetworkSnapshot，ArcSwap CAS） |
| 事件推送 | `infra/events.rs`（EventBus 15 个 emit）+ 附录 C 事件表 |
| 密码加密 | `account/crypto.rs`（DPAPI）；安卓 `android/plugins/keystore/` |
| 前端 IPC 封装 | `hooks/tauriApi.ts`（新增命令 → 此处加方法 + `useEventListeners` 加监听） |
| 前端入口 / 面板路由 | `main.tsx` → `App.tsx`（activePanel switch，转场用 deferredPanel） |
| 版本号权威源 | `tauri-app/src-tauri/tauri.conf.json` 的 `version`（build.rs 注入 `APP_VERSION`；同步清单见附录 H） |
| 发布构建脚本 | `tauri-app/build.ps1`（产物 + .sha256） |
| 安卓入口 / 命令注册 | `android/src-tauri/src/lib.rs`（44 命令 + 5 插件 + run_startup_tasks） |
| 安卓包名 / 窗口 / minSdk | `android/src-tauri/tauri.conf.json`（identifier `com.campuslogin.client`） |
| 安卓 Gradle 签名与产物命名 | `android/src-tauri/gen/android/app/build.gradle.kts`（release signingConfig + outputFileName） |
| IPC 命令与事件全表 | 附录 C |

---

## 三、关键约定

只列本项目特有、违反必出问题的规矩；通用工程规范不重复（流程类见 `AGENTS.md`）。

1. **协议核心单点共享（双端铁律）**：登录/注销/Portal/自助服务/网络质量的实现只存在于桌面 crate（`tauri-app/src-tauri`），安卓以 Cargo path 依赖复用，**禁止复制协议逻辑**。桌面 cfg 门控模块（app/helper/monitor/update）对安卓不可见。
2. **命令面两端同名对齐**：前端 `tauriApi` 一套接口两端消费；安卓命令与桌面同名同参（`do_login`/`save_config`…），差异只在实现。新增命令：桌面在 `app/startup.rs` 注册，安卓在 `android/src-tauri/src/lib.rs` 注册，并在 `hooks/tauriApi.ts` 加方法。
3. **敏感信息出站唯一出口**：一切把 Config 发往前端的路径必经 `Config::masked_for_display()`（桌面）/ `config_state::masked_for_display`（安卓），**禁止手工逐字段打码**（漏一处即明文出站，有回归单测锁死）。密码"空串/MASK=未修改回退已存值、显式清除走 `clear` 标志"语义两端同构。日志/错误/事件 payload 一律不得携带 password。
4. **验证门分级**：改变外部状态的命令设门（bind_operator / self_offline_session），只读查询有意不设门（总览卡自动刷新依赖免验证拉取），明文查看无论开关强制验证；前端门 TTL 570s + 后端 600s 复核（后端 TTL 才是真防线）。
5. **通知单通道**：系统通知只有 `emit_notification`（窗口不可见才发）；应用内提示走业务专用事件（`auto-login-result`/`login-log`/…）；`enable_notification` 只控制系统通知。新增提醒先想清楚走哪条通道，不要双发。
   - **2026-09-12 起系统通知带看板娘头像**：`emit_notification(app, title, body, mascot)` 第 4 参为变体名（`mascot-alert` 等）。Windows 桌面优先走 `platform/toast.rs::show_system_toast` 自组 WinRT toast（appLogoOverride 圆形头像；插件 notify-rust 在 Windows 不暴露图片参数，带不出娘），失败降级插件纯文本；安卓该函数无图，安卓系统通知的娘大图由 `monitor_loop::notify_system` 的 `large_icon`（drawable 资源名）负责。头像 PNG 经 `bundle.resources` 打包（`resources/mascot-toast/`，仅 PNG——toast 不支持 webp），运行时 `resource_dir()` 拼成 `file:///` URL。
6. **适配器操作范围**：操作类流程（检测/登录/注销/DNS 设置/DHCP）只作用于 `resolve_adapter_names` 解析出的主/副适配器；UI 展示类遍历全部。前端 `network/adapters.ts::resolveAdapterNames` 与后端**同源规则**，改任一侧必须同步另一侧（`adapters.test.ts` 锁行为）。
7. **质量检测键与驱动者**：`details`/`metrics` 字典键为英文标识符（`gateway`/`aliDns`/`bilibili`…），显示名走 i18n `quality.names.*`，前后端键同步改（同仓库同发版无兼容窗口）。周期质量检测唯一驱动者是定时测试循环；新增受开关控制的面板需三处联动（App 渲染 null / DockNav 过滤入口 / useInitialDataLoad 跳过恢复）。
8. **面板转场用 `deferredPanel`**：App.tsx 中面板 switch/转场 key/标题/方向全部消费 `useDeferredValue` 后的值，`activePanel` 仅用于 DockNav 高亮与 storage 恢复——用错会内容与标题错位。
9. **版本号五处同步**：`tauri.conf.json` 是唯一权威源（build.rs 注入 `APP_VERSION`），升级按附录 H 清单走；`version.json` 推送与 Release 发布必须同一次操作完成。
10. **验证基线**：`cargo test` 493 全绿 / `cargo clippy -D warnings` / `npx tsc --noEmit --incremental` / `npm test`；安卓只认 `tauri android build` 交叉编译。布局/交互改动需浏览器实测且用后 `git diff` 干净（方法见 §1.4）。
11. **模块声明双树**：`main.rs` 与 `lib.rs` 是两棵独立模块树，新增顶层模块必须两处同时声明（漏一处 bin target E0432）。
12. **安卓生成工程纪律**：`gen/android/` 版本化提交但 build 产物不入库；`settings.gradle` 插件声明为手改；构建再生的 `\n`→`\r\n` 行尾差异是噪音，提交前 `git checkout --` 还原；**改 identifier（包名）= 用户卸载重装**（Keystore 密钥按包名隔离），必须同步 gradle namespace/applicationId/MainActivity 包路径三处并 `git mv` Kotlin 目录；签名证书一经发布不可更换。
13. **Release 资产**：安装包与 `.sha256` 必须同传（校验源全 4xx 默认拒绝安装且用户无法自救）；APK 由 gradle 内置签名与命名；镜像 URL 拼接一律原样拼接不做百分号编码（gh-proxy 403 坑）。

---

## 四、踩坑记录

只留仍会再撞的坑（已修复且不再复发的一次性问题不记）；按域分组，一条一两行。

**工具链 / 构建**

- `npx tsc -b` 会 emit 出 `vite.config.js`/`.d.ts` 污染文件：tsconfig.node.json 是 composite 项目——类型检查一律 `npx tsc --noEmit --incremental`。
- tsc 全量检查必报错 exit 2：自动加载了工作区根 node_modules 里损坏的 `@types/yauzl`——tsconfig 显式 `types: ["vite/client"]`（node 侧 `["node"]`）阻止自动加载全部 @types。
- host `cargo check` 在 `android/src-tauri` 基线即失败：mobile-only 插件权限 host 收集不全——安卓验证只认 `tauri android build` 交叉编译。
- 安卓构建后 APK 没更新：tauri CLI 不跑 `beforeBuildCommand`，frontend/dist 还是旧的——先手动 `npx vite build`。
- `tauri android build` 完成报告指向 `app-universal-release.apk` 但文件不存在：产物名/签名已内置 gradle——以输出目录实际文件为准。
- 构建再生成的 `gen/` schemas 与插件 permissions 出现 `\n`→`\r\n` 全文件 diff：噪音——提交前 `git checkout --` 还原（见 §三-12）。

**协议 / 编码**

- 端口语义严格区分：`:801` 是 ePortal SPA（已在线仍渲染登录页、无状态特征），`:80` 网关页才有状态特征——协议请求强制 801（`ensure_portal_port`），页面探测用 80（详见 §4.5.4.1）。
- 子线程 reqwest panic `there is no reactor running`：裸子线程无 Tokio reactor——同步桥接统一走 `block_on_http` / `spawn_blocking`，禁止在 async 上下文直调同步协议函数。
- 安卓检测不到网关存活：非 root 无 ICMP（SELinux 禁原始 socket）——网关可达性用 TCP connect 表达。
- 本机（UTF-8 模式）正常、用户机（GBK 代码页）关键字匹配失效：netsh/ipconfig/Portal 响应编码随系统——解码统一 UTF-8 优先→OEM 回退（`decode_console_bytes`/`decode_charset_bytes`）。
- ipconfig 失败但代码认为成功：失败退出码常为 0——退出码之外按中英错误关键字兜底判定。

**Windows 平台**

- 不要手动 COM init：windows crate 的 factory cache 自带 `CoIncrementMTAUsage` 自愈，手动 init/uninit 反而破坏它。
- 适配器速度显示 18446744073 Gbps：Windows 未连接时 LinkSpeed 返回 u64::MAX（内部 -1 哨兵）——发现层归 0 表示未知。
- 动画撕裂顿挫（观感掉帧）：WebView2 注入 `--disable-gpu-vsync` 后 BeginFrame 不对齐显示器刷新经 DWM 合并紊乱——vsync 相关参数不要动。
- 旧配置文件缺一个字段整个 Config 加载失败：Config 已有容器级 `#[serde(default)]` + 损坏文件留档 `config.json.corrupt-<ts>.bak`；新增字段记得给 serde default。

**前端**

- 快速切面板时内容与标题错位：转场相关逻辑用了 `activePanel`——必须用 `deferredPanel`（见 §三-8）。
- 容器内 margin 间距不生效：`space-y-4 > * + *` 的 specificity 锁死 margin-bottom——间距用 padding 不用 margin。
- absolute 定位的菜单/按钮被卡片裁掉：`animated-card-interactive` 的 `contain: paint` 裁剪后代越界——菜单用 `createPortal(document.body)`。
- Radix Select 报错/不渲染某项：Item 不接受空串 value——"记住上次"类空值语义用哨兵值映射。
- 安卓 app 白屏在 `get_init_data` 少字段：前端 `useInitialDataLoad` 直接读字段不判空——安卓命令补桌面专属字段空默认（gpuInfo/adapters/…），新增字段两端同步。

**安卓端**

- 改包名后用户密码全部失效：AndroidKeyStore 密钥按包名隔离，密文作废——改 identifier 属破坏性操作（见 §三-12）。
- 应用内下载 APK 后安装失败（Android 7+）：应用私有目录 `file://` URI 必失败（FileUriExposedException）——FileProvider `content://` URI + FLAG_GRANT_READ_URI_PERMISSION。
- Android 10+ 开机自启只有通知没有界面：后台启 Activity 受 ROM 限制（MIUI 需"后台弹出界面"权限）——被拦时用户点开 app 一次即恢复完整链路（设计内行为）。

**发布**

- version.json 推送后全员收到更新通知但下载 404：Release 尚未发布——版本号提交与 Release 发布绑同一次操作（详见附录 H）。

---

## 五、决策记录

带日期的架构/方案决策及理由。**"因为"是关键**——后人需知道当时的权衡，否则容易把选择当唯一答案重做一遍。

- **2026-09-03 WebView2 浏览器参数精简到仅 `--js-flags=--max-old-space-size=512` 并恢复 vsync**：逐项核验原 11 参数——已从 Chromium 移除/Windows 默认即开/Windows 不支持/实验性强开有渲染异常风险；曾注入 `--disable-gpu-vsync` 致帧节奏紊乱（撕裂+顿挫）。渲染交还平台默认（测试最充分的配置），前端动画本就 rAF/vsync 驱动。
- **2026-09-03 Portal 页面探测用 :80、协议请求强制 :801**：801 是 ePortal SPA 管理前端（已在线仍渲染登录页、无状态特征），80 网关页（GBK）内嵌可匹配的登录态特征——两端口语义严格区分不再合并（详见 §4.5.4.1）。
- **2026-09-03 提权操作用 `--helper` 重启自身，弃用 PowerShell**：Rust 直调 Win32/winreg——移除 shell 拼接注入面与 `-EncodedCommand` 编码坑，耗时 200-500ms→1ms。
- **2026-09-04 质量检测驱动者收敛为定时测试循环独占**：曾双定时器叠加导致高频重复探测——代价是不开定时测试则质量面板无周期数据（有意接受，手动检测按钮兜底）。
- **2026-09-04 常用面板静态导入、仅 3 个低频对话框懒加载+启动预取**：React.lazy 切换实测 ~366ms（chunk 缓存命中仍如此），13KB 分包远不值切换延迟——首屏体积与交互流畅取舍取后者。
- **2026-09-04 面板内容渲染用 `deferredPanel`（useDeferredValue）**：120Hz 帧预算仅 8.3ms，快速连切时 React 并发跳过中间面板 mount——真机"快速切换短卡顿"的主修。
- **2026-09-05 身份验证仅 Windows Hello，删除 CredUI/SSPI 回退**：用户明确只要 Hello——设备未配置时返回引导文案而非降级到密码框。
- **2026-09-08 安卓常驻通知走标准安卓协议，不做厂商私有 extras**：ongoing + Chronometer + CATEGORY_SERVICE 即 promoted ongoing 特征——厂商岛态由 ROM 决定，用户侧需开"实时通知提升"类权限（已知体验边界，统一行为优先于逐厂商适配）。
- **2026-09-09 安卓后台检测间隔默认 15s→60s + schema 版本迁移机制**：稳态周期任务 15s 空转耗电——引入 `config_schema_version` 做一次性默认值迁移（迁移落盘后用户显式设回不再覆盖），后续默认值变更沿用该机制。
- **2026-09-11 注销协议改为 Radius 注销先行、成功即止，MAC 解绑收尾**：实测证实 unbind 不踢在线会话、连发无增益，"注销后打不开登录页"属间歇性服务端/环境异常（23 次注销 0 僵尸）——主流实现（cqu-net-auth/eptools/Meirs）均"一次到位绝不连发"，先 logout 后 unbind 保持防御性正确（详见 §4.5.4）。
- **2026-09-12 更新提醒分端定制：桌面 WinRT 自写 toast + 安卓应用内弹窗**：tauri-plugin-notification 桌面端（notify-rust）不暴露 Activated 回调、Windows 无通知点击自定义能力，桌面"点击通知跳关于界面"只能经 windows crate 自组 toast XML（AUMID 借 PowerShell 同款，与既有通知同源；失败降级普通通知）；安卓提醒链路原本只有日志+版本角标，补 `UpdateAvailableDialog` 应用内弹窗（双壳挂载）。
- **（早期重构）删除 auth 层 trait 抽象（AdapterResolver/PortalChecker/ProtocolClient）**：单实现 trait + mock 属无意义抽象——直接调自由函数，测试用真函数。
- **（设计）版本号 build.rs 单权威源注入**：应用内多处版本号引用手改必漏（已发生事故）——`tauri.conf.json` 唯一编辑点，编译期 `env!("APP_VERSION")` 同步，见附录 H。

---

## 附录 A：桌面后端模块详解 (Rust)

### 4.1 应用入口 — `main.rs` + `app/startup.rs`

**职责**: `main.rs` 共 51 行，核心流程：注册 panic hook → 拦截 `--helper` → 构建 Tokio runtime → 注入 Tauri → 调用 `app::startup::run()`。所有应用初始化逻辑（Tauri 插件注册、Setup 钩子、命令注册、窗口/托盘/事件处理）实际位于 `app/startup.rs` 的 `build_runtime()` / `run()` / `setup_app()` 函数中。

**main.rs 关键流程**:

1. **panic hook**: `log_error!` 写入日志文件 + `flush_quick`（500ms超时）确保日志落盘 + `eprintln` 兜底（release 模式 `windows_subsystem=windows` 不可见但保留）
2. **`--helper` 拦截**: 主进程以管理员身份重启自身（提权执行改 MAC / 设 DNS+DoH）时附带 `--helper <op>` 参数；`helper::parse_helper_args` 在 panic hook 之后、Tauri Builder 装配之前拦截，命中则 `helper::run_helper` 执行并 `std::process::exit`（不创建窗口/托盘/监听）；参数非法时退出码 2 不启动正常 UI。详见 §4.15 helper 提权子进程
3. **Tokio runtime 构建**: `build_runtime(core_count)` 根据 CPU 核心数动态配置 `worker_threads(2-8)` 和 `max_blocking_threads(8-64)`
4. **runtime 注入 Tauri**: `tauri::async_runtime::set(handle)` 将 Tokio handle 注入 Tauri 异步运行时
5. **启动应用**: `app::startup::run(core_count)` 进入 Tauri 主循环
6. **退出清理**: flush 日志 → shutdown logger（带超时 join）→ `runtime.shutdown_timeout(5s)`

**app/startup.rs 关键流程** (在 `setup_app` 钩子中):

1. **Tauri 插件注册**:
   - `tauri-plugin-shell` — 执行外部命令
   - `tauri-plugin-notification` — 系统通知
   - `tauri-plugin-autostart` — 开机自启
   - `tauri-plugin-global-shortcut` — 全局快捷键 (Ctrl+Shift+C 取消自动退出)
   - `tauri-plugin-single-instance` — 单实例锁
2. **Setup 钩子**:
   - 创建数据目录
   - 加载配置 (含密码DPAPI解密)
   - 窗口显隐由前端决定（`useInitialDataLoad` 按 `isAutoStart && hiddenStart` 判断是否调 showWindow），`--autostart` 参数仅影响后端自动登录初始延迟；后端另有 3 秒保底显示线程兜底
   - 创建系统托盘
   - 启动适配器监控和启动任务 (通过 `run_startup_tasks`)
   - **3 秒保底 showWindow**：独立线程 3 秒后检查窗口可见性，不可见则强制 `window.show()` + `set_focus()`，最多重试3次，防止前端初始化异常导致窗口永远隐藏
   - **前端心跳监控**：独立线程每 5 秒检查 `last_render_heartbeat_ms`，连续 3 次超过 20 秒无心跳则重载 WebView；判定需同时满足 `is_visible() && !is_minimized()`（前端在 `document.hidden` 含最小化时暂停心跳，只查可见性会对最小化窗口误触发重载）
  - **白屏/黑屏恢复（`app/webview_recovery.rs`）**：① 直订 WebView2 `ProcessFailed`（Tauri 2 Windows 侧不暴露该事件，经 `webview2-com` 0.38 `ICoreWebView2_4::add_ProcessFailed`；sys 0.38 方法名**无 Get 前缀**，token 参数为 `*mut i64`），并 cast `ICoreWebView2ProcessFailedEventArgs2` 读 **Reason / ExitCode**（v1 只有 kind，拿不到"为什么退出"：Reason 直接区分 进程崩溃/内存不足/被外部终止/未预期）——GPU/Utility/PPAPI 等仅记日志交运行时自愈（GPU 未自愈由 rAF 冻结→心跳兜底）。② **按 WebView 存活状态分流恢复动作**：`RENDER/FRAME_RENDER_PROCESS_EXITED`（WebView 仍有效、页面白屏）→ `attempt_webview_recovery`（5 分钟窗口最多 3 次 reload，`recovery_gate` 纯函数，3 单测，心跳超时路径共用同一限流器）；`BROWSER_PROCESS_EXITED`（**WebView 已进入 Closed**）→ `attempt_app_restart` **自动重启应用**（跨进程落盘限流 `webview_restart_guard`，10 分钟窗口最多 2 次，超限只留 ERROR 日志防闪屏循环）。**根因**：浏览器进程退出后 WebView 进入 Closed，reload 返回成功但页面保持全黑（微软文档明确必须重建），故该类故障只走重启路径。③ 诊断埋点 `record_webview2_runtime_version`：启动读注册表 EdgeUpdate `pv`（Evergreen 自动更新，白屏可能仅特定版本存在）；崩溃 dump 在用户数据目录 `EBWebView/Crashpad/reports/*.dmp`，GPU 类白屏可 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` A/B 对比。限流状态在 `UpdateStats`（`webview_recovery_window_start_ms`/`webview_recovery_count` 两原子字段）。
3. **WebView2 内存管理**: `on_window_event` Focused 时通过 `ICoreWebView2_19.SetMemoryUsageTargetLevel` 调节（前台 NORMAL，后台 LOW）
4. **WebView2 浏览器参数**: `platform/gpu.rs::build_browser_args()` 仅注入 `--js-flags=--max-old-space-size=512`（渲染交还平台默认，勿加实验参数，见踩坑/决策）
5. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭（分流逻辑在 `app/shutdown.rs::handle_window_close_event`）
6. **退出流程**: 设 `is_quitting` → `task_manager.shutdown()` 取消并等待后台任务（整体 10s 超时上限，防任务卡在不响应取消的阻塞调用时退出挂起）→ `exit(0)`（`app/shutdown.rs::graceful_exit` → `infra/lifecycle.rs::shutdown_and_exit`），窗口关闭与托盘退出行为统一
7. **命令注册**: 56 个 `#[tauri::command]` 函数（在 `run()` 中通过 `tauri::generate_handler!` 注册）

### 4.2 全局状态 — `infra/state/` 子目录

`state/` 子目录按职责拆分为 4 个文件：

| 文件 | 职责 |
|------|------|
| `mod.rs` | 模块入口与公共类型：`TaskLock`/`TaskGuard`/`TaskFlags`/`AppState`/`CommandResult`/`AccountResult`，以及常量 `AUTO_EXIT_DELAY_MS`/`CANCEL_EXIT_SHORTCUT` 和函数 `validate_account_name` |
| `store.rs` | `ConfigStore`：封装 `ArcSwap<Config>`，提供 CAS 原子更新 |
| `network.rs` | `NetworkState` + `NetworkSnapshot`：基于 `ArcSwap<NetworkSnapshot>` 的 CAS 快照更新 |
| `exit.rs` | `ExitStateStore`：封装应用退出相关状态与截止时间 |

#### TaskLock / TaskGuard 并发原语

```rust
pub struct TaskLock { flag: AtomicBool }
pub struct TaskGuard<'a> { lock: &'a TaskLock }

impl TaskLock {
    pub fn new() -> Self { ... }
    pub fn try_acquire(&self) -> Option<TaskGuard<'_>> { ... }  // CAS 抢占锁
    pub fn is_active(&self) -> bool { ... }
    #[cfg(test)]
    pub fn force_release(&self) { ... }  // 仅供测试使用
}

impl Drop for TaskGuard<'_> { /* RAII 自动释放锁 */ }
```

**说明**：`TaskGuard` 通过 RAII 在 `Drop` 时自动释放锁；`force_release` 标注 `#[cfg(test)]` 仅测试可用。

#### TaskFlags 任务标志

```rust
pub struct TaskFlags {
    pub is_checking: TaskLock,
    pub is_logging_in: TaskLock,
    pub is_logging_out: TaskLock,
    pub is_quality_checking: TaskLock,
}
```

**说明**：4 个 `TaskLock` 字段对应四类互斥任务；取消令牌由 `task_manager: BackgroundTaskManager` 统一管理。

#### ConfigStore 配置存储（store.rs）

```rust
pub struct ConfigStore { inner: ArcSwap<Config> }

impl ConfigStore {
    pub fn new(config: Config) -> Self { ... }
    pub fn load(&self) -> Arc<Config> { ... }           // 加载不可变快照
    pub fn load_full(&self) -> Arc<Config> { ... }      // 兼容旧名别名
    pub fn store(&self, config: Config) -> Arc<Config> { ... }  // 直接替换
    pub fn update<F>(&self, f: F) -> Arc<Config>        // CAS 循环原子更新，避免 TOCTOU 竞态
    where F: Fn(&mut Config) { ... }
}
```

**说明**：CAS 更新逻辑内聚在 `ConfigStore`，配置更新统一走 `state.config.update(...)`。

#### NetworkState + NetworkSnapshot 网络状态（network.rs）

```rust
#[derive(Clone)]
pub struct NetworkSnapshot {
    pub server_available: bool,
    pub any_adapter_online: bool,
    pub last_a1_online: bool,
    pub last_a2_online: bool,
    pub has_logged_online: bool,
    pub disconnect_reconnect_count: u32,
    pub background_check_count: u32,
    pub last_auto_login_attempt: Instant,
    pub last_network_quality: Option<String>,
    pub current_ssid: Option<String>,
    pub on_campus_network: bool,
    pub logout_protected_until: Instant,   // 注销保护期截止时间，60秒内阻止在线状态更新
    pub portal_failure_count: u32,         // Portal 请求连续失败计数，>=5 触发 DHCP 续租重置 MAC
    pub a1_auth_failure_count: u32,        // 适配器1 Portal认证连续失败计数
    pub a2_auth_failure_count: u32,        // 适配器2 Portal认证连续失败计数
    pub prep_login_failures: u32,          // 准备自动登录连续失败次数，达上限后本会话停止自动登录
}

pub struct NetworkState { snapshot: ArcSwap<NetworkSnapshot> }

impl NetworkState {
    pub fn new() -> Self { ... }
    pub fn load(&self) -> Arc<NetworkSnapshot> { ... }   // 加载一致性快照
    pub fn update<F>(&self, f: F) where F: FnMut(&mut NetworkSnapshot) { ... }  // CAS 循环更新
    pub fn update_with_result<F, T>(&self, f: F) -> T where F: FnOnce(&mut NetworkSnapshot) -> T { ... }  // 原子更新并返回闭包计算结果（计数器自增经此内联完成）
}
```

**说明**：网络状态整合为单一 `NetworkSnapshot`，经 `ArcSwap<NetworkSnapshot>` 整体原子读写——读端一次 `load()` 获一致性视图，写端 CAS 循环更新避免竞态。

#### ExitStateStore 退出状态（exit.rs）

```rust
pub struct ExitStateStore {
    pub is_quitting: Arc<AtomicBool>,
    pub auto_exit_deadline: Mutex<Option<Instant>>,
    pub auto_exit_cancelled: AtomicBool,
    pub campus_exit_started: AtomicBool,
    pub campus_exit_deadline: Mutex<Option<Instant>>,   // 校园网退出倒计时截止时间
}

impl ExitStateStore {
    pub fn new() -> Self { ... }
    pub fn deadline(&self) -> Option<Instant> { ... }                    // auto_exit_deadline getter
    pub fn set_deadline(&self, deadline: Option<Instant>) { ... }        // auto_exit_deadline setter
    pub fn campus_exit_deadline(&self) -> Option<Instant> { ... }        // campus_exit_deadline getter
    pub fn set_campus_exit_deadline(&self, deadline: Option<Instant>) { ... } // campus_exit_deadline setter
}
```

**说明**：`is_quitting` 使用 `Arc<AtomicBool>` 跨线程共享克隆。

#### AppState 顶层状态

```rust
pub struct AppState {
    pub config: ConfigStore,                       // 配置存储（封装 ArcSwap<Config>）
    pub tasks: TaskFlags,                          // 4 个互斥任务锁
    pub task_manager: BackgroundTaskManager,       // 后台任务统一管理（含取消能力）
    pub network: NetworkState,                     // 网络状态快照存储
    pub exit: ExitStateStore,                      // 退出状态存储
    pub update_stats: UpdateStats,                 // 更新/通知统计（5个原子字段合并子结构体）
}

pub struct UpdateStats {
    pub last_update_check_epoch_ms: AtomicU64,
    pub update_notified: AtomicBool,
    pub last_disabled_notification_ms: AtomicU64,
    pub last_render_heartbeat_ms: AtomicU64,
    pub last_network_change_notification_ms: AtomicU64,
    pub webview_recovery_window_start_ms: AtomicU64,   // WebView 恢复滑动窗口起点 (app/webview_recovery.rs)
    pub webview_recovery_count: AtomicU32,             // 当前窗口内 reload 次数
}
```

**说明**：`update_stats` 为更新/通知统计子结构体，`AppState` 共 6 字段；`task_manager` 承接取消令牌职责；配置 CAS 更新走 `state.config.update(...)`。

#### CommandResult / AccountResult 返回类型

```rust
#[derive(Serialize)]
pub struct CommandResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Config>,
}
```

**说明**：`message`/`data` 无值时序列化省略字段；`AccountResult` 标注 `rename_all = "camelCase"` 匹配前端命名约定。

**关键常量**:

| 常量 | 值 | 说明 |
|------|----|------|
| `AUTO_EXIT_DELAY_MS` | 20000 | 自动退出倒计时 (毫秒) |
| `CANCEL_EXIT_SHORTCUT` | `"CommandOrControl+Shift+C"` | 取消快捷键 |

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `validate_config()` | 校验配置字段 (枚举值/正则/URL)，含 Portal URL 迁移、校园网关校验、空值回填 (位于 config/validate.rs) |
| `validate_config_lenient()` | 宽松验证，逐字段降级，无效字段回退默认值并记录警告。用于加载磁盘配置，避免单个字段无效导致全量配置丢失 (位于 config/validate.rs) |
| `validate_account_name()` | 校验账号名 (1-32字符, 字母数字下划线中文连字符) (位于 state/mod.rs) |

### 4.3 配置管理 — `config/`

**`Config` 结构体** (40个字段):

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `user` | String | `""` | 学号 |
| `password` | String | `""` | 密码 (内存中明文, 磁盘上DPAPI加密) |
| `selfPassword` | String | `""` | 自助服务系统密码 (内存中明文, 磁盘上DPAPI加密; 回传前端时替换为 MASK; 命令层 resolve_self_password 在前端传空/MASK 时回退此值; 显式清除走 save_config 的 `clearSelfPassword` 标志——与 clear_password 对称, 绑定卡/自助面板密码框旁"清除密码"按钮) |
  ⚠️ 出站掩码纪律（§三-3）：所有把 Config 发往前端的路径**必须经 `Config::masked_for_display()`**（config/model.rs 唯一出口，password + self_password 双字段掩码，空值=未设置语义保留），**不得手工逐字段打码**——漏一处即明文出站（隐私泄露），回归单测（config_cmd.rs 双字段断言）锁死。后续加固方向：敏感字段改 `secrecy::SecretString` 编译期防出站
| `selfHelloEnabled` | bool | true | Windows Hello 验证总开关：关闭后绑定/自助服务面板不再弹验证（只读查询本就不设门），但"查看运营商密码明文"仍强制验证（`ignoreToggle` 门 + 后端 reveal TTL 校验不受开关影响） |
| `selfReverifyEachAction` | bool | false | 每次操作二次验证开关：开启即自助面板会话门退化为每次操作验证的严格模式；`selfHelloEnabled` 关闭时禁用此开关；两开关的**关闭方向**都要求先过 Hello 验证（防绕过界面关闭保护） |
| `operator` | String | `""` | 运营商后缀 (`""` 不拼接, `"@telecom"`/`"@unicom"`/`"@cmcc"` 直接拼接, 其他值 `validate_operator` 报错) |
| `adapter1` | String | `"自动检测"` | 主适配器名称 |
| `adapter2` | String | `""` | 副适配器名称 |
| `dualAdapter` | bool | false | 双适配器模式 |
| `autoLoginOnStart` | bool | true | 启动时自动登录 |
| `autoExitAfterLogin` | bool | true | 登录后自动退出 |
| `minimizeToTray` | bool | false | 关闭时最小化到托盘 |
| `hiddenStart` | bool | false | 静默启动 |
| `autoLaunch` | bool | true | 开机自启 |
| `enableBackgroundCheck` | bool | true | 启用后台检测 |
| `backgroundCheckInterval` | u64 | 15000 | 后台检测间隔 (ms) |
| `autoLoginOnPreparation` | bool | true | 登录准备模式 |
| `autoExitOnOnline` | bool | true | 检测到在线后自动退出 |
| `themeMode` | String | `"dark"` | 主题模式 |
| `enableNotification` | bool | true | 启用通知 |
| `activeAccount` | String | `""` | 当前活跃账号名 |
| `enableLatencyTest` | bool | true | 启用延迟测试（默认开启） |
| `latencyTestInterval` | u64 | 60000 | 延迟测试间隔 (ms) |
| `customThemeColor` | String | `"#6366f1"` | 自定义主题颜色 |
| `defaultPanel` | String | `""` | 默认面板 |
| `enableNetworkQuality` | bool | true | 启用网络质量检测 |
| `skipTtfbInLatency` | bool | true | 延迟测试跳过TTFB |
| `skipContentInLatency` | bool | true | 延迟测试跳过内容下载 |
| `portalUrl` | String | `"http://10.1.99.100"` | Portal地址 |
| `fixedGateway` | String | `"10.2.127.254"` | 固定网关IP (网络质量检测用) |
| `requiredNetworkName` | String | `"i-wxxy"` | 校园网名称 (空字符串回填默认值) |
| `enableNetworkNameCheck` | bool | true | 启用校园网名称检测 |
| `campusGateway` | String | `"10.2.127.254"` | 校园网关地址 (空字符串回填默认值) |
| `campusExitOnFail` | bool | true | 校园网验证失败时是否触发退出 |
| `campusCheckStartMinutes` | u16 | 480 | 校园网检测静默期截止时间（分钟数，480=8:00），支持旧字段名 `campusCheckStartHour` 反序列化 |
| `logRetentionDays` | u32 | 7 | 日志保留天数 |
| `maxDisconnectReconnect` | u32 | 3 | 断线重连最大次数 |
| `autoLoginCooldownSecs` | u64 | 60 | 自动登录冷却秒数 |
| `skipSha256WhenMissing` | bool | false | 更新包 SHA256 校验源全部 4xx 时是否跳过校验（无前端开关，默认拒绝安装） |
| `configVersion` | u32 | 2 | 配置版本号 |

**关键函数**:

| 函数 | 说明 |
|------|------|
| `atomic_write()` | 原子写入文件，3次重试+100ms间隔，重命名失败后删除临时文件；rename 前对临时文件 `sync_all` 落盘，断电不产生半截 config |
| `append_login_history()` | 登录历史追加（读-改-写全程持模块级 `LOGIN_HISTORY_LOCK` 互斥锁，防自动/手动登录并发覆盖丢历史） |
| `list_account_names()` | 共享函数，统一账号目录遍历逻辑 |
| `validate_username()` | 校验用户名 (位于 validate.rs) |
| `validate_operator()` | 校验运营商后缀 (返回 Result，非法值返回错误而非静默清空，位于 validate.rs) |
| `validate_password()` | 校验密码 (位于 validate.rs) |
| `deserialize_non_empty_or()` | 自定义反序列化器，空字符串自动回填默认值 (位于 model.rs) |

#### 4.3.1 安卓端配置 — `android/src-tauri/src/config_state.rs`

安卓端全量配置 `Settings`（结构对齐桌面 `Config` 可适用子集，camelCase IPC 契约；密码字段经 AndroidKeyStore AES-GCM 落盘）。**默认值双源覆盖关系**：前端 `DEFAULT_CONFIG` 与后端 `Settings::default` 都定义默认值，真机 `get_init_data` 返回后端值覆盖前端——**改默认值必须改后端**，只改前端无效。

- **开箱即用默认值**：`auto_login_on_start`/`enable_background_check`/`auto_login_on_preparation`/`enable_network_name_check`/`skip_ttfb_in_latency`/`skip_content_in_latency` 六开关默认 true（自动化登录、验证设置、质量跳过项）；`enable_boot_autostart`（开机自启）不属于登录自动化，保持 false 由用户主动开启。
- **schema 迁移链**（`migrate_legacy_defaults`，一次性、落盘后不重复触发、幂等）：v0→v1 后台检测间隔 15s→60s（仅命中历史默认值时）；v1→v2 上述六开关存量配置里的显式 false 一并刷为 true（开发阶段统一开箱即用；落盘后用户主动关闭不会再被覆盖）。新装 `config_schema_version=2` 跳过迁移。**注意迁移只在版本变更时落盘**，v2+ 配置读盘不写盘。
- **测试无法在本机 host 运行**：安卓 crate 按移动插件门控依赖（desktop cfg 缺 reqwest 等），host `cargo test` 编译不过；build.rs 解析 capabilities 的移动插件权限（biometric/opener/notification）同样失败——为存量环境限制。验证路径：`cargo check --target aarch64-linux-android --all-targets`（需 NDK 工具链注入 `CC_aarch64_linux_android` 等环境变量，host desktop target 不可用），测试断言锁定默认值与迁移语义。

### 4.4 加密工具 — `account/crypto.rs`

Windows DPAPI 加密/解密，绑定当前 Windows 用户。`encrypt()` 无显式空输入检查，直接将 `&[u8]` 传给 `CryptProtectData`，空数据加密行为依赖 DPAPI API（返回 0 或 output 为空时返回 `Err`）。`decrypt()` 解密失败返回 `Err`。

### 4.5 网络模块 — `network/`

#### 4.5.1 缓存基础设施 — `network/client.rs`

**全局变量**:

```rust
type ClientPoolKey = (Option<IpAddr>, u8, u64);   // (绑定源IP, TLS最低版本标识, 超时ms)

lazy_static! {
    pub(crate) static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new(default_portal_url()));
    static ref CLIENT_POOL: DashMap<ClientPoolKey, (reqwest::Client, Instant)> = DashMap::new();
}
```

> 注：适配器缓存在 `network/adapter_cache.rs`，网关/子网缓存在 `network/subnet.rs`，Portal 状态缓存由 `auth/portal.rs` 局部管理——`client.rs` 仅保留 Portal URL 与 HTTP 客户端池两个全局变量。

**HTTP 客户端池** (`CLIENT_POOL: DashMap`, LRU 淘汰):

- Key = `(Option<IpAddr>, u8, u64)` 元组（绑定源 IP + TLS 版本标识 + 超时毫秒），value 附带 `Instant` 访问时间，池上限 `CLIENT_POOL_MAX_ENTRIES=32`，TTL `CLIENT_POOL_TTL_SECS=600`
- **LRU 淘汰策略**：`client_pool_get` 命中时更新 `Instant::now()`（按访问时间淘汰）；容量超限时 `min_by_key(Instant)` 剔除最久未访问条目
- `create_safe_http_client(timeout, local_addr)` — TLS 1.3 优先 + TLS 1.2 降级，`no-cache/no-store` 头

**关键函数**:

| 函数 | 说明 |
|------|------|
| `create_safe_http_client(timeout, local_addr)` | 创建 HTTP 客户端 (TLS 1.3 强制 + TLS 1.2 回退) |
| `update_portal_url(url)` | 更新全局 Portal URL |
| `build_client(timeout, local_addr, min_tls)` | 构建 reqwest::Client (no_proxy + limited(5) 重定向 + 3s connect_timeout) |
| `client_pool_key(local_addr, min_tls, timeout)` | 生成客户端池 Key |

#### 4.5.2 适配器查询 — `adapter.rs` (选择逻辑) + `network/mod.rs` (re-export 收敛点)

`adapter.rs` 仅保留适配器选择逻辑，对外 re-export 收敛到 `network/mod.rs`，调用方直连源模块或经 `crate::network::` 顶层路径访问。

**adapter.rs 本地保留函数** (适配器选择):

| 函数 | 说明 |
|------|------|
| `find_by_name()` | 按名称查找适配器 |
| `find_with_valid_ip()` | 按名称查找具有有效 IP 的适配器 |
| `find_dual_adapters()` | 查找双适配器 (a1, a2)，a2 仅在 dual_adapter 且名称非空且与 a1 不同时查找 |
| `is_secondary_adapter_enabled()` | 副适配器是否启用 (dual_adapter && adapter2 非空) |
| `resolve_adapter_names()` | 解析主/副适配器名称 (支持自动检测：优先有线→任意有IP→首个) (定义于 adapter.rs:48 附近) |
| `select_adapter()` | 选择适配器并返回 (ip, name) |
| `filter_operation_adapters()` | 操作类流程的适配器范围过滤基准（见下"适配器操作范围约定"） |
| `ensure_ethernet_ip_for_login()` | 登录前确保以太网 IP (DHCP 续租兜底) |

**`network/mod.rs` re-export 来源**:
- `network::discovery` — `Adapter`/`AdapterDetail`/`DisabledAdapter` 类型 + `is_blacklisted` + Win32 API `GetAdaptersAddresses` 查询 + 适配器状态四分类
- `network::adapter_cache` — `get_adapters_force`/`get_adapters_cached`/`get_adapters_cached_async`/`get_disabled_adapters_cached`/`get_adapter_details_cached`/`get_all_adapters_cached`/`enable_adapter`/`wait_for_adapter`/`filter_operation_adapters` 等 + TTL 5秒缓存
- `network::dhcp` — `dhcp_renew_wired_only`/`dhcp_release_renew_all`/`dhcp_release_renew_single`
- `network::subnet` — `get_wireless_ssid`/`get_wired_network_profile`/`check_gateway_reachable`/`check_gateway_reachable_from`/`is_same_subnet_18`

**适配器状态四分类** (`AdapterStatus` 枚举，定义在 `network/discovery/`):
  - `Disabled` — 已禁用（OperStatus Down/NotPresent，管理员禁用或硬件缺失）
  - `Disconnected` — 未连接（OperStatus LowerLayerDown/Dormant，线缆未插或USB网卡未连接）
  - `EnabledNoIp` — 未禁用无IP（OperStatus Up 但无有效 IP，含 169.254 APIPA 清空后）
  - `Connected` — 已连接（OperStatus Up 且有有效 IP）

**连接速度 (LinkSpeed)**: `Adapter`/`AdapterDetail` 新增 `linkSpeed` 字段（u64 bit/s，0 表示未知），直接读 `IP_ADAPTER_ADDRESSES.ReceiveLinkSpeed`（无需额外 API）。**未连接时 Windows 返回 u64::MAX（内部 -1 哨兵），发现层归 0 表示未知**——原样透传会被前端换算成 18446744073.7 Gbps。前端 NetworkPanel 适配器卡片展示格式化后速度，统一 Mbps 单位（低于 1 Mbps 用 Kbps）。

**适配器操作范围约定**: 检测/优化/登录/注销/DNS 设置/DHCP 等**操作类**流程只作用于 `resolve_adapter_names` 解析出的主/副适配器（"自动检测"由 resolve 落到具体适配器），`filter_operation_adapters(adapters, a1, a2)` 为范围过滤基准；**UI 展示类**（get_adapters/get_adapter_details/adapter_watch/check_dns_doh_status/check_campus_status）保持遍历全部。落地：后台巡检与开机自启的校园网检测传过滤后列表；`select_adapter` 经 resolve 取主适配器 IP（无 IP 返回空由调用方兜底）；`setup_dns_doh_admin(targets)`/`dhcp_renew_wired_only(targets)`/`dhcp_release_renew_all(gw, targets)`/自动 MAC 重置均按名单收窄（helper 提权路径经 `--helper dns <名单...>` 传参，`spawn_elevated_helper` 对参数统一加引号防适配器名含空格被拆碎）；登录/注销（full_login/full_logout）本就只操作 a1/a2。

**前端选择器同源收敛**: 登录/注销的适配器选择浮层（DockNav `AdapterMenu`）只列主/副适配器（有 IP 者），与操作范围约定对齐——点击按钮仍走后端 resolve 全量逻辑，浮层只影响菜单选项。`frontend/src/network/adapters.ts` 的 `resolveAdapterNames(adapters, config)` 与后端 `resolve_adapter_names`（adapter.rs）**同源规则**：配置名有效→用配置名；空/"自动检测"/不在当前列表→自动检测（有线有 IP > 任意有 IP > 第一个；副适配器自动检测排除主适配器）。**两端规则若分叉，选择器与实际登录目标会对不上**，改任一侧必须同步另一侧（adapters.test.ts 锁行为）。DashboardPanel"获取新IP"菜单同步用 resolve；同卡菜单 `createPortal(document.body)` + fixed 定位——卡片容器 `contain: paint` 会裁掉 absolute 菜单（只露十几像素）。

**适配器可见性双重验证** (定义在 `network/discovery/registry.rs`):
  - `is_visible_in_ncpa()` — 注册表双重检查：`ShowInNetworkConnections` + Class subkey PnP 设备树交叉验证，过滤幽灵虚拟副本
  - `is_admin_disabled_via_registry()` — `ConfigFlags 0x1` 检测管理员禁用，区分"管理员禁用"vs"硬件缺失(USB未连接)"
  - **`ensure_cache_initialized` 锁优化**：`CLASS_SUBKEY_CACHE` 懒加载初始化采用双重检查锁定，`build_class_subkey_cache()`（注册表遍历，慢 I/O）在锁外执行，仅用写锁做 swap，避免阻塞读锁请求（`class_subkey_has_matching_guid`/`is_admin_disabled_via_registry`）。`refresh_class_subkey_cache` 同样采用锁外构建模式。

**校园网检测** (定义在 `network/subnet.rs`):
  - `get_wireless_ssid()` / `get_wired_network_profile()` — 获取当前连接的网络名称 (Wi-Fi SSID + 以太网配置文件)
  - `check_gateway_reachable()` / `check_gateway_reachable_from()` — Ping 检测网关可达性 (后者支持指定源 IP)
  - `is_same_subnet_18()` — /18 子网匹配检测

#### 4.5.3 Portal 检测 — `auth/portal.rs`

- Portal 认证状态检测
- URL `:801` 端口追加逻辑统一处理
- NAT 内网 IP 检测，NAT 环境下不发送 `wlan_user_ip`
- `PortalStatus` 新增 `error_kind` 字段区分"请求失败"与"Portal不可达"
- **页面探测禁止走 :801**：801 是 ePortal SPA 管理前端（`/eportal` 登录表单，已在线仍渲染登录页、HTML 无状态特征）——曾统一对齐 :801 导致误报"无法判断登录状态"；`check_portal_page` 请求配置的原始地址（默认 80 网关页，GBK 但 `Dr.COMWebLoginID`/`uid='`/`v4ip='` ASCII 特征可匹配）；登录/注销等协议请求仍强制 :801（`ensure_portal_port`）
- **`block_on_http` helper**：同步-异步桥接函数，用于在同步上下文（如 `std::thread::scope` 子线程）中执行 async reqwest 请求。优先使用 `Handle::try_current()` → `handle.block_on(future)`（设置 reactor guard），失败 fallback 到 `tauri::async_runtime::block_on(future)`。背景：b4d8e82 将 `reqwest::blocking` 迁移到异步 reqwest，但 `std::thread::scope` 子线程无 Tokio reactor 上下文导致 panic "there is no reactor running"。**约束**：不能在 async worker 线程上直接调用（`Handle::block_on` 会 panic），所有调用者必须通过 `spawn_blocking` 或在同步线程中调用。

#### 4.5.4 登录/注销请求 — `auth/protocol.rs`

**v 参数随机化**:

```rust
pub fn random_v() -> String {
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
    let v = 1000 + (seed % 9000);
    format!("{}", v)
}
```

每次请求独立生成 1000-9999 随机4位数 v 值，统一应用于登录与注销请求（`random_v` 定义于 `protocol.rs:6`，Portal 页面检测不发 v 参数）。

**响应解码与 JSONP 解析**:

- **编码**: 响应体经 `platform/console_output.rs::decode_charset_bytes` 解码——Content-Type 显式 GBK 族按 936，否则 UTF-8 优先 → OEM 回退（`from_utf8_lossy` 会把 GBK 字节全变 U+FFFD，中文关键词匹配全失效、真实失败被误报登录成功）
- **JSONP**: `jsonp_json_slice()` 从第一个 `(` 后的第一个 `{` 起做字符串/转义感知的花括号平衡扫描到配对 `}`（`rfind(')')` 旧法在 msg 含半角括号如"密码错误(剩余2次)"时截断 JSON → 解析失败重试全败）

**登录函数**:

| 函数 | 说明 |
|------|------|
| `do_login_with_retry()` | 登录请求+重试(重试次数由调用方传入)，重试等待可中断(每100ms检查退出标志) |

**注销函数** (Radius 注销先行 + MAC 解绑收尾):

| 函数 | 说明 |
|------|------|
| `do_logout_request()` | Radius 注销（主操作，最多 2 次、成功即止）+ MAC 解绑（单次收尾，best-effort） |
| `do_logout_with_retry()` | 注销重试(重试次数由调用方传入)，重试等待可中断 |
| `parse_logout_result()` | 注销结果解析 (JSONP)，支持多种成功条件 |
| `ip_to_eportal_int()` | 点分 IPv4 → ePortal 整数 IP（大端序，对齐前端 ip_to_int；NAT 空串回退原样） |

**注销请求顺序** (2026-09-11 重构，根治"注销后打不开登录页、换 MAC 才恢复"):

```
① Radius 注销（最多 2 次、成功即 break，两次间可中断等待 1.5s）
  GET /eportal/portal/logout?callback=dr100{round+2}&login_method=1
      &user_account=drcom&user_password=123&ac_logout=1
      &register_mode=1&wlan_user_ip={IP}&wlan_user_mac=000000000000
      &jsVersion=4.1.3&v={random}&lang=zh
  成功: result=1, msg="Radius注销成功！"
② MAC 解绑（单次，best-effort；退出中跳过）
  GET /eportal/portal/mac/unbind?callback=dr1002
      &user_account={学号}&wlan_user_mac=000000000000
      &wlan_user_ip={整数IP}&jsVersion=4.1.3&v={random}&lang=zh
  成功: result=0, msg="解绑终端MAC成功！"
```

> **为什么顺序倒置（2026-09-11 调研 + 虚拟机实测沉淀）**：旧实现每轮"先 unbind 后 logout"×2 轮、外层再重试 2 次（最坏 4+4 个请求），连发请求且顺序与同校开源项目（Rikka-Sei/wxxy-autoLogin-Script，同一认证服务器）相反；主流实现（cqu-net-auth/eptools/Meirs）均为"先查询、一次到位、绝不连发"，且 unbind 的 `wlan_user_ip` 传**整数形式**（旧实现传点分十进制）。重构后注销请求上限从 8 个降到 3 个。
> 配套修复（session.rs `login_adapter_with_log`）："已经在线"假成功复核——登录返回"已经在线"时再跑一次 `check_portal_full`，探测不通判为服务端会话残留，降级为认证失败（code=1）走 `update_auth_failure_count` 计数，连续 5 次自动触发该适配器 MAC 重置自愈（换 MAC 恢复的程序化等价物）。

**协议行为实测结论（2026-09-11，VMware 桥接 VM 四轮实验，累计 23 次注销 0 僵尸，95% 置信上界 ~15%）**：

- `chkstatus` 接口在本部署不存在（404）——"注销前查在线状态"不可行，登录态判定用 80 状态页（在线时内嵌 `uid='<服务端uid>'`/`v4ip='<本机IP>'`/`time='<在线秒数>'` 变量）；该页单次探针有 ~9% 瞬时抖动（同时段公网 204 均通），单次不可靠——后台巡检"失败计数阈值 5 才触发动作"的设计必要
- `mac/unbind` 不踢在线会话（unbind 后立即重登报"IP 已经在线"，Radius 会话仍活）——注销成败与 unbind 无关，unbind 仅作用于 MAC 绑定表；整数 IP 与点分 IP 行为无差异
- **"注销后打不开登录页/换 MAC 恢复"的僵尸状态未复现**（单次注销两种顺序、unbind+logout 4 连发、错 IP/空 IP 变异后 0 延迟重登均成功）——定性为**间歇性服务端/环境异常而非协议固有行为**，应用侧防御（成功即止 + "已经在线"复核 + MAC 重置自愈）保持
- 对已销毁会话的重复 logout/unbind 无增益也无污染——连发纯属浪费时间，"成功即止"正确
- **网关劫持实证**：未认证时公网探针 `connect.rom.miui.com/generate_204` 返回 302（劫持到 Portal），认证后 204——`net204` 状态码可作权威在线判据（区分"有 IP 未认证"与"真在线"）
- 空 `wlan_user_ip` 的 logout 成功（服务端按请求源 IP 定位会话，NAT 分支安全）；错 IP 的 logout/unbind 无污染
- "已经在线"（result=0 + ret_code=2）只在会话真实存活时出现——session.rs 复核与实测行为兼容，不误伤
- 801 端口 JSONP 响应为 UTF-8（80 网关页才是 GBK），与 `decode_charset_bytes` "UTF-8 优先 → OEM 回退"策略兼容
- 换 MAC = 换身份（DHCP 立即获得全新 IP，新身份首次登录即成功，"换 MAC 恢复登录"闭环）；未覆盖：僵尸态下的换 MAC 恢复（僵尸未复现）、WLAN 段身份

**注销成功判定**:
- 两步均成功 → 注销成功
- Radius 注销成功 + MAC 解绑失败 → "Radius注销成功，MAC解绑失败"
- `/logout` 接口 `result=1` 表示 Radius 注销成功（但 msg 含"非法"/"失败"/"错误"/"拒绝"时仍判失败）
- `/mac/unbind` 接口 `result=0` 且 msg 含"解绑终端MAC成功"表示解绑成功
- `result=0` 但 msg 含错误关键词（"非法"/"失败"/"错误"/"拒绝"）→ 失败
- `result=0` + "获取用户在线信息数据为空" → 判成功（"当前无在线设备"）
- HTML/纯文本回退响应含"注销成功"/"下线成功"/"已下线"/"logout" → 判成功

#### 4.5.4.1 校园网认证协议速查 (2026-09-03 沉淀)

> 来源：社区项目 [Senquan007/Wxxy_network_auto_login](https://github.com/Senquan007/Wxxy_network_auto_login) README + 2026-09-03 本机浏览器/curl 实测。认证系统为 Dr.COM 城市热点，GET 方式认证。

**端口语义（严格区分，勿再对齐）**:

| 端口 | 角色 | 用途 |
|------|------|------|
| `10.1.99.100` (80) | Dr.COM 网关状态页 | **仅页面状态检测**（`check_portal_page`）：GBK 页面内嵌 `注销页`/`Dr.COMWebLoginID_*`/`uid=`/`v4ip=` 状态特征；已在线渲染"您已经成功登录。"+注销按钮 |
| `10.1.99.100:801` | ePortal 认证 API + 管理前端 | **登录/注销/协议请求**（`ensure_portal_port`）；根路径 302→`/eportal` SPA 管理系统登录表单，已在线仍渲染登录页、无状态特征，**禁止用于状态探测** |

**登录请求模板**（`protocol.rs do_login_request` 与社区项目完全一致）:

```
GET http://10.1.99.100:801/eportal/portal/login?callback=dr1003&login_method=1
    &user_account={学号}{运营商后缀}&user_password={密码}
    &wlan_user_ip=&wlan_user_ipv6=&wlan_user_mac=000000000000
    &wlan_ac_ip=&wlan_ac_name=&jsVersion=4.1.3&terminal_type=1&lang=zh-cn&v={random}&lang=zh
```

- 运营商后缀：无锡学院无后缀 / `@cmcc` / `@unicom` / `@telecom`
- `wlan_user_ip` 非关键认证数据，可留空甚至删除（社区实验结论，与本项目 NAT 检测后不发的设计一致）
- `v` 为随机数即可（本项目 `random_v()` 1000-9999）

**登录响应语义（JSONP `dr1003({...})`）**:

| 响应 | 含义 | 本项目处理 |
|------|------|-----------|
| `result=1` + "Portal协议认证成功" | 登录成功 | success（还防"非法/失败/错误/拒绝"假成功） |
| `result=0` + "IP: x.x.x.x 已经在线！" + `ret_code=2` | **已在线（成功态）** | success —— result=0 是双语义，必须按 msg 区分 |
| `result=0` + "AC认证失败" | 凭据失败 | `ac_auth_failed`（AUTH_FAILURE_CODES 计数） |
| `result=0` + 其他 msg | 真实业务失败（账号过期/余额不足等） | `unknown_failure`，不误报成功 |
| `result=2` + "已经在线" | 已在线变体 | success |
| `result=3` / `result=4` | 流量超限 / 账号被禁用 | 失败 |

**环境事实**：校园网 IP 为 DHCP 动态分配，租期约 1 天（社区项目因此需 cron 定时重登；本项目由后台巡检 + 断线重连 + DHCP 续租覆盖）。

#### 4.5.4.2 自助服务系统（Dr.COM Self）运营商绑定协议 (2026-09-05)

> 来源：2026-09-05 本机 curl + 浏览器（含 IAB）实测。实现于 `self_service/mod.rs`（协议）+ `commands/self_service.rs`（`bind_operator` 命令），供新手教程"绑定运营商账号"步骤使用。系统地址 `http://10.1.80.200:8080/Self`（仅校园网内网可达，常量 `SELF_BASE_URL`）。

**业务背景**：校园网账号（学号）首次使用前必须在自助服务系统绑定运营商账号（办理套餐的手机号 + 运营商账户密码，由运营商在办理套餐时短信下发），否则 Portal 认证无法正常使用。账户密码**无法自行请求**，遗失可咨询运营商客服。

**协议链路（5 步，全部实测验证）**:

| 步骤 | 请求 | 要点 |
|------|------|------|
| 1. 取 checkcode | `GET /Self/login/` | hidden `name="checkcode" value="4位数字"`，会话级，正则提取 |
| 2. **验证码预热** | `GET /Self/login/randomCode?t=` | **必要隐式前置**：浏览器打开登录页时 `<img>` 自动加载此地址，服务端在 session 记录"已发放"；跳过则 verify 必 302 失败并提示"验证码错误！"。本部署验证码输入框隐藏（`randomDiv` class=hide），提交空 `code` 即通过，用户无需输入 |
| 3. 登录 | `POST /Self/login/verify` | `account=学号&password=md5(密码小写hex)&checkcode=…&code=`；成功 302 → `/Self/dashboard`；失败 302 → `/Self/login/`，重新 GET 登录页从内嵌 `})('提示文本');` 提取失败原因（如"账号或密码错误！"）。自助系统密码默认为身份证后 6 位 |
| 4. 取绑定表单 | `GET /Self/service/operatorId` | hidden `csrftoken`（UUID）+ `FLDEXTRA1..6`（预填已绑定值，需用 `extract_fld_values` 全量解析）；302 = 登录会话失效 |
| 5. 提交绑定 | `POST /Self/service/bind-operator` | `csrftoken + FLDEXTRA1..6`，映射：中国移动=1/2、中国电信=3/4、中国联通=5/6（账号/密码**明文**提交，无 MD5、无 JS 拦截、maxlength 20）；**表单整体保存：非目标运营商字段必须带回预填原值，填空串会清掉已有绑定**（2026-09-05 用户实测缺陷）；HTTP 200 重渲染页，内嵌 swal msg 含"绑定运营商账号信息成功"判成功（可含多行 `\n` 分隔的多运营商结果），建议带 Referer 头（实测缺失偶发服务端空响应） |

**实现约定**:

- `bind_operator(account, password, operator, phone, sms_password)` 为 async 命令，直接 await 协议 async fn（无 spawn_blocking）；`operator` 与 `Config.operator` 同源（`@cmcc`/`@telecom`/`@unicom`，经 `operator_fld_pair` 映射 FLDEXTRA 序号）
- 会话客户端独立于 `CLIENT_POOL`：`cookie_store(true)` 保持 JSESSIONID + `redirect(Policy::none())` 手动按 Location 判定 verify 结果 + 绑定校园网适配器源 IP（多网卡场景，与登录同源规则解析，复用 `create_safe_http_client` 的 local_addr 能力但不进池）
- reqwest 需开启 `cookies` feature（Cargo.toml）；MD5 用 `md-5` crate（lib 名 `md5`）
- **安全契约**：凭据仅本次请求内存传递，不写配置、不落盘、不写日志；手机号/运营商账户密码均不持久化
- **绑定状态查询**：`query_bind_status` 命令复用登录链路（`login_and_fetch_bind_page` 提取为共用函数），解析 FLDEXTRA 预填值返回三运营商绑定状态；手机号掩码**前三后二**（`mask_account`，如 `197******38`），密码仅回是否设置，明文不出协议模块
- **Windows Hello 本地身份验证**：`verify_windows_identity` → `platform/identity.rs` 的 `verify_identity(consent_message)`（async），**只走 Windows Hello**（`UserConsentVerifier`），设备未配置时直接返回引导文案（CredUI/SSPI 回退已删除，用户明确只要 Hello）；弹窗文案由前端 i18n 传入，通过后 `reveal_operator_credential` 才可返回明文。**平台问题**：阻塞等待 RequestVerificationAsync（.get()）时 Consent 弹窗（独立 broker 进程）无法完成前台转移（cppwinrt#999）——非阻塞 `await_winrt_operation`（SetCompleted 回调 + tokio oneshot，windows 0.58 无内建 Future；delegate 是 FnMut、oneshot Sender 消费 self 用 Option::take() 适配）；**不手动 CoInitializeEx**：windows-core factory cache 撞 `CO_E_NOTINITIALIZED` 时自动 `CoIncrementMTAUsage` 自愈，手动 init/uninit 反而破坏该机制（microsoft/windows-rs#1169），官方 sample/Bitwarden/OneKeePass 均不手写
- **Consent 弹窗前台两路径方案**：根因是 Windows bug（task.ms/49689617，broker 进程弹窗不绑定任何窗口，应用侧无法根治只能绕）。① **主路径（Win11 Build 22000+）**：`IUserConsentVerifierInterop::RequestVerificationForWindowAsync(hwnd, msg)` 把 Consent 对话框**绑定到主窗口 HWND**（作为其子级 UI 天然置前；Flutter local_auth_windows/Bitwarden 同做法）；HWND 取 `webview_window.hwnd()` 原始 isize；interop 是 COM 包装（!Send），须块作用域内创建 op 后立即释放、不跨 await。② **兜底（Win10/interop 不可用，`.ok()?` 回退）**：`spawn_consent_focus_nudger` 后台线程轮询对话框窗口类名 "Credential Dialog Xaml Host"（250ms×12 次）`SetForegroundWindow`（Chromium/gsudo 同款）；interop 拿到 op 后 await 结果即最终结论，**不再回退重弹**。命令层传 `owner_hwnd: Option<isize>`
- **验证与明文返回在后端关联（安全加固）**：`verify_windows_identity` 成功后 `note_identity_verified()` 写后端时间戳（TTL `IDENTITY_VERIFY_TTL_SECS`=600s，纯函数 `is_within_ttl` 含时钟回拨拒绝，有单测），`reveal_operator_credential` 校验 `identity_verified_recently()`——前端 helloGate 只是 UX 层，**后端 TTL 才是真防线**，webview 层绕过前端编排也拿不到明文。门覆盖面：`bind_operator`/`self_offline_session`（改变外部状态）经 `ensure_identity_gate` 校验（`selfHelloEnabled` 开启时要求 TTL 内验证）；**只读查询命令有意不设门**（总览卡自动刷新依赖免验证拉取，Bitwarden reprompt 分级保护/sudo 仅副作用命令需认证同思路）
- 单测 7 个（纯函数）：checkcode/csrftoken/swal msg 提取（实测 HTML 样例）、绑定成功判定、FLDEXTRA 映射、md5 标准测试向量（不使用真实凭据向量）、mask_account 掩码规则
- 前端：新手教程 5 步向导（欢迎→**绑定运营商账号(可跳过)**→账号→适配器→完成），`tauriApi.bindOperator`，i18n `onboarding.bind*` 键组（zh/en）；字段名"运营商账户密码"（键名 `bindSmsPassword` 保留历史命名），校园网登录密码与自助服务密码默认均为身份证后 6 位（placeholder 提醒）

#### 4.5.4.3 自助服务系统（Dr.COM Self）dashboard 卡片协议：在线信息 + 近期上网记录 (2026-09-05)

> 来源：2026-09-05 浏览器（IAB）登录态下读取 dashboard 页内嵌 bootstrapTable JS + 登录会话内 fetch 实测响应结构。实现于 `self_service/mod.rs`（`query_dashboard`/`offline_session`/`query_online_log`）+ `commands/self_service.rs`（`query_self_dashboard`/`self_offline_session`/`query_self_online_log`）。前端为独立"自助服务"面板（`SelfServicePanel.tsx`，自带凭据输入区）。

**协议链路（登录会话 cookie 即可，均无额外必填参数）**:

| 接口 | 请求 | 响应结构与字段语义 |
|------|------|------|
| 在线信息 | `GET /Self/dashboard/getOnlineList` | 对象数组：`loginTime`（字符串 "YYYY-MM-DD HH:mm:ss"）、`ip`、`mac`（12 位 hex 无分隔）、`useTime`（**秒**）、`downFlow`/`upFlow`（**KB**）、`hostName`（可空）、`terminalType`（`#PC` 带 `#` 前缀）、`sessionId`（注销用） |
| 近期上网记录 | `GET /Self/dashboard/getLoginHistory` | 数组的数组：`[上线时间 epoch ms, 注销时间 epoch ms, ip, mac, 时长(分), 流量(M), 计费方式 1时长/2流量/3包月, 金额, 主机名(null), 终端类型, ...]` |
| 注销会话 | `GET /Self/dashboard/tooffline?sessionid=` | `{"success":bool}`；实测对不存在的 sessionid 也返回 true（服务端宽松），前端只能以"接口成功"提示 |

**实现约定**:

- 登录链路已重构：`login_session`（共用前 3 步：checkcode → randomCode 预热 → verify）为最底层，`login_and_fetch_bind_page`（绑定/状态/明文查看）与 `query_dashboard`（一次登录连拉 getOnlineList + getLoginHistory，`fetch_dashboard_json` 共用"302=会话失效 + 非 JSON=异常页"判定）都从它出发
- 302 重定向 = 登录会话失效（统一文案"登录会话失效，请重试"）；`parse_offline_success` 对非 JSON/缺字段/非布尔一律判失败
- 前端格式化**对齐原站公式**（dashboard 页内嵌 JS）：MAC 每 2 字符加 `-`；useTime `parseInt/60` 分钟取整；流量 `(down+up)/1024` M 三位小数；终端类型截掉 `#` 前缀（空→`-`）；epoch → `YYYY-MM-DD HH:mm:ss`；null 主机名 → `-`。**注意 useTime 单位与上网记录的时长（分）不同，流量 KB 与 M 不同**，透传 JSON 由前端换算，后端不改结构
- **启动时序**：`configLoaded` 布尔（getInitData 成功/降级完成置位）是自助服务面板自动验证+自动刷新的前置条件——启动后立刻切入面板时配置还在加载，等信号再判断凭据/弹 Hello，加载窗口期显示"配置加载中..."；后端 setup_app 用 gpu-warmup 后台线程预热 GPU/刷新率检测（OnceLock 缓存），get_init_data 读缓存即返回不阻塞前端拿配置
- 前端：**独立"自助服务"面板**（`SelfServicePanel.tsx` 含在线信息+近期上网记录两卡片；**凭据与账号面板绑定卡共用 `useSelfCredStore`**——切换面板不丢失、学号仅内存保留，密码 blur 即经 `saveConfigDirect` DPAPI 落盘）。**密码"已保存"显示两面板统一读 store 独立布尔 `selfPasswordSaved` 而非 `config.selfPassword === MASK`**（blur 走 updateConfig 会把明文写进本地 config 且标 dirty，挡住 config-changed 回传的 MASK——输入框闪空甚至永久空白；布尔由 saveConfigDirect 非空成功置位 + 初始加载按 MASK 置位，`useConfigStore.selfPassword.test.ts` 锁行为）；查询/踢下线前过 `useHelloGate` 验证门；注销走 ConfirmDialog；i18n `nav.selfservice`/`panel.selfservice*`/`account.selfDashboard*` 键组（zh/en 对称）
- **逆向安全红线**：dashboard 页有注销功能，实验只允许用**必然不存在的 sessionid** 探测接口格式，绝不能点击/调用页面上真实会话的注销——误踢当前在线设备会导致用户断网
- **上网记录账单卡**：`SelfServicePanel` 第三张卡——日期范围（默认今天）+ 汇总 8 格 + 12 列明细表（横滚）；`query_online_log`（`login_session` + GET `bill/getUserOnlineLog?startTime&endTime&pageNumber&pageSize=500`，一次拉全不翻页，超 500 提示缩小范围）；命令 `query_self_online_log`（YYYY-MM-DD 校验 + resolve_self_password 回退）；前端 epoch ms 格式化（logoutTime 0/空 → `-`）；列名复用 dashboard 卡现有键，新增 `account.selfLog*` 键组（zh/en）
- **Hello 验证门拆分（两套门）**：绑定卡 `useHelloGate`（时间戳 `helloGateVerifiedAt`，**首次操作即验证**，通过后 TTL 内共用；TTL 570s 对齐后端 600s 留 30s 时钟余量——永真缓存会在 10 分钟后撞 reveal TTL 且无重验入口，功能死锁；`gateFresh` 含 `elapsed >= 0`，时钟回拨视为过期）——**查看明文与绑定/查询共用此门**，`useHelloGate({ ignoreToggle: true })`：未验证/已过期时无视总开关强制验证（明文特权操作不因总开关关闭而裸奔）；自助面板 `useSelfServiceVerify` 会话门（时间戳 `selfSessionVerifiedAt` 同款 TTL，面板卸载 `resetSelfSessionGate()` 重置；`selfReverifyEachAction=true` 时退化为每次操作验证）。`config.selfHelloEnabled=false` 时两门整体放行，**reveal 明文不走门不受开关限制**（后端 TTL 防线恒在）。设置页"安全设置"两开关**关闭方向必须先过 Hello 验证**（开启方向免验）。SelfServicePanel mount 后凭据就绪即自动验证+自动刷新（自动刷新直接走 `fetchDashboard` 内含验证门，避免外层+内层连弹两次 Hello）。`SelfServicePanel.test`/`AccountPanel.helloGate.test` 锁行为
- 单测：`offline_success_parsing`（success 判定 4 边界）；前端 vitest 5 用例（凭据联动禁用/表格格式化断言+切入验证/验证失败不查询/开关关闭放行/操作共用）

#### 4.5.5 网络质量检测 — `quality.rs`

**details/metrics 键契约**: `details`/`metrics` 字典以检测任务 `name` 为键，统一为英文标识符：`gateway`/`aliDns`/`tencentDns`/`xinfengDns`/`aliDoh`/`tencentDoh`/`dnsResolve`（SystemDns 聚合条目）+ 12 个 HTTPS 站点 `baidu`/`jd`/`bing`/`railway12306`/`bilibili`/`bilibiliLive`/`douyin`/`douyinLive`/`lol`/`genshin`/`pubg`/`naraka`。前端消费点（`lib/latency.ts` 的 gateway 过滤、`NetworkQualityCapsule` 的 dnsResolve、`QualityPanel` 分组 names）同步改标识符，**条目显示名**走 i18n `quality.names.*`（zh 保留原中文/en 英文名）。前后端键必须同步修改（同仓库同发版，无兼容窗口）。

**两阶段检测**:
1. Phase 1: 并行测试网关 + 3 个 DNS + 2 个 DoH + 系统 DNS → 更新评分表 → 增量推送
2. Phase 2: 并行预解析 HTTPS 主机名 → 分批（每批4个）并行测试 12 个 HTTPS 网站 → 每批完成后增量推送

**质量等级**: `excellent`(<=20ms) / `great`(<=50ms) / `good`(<=100ms) / `fair`(<=200ms) / `poor`(<=400ms) / `bad`(>400ms) / `unknown`(<0)

**函数签名**:
```rust
pub async fn check_network_quality_async(
    _adapter_name: &str, adapter_ip: &str, skip_ttfb: bool, skip_content: bool,
    fixed_gateway: &str, is_quitting: Arc<AtomicBool>, app_handle: Option<&AppHandle>
) -> NetworkQualityResult
```

**增量推送机制**:
- `app_handle` 为 `Some` 时启用增量推送（latency loop / watcher 调用），为 `None` 时不启用（手动检测调用）
- Phase 1 完成后立即 `emit("network-quality-result", ...)` 推送部分结果
- 每个 HTTPS 批次完成后推送累计结果（Phase 1 + 已完成的 HTTPS 批次）
- 前端 `mergeNetworkQuality` 函数天然支持合并增量数据

**关键行为**:

| 项 | 说明 |
|--------|------|
| HTTPS 绑定认证适配器 | HTTPS 测试绑定经 Portal 认证的适配器 IP（`ctx.bind_addr`）——系统默认路由选中未认证网卡（如 WLAN）时 TLS 握手全部超时；绑定直连失败且 bind_addr 非空时回退系统路由重试一次（覆盖 Clash TUN 等劫持场景） |
| DNS 解析优先 IPv4 | `resolve_host_uncached_with_bind` 优先返回 `is_ipv4()` 结果，避免 IPv6 地址导致连接失败 |
| 增量推送 | `app_handle: Option<&AppHandle>`：Phase 1 与 HTTPS 批次完成后立即 emit，前端 `mergeNetworkQuality` 合并增量数据；`None` 为手动检测不推送 |
| HTTPS 分批并发 | Phase 2 每批 4 个分批并发，减少校园网高 RTT 下 TLS 带宽竞争 |
| RAII guard + 启动延迟 1s | `is_quality_checking.try_acquire()` 返回 `TaskGuard` 自动释放；每轮检测前 sleep 1s 避免网络未稳定时 HTTPS 延迟异常 |

**任务类型** (`LatencyTask` 枚举):

| 变体 | 说明 |
|------|------|
| `Gateway` | 网关 ICMP ping |
| `DnsServer` | DNS 服务器延迟测试 |
| `Doh` | DoH 服务器延迟测试 |
| `Https` | HTTPS 网站延迟测试 |
| `SystemDns` | 系统 DNS 解析延迟测试 (多域名平均) |

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `tcp_then_icmp_latency()` | TCP 优先延迟测试，ICMP 降级 |

#### 4.5.6 DNS 按配置文件设置 — `platform/dns_config.rs`

**推荐服务器常量 (2026-09-03 起 IPv4+IPv6 双栈)**:
| 常量 | 值 | 说明 |
|------|------|------|
| `PRIMARY_DNS` / `SECONDARY_DNS` | `223.5.5.5` / `1.12.12.12` | 阿里 / 腾讯 IPv4 |
| `PRIMARY_DNS_V6` / `SECONDARY_DNS_V6` | `2400:3200::1` / `2402:4e00::` | 阿里 / 腾讯 DNSPod IPv6（官方公布地址） |
| `DOH_SERVERS` | 4 条 IPv4 + 3 条 IPv6（`2400:3200::1`、`2400:3200:baba::1` → dns.alidns.com，`2402:4e00::` → doh.pub） | DoH 模板按域名，双栈服务器复用同一模板 |

**Per-Profile DNS 设置**:

| 函数 | 说明 |
|------|------|
| `set_profile_dns_via_api()` | 使用 `DNS_SETTING_PROFILE_NAMESERVER` (0x0200) 设置配置文件级 DNS，仅对当前 WiFi 生效 |
| `clear_adapter_dns_via_api()` | 清除适配器级 DNS (`NameServer`)，IPv4/IPv6 两栈各调一次（只清 v4 栈会残留旧 v6 静态配置），使配置文件级 DNS 生效 |
| `set_dns_via_api()` | 适配器级 DNS+DoH 设置（DoH 属性已合并进 `set_dns_inner` 的 DnsTarget 分支，无独立 set_doh 函数） |

> **双栈拆分契约**: `SetInterfaceDnsSettings` 一次调用只作用于一个栈——默认仅 IPv4，带 `DNS_SETTING_IPV6` (0x0001) 时仅 IPv6，NameServer 地址族必须与目标栈一致（netioapi.h 文档 + Mullvad talpid-dns 双重佐证）。`split_families()` 把 NameServer 列表按 `:` 分组，v4/v6 各调一次 `set_dns_stack()`；DoH 属性字段与 flag 一一对应：Interface → `ServerProperties` (DNS_SETTING_DOH 0x1000)，Profile → `ProfileServerProperties` (DNS_SETTING_DOH_PROFILE 0x2000)——**Profile 的 DoH 若挂在 ServerProperties 上则 per-profile DoH 从未真正写入**。`ServerIndex` 按本栈列表实际下标（`doh_bindings`）。

> DNS+DoH 一键设置（`network/dns_setup.rs::setup_dns_doh_admin(targets, family)`）按 `family`（"ipv4"/"ipv6"/"both"，默认 both）决定 NameServer 列表（内部经 `split_families` 分栈各写一次，不做混合串），`doh_bindings` 按服务器 IP 精确匹配模板（含 IPv6），netsh 全局 DoH 注册循环同样覆盖 v6。WiFi 分支先 `clear_adapter_dns_via_api` 再写 profile；**清除失败时改走接口级设置**（接口级残留会覆盖 profile DNS）。前端 NetworkPanel 用 `SegmentTabs` 三档选择（IPv4/IPv6/IPv4+IPv6，默认双栈），helper 提权路径经 `--helper dns <名单...> --family <v>` 传递。DNS 检测 `should_filter_ip` 对非点分格式返回 false（IPv6 可正常读取显示）；`netsh dns show encryption` 输出解析按 Ipv4Addr/Ipv6Addr 可解析性识别服务器行（仅匹配点分十进制会漏检 v6 条目）；前端 `ALI_DNS`/`TENCENT_DNS` 推荐集合已含 v6 地址。

**DNS 检测增强**: `read_adapter_dns_from_registry()` 同时读取 `NameServer`（适配器级）和 `ProfileNameServer`（配置文件级），source 优先级为 manual > profile > dhcp，输出 `dnsSource`/`profileDnsServers`/`adapterDnsOverridesProfile` 字段

### 4.6 DNS 智能解析 — `network/dns.rs + network/timing.rs`

> **位置说明**: DNS 评分系统（`DNS_SERVER_SCORES`/`DOH_SERVER_SCORES`/`ServerScore`）、评分更新/查询函数、DoH 解析函数（`resolve_via_doh`/`resolve_host_smart`/`resolve_host_uncached_with_bind` 及辅助函数）均位于 `network/dns.rs`。HTTP 计时函数（`measure_https_timing`/`measure_dns_query`/`measure_doh_timing`）位于 `network/timing.rs`。

#### DNS 服务器动态评分系统

```rust
static ref DNS_SERVER_SCORES: DashMap<String, ServerScore>;   // network/dns.rs
static ref DOH_SERVER_SCORES: DashMap<String, ServerScore>;   // network/dns.rs

// DNS 与 DoH 评分共用同一结构体
struct ServerScore { latency_ms: i64, success: bool, last_tested: Instant }
```

**关键函数**:

| 函数 | 说明 |
|------|------|
| `update_dns_server_latency()` | 更新 DNS 服务器评分 |
| `update_doh_server_latency()` | 更新 DoH 服务器评分 |
| `get_best_dns_servers()` | 按延迟升序返回可用 DNS 服务器 (600s过期回退默认) |
| `get_best_doh_servers()` | 按延迟升序返回可用 DoH 服务器 |
| `resolve_host_uncached_with_bind()` | 无缓存 DNS 解析，自定义 DNS 服务器 + 系统降级 |

**默认服务器**:

| 类型 | 服务器 |
|------|--------|
| DNS | `223.5.5.5`, `1.12.12.12`, `114.114.114.114` |
| DoH | `dns.alidns.com` (223.5.5.5), `doh.pub` (1.12.12.12) |

#### 应用级 DoH 解析

```rust
pub(crate) async fn resolve_via_doh(doh_server: &str, doh_ip: IpAddr, domain: &str, bind_addr: Option<IpAddr>, timeout: Duration) -> Result<IpAddr, String>
```

- 直接 TCP 连接 DoH 服务器 443 端口 → TLS 握手 → 发送 RFC 8484 wire format 查询
- `?dns=<base64url>` + `Accept: application/dns-message`
- HTTP 200 状态校验，非 200 返回错误
- 完全绕过系统 DoH API

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `build_dns_query_wire()` | 构建标准 DNS wire format 查询报文 |
| `base64url_encode_no_pad()` | RFC 8484 要求的 base64url 编码 (无填充) |
| `parse_dns_response_wire()` | 解析 DNS wire format 响应，提取 A 记录 IP |

#### 三级智能解析策略

```rust
pub async fn resolve_host_smart(host: &str, timeout: Duration, bind_addr: Option<IpAddr>) -> Result<IpAddr, String>
```

```
DNS缓存 (TTL 60s) → DoH + 传统DNS 并发竞速（首个成功即返回并缓存）→ 系统DNS降级
```

- 第一级: 查询 DNS 缓存
- 第二级: DoH + 传统 DNS 并发竞速，使用延迟最优的服务器（实际并发"历史最快 1 路 DoH + 传统 DNS"，每域名 TLS 握手减半），首个成功即返回并缓存
- 第三级: 自定义 DNS 失败时自动回退到系统 DNS（`SYSTEM_RESOLVER_CONFIG = read_system_conf()` 读取真实 OS 配置，仅读取失败才退 `ResolverConfig::default()`）

#### HTTP 计时

| 函数 | 说明 |
|------|------|
| `measure_https_timing()` | HTTPS 完整计时 (DNS/TCP/TLS/TTFB/Content)，使用 `resolve_host_smart`，单一 deadline 避免累加超时 |
| `measure_dns_query()` | DNS 查询计时 (UDP + TCP 并发) |
| `measure_doh_timing()` | DoH 查询计时，DNS 降级重试 |
| `bind_and_connect()` | 绑定源 IP 的 TCP 连接 |
| `do_tls_handshake()` | tokio_rustls TLS 握手，协商版本检测 |

### 4.7 登录/注销模块 — `commands/login.rs` + `auth/service.rs`

**登录命令** (`commands/login.rs` 委托 `auth/service.rs`):

| 函数 | 所在模块 | 说明 |
|------|----------|------|
| `do_login(adapter_name?)` | `commands/login.rs` | Tauri 命令，支持可选指定适配器 |
| `full_login()` | `auth/service.rs` | 登录核心逻辑 (单/双适配器分支) |
| `login_adapter_with_log()` | `auth/session.rs` | 单适配器登录+日志 |
| `adapter_action_with_log()` | `auth/session.rs` | 通用适配器操作+日志封装 |
| `post_login_handler()` | `auth/service.rs` | 登录后处理：解除注销保护期 → 仅当 `enable_background_check` 开启时延迟500ms触发 `monitor::watcher::run_background_check` → 按需启动 `auto_exit` |
| `check_any_adapter_online()` | `commands/login.rs` | 双适配器 Portal 在线检测 `std::thread::scope` 并行（延迟减半）；`do_logout` 复用其逐适配器检测结果避免重复请求。**scope 裸子线程无 Tokio runtime context**（reqwest 构造计时器时 `Handle::current()` panic，panic=abort 整进程崩溃），子线程闭包必须 `tauri::async_runtime::handle().inner().enter()` 进入 context |

**注销命令** (`commands/login.rs` 委托 `auth/service.rs`):

| 函数 | 所在模块 | 说明 |
|------|----------|------|
| `do_logout(adapter_name?)` | `commands/login.rs` | Tauri 命令，支持可选指定适配器 |
| `full_logout()` | `auth/service.rs` | 注销核心逻辑 (双适配器串行) |
| `logout_adapter_with_log()` | `auth/service.rs` | 单适配器注销+日志 |

**锁语义**: 登录使用 `is_logging_in`，注销使用独立的 `is_logging_out`

**适配器解析** (直接函数调用，无 trait 抽象):

> 适配器解析为直接函数调用，无 trait 抽象：`auth/service.rs` 直接调 `crate::network::resolve_adapter_names`/`find_dual_adapters` 等自由函数，测试用真函数。

**双适配器并行执行** (`auth/dual_adapter_executor.rs`，168 行含测试):

| 项 | 说明 |
|------|------|
| `DualAdapterResult` | 双适配器执行结果结构体 (`primary`/`secondary` 两个 `Option<CommandResult>`) |
| `execute_dual<F1, F2>(a1_action: F1, a2_action: F2, is_quitting)` | 双适配器并行执行器：泛型 `F1`/`F2` 静态分发（无 Box 堆分配与虚函数调用）；适配器1立即 `spawn_blocking`，适配器2通过 10×100ms 轮询 `is_quitting` 实现可中断 1s 错峰 |

**认证失败计数与 Portal 请求失败容错** (`auth/failure_tracker.rs`):

| 函数 | 说明 |
|------|------|
| `is_auth_failure()` | 判断 CommandResult 是否为认证失败 (`AUTH_FAILURE_CODES: ["ac_auth_failed","1","4"]`) |
| `update_auth_failure_count()` | 单适配器认证失败计数，连续5次触发该登录适配器（调用方 resolve 后传入 `adapter_name`）的 MAC 重置+DHCP 续租（走 `dhcp_release_renew_single`） |
| `update_dual_adapter_auth_failure()` | 双适配器分别计数，各自5次触发单适配器 MAC 重置 |
| `handle_portal_request_failure()` | Portal HTTP 请求失败容错，`PORTAL_REQUEST_FAILURE_THRESHOLD=5`；网关不可达时跳过计数并重置（校园网断网/维护期避免误重置 MAC），达阈值触发 `dhcp_release_renew_single` |
| `reset_all()` | 重置所有认证失败计数器 |

> `AdapterFailureCounter` 枚举 (A1/A2) 统一认证失败与 Portal 请求失败的计数访问器（`get_adapter_failure_count`/`set_adapter_failure_count` 两个访问器；自增经 `NetworkState::update_with_result` 闭包内联完成，无独立 increment 函数）。

**注销成功后状态重置**（区分全量/单适配器）:

- **全量注销**（未指定 `adapter_name`）：重置 `any_adapter_online`/`last_a1_online`/`last_a2_online`/`has_logged_online` 为 false，`disconnect_reconnect_count` 归零，重置 `last_auto_login_attempt` 为当前时间，取消自动退出倒计时，设置 60 秒注销保护期 (`logout_protected_until`)
- **单适配器注销**（指定 `adapter_name`）：仅重置对应适配器的 `last_a1_online` 或 `last_a2_online`，重新计算 `any_adapter_online = a1 || a2`，其余标志保持不变

### 4.8 后台巡检 — `monitor/` (watcher 门面 + background_check 主体 + background_task 调度)

> `watcher.rs` 是 54 行门面：re-export 子模块函数 + `run_startup_tasks` 启动聚合入口；检测主体在 `background_check.rs`，任务调度在 `background_task.rs`，Portal 失败容错在 `auth/failure_tracker.rs`。外部调用路径（`monitor::watcher::run_background_check` 等）经 re-export。

#### 4.8.1 watcher.rs — 门面 + 启动聚合 (54 行)

**职责**：纯门面模块，re-export 子模块函数 + 启动期聚合任务。

**re-export**：
- `pub use super::background_check::run_background_check;`
- `pub use super::background_task::start_background_check_inner;`
- `pub use super::campus_check::check_campus_network;`
- `pub use super::background_emit::{adapter_status_entry, adapter_disabled_entry, adapter_disconnected_entry};`

**`run_startup_tasks(app_handle)`**：启动期聚合入口，依据 config 注册 3 个 `task_manager.spawn` 跟踪任务：
- `startup_bg_check` — 启动后台检测
- `startup_latency` — 启动延迟测试循环
- `startup_auto_login` — 启动自动登录

#### 4.8.2 background_check.rs — 后台检测主体 (~306 行)

**职责**：一次完整后台检测周期：获取适配器列表 → 解析双适配器名 → 校园网环境验证 → Portal 检测（含双适配器并行）→ Portal 失败容错 → 状态更新与事件下发 → 自动登录/断开重连触发 → 网络质量检测调度。

**关键函数**：

| 函数 | 说明 |
|------|------|
| `run_background_check_blocking(app_handle, state, cancel_token)` | 同步主体 (~306 行)，无返回值（质量检测已收敛至 latency loop 独占驱动，见 4.11） |
| `run_background_check(app_handle, cancel_token)` | async 包装：`spawn_blocking` 跑 `run_background_check_blocking`，不再触发质量检测 |

**核心类型** (定义于 monitor 子模块，非 watcher.rs)：

```rust
// portal_check.rs
enum PortalCheckResult {
    Success { online: bool, message: String, reachable: bool, login_available: bool },
    Error { is_request_failed: bool },
    NotFound,
}

// campus_check.rs
struct CampusCheckResult {
    wifi: Option<ConnectionCampusStatus>,   // WiFi 校园网状态
    wired: Option<ConnectionCampusStatus>,  // 有线校园网状态
    on_campus: bool,                         // 综合判定是否在校园网
    current_ssid: Option<String>,            // 当前 WiFi SSID
    message: String,                         // 状态消息
}

struct ConnectionCampusStatus {
    on_campus: bool,
    name: Option<String>,
    message: String,
}
```

**background_check 调用的子函数** (定义在 monitor/ 子模块):

| 函数 | 实际所在模块 | 说明 |
|------|------------|------|
| `check_adapter_portal()` | `portal_check.rs` | 单适配器 Portal 检测，消除主/副重复 |
| `handle_portal_request_failure()` | `auth/failure_tracker.rs` | Portal 请求失败容错（见 4.7） |
| `build_adapter_details()` / `handle_status_change()` / `emit_background_check_result()` / `update_network_state()` / `adapter_status_entry()` 等 | `background_emit.rs` | 适配器详情/状态变更/检测结果事件/网络状态更新/状态条目构建 |
| `check_campus_network()` | `campus_check.rs` | WiFi/有线分别检测校园网状态 |
| `run_quality_check()` | `quality_scheduler.rs` | 质量检测调度；签名含 `cancel: Option<&CancellationToken>`：定时测试循环传入循环 token，复核窗口可即时中断；手动检测路径传 None 行为不变 |

**双适配器并行 Portal 检测**：`run_background_check_blocking` 用 `tauri::async_runtime::spawn_blocking` + `tokio::join!` 并行检测双适配器（与 `dual_adapter_executor` 策略一致；**不可用 `std::thread::scope` 裸子线程——无 Tokio reactor 上下文会 panic**）。

**校园网检测集成**：三级校园网检测（网络名称→/18子网→网关Ping），结果含 `currentSsid`/`onCampusNetwork`。**无网络保护**：配置适配器均无IP时跳过校园网退出流程，等待网络恢复后重新检测。

**注销保护期机制**：注销成功后设置 60 秒保护期 (`logout_protected_until`)，期间：
- `update_network_state` 跳过原子状态更新
- `emit_background_check_result` 强制 `online=false`，避免 Portal 延迟导致前端误判
- `check_portal_status` API 直接返回 `{ online: false }`，不再请求 Portal 服务器

#### 4.8.3 background_task.rs — 任务调度层 (~55 行)

**职责**：后台检测任务的生命周期/调度层。配置更新（开启 `enable_background_check`、修正过小间隔）→ 通过 `task_manager.spawn("background_check", ...)` 注册可取消任务 → 首次立即执行 + `tokio::time::interval` 周期循环 + `cancel_token.cancelled()` 优雅退出 → 落盘配置。

**关键函数**：

| 函数 | 说明 |
|------|------|
| `start_background_check_inner(app_handle, state) -> Result<CommandResult, String>` | 注册 `background_check` 任务到 task_manager，循环内通过 `is_running`/`cancel_token` 管理自身 |

> 注：`trigger_background_check` 是 `commands/background.rs` 的独立 Tauri 命令（一次性手动触发，spawn 单次 `run_background_check`，不注册跟踪任务、不落盘），并非 `start_background_check_inner` 的别名；`monitor/mod.rs` 仅做 10 个 `pub mod` 声明，无 re-export。

#### 4.8.4 Portal 请求失败容错 — `auth/failure_tracker.rs`

> Portal HTTP 请求失败容错与认证失败计数统一在 `auth/failure_tracker.rs`，详见 4.7 `handle_portal_request_failure()`。

**Portal 容错完整链路**：

1. 主/副适配器 Portal 请求失败（`is_request_failed: true`）时，对应适配器 `a1_auth_failure_count`/`a2_auth_failure_count` 自增（CAS 更新 NetworkSnapshot）
2. 失败时先检查网关从该适配器IP是否可达（`check_gateway_reachable_from()`），不可达则跳过计数并重置（校园网断网/维护期避免误重置 MAC）
3. 连续 5 次失败（`PORTAL_REQUEST_FAILURE_THRESHOLD=5`）触发 `dhcp_release_renew_single`（仅对该失败适配器 MAC 重置 + DHCP 续租，仅对校园网子网适配器生效）
4. 触发后重置计数器为 0
5. Portal 检测恢复正常（`Success`）时 CAS 写入 0 重置计数器并记录原值日志

> `auth/failure_tracker.rs` 统一管理**认证失败**（`AUTH_FAILURE_CODES: ["ac_auth_failed","1","4"]`）与 **Portal HTTP 请求失败**两类计数，共用 `AdapterFailureCounter` 枚举 (A1/A2) 与计数访问器（`get_adapter_failure_count`/`set_adapter_failure_count`，自增经 `update_with_result` 闭包内联）。

### 4.9 自动登录模块 — `monitor/auto_auth.rs`

**公开函数**:

| 函数 | 说明 |
|------|------|
| `try_auto_login_on_preparation()` | 准备阶段自动登录 (冷却秒数取配置 `auto_login_cooldown_secs`，默认 60)，`has_logged_online` 为 true 时跳过；`prep_login_failures` 连续失败达上限后本会话停止 |
| `try_disconnect_reconnect()` | 断线重连 (最多 `max_disconnect_reconnect` 次，配置默认 3 + 间隔提醒) |
| `run_auto_login_on_start()` | 启动时自动登录 (条件延迟：自启场景5s/非自启1.5s + Portal预检 + 无网络保护：配置适配器无IP时跳过校园网退出) |

### 4.10 自动退出模块 — `infra/lifecycle.rs`

**关键常量**:

| 常量 | 值 | 说明 |
|------|----|------|
| `CAMPUS_MINIMIZE_DELAY_MS` | 30000 | 校园网退出最小化延迟 (不变) |
| `CAMPUS_EXIT_DELAY_MS` | 60000 | 校园网退出总延迟 (不变) |

| 函数 | 说明 |
|------|------|
| `start_auto_exit()` | 启动自动退出倒计时 + 快捷键注册 + 通知；spawn 失败时回滚 deadline（否则残留 deadline 使后续触发全部短路，本轮自动退出静默失效） |
| `cancel_auto_exit_inner()` | 取消自动退出：清 deadline + cancelled 标志 + **同步 `task_manager.cancel("auto_exit")`**（只清标志不清任务会留下同名残留，20s 内重新登录时 spawn 被拒→假倒计时且 deadline 残留短路后续触发） |
| `start_campus_exit()` | 校园网验证不通过时：30s后最小化到托盘，再30s后强制退出 (受 `campus_exit_on_fail` 控制)。**先 CAS 防止重复触发，成功后再设置 deadline**（顺序反了会 deadline 被推后、标志位永久卡死） |
| `cancel_campus_exit()` | 取消校园网退出流程：swap 标志 + 清 deadline + **同步 `task_manager.cancel("campus_exit")`**。如果自动退出未运行，注销快捷键 |
| `cancel_campus_exit_with_notification()` | 快捷键取消校园网退出 (含通知、快捷键注销与同步 cancel 任务) |
| `shutdown_and_exit()` | 统一退出入口 (async)：设置 `is_quitting` → `task_manager.shutdown()` 清理后台任务（**整体 10s 超时上限**）→ `app_handle.exit(0)`。被 `start_campus_exit` 和 `start_auto_exit` 共同调用 |

**TOCTOU 防护**：`start_campus_exit`/`cancel_campus_exit`/`cancel_campus_exit_with_notification` 3 处 `auto_exit_deadline` 的 check-then-act（含 `try_unregister_cancel_exit_shortcut`）均持同一锁临界区完成，防 `start_auto_exit` 在间隙注册快捷键。

### 4.11 延迟测试模块 — `monitor/latency.rs`

| 函数 | 说明 |
|------|------|
| `classify_quality_change()` | 质量档位切换纯判定（不落状态不发通知）：恶化到 poor/bad → `Some("bad")`，从 poor/bad 恢复 → `Some("good")`；`BAD_LEVELS` 常量与复核共用，5 个单测锁定 |
| `record_last_quality()` | 落盘 `last_network_quality` 状态（NetworkSnapshot） |
| `notify_quality_change()` | 发送网络质量通知（bad=网络拥堵 / good=网络恢复）+ `emit_login_log`，通知通道单一化 |
| `spawn_latency_test_loop()` | 启动延迟测试循环 (CancellationToken) |

**关键策略**:

| 策略 | 说明 |
|--------|------|
| 后端统一通知 | 网络质量变化通知由后端统一发送（`notify_quality_change`） |
| 未在线跳过 | `spawn_latency_test_loop` 每轮检查 `any_adapter_online`，Portal 未认证时跳过自动检测（未认证时外网 HTTPS 必被拦截、全超时且误报"网络拥堵"）；前端手动触发的 `check_network_quality` 命令不受限 |
| 质量驱动者收敛 | 全量质量检测周期驱动者收敛为定时测试循环一个（条件 `enable_network_quality && enable_latency_test`）；后台巡检不再顺带触发（`run_background_check_blocking` 返回 `()`），定时测试间隔（最小 10s）真实生效；信号量 `is_quality_checking` 防与手动检测并发。代价：不开"定时测试"则质量面板无周期数据（仅手动检测按钮） |
| 延迟升高复核确认 | 恶化到 poor/bad 不立即通知：`run_quality_check`（quality_scheduler.rs）以 15s 间隔再连续复核 2 次（`SPIKE_CONFIRM_COUNT=2`/`SPIKE_CONFIRM_INTERVAL_SECS=15`），全部达到 `BAD_LEVELS` 才发"网络拥堵"通知——防瞬时抖动误报；复核检测复用 `perform_quality_check`（前端数据照常更新仅门控系统通知），复核未执行成功按证据不足处理不通知，下轮可重触发；恢复通知不受影响 |
| 就绪短重试 | 未就绪（适配器无 IP / `any_adapter_online=false`）时内层循环每 2s 短重试且不消耗周期 tick（走 `continue` 会耗尽 interval 即时首 tick 后干等完整周期，而在线状态要等后台巡检首次探测才置 true）——启动后首次质量结果从 30s+ 缩短到数秒 |

### 4.12 适配器监控模块 — `monitor/adapter_watch.rs`

| 函数 | 说明 |
|------|------|
| `start_adapter_watch()` | 启动适配器状态监控循环 (15s间隔，CancellationToken可退出)，适配器恢复时触发重新检测，禁用适配器通知节流(60s内不重复)。**自动启用**：用户手选适配器（adapter1/adapter2 非空且非"自动检测"哨兵）出现在禁用列表时自动 `enable_adapter(name, false)`（COM 静默提权，**绝不弹 UAC**），失败按 0/60s/120s/300s 封顶退避重试（`auto_enable_backoff_ms`），成功清零计数并延迟 5s 触发一次性 `run_background_check` 衔接登录；"自动检测"模式不参与 |

### 4.13 网络命令模块 — `commands/network_cmd.rs`

**DNS/DoH 检测与设置** (委托 `platform/dns_config.rs`):

| 函数 | 说明 |
|------|------|
| `check_portal_status()` | 检测 Portal 认证状态（注销保护期内直接返回离线） |
| `check_campus_status()` | 检测校园网状态，返回 campusWifi/campusWired 字段 |
| `check_dns_doh_status()` | 通过 winreg 读取注册表检测 DNS/DoH 状态 |
| `setup_dns_doh()` | 一键设置推荐 DNS + DoH (带 `family` 参数 "ipv4"/"ipv6"/"both" 默认 both；WiFi用配置文件级DNS，有线用适配器级DNS；管理员直调 `dns_setup::setup_dns_doh_admin`，非管理员经 `--helper` 提权重启自身) |

**UAC 提权** (位于 `platform/elevation.rs`):

```rust
fn is_admin() -> bool {
    // OpenProcessToken + GetTokenInformation(TokenElevation) 检测管理员权限
}

fn run_elevated(cmd: &str, args: &str) -> Result<(), String> {
    // ShellExecuteW + "runas" 实现UAC提权，耗时约1ms
}

fn shell_exec_elevated(file: &str, params: &str, hide_window: bool) -> Result<(), String> {
    // COM ShellExecuteW 提权方式，作为 run_elevated 的优先替代
    // hide_window=true 时 n_show=0 (SW_HIDE)，否则 n_show=1 (SW_SHOWNORMAL)
}

fn co_get_object_raw() -> ... {
    // 直接链接 ole32::CoGetObject，COM 提权底层实现
}

fn parse_guid(s: &str) -> Result<GUID, String> {
    // 字符串 -> windows::core::GUID 解析
}
```

**注册表路径**:
- 适配器 DNS: `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{GUID}\NameServer`
- 适配器名称映射: `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-...}\{GUID}\Connection\Name`
- DoH 配置: `HKLM\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DohWellKnownServers\{IP}`

**安全**: 提权操作不再拼装 shell 命令 —— helper 由 Rust 直调 Win32/winreg，适配器名/参数通过 `get_adapters_force` 按 GUID 解析，无命令注入面

### 4.14 其他命令模块

**config_cmd.rs** — 配置保存/加载 (委托 `config/persist.rs`)，空密码兜底逻辑 (前端未传密码且旧密码存在时保留旧密码)；`save_config` 可选参数 `clear_password`：显式为 true 时跳过兜底强制置空密码，供账号面板"清除密码"使用；`clearSelfPassword` 同语义清除自助服务密码（前端经 `saveConfig(cfg, clearPassword, clearSelfPassword)` / useConfigStore 的 `saveConfigDirect` 透传）

**account.rs** — 多账号管理命令（逻辑自含于本文件；`account/mod.rs` 仅声明 crypto 子模块），使用 `list_account_names()` 共享函数，切换账号仅替换账号相关字段保留启动设置，删除账号前检查并清空 `active_account`

**system.rs** — 系统功能命令，`get_init_data` 复用 `persist::list_account_names()` 获取账号列表，返回字段含 `gpuInfo`/`refreshRate`；登录历史记录 `append_login_history()`（最多100条）定义于 `config/persist.rs`，由 `auth/session.rs` 与 `monitor/auto_auth.rs` 调用

**updater.rs** — 更新命令 (委托 `update/updater.rs`)，SHA256 校验和全 4xx 缺失时**默认拒绝安装**（需 `skipSha256WhenMissing`，无前端开关；5xx/传输错误/哈希不匹配一律拒绝），MSI 安装使用 `raw_arg` 支持含空格路径；`get_mirror_urls` 镜像 URL **原样拼接不做百分号编码**（gh-proxy.com 对整体编码形式返回 403，与 updater.rs 的 sha256 镜像拼接方式一致）；自动检查循环发现新版本时走 `platform/toast.rs::show_update_toast`（带点击回调的 WinRT toast，点击唤起主窗口并 emit `update-notification-click`，前端打开关于界面；失败降级普通 `emit_notification`）

### 4.15 提权辅助子进程 — `helper/` (--helper 模式)

**动机**: 需要管理员权限的操作（改 MAC / 设 DNS+DoH）走**提权重启自身**：以管理员身份启动当前 exe 并附加 `--helper <op>`，由 Rust 直调 Win32/winreg 完成，无 PowerShell 依赖与 shell 拼接注入面。

**执行流**:
1. **主进程** (`platform/helper_spawn.rs::spawn_elevated_helper`)：`std::env::current_exe()` 取自身路径，生成唯一结果文件路径（`%TEMP%/campus-login-helper-<pid>-<ts>.json`），拼参数 `--helper <op> ... --result <path>`，按现有降级链提权启动（COM ICMLuaUtil 静默 → 失败 ShellExecuteW runas 弹 UAC）
2. **helper 进程** (`main.rs` 顶部拦截)：`helper::parse_helper_args` 解析出 `HelperOp`（`Dns{targets 适配器名单, family}` / `Mac{guid, mac_no_dash}`，`--family` 参数默认 "both"，op 后到首个 `--` 参数前为位置参数），`run_helper` 执行：
   - `Dns` → `network::dns_setup::setup_dns_doh_admin(targets, family)`（枚举活跃适配器 → Win32 设置 → 全局 DoH 注册 → flushdns）
   - `Mac` → 按 GUID 在 `get_adapters_force` 中解析适配器名 → `dhcp::apply_mac_change_via_registry`（写注册表 NetworkAddress + release/disable/enable/renew）→ **`dhcp::remove_mac_from_registry` 在 helper 提权上下文内清除注册表伪装值**（非提权进程写必然 Access Denied 且被 log_warn 吞掉，伪装 MAC 会每次重启后持续生效；运行中 MAC 不受影响，重启后恢复物理 MAC）
3. **结果回传**: helper 把 `HelperResult{success, message, op, logs, details: Option<serde_json::Value>}` 原子写入结果文件（tmp + rename，`details` 透传 DNS 设置明细给前端），主进程 100ms 间隔轮询（DNS 超时 30s / MAC 超时 25s），读取后把 `logs` 并入主进程日志，返回 JSON 结果

**要点**: helper 进程不初始化 logger（避免与主进程跨进程写同一日志文件竞争）；参数仅含 GUID/MAC/结果路径等受控字符，适配器名由 helper 自行枚举，无 shell 拼接注入面。

---

### 4.16 安卓端后端 — `android/src-tauri/`

**总原则（与桌面端的复用边界）**: 协议核心**单点共享**——`campus-login = { path = "../../tauri-app/src-tauri" }`，登录/注销/Portal 探测/自助服务/网络质量/日志系统直接复用桌面 crate，**禁止复制协议逻辑**；桌面侧 cfg 门控模块（app/helper/monitor/update/platform Windows 部分）对安卓不可见。安卓侧只做三件事：**平台探针**（校园网判定/源 IP）、**状态管理**（Keystore 加密配置 + 监控状态机）、**命令面包装**（44 个命令与桌面同名对齐，前端 tauriApi 两端一致）。

**入口 `lib.rs`** (102 行): `#[cfg_attr(mobile, tauri::mobile_entry_point)]`。插件注册顺序：`campus_network_bind` → `campus_keystore` → `campus_monitor_service` → `tauri_plugin_biometric`（host 编译为空的 mobile-only crate）→ `tauri_plugin_notification` → `tauri_plugin_opener`。Setup 钩子：日志初始化到 `app_data_dir/logs`（与桌面 `infra::logger::get_log_dir` 的 android 分支一致）→ `monitor_loop::run_startup_tasks`（启动恢复，见下）。`generate_handler!` 注册 44 个命令。

#### 4.16.1 模块清单

| 模块 | 行数 | 职责 |
|------|------|------|
| `protocol_cmds.rs` | 180 | 登录/注销/Portal 探测/WiFi 绑定命令（包装桌面协议核心） |
| `campus_detect.rs` | 220 | 校园网探针：源 IP 选取 + 三层判定 + `detect_campus`/`check_campus_status` |
| `config_state.rs` | 546 | 全量配置 Settings（Keystore 加密落盘/掩码出口/schema 迁移）+ `CryptoBridge` |
| `monitor_loop.rs` | 620 | 后台监控状态机：tick 检测（静默期时间门控）→ 自动重登 → 事件推送 + 启动恢复编排 |
| `self_service_cmds.rs` | 318 | 自助服务六命令 + 生物识别验证门（包装桌面 self_service 协议） |
| `identity_gate.rs` | 66 | 验证门 TTL 时间戳（600s，语义同构桌面 platform/identity.rs） |
| `account_cmds.rs` | 238 | 多账号管理（与桌面 commands/account.rs 同构）+ 全局配置 IO 串行锁 |
| `system_cmds.rs` | 160 | `get_init_data` 聚合 + 日志族 + `get_soc_info` 设备分档 |
| `quality_cmds.rs` | 73 | 网络质量/定时延迟测试（复用跨平台 network::quality） |
| `update_cmds.rs` | 418 | 更新检查（镜像降级）+ APK 流式下载 + SHA256 校验 + 安装 |
| `login_history.rs` | 142 | 登录历史追加（manual/auto 来源） |
| `cpu_affinity.rs` | 69 | 大小核拓扑识别 + 线程绑小核（功耗适配） |
| `android_state.rs` | 13 | 进程态：wlan0 源 IP 缓存 + 配置内存态 |

#### 4.16.2 关键设计

- **登录/注销（protocol_cmds.rs）**: `do_login`/`do_logout` 参数与桌面同名对齐（adapter 参数收下不用），凭据空/MASK 时回退已存配置（总览一键登录免输凭据）。桌面 `do_login_with_retry`/`do_logout_with_retry` 是同步函数（内部 `block_on_http` 桥接），**禁止在 async 上下文直调**，一律 `tauri::async_runtime::spawn_blocking`。源 IP 用检测阶段缓存的 wlan0 地址（`AndroidState.cached_source_ip`）。敏感纪律：日志/错误/事件 payload 不携带 password。注销成功设 60s 注销保护期（防后台检测把用户自动登回）；手动登录成功清自动重登熔断计数与保护期。
- **校园网探针（campus_detect.rs）**: 安卓**无 SSID 通道且非 root 无 ICMP**（原始 socket 被 SELinux 禁止，surge-ping 不可用），桌面三层判定（SSID→子网→ICMP 网关）的安卓版改为：**/18 子网判定**（复用桌面纯函数 `is_same_subnet_18`）→ **网关 TCP 可达**（内网地址，救回跨 /18 的 AP 区段）→ **Portal TCP 可达**（最后兜底；配置公网 portal 域名时家宽也可能连通，属已知边界）。源 IP 选取 `pick_campus_source_ip`：排除蜂窝接口（`rmnet*` 高通系/`ccmni*` MTK 系——登录流量绝不出走移动数据）、link-local/回环，wlan0 优先。`check_campus_status` 形状对齐桌面 `network_cmd.rs`，`currentSsid` 恒空（无 netsh）。
- **配置管理（config_state.rs）**: `Settings` 31 字段 camelCase 契约对齐桌面 `Config` 可适用子集（含 `update_source` 更新渠道、`campus_check_start_minutes` 检测静默期门控（分钟数，0=禁用，默认 460=07:40 与桌面同值）、`config_schema_version`）。密码落盘形态 `EncodedSettings`：**密码字段与密文分离**（`passwordCipher`/`selfPasswordCipher`），Settings 内密码恒空——加密失败置空而非让配置不可用，解密失败（密钥变更/篡改）同样置空继续加载。`CryptoBridge` 抽象：真机走 keystore 插件，host 测试注入可逆假桥（base64），密码路径可测。出站一律 `masked_for_display`（非空→`***`，对齐桌面 `Config::masked_for_display` 唯一出口语义）；`resolve_password_field` 空/MASK 回退已存值 + `clear` 标志显式清除（与桌面 save_config 同构）。写入 tmp + rename 原子落盘。schema v0→v1 一次性迁移：旧默认后台检测间隔 15s 升 60s（稳态功耗），迁移落盘后用户主动设回不再覆盖。
- **监控状态机（monitor_loop.rs）**: `MonitorState` 全原子字段（running/check_count/consecutive_failures/reconnect_count/was_online/desired_interval_ms/logout_protected_until_ms）。tick 流程 `run_check_once` 五步：⓪**检测静默期门控**（对齐桌面 `background_check`：当前时间早于 `campus_check_start_minutes`（0=禁用）整拍跳过，防非在校时段反复探测与误报掉线通知；在线状态保持上一拍记忆）①校园网判定（顺带刷新源 IP 缓存）②Portal 探测——**专用短命线程绑小核**执行（60s 一拍的稳态周期任务，不占共享 worker 池拖累登录等前台任务；绑核失败静默回落内核调度）③**三态消费**：仅"确定判定"（`error_kind=None`）才翻转在线状态，Unknown（已在线页面特征失配）/Failed（超时）不构成可信离线证据，保持上一拍记忆（否则 Portal GBK 页面特征间歇失配即误判掉线）④自动重登判定：纯函数 `should_attempt_login`（在线/掉线/非校园网/重连上限/cooldown）+ 调用侧两道闸——**注销保护期 60s**（手动注销后不自动登回）+ **连续失败熔断 5 次**（凭据错误无限重试耗流量）⑤emit `background-check-result`（字段对齐桌面可适用子集）+ 更新常驻通知文案。间隔热更新：`start_background_check` 刷新 `desired_interval_ms`，循环体逐 tick 对比重建计时器（改间隔立即生效）。`run_startup_tasks` 启动恢复（对齐桌面 watcher::run_startup_tasks）：500ms 就绪窗口后读配置，三条启动链**并行 spawn**（后台检测/质量首测或定时测试/启动自动登录——冷启动登录不被 12 域名质量首测压尾）+ 24h 更新检查循环；启动探测 `probe_with_retry` 未确认校园网时 3s 后重试一次（开机自启 DHCP 未就绪场景）。**⚠️ 裸线程必须 enter runtime context**：绑小核的 `portal-probe` 是裸线程，无 Tokio thread-local context，其内调用的同步网络函数（`check_portal_full` → `block_on_sync` → reqwest 超时计时器 → `Handle::current()`）会 panic "there is no reactor running"。**仅靠 `block_on_sync` 的 Err 兜底分支（自持 Runtime）实测兜不住**；正确做法是闭包入口 `Handle::enter()`（桌面 `commands/login.rs` 的 scope 裸线程同款），并把闭包体包进 `catch_unwind` 把 panic 转 Err（否则调用侧只看到"线程提前退出"）。桌面侧无此问题——Portal 探测全在 `spawn_blocking` 内，唯一 scope 裸线程已显式 enter。
- **生物识别验证门（identity_gate.rs + self_service_cmds.rs）**: 前端 BiometricPrompt 认证成功后调 `verify_biometric_identity` 写 TTL 时间戳（600s，`AtomicU64`，时钟回拨视为过期）。`ensure_identity_gate` 语义同构桌面：`selfHelloEnabled` 关闭放行、TTL 内放行否则拦截；**改状态命令设门（bind_operator/self_offline_session），查询类不设门，reveal_operator_credential 无论开关强制验证**。协议六命令全部包装桌面 `campus_login_lib::self_service`，`local_addr` 绑定缓存源 IP。
- **多账号（account_cmds.rs）**: 一账号一 JSON（同 EncodedSettings 格式，仅目录不同）、切换合并登录字段、账号名消毒正则（1-32 字符字母/数字/下划线/中文/连字符，防路径穿越）。**`CONFIG_IO_LOCK` tokio 异步锁**：save_config/switch/delete/boot_autostart/通知开关并发读改写的互斥（guard 需跨 await 覆盖 load→merge→save 全序列）。
- **系统信息（system_cmds.rs）**: `get_init_data` 补桌面专属字段空默认（gpuInfo=null/adapters=[] 等）防前端 `useInitialDataLoad` 读 undefined 崩溃。`get_soc_info`：读 `ro.soc.model`（Android 12+ CDD 强制属性）分档 tier 0-3（骁龙 8 系全代 SM8250-SM8750 + 天玑 9300/9400 = 3 旗舰；骁龙 7 系/天玑 8 系 = 2；注意 8s Gen3=SM8635 是中端不能按 SM86 前缀误判），无属性值按大核数（≥1.8GHz）/内存启发式兜底；前端据此调帧率。
- **更新流（update_cmds.rs）**: `check_update` 按 `update_source` 渠道排序 version.json 4 源（ghfast.top/gh-proxy.com/ghproxy.net 镜像与 GitHub 官方互为降级）→ GitHub API 拉 APK 资产与**服务端 digest**（最可信校验源）→ `download_update` 域名白名单（8 个 host，防 SSRF）+ 500MB 上限 + 200ms 节流进度事件 + SHA256 强制比对（API digest 优先，version.json 兜底；不匹配删文件）→ `install_update` 路径 canonicalize 限定应用更新目录内。24h 自动检查循环（桌面同语义），有新版 emit + 系统通知。
- **绑核（cpu_affinity.rs）**: 解析 `/sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq` 按频率分簇，最低频簇=小核（A55 类）；**频率差 <30% 视为同构不绑定**；`libc::sched_setaffinity` 绑定。所有失败静默返回 false——只影响功耗不影响正确性。

#### 4.16.3 手写 Tauri 插件 — `android/plugins/` (Rust 壳 + Kotlin 实现)

| 插件 | 组成 | 要点 |
|------|------|------|
| `keystore` | KeystorePlugin.kt (90 行) | AndroidKeyStore **AES-256-GCM**，key alias `campus_login_master`（不存在则生成，硬件隔离），IV(12B)+密文 Base64 编解码；密钥按包名隔离——**改包名=密码密文作废**（用户卸载重装） |
| `foreground-service` | ForegroundService.kt (166 行) + MonitorServicePlugin.kt (153 行) + BootReceiver | 见下 |
| `network-bind` | NetworkBindPlugin.kt (~250 行) | 把进程网络绑定到 WLAN（`ConnectivityManager.bindProcessToNetwork`）——登录流量物理上只走 WiFi（fwmark 进程级，探测 TcpStream/HTTP/DNS 全覆盖）；`unbind` 恢复系统默认路由。**双路径**：① 快速路径 `allNetworks` **优先取有 `NET_CAPABILITY_INTERNET` 的 WiFi、没有再退到任意 WiFi**——校园网认证前是 captive portal，系统可能不给 INTERNET/VALIDATED，用"有互联网"筛选会把唯一可用的 WiFi 筛掉；② 失败回退 `requestNetwork`（3s 超时，`onAvailable` 内绑定），**显式 `removeCapability(NET_CAPABILITY_INTERNET)`**——AOSP WifiNetworkFactory 对含该能力的 WiFi 请求直接拒绝（"cannot contain NET_CAPABILITY_INTERNET"）。`allNetworks + bindProcessToNetwork` 在 Android 12+ 与部分 OEM 上会返回 false（issuetracker#249023377、WiFiFlutter#296）——真机实测该组合失败、回退 `requestNetwork` 成功。**权限**：`ACCESS_NETWORK_STATE`/`ACCESS_WIFI_STATE` + **`CHANGE_NETWORK_STATE`（`requestNetwork` 必需——缺失时回退路径直接抛 SecurityException，绑定全程失败）**。返回 `{bound, path, reason}`：成功时 reason 为能力摘要（net/nonet+val/unval+cp）、path 为 `allNetworks`/`requestNetwork`/`already_bound`（**同一网络重复绑定**时回 `already_bound`——启动瞬间多条链路并发调用，Rust 侧据此**跳过清连接池**，避免首批请求反复重建连接；`unbind` 会清空该记忆），失败时为 `allNetworks[<直绑失败原因>] requestNetwork[<异常类名:消息>]`（**两条原因分开呈现**——合并拼接会把权限问题伪装成 VPN 问题）。**调用时机（`ensure_wifi_bound`）**：启动自动登录、每拍探测/掉线重登、手动登录 `do_login`、手动探测 `check_portal_status`、注销 `do_logout` 前均先绑定；绑定成功后清空 HTTP 客户端池（`network::client::clear_client_pool`，fwmark 只在 socket 创建时生效），并顺带调 `acceptWifiNetwork`。日志走 `log_info!/log_warn!` 落盘 + `eprintln!` 进 logcat（release 包无 root 时的唯一可见通道）。**`acceptWifiNetwork`（让系统接受无互联网 WiFi）三路径**：`already_validated`（网络已验证，无需动作）→ 反射 `setAcceptUnvalidated`（**真机 `hiddenApi=false`：Android 9+ hidden API 名单拦截 + 需 CONNECTIVITY_INTERNAL，普通应用不可达**）→ 写 `Settings.Global`（`captive_portal_mode=0` + `network_avoid_bad_wifi=0`；由 `WRITE_SECURE_SETTINGS` 保护，普通签名应用**无法通过用户授权获得**，仅 root/Shizuku/系统预装可见）。**结论：普通签名应用两条底层路径都走不通，但实测也不需要**——真机：应用绑定该 WiFi（未验证状态也能经 `requestNetwork` 绑到）→ 完成 Portal 认证 → 系统探测到连通后把网络转 `VALIDATED` → 系统"登录到 WLAN"提示消失、默认路由落到 WiFi。即**"系统默认直连"是认证成功的自然结果**，不是需要修改的系统设置；`bindProcessToNetwork` 才是关键手段。注意 `Settings.System.canWrite()` 检查的是 System 表的 `WRITE_SETTINGS`，与写 Global 无关——直接试写并由系统给出真实异常 |

**foreground-service 插件**（保活三件套 + 自启 + 装 APK）:

- `ForegroundService`: 只做保活（监控逻辑在 Rust 侧循环，服务被回收=进程死亡=循环终止，状态天然一致）。START_STICKY；前台通知走**标准安卓协议**——ongoing + Chronometer 计时（起点固定为服务启动时刻，每拍更新文案不动计时）+ CATEGORY_SERVICE，即 Android 16 promoted ongoing 特征，ColorOS 流体云/HyperOS 原生通道按 Live Updates 自动识别（用户侧需开"实时通知提升"类权限），不做厂商私有 extras；FGS type `SPECIAL_USE`（API 34+）。**WifiLock**（`WIFI_MODE_FULL_HIGH_PERF`，抑制 WiFi 省电断流）+ **网络事件驱动短持 WakeLock**：NetworkCallback 的 onAvailable/onLost/onCapabilitiesChanged 时短持 3s PARTIAL_WAKE_LOCK nudge，保证 Rust 轮询 tick 在 CPU 唤醒窗口内立即执行，其余时间不持锁系统可正常 suspend（长持 WakeLock 的省电反模式被显式避开）。
- `MonitorServicePlugin`: `startMonitor`/`stopMonitor`（停止走 ACTION_STOP 意图让服务自杀，避免 stopService 与 startForeground 竞态）/`updateNotification`（与服务侧共用同一构建函数保证通知形态一致）/`installApk`（**FileProvider content:// URI** 交系统包安装器——应用私有目录 file:// 对 Android 7+ 必失败 FileUriExposedException；文件名只取末段防路径逃逸）/`setBootAutostart`（`setComponentEnabledSetting` 启停 BootReceiver 组件 + SharedPreferences 记忆）/`isBootAutostartEnabled`。
- `BootReceiver`: ACTION_BOOT_COMPLETED 时（且用户开启过自启）拉起前台服务 + **best-effort 拉起 MainActivity**——Rust 监控循环/自动登录/定时测试由 tauri Activity 的 setup→run_startup_tasks 驱动，FGS 只是通知壳；Android 10+ 后台启 Activity 受 ROM 限制（MIUI 需"后台弹出界面"权限），被拦时常驻通知仍在，用户点开 app 一次即恢复完整链路。

---

## 附录 B：前端模块详解 (React/TypeScript)

> **架构说明**: 前端采用业务域分目录架构，每个业务域目录包含面板组件、逻辑 Hook、类型定义和模块导出。类型定义分散在各业务域的 `types.ts` 中，而非集中在一个 `types/index.ts` 文件。

### 5.1 状态管理架构 — 领域 store 拆分

> 所有 store 基于 zustand ^5，持久化走 `safeStorage`（内存降级封装，避免隐私模式下不可用）；`useAppStore.ts` 仅 3 行 re-export 兼容壳。

**领域 store 一览**：

| Store | 文件 | 职责 | 关键 state | 关键 action |
|-------|------|------|-----------|-------------|
| `useConfigStore` | `useConfigStore.ts` (~220行) | 配置/账号/语言 + 防抖保存 + 密码掩码 + 脏字段跟踪 + configLoaded 启动信号 | `config`/`passwordSaved`/`selfPasswordSaved`/`accounts`/`activeAccount`/`language`/`api`/`dirtyFields` | `updateConfig`(500ms防抖)/`updateConfigLocal`/`syncPasswordSaved`/`syncSelfPasswordSaved`/`saveConfigDirect`(非空 selfPassword 成功置位 selfPasswordSaved)/`setLanguage`/`saveConfigInFlight`(in-flight 保存等待)/`mergeConfigFromBackend`/`clearDirtyFields` |
| `useAuthStore` | `useAuthStore.ts` (300行) | 登录/注销/在线检测/后台状态 + 登录后 60s 手动质量探测节流 | `isLoggingIn`/`isLoggingOut`/`status`/`bgStatus` | `doLogin`/`doLogout`/`checkOnline`/`setStatus`/`setBgStatus` |
| `useAdapterStore` | `useAdapterStore.ts` (74行) | 适配器列表/详情/面板 | `adapters`/`disabledAdapters`/`adapterDetails`/`isRefreshingAdapters`/`activePanel` | `refreshAdapters`/`setAdapters`/`setActivePanel` (模块级 `refreshAdapterData` 公共函数) |
| `useQualityStore` | `useQualityStore.ts` (90行) | 网络质量/DNS DoH/更新/GPU | `networkQuality`/`dnsDohStatus`/`dnsChecking`/`isRefreshingQuality`/`updateAvailable`/`latestVersion`/`releaseNotes`/`gpuInfo`/`refreshRate` | `refreshQuality`/`setNetworkQuality`/`setDnsDohStatus`/`setUpdateAvailable`/`setGpuInfo` |
| `useThemeStore` | `useThemeStore.ts` (86行) | 主题/亮暗/自定义色 + DOM 副作用 | `themeName`/`isLightMode`/`customThemeColor` | `setThemeName`/`setIsLightMode`/`initTheme`/`setCustomThemeColor` |
| `useLogToastStore` | `useLogToastStore.ts` | 日志/Toast（MAX_LOG_ENTRIES=300；**`addLog` 连续重复折叠+×N 计数**：与末条 message+type 相同时累计 `LogEntry.count` 并刷新 time，RightPanel 渲染 ×N 徽标——重复次数是诊断信息，跨条 A-B-A 不折叠；后端文件日志保留原始逐条不折叠，审计优先。Toast 上限 MAX_TOASTS=4，同 title 去重防刷屏） | `logs`/`toasts` | `addLog`/`addToast`/`addToastWithAction`/`removeToast`/`removeToastsByPrefix` |

> **通知单通道规范**（§三-5）：`emit_notification` 只发 Windows 系统通知（用户看不到主窗口即 `!is_visible() || is_minimized()` + `enable_notification` 时；系统通知文案中文硬编码，后端无法感知 UI 语言，为已知边界）；应用内 toast/日志走业务专用事件（auto-login-result / auto-exit-* / campus-exit-* / network-quality-result / login-log / update-available）；store 防护 `MAX_TOASTS=4` + 同 title 去重，窗口非前台普通 toast 不入队（信息由日志兜底），带 action 的 toast 仍入队（承载取消退出入口）；文案统一 `i18next.t('notify.*')`，`enable_notification` 仅控制系统通知。
| `useAppStore` | `useAppStore.ts` (3行) | **兼容壳**，仅 re-export `useAppInit`/`hasPendingConfig`/`flushPendingConfig` | 无 | 无 |

**密码处理** (迁移至 `useConfigStore`)：`password === PASSWORD_MASK` 时两层防护——`updateConfig` 合并挂起配置时若旧挂起有真实密码但新 partial 传 MASK，保留旧挂起真实密码；`flushPendingConfig` 最终合并时若 password 仍是 MASK 则 `delete`，让后端识别 MASK 并保留原密码。

**刷新锁统一模式**：各领域 store 均采用模块级 `_xxxLockFlag` 防抖锁模式：
- `useConfigStore`：`saveConfigTimer` + `saveConfigPending`（500ms 防抖保存）+ `dirtyFields`/`dirtyFailureCounts`（后端回写跳过脏字段，连续 3 次失败放弃脏标记）
- `useAuthStore`：`_checkOnlineLockFlag` + `checkOnlineEpoch`（promise settle 时 finally 立即释放，非 setTimeout 延迟释放）
- `useAdapterStore`：`_adapterLockFlag`
- `useQualityStore`：`_qualityLockFlag`

**主题 DOM 副作用** (迁移至 `useThemeStore`)：store 模块底部 `subscribe` 监听 `isLightMode`/`themeName`/`customThemeColor` 变化，toggle `dark` class、添加 `theme-${name}` 类、计算 CSS 变量 `--primary`/`--ring`/`--accent`。

> 注：`activePanel` 归 `useAdapterStore` 管理（历史归位，语义上与适配器关联较弱但实际如此）。`i18next.t()` 在 action 函数体内调用（非 Store 创建时），避免初始化时序问题。

### 5.2 IPC 封装 — `hooks/tauriApi.ts`

**导出**：`tauriApi: TauriApi`（默认对象，56 个 invoke 方法 + 15 个事件监听器工厂）、`tauriApiWithRetry: TauriApi`（对 `saveConfig` 包一层 `withRetry`）。

**事件监听器** (15 个，均通过 `createEventListener<T>(eventName)` 工厂创建，返回取消函数):

| 监听器方法 | 事件名 |
|------------|--------|
| `onBackgroundCheckResult` | `background-check-result` |
| `onAutoLoginResult` | `auto-login-result` |
| `onAdaptersChanged` | `adapters-changed` |
| `onAdapterDetailsChanged` | `adapter-details-changed` |
| `onDisabledAdaptersChanged` | `disabled-adapters-changed` |
| `onAdapterDisabledWarning` | `adapter-disabled-warning` |
| `onLoginLog` | `login-log` |
| `onNetworkQualityResult` | `network-quality-result` |
| `onAutoExitCountdown` / `onAutoExitCancelled` | `auto-exit-countdown` / `auto-exit-cancelled` |
| `onCampusExitCountdown` / `onCampusExitCancelled` | `campus-exit-countdown` / `campus-exit-cancelled` |
| `onUpdateAvailable` | `update-available` |
| `onDownloadProgress` | `update-download-progress` |
| `onConfigChanged` | `config-changed` |

**`createEventListener` 竞态处理**：闭包维护 `cancelled`/`unlisten` 双状态，处理"订阅尚未完成时即被取消"的竞态；取消函数若 `unlisten` 已就绪则直接调用，否则挂到 `listenPromise.then(fn => fn?.())` 延后清理。

**API 清单** (`TauriApi` interface 定义 56 个 API，按领域分组):

| 领域 | API |
|------|-----|
| 配置 | `getConfig` / `saveConfig(cfg, clearPassword?, clearSelfPassword?)` / `getInitData` |
| 适配器 | `getAdapters(force?)` / `getDisabledAdapters` / `enableAdapter` / `getAdapterDetails` |
| Portal/校园网 | `checkPortalStatus(adapterIp)` / `checkCampusStatus` |
| 登录 | `doLogin(adapterName?)` / `doLogout(adapterName?)` |
| 窗口 | `minimizeWindow` / `closeWindow` / `showWindow` |
| 账号 | `listAccounts` / `switchAccount` / `saveCurrentAsAccount` / `deleteAccount` / `getActiveAccount` |
| 后台检测 | `startBackgroundCheck` / `stopBackgroundCheck` / `triggerBackgroundCheck` / `getBackgroundStatus` |
| DHCP | `dhcpRenewAll` / `dhcpReleaseRenew` / `dhcpReleaseRenewAdapter` |
| 网络质量 | `checkNetworkQuality` / `startLatencyTest` / `stopLatencyTest` |
| 系统集成 | `openExternal` / `getAutoLaunch` / `setAutoLaunch` / `getNotificationEnabled` / `setNotificationEnabled` / `cancelAutoExit` |
| 日志/调试 | `getLogs(lines?)` / `clearLogs` / `getDebugMode` / `setDebugMode` / `getLogRetentionDays` / `setLogRetentionDays` |
| 更新 | `checkUpdate` / `downloadUpdate` / `installUpdate` / `getMirrorUrls` |
| DNS DoH | `checkDnsDohStatus` / `setupDnsDoh` |
| 自助服务/身份验证 | `bindOperator` / `queryBindStatus` / `verifyWindowsIdentity(consentMessage?)` / `revealOperatorCredential` / `querySelfDashboard` / `querySelfOnlineLog` / `selfOfflineSession` |
| 其它 | `renderHeartbeat` / `getGpuInfo` |

**重试机制** (`tauriApiWithRetry`)：`isRetryableError` 判断消息含 `timeout`/`network`/`fetch`/`connection` 之一；`withRetry(fn, maxRetries=2, baseDelay=500)` 指数退避 `baseDelay * 2^attempt + random(0..200)` ms，最多重试 2 次（共 3 次尝试）。**仅对 `saveConfig` 一个命令包装**——保存是一次性关键操作无其他兜底；checkPortalStatus 由 checkOnline 高频调用且后台检测循环本身周期性重试，checkNetworkQuality 有后端 latency loop 事件流兜底，包重试反而放大高频调用流量。

**openExternal 5 层安全逻辑**：①协议白名单(http/https) ②URL长度上限(2048) ③`new URL()` 解析校验 ④调用后端 `open_external` 二次校验 ⑤后端失败降级到 `@tauri-apps/plugin-shell` 的 `open`。所有失败路径仅 DEV 模式 `console.warn`，不抛错。

**类型定义**: 分散在各业务域的 `types.ts` 文件中（account/auth/monitor/network/settings/shared），合计 40+ 个类型/接口定义。

### 5.3 初始化编排 — `hooks/useAppInit.ts` (11 行)

**职责**：纯编排 hook，依次调用 4 个子 hook，无自身状态：

```ts
export function useAppInit() {
  useEventListeners()   // Tauri 事件监听统一注册
  useInitialDataLoad()  // getInitData 拉取并 bootstrap stores
  useHeartbeat()        // 5s 渲染心跳
  useGlobalShortcut()   // Ctrl+Shift+C 取消自动退出
}
```

#### 5.3.1 `useEventListeners.ts` (345 行) — 事件监听统一注册

mount 时注册全部 Tauri 事件监听器与窗口关闭拦截，unmount 时统一清理。注册 14 个事件订阅 + 1 个窗口关闭拦截：

- `getCurrentWindow().onCloseRequested` — 拦截关闭，若有 pending config 先 `flushPendingConfig()`，再 await 其返回的保存（返回值合并本次 flush 新发出的 `saveConfig` 与既有 in-flight 等待，避免 debounce pending 时丢数据；`Promise.race` 2s 上限）后才关闭
- `onBackgroundCheckResult` — 更新 `bgStatus`、记录在线/离线日志（1s 节流 + 5s 在线日志节流）
- `onAdaptersChanged` — 更新 store，500ms 节流（前缘+后缘双重保护）
- `onAdapterDetailsChanged` / `onDisabledAdaptersChanged` / `onAdapterDisabledWarning`
- `onAutoLoginResult` — Toast + 触发 `checkOnline`
- `onLoginLog` — 写入 `useLogToastStore.addLog`
- `onAutoExitCountdown`/`onAutoExitCancelled`/`onCampusExitCountdown`/`onCampusExitCancelled` — 倒计时 Toast
- `onNetworkQualityResult` — 合并到 `useQualityStore.networkQuality`，触发"延迟过高"告警 (`handleQualityBadAlert`)
- `onUpdateAvailable` / `onConfigChanged`（经 `mergeConfigFromBackend` 合并——跳过本地脏字段，不整体覆盖）

**关键策略**：监听器先于数据获取注册（在 `useInitialDataLoad` 之前），避免遗漏初始化期间事件；`mountedRef` 防止 unmount 后写状态；系统通知由后端统一发送；网络质量事件无防抖，增量推送可立即更新 UI。

#### 5.3.2 `useInitialDataLoad.ts` (157 行) — 初始数据 bootstrap

mount 时调 `api.getInitData()` 拉取全量数据，按流水线 bootstrap 所有领域 store：

1. 失败兜底：`api.showWindow()` + 重置 `config = DEFAULT_CONFIG`
2. 合并 `cfg = { ...DEFAULT_CONFIG, ...initData.config }`
3. 根据 `cfg.password === PASSWORD_MASK` 调 `syncPasswordSaved(true/false)`；`cfg.selfPassword === PASSWORD_MASK` 调 `syncSelfPasswordSaved(true/false)`（重启后恢复自助密码"已保存"显示）
4. `useConfigStore.setState({ config: cfg })` → `useThemeStore.initTheme(cfg)`
5. 从 storage 读 `campus-active-panel`，决定 `defaultPanel` 与窗口显示（`isAutoStart && hiddenStart` 则不显示）
6. 写入 adapters / bgStatus / adapterDetails / 异步拉取 disabledAdapters / accounts / activeAccount
7. 调 `useAuthStore.checkOnline(cfg, adps)`
8. GPU 信息：initData.gpuInfo 优先 + `correctGpuInfo` 校正；否则异步 `api.getGpuInfo` + 校正
9. 写入 `refreshRate`
10. 异步 `checkDnsDohStatus` + 检查推荐 DNS + 是否启用 DoH，缺失则告警日志
11. **网络质量检测由后端 latency loop 统一管理**，前端不再主动调用 `checkNetworkQuality`

**幂等保护**：`mountedRef` 防止 unmount 后写状态（StrictMode 二次 setup 时恢复 `mountedRef.current = true`，不短路初始化）；catch 块中 `showWindow` 不受 `mountedRef` 影响（应用级操作）。

**`configLoaded` 确定性信号**：成功路径与失败降级路径都置位 `useConfigStore.configLoaded`（157/165 行）——自助服务面板的自动验证+自动刷新严格等待该信号，配置加载完成且凭据就绪才弹 Hello 并拉取，消除启动加载窗口期的时序竞态；窗口期内面板卡提示"配置加载中..."。后端配套：setup 启动时 gpu-warmup 后台线程预热 GPU/刷新率检测（OnceLock 缓存），`get_init_data` 读缓存即返回，前端更早拿到配置。

#### 5.3.3 `useHeartbeat.ts` (19 行) — 渲染心跳

mount 时立即调一次 `api.renderHeartbeat()`，`setInterval` 每 5000ms 调一次；监听 `visibilitychange`，`document.hidden` 时暂停心跳；unmount 清理 interval 与 listener。

#### 5.3.4 `useGlobalShortcut.ts` (16 行) — 全局快捷键

监听 `window.keydown`，命中 `Ctrl+Shift+C` 时 `e.preventDefault()` 并调用 `api.cancelAutoExit?.()`（取消自动退出/校园网退出倒计时）。

#### 5.3.5 `useGpuCorrection.ts` (75 行) — WebGL GPU 校正

通过 WebGL `WEBGL_debug_renderer_info` 扩展读取真实 GPU，校正后端 WMI 报告：
- `getWebGlRenderer()` — `UNMASKED_VENDOR_WEBGL`/`UNMASKED_RENDERER_WEBGL`
- `parseWebGlGpu(renderer)` — 正则匹配 `ANGLE(Vendor, Model)` 格式
- `classifyTierFromWebGl(vendor, model)` — 返回 `GpuTier`：NVIDIA→discrete，Intel Arc→discrete/Iris Xe→mid-igpu/UHD→low-igpu，AMD RX/Pro→discrete/780m/880m→high-igpu 等
- `correctGpuInfoWithWebGl(wmiInfo)` — WMI 与 WebGL vendor 不一致且 WMI 为独显时，返回 WebGL 解析的 GPU

> **崩溃恢复说明**: `setupCrashRecovery`（含 GPU/WebGL/SharedArrayBuffer 错误重载、5秒心跳 GPU 崩溃检测、页面可见性暂停/恢复 GSAP globalTimeline）定义在 `main.tsx`，见 5.11

### 5.4 业务域模块

#### 5.4.1 认证模块 — `auth/`

| 文件 | 说明 |
|------|------|
| `DashboardPanel.tsx` | 总览面板，卡片可拖拽排序（framer-motion Reorder.Group），5 张自定义卡（`ALL_CARDS`: QuickActionsCard/AccountManageCard/SelfOnlineCard/SelfLogCard/NetworkQualityCard，质量总开关关闭时隐藏网络质量卡），布局持久化到 safeStorage。**自助两卡**（在线信息/上网记录）共用 hook `useSelfCardReveal`（凭据判断 + 掩码/验证切换）与 `useSelfCardFetch`（自动查询骨架：按学号只查一次、引用变化不重查、错误卡内重试 + toast），自动查询走 `querySelfDashboard`/`querySelfOnlineLog`（密码传空串由后端回退已保存值，仅回传非敏感概览）；关键信息（IP/登录时间/时长/流量明细）未验证时圆点掩码，点眼睛经 `useHelloGate` 验证后显示、再点切回不再验证；每卡显隐独立，与账号页绑定卡共用门生命周期；无凭据显示引导提示，`selfHelloEnabled` 关闭时眼睛直接放行。总览滚动末尾渲染低透明度背景看板娘 `mascot-bg-laptop`（文档流末尾 img 而非 absolute，随滚动被内容自然遮挡） |
| `DashboardPanel.selfCards.test.tsx` | 总览自助两卡单测：自动查询+掩码、验证后明细/切回不再验证、验证失败保持掩码、无凭据不查询 |
| `AboutDialog.tsx` | 关于对话框，双栏布局(应用信息+更新仪表盘)，镜像源选择，下载状态机(idle→selecting→downloading→done/error)，Release Notes渲染。**一键下载前 `ensureFullUpdateInfo` 确保 updateInfo 完整**（系统通知缓存路径构造的对象缺 `sha256Checksum`/`assets`，原样使用会下载 404 且安装被后端拒绝）；安装失败在 done 态显示错误文案。**赞助入口**：左栏"赞助支持"按钮点击后右栏原地切换为赞助内嵌页（不关对话框；关闭对话框时重置回仪表盘）。**固定亮色皮肤**：对话框挂 `.force-light-dialog`（容器级重定义主题变量 + 显式 `color` 继承），修复暗色模式白底近白文字不可读；浅色模式视觉无变化 |
| `useAuth.ts` | 认证逻辑 Hook |
| `types.ts` | 认证类型定义 (PortalStatusResult, CommandResult, LoginResult) |
| `index.ts` | 模块导出 |

#### 5.4.2 账号模块 — `account/`

| 文件 | 说明 |
|------|------|
| `AccountPanel.tsx` | 账号管理面板，两列网格等高布局(左列：登录信息卡+自动化设置开关卡(`flex-1` 撑满与右列底部对齐)；右列：**绑定运营商账号**卡输入框垂直排布+绑定状态区(query_bind_status 查询：手机号掩码前三后二/查看密码走 Hello 验证后 reveal_operator_credential 临时显示明文，与绑定/查询共用 `useHelloGate` 门)；两个密码框旁均有**"清除密码"入口**（登录密码 `clearPassword`、自助服务密码 `clearSelfPassword`，仅已保存时显示，`onMouseDown preventDefault` 防夺焦；清除自助密码顺带清空本地草稿防 blur 兜底存回)；下方账号管理卡全宽) |
| `selfServiceState.ts` | 绑定卡与自助服务面板**跨面板共享层**（zustand，非持久化）：`useSelfCredStore`（学号+自助服务密码共用输入，切面板不丢失，退出应用即清空）、`useHelloGate`（绑定/明文查看门：时间戳 `helloGateVerifiedAt` TTL 570s、`gateFresh` 含 `elapsed >= 0` 回拨守卫、`ignoreToggle` 选项使明文查看无视总开关强制验证）、`useSelfServiceVerify`（自助面板会话门 `selfSessionVerifiedAt` 同款 TTL，`resetSelfSessionGate()` 面板卸载重置） |
| `SelfServicePanel.tsx` | "自助服务"独立面板(协议见 §4.5.4.3；`PanelName`/`PANEL_TITLES`/NAV_ITEMS/设置页默认面板选项四处接入)：**在线信息**表(操作列注销→ConfirmDialog→self_offline_session→成功后本地移除该行，不自动重拉避免整会话重登开销) + **近期上网记录**卡(日期范围筛选默认今天 + 汇总 8 格 + 12 列明细表横滚；**金额列 `fmtMoney` 保留原始精度**（parseInt 截断会把 0.50 显示成 0）；空值/非数值显示 `-`)；凭据区在"在线信息"卡内顶部(与绑定卡共用 `useSelfCredStore`，学号默认取 config.user，密码 blur 经 `saveConfigDirect` DPAPI 落盘)，密码框旁"清除密码"入口；未填凭据时按钮禁用+提示 |
| `AccountPanel.helloGate.test.tsx` | 绑定门行为单测（首次即验证/验证失败不执行且下次仍需验证/查询验证后查看明文不再二次验证） |
| `SelfServicePanel.test.tsx` | 自助面板单测（每次进面板验证/会话内操作共用/开关关闭放行/凭据输入与格式化） |
| `useAccount.ts` | 账号逻辑 Hook |
| `types.ts` | 账号类型定义 (SwitchAccountResult, DeleteAccountResult, SaveAccountResult) |
| `index.ts` | 模块导出 |

#### 5.4.3 监控模块 — `monitor/`

| 文件 | 说明 |
|------|------|
| `MonitorPanel.tsx` | 监控面板，2卡片(网络状态检测含适配器在线状态分区/验证设置)+校园网验证配置 |
| `QualityPanel.tsx` | 网络质量面板，3卡片(质量概览+LatencyPair/定时测试/测试详情5分类Tab)，质量差时红色发光 |
| `SpeedTestPanel.tsx` | 速度测试面板，8个预设网站分3类(综合/教育网/轻量)，PanelName包含'speedtest' |
| `StatusBar.tsx` | 状态栏，在线(绿)/离线(红)/加载(蓝)指示器+校园网状态Tooltip+质量胶囊+刷新延迟+自助服务+Portal入口 |
| `LatencyComponents.tsx` | 延迟组件，SignalBars(5段信号柱+GSAP发光点)+LatencyPair(内网/外网延迟双栏) |
| `LatencyTimeline.tsx` | 延迟时间线，UDP/DNS/TCP/TLS/TTFB/内容/网络各阶段彩色分段 |
| `NetworkQualityCapsule.tsx` | 网络质量胶囊，悬浮弹出详情面板(createPortal)，延迟变化动画(变差heartbeat/变好flash) |
| `useMonitor.ts` | 监控逻辑 Hook |
| `types.ts` | 监控类型定义 (AdapterOnlineStatus, BackgroundStatus, NetworkQuality 等) |
| `index.ts` | 模块导出 |

#### 5.4.4 网络模块 — `network/`

| 文件 | 说明 |
|------|------|
| `NetworkPanel.tsx` | 3个卡片（网络适配器列表含状态四分类/适配器设置/DNS优化），适配器启用/单适配器获取新IP |
| `useNetwork.ts` | 网络逻辑 Hook；模块级导出 `normalizeDhcpResults`（单条/批量结果归一化）+ `announceDhcpResults`（成功/跳过/失败三类 i18n toast，与 NetworkPanel "获取新IP"共用同一实现） |
| `adapters.ts` | `resolveAdapterNames(adapters, config)` 前端适配器解析，与后端 `resolve_adapter_names` 同源规则 |
| `adapters.test.ts` | resolveAdapterNames 单测（锁同源行为） |
| `constants.ts` | 网络常量 (QUALITY_CONFIG: 9级质量配置含labelKey/color/bg/border/borderBg/icon/hex/activeBars/glow，定义于此) |
| `types.ts` | 网络类型定义 (AdapterStatus四分类: disabled/disconnected/enabledNoIp/connected, Adapter, DnsDohStatus, DnsServerInfo 等 11 个类型) |
| `index.ts` | 模块导出 |

**NetworkPanel.tsx**:
- **3个卡片**: 网络适配器列表/适配器设置/DNS优化
- **适配器启用**: 新增启用适配器功能
- **单适配器获取新IP**: 新增单适配器 DHCP 释放续租
- **DNS 优化卡片**: 检测当前 DNS/DoH 配置状态
- **一键优化按钮**: 始终显示，设置阿里 DNS + 腾讯 DNS + 启用 DoH
- DNS 列表使用 `dns.address` 作为稳定 key
- **适配器列表**: 显示适配器名称/IP/类型/状态(AdapterStatus四分类)，支持启用/获取新IP

#### 5.4.5 设置模块 — `settings/`

| 文件 | 说明 |
|------|------|
| `SettingsPanel.tsx` | 设置面板，页面顺序：外观 → **两列区**(md 断点起：左"启动设置"卡；右列"系统通知"/"安全设置"/"新手指引"三张独立卡垂直排列，grid stretch 拉伸至与左列等高) → "网络质量检测"大卡整宽收尾；"启动时默认显示"用 **Radix Select 下拉框**（9 选项压缩为 1 行，"记住上次"用哨兵值映射——Radix Item 不接受空串 value；质量检测关闭时下拉中隐藏 quality 项）；**"安全设置"卡两开关**：`selfHelloEnabled`（默认开，Hello 验证总门）与 `selfReverifyEachAction`（默认关，每次操作验证严格模式）——**关闭方向必须先过 Hello 验证**（`handleSecurityDisable`，防绕过界面关闭保护），开启方向免验，总开关关闭时二次验证开关禁用；+7种主题+12色预设+取色器+亮暗模式 |
| `ThemeDialog.tsx` | 主题对话框，2列布局+亮暗模式切换 |
| `OnboardingWizard.tsx` | 5步引导向导(欢迎→绑定运营商账号(可跳过)→账号→适配器→完成)，Framer Motion滑动转场，含语言切换，完成后自动登录；绑定步骤调 `bind_operator` 命令完成自助系统登录+运营商绑定，并**接入 `useHelloGate` 验证门**（不先行验证会被后端 `ensure_identity_gate` 拒绝） |
| `useSettings.ts` | 设置逻辑 Hook |
| `constants.ts` | 设置常量 (DEFAULT_CONFIG/ISP_OPTIONS(4种)/THEME_OPTIONS(7种)/VALID_THEMES/DEFAULT_PANEL_OPTIONS) |
| `types.ts` | 设置类型定义 (Config(39字段含logRetentionDays/configVersion，排除后业务字段37个), AutoLaunchResult, InitData) |
| `index.ts` | 模块导出 |

### 5.5 共享组件 — `shared/`

| 文件 | 说明 |
|------|------|
| `LogPanel.tsx` | 日志面板 (级别过滤/模块过滤/关键词搜索/行数选择/保留天数/Debug模式/GSAP清空动画/自动滚动/5秒刷新) |
| `ErrorBoundary.tsx` | React Class Component 错误边界，显示错误信息+重新加载按钮 |
| `ConfirmDialog.tsx` | 确认对话框 |
| `FluidBackground.tsx` | 简单背景层，使用CSS变量 `--surface-main` |
| `AnimatedNumber.tsx` | 动画数字，GSAP quickTo驱动，支持unit/decimals/duration，economy档禁用scale弹跳 |
| `RefreshButton.tsx` | 刷新按钮，旋转动画+完成时shake效果+showCheck绿色对勾动画 |
| `SegmentTabs.tsx` | 分段Tab，Framer Motion layoutId滑块动画+TabContent(AnimatePresence) |
| `ToastContainer.tsx` | Toast容器，4种类型(info/success/error/warning)，economy档简单transition替代spring，支持action按钮。**全部 toast 统一渲染看板娘头像**（裸 img 非 MascotFigure，避免 tailwind 尺寸类冲突）：`mascot` 字段（`portrait|celebrate|offline|alert|update`）显式传参优先，缺省按 type 映射（info→portrait/success→celebrate/error→offline/warning→alert，见 `TOAST_MASCOTS`）；特殊变体经 `useLogToastStore.addToast` 第 5 参传入（如新版本提示传 update） |
| `MascotFigure.tsx` | 看板娘展示组件（双端同构）。`variant` 对应 `/girl/mascot-{portrait,welcome,empty,celebrate,sponsor,offline,alert,busy,update}.webp`（2026-09-12 新增 alert/busy/update 三变体，见 `assets/ui-girl/original/` 归档），`size` 三档（sm/md/lg）；tailwind 尺寸类无法被 className 可靠覆盖，特殊尺寸场景用裸 img。**背景装饰娘 4 张**（`mascot-bg-{laptop,nap,lounge,tea}.png`）不经组件、低透明度裸 img 直引。**桌面端**：App.tsx 布局层两个 fixed 侧边娘（≥1360px 宽视口显示，`top-1/2 -translate-y-1/2` 锚定视口两侧垂直居中、滚动恒定可见，w-36、opacity 0.26/暗色 0.16，right 侧避让 w-72 的 RightPanel；桌面默认窗口 1360×768 使立绘默认即可见（常态左右间隙 24/16px），<1360 视口媒体查询自动隐藏；立绘宽 W、断点 B、默认窗口三者联动约束 `B ≥ 320 + 720 + 2×(W+16)` 且 B 取默认窗口宽）；RightPanel 日志栏的茶娘为**运行日志卡内部底部水印**（absolute bottom-3 低透明度，卡容器 relative overflow-hidden，日志条目少时从空白处露出）。**安卓端**：四面板滚动末尾底部娘保留（窄屏无侧边空间），设置页图加 `pb-72` 撑滚动余量避让固定底栏。**坑：`space-y-*` 容器的子元素 margin-bottom 被 `space-y-4 > * + *` 规则锁死（specificity 更高），间距要用 padding 不用 margin**。素材链：AI 原图归档 `assets/ui-girl/original/` → rembg(isnet-anime) 抠图 → PNG 档案 → 转 WebP（q84 原尺寸，-85%）→ 双端 `public/girl/*.webp`（代码引用 .webp，PNG 不入打包） |
| `SponsorCard.tsx` | 赞助下拉浮层。**非模态**：无遮罩、不抢焦点、不阻塞交互，点击浮层外任意处(window pointerdown capture)或 Esc 即关闭；锚定标题栏赞助按钮下方自然向下展开（fixed，z-[60]，高于 DockNav 菜单低于 toast），自动弹出与手动入口共用此浮层；内嵌微信/支付宝收款码；文案走 i18n sponsor 段 + about.sponsor |
| `types.ts` | 共享类型定义 (UpdateAvailableData, UpdateInfo, DownloadProgress, MirrorSource 等) |
| `ui-types.ts` | UI 类型定义 (StatusState, PanelName(8个: dashboard/account/network/monitor/quality/settings/log/speedtest), ThemeName(7种), LogType, GpuTier, GpuInfo, LogEntry, ToastMessage, AdapterDisabledWarningData, AutoExitCountdownData, SaveConfigResult 等 10 个导出) |
| `ui-constants.ts` | UI 常量 (MAX_LOG_ENTRIES=300/APP_VERSION='2.3.0'/APP_NAME='校园网登录助手'/PASSWORD_MASK='***'/NAV_ITEMS=8个导航项/Z_INDEX 分层常量) |
| `index.ts` | 模块导出 |

### 5.6 布局组件 — `components/layout/`

| 文件 | 说明 |
|------|------|
| `DockNav.tsx` | 适配器选择浮层 + 注销按钮 (无线蓝Wifi/有线绿Cable图标, 300ms延迟关闭/150ms延迟打开)，选择项收敛为主/副适配器（`scopedAdapters`），GSAP 磁吸效果（MAGNETIC_RANGE=80, MAX_SCALE=1.35, MAX_LIFT=-14），economy档禁用磁吸，RAF节流。tooltip 水平居中必须用 Tailwind `-translate-x-1/2`（inline `translateX(-50%)` 会覆盖 class transform 导致上浮动画失效） |
| `RightPanel.tsx` | 右侧面板，运行日志+网络适配器信息(可展开/折叠，显示IP/子网掩码/网关/DHCP/MAC)，空日志时呼吸动画。清空日志 GSAP 动画 stagger 动态封顶（>8条0.05s/>4条0.1s，固定 0.2s/条在日志满 300 条时动画约 60 秒且按钮禁用无法取消），与 LogPanel 同策略 |
| `TitleBar.tsx` | 标题栏，看板娘头像 logo（mascot-portrait 圆形裁剪 w-10）+版本号+更新提示+工具按钮(亮暗/语言/通知/主题/赞助Heart/关于/最小化/最大化/关闭)，双击最大化，拖拽移动窗口 |

### 5.7 延迟颜色 — `lib/latency.ts`

- `QUALITY_CONFIG` 新增显式 `borderBg` 字段，确保 Tailwind JIT 可扫描
- `getLatencyColor()` 使用 `cfg.borderBg` 替代动态字符串替换
- `getLatencyLevel()` — 延迟等级计算
- `extractGatewayLatency()` — 提取网关延迟
- `extractExternalLatency()` — 提取外网延迟

### 5.8 国际化 — i18n/

- 基于 react-i18next + i18next-browser-languagedetector
- 翻译文件按 JSON 顶级 key 分组（单一 "translation" namespace，共24个）：nav, titlebar, dock, auth, account, settings, monitor, network, quality, speedtest, statusbar, dashboard, log, rightPanel, about, common, onboarding, confirmDialog, isp, panel, themeDialog, crashRecovery, sponsor, notify
- 非组件中使用 `import i18next from 'i18next'` + `i18next.t()` 而非 useTranslation hook
- 常量文件（NAV_ITEMS、ISP_OPTIONS、THEME_OPTIONS、QUALITY_CONFIG）添加 labelKey 字段，运行时通过 t(labelKey) 翻译
- 默认语言中文，i18n 仍使用 `localStorage`（非 safeStorage），仅 `useAppStore.setLanguage` 使用 `safeStorage`

### 5.9 其他 Hooks — `hooks/`

| Hook | 说明 |
|------|------|
| `useAnimationProfile.ts` | 动画配置，AnimationTier(high/standard/economy) + AnimationProfile 18字段，economy档禁用willChangeOrbs/enableTilt/startupBoost |
| `useAsyncLock.ts` | 异步锁，`useAsyncLock<T>(fn, cooldownMs=1500)` 防止并发调用 |
| `useBreatheAnimation.ts` | 呼吸动画，GSAP yoyo循环，支持opacity/scale/rotation，空闲时暂停 |
| `useGlowAnimation.ts` | 发光动画，GSAP yoyo循环，opacity+scale，空闲时暂停 |
| `useLogToastStore.ts` | 独立 zustand store，MAX_LOG_ENTRIES=300 |
| `usePageIdle.ts` | 页面空闲检测，2秒空闲超时，useAnimationActive = isVisible && isFocused && !isIdle |
| `usePulseAnimation.ts` | 脉冲动画，三种类型: heartbeat(3s循环)/statusPulse(1.5s重复2次)/loadingPulse(1.2s循环) |
| `useStartupBoost.ts` | 启动加速，GSAP Timeline编排5元素入场(titleBar/statusBar/title/rightPanel/dockNav)，economy档跳过动画 |

### 5.10 工具库 — `lib/`

| 文件 | 说明 |
|------|------|
| `utils.ts` | 工具函数 (含 safeStorage 内存降级封装，替代 localStorage)，新增 `cn()`/`extractErrorMessage()` |
| `color.ts` | HEX→HSL 颜色转换 |
| `latency.ts` | 延迟等级/颜色计算 (显式 borderBg)，新增 `getLatencyLevel()`/`extractGatewayLatency()`/`extractExternalLatency()`/`mergeNetworkQuality()` |
| `animations.ts` | Framer Motion 动画变体，新增 `createLogEntryVariants()`/`getPanelDirection()`/`createPanelAppleVariants()` |
| `easing-config.ts` | 缓动配置，EASING_60HZ/EASING_120HZ 两套预设，`getEasingConfig(refreshRate)` |

### 5.11 入口点 — `main.tsx`

- **GSAP 全局配置**: `expo.out` 默认缓动, `autoSleep: 5`, `lagSmoothing(500, 33)`, `nullTargetWarn: false`。`force3D` 不设全局默认——transform 相关 tween 均已显式声明 `force3D: true`，全局强制反而让动画结束后合成层不易回收
- **prefers-reduced-motion**: GSAP duration 设为 0
- **主题初始化**: `initTheme()` — 从 safeStorage（localStorage 封装）恢复亮暗模式 + 主题类
- **崩溃恢复** (`setupCrashRecovery`): 最多3次自动重载，GPU/WebGL/SharedArrayBuffer 错误触发重载，渲染心跳10秒无响应视为GPU崩溃触发重载，页面可见性变化时暂停/恢复 GSAP globalTimeline
- **渲染链**: `ErrorBoundary` > `LazyMotion(domMax)` > `MotionConfig(reducedMotion="user")` > `App`
- **开发模式**: 使用 `React.StrictMode`

### 5.12 基础 UI 组件 — `components/ui/`

shadcn/ui 风格的基础组件，被各面板广泛引用：

| 文件 | 说明 |
|------|------|
| `animated-card.tsx` | 动画卡片，支持 noEnterAnimation 属性，被各面板 AnimatedCard 引用 |
| `badge.tsx` | 徽章组件 |
| `button.tsx` | 按钮组件（多 variant/size） |
| `card.tsx` | 卡片基础组件（CardHeader/CardTitle/CardDescription/CardContent） |
| `dialog.tsx` | 对话框组件（Radix UI Dialog 封装） |
| `input.tsx` | 输入框组件 |
| `label.tsx` | 标签组件 |
| `select.tsx` | 下拉选择组件 |
| `separator.tsx` | 分隔线组件 |
| `switch.tsx` | 开关组件 |
| `tooltip.tsx` | 提示框组件（Radix UI Tooltip 封装） |

### 5.13 主应用组件 — `App.tsx`

应用主组件（466 行），编排所有业务模块：

- **面板路由**: 基于 `activePanel` switch 渲染 8 个面板（dashboard/account/network/monitor/quality/speedtest/settings/log）
- **PANEL_TITLES**: 面板标题 i18n key 映射表（titleKey/descKey）
- **初始化**: 调用 `useAppInit()` + 5 个业务 Hook（useAuth/useMonitor/useNetwork/useAccount/useSettings）
- **启动加速**: `useStartupBoost` 编排 5 元素入场动画（titleBar/statusBar/title/rightPanel/dockNav）
- **面板转场**: `AnimatePresence mode="wait"` + `panelVariants`（createPanelAppleVariants）+ slideDirection；切换锁 60ms（锁只需覆盖退出时长，锁内点击仍按设计丢弃）。**内容用 deferredPanel**（§三-8：`useDeferredValue(activePanel)`，快速连切跳过中间面板 mount）——面板 switch/转场 key/标题 key/滑动方向全部消费 `deferredPanel`，`activePanel` 仅用于 DockNav 高亮与 storage 恢复
- **quality 面板可见性联动**: `enableNetworkQuality === false` 时 App 对 quality 面板渲染 `null`、DockNav 过滤入口。三处必须联动——`useInitialDataLoad` 启动恢复 `defaultPanel`/`savedPanel` 时跳过 quality（否则重启后主区域空白）、`SettingsPanel` 关闭质量开关时清 `defaultPanel` 并把 `activePanel` 切回 dashboard。新增受开关控制的面板时同样需三处联动
- **窗口监听**: `getCurrentWindow().onResized` 监听窗口大小变化
- **引导向导**: 首次启动检测（`safeStorage.get('campus-onboarding-done')`），未完成则弹出 OnboardingWizard
- **赞助下拉浮层自动弹出**: 已有账号才弹（`configUser` 非空，与 onboarding 的 `!configUser` 条件天然互斥）→ 启动 1s 延迟（等启动入场动画完成）→ `document.visibilityState === 'visible'` 才弹（静默启动/最小化时挂 visibilitychange 推迟到可见）→ 7 天频控（localStorage epoch ms）。弹出瞬间即写时间戳；标题栏 Heart 与关于对话框"赞助支持"两个手动入口不受频控、不写时间戳
 外层 ErrorBoundary（L460）+ 面板内容 ErrorBoundary（L383）+ main.tsx ErrorBoundary
- **useLogToastStore**: 独立 zustand store 用于 Toast 管理

### 5.14 安卓端前端 — `android/frontend/` (2026-09-10 并入)

**代码形态**: 独立 React 代码库（**桌面复刻 + 移动裁剪**，非源码级共享）——独立 package.json（版本号与桌面同步维护），vite alias 仅 `@ → ./src`，dev 端口 5174（strictPort）。目录结构与桌面 frontend 同构（auth/account/monitor/settings/shared/hooks/i18n/lib），测试基建不复刻。

**平台判断**: `frontend/.env` 设 `VITE_PLATFORM=android`，源码经 `import.meta.env.VITE_PLATFORM === 'android'` 分支（消费点：AccountPanel/MonitorPanel/SettingsPanel/LogPanel 四处，如日志面板移动端改纵向布局）。

**移动适配要点**:
- **导航**: `BottomNav`（MobileTab 底部导航）替代桌面 DockNav；桌面件不渲染——TitleBar/StatusBar/RightPanel/DockNav/FluidBackground/SponsorCard（见 App.tsx 头部注释）。新手向导不共用桌面 Dialog 版本，改用手机专属全屏版 `OnboardingWizardMobile`（见 §5.14.2）
- **生物识别**: 新增 `@tauri-apps/plugin-biometric`（BiometricPrompt，兜底锁屏凭据）对应桌面 Windows Hello；验证成功调 `verify_biometric_identity` 写后端 TTL
- **关于对话框**: `AboutDialogMobile.tsx` 移动版；更新检查循环发现新版本时由 `UpdateAvailableDialog`（shared/，手机 App.tsx 与平板 TabletShell 双壳挂载）弹窗提醒，含「立即前往更新」（跳转关于界面）与「关闭」按钮，状态 `qualityStore.updatePromptOpen`
- **tauri.conf.json**: identifier `com.campuslogin.client`、窗口 400×800（移动竖屏）、`bundle.android.minSdkVersion` 29（Android 10+）、devUrl 5174

### 5.14.1 平板/手机双外壳 — 平板/手机形态区分 (2026-09-10)

安卓端同一 APK 需同时覆盖手机与平板。运行时按**设备短边**区分形态（`hooks/useFormFactor.ts`）：`Math.min(innerWidth, innerHeight) >= 600` 判为平板——即 Android 官方 **sw600dp（smallest-width）** 语义（viewport `width=device-width` 时 1 CSS px ≈ 1dp），与 react-native-device-info 的 `smallestScreenWidthDp >= 600`、Flutter 社区 `shortestSide >= 600` 惯例同源。**不用"宽>高"判方向**：手机横屏宽>高但短边仍 <600 仍是手机，平板竖屏宽<高但短边仍 ≥600 仍是平板，旋转不翻转结论；`resize`+`orientationchange` 监听实现旋转/分屏实时切换（无需刷新）。

- **双外壳路由**（`App.tsx`）: `formFactor === 'tablet'` 渲染 `components/tablet/TabletShell.tsx`（桌面 Dock 布局），否则渲染原移动外壳 `AppInner`（顶 header + 单列卡流 + 底部 5 tab）。两外壳各自调用 `useAppInit`，仅渲染其一不重复初始化。
- **TabletShell 复用面**: TitleBar/StatusBar/RightPanel/DockNav 与桌面端同源副本（样式零改动）；面板内容全用安卓版——总览=MobileDashboard（安卓 DashboardPanel + 移动卡注入）、设置=安卓 SettingsPanel（生物识别等安卓项保留、Windows 专属项经 `VITE_PLATFORM` 内部裁剪）、账号/自助服务/监控/测速/日志均为安卓裁剪版面板。**network 面板不接入**（安卓 NAV_ITEMS 无此项，DNS 优化为 Windows 注册表能力）。
- **TabletShell 相对桌面 App 的裁剪**: 无窗口控制（TitleBar 新增 `showWindowControls?: boolean`，安卓副本传 false 隐藏最小化/最大化/关闭三键并禁用 `startDragging`/双击最大化——安卓无窗口管理 API，`minimizeWindow` 等在安卓 tauriApi 是 `desktopOnly` 必 reject）；无 useStartupBoost 开场序列；**有 Onboarding 向导**（桌面同款 Dialog，尺寸改响应式适配平板窄边，见 §5.14.2）；无赞助自动弹出；面板过渡沿用移动外壳轻量 y 位移变体；根容器去桌面 `min-w-[800px]`（平板竖屏 600-800dp 会被压出横向滚动）；标题栏外层加 `env(safe-area-inset-top)`、DockNav bottom 改 `calc(1.25rem + env(safe-area-inset-bottom))`（安卓 edge-to-edge 状态栏/手势条）。
- **DockNav 触屏化**（安卓副本）: 模块级 `IS_TOUCH = matchMedia('(hover: none)')`——触屏设备无 hover，登录/注销按钮**首次点击弹适配器菜单**（已选过则直接执行），菜单顶部新增"自动检测"项（用 `resolveAdapterNames` 同源算出的主适配器执行，与后端规则一致），保留"不指定适配器"路径；鼠标设备行为完全不变。
- **触屏缩放分层**: 桌面布局件按鼠标设计，TabletShell 内以 CSS `zoom` 分层适配触屏——顶部两栏（TitleBar/StatusBar 包裹层）`TOPBAR_ZOOM=1.3`（标题栏按钮 28→36px），内容区（main+RightPanel 父容器）`CONTENT_ZOOM=0.9`；safe-area padding 置于 zoom 层外；DockNav 不缩放。系数为 TabletShell 顶部常量，真机体验后可调。
- **竖屏隐藏日志侧栏**: `useFormFactor.ts` 另提供 `useOrientation`（宽≥高为横屏）。TabletShell 竖屏时不渲染 RightPanel（日志经 Dock"日志"面板仍可达），并把根容器 `--right-panel-width` 置 0px——DockNav 宽度公式 `calc(100vw - var(--right-panel-width, 288px))` 由此自动从"视口-288 居中"切到全视口居中，横屏恢复默认。
- **副本分叉警示**: `TitleBar.tsx`/`DockNav.tsx` 的安卓副本已在桌面版基础上分叉（showWindowControls/IS_TOUCH/safe-area）。**后续从桌面同步这两个文件时不可整文件覆盖**，需人工比对分叉点。

### 5.14.2 安卓端新手向导 — 流程单点 + 双形态外壳

**结构与分工**：
- `settings/useOnboardingFlow.ts` — **流程逻辑单点**（4 步状态机、账号校验、绑定运营商、登录并收尾、配置落盘、`campus-onboarding-done` 写入）。抽出的理由：两套外壳 UI 完全不同，但后端契约必须一致。**无适配器步骤**：安卓后端 `do_login` 丢弃 adapter 参数（网络出口由系统决定），选网卡不生效，指引移除该步。
- `settings/OnboardingWizardMobile.tsx` — **手机专属全屏向导**（`fixed inset-0`）。相对 Dialog 形态的差异：段式进度轨（当前段拉长 + `3/4` 计步）替代 7 个圆点、触控目标放大到 48px、去 `autoFocus`（避免进账号步骤即弹键盘遮住表单）、上下各留 `env(safe-area-inset-*)`、底部操作区固定不随内容滚动、跳过确认用自有遮罩卡片（不用桌面 Dialog）。
- `settings/OnboardingWizard.tsx`（安卓副本）— 平板复用，改为消费同一 hook；`DialogContent` 由固定 `w-[640px] h-[640px]` 收敛为 `w-[min(640px,92vw)] h-[min(640px,86vh)]`，600dp 竖屏实测收敛到 552×640 居中不溢出。

**接入**：手机 `App.tsx`（`OnboardingWizardMobile`）+ 平板 `TabletShell.tsx`（`OnboardingWizard`）都在"首次启动且无账号"时打开，引导标记由向导在「跳过」或「登录成功」时写入——未走完则下次启动继续引导。设置里的重新打开入口经 `onShowOnboarding` 贯通到外壳（平板 SettingsPanel 也传入该 prop）。

**注意**：桌面端 `tauri-app/frontend/settings/OnboardingWizard.tsx` 仍为自包含逻辑（未消费 hook），安卓副本已分叉——后续跨端同步该文件不可整文件覆盖。

### 5.14.3 2D 人脸验证 — `face/` (2026-09-12)

- **背景**：国产设备 2D 人脸全是 Class 1，系统级不可达（BiometricPrompt 只暴露 Class 2/3）；应用内自实现成为唯一现实路径（支付宝同款思路，本地化）。选型 `@vladmandic/human`（代码 MIT；blazeface/facemesh/faceres 模型 Apache-2.0；**不启用**包内 antispoof/liveness/insightface 模型——许可不明或 NC），模型 6 文件随 APK 分发（`frontend/public/models/`，≈8.9MB）。许可与用途声明见 THIRD-PARTY-NOTICES.md。
- **模块**：`faceService.ts`（human 单例 + warmup、相机流、录入=质量门控 8 帧描述子均值、验证=随机动作挑战[眨眼/左转/右转，10s 窗] + similarity≥0.55 比对 3 帧；眨眼用 human mesh 比率判据+帧间状态机，防瞬态丢帧）；`faceVerifyStore.ts`（命令式弹窗桥 + shouldUseFaceFallback；独立于 tauriApi 避免 hook 循环引用）；`FaceCaptureDialog.tsx`（录入/验证共用 UI，App 根部单例，双外壳共用）。
- **验证链语义**（`verifyWindowsIdentity` 单点分支）：开关 `allow2dFaceVerify`(后端 Settings,默认 false)开启 + 模板已录入 + `plugin-biometric.checkStatus()` 不可用 → 2D 人脸；否则系统 BiometricPrompt(BIOMETRIC_WEAK|DEVICE_CREDENTIAL，生物/锁屏自动兜底)。人脸失败码（faceTimeout/faceChallenge/faceMismatch/faceCamera）并入 `biometricFailMessage` 本地化。模板仅存本机 safeStorage，验证通过仍走后端 `verify_biometric_identity` TTL 门——后端不感知验证方式。
- **配置要点**：CSP 必须含 `script-src 'wasm-unsafe-eval'`（tfjs WebGL/WASM），`img-src`/`worker-src` 需 blob:；Manifest 需 CAMERA——wry 0.55.1 Kotlin `RustWebChromeClient.onPermissionRequest` 已内置 VIDEO_CAPTURE→CAMERA 运行时权限链路，无需自写插件。相似度阈值 0.55 为 human 默认注释值从严起步，**真机标定后可调**；不依赖 SharedArrayBuffer 多线程（Android 自定义协议下 crossOriginIsolated 未验证）。

---

## 附录 C：IPC 通信完整清单

### 6.1 请求-响应命令 (56 个)

| 命令名 | 说明 |
|--------|------|
| `get_config` | 获取配置 |
| `show_window` | 显示窗口 |
| `save_config` | 保存配置 (空密码兜底；可选 `clear_password`/`clearSelfPassword` 显式清除对应密码，跳过兜底强制置空) |
| `do_login` | 登录 (支持 adapterName) |
| `do_logout` | 注销 (支持 adapterName) |
| `get_adapters` | 获取适配器列表 |
| `get_adapter_details` | 获取适配器详情 |
| `check_portal_status` | 检测 Portal 状态 |
| `get_disabled_adapters` | 获取禁用适配器 |
| `enable_adapter` | 启用适配器 |
| `dhcp_renew_all` | DHCP 续租 |
| `check_network_quality` | 网络质量检测 |
| `start_latency_test` | 启动延迟测试 |
| `stop_latency_test` | 停止延迟测试 |
| `check_dns_doh_status` | 检测 DNS/DoH 状态 |
| `setup_dns_doh` | 一键设置 DNS + DoH |
| `list_accounts` | 列出账号 |
| `switch_account` | 切换账号 |
| `save_current_as_account` | 保存当前为账号 |
| `delete_account` | 删除账号 |
| `get_active_account` | 获取活跃账号 |
| `start_background_check` | 启动后台检测 |
| `stop_background_check` | 停止后台检测 |
| `trigger_background_check` | 触发一次检测 |
| `get_background_status` | 获取后台状态 |
| `get_auto_launch` / `set_auto_launch` | 开机自启 |
| `get_notification_enabled` / `set_notification_enabled` | 通知开关 |
| `cancel_auto_exit` | 取消自动退出 |
| `minimize_window` / `close_window` | 窗口控制 |
| `open_external` | 打开外部链接 |
| `get_logs` / `clear_logs` | 日志管理 |
| `get_init_data` | 初始化数据 |
| `check_update` / `download_update` / `install_update` / `get_mirror_urls` | 更新管理 |
| `set_debug_mode` / `get_debug_mode` | 调试模式 |
| `get_log_retention_days` | 获取日志保留天数 |
| `set_log_retention_days` | 设置日志保留天数 |
| `check_campus_status` | 检测校园网状态 |
| `dhcp_release_renew` | DHCP 释放续租 |
| `dhcp_release_renew_adapter` | 指定适配器 DHCP 释放续租 |
| `render_heartbeat` | 前端心跳 |
| `get_gpu_info` | 获取 GPU 信息 |
| `bind_operator` | 绑定运营商账号（自助服务系统登录 + 运营商绑定；`selfHelloEnabled` 开启时经 `ensure_identity_gate` 后端验证门） |
| `query_bind_status` | 查询运营商绑定状态（手机号掩码显示） |
| `verify_windows_identity` | Windows Hello 本地身份验证（成功写后端验证时间戳，弹窗文案由前端传入） |
| `reveal_operator_credential` | 查看运营商账户密码明文（校验后端 600s TTL 内验证通过，过期拒绝） |
| `query_self_dashboard` | 自助服务在线设备查询（一次登录连拉两接口，原始 JSON 透传） |
| `query_self_online_log` | 自助服务上网记录查询（严格 YYYY-MM-DD 日期校验，拒绝 start>end） |
| `self_offline_session` | 踢设备下线（改变外部状态，经 `ensure_identity_gate` 后端验证门） |

#### 6.1.1 安卓端命令面 (44 个，与桌面同名对齐)

注册于 `android/src-tauri/src/lib.rs`，前端 `tauriApi` 接口面两端一致。桌面专属命令（app/helper/monitor 启动/DNS 设置/适配器管理/Windows Hello/窗口控制等）在安卓 cfg 门控不可见。

| 模块 | 命令 |
|------|------|
| protocol_cmds | `do_login` / `do_logout` / `check_portal_status` / `ping_test` / `bind_to_wifi`（进程网络绑定 WLAN） |
| campus_detect | `detect_campus`（阶段 1 检测卡详情） / `check_campus_status`（桌面同名，`currentSsid` 恒空） |
| config_state | `get_config` / `save_config`（Keystore 加密落盘 + 掩码出口 + clear 标志） |
| self_service_cmds | `verify_biometric_identity`（BiometricPrompt 成功后写后端 TTL） / `bind_operator` / `query_bind_status` / `query_self_dashboard` / `query_self_online_log` / `self_offline_session` / `reveal_operator_credential`（门语义同桌面：改状态设门、查询免门、reveal 强制） |
| account_cmds | `list_accounts` / `switch_account` / `save_current_as_account` / `delete_account` / `get_active_account` |
| system_cmds | `get_init_data`（补桌面字段空默认） / `get_soc_info`（SoC 分档 tier 0-3） / `get_logs` / `clear_logs` / `get_log_retention_days` / `set_log_retention_days` / `get_debug_mode` / `set_debug_mode` |
| monitor_loop | `start_background_check` / `stop_background_check` / `trigger_background_check` / `get_background_status` / `get_boot_autostart` / `set_boot_autostart` / `get_notification_enabled` / `set_notification_enabled` |
| quality_cmds | `check_network_quality` / `start_latency_test` / `stop_latency_test` |
| update_cmds | `check_update` / `download_update` / `get_mirror_urls` / `install_update`（APK 路径限定更新目录 → FileProvider 唤起安装器） |

### 6.2 事件推送

| 事件名 | 说明 |
|--------|------|
| `background-check-result` | 后台检测结果 (含 adapter1Name/adapter2Name/currentSsid/onCampusNetwork) |
| `auto-login-result` | 自动登录结果 |
| `adapters-changed` | 适配器状态变更 |
| `disabled-adapters-changed` | 禁用适配器变更 |
| `adapter-disabled-warning` | 适配器禁用警告 |
| `login-log` | 登录/注销日志 |
| `network-quality-result` | 网络质量结果 |
| `auto-exit-countdown` | 自动退出倒计时 |
| `auto-exit-cancelled` | 自动退出已取消 |
| `update-available` | 更新可用 |
| `update-download-progress` | 下载进度 |
| `update-notification-click` | 更新系统通知被点击（platform/toast.rs WinRT Activated 回调发出，仅桌面） |
| `adapter-details-changed` | 适配器详情变更 |
| `campus-exit-countdown` | 校园网退出倒计时 |
| `campus-exit-cancelled` | 校园网退出已取消 |
| `config-changed` | 配置变更 |

**安卓端 IPC 面**：命令面见 §6.1.1。事件面为桌面子集：`background-check-result`/`login-log`/`auto-login-result`/`network-quality-result`/`update-available` 等；适配器×4、自动退出×2、校园网退出×2、`config-changed` 等桌面事件安卓不 emit。

---

## 附录 D：依赖关系

### 7.1 Rust 依赖 (Cargo.toml)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tauri-build` | 2 (build-dependencies, features=[]) | Tauri 构建工具 ([build-dependencies]) |
| `tauri` | 2 (features: tray-icon, image-png, image-ico) | 应用框架 |
| `tauri-plugin-*` | 2 | shell/notification/autostart/global-shortcut/single-instance |
| `serde` / `serde_json` | 1 (serde: derive) | 序列化 |
| `tokio` | 1 (rt-multi-thread, time, net, macros, io-util) | 异步运行时 |
| `tokio-util` | 0.7 (rt) | CancellationToken |
| `reqwest` | 0.12 (default-features=false; json, http2, rustls-tls, charset) | HTTP客户端 |
| `tokio-rustls` | 0.26 (features: ring) | TLS 连接 (DoH) |
| `rustls-pki-types` | 1 | TLS 类型 |
| `webpki-roots` | 0.26 | TLS 根证书 |
| `hickory-resolver` | 0.24 (tokio-runtime) | DNS 解析 |
| `dashmap` | 6 | 并发 HashMap (DNS 评分/缓存) |
| `parking_lot` | 0.12 | 高性能同步原语 |
| `arc-swap` | 1 | 原子引用交换 |
| `windows` | 0.58 (features: 14项 — IpHelper/Ndis/WinSock/Foundation/Globalization/Security/Shell/WindowsAndMessaging/Threading/Com/Ole/Variant/Gdi/Dxgi) | Win32 API |
| `webview2-com-sys` | 0.38 (Windows 目标) | WebView2 COM 接口 (ICoreWebView2_19 内存管理) |
| `windows-core` | 0.61 (Windows 目标) | Windows COM 核心类型 |
| `winreg` | 0.52 (Windows 目标) | Windows 注册表读写 |
| `surge-ping` | 0.8 | ICMP ping |
| `sha2` | 0.10 | SHA-256 校验 (更新安装包完整性验证) |
| `urlencoding` | 2 | URL 编码 |
| `regex` | 1 | 正则表达式 |
| `url` | 2 | URL 解析验证 |
| `dirs` | 6 | 数据目录 |
| `lazy_static` | 1.5 | 静态初始化 |
| `getrandom` | 0.3 | 密码学随机数 (MAC 随机化 generate_random_mac) |
| `base64` | 0.22 | Base64 编解码 |
| `chrono` | 0.4 | 时间处理 |
| `open` | 5 | 打开外部链接 |

### 7.1.1 安卓端依赖 (android/src-tauri/Cargo.toml)

- **协议核心单点**：`campus-login = { path = "../../tauri-app/src-tauri" }`——登录/注销/Portal 探测/自助服务/网络质量等协议实现零复制，直接复用桌面 crate；桌面侧 cfg 门控的模块（app/helper/monitor/update）对安卓不可见
- 安卓 target 专属：`libc`（sched_setaffinity 小核绑定）；手写插件 `keystore`/`foreground-service`/`network-bind`（Cargo path 依赖，含 Kotlin 侧）
- 密码加密 = AndroidKeyStore AES-GCM（手写 keystore 插件，`CryptoBridge` 抽象与桌面 DPAPI 同构），磁盘形态 `EncodedSettings`（密码字段与密文分离）

### 7.2 前端依赖 (frontend/package.json)

**运行时依赖 (dependencies, 21 项)**:

| 依赖 | 版本 | 用途 |
|------|------|------|
| `react` / `react-dom` | ^19.0.0 | UI框架 |
| `@tauri-apps/api` | ^2 | Tauri前端API |
| `@tauri-apps/plugin-shell` | ^2 | Tauri Shell 插件前端绑定 |
| `framer-motion` | ^12.38.0 | 动画 |
| `react-i18next` | ^17.0.8 | 国际化 React 绑定 |
| `i18next-browser-languagedetector` | ^8.2.1 | 语言自动检测 |
| `i18next` | ^26.3.1 | i18n 核心 |
| `lucide-react` | ^0.446.0 | 图标 |
| `zustand` | ^5.0.13 | 状态管理 |
| `gsap` | ^3.15.0 | 高性能动画 |
| `class-variance-authority` | ^0.7.0 | 组件变体样式 (CVA) |
| `clsx` | ^2.1.1 | className 合并工具 |
| `tailwind-merge` | ^2.5.0 | Tailwind class 冲突合并 |
| Radix UI primitives (7 项) | ^1.1.x ~ ^2.1.x | 无障碍UI (dialog/label/select/separator/slot/switch/tooltip) |

**开发依赖 (devDependencies, 12 项)**:

| 依赖 | 版本 | 用途 |
|------|------|------|
| `@testing-library/jest-dom` | ^6.9.1 | Jest DOM 断言扩展 (vitest 单元测试) |
| `@testing-library/react` | ^16.3.2 | React 组件测试工具 (渲染/查询/交互) |
| `@types/react` / `@types/react-dom` | ^19.0.0 | React 类型定义 |
| `@vitejs/plugin-react` | ^4.3.0 | Vite React 插件 |
| `autoprefixer` | ^10.4.20 | 自动添加浏览器前缀 |
| `jsdom` | ^29.1.1 | DOM 环境模拟 (vitest 测试环境) |
| `postcss` | ^8.4.40 | CSS 后处理器 |
| `tailwindcss` | ^3.4.10 | CSS 框架 |
| `typescript` | ^5.5.0 | 类型系统 |
| `vite` | ^6.0.0 | 构建工具 |
| `vitest` | ^4.1.10 | 单元测试框架 (Vite 原生集成) |

**scripts 脚本**:

| 脚本 | 命令 | 说明 |
|------|------|------|
| `dev` | `vite` | 启动开发服务器 |
| `build` | `vite build` | 生产构建 |
| `preview` | `vite preview` | 预览构建产物 |
| `test` | `vitest run` | 单次运行单元测试 (CI 用) |
| `test:watch` | `vitest` | 监听模式运行单元测试 |
| `clean` | node 脚本 | 清理 dist 与 node_modules/.vite 缓存目录 |

---

## 附录 E：安全体系

| 措施 | 实现 |
|------|------|
| 密码加密存储 | Windows DPAPI，绑定当前用户 |
| 密码防意外清空 | 前端空密码不发送，后端空密码+旧密码存在时保留旧密码 |
| DPAPI 空数据防护 | 加密结果为空数据时返回 Err，不静默成功 |
| TLS 1.3 强制 | HTTP 客户端默认 TLS 1.3，回退 TLS 1.2 |
| DoH 安全 | RFC 8484 wire format，兼容主流 DNS 服务商 |
| CSP 策略 | 限制脚本、插件和表单提交来源 |
| 外部链接验证 | http/https 前缀 + `url::Url::parse` + 长度限制(2048) + 用户名/密码字段检查 (commands/system.rs::open_external) |
| 认证失败计数 | auth/failure_tracker.rs 连续 5 次 auth_failure 触发 MAC 重置 + DHCP 续租 (MAX_FAILURES=5)；并发互斥靠 TaskLock |
| 账号名称校验 | infra/state/mod.rs::validate_account_name 正则白名单 `^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$` + 长度 1-32，防路径遍历 |
| panic=abort | 编译选项减小二进制体积，避免信息泄露 |
| TaskGuard RAII 防死锁 | TaskGuard::Drop 自动释放任务锁；`force_release` 标注 `#[cfg(test)]` 仅供测试 |
| SHA256 更新校验 | 校验源优先级：GitHub API asset digest（服务端计算，发布者漏传 .sha256 时兜底）→ 官方 .sha256 → 3 镜像 .sha256，任一成功即用 (`updater.rs extract_checksum`)；全 4xx 默认拒绝安装（需 `skipSha256WhenMissing`，无前端开关），5xx/传输错误/哈希不匹配一律拒绝 |
| 更新发布约定 | ① `check_update_inner` 对下载 URL 做 HEAD 探测，Release 资产 404（version.json 先行而未发布）则本轮不提示更新，探测网络失败保守视为存在；② `version.json` 支持可选 `notes` 字段填充 release_notes；③ `build.ps1` 构建后自动生成 `<installer>.sha256`（shasum 兼容格式），**发布 Release 必须同时上传安装包与 .sha256 文件**，版本号提交与 Release 发布需同流程完成 |
| 适配器名称校验 | network/adapter_cache.rs::validate_adapter_name，禁止 `&\|;\`$()<>\"'\n\r\0` 等元字符，防命令注入 |
| 敏感操作验证门 | 查看明文/绑定/踢下线过 Windows Hello 门：前端 TTL 570s 会话（`account/selfServiceState.ts`，明文查看 `ignoreToggle` 无视总开关）+ 后端 600s TTL 真防线（`platform/identity.rs::identity_verified_recently` 含回拨拒绝；`commands/self_service.rs::ensure_identity_gate` 仅对改外部状态命令校验）；只读查询有意不设门；`selfHelloEnabled=false` 整体放行但明文查看仍强制验证（详见 §三-4 / §4.5.4.2） |
| 出站掩码唯一出口 | 所有把 Config 发往前端的路径必经 `Config::masked_for_display()`（password + self_password 双字段掩码，空值=未设置语义保留），回归单测锁死——详见 §4.3 掩码纪律 |
| 自助服务凭据纪律 | 学号/自助服务密码仅内存传递不落盘不写日志（查询命令参数为空/MASK 时后端回退已保存值）；自助服务密码持久化与登录密码同措施（DPAPI + MASK 出站 + 显式清除标志） |

---

## 附录 F：性能优化与已知限制

| 优化项 | 说明 |
|--------|------|
| PowerShell 全消除 | 提权（`--helper` 重启自身直调 Win32/winreg）、DNS/DoH 设置（`dns_config.rs` Win32 API）、GPU 检测（DXGI）均为 Rust 直调，无子进程 shell |
| WebView2 内存与渲染策略 | 前台 NORMAL/后台 LOW（`ICoreWebView2_19.SetMemoryUsageTargetLevel`）；浏览器参数仅 `--js-flags=--max-old-space-size=512`，渲染交还平台默认（勿动 vsync/GPU 实验参数，见踩坑） |
| Tokio 线程池动态配置 | 按 CPU 核心数配置 `worker_threads(2-8)` / `max_blocking_threads(8-64)` |
| CAS 原子状态更新 | `ConfigStore`/`NetworkState` 均为 ArcSwap CAS 循环，避免 TOCTOU 竞态 |
| 后台巡检降载 | 注册表遍历进 `spawn_blocking`；网关探测 surge_ping 替代 ping 子进程；SSID/网络 Profile 60s TTL 缓存；适配器查询 5s TTL + 4s 后台刷新 |
| DNS 解析降耗 | Resolver 复用小池；竞速降级为"历史最快 1 路 DoH + 传统 DNS"（每域名 TLS 握手减半）；缓存淘汰单次排序 |
| 质量检测收敛 | 唯一周期驱动者 = 定时测试循环（`enable_network_quality && enable_latency_test`）；未在线跳过；恶化到 poor/bad 需 15s×2 复核才通知；Phase 1 分批 ≤3 并发 |
| HTTP 客户端池 LRU | 命中刷新访问时间，超限剔除最久未访问（上限 32，TTL 600s） |
| 更新下载 | 流式 SHA256（64KB 分块）+ tokio::fs 异步写盘 |
| 日志 IO | 批量落盘（32 条或 2s）；read_recent_logs 尾部倒读；shutdown 带超时 join 替代固定 sleep |
| 前端渲染 | useShallow 最小粒度订阅；高频事件节流；面板内容 `deferredPanel`（见 §三-8）；常用面板静态导入 + 3 个低频对话框 lazy + 启动预取；GSAP 统一管理（`expo.out` / autoSleep 5 / lagSmoothing，prefers-reduced-motion 全停） |
| 类型检查 | tsconfig 显式 `types: ["vite/client"]`（node 侧 `["node"]`）阻断 @types 自动加载；命令 `npx tsc --noEmit --incremental`（禁 `tsc -b`，见踩坑） |

**已知限制（有意不修）**：① netsh 文本解析依赖中/英关键字，其他系统语言静默失效（目标用户群为中文系统，结构化解析无官方 JSON 接口）；② index.css 的 Tailwind 语义类 !important 劫持（.rounded-xl 等）与全局 border 透明为 v2.5.0 设计系统决策，全局移除会引发不可控视觉回归，维持现状；③ framer-motion 对 Reorder.Item 内联写 touch-action: pan-x（触摸屏垂直滚动让位于拖拽排序，框架行为）；④ v6 栈"空 NameServer 清除"的 API 接受性未经 Win11 实测（失败已降级警告）；⑤ 后端 serde_json::json! 手写返回体与前端类型的字段对齐靠约定，无编译期保证；⑥ auth/session 页面特征误判"已在线"（Portal 页面残留 uid='/v4ip=' 时跳过登录）——判定逻辑需真机实测 Portal 页面后才能改；⑦ 注销占位凭据 drcom/123 为 Dr.COM 惯例，现实 Portal 接受，不为假想故障加真实凭据回退；⑧ config `campusCheckStartHour` alias 在残留旧字段的脏数据下覆盖分钟值；⑨ atomic_write 在 rename 前崩溃的窗口回退旧配置（非丢失）。

---

## 附录 G：编译配置与测试基线

```toml
[profile.release]
lto = "thin"
codegen-units = 1
opt-level = 3
strip = true
panic = "abort"
```

### 测试与构建基线（2026-09-10 实测）

- **主仓库 `cargo test`**（`tauri-app/src-tauri` 下）：**493 全绿**（lib 246 + bin 同套 246 + repro 1）；改动后不得低于此基线
- **安卓 Rust 验证**：host `cargo check` 在 `android/src-tauri` 基线即失败（mobile-only 插件权限 host 收集不全），靠 `tauri android build` 交叉编译验证
- **前端验证**：`npx tsc --noEmit --incremental`（禁止 `tsc -b`——tsconfig.node.json 是 composite 项目，会 emit 出 vite.config.js/.d.ts 污染文件）
- **安卓构建链**：先手动 `npx vite build`（tauri CLI 不跑 beforeBuildCommand）→ `tauri android build --target aarch64 --apk`；Windows 需开启开发者模式（允许符号链接）
- **安卓产物命名与签名（gradle 内置）**：`gen/android/app/build.gradle.kts` 配置 release signingConfig（本机 `~/.android/debug.keystore`）+ `applicationVariants` outputFileName——构建直接产出已签名/已对齐的 `Wxxy-CampusLogin_<版本>.apk`（版本号跟随 tauri.conf.json），zipalign/apksigner 后处理整体消失。**CLI 完成报告仍指向旧约定名 `app-universal-release.apk`（预期路径，实际不存在），以输出目录实际文件为准**。签名证书一经发布不可更换（换=用户卸载重装）。一键链 `pwsh android/build-apk.ps1`（含本机 JDK 路径，gitignore 不入库）

---

## 附录 H：版本号管理

### 设计目标

应用内存在多处版本号引用（Tauri 配置 / Rust 代码 / 前端常量 / 发布资源 / 文档），传统做法是手动同步每个位置，易遗漏导致发布包版本错乱。本项目通过 **build.rs 自动注入** + **权威源** 设计，实现"只改一处 + 编译期自动同步"。

### 权威源与流向

```
[tauri.conf.json]  (用户编辑的唯一位置)
       │
       │  build.rs 读取 → cargo:rustc-env=APP_VERSION
       ▼
[Rust 编译期 env!("APP_VERSION")]
       │   main.rs / updater.rs / commands/system.rs 共 3 处
       ▼
   应用运行时的版本号
```

**权威源**：`tauri-app/src-tauri/tauri.conf.json` 的 `"version"` 字段。

**Cargo.toml 的 version 字段**：cargo 强制要求存在，**必须与 tauri.conf.json 保持一致**（否则 tauri 编译会发出 `Version mismatch` 警告），由发布流程手动同步。

### build.rs 注入机制

`tauri-app/src-tauri/build.rs` 的核心逻辑：

```rust
tauri_build::build();
// 从 tauri.conf.json 提取 "version" 字段（手写字符串提取，不引 serde_json）
println!("cargo:rerun-if-changed=tauri.conf.json");
println!("cargo:rustc-env=APP_VERSION={version}");
```

- Rust 侧以编译期宏 `env!("APP_VERSION")` 引用（main.rs / updater.rs / commands/system.rs 共 3 处），零运行时开销
- `rerun-if-changed` 仅在 tauri.conf.json 变化时触发重编译

### 升级版本号的完整流程

发布新版本时（以 v2.3.0 为例）：

1. **编辑唯一权威源** — 修改 `tauri-app/src-tauri/tauri.conf.json` 的 `"version"` 字段为 `"2.3.0"`
2. **手动同步 Cargo.toml** — 修改 `tauri-app/src-tauri/Cargo.toml` 的 `version` 字段为 `"2.3.0"`（cargo 强制要求）
3. **同步发布标记** — 修改仓库根 `version.json` 的 `"version"` 为 `"v2.3.0"`（带 v 前缀，是 GitHub release tag 的格式）
4. **同步前端 package.json** — 两个 `package.json` 的 `"version"` 字段（npm 规范要求，无 v 前缀）
5. **同步前端常量** — `tauri-app/frontend/src/shared/ui-constants.ts` 的 `APP_VERSION`
6. **同步静态预览** — `tauri-app/frontend/about-preview.html` 的 `app-version` 和 `status-version` 两个 div（**注意**：此处带 `v` 前缀，如 `v2.3.0`）
7. **同步徽章** — `README.md` 的 `version-2.3.0` 徽章
8. **同步文档** — `CODE_WIKI.md` 顶部版本号 + 底部元信息

> ⚠️ **Cargo.lock 中的 version**：由 cargo 自动更新，下次 `cargo build` 时自动重写。

> ⚠️ **版本号提交与 Release 发布必须同流程完成**（v2.3.0 事故教训）：version.json 先行推送而 Release 未发布时，旧版用户收到更新通知但下载 404；后端已有 HEAD 探测兜底（资产 404 本轮不提示），流程上仍须绑定同一次操作。

> ⚠️ **Release 资产发布检查清单**：
> 1. 上传 `Wxxy-CampusLogin_{ver}_x64-setup.exe`（文件名与硬编码拼接一致；若改名，在 `version.json` 加 `"asset": "<完整文件名>"` 覆盖默认命名）
> 2. 同时上传构建产物目录中的 `{安装包名}.sha256`（`build.ps1` 第 [5/5] 步已自动生成）——缺失时应用内更新校验全 4xx，默认拒绝安装且用户无法自救
> 3. `version.json` 可选填 `notes` 字段（字符串，Markdown 列表），将显示为应用内更新日志（release_notes）
> 4. 版本号支持任意段数（`2.3.0.1` hotfix 可正确提示升级）

### 安卓端 identifier（包名）

- **两端 identifier 相互独立**：桌面 `tauri-app/src-tauri/tauri.conf.json`（`com.campus.login`）、安卓 `android/src-tauri/tauri.conf.json`（`com.campuslogin.client`）。安卓 identifier 直接决定 `gen/android/app/build.gradle.kts` 的 `namespace`/`applicationId` 与 MainActivity 包路径——改 identifier 必须同步这三处并 `git mv` Kotlin 目录；**改包名=卸载重装**（AndroidKeyStore 密钥按包名隔离，密码密文作废）
- `.app` 结尾的 identifier 会触发 tauri-cli 的 macOS bundle 冲突警告（纯 lint，无 macOS 目标也无碍）
- `gen/android/buildSrc` 的 Kotlin 文件**无 package 声明（默认包）**，目录名 `com/campuslogin/app/kotlin/` 是历史残留，与包名无关不影响编译；Manifest 的 activity 用相对名 `.MainActivity` 自动跟随 namespace
- 构建后用 `aapt2 dump badging <apk>` 验证 applicationId；gen/schemas 与插件 permissions 随构建再生的 `\n`→`\r\n` 行尾差异是噪音（语义零变化，node 深比较可证），提交前 `git checkout --` 还原

### 后端代码引用方式

```rust
// app/startup.rs:149 启动日志
crate::log_info!("app", "应用启动, 版本: v{}", env!("APP_VERSION"));

// update/updater.rs:387 更新检查
let current = env!("APP_VERSION");
let has_update = compare_versions(current, &latest_tag);

// commands/system.rs:120 系统信息接口
let version = env!("APP_VERSION").to_string();
```

### 前端版本号来源（单轨）

| 来源 | 用途 | 修改方式 |
|---|---|---|
| `ui-constants.ts` `APP_VERSION` 常量 | 代码内直接 import | 手动同步 |

> `vite.config.ts` 不做版本号注入，前端版本号唯一来源为 `ui-constants.ts` 的 `APP_VERSION` 常量。

> 注：`ui-constants.ts` 的 `APP_VERSION` 保留硬编码是为了在非 Tauri 环境（如纯前端 Storybook / 单元测试 mock）下也能取到合理默认值。**升级时仅需同步 `ui-constants.ts` 一处**。

### 版本号格式约定

- **semver 格式（不带 v）**：`Cargo.toml` / `tauri.conf.json` / `package.json` × 2 / `ui-constants.ts` → `2.3.0`
- **发布 tag 格式（带 v）**：`version.json` / 后端日志（`v{}`）/ `about-preview.html` 的 `app-version` 和 `status-version` div（`v2.3.0`）/ README 徽章（`version-2.3.0` 不带 v，但后端启动日志带 v）

---

*文档版本: v2.3.5 | 基于代码版本: CampusLogin v2.3.5 | 更新日期: 2026-09-12 | 五段手册层（概览/架构速查/关键约定/踩坑记录/决策记录）+ 附录 A~H 参考层；含安卓端详解（附录 A §4.16 / 附录 B §5.14 / 附录 C §6.1.1）；精简原则：已解决缺陷的修复过程叙述与历史版本流水移除，保留架构、约定、踩坑、决策、认证与自助服务协议知识及已知限制*

