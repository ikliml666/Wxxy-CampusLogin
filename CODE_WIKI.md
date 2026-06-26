# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.2.8 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
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
│   │   ├── package.json             # 前端依赖 (含 zustand ^5.0, framer-motion ^12)
│   │   ├── vite.config.ts           # Vite 构建配置
│   │   ├── tailwind.config.js       # Tailwind CSS 配置
│   │   ├── tsconfig.json            # TypeScript 配置
│   │   ├── index.html               # HTML 入口
│   │   └── src/
│   │       ├── main.tsx             # React 入口
│   │       ├── App.tsx              # 根组件
│   │       ├── index.css            # 全局样式
│   │       ├── hooks/               # 自定义 Hooks (12个)
│   │       │   ├── useAppStore.ts   # 统一状态管理 (zustand) + 密码处理
│   │       │   ├── useIpc.ts        # Tauri IPC 封装 (含 DNS/DoH/注销 API)
│   │       │   ├── useAppInit.ts    # 初始化逻辑 + 事件监听 + 在线日志去重
│   │       │   ├── useAnimationProfile.ts  # 动画配置
│   │       │   ├── useAsyncLock.ts  # 异步锁
│   │       │   ├── useBreatheAnimation.ts  # 呼吸动画
│   │       │   ├── useGlowAnimation.ts     # 发光动画
│   │       │   ├── useLogToastStore.ts     # 日志 Toast 状态
│   │       │   ├── usePageIdle.ts   # 页面空闲检测
│   │       │   ├── usePulseAnimation.ts    # 脉冲动画
│   │       │   ├── useRipple.ts     # 涟漪效果
│   │       │   └── useStartupBoost.ts      # 启动加速
│   │       ├── lib/
│   │       │   ├── utils.ts         # 工具函数 (含 safeStorage 内存降级封装，替代 localStorage)
│   │       │   ├── color.ts         # HEX→HSL 颜色转换
│   │       │   ├── latency.ts       # 延迟等级/颜色计算 (显式 borderBg)
│   │       │   ├── animations.ts    # Framer Motion 动画变体
│   │       │   └── easing-config.ts # 缓动配置
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
│           │   ├── client.rs        # 缓存基础设施 (NET_CACHE/CLIENT_POOL/HTTP客户端/TLS 1.3+回退)
│           │   ├── adapter.rs       # 适配器查询/Win32 API/DHCP/网关/TTL缓存/校园网检测/AdapterStatus四分类
│           │   ├── adapter_cache.rs # 适配器查询缓存 (force/cached 双模式)
│           │   ├── dhcp.rs          # DHCP 操作 (release/renew)
│           │   ├── subnet.rs        # 子网判定 (/18 校园网子网匹配)
│           │   ├── dns.rs           # DNS 缓存管理 + DoH解析 + 智能解析策略
│           │   ├── timing.rs        # HTTP计时 + DNS智能解析 + DoH + 评分系统
│           │   ├── quality.rs       # 网络质量并发延迟测试 (两阶段检测+增量推送)
│           │   └── discovery/       # 适配器发现子模块
│           │       ├── mod.rs       # 重导出
│           │       ├── registry.rs  # 注册表遍历 (CLASS_SUBKEY_CACHE 懒加载+锁优化)
│           │       └── windows.rs   # Windows 特定发现逻辑
│           ├── auth/                # 认证模块
│           │   ├── mod.rs           # 重导出
│           │   ├── portal.rs        # Portal认证状态检测 (random_v + block_on_http 同步-异步桥接)
│           │   ├── protocol.rs      # 登录/两步注销/重试/响应解析 (random_v)
│           │   ├── session.rs       # 登录/注销会话管理 (full_login_inner/full_logout_inner/认证失败计数+MAC重置)
│           │   ├── service.rs       # 认证服务编排 (full_login/full_logout 统一入口)
│           │   ├── traits.rs        # AdapterResolver trait 抽象 (主/副适配器名称解析)
│           │   ├── failure_tracker.rs # 认证失败计数跟踪
│           │   └── dual_adapter_executor.rs # 双适配器并行执行器
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
│           │   ├── logger.rs        # 日志系统 (文件+通道+调试模式切换+日志保留天数清理)
│           │   ├── lifecycle.rs     # 自动退出控制 + 校园网退出流程
│           │   ├── notification.rs  # 通知封装 (emit_notification)
│           │   ├── events.rs        # 事件总线 EventBus (16 个 emit_xxx 方法)
│           │   ├── command_context.rs # 命令上下文 CommandContext::from_app
│           │   └── task_manager.rs  # 后台任务管理器 BackgroundTaskManager (cancel token 统一管理)
│           ├── monitor/             # 监控模块
│           │   ├── mod.rs           # 重导出
│           │   ├── watcher.rs       # 后台检测调度器 (PortalCheckResult 职责分离/校园网检测/Portal容错/Handle::enter 上下文修复)
│           │   ├── auto_auth.rs     # 自动登录/断线重连
│           │   ├── latency.rs       # 网络质量通知+延迟测试循环
│           │   ├── adapter_watch.rs # 适配器状态监控 (CancellationToken可退出)
│           │   ├── campus_check.rs  # 校园网检测 (从 watcher 拆分)
│           │   ├── portal_check.rs  # Portal 检测 (check_adapter_portal 并行检测)
│           │   ├── quality_scheduler.rs # 质量检测调度器
│           │   └── background_emit.rs   # 后台事件推送
│           ├── platform/            # 平台交互模块
│           │   ├── mod.rs           # 重导出
│           │   ├── dns_config.rs    # DNS/DoH 配置文件设置 (per-profile/适配器级/DoH API)
│           │   ├── elevation.rs     # UAC 提权 (ShellExecuteW + COM ShellExec) + GUID 解析 + is_admin
│           │   ├── gpu.rs           # GPU 信息检测 (DXGI) + 刷新率检测 + 浏览器参数 + gpu_preference
│           │   └── autostart.rs     # 开机自启 (注册表/Tauri 插件)
│           ├── update/              # 更新模块
│           │   ├── mod.rs           # 重导出
│           │   └── updater.rs       # 更新检查/下载/安装 (SHA256校验)
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
│              单一状态管理层 (State Layer)             │
│  useAppStore (zustand) — 集中管理所有前端状态         │
│  useIpc Hook — Tauri IPC 通信封装 (单一实例)          │
├─────────────────────────────────────────────────────┤
│                IPC 通信层 (Bridge Layer)              │
│  Tauri invoke (请求-响应) | Tauri listen (事件推送)   │
│  前端 ←→ Rust 后端                                   │
├─────────────────────────────────────────────────────┤
│                 业务逻辑层 (Logic Layer)              │
│  ┌─────────────────────────────────────────────────┐ │
│  │  monitor/watcher.rs (调度器, PortalCheckResult 职责分离) │
│  │    ├─→ monitor/auto_auth.rs (自动登录/断线重连)  │ │
│  │    ├─→ infra/lifecycle.rs  (自动退出倒计时+校园网退出) │ │
│  │    ├─→ monitor/latency.rs  (质量通知/延迟循环)   │ │
│  │    └─→ monitor/adapter_watch.rs (适配器监控,可取消) │ │
│  │  app/ — 应用生命周期 (startup/tray/window/shortcut/heartbeat/shutdown) │ │
│  │  auth/ — 认证模块 (8个子模块: session/protocol/portal/service/traits/failure_tracker/dual_adapter_executor) │ │
│  │  auth/portal.rs — Portal 检测 (block_on_http 同步-异步桥接) │ │
│  │  network/ — 网络检测/延迟测试/质量检测 (9个子模块 + discovery/ 子目录) │ │
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
│  DXGI — 显示器刷新率检测 (EnumDisplaySettingsW)      │
│  winreg — 注册表读写 (DNS/DoH 配置)                  │
│  reqwest — HTTP 请求 (TLS 1.3强制+1.2回退)          │
│  hickory-resolver — 传统 DNS 解析                    │
│  tokio-rustls — DoH TLS 连接 (RFC 8484)             │
│  tokio — 异步运行时                                  │
│  Windows Registry — 开机自启/DNS配置                 │
└─────────────────────────────────────────────────────┘
```

### 3.2 Commands 模块依赖关系 (v2.2.8)

```
// [架构说明] 模块间耦合关系
//
//  依赖链（箭头表示 "调用/依赖"）：
//
//  monitor/watcher ──→ monitor/auto_auth ──→ infra/lifecycle
//       │                    │                    │
//       │                    └──→ infra/notification (emit_notification)
//       │
//       ├──→ monitor/latency ──→ infra/notification (emit_notification)
//       │
//       └──→ infra/lifecycle
//
//  commands/login ──→ auth/session ──→ auth/protocol (两步注销)
//                  └──→ auth/portal (Portal 检测)
//                  └──→ infra/notification (emit_notification)
//
//  infra/lifecycle ──→ infra/notification (emit_notification)
//
//  monitor/adapter_watch ─ (无跨模块调用，仅依赖 state + network)
//                          CancellationToken 可退出
//
//  commands/network_cmd ──→ network (适配器/质量检测)
//                       └──→ network/timing (DNS/DoH 测试)
//                       └──→ platform/dns_config (DNS/DoH 读写)
//                       └──→ platform/elevation (UAC 提权)
//
//  耦合问题：
//    1. monitor/watcher 是核心调度器，同时依赖 auto_auth/lifecycle/latency 三个子模块，
//       任何子模块的接口变更都会影响 watcher
//    2. auto_auth 同时调用 lifecycle 和 notification，形成 watcher→auto_auth→lifecycle
//       的三层调用链，中间层的变更会向上传播
//    3. emit_notification 被 auto_auth/lifecycle/latency 三处调用，是事实上的共享工具，
//       但定义在 infra/notification 模块中，语义上更清晰
//
// 所有模块通过 AppState 共享状态（见 infra/state/ 子目录），状态一致性依赖原子操作和 ArcSwap 保证
// 后台任务通过 BackgroundTaskManager 统一管理 cancel token，响应退出信号避免退出挂起
```

### 3.3 数据流

```
用户操作 → React组件 → useAppStore → useIpc.invoke()
                                         ↓
                                    Tauri IPC
                                         ↓
                              #[tauri::command] Rust函数
                                         ↓
                              AppState (ConfigStore + TaskFlags + BackgroundTaskManager + NetworkState + ExitStateStore)
                                         ↓
                              Win32 API / HTTP / 注册表 / 文件系统
                                         ↓
                              结果返回 / 事件推送 (emit)
                                         ↓
                              useIpc.listen() → useAppStore → UI更新
```

---

## 四、后端模块详解 (Rust)

### 4.1 应用入口 — `main.rs`

**职责**: 应用初始化、Tauri 插件注册、窗口/托盘/事件处理

**关键流程**:

1. **Tauri 插件注册**:
   - `tauri-plugin-shell` — 执行外部命令
   - `tauri-plugin-notification` — 系统通知
   - `tauri-plugin-autostart` — 开机自启
   - `tauri-plugin-global-shortcut` — 全局快捷键 (Ctrl+Shift+C 取消自动退出)
   - `tauri-plugin-single-instance` — 单实例锁
2. **Tokio 线程池配置**: 根据 CPU 核心数动态配置 `worker_threads(2-8)` 和 `max_blocking_threads(8-64)`
3. **panic hook**: `log_error!` 写入日志文件 + `flush_quick`（500ms超时）确保日志落盘 + `eprintln` 兜底（release 模式 `windows_subsystem=windows` 不可见但保留）
4. **Setup 钩子**:
   - 创建数据目录
   - 加载配置 (含密码DPAPI解密)
   - 根据 `--autostart` 参数和 `hiddenStart` 配置决定是否显示窗口
   - 创建系统托盘
   - 启动适配器监控和启动任务 (通过 `run_startup_tasks`)
   - **3 秒保底 showWindow**：独立线程 3 秒后检查窗口可见性，不可见则强制 `window.show()` + `set_focus()`，最多重试3次，防止前端初始化异常导致窗口永远隐藏
   - **前端心跳监控**：独立线程每 5 秒检查 `last_render_heartbeat_ms`，连续 3 次超过 20 秒无心跳则重载 WebView
5. **WebView2 内存管理**: `on_window_event` Focused 时通过 `ICoreWebView2_19.SetMemoryUsageTargetLevel` 调节（前台 NORMAL，后台 LOW）
6. **GPU 动态浏览器参数**: `build_browser_args()` 根据 GPU 厂商动态设置 WebView2 参数（ANGLE 后端/SkiaGraphite/DrDc）
7. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭，退出时使用 `force_release()` 清理任务标志
8. **退出流程**: cancel token → 短暂等待后台任务响应 → force_release 兜底 → `exit(0)`，窗口关闭与托盘退出行为统一
9. **命令注册**: 50个 `#[tauri::command]` 函数

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
    pub fn increment_a1_auth_failure_count(&self);
    pub fn increment_a2_auth_failure_count(&self);
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
    pub last_update_check_epoch_ms: AtomicU64,
    pub update_notified: AtomicBool,
    pub last_disabled_notification_ms: AtomicU64,
    pub last_render_heartbeat_ms: AtomicU64,
}
```

**说明**：相比旧版，`config`/`network`/`exit` 三个字段均升级为对应的 Store/State 封装类型；新增 `task_manager: BackgroundTaskManager` 字段，承接原 `TaskFlags` 中迁移出去的取消令牌职责。`AppState` 不再直接持有 `update_config` 方法，配置 CAS 更新改走 `state.config.update(...)`。

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
| `maxDisconnectReconnect` | u32 | 3 | 断线重连最大次数 |
| `autoLoginCooldownSecs` | u64 | 60 | 自动登录冷却秒数 |
| `logRetentionDays` | u32 | 7 | 日志保留天数 |
| `configVersion` | u32 | 2 | 配置版本号 |

**关键函数**:

| 函数 | 说明 |
|------|------|
| `atomic_write()` | 原子写入文件，3次重试+100ms间隔，失败保留临时文件 |
| `list_account_names()` | 共享函数，统一账号目录遍历逻辑 |
| `validate_username()` | 校验用户名 (位于 validate.rs) |
| `validate_operator()` | 校验运营商后缀 (返回 Result，非法值返回错误而非静默清空，位于 validate.rs) |
| `validate_password()` | 校验密码 (位于 validate.rs) |
| `deserialize_non_empty_or()` | 自定义反序列化器，空字符串自动回填默认值 (位于 model.rs) |

### 4.4 加密工具 — `account/crypto.rs`

Windows DPAPI 加密/解密，绑定当前 Windows 用户。空数据加密结果返回 `Err` 而非静默成功。

### 4.5 网络模块 — `network/`

#### 4.5.1 缓存基础设施 — `network/client.rs`

**`NetworkCache` 结构体** (全局单例 `NET_CACHE`):

```rust
struct NetworkCache {
    pub adapter: ArcSwap<Option<AdapterCache>>,      // 适配器缓存 (TTL=5s)
    pub gateway: ArcSwap<Option<GatewayCacheEntry>>, // 网关缓存
    pub portal: ArcSwap<Option<PortalCacheEntry>>,   // Portal状态缓存
    pub portal_url: ArcSwap<String>,                 // Portal URL
}
```

**HTTP 客户端池** (`CLIENT_POOL: DashMap`):

- Key = `local_addr:tls_version:timeout`，池上限 32 个连接
- `create_safe_http_client(timeout, local_addr)` — TLS 1.3 优先 + TLS 1.2 降级，`no-cache/no-store` 头

**关键函数**:

| 函数 | 说明 |
|------|------|
| `clear_adapter_cache_only()` | 仅清除适配器缓存 |
| `clear_portal_cache()` | 仅清除 Portal 状态缓存 |
| `create_safe_http_client(timeout, local_addr)` | 创建 HTTP 客户端 (TLS 1.3 强制 + TLS 1.2 回退) |
| `update_portal_url(url)` | 更新全局 Portal URL |

#### 4.5.2 适配器查询 — `adapter.rs`

- Win32 API `GetAdaptersAddresses` 查询适配器
- TTL 5秒缓存，`get_adapters_force()` 先清除缓存再查询
- DHCP 续租、网关检测、适配器启用
- **适配器状态四分类** (`AdapterStatus` 枚举):
  - `Disabled` — 已禁用（OperStatus Down/NotPresent，管理员禁用或硬件缺失）
  - `Disconnected` — 未连接（OperStatus LowerLayerDown/Dormant，线缆未插或USB网卡未连接）
  - `EnabledNoIp` — 未禁用无IP（OperStatus Up 但无有效 IP，含 169.254 APIPA 清空后）
  - `Connected` — 已连接（OperStatus Up 且有有效 IP）
- **适配器可见性双重验证**:
  - `is_visible_in_ncpa()` — 注册表双重检查：`ShowInNetworkConnections` + Class subkey PnP 设备树交叉验证，过滤幽灵虚拟副本
  - `is_admin_disabled_via_registry()` — `ConfigFlags 0x1` 检测管理员禁用，区分"管理员禁用"vs"硬件缺失(USB未连接)"
  - **`ensure_cache_initialized` 锁优化**（位于 `network/discovery/registry.rs`）：`CLASS_SUBKEY_CACHE` 懒加载初始化采用双重检查锁定，`build_class_subkey_cache()`（注册表遍历，慢 I/O）在锁外执行，仅用写锁做 swap，避免阻塞读锁请求（`class_subkey_has_matching_guid`/`is_admin_disabled_via_registry`）。`refresh_class_subkey_cache` 同样采用锁外构建模式。
- **校园网检测**:
  - `get_connected_network_names()` — 获取当前连接的网络名称 (Wi-Fi SSID + 以太网配置文件)
  - `check_gateway_reachable()` — Ping 检测网关可达性
  - `is_same_subnet_18()` — /18 子网匹配检测

#### 4.5.3 Portal 检测 — `auth/portal.rs`

- Portal 认证状态检测
- URL `:801` 端口追加逻辑统一处理
- v 参数使用 `random_v()` 随机生成
- NAT 内网 IP 检测，NAT 环境下不发送 `wlan_user_ip`
- `PortalStatus` 新增 `error_kind` 字段区分"请求失败"与"Portal不可达"
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

**两步注销流程** (2轮循环，每轮先 MAC 解绑再 Radius 注销):

```
步骤1: MAC 解绑
  GET /eportal/portal/mac/unbind?callback=dr1002
      &user_account={学号}&wlan_user_mac=000000000000
      &wlan_user_ip={IP整数}&jsVersion=4.1.3&v={random}&lang=zh
  成功: result=0, msg="解绑终端MAC成功！"

步骤2: Radius 注销
  GET /eportal/portal/logout?callback=dr1004&login_method=1
      &user_account=drcom&user_password=123&ac_logout=1
      &register_mode=1&wlan_user_ip={IP}&wlan_user_mac=000000000000
      &jsVersion=4.1.3&v={random}&lang=zh
  成功: result=1, msg="Radius注销成功！"
```

**注销成功判定**:
- 两步均成功 → 注销成功
- Radius 注销成功 + MAC 解绑失败 → "Radius注销成功，MAC解绑失败"
- `/logout` 接口 `result=1` 表示 Radius 注销成功
- `/mac/unbind` 接口 `result=0` 且 msg 含"解绑终端MAC成功"表示解绑成功
- `result=0` 但 msg 含错误关键词（"非法"/"失败"/"错误"/"拒绝"）→ 失败

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
| HTTPS 不绑定适配器 | `bind_addr: None`，让系统路由表决定出口网卡，避免校园网绑定主适配器 IP 导致外网 TCP 超时 |
| DNS 解析优先 IPv4 | `resolve_host_uncached_with_bind` 中优先返回 `is_ipv4()` 的结果，避免 IPv6 地址导致连接失败 |
| 增量推送 | `app_handle: Option<&AppHandle>` 参数，Phase 1 和 HTTPS 批次完成后立即 emit，前端逐步填充数据 |
| HTTPS 分批并发 | Phase 2 改为每批 4 个分批并发，减少校园网高 RTT 环境下 TLS 带宽竞争 |
| 前端不再主动触发 | 移除前端 `qualityPromise`，由后端 latency loop 统一管理质量检测时机 |
| 前端防抖移除 | 移除 500ms 防抖，增量推送事件可立即更新 UI |
| 启动延迟 1 秒 | latency loop 启动后先 sleep 1s 再开始检测，避免网络未稳定时 HTTPS 延迟异常 |
| RAII guard 替代手动锁 | `is_quality_checking.try_acquire()` 返回 `TaskGuard`，作用域结束自动释放，替代 `swap_acquire + force_release` |
| DNS/DoH Sleep 优化 (v2.2.6) | enable_doh_for_dns cmd路径 sleep 2.5s→1.5s，setup_dns_doh PowerShell路径 2s→1.5s，cmd路径 3s→2s |
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

### 4.7 登录/注销模块 — `commands/login.rs` + `auth/session.rs`

**登录命令**:

| 函数 | 说明 |
|------|------|
| `do_login(adapter_name?)` | Tauri 命令，支持可选指定适配器 |
| `full_login_inner()` | 登录核心逻辑 |
| `login_adapter_with_log()` | 单适配器登录+日志 |

**注销命令**:

| 函数 | 说明 |
|------|------|
| `do_logout(adapter_name?)` | Tauri 命令，支持可选指定适配器 |
| `full_logout_inner()` | 注销核心逻辑 (双适配器串行) |
| `logout_adapter_with_log()` | 单适配器注销+日志 |

**锁语义**: 登录使用 `is_logging_in`，注销使用独立的 `is_logging_out`

**认证失败计数与 MAC 重置** (session.rs 内部函数):

| 函数 | 说明 |
|------|------|
| `update_auth_failure_count()` | 单适配器认证失败计数，连续5次(ac_auth_failed/1/4)触发 MAC 重置+DHCP 续租 |
| `update_dual_adapter_auth_failure()` | 双适配器分别计数，各自5次触发单适配器 MAC 重置 |
| `handle_single_adapter_failure()` | 处理单个适配器的认证失败计数与 MAC 重置 |

**注销成功后状态重置** (v2.2.5 区分全量/单适配器):

- **全量注销**（未指定 `adapter_name`）：重置 `any_adapter_online`/`last_a1_online`/`last_a2_online`/`has_logged_online` 为 false，`disconnect_reconnect_count` 归零，重置 `last_auto_login_attempt` 为当前时间，取消自动退出倒计时，设置 60 秒注销保护期 (`logout_protected_until`)
- **单适配器注销**（指定 `adapter_name`）：仅重置对应适配器的 `last_a1_online` 或 `last_a2_online`，重新计算 `any_adapter_online = a1 || a2`，其余标志保持不变

### 4.8 后台巡检调度器 — `monitor/watcher.rs`

**职责**: 纯调度器，职责分离重构

**核心类型**:

```rust
enum PortalCheckResult {
    Success { online: bool, message: String, reachable: bool, login_available: bool },
    Error { is_request_failed: bool },
    NotFound,
}

struct CampusCheckResult {
    wifi: Option<ConnectionCampusStatus>,   // WiFi 校园网状态
    wired: Option<ConnectionCampusStatus>,  // 有线校园网状态
}

struct ConnectionCampusStatus {
    on_campus: bool,    // 是否在校园网
    name: String,       // 连接名称
    message: String,    // 状态消息
}
```

**提取的独立函数**:

| 函数 | 说明 |
|------|------|
| `check_adapter_portal()` | 消除主/副适配器检测逻辑重复 |
| `build_adapter_details()` | 适配器详情构建 |
| `handle_status_change()` | 状态变更通知 |
| `emit_background_check_result()` | 统一检测结果 JSON 构建和事件发送（含 campusWifi/campusWired/a1CampusMessage/a2CampusMessage/a1OnCampus/a2OnCampus 字段） |
| `update_network_state()` | 独立网络状态更新逻辑 |
| `adapter_status_entry()` / `adapter_disabled_entry()` / `adapter_disconnected_entry()` | 适配器状态条目构建 |
| `check_campus_network()` | WiFi/有线分别检测校园网状态 |

**Handle::enter 上下文修复** (v2.2.7 修复): 双适配器并行 Portal 检测使用 `std::thread::scope` 启动子线程，但子线程无 Tokio reactor 上下文，导致 `check_portal_full` 中的 `block_on_http`（`Handle::try_current().block_on()`）panic "there is no reactor running"。修复方式：在 `run_background_check_blocking` 中通过 `tokio::runtime::Handle::current()` 取得当前 runtime handle，子线程入口处调用 `let _guard = h.enter();` 设置 Tokio 上下文，使 `Handle::current().block_on()` 能正确工作。`_guard` 在子线程退出时 RAII 释放。

```rust
let runtime_handle = tokio::runtime::Handle::current();
std::thread::scope(|s| {
    let h1 = runtime_handle.clone();
    let t1 = s.spawn(move || {
        let _guard = h1.enter();           // 设置 Tokio 上下文
        check_adapter_portal(adapter1, app_handle)
    });
    // ...
});
```

**校园网检测集成**: 后台检测中集成三级校园网检测（网络名称→/18子网→网关Ping），检测结果包含 `currentSsid` 和 `onCampusNetwork` 字段。**无网络保护**：当配置的适配器均无IP时（完全无网络连接），跳过校园网退出流程，等待网络恢复后重新检测，避免误判"非校园网"触发退出

**注销保护期机制**: 注销成功后设置 60 秒保护期 (`logout_protected_until`)，期间：
- `update_network_state` 跳过原子状态更新
- `emit_background_check_result` 强制 `online=false`，避免 Portal 延迟导致前端误判
- `check_portal_status` API 直接返回 `{ online: false }`，不再请求 Portal 服务器

**Portal 容错机制** (v2.2.5 新增, v2.2.7 增强):

`NetworkStatus.portal_failure_count: AtomicU32` 字段记录 Portal 请求连续失败次数。后台巡检中：

1. 每次主/副适配器 Portal 请求失败（`is_request_failed: true`）时，对应适配器的 `a1_auth_failure_count` / `a2_auth_failure_count` 自增（按适配器分别计数）
2. Portal 失败时先检查网关从该适配器IP是否可达（`check_gateway_reachable_from()`），不可达则跳过计数（校园网断网/维护场景）
3. 连续 5 次失败时触发 `dhcp_release_renew_all`（MAC 重置 + DHCP 续租），仅对校园网子网适配器生效
4. 触发后重置计数器为 0
5. Portal 检测恢复正常（`Success`）时，`swap(0)` 重置计数器并记录原值日志

**新增函数**:

| 函数 | 说明 |
|------|------|
| `check_gateway_reachable_from()` | 从指定适配器IP检查网关可达性，不可达时跳过容错计数 |

**量化改进**:

| 指标 | 重构前 | 重构后 |
|------|--------|--------|
| `run_background_check_blocking` 行数 | ~190 行 | ~88 行 |
| 重复 JSON 构建代码 | 3 处 | 0 处 |
| 主函数圈复杂度 | 15+ | ~5 |

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
| `start_campus_exit()` | 校园网验证不通过时：30s后最小化到托盘，再30s后强制退出 (受 `campus_exit_on_fail` 控制)。先设置 deadline 再 CAS 设置标志位，避免状态不一致 |
| `cancel_campus_exit()` | 取消校园网退出流程。如果自动退出未运行，注销快捷键 |
| `cancel_campus_exit_with_notification()` | 快捷键取消校园网退出 (含通知和快捷键注销) |

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
| `enable_doh_for_dns()` | 启用 DoH (COM ShellExecuteW 提权 `shell_exec_elevated` 优先 → netsh → 回退 `run_elevated`) |
| `setup_dns_doh()` | 一键设置推荐 DNS + DoH (WiFi用配置文件级DNS，有线用适配器级DNS，COM ShellExecuteW 提权 `shell_exec_elevated` 优先，回退 `run_elevated`) |

**UAC 提权** (位于 `platform/elevation.rs`):

```rust
fn is_admin() -> bool {
    // OpenProcessToken + GetTokenInformation(TokenElevation) 检测管理员权限
}

fn run_elevated(cmd: &str, args: &str) -> Result<(), String> {
    // ShellExecuteW + "runas" 实现UAC提权，耗时约1ms
}

fn shell_exec_elevated(cmd: &str, args: &str) -> Result<(), String> {
    // COM ShellExecuteW 提权方式，作为 run_elevated 的优先替代
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

**安全**: 适配器名称校验（禁止 `&|;` 等元字符），含空格适配器名使用引号包裹

### 4.14 其他命令模块

**config_cmd.rs** — 配置保存/加载 (委托 `config/persist.rs`)，空密码兜底逻辑 (前端未传密码且旧密码存在时保留旧密码)

**account.rs** — 多账号管理 (委托 `account/mod.rs`)，使用 `list_account_names()` 共享函数，切换账号仅替换账号相关字段保留启动设置，删除账号前检查并清空 `active_account`

**system.rs** — 系统功能命令，`get_init_data` 使用 `list_account_names()`，新增返回字段 `gpuInfo`/`refreshRate`；新增 `append_login_history()` 登录历史记录（最多100条）

**updater.rs** — 更新命令 (委托 `update/updater.rs`)，SHA256 校验缺失时拒绝安装，MSI 安装使用 `raw_arg` 支持含空格路径，403 返回中文友好提示

---

## 五、前端模块详解 (React/TypeScript)

> **架构说明**: 前端采用业务域分目录架构，每个业务域目录包含面板组件、逻辑 Hook、类型定义和模块导出。类型定义分散在各业务域的 `types.ts` 中，而非集中在一个 `types/index.ts` 文件。

### 5.1 状态管理 — `hooks/useAppStore.ts`

- 基于 zustand ^5.0 的全局状态管理
- 密码处理：`password === '***'` 时直接 `delete` 密码字段，三层防护过滤后端返回的遮蔽值
- 注销状态：`isLoggingOut` + `doLogout` action
- 登录/注销均支持 `adapterName` 可选参数
- `checkOnline` 使用 epoch 计数器防竞态 + 并发锁(`_checkOnlineLockFlag`)，包含完整校园网检测逻辑（campusWifi/campusWired/a1OnCampus/a2OnCampus 等）
- `doLogin` 使用 `get()` 获取最新配置避免 Stale Closure，成功后自动调用 `checkNetworkQuality`
- `i18next.t()` 在 action 函数体内调用（非 Store 创建时），避免初始化时序问题
- `localStorage` 已替换为 `safeStorage`（内存降级封装），避免隐私模式下 localStorage 不可用
- **新增状态字段**: `passwordSaved`/`isRefreshingQuality`/`isRefreshingAdapters`/`refreshRate`/`language`
- **新增 Action**: `updateConfigLocal`/`syncPasswordSaved`/`addToastWithAction`/`removeToastsByPrefix`/`refreshAdapters`/`setGpuInfo`/`setLanguage`/`initTheme`/`setCustomThemeColor`/`setUpdateAvailable`/`setLatestVersion`/`setReleaseNotes`
- **密码保存防抖**: `saveConfigPending` + `saveConfigTimer` 实现 500ms 防抖保存，`flushPendingConfig`/`hasPendingConfig` 导出
- **刷新锁机制**: `refreshQuality`/`refreshAdapters` 含 500ms 冷却期锁(`_qualityLockFlag`/`_adapterLockFlag`)
- **主题订阅**: `useAppStore.subscribe` 监听 `isLightMode`/`themeName`/`customThemeColor` 变化

### 5.2 IPC 封装 — `hooks/useIpc.ts`

**新增类型**: `ConnectionCampusStatus`/`CampusStatusResult`

**事件监听器** (通过 `createEventListener` 工厂函数):

| 事件监听器 | 说明 |
|------------|------|
| `onBackgroundCheckResult` | 后台检测结果 |
| `onAutoLoginResult` | 自动登录结果 |
| `onAdaptersChanged` | 适配器状态变更 |
| `onAdapterDetailsChanged` | 适配器详情变更 |
| `onDisabledAdaptersChanged` | 禁用适配器变更 |
| `onAdapterDisabledWarning` | 适配器禁用警告 |
| `onLoginLog` | 登录/注销日志 |
| `onAutoExitCountdown` / `onAutoExitCancelled` | 自动退出倒计时/取消 |
| `onCampusExitCountdown` / `onCampusExitCancelled` | 校园网退出倒计时/取消 |
| `onNetworkQualityResult` | 网络质量结果 |
| `onSystemNotification` | 系统通知 |
| `onUpdateAvailable` | 更新可用 |
| `onDownloadProgress` | 下载进度 |
| `onConfigChanged` | 配置变更 |

**API 清单**:

| API | 说明 |
|-----|------|
| `doLogin(adapterName?)` | 登录，可选指定适配器 |
| `doLogout(adapterName?)` | 注销，可选指定适配器 |
| `checkDnsDohStatus()` | 检测 DNS/DoH 状态 |
| `setupDnsDoh()` | 一键设置推荐 DNS + DoH |
| `installUpdate(checksumUrl)` | 安装更新，传递 SHA256 校验URL |
| `checkCampusStatus()` | 检测校园网状态 |
| `enableAdapter(name)` | 启用适配器 |
| `dhcpRenewAll()` | DHCP 全部续租 |
| `dhcpReleaseRenew()` | DHCP 释放续租 |
| `dhcpReleaseRenewAdapter(name)` | 指定适配器 DHCP 释放续租 |
| `renderHeartbeat()` | 前端心跳 |
| `getGpuInfo()` | 获取 GPU 信息 |
| `getLogRetentionDays()` | 获取日志保留天数 |
| `setLogRetentionDays(days)` | 设置日志保留天数 |

**新增重试机制**: `tauriApiWithRetry` 对 `saveConfig`/`checkPortalStatus`/`checkNetworkQuality` 自动重试（最多2次，指数退避+随机抖动）

**openExternal 安全逻辑**: 协议白名单(http/https) + URL长度限制(2048) + URL解析验证，先尝试 invoke 失败后 fallback 到 shell 插件

**类型定义**: 分散在各业务域的 `types.ts` 文件中（account/auth/monitor/network/settings/shared），合计 40+ 个类型/接口定义

### 5.3 初始化逻辑 — `hooks/useAppInit.ts`

- 在线日志去重：5秒内在线日志自动去重
- 主/副适配器状态合并显示：`"已在线（以太网、WLAN）"`
- 监听器先于数据获取注册，避免遗漏初始化期间事件
- `mountedRef` 保护异步回调，避免卸载后写入
- **catch 块中 `showWindow` 不受 `mountedRef` 影响**：窗口显示是应用级别操作，即使组件已卸载仍需执行
- **前端不再主动调用 `checkNetworkQuality`**：移除 `qualityPromise`，由后端 latency loop 统一管理
- **系统通知由后端统一发送**：前端不再重复调用 `api.sendNotification`，由 `notify_network_quality_change` 统一处理
- **网络质量事件无防抖**：移除 500ms 防抖，增量推送的 Phase 1 和 HTTPS 批次结果可立即更新 UI
- **WebGL GPU 检测校正**: `getWebGlRenderer`/`parseWebGlGpu`/`classifyTierFromWebGl`/`correctGpuInfoWithWebGl`
- **网络质量告警**: `handleQualityBadAlert`
- **事件监听节流**: `background-check-result` 1000ms, `adapters-changed` 500ms
- **新增事件监听器**: `adapter-details-changed`/`adapter-disabled-warning`/`login-log`/`campus-exit-countdown`/`campus-exit-cancelled`/`system-notification`/`update-available`/`config-changed`
- **窗口关闭处理**: `onCloseRequested` 时 `flushPendingConfig`
- **前端心跳**: 每5秒调用 `renderHeartbeat`
- **Ctrl+Shift+C 快捷键**: 前端也注册
- **DNS 初始化检查**
- **崩溃恢复** (`setupCrashRecovery`): 最多3次自动重载，GPU/WebGL/SharedArrayBuffer 错误触发重载，渲染心跳检测5秒无心跳视为GPU崩溃，页面可见性变化时暂停/恢复 GSAP globalTimeline

### 5.4 业务域模块

#### 5.4.1 认证模块 — `auth/`

| 文件 | 说明 |
|------|------|
| `DashboardPanel.tsx` | 总览面板，卡片可拖拽排序（framer-motion Reorder.Group），3种子组件（QuickActionsCard/AccountManageCard/NetworkQualityCard），布局持久化到localStorage |
| `AboutDialog.tsx` | 关于对话框，双栏布局(应用信息+更新仪表盘)，镜像源选择，下载状态机(idle→selecting→downloading→done/error)，Release Notes渲染 |
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
| `MonitorPanel.tsx` | 监控面板，3卡片(网络状态检测/适配器在线状态/验证设置)+校园网验证配置 |
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
| `types.ts` | 网络类型定义 (AdapterStatus四分类: disabled/disconnected/enabledNoIp/connected, Adapter, DnsDohStatus, DnsServerInfo 等 10 个类型) |
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
| `SettingsPanel.tsx` | 设置面板，4卡片(外观/启动设置/通知/质量检测)+7种主题+12色预设+取色器+亮暗模式 |
| `ThemeDialog.tsx` | 主题对话框，2列布局+亮暗模式切换 |
| `OnboardingWizard.tsx` | 4步引导向导(欢迎→账号→适配器→完成)，Framer Motion滑动转场，含语言切换，完成后自动登录 |
| `useSettings.ts` | 设置逻辑 Hook |
| `constants.ts` | 设置常量 (DEFAULT_CONFIG/ISP_OPTIONS(4种)/THEME_OPTIONS(7种)/VALID_THEMES/DEFAULT_PANEL_OPTIONS) |
| `types.ts` | 设置类型定义 (Config(35字段,不含logRetentionDays/configVersion), AutoLaunchResult, InitData) |
| `index.ts` | 模块导出 |

### 5.5 共享组件 — `shared/`

| 文件 | 说明 |
|------|------|
| `LogPanel.tsx` | 日志面板 (级别过滤/模块过滤/关键词搜索/行数选择/保留天数/Debug模式/GSAP清空动画/自动滚动/5秒刷新) |
| `ErrorBoundary.tsx` | React Class Component 错误边界，显示错误信息+重新加载按钮 |
| `ConfirmDialog.tsx` | 确认对话框 |
| `FluidBackground.tsx` | 简单背景层，使用CSS变量 `--surface-main` |
| `AnimatedNumber.tsx` | 动画数字，GSAP quickTo驱动，支持unit/decimals/duration，economy档禁用scale弹跳 |
| `RefreshButton.tsx` | 刷新按钮，旋转动画+完成时shake效果 |
| `SegmentTabs.tsx` | 分段Tab，Framer Motion layoutId滑块动画+TabContent(AnimatePresence) |
| `ToastContainer.tsx` | Toast容器，4种类型(info/success/error/warning)，economy档简单transition替代spring，支持action按钮 |
| `types.ts` | 共享类型定义 (UpdateAvailableData, UpdateInfo, DownloadProgress, MirrorSource 等) |
| `ui-types.ts` | UI 类型定义 (StatusState, PanelName(8个面板含speedtest), ThemeName(7种), LogType, GpuTier, GpuInfo, LogEntry, ToastMessage, AdapterDisabledWarningData, AutoExitCountdownData, SystemNotificationData, SaveConfigResult 等) |
| `ui-constants.ts` | UI 常量 (MAX_LOG_ENTRIES=300/APP_VERSION='2.2.8'/APP_NAME='校园网登录助手'/PASSWORD_MASK='***'/NAV_ITEMS=8个导航项) |
| `index.ts` | 模块导出 |

### 5.6 布局组件 — `components/layout/`

| 文件 | 说明 |
|------|------|
| `DockNav.tsx` | 适配器选择浮层 + 注销按钮 (无线蓝色Wifi/有线绿色Cable图标, 200ms延迟关闭)，GSAP 磁吸效果（MAGNETIC_RANGE=80, MAX_SCALE=1.35, MAX_LIFT=-14），economy档禁用磁吸，RAF节流 |
| `RightPanel.tsx` | 右侧面板，运行日志+网络适配器信息(可展开/折叠，显示IP/子网掩码/网关/DHCP/MAC)，空日志时呼吸动画 |
| `TitleBar.tsx` | 标题栏，应用图标+版本号+更新提示+工具按钮(亮暗/语言/通知/主题/关于/最小化/最大化/关闭)，双击最大化，拖拽移动窗口 |

### 5.7 延迟颜色 — `lib/latency.ts`

- `QUALITY_CONFIG` 新增显式 `borderBg` 字段，确保 Tailwind JIT 可扫描
- `getLatencyColor()` 使用 `cfg.borderBg` 替代动态字符串替换
- `getLatencyLevel()` — 延迟等级计算
- `extractGatewayLatency()` — 提取网关延迟
- `extractExternalLatency()` — 提取外网延迟

### 5.8 国际化 — i18n/

- 基于 react-i18next + i18next-browser-languagedetector
- 翻译文件按 namespace 分组：nav, titlebar, dock, auth, settings, monitor, log, rightPanel, about, common, onboarding
- 非组件中使用 `import i18next from 'i18next'` + `i18next.t()` 而非 useTranslation hook
- 常量文件（NAV_ITEMS、ISP_OPTIONS、THEME_OPTIONS、QUALITY_CONFIG）添加 labelKey 字段，运行时通过 t(labelKey) 翻译
- 默认语言中文，i18n 仍使用 `localStorage`（非 safeStorage），仅 `useAppStore.setLanguage` 使用 `safeStorage`

### 5.9 其他 Hooks — `hooks/`

| Hook | 说明 |
|------|------|
| `useAnimationProfile.ts` | 动画配置，AnimationTier(high/standard/economy) + AnimationProfile 17字段，economy档禁用willChangeOrbs/enableTilt/startupBoost |
| `useAsyncLock.ts` | 异步锁，`useAsyncLock<T>(fn, cooldownMs=1500)` 防止并发调用 |
| `useBreatheAnimation.ts` | 呼吸动画，GSAP yoyo循环，支持opacity/scale/rotation，空闲时暂停 |
| `useGlowAnimation.ts` | 发光动画，GSAP yoyo循环，opacity+scale，空闲时暂停 |
| `useLogToastStore.ts` | 独立 zustand store，MAX_LOG_ENTRIES=300 |
| `usePageIdle.ts` | 页面空闲检测，2秒空闲超时，useAnimationActive = isVisible && isFocused && !isIdle |
| `usePulseAnimation.ts` | 脉冲动画，三种类型: heartbeat(3s循环)/statusPulse(1.5s重复2次)/loadingPulse(1.2s循环) |
| `useRipple.ts` | 涟漪效果，点击创建span.ripple-effect，animationend后移除 |
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

- **GSAP 全局配置**: `expo.out` 默认缓动, `force3D: true`, `autoSleep: 5`, `lagSmoothing(500, 33)`
- **prefers-reduced-motion**: GSAP duration 设为 0
- **主题初始化**: `initTheme()` — 从 localStorage 恢复亮暗模式 + 主题类
- **崩溃恢复** (`setupCrashRecovery`): 最多3次自动重载，GPU/WebGL/SharedArrayBuffer 错误触发重载
- **渲染链**: `ErrorBoundary` > `LazyMotion(domAnimation)` > `MotionConfig(reducedMotion="user")` > `App`
- **开发模式**: 使用 `React.StrictMode`

---

## 六、IPC 通信完整清单

### 6.1 请求-响应命令 (v2.2.8: 50个)

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
| `tauri` | 2 | 应用框架 |
| `tauri-plugin-*` | 2 | shell/notification/autostart/global-shortcut/single-instance |
| `serde` / `serde_json` | 1 | 序列化 |
| `tokio` | 1 (rt-multi-thread, time, net, macros, io-util) | 异步运行时 |
| `tokio-util` | 0.7 (rt) | CancellationToken |
| `reqwest` | 0.12 (json, http2, rustls-tls, charset) | HTTP客户端 |
| `tokio-rustls` | 0.26 | TLS 连接 (DoH) |
| `rustls-pki-types` | 1 | TLS 类型 |
| `webpki-roots` | 0.26 | TLS 根证书 |
| `hickory-resolver` | 0.24 (tokio-runtime) | DNS 解析 |
| `dashmap` | 6 | 并发 HashMap (DNS 评分/缓存) |
| `parking_lot` | 0.12 | 高性能同步原语 |
| `arc-swap` | 1 | 原子引用交换 |
| `windows` | 0.58 | Win32 API (含 Shell/UI/Threading/Graphics/Dxgi) |
| `webview2-com-sys` | 0.38 | WebView2 COM 接口 (ICoreWebView2_19 内存管理) |
| `windows-core` | 0.61 | Windows COM 核心类型 |
| `winreg` | 0.52 | Windows 注册表读写 |
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

### 7.2 前端依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `react` / `react-dom` | 19 | UI框架 |
| `@tauri-apps/api` | ^2 | Tauri前端API |
| `framer-motion` | ^12.38.0 | 动画 |
| `react-i18next` | ^17 | 国际化 |
| `i18next-browser-languagedetector` | ^8 | 语言自动检测 |
| `i18next` | ^26.3.1 | i18n 核心 |
| `lucide-react` | ^0.446 | 图标 |
| `zustand` | ^5.0.13 | 状态管理 |
| `gsap` | ^3.15.0 | 高性能动画 (frontend package.json) |
| `tailwindcss` | ^3.4 | CSS框架 |
| `vite` | ^6 | 构建 |
| `typescript` | ^5.5 | 类型系统 |
| Radix UI primitives | 各版本 | 无障碍UI |

### 7.3 依赖关系图

```
main.rs
  ├── lib.rs (Tauri库入口)
  └── commands/mod.rs
        ├── config_cmd.rs ← config/, account/crypto.rs, infra/state.rs
        ├── login.rs ← auth/session.rs, auth/portal.rs, auth/protocol.rs, infra/state.rs
        │   [do_login + do_logout (两步注销), adapter_name 可选参数]
        ├── background.rs (命令入口，委托 monitor::watcher)
        ├── network_cmd.rs ← network/*, infra/state.rs, network/timing.rs, platform/dns_config.rs, platform/elevation.rs
        │   [check_dns_doh_status / setup_dns_doh / run_elevated]
        ├── account.rs ← config/, account/crypto.rs, infra/state.rs, config_cmd.rs
        ├── system.rs ← config/, network/*, infra/state.rs, platform/dns_config.rs
        └── updater.rs ← update/updater.rs

infra/
  ├── state.rs ← config/model.rs, arc-swap, parking_lot, tokio-util
  │   [TaskLock/TaskGuard 抽象]
  │   [ExitState / AppState 分层]
  │   [CancellationToken 管理后台任务退出]
  ├── logger.rs — 日志系统 (flush_quick: panic hook 专用 500ms 超时 flush; cleanup_old_logs_by_time: retention_days==0 时永久保留)
  ├── lifecycle.rs ← infra/state.rs, infra/notification.rs
  │   [start_auto_exit / cancel_auto_exit_inner / start_campus_exit]
  └── notification.rs — emit_notification 封装

monitor/
  ├── watcher.rs (调度器, PortalCheckResult 职责分离, 校园网检测)
  │   ├── auto_auth.rs ← infra/state.rs, auth/session.rs, infra/notification.rs, infra/lifecycle.rs
  │   ├── latency.rs ← infra/state.rs, network/*, infra/notification.rs
  │   └── adapter_watch.rs ← infra/state.rs, CancellationToken
  └── mod.rs (重导出)

auth/
  ├── portal.rs ← network/client.rs, reqwest, url [random_v]
  ├── protocol.rs ← network/client.rs, reqwest, urlencoding, regex [random_v]
  │   [两步注销: Radius注销 + MAC解绑]
  └── session.rs ← auth/portal.rs, auth/protocol.rs, network/adapter.rs
      [full_login_inner / full_logout_inner]

network/
  ├── mod.rs (重导出)
  ├── client.rs ← arc-swap, lazy_static, dashmap, reqwest [TLS 1.3]
  ├── adapter.rs ← client.rs, windows, regex [TTL 5s 缓存]
  │   [校园网检测: 网络名称/子网/网关Ping]
  ├── dns.rs — DNS 缓存管理
  ├── timing.rs
  │   ├── DNS_SERVER_SCORES / DOH_SERVER_SCORES (dashmap 评分表)
  │   ├── resolve_host_smart (三级智能解析)
  │   ├── resolve_via_doh (RFC 8484 wire format)
  │   └── measure_https_timing / measure_dns_query / measure_doh_timing
  └── quality.rs ← adapter.rs, client.rs, surge-ping, tokio-rustls, timing.rs, tauri::AppHandle
      [两阶段检测: DNS/DoH → HTTPS(分批并发) + 增量推送]

platform/
  ├── dns_config.rs ← platform/elevation.rs, winreg
  │   [set_profile_dns_via_api / set_dns_via_api / set_doh_via_api / clear_adapter_dns_via_api]
  ├── elevation.rs — ShellExecuteW UAC 提权 + GUID 解析 + COM ShellExecuteW 提权 (shell_exec_elevated)
  ├── gpu.rs — GPU 信息检测 (DXGI EnumAdapters1) + 显示器刷新率检测 (EnumDisplaySettingsW) + 动态浏览器参数 (build_browser_args)
  │   [GpuInfo 含 gpu_preference: u8 (0=默认/1=节能/2=高性能, 读注册表 UserGpuPreferences)]
  │   [determine_tier: NVIDIA→discrete, Intel Arc→discrete, Iris Xe→mid-igpu, UHD→low/mid-igpu, AMD RX/Pro→discrete, 780M/880M→high-igpu]
  │   [build_browser_args: NVIDIA→d3d12+SkiaGraphite+DrDc, Intel/AMD→d3d11+SkiaGraphite+DrDc, 未知→d3d11+禁用DrDc]
  └── autostart.rs — 开机自启

config/
  ├── model.rs — Config 结构体 + Default
  ├── persist.rs — atomic_write + list_account_names
  └── validate.rs — 校验逻辑

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

App.tsx
  └── useAppStore (zustand, useShallow 选择性订阅)
        ├── useIpc.ts ← @tauri-apps/api
        └── useAppInit.ts
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
| 外部链接验证 | URL 验证 + 本地地址黑名单 |
| 登录频率限制 | 防止滥用 |
| panic=abort | 编译选项减小二进制体积，避免信息泄露 |
| force_release 防死锁 | 退出场景强制释放任务锁 |
| SHA256 更新校验 | 安装包完整性校验，校验缺失时拒绝安装 |
| 适配器名称校验 | 禁止 `&\|;` 等元字符，防止命令注入 |

---

## 九、性能优化

| 优化项 | 实现 | 效果 |
|--------|------|------|
| PowerShell 消除 | ShellExecuteW 替代 PowerShell UAC 提权 | 200-500ms → 1ms |
| GPU 检测改用 DXGI | `CreateDXGIFactory1` + `EnumAdapters1` 替代 PowerShell | 1~3s → 50~200ms |
| DNS 设置优化 | netsh + ShellExecuteW 替代 PowerShell | 1-3s → 100-300ms |
| DNS 注册表检测 | winreg 替代 PowerShell | 10x+ 速度提升 |
| 适配器 TTL 缓存 | 5秒 TTL，避免频繁调用 Win32 API | 减少 API 调用 |
| codegen-units=1 | LTO 跨单元优化 | +2~5% 性能 |
| panic=abort | 编译选项 | 体积↓5~10% |
| 登录重试可中断 | 每100ms检查退出标志 | 退出延迟 2s → 100ms |
| 前端选择性订阅 | useShallow 减少不必要重渲染 | UI 响应更流畅 |
| 高频事件节流 | 500ms 时间戳节流 | 防止 UI 频繁更新 |
| FluidBackground CSS动画移除 | 3个大型渐变层动画完全移除 | GPU进程CPU占用显著降低 |
| GSAP动画迁移 | 12个CSS动画迁移至GSAP，启用force3D+lazy+autoSleep | GPU合成层加速，空闲自动暂停 |
| RAF节流+位置去抖 | Button/DockNav/AnimatedCard鼠标事件节流 | 减少无效getBoundingClientRect调用 |
| transition-all替换 | 10处替换为显式属性列表 | 减少不必要的属性过渡计算 |
| WebView2 内存管理 | 前台 NORMAL/后台 LOW (ICoreWebView2_19.SetMemoryUsageTargetLevel) | 后台内存占用显著降低 |
| GPU 动态浏览器参数 | build_browser_args 根据 GPU 厂商设置 ANGLE 后端/SkiaGraphite/DrDc | 渲染兼容性和性能优化 |
| Tokio 线程池动态配置 | 根据 CPU 核心数配置 worker_threads(2-8)/max_blocking_threads(8-64) | 资源利用更合理 |
| CAS 原子配置更新 | update_config CAS 原子更新避免 TOCTOU 竞态 | 配置一致性保证 |
| 流式 SHA256 校验 | 分块流式读取计算 SHA256，64KB buffer | 大文件校验内存占用降低 |

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
let content = fs::read_to_string(conf_path)?;
let version = content.lines()
    .find_map(|line| {
        let t = line.trim();
        if t.starts_with("\"version\"") {
            Some(t.split(':').nth(1)?.trim()
                .trim_end_matches(',').trim_matches('"').to_string())
        } else { None }
    })?;

println!("cargo:rerun-if-changed=tauri.conf.json");
println!("cargo:rustc-env=APP_VERSION={}", version);
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
6. **同步静态预览** — `tauri-app/frontend/about-preview.html` 的 `app-version` 和 `status-version` 两个 div
7. **同步徽章** — `README.md` 的 `version-2.2.8` 徽章
8. **同步测试脚本** — `scripts/test-update-download.ps1` 的 `$VERSION` 变量
9. **同步文档** — `CODE_WIKI.md` 顶部版本号 + 底部元信息

> ⚠️ **Cargo.lock 中的 version**：由 cargo 自动更新，下次 `cargo build` 时自动重写。

> ⚠️ **升级检查清单**：建议在发布前对照以下 5 个**必须保持一致**的位置：
> 1. `tauri-app/src-tauri/tauri.conf.json` → `"version": "2.2.8"`
> 2. `tauri-app/src-tauri/Cargo.toml` → `version = "2.2.8"`
> 3. `tauri-app/frontend/src/shared/ui-constants.ts` → `APP_VERSION = '2.2.8'`
> 4. `tauri-app/package.json` + `tauri-app/frontend/package.json` → `"version": "2.2.8"`
> 5. 根 `version.json` → `"version": "v2.2.8"`（带 v 是发布 tag 格式）

### 后端代码引用方式

```rust
// main.rs:125 启动日志
crate::log_info!("app", "应用启动, 版本: v{}", env!("APP_VERSION"));

// updater.rs:314 更新检查
let current = env!("APP_VERSION");
let has_update = compare_versions(current, &latest_tag);

// commands/system.rs:210 系统信息接口
let version = env!("APP_VERSION").to_string();
```

### 前端版本号来源（双轨）

| 来源 | 用途 | 修改方式 |
|---|---|---|
| `tauri.conf.json` → `__APP_VERSION__` 编译时注入 | vite.config.ts 注入到前端构建产物 | 自动随 tauri.conf.json 同步 |
| `ui-constants.ts` `APP_VERSION` 常量 | 代码内直接 import | 手动同步 |

前端 `vite.config.ts` 已从 `tauriConf.version` 动态读取并通过 `__APP_VERSION__` 注入：

```typescript
// vite.config.ts
const appVersion = tauriConf.version || '0.0.0'
define: {
  __APP_VERSION__: JSON.stringify(appVersion),
}
```

> 注：`ui-constants.ts` 的 `APP_VERSION` 保留硬编码是为了在非 Tauri 环境（如纯前端 Storybook / 单元测试 mock）下也能取到合理默认值。**升级时务必同步两处**。

### 版本号格式约定

- **semver 格式（不带 v）**：`Cargo.toml` / `tauri.conf.json` / `package.json` × 2 / `ui-constants.ts` / `about-preview.html` 静态部分 → `2.2.8`
- **发布 tag 格式（带 v）**：`version.json` / 后端日志（`v{}`）/ README 徽章（`version-2.2.8` 不带 v，但后端启动日志带 v）

---

*文档版本: v2.2.8 | 基于代码版本: CampusLogin v2.2.8 | 更新日期: 2026-06-26*
