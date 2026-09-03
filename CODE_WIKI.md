# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.2.9 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
> **目标平台**: Windows (x64)
> **通信方式**: Tauri IPC (`invoke` / `listen`)

---

## 一、项目概览

CampusLogin 是一款校园网自动登录助手桌面应用，面向无锡学院校园网认证系统（锐捷 ePortal），提供一键登录/注销、自动重连、校园网智能检测、DNS 智能解析与优化、网络质量监测、多账号管理等功能。

### 核心特性

| 特性 | 说明 |
|------|------|
| 一键登录 | 自动检测适配器、DHCP续租、智能重试(指数退避+随机抖动) |
| 一键注销 | 两步注销：Radius注销 + MAC解绑，支持指定适配器注销或全部注销 |
| 自动重连 | 后台巡检断线检测，最多3次自动重连 |
| 校园网检测 | 三级检测：网络名称匹配 → /18子网匹配 → 网关Ping可达 |
| DNS 智能解析 | 动态评分选择最优 DNS 服务器，应用级 DoH 解析，三级智能解析策略 |
| DNS 优化 | 检测 DNS/DoH 配置，一键设置推荐 DNS + 启用 DoH 加密 |
| 网络质量检测 | 网关/DNS/DoH/HTTPS/游戏服务器延迟并发测试，DNS 解析专项测试，增量推送逐步填充 |
| 多账号管理 | DPAPI 加密存储、快速切换 |
| 双适配器支持 | 有线 + 无线同时管理，Dock 栏适配器选择菜单 |
| 系统托盘 | 最小化到托盘后台运行，支持托盘快速登录 |
| 开机自启 | 注册表写入 / Tauri 插件 |
| 自动退出 | 登录成功后倒计时退出，快捷键取消(Ctrl+Shift+C) |
| 主题系统 | 7种预设主题 + 自定义主题色 + 深浅模式 |
| 用户自助服务 | 一键打开校园网自助服务系统 |
| 中英语言切换 | 标题栏一键切换中英文，react-i18next + i18next-browser-languagedetector，默认中文 |
| 日志自动清理 | 可选保存时间（3/7/14/30天+永久），AtomicU32全局存储，后端定时清理 |
| 测速面板 | 速度测试面板，网络速度实时检测 |

---

## 二、项目目录结构

```
Wxxy-CampusLogin/
├── assets/                          # 截图等资源
├── tauri-app/
│   ├── package.json                 # 根层依赖
│   ├── frontend/                    # React 前端
│   │   ├── package.json             # 前端依赖 (含 zustand ^5.0, framer-motion ^12, vitest ^4.1)
│   │   ├── vite.config.ts           # Vite 构建配置 (manualChunks 分组)
│   │   ├── vitest.config.ts         # Vitest 测试配置 (jsdom + globals + @ 别名)
│   │   ├── tailwind.config.js       # Tailwind CSS 配置
│   │   ├── postcss.config.js        # PostCSS 配置
│   │   ├── tsconfig.json            # TypeScript 配置
│   │   ├── index.html               # HTML 入口
│   │   ├── public/                  # 静态资源 (图标 PNG)
│   │   └── src/
│   │       ├── main.tsx             # React 入口
│   │       ├── App.tsx              # 根组件
│   │       ├── vite-env.d.ts        # Vite 环境类型声明
│   │       ├── index.css            # 全局样式
│   │       ├── hooks/               # 自定义 Hooks (22个)
│   │       │   ├── tauriApi.ts      # Tauri IPC 封装 (原 useIpc.ts，含 DNS/DoH/注销 API + 重试 + 事件工厂)
│   │       │   ├── useAppStore.ts   # 兼容壳 (3行 re-export，原单体已按领域拆分)
│   │       │   ├── useConfigStore.ts     # 配置领域 store (config/密码防抖保存/accounts/language)
│   │       │   ├── useAuthStore.ts       # 认证领域 store (doLogin/doLogout/checkOnline/status/bgStatus)
│   │       │   ├── useAdapterStore.ts    # 适配器领域 store (adapters/details/disabled/activePanel)
│   │       │   ├── useQualityStore.ts    # 网络质量领域 store (networkQuality/dnsDoh/gpuInfo/refreshRate)
│   │       │   ├── useThemeStore.ts      # 主题领域 store (themeName/isLightMode/customThemeColor + DOM 副作用)
│   │       │   ├── useLogToastStore.ts   # 日志/Toast store (独立 zustand)
│   │       │   ├── useAppInit.ts         # 初始化编排 hook (调用 4 个子 hook)
│   │       │   ├── useEventListeners.ts  # Tauri 事件监听统一注册 (15 个事件 + 窗口关闭拦截)
│   │       │   ├── useInitialDataLoad.ts # getInitData 拉取并 bootstrap 所有 store
│   │       │   ├── useHeartbeat.ts       # 渲染心跳 (5s 间隔，visibility 暂停)
│   │       │   ├── useGlobalShortcut.ts  # 全局快捷键 (Ctrl+Shift+C 取消自动退出)
│   │       │   ├── useGpuCorrection.ts   # WebGL GPU 信息校正 (WEBGL_debug_renderer_info)
│   │       │   ├── useAnimationProfile.ts  # 动画配置
│   │       │   ├── useAsyncLock.ts  # 异步锁
│   │       │   ├── useBreatheAnimation.ts  # 呼吸动画
│   │       │   ├── useGlowAnimation.ts     # 发光动画
│   │       │   ├── usePageIdle.ts   # 页面空闲检测
│   │       │   ├── usePulseAnimation.ts    # 脉冲动画
│   │       │   └── useStartupBoost.ts      # 启动加速
│   │       ├── lib/
│   │       │   ├── utils.ts         # 工具函数 (含 safeStorage 内存降级封装，替代 localStorage)
│   │       │   ├── color.ts         # HEX→HSL 颜色转换
│   │       │   ├── latency.ts       # 延迟等级/颜色计算 (显式 borderBg)
│   │       │   ├── animations.ts    # Framer Motion 动画变体
│   │       │   ├── easing-config.ts # 缓动配置
│   │       │   ├── color.test.ts    # 单元测试 (hexToHsl)
│   │       │   ├── latency.test.ts  # 单元测试 (getLatencyLevel/getLatencyColor/mergeNetworkQuality 等)
│   │       │   └── utils.test.ts    # 单元测试 (cn/extractErrorMessage)
│   │       ├── i18n/
│   │       │   ├── index.ts         # i18next 初始化配置
│   │       │   └── locales/         # 翻译文件 (zh.json / en.json)
│   │       ├── account/             # 账号模块
│   │       │   ├── AccountPanel.tsx # 账号管理面板
│   │       │   ├── useAccount.ts    # 账号逻辑
│   │       │   ├── types.ts         # 账号类型定义
│   │       │   └── index.ts         # 模块导出
│   │       ├── auth/                # 认证模块
│   │       │   ├── DashboardPanel.tsx # 总览面板
│   │       │   ├── AboutDialog.tsx  # 关于对话框
│   │       │   ├── useAuth.ts       # 认证逻辑
│   │       │   ├── types.ts         # 认证类型定义
│   │       │   └── index.ts         # 模块导出
│   │       ├── monitor/             # 监控模块
│   │       │   ├── MonitorPanel.tsx # 监控面板
│   │       │   ├── QualityPanel.tsx # 网络质量面板
│   │       │   ├── SpeedTestPanel.tsx # 速度测试面板
│   │       │   ├── StatusBar.tsx    # 状态栏 (用户自助服务按钮 + 交互动画)
│   │       │   ├── LatencyComponents.tsx # 延迟组件
│   │       │   ├── LatencyTimeline.tsx   # 延迟时间线
│   │       │   ├── NetworkQualityCapsule.tsx # 网络质量胶囊
│   │       │   ├── useMonitor.ts    # 监控逻辑
│   │       │   ├── types.ts         # 监控类型定义
│   │       │   └── index.ts         # 模块导出
│   │       ├── network/             # 网络模块
│   │       │   ├── NetworkPanel.tsx # DNS 优化卡片
│   │       │   ├── useNetwork.ts    # 网络逻辑
│   │       │   ├── constants.ts     # 网络常量
│   │       │   ├── types.ts         # 网络类型定义
│   │       │   └── index.ts         # 模块导出
│   │       ├── settings/            # 设置模块
│   │       │   ├── SettingsPanel.tsx # 设置面板
│   │       │   ├── ThemeDialog.tsx  # 主题对话框
│   │       │   ├── OnboardingWizard.tsx # 新手教程
│   │       │   ├── useSettings.ts   # 设置逻辑
│   │       │   ├── constants.ts     # 设置常量
│   │       │   ├── types.ts         # 设置类型定义
│   │       │   └── index.ts         # 模块导出
│   │       ├── shared/              # 共享组件
│   │       │   ├── LogPanel.tsx     # 日志面板
│   │       │   ├── ErrorBoundary.tsx # 错误边界
│   │       │   ├── ConfirmDialog.tsx # 确认对话框
│   │       │   ├── FluidBackground.tsx # 流体背景
│   │       │   ├── AnimatedNumber.tsx # 动画数字
│   │       │   ├── RefreshButton.tsx # 刷新按钮
│   │       │   ├── SegmentTabs.tsx  # 分段Tab
│   │       │   ├── ToastContainer.tsx # Toast 容器
│   │       │   ├── types.ts         # 共享类型定义
│   │       │   ├── ui-types.ts      # UI 类型定义
│   │       │   ├── ui-constants.ts  # UI 常量 (APP_VERSION/APP_NAME/PASSWORD_MASK/MAX_LOG_ENTRIES)
│   │       │   └── index.ts         # 模块导出
│   │       └── components/          # 基础组件
│   │           ├── layout/          # 布局组件
│   │           │   ├── DockNav.tsx  # 适配器选择浮层 + 注销按钮
│   │           │   ├── RightPanel.tsx # 右侧面板
│   │           │   └── TitleBar.tsx # 标题栏
│   │           └── ui/              # 基础 UI 组件 (shadcn/ui)
│   └── src-tauri/                   # Rust 后端
│       ├── Cargo.toml               # Rust 依赖 (含 webview2-com-sys 0.38, windows-core 0.61)
│       ├── Cargo.lock               # 依赖锁定文件
│       ├── build.rs                 # Tauri 构建脚本
│       ├── tauri.conf.json          # Tauri 应用配置
│       ├── .cargo/
│       │   └── config.toml          # Cargo 构建配置
│       ├── capabilities/
│       │   └── default.json         # Tauri 权限声明
│       ├── icons/                   # 应用图标
│       └── src/
│           ├── main.rs              # 应用入口
│           ├── lib.rs               # 库模块声明
│           ├── config/              # 配置模块
│           │   ├── mod.rs           # 重导出
│           │   ├── model.rs         # 配置模型 + PASSWORD_MASK + user_account_with_operator
│           │   ├── persist.rs       # 配置持久化 (atomic_write 重试 + list_account_names)
│           │   └── validate.rs      # 配置校验 (枚举值/正则/URL/Portal URL 迁移/校园网关校验)
│           ├── network/             # 网络模块
│           │   ├── mod.rs           # 重导出
│           │   ├── client.rs        # 缓存基础设施 (PORTAL_URL/CLIENT_POOL/HTTP客户端/TLS 1.3+回退)
│           │   ├── adapter.rs       # 适配器选择 (薄 re-export 模块: discovery/adapter_cache/dhcp/subnet)
│           │   ├── adapter_cache.rs # 适配器查询缓存 (force/cached 双模式)
│           │   ├── dhcp.rs          # DHCP 操作 (release/renew, MAC 重置, apply_mac_change_via_registry)
│           │   ├── subnet.rs        # 子网判定 (/18 校园网子网匹配)
│           │   ├── dns.rs           # DNS 缓存管理 + DoH解析 + 智能解析策略
│           │   ├── timing.rs        # HTTP计时 + DNS智能解析 + DoH + 评分系统
│           │   ├── quality.rs       # 网络质量并发延迟测试 (两阶段检测+增量推送)
│           │   ├── dns_setup.rs     # DNS+DoH 一键设置 (管理员/提权 helper 共用, setup_dns_doh_admin)
│           │   └── discovery/       # 适配器发现子模块
│           │       ├── mod.rs       # 重导出
│           │       ├── registry.rs  # 注册表遍历 (CLASS_SUBKEY_CACHE 懒加载+锁优化)
│           │       └── windows.rs   # Windows 特定发现逻辑 (GetAdaptersAddresses + LinkSpeed)
│           ├── auth/                # 认证模块 (6个子模块，原 traits.rs 已删除)
│           │   ├── mod.rs           # 重导出
│           │   ├── portal.rs        # Portal认证状态检测 (random_v + block_on_http 同步-异步桥接)
│           │   ├── protocol.rs      # 登录/两步注销/重试/响应解析 (random_v)
│           │   ├── session.rs       # 登录/注销会话管理 (adapter_action_with_log/login_adapter_with_log 通用封装)
│           │   ├── service.rs       # 认证服务编排 (full_login/full_logout 统一入口 + post_login_handler)
│           │   ├── failure_tracker.rs # 认证+Portal请求失败计数 (9c 合并原 monitor/portal_failure.rs)
│           │   └── dual_adapter_executor.rs # 双适配器并行执行器 (泛型静态分发 + tokio spawn_blocking + 可中断错峰)
│           ├── account/             # 账号模块
│           │   ├── mod.rs           # 多账号管理命令
│           │   └── crypto.rs        # 加密工具 (Windows DPAPI)
│           ├── infra/               # 基础设施模块
│           │   ├── mod.rs           # 重导出
│           │   ├── state/           # 全局状态子模块 (重构自 state.rs)
│           │   │   ├── mod.rs       # TaskLock/TaskGuard/TaskFlags/AppState/CommandResult/AccountResult
│           │   │   ├── store.rs     # ConfigStore (封装 ArcSwap<Config>)
│           │   │   ├── network.rs   # NetworkState + NetworkSnapshot (CAS 快照更新)
│           │   │   └── exit.rs      # ExitStateStore
│           │   ├── logger.rs        # 日志系统 (文件+通道+调试模式切换+日志保留天数清理+shutdown mpsc超时join)
│           │   ├── lifecycle.rs     # 自动退出控制 + 校园网退出流程
│           │   ├── notification.rs  # 系统通知封装 (emit_notification，仅非前台 Windows 通知)
│           │   ├── events.rs        # 事件总线 EventBus (16 个 emit_xxx 方法)
│           │   ├── command_context.rs # 命令上下文 CommandContext::from_app
│           │   └── task_manager.rs  # 后台任务管理器 BackgroundTaskManager (cancel token 统一管理)
│           ├── monitor/             # 监控模块 (10个子模块，portal_failure 已迁入 auth/failure_tracker)
│           │   ├── mod.rs           # 重导出 (含 trigger_background_check 别名)
│           │   ├── watcher.rs       # 门面 + 启动聚合 (51行，re-export background_check/background_task + run_startup_tasks)
│           │   ├── background_check.rs  # 后台检测主体 (run_background_check_blocking，从 watcher 拆出)
│           │   ├── background_task.rs   # 后台任务调度 (start_background_check_inner + task_manager.spawn)
│           │   ├── auto_auth.rs     # 自动登录/断线重连
│           │   ├── latency.rs       # 网络质量通知+延迟测试循环
│           │   ├── adapter_watch.rs # 适配器状态监控 (CancellationToken可退出)
│           │   ├── campus_check.rs  # 校园网检测 (CampusCheckResult 定义于此)
│           │   ├── portal_check.rs  # Portal 检测 (PortalCheckResult 定义于此 + check_adapter_portal)
│           │   ├── quality_scheduler.rs # 质量检测调度器
│           │   └── background_emit.rs   # 后台事件推送
│           ├── platform/            # 平台交互模块
│           │   ├── mod.rs           # 重导出
│           │   ├── dns_config.rs    # DNS/DoH 配置文件设置 (per-profile/适配器级/DoH API)
│           │   ├── elevation.rs     # UAC 提权 (ShellExecuteW + COM ShellExec) + GUID 解析 + is_admin
│           │   ├── gpu.rs           # GPU 信息检测 (DXGI) + 刷新率检测 + 浏览器参数 + gpu_preference
│           │   ├── autostart.rs     # 开机自启 (注册表/Tauri 插件)
│           │   └── helper_spawn.rs  # --helper 提权子进程启动 + 结果文件轮询 (spawn_elevated_helper)
│           ├── update/              # 更新模块
│           │   ├── mod.rs           # 重导出
│           │   └── updater.rs       # 更新检查/下载/安装 (SHA256校验)
│           ├── helper/              # 提权辅助子进程 (--helper 模式, 主进程入口最先拦截)
│           │   └── mod.rs           # HelperOp/parse_helper_args/run_helper + 结果文件回写
│           ├── app/                 # 应用生命周期模块
│           │   ├── mod.rs           # 重导出
│           │   ├── startup.rs       # 应用启动 (setup_app + 命令注册 + panic hook)
│           │   ├── tray.rs          # 系统托盘 (菜单/事件处理)
│           │   ├── window.rs        # 窗口管理 (最小化/关闭/显示)
│           │   ├── shortcut.rs      # 全局快捷键 (Ctrl+Shift+C 取消自动退出)
│           │   ├── heartbeat.rs     # 渲染进程心跳检测
│           │   └── shutdown.rs      # 关机/退出流程 (shutdown_and_exit 统一入口)
│           └── commands/            # Tauri 命令 (模块化拆分)
│               ├── mod.rs           # 命令模块声明与架构文档
│               ├── config_cmd.rs    # 配置相关命令 (空密码兜底)
│               ├── login.rs         # 登录/注销命令
│               ├── background.rs    # 后台检测命令入口 (委托 monitor::watcher)
│               ├── network_cmd.rs   # 网络命令 + DNS/DoH 检测与设置 (winreg + ShellExecuteW)
│               ├── system.rs        # 系统功能命令
│               ├── account.rs       # 多账号管理命令 (委托 account 模块)
│               └── updater.rs       # 更新命令 (委托 update 模块)
├── CODE_WIKI.md                     # 本文档
├── README.md                        # 项目说明
├── version.json                     # 版本号配置
├── backend-modular-evaluation-report.md  # 后端模块化评估报告
├── backend-refactor-implementation-plan.md # 后端重构实施计划
└── .gitignore
```

---

## 三、架构总览

### 3.1 分层架构

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
│  │  monitor/watcher.rs (门面+启动聚合, 51行, re-export background_check/background_task) │ │
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

### 3.2 Commands 模块依赖关系 (v2.2.9)

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
//                                            ├──→ monitor/latency ──→ infra/notification
//                                            └──→ infra/lifecycle
//
//  monitor/background_task (调度层) ──→ task_manager.spawn("background_check") ──→ background_check
//
//  commands/login ──→ auth/service (full_login/full_logout 统一入口 + post_login_handler)
//                  │     └──→ auth/session ──→ auth/protocol (两步注销)
//                  │                          └──→ auth/portal (Portal 检测)
//                  ├──→ infra/events (EventBus emit_login_log)
//                  ├──→ infra/lifecycle (start_auto_exit)
//                  └──→ monitor/watcher (run_background_check，经 re-export 跳转 background_check)
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
//  耦合说明：
//    1. monitor/watcher 已瘦身为门面(51行)，检测主体迁移至 background_check.rs，
//       外部调用路径不变（通过 pub use re-export）
//    2. background_check 是核心检测主体，依赖 auth/failure_tracker/auto_auth/lifecycle/latency
//    3. emit_notification 被 auto_auth/lifecycle/latency 三处调用，是事实上的共享工具，
//       但定义在 infra/notification 模块中，语义上更清晰
//
// 所有模块通过 AppState 共享状态（见 infra/state/ 子目录），状态一致性依赖原子操作和 ArcSwap 保证
// 后台任务通过 BackgroundTaskManager 统一管理 cancel token，响应退出信号避免退出挂起
```

### 3.3 数据流

```
用户操作 → React组件 → 领域 store (useConfigStore/useAuthStore/useAdapterStore/useQualityStore) → tauriApi.invoke()
                                                                                                       ↓
                                                                                                  Tauri IPC
                                                                                                       ↓
                                                                                            #[tauri::command] Rust函数
                                                                                                       ↓
                              AppState (ConfigStore + TaskFlags + BackgroundTaskManager + NetworkState + ExitStateStore + UpdateStats[含4个原子字段: last_update_check_epoch_ms/update_notified/last_disabled_notification_ms/last_render_heartbeat_ms])
                                                                                                       ↓
                                                                                              Win32 API / HTTP / 注册表 / 文件系统
                                                                                                       ↓
                                                                                              结果返回 / 事件推送 (emit)
                                                                                                       ↓
                                                                                              tauriApi.onXxx() → useEventListeners → 领域 store → UI更新
```

---

## 四、后端模块详解 (Rust)

### 4.1 应用入口 — `main.rs` + `app/startup.rs`

**职责**: `main.rs` 仅 36 行，做三件事：注册 panic hook、构建 Tokio runtime、调用 `app::startup::run()`。所有应用初始化逻辑（Tauri 插件注册、Setup 钩子、命令注册、窗口/托盘/事件处理）实际位于 `app/startup.rs` 的 `build_runtime()` / `run()` / `setup_app()` 函数中。

**main.rs 关键流程**:

1. **panic hook**: `log_error!` 写入日志文件 + `flush_quick`（500ms超时）确保日志落盘 + `eprintln` 兜底（release 模式 `windows_subsystem=windows` 不可见但保留）
2. **`--helper` 拦截**: 主进程以管理员身份重启自身（提权执行改 MAC / 设 DNS+DoH）时附带 `--helper <op>` 参数；`helper::parse_helper_args` 在 panic hook 之后、Tauri Builder 装配之前拦截，命中则 `helper::run_helper` 执行并 `std::process::exit`（不创建窗口/托盘/监听）；参数非法时退出码 2 不启动正常 UI。详见 §4.15 helper 提权子进程
3. **Tokio runtime 构建**: `build_runtime(core_count)` 根据 CPU 核心数动态配置 `worker_threads(2-8)` 和 `max_blocking_threads(8-64)`
4. **runtime 注入 Tauri**: `tauri::async_runtime::set(handle)` 将 Tokio handle 注入 Tauri 异步运行时
5. **启动应用**: `app::startup::run(core_count)` 进入 Tauri 主循环
6. **退出清理**: flush 日志 → shutdown logger (mpsc + recv_timeout 500ms 带超时 join，B9-14 删除原固定 sleep(200ms)) → `runtime.shutdown_timeout(5s)`

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
   - 根据 `--autostart` 参数和 `hiddenStart` 配置决定是否显示窗口
   - 创建系统托盘
   - 启动适配器监控和启动任务 (通过 `run_startup_tasks`)
   - **3 秒保底 showWindow**：独立线程 3 秒后检查窗口可见性，不可见则强制 `window.show()` + `set_focus()`，最多重试3次，防止前端初始化异常导致窗口永远隐藏
   - **前端心跳监控**：独立线程每 5 秒检查 `last_render_heartbeat_ms`，连续 3 次超过 20 秒无心跳则重载 WebView
3. **WebView2 内存管理**: `on_window_event` Focused 时通过 `ICoreWebView2_19.SetMemoryUsageTargetLevel` 调节（前台 NORMAL，后台 LOW）
4. **WebView2 浏览器参数**: `build_browser_args()` 仅注入 `--js-flags=--max-old-space-size=512`（2026-09-03 精简：原 ANGLE/SkiaGraphite/DrDc/zero-copy 等 9 个参数经核验已失效/Windows 默认即开/Windows 不支持/实验性强开，一并删除交还平台默认）
5. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭，退出时使用 `force_release()` 清理任务标志
6. **退出流程**: cancel token → 短暂等待后台任务响应 → force_release 兜底 → `exit(0)`，窗口关闭与托盘退出行为统一
7. **命令注册**: 50个 `#[tauri::command]` 函数 (在 `run()` 中通过 `tauri::generate_handler!` 注册)

### 4.2 全局状态 — `infra/state/` 子目录

本模块已从单文件 `state.rs` 重构为 `state/` 子目录，按职责拆分为 4 个文件：

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

**说明**：`TaskGuard` 通过 RAII 在 `Drop` 时自动释放锁；`force_release` 标注 `#[cfg(test)]`，仅在测试编译中可用。原 `acquire_guard`/`swap_acquire` 方法已在重构中移除。

#### TaskFlags 任务标志

```rust
pub struct TaskFlags {
    pub is_checking: TaskLock,
    pub is_logging_in: TaskLock,
    pub is_logging_out: TaskLock,
    pub is_quality_checking: TaskLock,
}
```

**说明**：仅 4 个 `TaskLock` 字段，对应四类互斥任务。重构后已移除 `ArcSwap<CancellationToken>` 取消令牌字段（迁移至 `task_manager: BackgroundTaskManager` 统一管理）。

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

**说明**：`AppState.config` 由裸 `ArcSwap<Config>` 升级为 `ConfigStore`，CAS 更新逻辑内聚到此结构。原 `AppState::update_config` 方法迁移为 `ConfigStore::update`。

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
}

pub struct NetworkState { snapshot: ArcSwap<NetworkSnapshot> }

impl NetworkState {
    pub fn new() -> Self { ... }
    pub fn load(&self) -> Arc<NetworkSnapshot> { ... }   // 加载一致性快照
    pub fn update<F>(&self, f: F) where F: FnMut(&mut NetworkSnapshot) { ... }  // CAS 循环更新
    // 计数器自增便捷方法（内部走 update CAS）：
    pub fn increment_background_check_count(&self);
    pub fn increment_disconnect_reconnect_count(&self);
    pub fn increment_portal_failure_count(&self);
}
```

**说明**：重构关键点——网络状态由原先分散的 15+ 个 `AtomicBool`/`AtomicU32`/`ArcSwap<...>` 字段（旧 `NetworkStatus`）整合为单一 `NetworkSnapshot` 结构体，再通过 `ArcSwap<NetworkSnapshot>` 提供整体原子快照读写。读端一次 `load()` 获得一致性视图，写端通过 `update` CAS 循环避免竞态。

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

**说明**：原 `ExitState` 重命名为 `ExitStateStore`，与 `ConfigStore`/`NetworkState` 命名风格一致。`is_quitting` 使用 `Arc<AtomicBool>` 以便跨线程共享克隆。

#### AppState 顶层状态

```rust
pub struct AppState {
    pub config: ConfigStore,                       // 配置存储（封装 ArcSwap<Config>）
    pub tasks: TaskFlags,                          // 4 个互斥任务锁
    pub task_manager: BackgroundTaskManager,       // 后台任务统一管理（含取消能力）
    pub network: NetworkState,                     // 网络状态快照存储
    pub exit: ExitStateStore,                      // 退出状态存储
    pub update_stats: UpdateStats,                 // 更新/通知统计（4个原子字段合并子结构体）
}

pub struct UpdateStats {
    pub last_update_check_epoch_ms: AtomicU64,
    pub update_notified: AtomicBool,
    pub last_disabled_notification_ms: AtomicU64,
    pub last_render_heartbeat_ms: AtomicU64,
}
```

**说明**：wave 3c (AM-4) 将原散落在 `AppState` 顶层的 4 个原子更新/通知标志（`last_update_check_epoch_ms`/`update_notified`/`last_disabled_notification_ms`/`last_render_heartbeat_ms`）合并为子结构体 `UpdateStats`，`AppState` 现仅 6 个字段。`config`/`network`/`exit` 均为对应 Store/State 封装类型；`task_manager` 承接取消令牌职责。`AppState` 不再直接持有 `update_config` 方法，配置 CAS 更新改走 `state.config.update(...)`。

#### CommandResult / AccountResult 返回类型

```rust
#[derive(Serialize)]
pub struct CommandResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,    // 原 String，改为 Option<String>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,    // 原 String，改为 Option<String>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Config>,
}
```

**说明**：`message` 字段由 `String` 改为 `Option<String>` 并标注 `skip_serializing_if`，序列化时无消息则省略字段。`AccountResult` 标注 `rename_all = "camelCase"` 匹配前端命名约定。

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

**`Config` 结构体** (36个字段):

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `user` | String | `""` | 学号 |
| `password` | String | `""` | 密码 (内存中明文, 磁盘上DPAPI加密) |
| `operator` | String | `""` | 运营商后缀 (`""`/`"__default__"` 不拼接, `"@telecom"`/`"@unicom"`/`"@cmcc"`) |
| `adapter1` | String | `"自动检测"` | 主适配器名称 |
| `adapter2` | String | `""` | 副适配器名称 |
| `dualAdapter` | bool | false | 双适配器模式 |
| `autoLoginOnStart` | bool | true | 启动时自动登录 |
| `autoExitAfterLogin` | bool | true | 登录后自动退出 |
| `minimizeToTray` | bool | false | 关闭时最小化到托盘 |
| `hiddenStart` | bool | true | 静默启动 |
| `autoLaunch` | bool | true | 开机自启 |
| `enableBackgroundCheck` | bool | true | 启用后台检测 |
| `backgroundCheckInterval` | u64 | 15000 | 后台检测间隔 (ms) |
| `autoLoginOnPreparation` | bool | true | 登录准备模式 |
| `autoExitOnOnline` | bool | true | 检测到在线后自动退出 |
| `themeMode` | String | `"dark"` | 主题模式 |
| `enableNotification` | bool | true | 启用通知 |
| `activeAccount` | String | `""` | 当前活跃账号名 |
| `enableLatencyTest` | bool | false | 启用延迟测试 |
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
| `configVersion` | u32 | 2 | 配置版本号 |

**关键函数**:

| 函数 | 说明 |
|------|------|
| `atomic_write()` | 原子写入文件，3次重试+100ms间隔，重命名失败后删除临时文件（注：日志文案写"保留临时文件"但实际执行 `remove_file`） |
| `list_account_names()` | 共享函数，统一账号目录遍历逻辑 |
| `validate_username()` | 校验用户名 (位于 validate.rs) |
| `validate_operator()` | 校验运营商后缀 (返回 Result，非法值返回错误而非静默清空，位于 validate.rs) |
| `validate_password()` | 校验密码 (位于 validate.rs) |
| `deserialize_non_empty_or()` | 自定义反序列化器，空字符串自动回填默认值 (位于 model.rs) |

### 4.4 加密工具 — `account/crypto.rs`

Windows DPAPI 加密/解密，绑定当前 Windows 用户。`encrypt()` 无显式空输入检查，直接将 `&[u8]` 传给 `CryptProtectData`，空数据加密行为依赖 DPAPI API（返回 0 或 output 为空时返回 `Err`）。`decrypt()` 解密失败返回 `Err`。

### 4.5 网络模块 — `network/`

#### 4.5.1 缓存基础设施 — `network/client.rs`

**全局变量**:

```rust
lazy_static! {
    pub(crate) static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new(default_portal_url()));
    static ref CLIENT_POOL: DashMap<String, reqwest::Client> = DashMap::new();
}
```

> 注：原 `NetworkCache` 结构体与 `NET_CACHE` 单例已拆分 — 适配器缓存迁移到 `network/adapter_cache.rs`，网关/子网缓存迁移到 `network/subnet.rs`，Portal 状态缓存由 `auth/portal.rs` 局部管理。`client.rs` 仅保留 Portal URL 与 HTTP 客户端池两个全局变量。

**HTTP 客户端池** (`CLIENT_POOL: DashMap`, B9-17 LRU 淘汰):

- Key = `local_addr:tls_version:timeout`，池上限 `CLIENT_POOL_MAX_ENTRIES=32`，TTL `CLIENT_POOL_TTL_SECS=600`
- **LRU 淘汰策略** (B9-17)：`client_pool_get` 命中时更新 `Instant::now()`（按访问时间淘汰，非原 FIFO 按创建时间）；容量超限时 `min_by_key(Instant)` 剔除最久未访问条目
- `create_safe_http_client(timeout, local_addr)` — TLS 1.3 优先 + TLS 1.2 降级，`no-cache/no-store` 头

**关键函数**:

| 函数 | 说明 |
|------|------|
| `create_safe_http_client(timeout, local_addr)` | 创建 HTTP 客户端 (TLS 1.3 强制 + TLS 1.2 回退) |
| `update_portal_url(url)` | 更新全局 Portal URL |
| `build_client(timeout, local_addr, min_tls)` | 构建 reqwest::Client (no_proxy + limited(5) 重定向 + 3s connect_timeout) |
| `client_pool_key(local_addr, min_tls, timeout)` | 生成客户端池 Key |

#### 4.5.2 适配器查询 — `adapter.rs` (薄 re-export 模块)

`adapter.rs` 已重构为薄 re-export 模块，保留适配器选择逻辑，其余功能迁移到子模块（通过 `pub use` re-export 保持外部调用方不变）:

**adapter.rs 本地保留函数** (适配器选择):

| 函数 | 说明 |
|------|------|
| `find_by_name()` | 按名称查找适配器 |
| `find_with_valid_ip()` | 按名称查找具有有效 IP 的适配器 |
| `find_dual_adapters()` | 查找双适配器 (a1, a2)，a2 仅在 dual_adapter 且名称非空且与 a1 不同时查找 |
| `is_secondary_adapter_enabled()` | 副适配器是否启用 (dual_adapter && adapter2 非空) |
| `resolve_adapter_names()` | 解析主/副适配器名称 (支持自动检测：优先有线→任意有IP→首个) |
| `select_adapter()` | 选择适配器并返回 (ip, name) |
| `ensure_ethernet_ip_for_login()` | 登录前确保以太网 IP (DHCP 续租兜底) |

**re-export 来源**:
- `network::discovery` — `Adapter`/`AdapterDetail`/`DisabledAdapter` 类型 + `is_blacklisted`/`new_command` + Win32 API `GetAdaptersAddresses` 查询 + 适配器状态四分类
- `network::adapter_cache` — `get_adapters_force`/`validate_adapter_name`/`poll_adapter_ip_quick` + TTL 5秒缓存
- `network::dhcp` — `dhcp_renew_wired_only`/`dhcp_release_renew_all`/`dhcp_release_renew_single`/`apply_mac_change_via_registry`
- `network::subnet` — `get_wireless_ssid`/`get_wired_network_profile`/`check_gateway_reachable`/`check_gateway_reachable_from`/`is_same_subnet_18`

**适配器状态四分类** (`AdapterStatus` 枚举，定义在 `network/discovery/`):
  - `Disabled` — 已禁用（OperStatus Down/NotPresent，管理员禁用或硬件缺失）
  - `Disconnected` — 未连接（OperStatus LowerLayerDown/Dormant，线缆未插或USB网卡未连接）
  - `EnabledNoIp` — 未禁用无IP（OperStatus Up 但无有效 IP，含 169.254 APIPA 清空后）
  - `Connected` — 已连接（OperStatus Up 且有有效 IP）

**连接速度 (LinkSpeed)**: `Adapter`/`AdapterDetail` 新增 `linkSpeed` 字段（u64 bit/s，0 表示未知），直接读 `IP_ADAPTER_ADDRESSES.ReceiveLinkSpeed`（无需额外 API）。前端 NetworkPanel 适配器卡片展示格式化后速度（Gbps/Mbps）。

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
- v 参数使用 `random_v()` 随机生成
- NAT 内网 IP 检测，NAT 环境下不发送 `wlan_user_ip`
- `PortalStatus` 新增 `error_kind` 字段区分"请求失败"与"Portal不可达"
- **页面探测禁止走 :801** (2026-09-03)：v2.2.x 曾把页面探测端口强制对齐 :801（与协议请求一致），但校园网 801 是 Dr.COM EPortal 管理系统前端（`/eportal` SPA 登录表单），浏览器实测已在线状态下打开仍渲染登录页、HTML 无任何状态特征 → 旧 Dr.COM 特征永远失配，误报"Portal 页面无法判断登录状态，请手动确认"。修复：`check_portal_page` 改回请求配置的原始地址（默认 80 端口 Dr.COM 网关页，GBK 编码但 `Dr.COMWebLoginID`/`uid='`/`v4ip='` 等 ASCII 特征可正常匹配，渲染结果为"您已经成功登录。"+注销按钮）；登录/注销等协议请求仍强制 :801（`ensure_portal_port`，两者端口本就不同）。
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

每次请求独立生成 1000-9999 随机4位数 v 值，统一应用于登录、注销、Portal 检测。

**登录函数**:

| 函数 | 说明 |
|------|------|
| `do_login_with_retry()` | 登录请求+重试(重试次数由调用方传入)，重试等待可中断(每100ms检查退出标志) |

**注销函数** (两步注销):

| 函数 | 说明 |
|------|------|
| `do_logout_request()` | 两步注销：① Radius注销 ② MAC解绑 |
| `do_logout_with_retry()` | 注销重试(重试次数由调用方传入)，重试等待可中断 |
| `parse_logout_result()` | 注销结果解析 (JSONP)，支持多种成功条件 |

**两步注销流程** (2轮循环，每轮先 MAC 解绑再 Radius 注销，callback 动态生成):

```
第1轮 (round=1): unbind_cb=dr1002, logout_cb=dr1003
  步骤A: MAC 解绑
    GET /eportal/portal/mac/unbind?callback=dr1002
        &user_account={学号}&wlan_user_mac=000000000000
        &wlan_user_ip={IP整数}&jsVersion=4.1.3&v={random}&lang=zh
    成功: result=0, msg="解绑终端MAC成功！"
  步骤B: Radius 注销
    GET /eportal/portal/logout?callback=dr1003&login_method=1
        &user_account=drcom&user_password=123&ac_logout=1
        &register_mode=1&wlan_user_ip={IP}&wlan_user_mac=000000000000
        &jsVersion=4.1.3&v={random}&lang=zh
    成功: result=1, msg="Radius注销成功！"

第2轮 (round=2): unbind_cb=dr1003, logout_cb=dr1004
  (重复步骤A+B，callback 值递增)
```

> 注：`unbind_cb = format!("dr100{}", round + 1)`，`logout_cb = format!("dr100{}", round + 2)`，每轮 callback 递增。任一轮 Radius 注销或 MAC 解绑成功即标记 `any_radius_ok`/`any_unbind_ok`。

**注销成功判定**:
- 两步均成功 → 注销成功
- Radius 注销成功 + MAC 解绑失败 → "Radius注销成功，MAC解绑失败"
- `/logout` 接口 `result=1` 表示 Radius 注销成功
- `/mac/unbind` 接口 `result=0` 且 msg 含"解绑终端MAC成功"表示解绑成功
- `result=0` 但 msg 含错误关键词（"非法"/"失败"/"错误"/"拒绝"）→ 失败

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

#### 4.5.5 网络质量检测 — `quality.rs`

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

**v2.2.5 优化**:

| 优化项 | 说明 |
|--------|------|
| HTTPS 恢复绑定适配器 (2026-09-03) | v2.2.5 的"HTTPS 不绑定适配器"在双网卡场景失效：系统默认路由选中未认证网卡（如 WLAN）时全部 HTTPS TLS 握手超时（实测 14/19 项失败）。现 HTTPS 测试绑定经 Portal 认证的适配器 IP（`ctx.bind_addr`），与网关/DNS/DoH 测试一致 |
| DNS 解析优先 IPv4 | `resolve_host_uncached_with_bind` 中优先返回 `is_ipv4()` 的结果，避免 IPv6 地址导致连接失败 |
| 增量推送 | `app_handle: Option<&AppHandle>` 参数，Phase 1 和 HTTPS 批次完成后立即 emit，前端逐步填充数据 |
| HTTPS 分批并发 | Phase 2 改为每批 4 个分批并发，减少校园网高 RTT 环境下 TLS 带宽竞争 |
| 前端不再主动触发 | 移除前端 `qualityPromise`，由后端 latency loop 统一管理质量检测时机 |
| 前端防抖移除 | 移除 500ms 防抖，增量推送事件可立即更新 UI |
| 启动延迟 1 秒 | latency loop 启动后先 sleep 1s 再开始检测，避免网络未稳定时 HTTPS 延迟异常 |
| RAII guard 替代手动锁 | `is_quality_checking.try_acquire()` 返回 `TaskGuard`，作用域结束自动释放，替代 `swap_acquire + force_release` |
| DNS/DoH Sleep 优化 (v2.2.6) | setup_dns_doh PowerShell路径 sleep 2s→1.5s，cmd路径 3s→2s (注：原 enable_doh_for_dns 命令已合并入 setup_dns_doh 并删除) |
| 移除 15s 冷却机制 (v2.2.6) | 删除 last_quality_check_time 字段及 latency.rs/network_cmd.rs/watcher.rs 中的冷却检查，首次检测可立即执行 |

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

**Per-Profile DNS 设置**:

| 函数 | 说明 |
|------|------|
| `set_profile_dns_via_api()` | 使用 `DNS_SETTING_PROFILE_NAMESERVER` (0x0200) 设置配置文件级 DNS，仅对当前 WiFi 生效 |
| `clear_adapter_dns_via_api()` | 清除适配器级 DNS (`NameServer`)，使配置文件级 DNS 生效 |
| `set_dns_via_api()` | 适配器级 DNS+DoH 设置（原有函数，有线适配器使用） |
| `set_doh_via_api()` | 适配器级 DoH 设置（仅设置 DoH，不修改 NameServer） |

**DNS 检测增强**: `read_adapter_dns_from_registry()` 同时读取 `NameServer`（适配器级）和 `ProfileNameServer`（配置文件级），source 优先级为 manual > profile > dhcp，输出 `dnsSource`/`profileDnsServers`/`adapterDnsOverridesProfile` 字段

### 4.6 DNS 智能解析 — `network/dns.rs + network/timing.rs`

> **位置说明**: DNS 评分系统（`DNS_SERVER_SCORES`/`DOH_SERVER_SCORES`/`DnsServerScore`/`DohServerScore`）、评分更新/查询函数、DoH 解析函数（`resolve_via_doh`/`resolve_host_smart`/`resolve_host_uncached_with_bind` 及辅助函数）均位于 `network/dns.rs`。HTTP 计时函数（`measure_https_timing`/`measure_dns_query`/`measure_doh_timing`）位于 `network/timing.rs`。

#### DNS 服务器动态评分系统

```rust
static ref DNS_SERVER_SCORES: DashMap<String, DnsServerScore>;
static ref DOH_SERVER_SCORES: DashMap<String, DohServerScore>;

struct DnsServerScore { latency_ms: i64, success: bool, last_tested: Instant }
struct DohServerScore { latency_ms: i64, success: bool, last_tested: Instant }
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
- 第二级: DoH + 传统 DNS 并发竞速，使用延迟最优的服务器，首个成功即返回并缓存
- 第三级: 自定义 DNS 失败时自动回退到系统 DNS (`ResolverConfig::default()`)

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
| `post_login_handler()` | `auth/service.rs` | 登录后处理 (AM-13 从 commands/login.rs 下沉)：解除注销保护期 → 延迟500ms触发 `monitor::watcher::run_background_check` → 按需启动 `auto_exit` |
| `check_any_adapter_online()` | `commands/login.rs` | **B9-10 并行化**：双适配器 Portal 在线检测从串行改为 `std::thread::scope` 并行，双适配器检测延迟减半；`do_logout` 复用其逐适配器检测结果避免重复 HTTP 请求 |

**注销命令** (`commands/login.rs` 委托 `auth/service.rs`):

| 函数 | 所在模块 | 说明 |
|------|----------|------|
| `do_logout(adapter_name?)` | `commands/login.rs` | Tauri 命令，支持可选指定适配器 |
| `full_logout()` | `auth/service.rs` | 注销核心逻辑 (双适配器串行) |
| `logout_adapter_with_log()` | `auth/service.rs` | 单适配器注销+日志 |

**锁语义**: 登录使用 `is_logging_in`，注销使用独立的 `is_logging_out`

**适配器解析** (直接函数调用，无 trait 抽象):

> 注：`auth/traits.rs` 整个文件已在重构中删除（不仅删除 `DefaultAdapterResolver`，连 `AdapterResolver` trait 与 `MockAdapterResolver` 一并移除）。`auth/service.rs` 现直接调用 `crate::network::resolve_adapter_names` 与 `crate::network::find_dual_adapters` 等自由函数，不再经过 trait 抽象。原 `PortalChecker` / `ProtocolClient` trait 及其 impl/mock 早在 v2.2.8 已删除（dead code）。

**双适配器并行执行** (`auth/dual_adapter_executor.rs`，158 行含测试, B9-7 泛型化):

| 项 | 说明 |
|------|------|
| `DualAdapterResult` | 双适配器执行结果结构体 (`primary`/`secondary` 两个 `Option<CommandResult>`) |
| `execute_dual<F1, F2>(a1_action: F1, a2_action: F2, is_quitting)` | 双适配器并行执行器：**B9-7 将 `Box<dyn FnOnce>` 改为泛型 `F1`/`F2` 静态分发**（`F: FnOnce() -> Option<CommandResult> + Send + 'static`），消除堆分配与虚函数调用；适配器1立即 `spawn_blocking`，适配器2通过 10×100ms 轮询 `is_quitting` 实现可中断 1s 错峰；2 处调用点（service.rs 的 `full_login` + `full_logout`）去 `Box::new` 直接传闭包 |

**认证失败计数与 Portal 请求失败容错** (`auth/failure_tracker.rs`, 9c B9-5 合并原 `monitor/portal_failure.rs`):

| 函数 | 说明 |
|------|------|
| `is_auth_failure()` | 判断 CommandResult 是否为认证失败 (`AUTH_FAILURE_CODES: ["ac_auth_failed","1","4"]`) |
| `update_auth_failure_count()` | 单适配器认证失败计数，连续5次触发 MAC 重置+DHCP 续租 |
| `update_dual_adapter_auth_failure()` | 双适配器分别计数，各自5次触发单适配器 MAC 重置 |
| `handle_portal_request_failure()` | **9c 从 portal_failure.rs 迁入**：Portal HTTP 请求失败容错，`PORTAL_REQUEST_FAILURE_THRESHOLD=5`；网关不可达时跳过计数并重置（校园网断网/维护期避免误重置 MAC），达阈值触发 `dhcp_release_renew_single` |
| `reset_all()` | 重置所有认证失败计数器 |

> `AdapterFailureCounter` 枚举 (A1/A2) 统一认证失败与 Portal 请求失败的计数访问器 (`get/set/increment_adapter_failure_count`)。

**注销成功后状态重置** (v2.2.5 区分全量/单适配器):

- **全量注销**（未指定 `adapter_name`）：重置 `any_adapter_online`/`last_a1_online`/`last_a2_online`/`has_logged_online` 为 false，`disconnect_reconnect_count` 归零，重置 `last_auto_login_attempt` 为当前时间，取消自动退出倒计时，设置 60 秒注销保护期 (`logout_protected_until`)
- **单适配器注销**（指定 `adapter_name`）：仅重置对应适配器的 `last_a1_online` 或 `last_a2_online`，重新计算 `any_adapter_online = a1 || a2`，其余标志保持不变

### 4.8 后台巡检 — `monitor/` (watcher 门面 + background_check 主体 + background_task 调度)

> **重构说明**：原 `watcher.rs` 大文件已按职责拆分。`watcher.rs` 现仅 51 行，作为门面 re-export `background_check`/`background_task`，并提供 `run_startup_tasks` 启动聚合入口。检测主体迁移至 `background_check.rs`，任务调度迁移至 `background_task.rs`。Portal 失败容错原位于 `portal_failure.rs`，9c 阶段 (B9-5) 已合并入 `auth/failure_tracker.rs`（统一失败计数入口）。外部调用路径（`monitor::watcher::run_background_check` 等）通过 re-export 保持不变。

#### 4.8.1 watcher.rs — 门面 + 启动聚合 (51 行)

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

#### 4.8.2 background_check.rs — 后台检测主体 (~320 行)

**职责**：一次完整后台检测周期：获取适配器列表 → 解析双适配器名 → 校园网环境验证 → Portal 检测（含双适配器并行）→ Portal 失败容错 → 状态更新与事件下发 → 自动登录/断开重连触发 → 网络质量检测调度。

**关键函数**：

| 函数 | 说明 |
|------|------|
| `run_background_check_blocking(app_handle, state, cancel_token) -> Option<(String,String)>` | 同步主体 (~290 行)，返回 `Some((adapter_name, adapter_ip))` 表示需继续网络质量检测 |
| `run_background_check(app_handle, cancel_token)` | async 包装：`spawn_blocking` 跑 `run_background_check_blocking`，成功后调用 `run_quality_check` |

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
| `handle_portal_request_failure()` | `auth/failure_tracker.rs` | Portal 请求失败容错（9c 从 portal_failure.rs 迁入，见 4.7） |
| `build_adapter_details()` / `handle_status_change()` / `emit_background_check_result()` / `update_network_state()` / `adapter_status_entry()` 等 | `background_emit.rs` | 适配器详情/状态变更/检测结果事件/网络状态更新/状态条目构建 |
| `check_campus_network()` | `campus_check.rs` | WiFi/有线分别检测校园网状态 |
| `run_quality_check()` | `quality_scheduler.rs` | 质量检测调度 |

**双适配器并行 Portal 检测**：`run_background_check_blocking` 使用 `tauri::async_runtime::spawn_blocking` + `tokio::join!` 并行检测双适配器 Portal 状态（替代原 `std::thread::scope` 方案，与 `dual_adapter_executor` 实现策略一致）。v2.2.7 历史：原 `std::thread::scope` 子线程无 Tokio reactor 上下文导致 `block_on_http` panic，曾通过 `Handle::current()` + `h.enter()` 设置上下文修复；当前 `spawn_blocking` 方案天然具备 reactor 上下文，无需手动 enter。

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

> 注：`monitor/mod.rs` 将 `start_background_check_inner` re-export 为 `trigger_background_check`（统一触发入口别名）。

#### 4.8.4 Portal 请求失败容错 — 已迁入 `auth/failure_tracker.rs` (9c B9-5)

> **迁移说明**：原 `monitor/portal_failure.rs` (~90 行) 已在 9c 阶段 (B9-5) 整体合并入 `auth/failure_tracker.rs`，统一失败计数入口。`monitor/portal_failure.rs` 文件已删除，`monitor/mod.rs` 子模块从 11 个降为 10 个。逻辑详见 4.7 章节 `handle_portal_request_failure()`。

**Portal 容错完整链路** (v2.2.5 新增, v2.2.7 增强, 9c 迁入 failure_tracker)：

1. 主/副适配器 Portal 请求失败（`is_request_failed: true`）时，对应适配器 `a1_auth_failure_count`/`a2_auth_failure_count` 自增（CAS 更新 NetworkSnapshot）
2. 失败时先检查网关从该适配器IP是否可达（`check_gateway_reachable_from()`），不可达则跳过计数并重置（校园网断网/维护期避免误重置 MAC）
3. 连续 5 次失败（`PORTAL_REQUEST_FAILURE_THRESHOLD=5`）触发 `dhcp_release_renew_single`（仅对该失败适配器 MAC 重置 + DHCP 续租，仅对校园网子网适配器生效）
4. 触发后重置计数器为 0
5. Portal 检测恢复正常（`Success`）时 CAS 写入 0 重置计数器并记录原值日志

> 9c 合并后，`auth/failure_tracker.rs` 统一管理**认证失败**（`AUTH_FAILURE_CODES: ["ac_auth_failed","1","4"]`）与 **Portal HTTP 请求失败**两类计数，共用 `AdapterFailureCounter` 枚举 (A1/A2) 与计数访问器（`get/set/increment_adapter_failure_count`）。

**量化改进** (职责分离重构)：

| 指标 | 重构前 (watcher.rs) | 当前 (background_check.rs) |
|------|---------------------|---------------------------|
| `run_background_check_blocking` 行数 | ~190 行（单文件） | ~290 行（含 Portal 容错调用） |
| 重复 JSON 构建代码 | 3 处 | 0 处 |
| watcher.rs 总行数 | ~337 行 | 51 行（门面） |

### 4.9 自动登录模块 — `monitor/auto_auth.rs`

**公开函数**:

| 函数 | 说明 |
|------|------|
| `try_auto_login_on_preparation()` | 准备阶段自动登录 (60秒冷却)，`has_logged_online` 为 true 时跳过 |
| `try_disconnect_reconnect()` | 断线重连 (最多3次 + 间隔提醒) |
| `run_auto_login_on_start()` | 启动时自动登录 (条件延迟：自启场景5s/非自启1.5s + Portal预检 + 无网络保护：配置适配器无IP时跳过校园网退出) |

### 4.10 自动退出模块 — `infra/lifecycle.rs`

**关键常量**:

| 常量 | 值 | 说明 |
|------|----|------|
| `CAMPUS_MINIMIZE_DELAY_MS` | 30000 | 校园网退出最小化延迟 (不变) |
| `CAMPUS_EXIT_DELAY_MS` | 60000 | 校园网退出总延迟 (不变) |

| 函数 | 说明 |
|------|------|
| `start_auto_exit()` | 启动自动退出倒计时 + 快捷键注册 + 通知 |
| `cancel_auto_exit_inner()` | 取消自动退出 |
| `start_campus_exit()` | 校园网验证不通过时：30s后最小化到托盘，再30s后强制退出 (受 `campus_exit_on_fail` 控制)。**先 CAS 防止重复触发，成功后再设置 deadline**，避免 `campus_exit_started` 永久卡死 (原实现先 set_deadline 再 CAS 会导致周期性调用时 deadline 被推后、CAS 失败后运行中任务最终校验 deadline 未到期而 return，标志位永久 true) |
| `cancel_campus_exit()` | 取消校园网退出流程。如果自动退出未运行，注销快捷键 |
| `cancel_campus_exit_with_notification()` | 快捷键取消校园网退出 (含通知和快捷键注销) |
| `shutdown_and_exit()` | 统一退出入口 (async)：设置 `is_quitting` → `task_manager.shutdown()` 清理后台任务 → `app_handle.exit(0)`。被 `start_campus_exit` 和 `start_auto_exit` 共同调用 |

**B9-8 TOCTOU 竞态修复** (9c)：`start_campus_exit`/`cancel_campus_exit`/`cancel_campus_exit_with_notification` 3 处 `auto_exit_deadline` 的 check-then-act（`is_none()` 检查 + `try_unregister_cancel_exit_shortcut`）原跨锁边界存在 TOCTOU 竞态。修复：3 处均改为持有 `auto_exit_deadline` 锁覆盖 check-then-act，在同一锁临界区内完成 `is_none()` 检查与 unregister，防止 `start_auto_exit` 在间隙注册快捷键。

### 4.11 延迟测试模块 — `monitor/latency.rs`

| 函数 | 说明 |
|------|------|
| `notify_network_quality_change()` | 网络质量变化通知 (bad/good 级别切换)，由后端统一发送，前端不再重复调用 `sendNotification` |
| `spawn_latency_test_loop()` | 启动延迟测试循环 (CancellationToken) |

**v2.2.5 改进**:

| 改进项 | 说明 |
|--------|------|
| 启动延迟 1 秒 | 循环开始前 `sleep(1s)`，避免网络未稳定时 HTTPS 测试延迟异常 |
| RAII guard | `is_quality_checking.try_acquire()` 返回 `TaskGuard`，作用域结束自动释放，替代手动 `swap_acquire + force_release` |
| 增量推送 | 传递 `Some(&app_handle)` 给 `check_network_quality_async`，启用 Phase 1 + HTTPS 批次增量推送 |
| 后端统一通知 | `notify_network_quality_change` 在后端发送网络质量变化通知，前端不再主动调用 `sendNotification` |
| 移除 15s 冷却 (v2.2.6) | 删除 last_quality_check_time 字段及冷却检查逻辑，首次检测可立即执行 |
| 未在线跳过 (2026-09-03) | `spawn_latency_test_loop` 每轮检查 `any_adapter_online`，Portal 未认证时跳过自动检测（未认证时外网 HTTPS 必被拦截、全超时且误报"网络拥堵"）；前端手动触发的 `check_network_quality` 命令不受限 |

### 4.12 适配器监控模块 — `monitor/adapter_watch.rs`

| 函数 | 说明 |
|------|------|
| `start_adapter_watch()` | 启动适配器状态监控循环 (15s间隔，CancellationToken可退出)，适配器恢复时触发重新检测，禁用适配器通知节流(60s内不重复) |

### 4.13 网络命令模块 — `commands/network_cmd.rs`

**DNS/DoH 检测与设置** (委托 `platform/dns_config.rs`):

| 函数 | 说明 |
|------|------|
| `check_portal_status()` | 检测 Portal 认证状态（注销保护期内直接返回离线） |
| `check_campus_status()` | 检测校园网状态，返回 campusWifi/campusWired 字段 |
| `check_dns_doh_status()` | 通过 winreg 读取注册表检测 DNS/DoH 状态 |
| `setup_dns_doh()` | 一键设置推荐 DNS + DoH (WiFi用配置文件级DNS，有线用适配器级DNS；管理员直调 `dns_setup::setup_dns_doh_admin`，非管理员经 `--helper` 提权重启自身) |

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

**config_cmd.rs** — 配置保存/加载 (委托 `config/persist.rs`)，空密码兜底逻辑 (前端未传密码且旧密码存在时保留旧密码)；`save_config` 可选参数 `clear_password`（2026-09-03）：显式为 true 时跳过兜底强制置空密码，供账号面板"清除密码"使用（前端经 `saveConfig(cfg, clearPassword)` / `saveConfigDirect(cfg, clearPassword)` 透传）

**account.rs** — 多账号管理 (委托 `account/mod.rs`)，使用 `list_account_names()` 共享函数，切换账号仅替换账号相关字段保留启动设置，删除账号前检查并清空 `active_account`

**system.rs** — 系统功能命令，`get_init_data` 手动遍历 accounts 目录获取账号列表（与 `list_account_names()` 逻辑重复，未复用），新增返回字段 `gpuInfo`/`refreshRate`；新增 `append_login_history()` 登录历史记录（最多100条）

**updater.rs** — 更新命令 (委托 `update/updater.rs`)，SHA256 校验和全 4xx 缺失时**默认拒绝安装**（需 `skipSha256WhenMissing`，无前端开关；5xx/传输错误/哈希不匹配一律拒绝），MSI 安装使用 `raw_arg` 支持含空格路径；`get_mirror_urls` 镜像 URL **原样拼接不做百分号编码**（2026-09-03：gh-proxy.com 对整体编码形式返回 403，与 updater.rs 的 sha256 镜像拼接方式保持一致）

### 4.15 提权辅助子进程 — `helper/` (--helper 模式)

**动机**: 需要管理员权限的操作（改 MAC / 设 DNS+DoH）此前在非管理员下提权执行 PowerShell 脚本（`Set-NetAdapter`/`Set-DnsClientServerAddress`）。自 2.4.0 起改为**提权重启自身**：以管理员身份启动当前 exe 并附加 `--helper <op>`，由 Rust 直调 Win32/winreg 完成操作，彻底移除 PowerShell 依赖（含 `-EncodedCommand` Base64 编码与 `escape_ps_single_quote`）。

**执行流**:
1. **主进程** (`platform/helper_spawn.rs::spawn_elevated_helper`)：`std::env::current_exe()` 取自身路径，生成唯一结果文件路径（`%TEMP%/campus-login-helper-<pid>-<ts>.json`），拼参数 `--helper <op> ... --result <path>`，按现有降级链提权启动（COM ICMLuaUtil 静默 → 失败 ShellExecuteW runas 弹 UAC）
2. **helper 进程** (`main.rs` 顶部拦截)：`helper::parse_helper_args` 解析出 `HelperOp`（`Dns` / `Mac{guid, mac_no_dash}`），`run_helper` 执行：
   - `Dns` → `network::dns_setup::setup_dns_doh_admin()`（枚举活跃适配器 → Win32 设置 → 全局 DoH 注册 → flushdns）
   - `Mac` → 按 GUID 在 `get_adapters_force` 中解析适配器名 → `dhcp::apply_mac_change_via_registry`（写注册表 NetworkAddress + release/disable/enable/renew）
3. **结果回传**: helper 把 `HelperResult{success, message, op, logs}` 原子写入结果文件（tmp + rename），主进程 100ms 间隔轮询（DNS 超时 30s / MAC 超时 25s），读取后把 `logs` 并入主进程日志，返回 JSON 结果

**要点**: helper 进程不初始化 logger（避免与主进程跨进程写同一日志文件竞争）；参数仅含 GUID/MAC/结果路径等受控字符，适配器名由 helper 自行枚举，无 shell 拼接注入面。

---

## 五、前端模块详解 (React/TypeScript)

> **架构说明**: 前端采用业务域分目录架构，每个业务域目录包含面板组件、逻辑 Hook、类型定义和模块导出。类型定义分散在各业务域的 `types.ts` 中，而非集中在一个 `types/index.ts` 文件。

### 5.1 状态管理架构 — 领域 store 拆分 (3b 阶段 AM-7)

> **重构说明**：原单体 `useAppStore.ts` 已按领域拆分为 5 个独立 store + 1 个日志/Toast store。`useAppStore.ts` 现仅 3 行 re-export 兼容壳，无任何自身状态。所有 store 基于 zustand ^5.0，`localStorage` 已替换为 `safeStorage`（内存降级封装），避免隐私模式下 localStorage 不可用。

**领域 store 一览**：

| Store | 文件 | 职责 | 关键 state | 关键 action |
|-------|------|------|-----------|-------------|
| `useConfigStore` | `useConfigStore.ts` (121行) | 配置/账号/语言 + 防抖保存 + 密码掩码 | `config`/`passwordSaved`/`accounts`/`activeAccount`/`language`/`api` | `updateConfig`(500ms防抖)/`updateConfigLocal`/`syncPasswordSaved`/`saveConfigDirect`/`setLanguage` |
| `useAuthStore` | `useAuthStore.ts` (272行) | 登录/注销/在线检测/后台状态 | `isLoggingIn`/`isLoggingOut`/`status`/`bgStatus` | `doLogin`/`doLogout`/`checkOnline`/`setStatus`/`setBgStatus` |
| `useAdapterStore` | `useAdapterStore.ts` (53行) | 适配器列表/详情/面板 | `adapters`/`disabledAdapters`/`adapterDetails`/`isRefreshingAdapters`/`activePanel` | `refreshAdapters`/`setAdapters`/`setActivePanel` |
| `useQualityStore` | `useQualityStore.ts` (76行) | 网络质量/DNS DoH/更新/GPU | `networkQuality`/`dnsDohStatus`/`dnsChecking`/`isRefreshingQuality`/`updateAvailable`/`latestVersion`/`releaseNotes`/`gpuInfo`/`refreshRate` | `refreshQuality`/`setNetworkQuality`/`setDnsDohStatus`/`setUpdateAvailable`/`setGpuInfo` |
| `useThemeStore` | `useThemeStore.ts` (81行) | 主题/亮暗/自定义色 + DOM 副作用 | `themeName`/`isLightMode`/`customThemeColor` | `setThemeName`/`setIsLightMode`/`initTheme`/`setCustomThemeColor` |
| `useLogToastStore` | `useLogToastStore.ts` | 日志/Toast (独立 zustand，MAX_LOG_ENTRIES=300；Toast 上限 MAX_TOASTS=4，`addToast`/`addToastWithAction` 同 title 去重——同一条业务事件经"专用事件 + system-notification"双通道各弹一次时只保留先到的) | `logs`/`toasts` | `addLog`/`addToast`/`addToastWithAction`/`removeToast`/`removeToastsByPrefix` |

> **通知单通道规范 (2026-09-03 重构)**：一条通知只有一个来源、一个通道、一个文案源，杜绝双通道重复。
> - `emit_notification`（`infra/notification.rs`）**只发 Windows 系统通知**（应用非前台 + `enable_notification` 时），不再向前端发 `system-notification` 事件（`EventBus.emit_system_notification` 已删除）；系统通知文案为中文硬编码（后端无法感知前端 UI 语言，为已知边界）
> - 应用内 toast/日志由**业务专用事件**负责：`onAutoLoginResult`（登录成功/失败）、`onAutoExitCountdown`（即将退出+取消按钮）、`onAutoExitCancelled`、`onCampusExitCountdown`/`onCampusExitCancelled`（校园网退出/取消+按钮）、`onNetworkQualityResult`→`handleQualityBadAlert`（质量告警）、`onLoginLog`（过程告警日志：检测到断线/重连失败/网络仍断线/网络拥堵/恢复——原 emit_notification 调用点已补 `emit_login_log`）、`onUpdateAvailable`（发现新版本）
> - store 防护：`MAX_TOASTS=4` + 同 title 去重 + 超限淘汰时清理定时器；窗口非前台时普通 toast 不入队（信息由日志兜底），带 action 的 toast 仍入队（承载取消退出操作入口）
> - 通知文案 i18n：专用通道统一走 `i18next.t('notify.*')`；`enable_notification` 开关仅控制系统通知，应用内 toast 不受影响（设置面板描述已注明）
| `useAppStore` | `useAppStore.ts` (3行) | **兼容壳**，仅 re-export `useAppInit`/`hasPendingConfig`/`flushPendingConfig` | 无 | 无 |

**密码处理** (迁移至 `useConfigStore`)：`password === PASSWORD_MASK` 时两层防护——`updateConfig` 合并挂起配置时若旧挂起有真实密码但新 partial 传 MASK，保留旧挂起真实密码；`flushPendingConfig` 最终合并时若 password 仍是 MASK 则 `delete`，让后端识别 MASK 并保留原密码。

**刷新锁统一模式**：各领域 store 均采用模块级 `_xxxLockFlag` + `setTimeout(..., 500)` 的统一防抖锁模式：
- `useConfigStore`：`saveConfigTimer` + `saveConfigPending`（500ms 防抖保存）
- `useAuthStore`：`_checkOnlineLockFlag` + `checkOnlineEpoch`（防竞态 + 防旧请求覆盖）
- `useAdapterStore`：`_adapterLockFlag`
- `useQualityStore`：`_qualityLockFlag`

**主题 DOM 副作用** (迁移至 `useThemeStore`)：store 模块底部 `subscribe` 监听 `isLightMode`/`themeName`/`customThemeColor` 变化，toggle `dark` class、添加 `theme-${name}` 类、计算 CSS 变量 `--primary`/`--ring`/`--accent`。

> 注：`activePanel` 归 `useAdapterStore` 管理（历史归位，语义上与适配器关联较弱但实际如此）。`i18next.t()` 在 action 函数体内调用（非 Store 创建时），避免初始化时序问题。

### 5.2 IPC 封装 — `hooks/tauriApi.ts` (原 useIpc.ts)

> **重命名**：`useIpc.ts` 已重命名为 `tauriApi.ts`（commit e06203d），从 hook 风格转向纯 API 模块（无 React 依赖）。

**导出**：`tauriApi: TauriApi`（默认对象，~50 个 invoke 方法 + 15 个事件监听器工厂）、`tauriApiWithRetry: TauriApi`（对 3 个易失败命令包一层 `withRetry`）。

**事件监听器** (16 个，均通过 `createEventListener<T>(eventName)` 工厂创建，返回取消函数):

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
| `onSystemNotification` | `system-notification` |
| `onUpdateAvailable` | `update-available` |
| `onDownloadProgress` | `update-download-progress` |
| `onConfigChanged` | `config-changed` |

**`createEventListener` 竞态处理**：闭包维护 `cancelled`/`unlisten` 双状态，处理"订阅尚未完成时即被取消"的竞态；取消函数若 `unlisten` 已就绪则直接调用，否则挂到 `listenPromise.then(fn => fn?.())` 延后清理。

**API 清单** (`TauriApi` interface 定义 ~50 个 API，按领域分组):

| 领域 | API |
|------|-----|
| 配置 | `getConfig` / `saveConfig` / `getInitData` |
| 适配器 | `getAdapters(force?)` / `getDisabledAdapters` / `enableAdapter` / `getAdapterDetails` |
| Portal/校园网 | `checkPortalStatus(adapterIp)` / `checkCampusStatus` |
| 登录 | `doLogin(adapterName?)` / `doLogout(adapterName?)` |
| 窗口 | `minimizeWindow` / `closeWindow` / `showWindow` |
| 账号 | `listAccounts` / `switchAccount` / `saveCurrentAsAccount` / `deleteAccount` / `getActiveAccount` |
| 后台检测 | `startBackgroundCheck` / `stopBackgroundCheck` / `triggerBackgroundCheck` / `getBackgroundStatus` |
| DHCP | `dhcpRenewAll` / `dhcpReleaseRenew` / `dhcpReleaseRenewAdapter` |
| 网络质量 | `checkNetworkQuality` / `startLatencyTest` / `stopLatencyTest` |
| 系统集成 | `openExternal` / `getAutoLaunch` / `setAutoLaunch` / `getNotificationEnabled` / `setNotificationEnabled` / `sendNotification` / `cancelAutoExit` |
| 日志/调试 | `getLogs(lines?)` / `clearLogs` / `getDebugMode` / `setDebugMode` / `getLogRetentionDays` / `setLogRetentionDays` |
| 更新 | `checkUpdate` / `downloadUpdate` / `installUpdate` / `getMirrorUrls` |
| DNS DoH | `checkDnsDohStatus` / `setupDnsDoh` |
| 其它 | `renderHeartbeat` / `getGpuInfo` |

**重试机制** (`tauriApiWithRetry`)：`isRetryableError` 判断消息含 `timeout`/`network`/`fetch`/`connection` 之一；`withRetry(fn, maxRetries=2, baseDelay=500)` 指数退避 `baseDelay * 2^attempt + random(0..200)` ms，最多重试 2 次（共 3 次尝试）。仅对 `saveConfig`/`checkPortalStatus`/`checkNetworkQuality` 三个命令包装。

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

mount 时注册全部 Tauri 事件监听器与窗口关闭拦截，unmount 时统一清理。注册 15 个事件订阅 + 1 个窗口关闭拦截：

- `getCurrentWindow().onCloseRequested` — 拦截关闭，若有 pending config 先 `flushPendingConfig()`，等 300ms 再关闭
- `onBackgroundCheckResult` — 更新 `bgStatus`、记录在线/离线日志（1s 节流 + 5s 在线日志节流）
- `onAdaptersChanged` — 更新 store，500ms 节流（前缘+后缘双重保护）
- `onAdapterDetailsChanged` / `onDisabledAdaptersChanged` / `onAdapterDisabledWarning`
- `onAutoLoginResult` — Toast + 触发 `checkOnline`
- `onLoginLog` — 写入 `useLogToastStore.addLog`
- `onAutoExitCountdown`/`onAutoExitCancelled`/`onCampusExitCountdown`/`onCampusExitCancelled` — 倒计时 Toast
- `onNetworkQualityResult` — 合并到 `useQualityStore.networkQuality`，触发"延迟过高"告警 (`handleQualityBadAlert`)
- `onSystemNotification` / `onUpdateAvailable` / `onConfigChanged`（仅本地 `updateConfigLocal`，不回写）

**关键策略**：监听器先于数据获取注册（在 `useInitialDataLoad` 之前），避免遗漏初始化期间事件；`mountedRef` 防止 unmount 后写状态；系统通知由后端统一发送，前端不再调用 `api.sendNotification`；网络质量事件无防抖，增量推送可立即更新 UI。

#### 5.3.2 `useInitialDataLoad.ts` (157 行) — 初始数据 bootstrap

mount 时调 `api.getInitData()` 拉取全量数据，按流水线 bootstrap 所有领域 store：

1. 失败兜底：`api.showWindow()` + 重置 `config = DEFAULT_CONFIG`
2. 合并 `cfg = { ...DEFAULT_CONFIG, ...initData.config }`
3. 根据 `cfg.password === PASSWORD_MASK` 调 `syncPasswordSaved(true/false)`
4. `useConfigStore.setState({ config: cfg })` → `useThemeStore.initTheme(cfg)`
5. 从 storage 读 `campus-active-panel`，决定 `defaultPanel` 与窗口显示（`isAutoStart && hiddenStart` 则不显示）
6. 写入 adapters / bgStatus / adapterDetails / 异步拉取 disabledAdapters / accounts / activeAccount
7. 调 `useAuthStore.checkOnline(cfg, adps)`
8. GPU 信息：initData.gpuInfo 优先 + `correctGpuInfo` 校正；否则异步 `api.getGpuInfo` + 校正
9. 写入 `refreshRate`
10. 异步 `checkDnsDohStatus` + 检查推荐 DNS + 是否启用 DoH，缺失则告警日志
11. **网络质量检测由后端 latency loop 统一管理**，前端不再主动调用 `checkNetworkQuality`

**幂等保护**：`initDoneRef` 防止 StrictMode 双触发；`mountedRef` 防止 unmount 后写状态；catch 块中 `showWindow` 不受 `mountedRef` 影响（应用级操作）。

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
| `DashboardPanel.tsx` | 总览面板，卡片可拖拽排序（framer-motion Reorder.Group），3种子组件（QuickActionsCard/AccountManageCard/NetworkQualityCard），布局持久化到safeStorage |
| `AboutDialog.tsx` | 关于对话框，双栏布局(应用信息+更新仪表盘)，镜像源选择，下载状态机(idle→selecting→downloading→done/error)，Release Notes渲染。**2026-09-03 修复**：`ensureFullUpdateInfo` 在一键下载前确保 updateInfo 完整（系统通知缓存路径构造的对象缺 `sha256Checksum`/`assets`，原样使用会下载 404 且安装被后端拒绝）；安装失败在 done 态显示错误文案（原先静默失败无任何反馈）；兜底下载文件名对齐真实资产命名 `Wxxy-CampusLogin_{v}_x64-setup.exe` |
| `useAuth.ts` | 认证逻辑 Hook |
| `types.ts` | 认证类型定义 (PortalStatusResult, CommandResult, LoginResult) |
| `index.ts` | 模块导出 |

#### 5.4.2 账号模块 — `account/`

| 文件 | 说明 |
|------|------|
| `AccountPanel.tsx` | 账号管理面板，3卡片(登录信息含密码显示隐藏/账号管理含添加切换删除/自动登录退出开关) |
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
| `useNetwork.ts` | 网络逻辑 Hook |
| `constants.ts` | 网络常量 (QUALITY_CONFIG: 9级质量配置含labelKey/color/bg/border/borderBg/icon/hex/activeBars/glow) |
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
| `SettingsPanel.tsx` | 设置面板，5卡片(外观/启动设置/通知/质量检测/引导向导)+7种主题+12色预设+取色器+亮暗模式 |
| `ThemeDialog.tsx` | 主题对话框，2列布局+亮暗模式切换 |
| `OnboardingWizard.tsx` | 4步引导向导(欢迎→账号→适配器→完成)，Framer Motion滑动转场，含语言切换，完成后自动登录 |
| `useSettings.ts` | 设置逻辑 Hook |
| `constants.ts` | 设置常量 (DEFAULT_CONFIG/ISP_OPTIONS(4种)/THEME_OPTIONS(7种)/VALID_THEMES/DEFAULT_PANEL_OPTIONS) |
| `types.ts` | 设置类型定义 (Config(36字段含logRetentionDays/configVersion，排除后业务字段34个), AutoLaunchResult, InitData) |
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
| `ToastContainer.tsx` | Toast容器，4种类型(info/success/error/warning)，economy档简单transition替代spring，支持action按钮 |
| `types.ts` | 共享类型定义 (UpdateAvailableData, UpdateInfo, DownloadProgress, MirrorSource 等) |
| `ui-types.ts` | UI 类型定义 (StatusState, PanelName(8个面板含speedtest), ThemeName(7种), LogType, GpuTier, GpuInfo, LogEntry, ToastMessage, AdapterDisabledWarningData, AutoExitCountdownData, SystemNotificationData, SaveConfigResult 等) |
| `ui-constants.ts` | UI 常量 (MAX_LOG_ENTRIES=300/APP_VERSION='2.2.9'/APP_NAME='校园网登录助手'/PASSWORD_MASK='***'/NAV_ITEMS=8个导航项) |
| `index.ts` | 模块导出 |

### 5.6 布局组件 — `components/layout/`

| 文件 | 说明 |
|------|------|
| `DockNav.tsx` | 适配器选择浮层 + 注销按钮 (无线蓝色Wifi/有线绿色Cable图标, 300ms延迟关闭/150ms延迟打开)，GSAP 磁吸效果（MAGNETIC_RANGE=80, MAX_SCALE=1.35, MAX_LIFT=-14），economy档禁用磁吸，RAF节流。tooltip 水平居中用 Tailwind `-translate-x-1/2`（2026-09-03：原 inline `translateX(-50%)` 覆盖 class transform 导致上浮动画失效） |
| `RightPanel.tsx` | 右侧面板，运行日志+网络适配器信息(可展开/折叠，显示IP/子网掩码/网关/DHCP/MAC)，空日志时呼吸动画。清空日志 GSAP 动画 stagger 动态封顶（>8条0.05s/>4条0.1s，2026-09-03：原固定 0.2s/条，日志满 300 条时动画约 60 秒且按钮禁用无法取消），与 LogPanel 同策略 |
| `TitleBar.tsx` | 标题栏，应用图标+版本号+更新提示+工具按钮(亮暗/语言/通知/主题/关于/最小化/最大化/关闭)，双击最大化，拖拽移动窗口 |

### 5.7 延迟颜色 — `lib/latency.ts`

- `QUALITY_CONFIG` 新增显式 `borderBg` 字段，确保 Tailwind JIT 可扫描
- `getLatencyColor()` 使用 `cfg.borderBg` 替代动态字符串替换
- `getLatencyLevel()` — 延迟等级计算
- `extractGatewayLatency()` — 提取网关延迟
- `extractExternalLatency()` — 提取外网延迟

### 5.8 国际化 — i18n/

- 基于 react-i18next + i18next-browser-languagedetector
- 翻译文件按 JSON 顶级 key 分组（单一 "translation" namespace，共22个）：nav, titlebar, dock, auth, account, settings, monitor, network, quality, speedtest, statusbar, dashboard, log, rightPanel, about, common, onboarding, confirmDialog, isp, panel, themeDialog, crashRecovery
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
- **主题初始化**: `initTheme()` — 从 localStorage 恢复亮暗模式 + 主题类
- **崩溃恢复** (`setupCrashRecovery`): 最多3次自动重载，GPU/WebGL/SharedArrayBuffer 错误触发重载，渲染心跳5秒无响应视为GPU崩溃触发重载，页面可见性变化时暂停/恢复 GSAP globalTimeline
- **渲染链**: `ErrorBoundary` > `LazyMotion(domAnimation, strict)` > `MotionConfig(reducedMotion="user")` > `App`
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

应用主组件（~360 行），编排所有业务模块：

- **面板路由**: 基于 `activePanel` switch 渲染 8 个面板（dashboard/account/auth(network)/monitor/quality/speedtest/network/settings）
- **PANEL_TITLES**: 面板标题 i18n key 映射表（titleKey/descKey）
- **初始化**: 调用 `useAppInit()` + 5 个业务 Hook（useAuth/useMonitor/useNetwork/useAccount/useSettings）
- **启动加速**: `useStartupBoost` 编排 5 元素入场动画（titleBar/statusBar/title/rightPanel/dockNav）
- **面板转场**: `AnimatePresence mode="wait"` + `panelVariants`（createPanelAppleVariants）+ slideDirection；切换锁 120ms（2026-09-03：原 500ms 远超退出动画 0.08s，快速连续点击被静默吞掉）
- **quality 面板可见性联动**（2026-09-03 约定）: `enableNetworkQuality === false` 时 App 对 quality 面板渲染 `null`、DockNav 过滤入口。三处必须联动——`useInitialDataLoad` 启动恢复 `defaultPanel`/`savedPanel` 时跳过 quality（否则重启后主区域空白）、`SettingsPanel` 关闭质量开关时清 `defaultPanel` 并把 `activePanel` 切回 dashboard。新增受开关控制的面板时同样需三处联动
- **窗口监听**: `getCurrentWindow().onResized` 监听窗口大小变化
- **引导向导**: 首次启动检测（`safeStorage.get('campus-onboarding-done')`），未完成则弹出 OnboardingWizard
- **ErrorBoundary 嵌套**: 外层 ErrorBoundary（L361）+ 面板内容 ErrorBoundary（L288）+ main.tsx ErrorBoundary
- **useLogToastStore**: 独立 zustand store 用于 Toast 管理

---

## 六、IPC 通信完整清单

### 6.1 请求-响应命令 (v2.2.9: 50个)

| 命令名 | 说明 |
|--------|------|
| `get_config` | 获取配置 |
| `show_window` | 显示窗口 |
| `save_config` | 保存配置 (空密码兜底) |
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
| `send_notification` | 发送通知（前端仅更新场景调用，网络质量通知由后端统一发送） |
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
| `system-notification` | 系统通知 |
| `update-available` | 更新可用 |
| `update-download-progress` | 下载进度 |
| `adapter-details-changed` | 适配器详情变更 |
| `campus-exit-countdown` | 校园网退出倒计时 |
| `campus-exit-cancelled` | 校园网退出已取消 |
| `config-changed` | 配置变更 |

---

## 七、依赖关系

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
| `windows` | 0.58 (features: 13项 — IpHelper/Ndis/WinSock/Foundation/Security/Shell/WindowsAndMessaging/Threading/Com/Ole/Variant/Gdi/Dxgi) | Win32 API |
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
| `base64` | 0.22 | Base64 编解码 |
| `chrono` | 0.4 | 时间处理 |
| `open` | 5 | 打开外部链接 |

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

### 7.3 依赖关系图

```
main.rs (二进制入口)
  └── lib.rs (库入口) → app/startup.rs::run()
        │   [build_runtime: Tokio multi-thread, worker=clamp(2,8), max_blocking=clamp(8,64)]
        │   [WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS ← platform/gpu.rs::build_browser_args]
        │   [plugin 注册: shell/notification/autostart/global-shortcut/single-instance]
        │   [setup_app: panic hook + 命令注册 + 状态管理 + 托盘 + 心跳 + 校园网检测]
        │
        └── commands/mod.rs
              ├── config_cmd.rs ← config/, account/crypto.rs, infra/state/, infra/logger.rs [set_debug_mode]
              ├── login.rs ← auth/service.rs, infra/events.rs, infra/lifecycle.rs, monitor/watcher.rs
              │   [do_login + do_logout (两步注销), adapter_name 可选参数]
              ├── background.rs (命令入口，委托 monitor::watcher)
              ├── network_cmd.rs ← network/*, infra/state/, platform/dns_config.rs, platform/elevation.rs, platform/helper_spawn.rs, network/dns_setup.rs, monitor/watcher.rs, monitor/latency.rs, auth/portal.rs
              │   [check_dns_doh_status / setup_dns_doh / check_campus_status / check_portal_status / start_latency_test]
              ├── account.rs ← config/, account/crypto.rs, infra/state/, config_cmd.rs
              ├── system.rs ← config/, network/*, infra/state/, platform/dns_config.rs
              └── updater.rs ← update/updater.rs

app/ (应用生命周期模块)
  ├── mod.rs (重导出)
  ├── startup.rs — run/build_runtime/setup_app (入口 + 命令注册 + panic hook)
  ├── tray.rs — 系统托盘 (菜单/事件处理)
  ├── window.rs — 窗口管理 (最小化/关闭/显示/show_and_focus_main)
  ├── shortcut.rs — 全局快捷键 (Ctrl+Shift+C 取消自动退出)
  ├── heartbeat.rs — 渲染进程心跳检测
  └── shutdown.rs — 关机/退出流程 (shutdown_and_exit 统一入口)

infra/
  ├── mod.rs (重导出)
  ├── state/ ← config/model.rs, arc-swap, parking_lot, tokio-util (子目录重构自 state.rs)
  │   ├── mod.rs     [TaskLock/TaskFlags/AppState (6 字段 + UpdateStats 子结构体) /CommandResult/AccountResult]
  │   ├── store.rs   [ConfigStore: ArcSwap<Config> CAS 更新]
  │   ├── network.rs [NetworkState + NetworkSnapshot: ArcSwap 快照 CAS 读写]
  │   └── exit.rs    [ExitStateStore]
  ├── task_manager.rs — BackgroundTaskManager (cancel token 统一管理)
  ├── logger.rs — 日志系统 (flush_quick: panic hook 专用 500ms 超时 flush; cleanup_old_logs_by_time: retention_days==0 时永久保留; set_debug_mode/get_debug_mode)
  ├── lifecycle.rs ← infra/state/, infra/notification.rs
  │   [start_auto_exit / cancel_auto_exit_inner / start_campus_exit / shutdown_and_exit]
  ├── notification.rs — emit_notification 封装
  ├── events.rs — EventBus (16 个 emit_xxx 方法, emit_login_log/emit_network_quality/...)
  └── command_context.rs — CommandContext::from_app (统一访问 ConfigStore/TaskFlags/NetworkState/ExitStateStore)

monitor/ (10 个子模块，portal_failure 已迁入 auth/failure_tracker)
  ├── mod.rs (重导出, 含 trigger_background_check 别名)
  ├── watcher.rs (门面 51行, re-export background_check/background_task + run_startup_tasks)
  ├── background_check.rs ← auth/failure_tracker/auto_auth/lifecycle/latency/portal_check/campus_check/background_emit
  │   [run_background_check_blocking 检测主体 + run_background_check async 包装]
  ├── background_task.rs ← infra/task_manager [start_background_check_inner + task_manager.spawn]
  ├── auto_auth.rs ← infra/state/, auth/session.rs, infra/notification.rs, infra/lifecycle.rs
  ├── latency.rs ← infra/state/, network/*, infra/notification.rs [spawn_latency_test_loop]
  ├── adapter_watch.rs ← infra/state/, infra/events.rs, CancellationToken
  ├── campus_check.rs — 校园网检测 (CampusCheckResult 定义于此)
  ├── portal_check.rs — Portal 检测 (PortalCheckResult 定义于此 + check_adapter_portal)
  ├── quality_scheduler.rs — 质量检测调度器
  └── background_emit.rs — 后台事件推送

auth/ (6 个子模块，原 traits.rs 已删除)
  ├── mod.rs (重导出)
  ├── portal.rs ← network/client.rs, reqwest, url [random_v, block_on_http 同步-异步桥接]
  ├── protocol.rs ← network/client.rs, reqwest, urlencoding, regex [random_v]
  │   [两步注销: 2轮循环 MAC解绑+Radius注销, callback 动态生成 dr100{round+1}/dr100{round+2}]
  ├── session.rs ← auth/portal.rs, auth/protocol.rs, network/adapter.rs
  │   [adapter_action_with_log / login_adapter_with_log 通用封装]
  ├── service.rs ← auth/session.rs, auth/failure_tracker.rs, auth/dual_adapter_executor.rs, network/adapter.rs
  │   [full_login / full_logout 统一入口 + logout_adapter_with_log + post_login_handler (直接调用 network::resolve_adapter_names，无 trait)]
  ├── failure_tracker.rs ← infra/state/, network/dhcp [is_auth_failure / update_auth_failure_count / handle_portal_request_failure / reset_all]
  └── dual_adapter_executor.rs — execute_dual<F1,F2> 双适配器并行执行 (B9-7 泛型静态分发 + tokio spawn_blocking + 可中断错峰)

network/ (9 个业务子模块 + discovery/ 子目录)
  ├── mod.rs (重导出)
  ├── client.rs ← arc-swap, lazy_static, dashmap, reqwest [TLS 1.3+回退, PORTAL_URL/CLIENT_POOL]
  ├── adapter.rs ← client.rs, windows, regex [TTL 5s 缓存, validate_adapter_name]
  │   [校园网检测: 网络名称/子网/网关Ping]
  ├── adapter_cache.rs — 适配器查询缓存 (force/cached 双模式)
  ├── dhcp.rs — DHCP 操作 (release/renew, MAC 重置, apply_mac_change_via_registry)
  ├── subnet.rs — 子网判定 (/18 校园网子网匹配)
  ├── dns.rs — DNS 缓存管理 + DoH解析 + 智能解析策略
  ├── timing.rs
  │   ├── DNS_SERVER_SCORES / DOH_SERVER_SCORES (dashmap 评分表)
  │   ├── resolve_host_smart (三级智能解析)
  │   ├── resolve_via_doh (RFC 8484 wire format)
  │   └── measure_https_timing / measure_dns_query / measure_doh_timing
  ├── quality.rs ← adapter.rs, client.rs, surge-ping, tokio-rustls, timing.rs, tauri::AppHandle
  │   [两阶段检测: DNS/DoH → HTTPS(分批并发) + 增量推送]
  ├── dns_setup.rs ← dns_config.rs, get_adapters_force
  │   [setup_dns_doh_admin: 管理员/提权 helper 共用的一键 DNS+DoH 设置]
  └── discovery/ (适配器发现子模块)
      ├── mod.rs (重导出)
      ├── registry.rs — 注册表遍历 (CLASS_SUBKEY_CACHE 懒加载+锁优化)
      └── windows.rs — Windows 特定发现逻辑 (GetAdaptersAddresses + LinkSpeed)

platform/
  ├── mod.rs (重导出)
  ├── dns_config.rs ← platform/elevation.rs, winreg
  │   [set_profile_dns_via_api / set_dns_via_api / set_doh_via_api / clear_adapter_dns_via_api]
  ├── elevation.rs — ShellExecuteW UAC 提权 + GUID 解析 + COM ShellExecuteW 提权 (shell_exec_elevated)
  ├── gpu.rs — GPU 信息检测 (DXGI EnumAdapters1) + 显示器刷新率检测 (EnumDisplaySettingsW) + 动态浏览器参数 (build_browser_args)
  │   [GpuInfo 含 gpu_preference: u8 (0=默认/1=节能/2=高性能, 读注册表 UserGpuPreferences)]
  │   [determine_tier: NVIDIA→discrete, Intel Arc→discrete, Iris Xe→mid-igpu, UHD→low/mid-igpu, AMD RX/Pro→discrete, 780M/880M→high-igpu]
  │   [build_browser_args: 仅 --js-flags=--max-old-space-size=512（2026-09-03 精简，渲染交还平台默认）]
  ├── autostart.rs — 开机自启
  └── helper_spawn.rs ← elevation.rs
      [spawn_elevated_helper: 提权重启自身(--helper) + 结果文件轮询]

config/
  ├── mod.rs (重导出)
  ├── model.rs — Config 结构体 + Default + user_account_with_operator + default_campus_gateway
  ├── persist.rs — atomic_write + list_account_names
  └── validate.rs — 校验逻辑 (枚举值/正则/URL/Portal URL 迁移/校园网关校验)

account/
  ├── mod.rs — 多账号管理命令
  └── crypto.rs — Windows DPAPI 加密/解密

update/
  ├── mod.rs (重导出)
  └── updater.rs ← reqwest, url, sha2
      [VERSION_MIRRORS: 4个镜像源(ghfast.top/gh-proxy.com/ghproxy.net/gh.llkk.cc)]
      [start_update_check_loop: 24小时间隔自动更新检查]
      [verify_download_sha256: 分块流式读取计算 SHA256，64KB buffer]
      [SHA256 校验文件支持镜像源 URL 列表]

App.tsx (377行, App + AppInner)
  ├── 领域 store (zustand, useShallow 选择性订阅)
  │   ├── useConfigStore (config/accounts/language + 防抖保存)
  │   ├── useAuthStore (doLogin/doLogout/checkOnline/status/bgStatus)
  │   ├── useAdapterStore (adapters/details/activePanel)
  │   ├── useQualityStore (networkQuality/dnsDoh/gpuInfo)
  │   ├── useThemeStore (themeName/isLightMode/customThemeColor)
  │   └── useLogToastStore (logs/toasts)
  ├── tauriApi.ts ← @tauri-apps/api (原 useIpc.ts，纯模块非 hook)
  └── useAppInit.ts (编排 hook)
        ├── useEventListeners.ts (15 个事件订阅 + 窗口关闭拦截)
        ├── useInitialDataLoad.ts (getInitData bootstrap)
        ├── useHeartbeat.ts (5s 渲染心跳)
        └── useGlobalShortcut.ts (Ctrl+Shift+C)
```

---

## 八、安全体系

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
| 更新发布约定 (2026-09-03) | ① `check_update_inner` 对下载 URL 做 HEAD 探测，Release 资产 404（version.json 先行而未发布）则本轮不提示更新，探测网络失败保守视为存在；② `version.json` 支持可选 `notes` 字段填充 release_notes；③ `build.ps1` 构建后自动生成 `<installer>.sha256`（shasum 兼容格式），**发布 Release 必须同时上传安装包与 .sha256 文件**，版本号提交与 Release 发布需同流程完成 |
| 适配器名称校验 | network/adapter_cache.rs::validate_adapter_name (经 adapter.rs re-export)，禁止 `&\|;\`$()<>\"'\n\r\0` 等元字符，防命令注入 |

---

## 九、性能优化

| 优化项 | 实现 | 效果 |
|--------|------|------|
| PowerShell 消除 | `--helper` 提权重启自身，Rust 直调 Win32/winreg 替代 PowerShell 脚本（改 MAC/设 DNS+DoH，2.4.0） | 200-500ms → 1ms |
| GPU 检测改用 DXGI | `CreateDXGIFactory1` + `EnumAdapters1` 替代 PowerShell | 1~3s → 50~200ms |
| DNS 设置优化 | `SetInterfaceDnsSettings` Win32 API（`dns_config.rs`）+ 管理员/helper 共用 `dns_setup.rs::setup_dns_doh_admin` 替代 PowerShell | 1-3s → 100-300ms |
| DNS 注册表检测 | winreg 替代 PowerShell (read_adapter_dns_from_registry 中用 `netsh dns show encryption` 读取) | 10x+ 速度提升 |
| 适配器 TTL 缓存 | 5秒 TTL + 4秒后台主动刷新 (adapter_cache.rs::start_cache_refresh_task, CACHE_REFRESH_INTERVAL_SECS=4) | 减少 API 调用，保证缓存新鲜 |
| codegen-units=1 | LTO 跨单元优化 | +2~5% 性能 |
| panic=abort | 编译选项 | 体积↓5~10% |
| 登录重试可中断 | 每100ms检查退出标志 | 退出延迟 2s → 100ms |
| 前端选择性订阅 | useShallow 减少不必要重渲染 | UI 响应更流畅 |
| 高频事件节流 | 500ms 时间戳节流 | 防止 UI 频繁更新 |
| FluidBackground CSS动画移除 | 3个大型渐变层动画完全移除 | GPU进程CPU占用显著降低 |
| GSAP动画迁移 | 约 12 个 CSS 动画迁移至 GSAP，全局配置 `gsap.defaults({ease:'expo.out'})` + `gsap.config({autoSleep:5})` + `gsap.ticker.lagSmoothing(500,33)` (main.tsx:14-15)，`force3D` 不再全局默认（各 tween 显式声明） | GPU合成层加速，空闲自动暂停，动画结束回收 |
| RAF节流+位置去抖 | Button/DockNav/AnimatedCard鼠标事件节流 | 减少无效getBoundingClientRect调用 |
| transition-all替换 | 10 处替换为显式属性列表（剩余 4 处 AboutDialog/OnboardingWizard/DockNav×2 为有意保留） | 减少不必要的属性过渡计算 |
| WebView2 内存管理 | 前台 NORMAL/后台 LOW (ICoreWebView2_19.SetMemoryUsageTargetLevel) | 后台内存占用显著降低 |
| WebView2 参数精简 (2026-09-03) | `build_browser_args` 由厂商分支 11 参数精简为仅 `--js-flags=--max-old-space-size=512`：`--enable-native-gpu-memory-buffers`/`--gpu-memory-buffer-size-mb`/`UseSkiaRenderer` 已从 Chromium 移除，`--renderer-process-limit` 不再被读取，`--num-raster-threads`/`--enable-gpu-rasterization` Windows 默认即开，`EnableDrDc` 源码标注 Windows NOT SUPPORTED，`--use-angle=d3d12` 非法值回退默认，`SkiaGraphite` 实验性强开有渲染异常风险，`--enable-zero-copy` 收益不可测 | 渲染交还平台默认（测试最充分配置），消除实验组合隐患，厂商分支代码删除 |
| Tokio 线程池动态配置 | 根据 CPU 核心数配置 worker_threads(2-8)/max_blocking_threads(8-64) (app/startup.rs::build_runtime) | 资源利用更合理 |
| CAS 原子配置更新 | `ConfigStore::update` CAS 原子更新 (compare_and_swap 循环) 避免 TOCTOU 竞态 (infra/state/store.rs:35-49) | 配置一致性保证 |
| 流式 SHA256 校验 | 分块流式读取计算 SHA256，64KB buffer | 大文件校验内存占用降低 |
| dual_adapter_executor 泛型化 (B9-7) | `Box<dyn FnOnce>` 改泛型 `F1`/`F2` 静态分发，4 处调用点去 `Box::new` (auth/dual_adapter_executor.rs) | 消除堆分配与虚函数调用 |
| 双适配器在线检测并行化 (B9-10) | `check_any_adapter_online` 串行改 `std::thread::scope` 并行 (commands/login.rs) | 双适配器检测延迟减半 |
| logger shutdown 超时 join (B9-14) | `mpsc` + `recv_timeout(500ms)` 带超时 join 替代固定 `sleep(200ms)` (infra/logger.rs)；main.rs 删除固定 sleep | 避免 logger 线程卡死阻塞退出，同时消除不必要的 200ms 等待 |
| CLIENT_POOL LRU 淘汰 (B9-17) | `client_pool_get` 命中时更新 `Instant`，容量超限 `min_by_key` 剔除最久未访问 (network/client.rs) | 热点连接保活，冷连接及时回收 |
| background_check CAS 合并 (B9-4) | 合并连续 `network.update` 调用，2 处从 2 次 CAS 降为 1 次 (monitor/background_check.rs) | 减少 CAS 循环开销 |
| lifecycle TOCTOU 竞态修复 (B9-8) | 3 处 `auto_exit_deadline` check-then-act 收入同一锁临界区 (infra/lifecycle.rs) | 消除快捷键注册/注销竞态 |
| 质量检测全局节流 (v2.4.0) | `LAST_QUALITY_CHECK_DONE_MS` 全局 60s 最小间隔，后台巡检（15s）与延迟循环（30s）共用，手动命令不受限 (monitor/quality_scheduler.rs) | 消除每 15s 全量外网探测的高频重复（原两定时器叠加） |
| 注册表遍历移入 blocking 池 (v2.4.0) | adapter_watch 15s 周期的 `refresh_class_subkey_cache` 包进 spawn_blocking (monitor/adapter_watch.rs) | 不再在 async worker 线程同步遍历 HKLM Class 子键 |
| 网关 surge_ping 替代 ping (v2.4.0) | `check_gateway_reachable_from` 改 surge_ping 异步 ICMP（bind 源 IP 等同 `-S`）(network/subnet.rs) | 消除每 15s spawn ping 子进程 |
| netsh 查询 TTL 缓存 (v2.4.0) | SSID/有线 Profile 结果 60s 缓存，仅缓存 Ok (network/subnet.rs) | 消除每 15s 各 spawn 一次 netsh wlan/lan |
| DNS Resolver 复用 (v2.4.0) | 按 (bind_addr, server 集, 超时) 维度缓存的 Resolver 小池 (network/dns.rs) | 替代每次解析新建 Resolver（含自带 runtime） |
| DNS 竞速降级 (v2.4.0) | `resolve_host_smart` 由"全部 DoH+传统 DNS"降级为"历史最快 1 路 DoH+传统 DNS" (network/dns.rs) | 每域名 TLS 握手减半；QR/RCODE 校验保留 |
| DNS 缓存淘汰优化 (v2.4.0) | O(n²) 逐条删除改单次排序取最旧 N 条 (network/dns.rs) | 容量超限清理 O(n log n) |
| 质量检测 Phase1 分批 (v2.4.0) | 网关/DNS/DoH/SystemDns 分 3 小批，每批 ≤3 并发；SystemDns 2 个/批 (network/quality.rs) | 瞬时并发 20+ → ≤3 |
| 适配器轮询与缓存 (v2.4.0) | IP 强刷 100ms→300ms；`ShowInNetworkConnections` 5s TTL 缓存；adapter_watch 改读缓存；命中路径按需克隆 (network/adapter_cache.rs, registry.rs, adapter_watch.rs) | 减少系统调用；消除 15s force 与 4s 刷新叠加 |
| 更新下载异步写盘 (v2.4.0) | download_update 改 tokio::fs 异步写 (commands/updater.rs) | 大文件下载不再阻塞 async 线程 |
| 日志 IO 优化 (v2.4.0) | 消 line clone（SendError 归还）；批量落盘（32 条或 2s）；read_recent_logs 尾部倒读 (infra/logger.rs) | 减少分配；消除每条日志一次写+flush |
| block_on 统一安全工具 (v2.4.0) | 新增 `infra::async_util::block_on_sync`，protocol/双适配器 5 处迁移 (auth/protocol.rs, dual_adapter_executor.rs) | 防 async worker 线程 block_on panic（前瞻） |
| 客户端池键去 String (v2.4.0) | CLIENT_POOL key 改 `(Option<IpAddr>, u8, u64)` 元组 (network/client.rs) | 热路径零堆分配 |
| MAC 随机化 getrandom (v2.4.0) | generate_random_mac 改 getrandom 填充（低概率失败降级 LCG）(network/dhcp.rs) | 原时间+计数器 LCG 可预测 → 密码学随机 |
| 前端订阅粒度化 (v2.4.0) | App/StatusBar/RightPanel config 全量订阅改最小粒度 selector (App.tsx, StatusBar.tsx, RightPanel.tsx) | 任意字段变化不再级联重渲染外壳与面板 |
| 面板代码分割 (v2.4.0) | 10 个低频面板/对话框 React.lazy 分包并绕开 barrel (App.tsx) | 主包 402KB→285KB（-29%） |
| 文本输入本地草稿 (v2.4.0) | 用户名/网关/SSID/fixedGateway 改本地 draft + blur 提交；主题色 80ms 节流 (AccountPanel, MonitorPanel, SettingsPanel) | 不再每键写 store + 触发防抖保存 |
| 日志面板渲染优化 (v2.4.0) | 轮询内容未变跳过 setState；叠加窗口可见性门控；单遍解析 (shared/LogPanel.tsx) | 消除四层 useMemo 全量重算与整表重渲染 |
| 事件与动画资源 (v2.4.0) | setStatus 浅比较；usePageIdle interval 化；GSAP/RAF/ripple 清理与 reduced-motion 兜底 (useEventListeners, usePageIdle, AnimatedNumber, button, animated-card, useRipple) | 减少无效 setState 与未清理资源 |
| 动画合成层精细化管理 (v2.5.0) | 移除一次性/瞬态动画的常驻 will-change（card-enter 等）；main.tsx 去全局 force3D；.anim-idle 补 signal-glow-active 暂停 (index.css, main.tsx) | 动画结束即回收合成层，降低持续 GPU 层开销 |
| 设计基础整治 (v2.5.0) | html font-size 15px→16px（对齐 DockNav fallback 修正 dock 偏移）；统一圆角体系（移除按钮 9999px 胶囊化与 w-8 h-8 强转圆形，md=10/lg=12/xl=16）；**移除涟漪动画**（卡片+按钮，删 useRipple.ts）；补全 prefers-reduced-motion；清理 9+ 死代码类；z-index 语义化（DockNav z-50→z-30 低于遮罩，新增 Z_INDEX 表） (index.css, main.tsx, ui-constants.ts, DockNav.tsx) | 视觉层级一致，reduced-motion 全停，减小样式体积与持续 GPU 开销，消除 Dock 与遮罩层级竞态 |
| 动画丢失修复 (v2.5.0) | `LazyMotion features={domAnimation}` → `domMax`（恢复 layout 特性，修复 SegmentTabs 选中背景块 slide 动画）；`.panel-content` `content-visibility: auto`→`visible`（避免含动画元素被跳过渲染合成，修复卡片入场/信号条/数字滚动"数据在但没播"） (main.tsx, index.css) | 恢复面板/子标签切换动画与测试过程动画显示 |
| 子标签切换动画修复 (v2.5.0) | `QualityPanel` 测试详情子标签切换：`TooltipProvider` 从 `m.div(key=activeTab)` 外层移入内层，让 `AnimatePresence mode=wait` 感知到 key 变化从而播进出场动画；`tabContainerVariants` 补全 initial/终态；子元素 `m.div` 补 `custom={tabDirection}` (QualityPanel.tsx) | 恢复测试详情网关/DNS/网站/视频/游戏子标签切换的滑动+淡入过渡 |
| 面板启动预加载 (v2.5.0) | `App.tsx` 面板 lazy loader 抽出复用（`loadAccountPanel` 等），新增 `preloadPanels()` 在启动动画播完后经 `requestIdleCallback` 空闲时 `import()` 预取所有面板/对话框 chunk (App.tsx) | 切面板时 chunk 已就绪，避免首次切换等待下载导致卡顿 |
| 常用面板静态导入 (v2.5.0) | Account/Network/Monitor/Quality/SpeedTest/Settings 6 个常用面板改静态 `import`（并入主包 285→368KB），切换零等待；仅 LogPanel + About/Theme/Onboarding 对话框保留懒加载 + 启动预取 (App.tsx) | 消除常用面板切换卡顿，首屏体积仍低于分包前 402KB |
| WebView2 vsync 恢复 (2026-09-03) | `build_browser_args` 移除 `--disable-gpu-vsync`（platform/gpu.rs）；前端 GSAP/Framer/CSS 本就 rAF/vsync 驱动无 JS 上限 | 解除 vsync 后 BeginFrame 不对齐显示器刷新，帧节奏紊乱经 DWM 合并呈撕裂+顿挫（观感"掉帧"）且 GPU 空耗；恢复后管线锁显示器刷新率，120Hz 屏动画最高 120fps |

---

## 十、编译配置

```toml
[profile.release]
lto = "thin"
codegen-units = 1
opt-level = 3
strip = true
panic = "abort"
```

---

## 十一、版本号管理

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
// 1. 保留 Tauri 原有构建
tauri_build::build();

// 2. 从 tauri.conf.json 提取 version，注入编译期环境变量
let conf_path = Path::new("tauri.conf.json");
if !conf_path.exists() {
    panic!("tauri.conf.json not found in src-tauri directory");
}
let content = fs::read_to_string(conf_path).expect("Failed to read tauri.conf.json");
let version = content
    .lines()
    .find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with("\"version\"") {
            let value_part = trimmed.split(':').nth(1)?;
            let value = value_part
                .trim()
                .trim_end_matches(',')
                .trim_matches('"');
            Some(value.to_string())
        } else {
            None
        }
    })
    .expect("tauri.conf.json missing 'version' field");

println!("cargo:rerun-if-changed=tauri.conf.json");
println!("cargo:rustc-env=APP_VERSION={version}");
```

**关键设计**：
- 不用 `serde_json` 解析（避免引入额外 build-dependency），手写简单字符串提取 `"version"` 字段
- `cargo:rerun-if-changed=tauri.conf.json`：仅当配置文件变化时重新编译，避免无关修改触发全量重编
- `cargo:rustc-env=APP_VERSION=...`：将版本号暴露为编译期常量，Rust 代码用 `env!("APP_VERSION")` 引用（编译期宏，零运行时开销）

### 升级版本号的完整流程

发布新版本时（以 v2.2.9 为例）：

1. **编辑唯一权威源** — 修改 `tauri-app/src-tauri/tauri.conf.json` 的 `"version"` 字段为 `"2.2.9"`
2. **手动同步 Cargo.toml** — 修改 `tauri-app/src-tauri/Cargo.toml` 的 `version = "2.2.8"` 为 `"2.2.9"`（cargo 强制要求）
3. **同步发布标记** — 修改仓库根 `version.json` 的 `"version": "v2.2.8"` 为 `"v2.2.9"`（带 v 前缀，是 GitHub release tag 的格式）
4. **同步前端 package.json** — 两个 `package.json` 的 `"version"` 字段（npm 规范要求，无 v 前缀）
5. **同步前端常量** — `tauri-app/frontend/src/shared/ui-constants.ts` 的 `APP_VERSION`（保持当前架构，不改为环境变量注入）
6. **同步静态预览** — `tauri-app/frontend/about-preview.html` 的 `app-version` 和 `status-version` 两个 div（**注意**：此处带 `v` 前缀，如 `v2.2.8`）
7. **同步徽章** — `README.md` 的 `version-2.2.8` 徽章
8. **同步文档** — `CODE_WIKI.md` 顶部版本号 + 底部元信息

> ⚠️ **Cargo.lock 中的 version**：由 cargo 自动更新，下次 `cargo build` 时自动重写。

> ⚠️ **版本号提交与 Release 发布必须同流程完成**（2026-09-03 教训，v2.3.0 事故）：`version.json` 一旦推送到 main 而对应 tag 的 Release 尚未发布，所有旧版本用户会收到更新通知但下载必然 404。现版本后端已加 HEAD 探测兜底（资产 404 则本轮不提示更新），但仍应把"改 version.json"与"发布 Release"绑在同一次操作里。

> ⚠️ **Release 资产发布检查清单**（2026-09-03 新增）：
> 1. 上传 `Wxxy-CampusLogin_{ver}_x64-setup.exe`（文件名必须与 `update/updater.rs` 硬编码拼接完全一致）
> 2. 同时上传构建产物目录中的 `{安装包名}.sha256`（`build.ps1` 第 [5/5] 步已自动生成）——缺失时应用内更新校验全 4xx，默认拒绝安装且用户无法自救
> 3. `version.json` 可选填 `notes` 字段（字符串，Markdown 列表），将显示为应用内更新日志（release_notes）

> ⚠️ **升级检查清单**：建议在发布前对照以下 5 个**必须保持一致**的位置：
> 1. `tauri-app/src-tauri/tauri.conf.json` → `"version": "2.2.9"`
> 2. `tauri-app/src-tauri/Cargo.toml` → `version = "2.2.9"`
> 3. `tauri-app/frontend/src/shared/ui-constants.ts` → `APP_VERSION = '2.2.9'`
> 4. `tauri-app/package.json` + `tauri-app/frontend/package.json` → `"version": "2.2.9"`
> 5. 根 `version.json` → `"version": "v2.2.9"`（带 v 是发布 tag 格式）

### 后端代码引用方式

```rust
// app/startup.rs:150 启动日志
crate::log_info!("app", "应用启动, 版本: v{}", env!("APP_VERSION"));

// update/updater.rs:318 更新检查
let current = env!("APP_VERSION");
let has_update = compare_versions(current, &latest_tag);

// commands/system.rs:200 系统信息接口
let version = env!("APP_VERSION").to_string();
```

### 前端版本号来源（单轨）

| 来源 | 用途 | 修改方式 |
|---|---|---|
| `ui-constants.ts` `APP_VERSION` 常量 | 代码内直接 import | 手动同步 |

> v2.2.9 重构清理：`vite.config.ts` 已移除原 `__APP_VERSION__` 注入逻辑（不再 `import tauriConf`、不再 `define` 注入），前端版本号唯一来源为 `ui-constants.ts` 的 `APP_VERSION` 硬编码常量。

> 注：`ui-constants.ts` 的 `APP_VERSION` 保留硬编码是为了在非 Tauri 环境（如纯前端 Storybook / 单元测试 mock）下也能取到合理默认值。**升级时仅需同步 `ui-constants.ts` 一处**。

### 版本号格式约定

- **semver 格式（不带 v）**：`Cargo.toml` / `tauri.conf.json` / `package.json` × 2 / `ui-constants.ts` → `2.2.9`
- **发布 tag 格式（带 v）**：`version.json` / 后端日志（`v{}`）/ `about-preview.html` 的 `app-version` 和 `status-version` div（`v2.2.9`）/ README 徽章（`version-2.2.9` 不带 v，但后端启动日志带 v）

---

*文档版本: v2.2.9 | 基于代码版本: CampusLogin v2.2.9 | 更新日期: 2026-07-14*
