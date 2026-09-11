# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.3.4 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
> **目标平台**: Windows (x64) + Android
> **通信方式**: Tauri IPC (`invoke` / `listen`)

---

## 一、项目概览

CampusLogin 是一款校园网自动登录助手，面向无锡学院校园网认证系统（锐捷 ePortal），提供一键登录/注销、自动重连、校园网智能检测、DNS 智能解析与优化、网络质量监测、多账号管理等功能。**双端同构**：Windows 桌面端（`tauri-app/`）与安卓端（`android/`，2026-09-10 并入本仓库）共用同一协议核心（安卓以 Cargo path 依赖桌面 crate），命令面同名对齐，UI 各自适配（桌面 Dock 布局 / 移动底部导航）。

### 核心特性

| 特性 | 说明 |
|------|------|
| 一键登录 | 自动检测适配器、DHCP续租、可重试失败智能重试(retryable 判定) |
| 一键注销 | Radius注销先行(成功即止) + MAC解绑收尾，支持指定适配器注销或全部注销 |
| 自动重连 | 后台巡检断线检测，最多3次自动重连 |
| 校园网检测 | 三级检测：网络名称匹配 → /18子网匹配 → 网关Ping可达 |
| DNS 智能解析 | 动态评分选择最优 DNS 服务器，应用级 DoH 解析，三级智能解析策略 |
| DNS 优化 | 检测 DNS/DoH 配置，一键设置推荐 DNS + 启用 DoH 加密 |
| 网络质量检测 | 网关/DNS/DoH/HTTPS/游戏服务器延迟并发测试，DNS 解析专项测试，增量推送逐步填充 |
| 多账号管理 | DPAPI 加密存储、快速切换 |
| 运营商账号绑定 | 对接自助服务系统（Dr.COM Self），绑定/查询运营商账号，手机号掩码显示 |
| Windows Hello 验证 | 查看密码明文/绑定/踢下线等敏感操作本地生物识别或 PIN 验证，前端门 TTL 570s + 后端 600s TTL 复核 |
| 自助服务查询 | 独立面板：在线设备信息（可踢下线）+ 近期上网记录（日期筛选+汇总统计） |
| 总览自定义卡片 | 首页卡片可增删拖拽排序，内置"在线信息"与"近期上网记录"卡（关键信息验证后查看） |
| 双适配器支持 | 有线 + 无线同时管理，Dock 栏适配器选择菜单 |
| 系统托盘 | 最小化到托盘后台运行，支持托盘快速登录 |
| 开机自启 | 注册表写入 / Tauri 插件 |
| 自动退出 | 登录成功后倒计时退出，快捷键取消(Ctrl+Shift+C) |
| 主题系统 | 7种预设主题 + 自定义主题色 + 深浅模式 |
| 用户自助服务 | 一键打开校园网自助服务系统 |
| 中英语言切换 | 标题栏一键切换中英文，react-i18next + i18next-browser-languagedetector，默认中文 |
| 日志自动清理 | 可选保存时间（3/7/14/30天+永久），AtomicU32全局存储，后端定时清理 |
| 测速面板 | 第三方测速站点快捷导航卡片(speedtest.cn/speedtest.net/ustc/neu) |
| 安卓端 | AndroidKeyStore AES-GCM 密文存储（替代 DPAPI）+ 生物识别验证门（BiometricPrompt 兜底锁屏凭据）+ 前台服务保活监控（常驻通知/WifiLock/WakeLock）+ 开机自启，包名 `com.campuslogin.client` |

---

## 二、项目目录结构

```
Wxxy-CampusLogin/
├── assets/                          # 截图等资源
├── tauri-app/
│   ├── package.json                 # 根层依赖
│   ├── build.ps1                    # 构建脚本 (构建后自动生成 <安装包>.sha256)
│   ├── frontend/                    # React 前端
│   │   ├── package.json             # 前端依赖 (含 zustand ^5.0, framer-motion ^12, vitest ^4.1)
│   │   ├── about-preview.html       # 关于对话框静态预览 (版本号带 v 前缀)
│   │   ├── tsconfig.node.json       # vite.config 的 TS 配置 (composite, 禁止 tsc -b)
│   │   ├── ANIMATION_GUIDE.md       # 动画设计指南
│   │   ├── vite.config.ts           # Vite 构建配置 (manualChunks 分组)
│   │   ├── vitest.config.ts         # Vitest 测试配置 (jsdom + globals + @ 别名)
│   │   ├── tailwind.config.js       # Tailwind CSS 配置
│   │   ├── postcss.config.js        # PostCSS 配置
│   │   ├── tsconfig.json            # TypeScript 配置
│   │   ├── index.html               # HTML 入口
│   │   ├── public/                  # 静态资源 (图标 PNG + sponsor-weixin.png / sponsor-alipay.jpg + girl/ 看板娘场景图)
│   │   └── src/
│   │       ├── main.tsx             # React 入口
│   │       ├── App.tsx              # 根组件
│   │       ├── vite-env.d.ts        # Vite 环境类型声明
│   │       ├── index.css            # 全局样式
│   │       ├── hooks/               # 自定义 Hooks (21个 + 2个测试文件)
│   │       │   ├── tauriApi.ts      # Tauri IPC 封装 (原 useIpc.ts，含 DNS/DoH/注销 API + 重试 + 事件工厂)
│   │       │   ├── useAppStore.ts   # 兼容壳 (3行 re-export，原单体已按领域拆分)
│   │       │   ├── useConfigStore.ts     # 配置领域 store (config/密码防抖保存/accounts/language)
│   │       │   ├── useAuthStore.ts       # 认证领域 store (doLogin/doLogout/checkOnline/status/bgStatus)
│   │       │   ├── useAdapterStore.ts    # 适配器领域 store (adapters/details/disabled/activePanel)
│   │       │   ├── useQualityStore.ts    # 网络质量领域 store (networkQuality/dnsDoh/gpuInfo/refreshRate)
│   │       │   ├── useThemeStore.ts      # 主题领域 store (themeName/isLightMode/customThemeColor + DOM 副作用)
│   │       │   ├── useLogToastStore.ts   # 日志/Toast store (独立 zustand)
│   │       │   ├── useAppInit.ts         # 初始化编排 hook (调用 4 个子 hook)
│   │       │   ├── useEventListeners.ts  # Tauri 事件监听统一注册 (14 个事件 + 窗口关闭拦截)
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
│   │       │   ├── renderLiveness.ts # rAF 渲染活性检测（lastRafTime 停滞>10s 判渲染冻结；心跳发送前门控，GPU 崩溃 JS 存活场景交由后端重载）
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
│   │       │   ├── SelfServicePanel.tsx # 自助服务面板（在线信息+近期上网记录两卡片，2026-09-05）
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
│   │       │   ├── adapters.ts      # 前端适配器解析 (resolveAdapterNames, 与后端同源规则) + AUTO_DETECT_ADAPTER 哨兵常量（2026-09-05 起全前端"自动检测"字面量统一引用它，存储语义不变）
│   │       │   ├── adapters.test.ts # resolveAdapterNames 单测 (锁同源行为)
│   │       │   ├── constants.ts     # 网络常量 (QUALITY_CONFIG 9级在此定义)
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
│   │       │   ├── SponsorCard.tsx  # 赞助浮层 (非模态, 2026-09-04)
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
│   │           └── ui/              # 基础 UI 组件 (12个文件, shadcn/ui 风格, 含 button.test.tsx)
│   └── src-tauri/                   # Rust 后端
│       ├── Cargo.toml               # Rust 依赖 (含 webview2-com-sys 0.38, windows-core 0.61, getrandom 0.3)
│       ├── Cargo.lock               # 依赖锁定文件
│       ├── gen/                     # Tauri 生成目录 (schemas 等, 构建产物)
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
│           │   ├── model.rs         # 配置模型(40字段) + PASSWORD_MASK + deserialize_non_empty_or
│           │   ├── persist.rs       # 配置持久化 (atomic_write 重试 + list_account_names + append_login_history + save_config_to_disk_encrypted)
│           │   └── validate.rs      # 配置校验 (枚举值/正则/URL/Portal URL 迁移/校园网关校验)
│           ├── network/             # 网络模块
│           │   ├── mod.rs           # 重导出
│           │   ├── client.rs        # 缓存基础设施 (PORTAL_URL/CLIENT_POOL/HTTP客户端/TLS 1.3+回退)
│           │   ├── adapter.rs       # 适配器选择 (薄 re-export 模块: discovery/adapter_cache/dhcp/subnet)
│           │   ├── adapter_cache.rs # 适配器查询缓存 (force/cached 双模式)
│           │   ├── dhcp.rs          # DHCP 操作 (release/renew, MAC 重置, apply_mac_change_via_registry)
│           │   ├── subnet.rs        # 子网判定 (/18 校园网子网匹配)
│           │   ├── dns.rs           # DNS 缓存管理 + DoH解析 + 智能解析策略
│           │   ├── timing.rs        # HTTP计时 (measure_https_timing/measure_dns_query/measure_doh_timing, 评分与智能解析在 dns.rs)
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
│           │   ├── mod.rs           # 仅声明 crypto 子模块 (多账号命令逻辑在 commands/account.rs)
│           │   └── crypto.rs        # 加密工具 (Windows DPAPI)
│           ├── infra/               # 基础设施模块
│           │   ├── mod.rs           # 重导出
│           │   ├── async_util.rs    # block_on_sync 同步上下文驱动 future (防 async 线程 block_on panic)
│           │   ├── state/           # 全局状态子模块 (重构自 state.rs)
│           │   │   ├── mod.rs       # TaskLock/TaskGuard/TaskFlags/AppState/CommandResult/AccountResult
│           │   │   ├── store.rs     # ConfigStore (封装 ArcSwap<Config>)
│           │   │   ├── network.rs   # NetworkState + NetworkSnapshot (CAS 快照更新)
│           │   │   └── exit.rs      # ExitStateStore
│           │   ├── logger.rs        # 日志系统 (文件+通道+调试模式切换+日志保留天数清理+shutdown mpsc超时join)
│           │   ├── lifecycle.rs     # 自动退出控制 + 校园网退出流程
│           │   ├── notification.rs  # 系统通知封装 (emit_notification，仅非前台 Windows 通知)
│           │   ├── events.rs        # 事件总线 EventBus (15 个 emit_xxx 方法)
│           │   ├── command_context.rs # 命令上下文 CommandContext::from_app
│           │   └── task_manager.rs  # 后台任务管理器 BackgroundTaskManager (cancel token 统一管理)
│           ├── monitor/             # 监控模块 (10个子模块，portal_failure 已迁入 auth/failure_tracker)
│           │   ├── mod.rs           # 模块声明 (10个 pub mod, 无 re-export)
│           │   ├── watcher.rs       # 门面 + 启动聚合 (54行，re-export background_check/background_task + run_startup_tasks)
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
│           │   ├── console_output.rs # 控制台输出解码 (UTF-8 优先 → OEM 代码页 MultiByteToWideChar)
│           │   ├── dns_config.rs    # DNS/DoH 配置文件设置 (per-profile/适配器级/DoH API)
│           │   ├── elevation.rs     # UAC 提权 (ShellExecuteW + COM ShellExec) + GUID 解析 + is_admin
│           │   ├── gpu.rs           # GPU 信息检测 (DXGI) + 刷新率检测 + 浏览器参数 + gpu_preference
│           │   ├── identity.rs      # Windows 本地身份验证 (Hello UserConsentVerifier + CredUI/SSPI NTLM 回退, 2026-09-05)
│           │   ├── autostart.rs     # 开机自启 (注册表读写; Tauri autostart 插件在 startup.rs 注册)
│           │   └── helper_spawn.rs  # --helper 提权子进程启动 + 结果文件轮询 (spawn_elevated_helper)
│           ├── update/              # 更新模块
│           │   ├── mod.rs           # 模块声明 (仅 pub mod updater, 无重导出)
│           │   └── updater.rs       # 更新检查/下载/安装 (SHA256校验)
│           ├── helper/              # 提权辅助子进程 (--helper 模式, 主进程入口最先拦截)
│           │   └── mod.rs           # HelperOp/parse_helper_args/run_helper + 结果文件回写
│           ├── app/                 # 应用生命周期模块
│           │   ├── mod.rs           # 重导出
│           │   ├── startup.rs       # 应用启动 (run + build_runtime + setup_app: 状态/托盘/启动任务聚合; panic hook 在 main.rs)
│           │   ├── tray.rs          # 系统托盘 (菜单/事件处理)
│           │   ├── window.rs        # 窗口焦点内存调节 (handle_window_focus_event) + show_and_focus_main
│           │   ├── shortcut.rs      # 全局快捷键 (Ctrl+Shift+C 取消自动退出)
│           │   ├── heartbeat.rs     # 渲染进程心跳检测 + spawn_window_safety_thread (3秒保底显示窗口)
│           │   ├── webview_recovery.rs # WebView2 ProcessFailed 订阅 + 恢复限流入口 + 运行时版本记录 (2026-09-11 白屏修复)
│           │   └── shutdown.rs      # graceful_exit + handle_window_close_event (关闭进托盘/退出分流)
│           ├── self_service/        # 自助服务系统 (Dr.COM Self) 协议：登录 + 运营商绑定 + dashboard 在线信息/上网记录/注销会话 (2026-09-05)
│           │   └── mod.rs           # bind_operator 协议链路 + checkcode/csrftoken/swal msg 提取 + 6 个单测
│           └── commands/            # Tauri 命令 (模块化拆分)
│               ├── mod.rs           # 命令模块声明与架构文档
│               ├── config_cmd.rs    # 配置相关命令 (空密码兜底)
│               ├── login.rs         # 登录/注销命令
│               ├── background.rs    # 后台检测命令入口 (委托 monitor::watcher)
│               ├── network_cmd.rs   # 网络命令 + DNS/DoH 检测与设置 (winreg + ShellExecuteW)
│               ├── system.rs        # 系统功能命令
│               ├── account.rs       # 多账号管理命令 (逻辑自含; account/mod.rs 仅声明 crypto)
│               ├── self_service.rs  # bind_operator/query_bind_status/query_self_dashboard/self_offline_session 等命令 (校验 + 校园网源 IP 解析, 委托 self_service 模块)
│               └── updater.rs       # 更新命令 (委托 update 模块)
├── android/                         # 安卓端 (2026-09-10 由独立仓库并入, 与桌面共用本仓库)
│   ├── frontend/                    # React 前端 (桌面复刻+移动裁剪; VITE_PLATFORM=android 平台判断)
│   ├── src-tauri/                   # 安卓 Rust 后端 (monitor_loop 后台检测+自动重登 / config_state CryptoBridge / protocol_cmds / cpu_affinity 小核绑定)
│   │   ├── Cargo.toml               # campus-login path 依赖主仓库协议核心 + libc(android target)
│   │   └── gen/android/             # tauri CLI 生成的 gradle 工程 (settings.gradle 插件声明为手改; build 产物不入库)
│   └── plugins/                     # 手写 tauri 插件: keystore / foreground-service(MonitorService+installApk) / network-bind
├── CODE_WIKI.md                     # 本文档
├── AGENTS.md                        # AI 编码助手项目约定
├── README.md                        # 项目说明
├── version.json                     # 版本号配置
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

### 3.2 Commands 模块依赖关系 (v2.3.0)

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
//  耦合说明：
//    1. monitor/watcher 已瘦身为门面(54行)，检测主体迁移至 background_check.rs，
//       外部调用路径不变（通过 pub use re-export）
//    2. background_check 是核心检测主体，依赖 auth/failure_tracker/auto_auth/lifecycle
//       (+ campus_check/portal_check/background_emit 子模块)
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
                              AppState (ConfigStore + TaskFlags + BackgroundTaskManager + NetworkState + ExitStateStore + UpdateStats[含5个原子字段: last_update_check_epoch_ms/update_notified(AtomicBool)/last_disabled_notification_ms/last_render_heartbeat_ms/last_network_change_notification_ms])
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

**职责**: `main.rs` 共 51 行，核心流程：注册 panic hook → 拦截 `--helper` → 构建 Tokio runtime → 注入 Tauri → 调用 `app::startup::run()`。所有应用初始化逻辑（Tauri 插件注册、Setup 钩子、命令注册、窗口/托盘/事件处理）实际位于 `app/startup.rs` 的 `build_runtime()` / `run()` / `setup_app()` 函数中。

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
   - 窗口显隐由前端决定（`useInitialDataLoad` 按 `isAutoStart && hiddenStart` 判断是否调 showWindow），`--autostart` 参数仅影响后端自动登录初始延迟；后端另有 3 秒保底显示线程兜底
   - 创建系统托盘
   - 启动适配器监控和启动任务 (通过 `run_startup_tasks`)
   - **3 秒保底 showWindow**：独立线程 3 秒后检查窗口可见性，不可见则强制 `window.show()` + `set_focus()`，最多重试3次，防止前端初始化异常导致窗口永远隐藏
   - **前端心跳监控**：独立线程每 5 秒检查 `last_render_heartbeat_ms`，连续 3 次超过 20 秒无心跳则重载 WebView；窗口可监控判定需同时满足 `is_visible() && !is_minimized()`（2026-09-05：前端 `useHeartbeat` 在 `document.hidden` 含最小化时暂停心跳，而 Win32 最小化窗口 is_visible 仍为 true，只查可见性会让最小化超阈值后必误触发重载）
   - **白屏恢复缺口修复（2026-09-11 GitHub 调研后实施，`app/webview_recovery.rs`）**：① **订阅 ProcessFailed**——Tauri 2 Windows 侧不暴露该事件（官方 PR #15162 仅 Apple 平台有 `on_web_content_process_terminate` 且带 10s/3次限流；wry `src/webview2/mod.rs` 未订阅），经 `webview2-com 0.38`（新增依赖）`ICoreWebView2_4::add_ProcessFailed` 直订（`ProcessFailedEventHandler::create(Box<closure>)`，闭包拿 `args.ProcessFailedKind`——注意 sys 0.38 方法名**无 Get 前缀**，token 参数为 `*mut i64`）；按 kind 分级：RENDER/FRAME_RENDER/BROWSER_PROCESS_EXITED → 立即走恢复入口（渲染崩溃比心跳路径快 20-35s），GPU/Utility/PPAPI 等 → 仅记日志交运行时自愈（GPU 未自愈由 rAF 冻结→心跳兜底）。② **恢复动作唯一入口 `attempt_webview_recovery`**：5 分钟窗口最多 3 次 reload（`recovery_gate` 纯函数，3 单测），心跳超时路径（原 `heartbeat.rs` 裸 eval 无限流、`let _ =` 忽略结果）与事件路径共用同一限流器；超限停止自动恢复只留 ERROR 日志（reload 对已退出/无响应进程无效，无限重发是原空转缺陷），eval 失败（webview 整体失效）亦留 ERROR——不自动重启应用，重启是用户决策。③ **诊断埋点 `record_webview2_runtime_version`**：启动时读注册表 EdgeUpdate `{F3017226-FAC6-4E36-9A38-E4524AA30166}\pv`（HKLM WOW6432Node 优先 → HKLM → HKCU），Evergreen 自动更新、白屏可能仅特定版本存在（WebView2Feedback#5692 证实 4191.47 白屏 bug），版本号是诊断第一证据。限流状态在 `UpdateStats`（`webview_recovery_window_start_ms`/`webview_recovery_count` 两原子字段）。诊断手段备忘：崩溃 dump 在用户数据目录 `EBWebView/Crashpad/reports/*.dmp`；GPU 类白屏可 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` A/B 对比
3. **WebView2 内存管理**: `on_window_event` Focused 时通过 `ICoreWebView2_19.SetMemoryUsageTargetLevel` 调节（前台 NORMAL，后台 LOW）
4. **WebView2 浏览器参数**: `platform/gpu.rs::build_browser_args()` 仅注入 `--js-flags=--max-old-space-size=512`（2026-09-03 精简：原 ANGLE/SkiaGraphite/DrDc/zero-copy 等 11 个参数经核验已失效/Windows 默认即开/Windows 不支持/实验性强开，一并删除交还平台默认）
5. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭（分流逻辑在 `app/shutdown.rs::handle_window_close_event`）
6. **退出流程**: 设 `is_quitting` → `task_manager.shutdown()` 取消并等待后台任务（整体 10s 超时上限，防任务卡在不响应取消的阻塞调用时退出挂起）→ `exit(0)`（`app/shutdown.rs::graceful_exit` → `infra/lifecycle.rs::shutdown_and_exit`），窗口关闭与托盘退出行为统一
7. **命令注册**: 55个 `#[tauri::command]` 函数 (在 `run()` 中通过 `tauri::generate_handler!` 注册)

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

**说明**：wave 3c (AM-4) 将原散落在 `AppState` 顶层的原子更新/通知标志合并为子结构体 `UpdateStats`（现 5 个字段：`last_update_check_epoch_ms`/`update_notified`/`last_disabled_notification_ms`/`last_render_heartbeat_ms`/`last_network_change_notification_ms`），`AppState` 现仅 6 个字段。`config`/`network`/`exit` 均为对应 Store/State 封装类型；`task_manager` 承接取消令牌职责。`AppState` 不再直接持有 `update_config` 方法，配置 CAS 更新改走 `state.config.update(...)`。

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

**`Config` 结构体** (40个字段):

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `user` | String | `""` | 学号 |
| `password` | String | `""` | 密码 (内存中明文, 磁盘上DPAPI加密) |
| `selfPassword` | String | `""` | 自助服务系统密码 (2026-09-05, 内存中明文, 磁盘上DPAPI加密; 回传前端时替换为 MASK; 命令层 resolve_self_password 在前端传空/MASK 时回退此值; 显式清除走 save_config 的 `clearSelfPassword` 标志——与 clear_password 对称, 绑定卡/自助面板密码框旁"清除密码"按钮, 2026-09-06) |
  ⚠️ 出站掩码纪律（2026-09-06 缺陷修复后收敛）：所有把 Config 发往前端的路径**必须经 `Config::masked_for_display()`（config/model.rs 唯一出口，password + self_password 双字段掩码，空值=未设置语义保留）**，不得手工逐字段打码。历史缺陷链：state 内是解密明文，漏掩码会把明文发给 webview（隐私泄露），且前端 selfPasswordSaved（依赖 === MASK）永远 false → 重启后密码框显示空、切入自助服务面板的自动 Hello 验证永不触发；fe000de 修 get_init_data/get_config 时漏掉 account 三命令（switch/save_as/delete 经 `masked_for_display` 组装 `AccountResult.config`），明文 selfPassword 随 IPC 出站——掩码收敛进 Config 自身后该类遗漏被回归单测（config_cmd.rs，双字段断言）锁死，独立函数 `mask_self_password` 已删除。后续加固方向（评审调研）：敏感字段改 `secrecy::SecretString` 类型锁（rust-lang/crates.io、rage 同款，默认禁 Serialize 编译期防出站）
| `selfHelloEnabled` | bool | true | Windows Hello 验证总开关 (2026-09-06，serde default true 旧配置安全兜底)：关闭后绑定/自助服务面板不再弹验证（只读查询本就不设门），但"查看运营商密码明文"仍强制验证（`ignoreToggle` 门 + 后端 reveal TTL 校验不受开关影响） |
| `selfReverifyEachAction` | bool | false | 每次操作二次验证开关 (2026-09-06，default false)：开启即自助面板会话门退化为每次操作验证的严格模式；`selfHelloEnabled` 关闭时禁用此开关；两开关的**关闭方向**都要求先过 Hello 验证（防绕过界面关闭保护） |
| `operator` | String | `""` | 运营商后缀 (`""` 不拼接, `"@telecom"`/`"@unicom"`/`"@cmcc"` 直接拼接, 其他值 `validate_operator` 报错) |
| `adapter1` | String | `"自动检测"` | 主适配器名称 |
| `adapter2` | String | `""` | 副适配器名称 |
| `dualAdapter` | bool | false | 双适配器模式 |
| `autoLoginOnStart` | bool | true | 启动时自动登录 |
| `autoExitAfterLogin` | bool | true | 登录后自动退出 |
| `minimizeToTray` | bool | false | 关闭时最小化到托盘 |
| `hiddenStart` | bool | false | 静默启动 (2026-09-04 默认 true→false) |
| `autoLaunch` | bool | true | 开机自启 |
| `enableBackgroundCheck` | bool | true | 启用后台检测 |
| `backgroundCheckInterval` | u64 | 15000 | 后台检测间隔 (ms) |
| `autoLoginOnPreparation` | bool | true | 登录准备模式 |
| `autoExitOnOnline` | bool | true | 检测到在线后自动退出 |
| `themeMode` | String | `"dark"` | 主题模式 |
| `enableNotification` | bool | true | 启用通知 |
| `activeAccount` | String | `""` | 当前活跃账号名 |
| `enableLatencyTest` | bool | true | 启用延迟测试（默认开启，2026-09-04） |
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
| `atomic_write()` | 原子写入文件，3次重试+100ms间隔，重命名失败后删除临时文件；rename 前对临时文件 `sync_all` 落盘（2026-09-05），断电不产生半截 config |
| `append_login_history()` | 登录历史追加（读-改-写全程持模块级 `LOGIN_HISTORY_LOCK` 互斥锁，2026-09-05，防自动/手动登录并发覆盖丢历史） |
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
type ClientPoolKey = (Option<IpAddr>, u8, u64);   // (绑定源IP, TLS最低版本标识, 超时ms)

lazy_static! {
    pub(crate) static ref PORTAL_URL: ArcSwap<String> = ArcSwap::from(Arc::new(default_portal_url()));
    static ref CLIENT_POOL: DashMap<ClientPoolKey, (reqwest::Client, Instant)> = DashMap::new();
}
```

> 注：原 `NetworkCache` 结构体与 `NET_CACHE` 单例已拆分 — 适配器缓存迁移到 `network/adapter_cache.rs`，网关/子网缓存迁移到 `network/subnet.rs`，Portal 状态缓存由 `auth/portal.rs` 局部管理。`client.rs` 仅保留 Portal URL 与 HTTP 客户端池两个全局变量。

**HTTP 客户端池** (`CLIENT_POOL: DashMap`, B9-17 LRU 淘汰):

- Key = `(Option<IpAddr>, u8, u64)` 元组（绑定源 IP + TLS 版本标识 + 超时毫秒，v2.4.0 起告别字符串键零堆分配），value 附带 `Instant` 访问时间，池上限 `CLIENT_POOL_MAX_ENTRIES=32`，TTL `CLIENT_POOL_TTL_SECS=600`
- **LRU 淘汰策略** (B9-17)：`client_pool_get` 命中时更新 `Instant::now()`（按访问时间淘汰，非原 FIFO 按创建时间）；容量超限时 `min_by_key(Instant)` 剔除最久未访问条目
- `create_safe_http_client(timeout, local_addr)` — TLS 1.3 优先 + TLS 1.2 降级，`no-cache/no-store` 头

**关键函数**:

| 函数 | 说明 |
|------|------|
| `create_safe_http_client(timeout, local_addr)` | 创建 HTTP 客户端 (TLS 1.3 强制 + TLS 1.2 回退) |
| `update_portal_url(url)` | 更新全局 Portal URL |
| `build_client(timeout, local_addr, min_tls)` | 构建 reqwest::Client (no_proxy + limited(5) 重定向 + 3s connect_timeout) |
| `client_pool_key(local_addr, min_tls, timeout)` | 生成客户端池 Key |

#### 4.5.2 适配器查询 — `adapter.rs` (选择逻辑) + `network/mod.rs` (re-export 收敛点)

`adapter.rs` 原为 pub use 兼容层，现已**扁平化到源模块**（adapter.rs 顶部注释自述）；对外 re-export 收敛到 `network/mod.rs`（mod.rs 注释"从源模块直接 re-export，消除 adapter.rs 中转层"），调用方直连源模块或经 `crate::network::` 顶层路径访问。

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
- `network::adapter_cache` — `get_adapters_force`/`get_adapters_cached`/`get_adapters_cached_async`/`get_disabled_adapters_cached`/`get_adapter_details_cached`/`get_all_adapters_cached`/`enable_adapter`/`wait_for_adapter`/`filter_operation_adapters` 等 + TTL 5秒缓存（`validate_adapter_name`/`poll_adapter_ip_quick` 由调用方直连源模块，未再中转）
- `network::dhcp` — `dhcp_renew_wired_only`/`dhcp_release_renew_all`/`dhcp_release_renew_single`
- `network::subnet` — `get_wireless_ssid`/`get_wired_network_profile`/`check_gateway_reachable`/`check_gateway_reachable_from`/`is_same_subnet_18`

**适配器状态四分类** (`AdapterStatus` 枚举，定义在 `network/discovery/`):
  - `Disabled` — 已禁用（OperStatus Down/NotPresent，管理员禁用或硬件缺失）
  - `Disconnected` — 未连接（OperStatus LowerLayerDown/Dormant，线缆未插或USB网卡未连接）
  - `EnabledNoIp` — 未禁用无IP（OperStatus Up 但无有效 IP，含 169.254 APIPA 清空后）
  - `Connected` — 已连接（OperStatus Up 且有有效 IP）

**连接速度 (LinkSpeed)**: `Adapter`/`AdapterDetail` 新增 `linkSpeed` 字段（u64 bit/s，0 表示未知），直接读 `IP_ADAPTER_ADDRESSES.ReceiveLinkSpeed`（无需额外 API）。**未连接时 Windows 返回 u64::MAX（内部 -1 哨兵），发现层归 0 表示未知**（2026-09-03：原样透传曾被前端换算成 18446744073.7 Gbps）。前端 NetworkPanel 适配器卡片展示格式化后速度，统一 Mbps 单位（低于 1 Mbps 用 Kbps）。

**适配器操作范围约定（2026-09-03）**: 检测/优化/登录/注销等**操作类**流程只作用于 `resolve_adapter_names` 解析出的主/副适配器（"自动检测"由 resolve 落到具体适配器），新增 `filter_operation_adapters(adapters, a1, a2)` 作为范围过滤基准；**UI 展示类**（get_adapters/get_adapter_details/adapter_watch/check_dns_doh_status/check_campus_status 命令）保持遍历全部。落地位置：后台巡检与开机自启的校园网检测传过滤后列表；`select_adapter` 经 resolve 取主适配器 IP（不再回退到配置范围外适配器，无 IP 返回空由调用方兜底）；`setup_dns_doh_admin(targets)` DNS/DoH 一键设置只写名单内适配器（helper 提权路径经 `--helper dns <名单...>` 传参，`spawn_elevated_helper` 对参数统一加引号防适配器名含空格被拆碎）；`dhcp_renew_wired_only(targets)`/`dhcp_release_renew_all(gw, targets)` DHCP 操作、`update_auth_failure_count(..., adapter_name)` 自动 MAC 重置（原对全部适配器重置，现只重置登录适配器）均按名单收窄；登录/注销（full_login/full_logout）本就只操作 a1/a2，未变。

**前端选择器同源收敛（2026-09-04）**: 登录/注销的适配器选择浮层（DockNav `AdapterMenu`）只列主/副适配器（有 IP 者），与"适配器操作范围约定"对齐——直接点击按钮仍走后端 resolve 全量逻辑，浮层只影响菜单选项。新增 `frontend/src/network/adapters.ts` 的 `resolveAdapterNames(adapters, config)`，与后端 `resolve_adapter_names`（adapter.rs:48）**同源规则**：配置名有效→用配置名；空/"自动检测"/不在当前列表→自动检测（有线有 IP > 任意有 IP > 第一个；副适配器自动检测排除主适配器）。**两端规则若分叉，选择器与实际登录目标会对不上**，改任一侧必须同步另一侧（有单测 adapters.test.ts 锁行为）。DashboardPanel"获取新IP"菜单同步改用 resolve：修复 adapter1/adapter2 为"自动检测"时把字面量传给 `dhcp_release_renew_adapter` 导致 `validate_adapter_name` 校验失败的存量缺陷。同卡菜单改为 `createPortal(document.body)` + fixed 按钮坐标定位：卡片容器 `animated-card-interactive` 的 `overflow:hidden + contain:content`（paint）会裁掉 absolute 菜单（实测菜单 bottom 514 被裁到卡片 bottom 381，仅露 17px），存量缺陷顺带修复。

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

每次请求独立生成 1000-9999 随机4位数 v 值，统一应用于登录与注销请求（`random_v` 定义于 `protocol.rs:6`，Portal 页面检测不发 v 参数）。

**响应解码与 JSONP 解析 (2026-09-04 补扫修复)**:

- **编码**: 响应体经 `platform/console_output.rs::decode_charset_bytes` 解码——Content-Type 显式 GBK 族按 936，否则 UTF-8 优先 → OEM 回退。修复老 Dr.COM Portal（GBK 响应）下"认证成功/已经在线/非法"等中文关键词匹配全部失效：result==1 的真实失败会被误报为登录成功（`from_utf8_lossy` 时代 GBK 字节全变 U+FFFD）
- **JSONP**: `jsonp_json_slice()` 从第一个 `(` 后的第一个 `{` 起做字符串/转义感知的花括号平衡扫描到配对 `}`。旧实现 `rfind(')')` 取最后一个右括号，msg 含半角 `)`（如"密码错误(剩余2次)"）时 JSON 被截断 → 解析失败 → 重试 3 次全败误报

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

**协议行为实测（2026-09-11，VMware 桥接 VM 独立身份 10.2.94.60，宿主身份零影响）**：

- **`chkstatus` 接口在本部署不存在**：返回 `{"code":0,"msg":"404 eportal controller Chkstatus not found"}`——"注销前查在线状态"不可行；登录态判定用 80 状态页（在线时内嵌 `uid='<服务端uid>'`/`v4ip='<本机IP>'`/`time='<在线秒数>'` 变量）。
- **`mac/unbind` 不踢在线会话**：unbind 后立即重登返回"IP: x.x.x.x 已经在线"（ret_code=2），Radius 会话仍活——社区 POC（Eportalcutdown"按 IP 断网"）在本部署不成立，unbind 仅作用于 MAC 绑定表；**注销成败与 unbind 无关，顺序倒置是防御性对齐而非修复实效**。
- **注销后立即重登始终成功**：单次注销（两种顺序）、unbind+logout 4 连发后 0 延迟重登均 result=1 认证成功——**"注销后打不开登录页/换 MAC 恢复"的僵尸状态未复现**（单会话与同账号跨网段双会话并存两种条件均正常）；故障再现场景用 80 状态页 uid/v4ip/time 抓现场。
- **重复请求无增益也无污染**：对已销毁会话的重复 logout 返回 result=0 失败、重复 unbind 返回稳定错误，后续登录不受影响——连发纯属浪费时间，"成功即止"正确。
- **"已经在线"（result=0 + ret_code=2）只在会话真实存活时出现**（登录成功后 10s/30s 重登均如此）——session.rs 复核与实测行为兼容：真在线时 check_portal_full 确认放行不误伤。
- **unbind 整数 IP 与点分 IP 行为无差异**（相同 msg 相同后果）；整数换算旁证：10.2.94.60 → 167927356，与 `ip_to_eportal_int`（Ipv4Addr::to_bits）一致。
- **801 端口 JSONP 响应为 UTF-8**（80 网关页才是 GBK）——与 `decode_charset_bytes` "UTF-8 优先 → OEM 回退"策略兼容。exp3 已用 UTF-8 解码实测证实（中文 msg 完整可读）。

**压测与变异补充实测（exp3，2026-09-11，15 轮循环 + 错误参数变异，双探针验证）**：

- **累计 19 次注销（点分/整数/空 IP/错 IP/连发/双顺序全谱系）后立即重登全部成功，0 僵尸**——故障率 95% 置信上界约 15%；结论修正为：**"注销后僵尸"不是协议的固有行为，属间歇性服务端/环境异常**，应用侧防御（成功即止 + "已经在线"复核 + MAC 重置自愈）保持。
- **网关劫持实证**：未认证时公网探针（`connect.rom.miui.com/generate_204`）返回 **302**（被劫持到 Portal），认证后返回 204——`net204` 状态码可作为权威在线判据（区分"有 IP 未认证"与"真在线"）。
- **空 `wlan_user_ip` 的 logout 成功**（result=1）——服务端按请求源 IP 定位会话，NAT 分支安全。
- **错 IP 的 logout/unbind 无污染**：logout 传不存在 IP 返回"Radius注销失败！"（result=0），unbind 传无效 IP 返回**"获取用户在线信息数据为空！"**（项目 parse 将该响应判为成功——语义实为"无会话可注销"，宽松但无害），本机会话与后续登录均不受影响。
- **80 状态页单次探针有 ~9% 瞬时抖动**（33 次在线期采样 3 次取不到 uid/v4ip 变量，同时段公网 204 均通）——单次页面探测不可靠，后台巡检"失败计数阈值 5 才触发动作"的设计必要。

**换 MAC 恢复链路实测（exp4，2026-09-11，用户手动切换 VM 网卡 MAC 后）**：

- 换 MAC（04:83:D8:28:0E:52 → 00-50-56-37-48-E9）后 DHCP 立即获得**全新 IP**（10.2.94.60 → 10.2.79.99）——"换 MAC = 换身份"实证；新身份首次登录即成功（result=1，80 页 uid/v4ip 与公网 204 双探针确认），随后 3 轮注销-重登同样全绿。**"换 MAC 后可恢复登录"的恢复面闭环**；累计四轮实验 23 次注销 0 僵尸。
- 未覆盖残留：僵尸态下的换 MAC 恢复（前提是僵尸复现，四轮实验均未复现）；WLAN 段身份。

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
- **main.rs 与 lib.rs 是两棵独立模块树**：新增顶层模块必须同时在这两个文件声明（本次曾漏 main.rs 导致 bin target E0432）
- **绑定状态查询**：`query_bind_status` 命令复用登录链路（`login_and_fetch_bind_page` 提取为共用函数），解析 FLDEXTRA 预填值返回三运营商绑定状态；手机号掩码**前三后二**（`mask_account`，如 `197******38`），密码仅回是否设置，明文不出协议模块
- **Windows Hello 本地身份验证（2026-09-05 重构：仅 Hello，删除 CredUI 回退）**：`verify_windows_identity` → `platform/identity.rs` 的 `verify_identity(consent_message)`（async）。**只走 Windows Hello**（`UserConsentVerifier`，指纹/面部/Hello PIN），设备未配置时直接返回引导文案；CredUI 凭据对话框 + SSPI NTLM 回退已整体删除（用户明确只要 Hello）。弹窗文案由前端 i18n 传入。通过后 `reveal_operator_credential` 才可返回运营商明文。**关键平台问题（cppwinrt#999）：阻塞等待 RequestVerificationAsync（.get()）时 Consent 弹窗（独立进程 Credential Manager UI Host）无法完成前台转移，留在应用窗口后面且被 topmost 主窗口盖死（任务栏点击也提不上来）**；修复为非阻塞等待——`await_winrt_operation`（SetCompleted 回调 + tokio oneshot，windows 0.58 无内建 Future；delegate 是 FnMut、oneshot Sender 消费 self，用 Option::take() 适配）。**不手动 CoInitializeEx（2026-09-06 评审调研后删除）**：本文件所有 COM 入口都经 `windows::core::factory()`，windows-core 0.58 factory_cache 撞 `CO_E_NOTINITIALIZED` 时自动 `CoIncrementMTAUsage` 重试（imp/factory_cache.rs:88-95），tokio worker 线程迁移后照样自愈；手动 init/uninit 轮转反而破坏该机制（microsoft/windows-rs#1169），官方 sample / Bitwarden desktop_native / OneKeePass 均不手写。**绑定/查询/自助服务过前端验证门**（`account/selfServiceState.ts`，门语义 2026-09-06 演进为"首次即验证 + TTL 570s 会话"，详见 §4.5.4.3 验证门拆分条目），`AccountPanel.helloGate.test.tsx` 锁行为
- （Win11 置顶问题已并入上行非阻塞方案：原 TopmostGuard 置顶主窗口方案把非置顶的 Consent 彻底盖死，已删除）
- **Consent 弹窗前台问题终局方案（2026-09-06，GitHub 调研后实施）**：非阻塞等待解决了"完全弹不出"，但 Consent UI 仍不抢前台——这是 **Windows bug（task.ms/49689617，Chromium 代码注释确认）**：弹窗由 broker 进程创建且不绑定任何窗口。两路径修复：① **主路径（Win11 Build 22000+）**：官方 interop 接口 `IUserConsentVerifierInterop::RequestVerificationForWindowAsync(hwnd, msg)`（`windows::Win32::System::WinRT`，feature `Win32_System_WinRT`）把 Consent 对话框**绑定到主窗口 HWND**，作为其子级 UI 天然置前——Flutter local_auth_windows/Bitwarden/ProtonMail(Tauri2) 同做法；HWND 取 `webview_window.hwnd()` 的原始 isize（跨 windows crate 版本安全），构造时转 `HWND(isize as *mut c_void)`（0.58 句柄是指针类型）；interop 是 COM 包装（!Send），须块作用域内创建 op 后立即释放、不跨 await，否则命令 future 非 Send 编译失败。② **兜底（Win10/interop 不可用，`.ok()?` 回退）**：无窗口绑定 + `spawn_consent_focus_nudger` 后台线程轮询对话框窗口类名 **"Credential Dialog Xaml Host"**（250ms×12 次）`SetForegroundWindow` 提前台——Chromium（crypto/user_verifying_key_win.cc）/gsudo 同款；interop 路径一旦拿到 op，await 结果即最终结论（含用户取消），**不再回退重弹**。命令层传 `owner_hwnd: Option<isize>` 给 `verify_identity(msg, owner_hwnd)`
- **验证与明文返回在后端关联（2026-09-05 安全加固）**：`verify_windows_identity` 成功后 `note_identity_verified()` 写后端时间戳（`LAST_VERIFY_EPOCH_SECS` AtomicU64），`reveal_operator_credential` 校验 `identity_verified_recently()`（TTL `IDENTITY_VERIFY_TTL_SECS`=600 秒，纯函数 `is_within_ttl` 含时钟回拨拒绝，有单测）——前端 helloGate 只是 UX 层，后端 TTL 才是真防线，webview 层绕过前端编排也无法拿明文。**门覆盖面（2026-09-06 评审后扩展）**：`bind_operator`/`self_offline_session`（改变外部状态）经 `ensure_identity_gate` 同源校验（`selfHelloEnabled` 开启时要求 TTL 内验证；关闭时放行——用户主动弃用则两门整体失效）；**只读查询命令（query_self_dashboard/query_self_online_log/query_bind_status）有意不设门**：总览卡自动刷新依赖免验证拉取（数据本就存于本机、掩码属渲染层），参考 Bitwarden reprompt 分级保护/sudo 仅副作用命令需认证
- 单测 7 个（纯函数）：checkcode/csrftoken/swal msg 提取（实测 HTML 样例）、绑定成功判定、FLDEXTRA 映射、md5 标准测试向量（不使用真实凭据向量）、mask_account 掩码规则
- 前端：新手教程 5 步向导（欢迎→**绑定运营商账号(可跳过)**→账号→适配器→完成），`tauriApi.bindOperator`，i18n `onboarding.bind*` 键组（zh/en）；字段名"运营商账户密码"（键名 `bindSmsPassword` 保留历史命名），校园网登录密码与自助服务密码默认均为身份证后 6 位（placeholder 提醒）；账户管理页登录信息卡与绑定卡并列两列（2026-09-05）

#### 4.5.4.3 自助服务系统（Dr.COM Self）dashboard 卡片协议：在线信息 + 近期上网记录 (2026-09-05)

> 来源：2026-09-05 浏览器（IAB）登录态下读取 dashboard 页内嵌 bootstrapTable JS + 登录会话内 fetch 实测响应结构。实现于 `self_service/mod.rs`（`query_dashboard`/`offline_session`/2026-09-06 增 `query_online_log`）+ `commands/self_service.rs`（`query_self_dashboard`/`self_offline_session`/`query_self_online_log`，55 → 56 个命令）。前端为独立"自助服务"面板（`SelfServicePanel.tsx`，2026-09-05 从账户管理页单开，面板自带凭据输入区）。

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
- 自助服务面板根容器 `space-y-4`（2026-09-06 从 Fragment 平铺改为对齐全局 16px 卡间距标准）
- **启动时序（2026-09-06）**：`configLoaded` 布尔（getInitData 成功/降级完成置位）是自助服务面板自动验证+自动刷新的前置条件——启动后立刻切入面板时配置还在加载，等信号再判断凭据/弹 Hello，加载窗口期显示"配置加载中..."；后端 setup_app 用 gpu-warmup 后台线程预热 GPU/刷新率检测（OnceLock 缓存），get_init_data 读缓存即返回不阻塞前端拿配置
- 前端：**独立"自助服务"面板**（2026-09-05 从账户管理页单开；`PanelName`/`PANEL_TITLES`/NAV_ITEMS/设置页默认面板选项四处接入，DockNav Globe 图标）——`SelfServicePanel.tsx` 含在线信息+近期上网记录两卡片，**凭据与账号面板绑定卡共用 `selfServiceState.ts` 的 `useSelfCredStore`**（切换面板不丢失、样式同绑定卡垂直排布；学号仅内存保留，密码 blur 即经 `saveConfigDirect` DPAPI 落盘）。**密码"已保存"显示两面板（绑定卡+自助服务面板）统一读 store 独立布尔 `selfPasswordSaved` 而非 `config.selfPassword === MASK`（2026-09-06 缺陷修复，自助服务面板第二日补修——首修只改了 AccountPanel）**：blur 走 updateConfig 会把明文写进本地 config 且标 dirty，挡住 config-changed 回传的 MASK（清 dirty 与事件到达竞态），输入框闪空甚至永久空白；布尔由 saveConfigDirect 非空成功置位 + 初始加载按 MASK 置位，`useConfigStore.selfPassword.test.ts` 锁行为；查询/踢下线前过 `useHelloGate` 验证门；注销走 ConfirmDialog（明示断网风险）→ `self_offline_session` → 成功后前端本地移除该行（不自动重拉，避免整会话重登开销）；i18n `nav.selfservice`/`panel.selfservice*`/`account.selfDashboard*/col*/payStyle*/selfOffline*` 键组（zh/en 对称）
- **逆向安全红线**：dashboard 页有注销功能，实验只允许用**必然不存在的 sessionid** 探测接口格式，绝不能点击/调用页面上真实会话的注销——误踢当前在线设备会导致用户断网
- **上网记录账单卡（2026-09-06，协议第 9 条）**：`SelfServicePanel` 第三张卡——日期范围（默认今天）+ 汇总 8 格 + 12 列明细表（横滚）；`query_online_log`（`login_session` + GET `bill/getUserOnlineLog?startTime&endTime&pageNumber&pageSize=500`，一次拉全不翻页，超 500 提示缩小范围）；命令 `query_self_online_log`（YYYY-MM-DD 校验 + resolve_self_password 回退）；前端 epoch ms 格式化（logoutTime 0/空 → `-`）、数值 toFixed(2)（记录数 COU 取整）；列名复用 dashboard 卡现有键（注销时间/使用时长/使用流量/计费金额/IP地址），新增 `account.selfLog*` 26 键（zh/en）；汇总键全大写（INTERNETUPFLOW/…/TIME/COSTMONEY/COU）
- **Hello 验证门拆分（2026-09-06 用户收紧）**：`selfServiceState.ts` 两套门——绑定卡 `useHelloGate`（时间戳 `helloGateVerifiedAt`：**首次操作即验证**，通过后 TTL 内共用；2026-09-06 删除首免——原首免会让自助服务验证后绑定首次也免验，观感像共用，两门已彻底独立；同日评审后由"应用生命周期永真"改为 **TTL 570s 对齐后端 600s、留 30s 时钟余量（sudo timestamp 式会话）**——永真缓存会在 10 分钟后撞上后端 reveal TTL 过期且无重验入口，重启前功能死锁；门新鲜判定 `gateFresh` 含 `elapsed >= 0`（时钟回拨视为过期，与后端 is_within_ttl 同向，回拨后重验即重新对齐，2026-09-06））；**查看明文与绑定/查询共用此门**（2026-09-06 用户要求查询验证后查看免二次验证），`useHelloGate({ ignoreToggle: true })`：门 TTL 内放行、未验证/已过期时无视总开关强制验证（明文特权操作不因总开关关闭而裸奔，后端 reveal TTL 校验呼应），`markGateVerified` 导出已移除；**自助服务面板 `useSelfServiceVerify` 会话门：切入面板验证一次后 TTL 内操作共用**（时间戳 `selfSessionVerifiedAt` 同款 TTL，面板卸载 `resetSelfSessionGate()` 重置；`selfReverifyEachAction=true` 时退化为每次操作验证）；`config.selfHelloEnabled=false` 时两门整体放行，**reveal 明文密码不走门不受开关限制**（后端 TTL 防线恒在）。设置页"安全设置"卡两开关（`selfHelloEnabled`/`selfReverifyEachAction`），**关闭方向必须先过 Hello 验证**（`handleSecurityDisable`），开启方向免验；SelfServicePanel mount 后凭据就绪即自动验证+自动刷新（`autoRefreshedRef` 防重复；**自动刷新直接走 `fetchDashboard` 内含验证门**——2026-09-06 修 `selfReverifyEachAction=true` 时外层验证+内层验证连弹两次 Hello）；新手教程绑定步骤接入 `useHelloGate`（后端 bind 命令补门配套）；SelfServicePanel.test 锁"每次进面板验证/操作共用/开关关闭放行"
- 单测：`offline_success_parsing`（success 判定 4 边界）；前端 vitest 5 用例（凭据联动禁用/表格格式化断言+切入验证/验证失败不查询/开关关闭放行/操作共用）

#### 4.5.5 网络质量检测 — `quality.rs`

**details/metrics 键契约（2026-09-05 标识符化）**: `details`/`metrics` 字典以检测任务 `name` 为键，原为中文字面量（前端硬编码同一中文取值，英文界面显示中文条目名）。现统一为英文标识符：`gateway`/`aliDns`/`tencentDns`/`xinfengDns`/`aliDoh`/`tencentDoh`/`dnsResolve`（SystemDns 聚合条目）+ 12 个 HTTPS 站点 `baidu`/`jd`/`bing`/`railway12306`/`bilibili`/`bilibiliLive`/`douyin`/`douyinLive`/`lol`/`genshin`/`pubg`/`naraka`。前端消费点（`lib/latency.ts` 的 gateway 过滤、`NetworkQualityCapsule` 的 dnsResolve、`QualityPanel` 分组 names）同步改标识符，**条目显示名**走 i18n `quality.names.*`（zh 保留原中文/en 英文名）。前后端键必须同步修改（同仓库同发版，无兼容窗口）。

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
| HTTPS 恢复绑定适配器 (2026-09-03) | v2.2.5 的"HTTPS 不绑定适配器"在双网卡场景失效：系统默认路由选中未认证网卡（如 WLAN）时全部 HTTPS TLS 握手超时（实测 14/19 项失败）。现 HTTPS 测试绑定经 Portal 认证的适配器 IP（`ctx.bind_addr`），与网关/DNS/DoH 测试一致；绑定直连失败且 bind_addr 非空时回退系统路由重试一次（覆盖 Clash TUN 等劫持场景） |
| DNS 解析优先 IPv4 | `resolve_host_uncached_with_bind` 中优先返回 `is_ipv4()` 的结果，避免 IPv6 地址导致连接失败 |
| 增量推送 | `app_handle: Option<&AppHandle>` 参数，Phase 1 和 HTTPS 批次完成后立即 emit，前端逐步填充数据 |
| HTTPS 分批并发 | Phase 2 改为每批 4 个分批并发，减少校园网高 RTT 环境下 TLS 带宽竞争 |
| 前端不再主动触发 | 移除前端 `qualityPromise`，由后端 latency loop 统一管理质量检测时机 |
| 前端防抖移除 | 移除 500ms 防抖，增量推送事件可立即更新 UI |
| 启动延迟 1 秒 | 每轮检测前 sleep 1s（latency loop 首轮等效启动延迟 1s），避免网络未稳定时 HTTPS 延迟异常 |
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

> **双栈拆分契约 (2026-09-04)**: `SetInterfaceDnsSettings` 一次调用只作用于一个栈——默认仅 IPv4，带 `DNS_SETTING_IPV6` (0x0001) 时仅 IPv6，NameServer 地址族必须与目标栈一致（官方 netioapi.h 文档 + Mullvad talpid-dns 双重佐证）。`split_families()` 把 NameServer 列表按 `:` 分组，v4/v6 各调一次 `set_dns_stack()`；DoH 属性字段与 flag 一一对应：Interface → `ServerProperties` (DNS_SETTING_DOH 0x1000)，Profile → `ProfileServerProperties` (DNS_SETTING_DOH_PROFILE 0x2000)（历史缺陷：Profile 的 DoH 属性挂在 ServerProperties 上，per-profile DoH 从未真正写入，表面生效全靠 netsh 全局注册 autoupgrade 兜底）。`ServerIndex` 按本栈列表实际下标（`doh_bindings`）。

> DNS+DoH 一键设置（`network/dns_setup.rs::setup_dns_doh_admin(targets, family)`）按 `family`（"ipv4"/"ipv6"/"both"，默认 both）决定 NameServer 列表：ipv4 只写 2 条 v4、ipv6 只写 2 条 v6、both 写 v4+v6 共 4 条（内部经 `split_families` 分栈各写一次，不做混合串），`doh_bindings` 按服务器 IP 精确匹配模板（含 IPv6，`ServerIndex` 取实际下标），netsh 全局 DoH 注册循环同样覆盖 v6 服务器。WiFi 分支先 `clear_adapter_dns_via_api` 再写 profile；**清除失败时不再吞掉**，改走接口级设置（接口级残留会覆盖 profile DNS）。成功消息按 family 反映实际写入的服务器；仅 IPv6 档附加"请确保网络支持 IPv6 出口"提示。前端 NetworkPanel DNS 卡片用 `SegmentTabs` 三档选择（IPv4/IPv6/IPv4+IPv6，默认双栈），选择经 `setup_dns_doh` 命令的 `family` 参数传入，helper 提权路径经 `--helper dns <名单...> --family <v>` 传递。DNS 检测的 `should_filter_ip` 对非点分格式返回 false（不过滤），IPv6 地址可正常读取与显示；`netsh dns show encryption` 输出解析按 Ipv4Addr/Ipv6Addr 可解析性识别服务器行（历史缺陷：仅匹配点分十进制，v6 条目漏检且模板行串染到上一个 v4 条目）；前端 `ALI_DNS`/`TENCENT_DNS` 推荐集合已含 v6 地址。

**DNS 检测增强**: `read_adapter_dns_from_registry()` 同时读取 `NameServer`（适配器级）和 `ProfileNameServer`（配置文件级），source 优先级为 manual > profile > dhcp，输出 `dnsSource`/`profileDnsServers`/`adapterDnsOverridesProfile` 字段

### 4.6 DNS 智能解析 — `network/dns.rs + network/timing.rs`

> **位置说明**: DNS 评分系统（`DNS_SERVER_SCORES`/`DOH_SERVER_SCORES`/`ServerScore`）、评分更新/查询函数、DoH 解析函数（`resolve_via_doh`/`resolve_host_smart`/`resolve_host_uncached_with_bind` 及辅助函数）均位于 `network/dns.rs`。HTTP 计时函数（`measure_https_timing`/`measure_dns_query`/`measure_doh_timing`）位于 `network/timing.rs`。

#### DNS 服务器动态评分系统

```rust
static ref DNS_SERVER_SCORES: DashMap<String, ServerScore>;   // network/dns.rs
static ref DOH_SERVER_SCORES: DashMap<String, ServerScore>;   // network/dns.rs

// DNS 与 DoH 评分共用同一结构体（原 DnsServerScore/DohServerScore 已合并）
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
| `post_login_handler()` | `auth/service.rs` | 登录后处理 (AM-13 从 commands/login.rs 下沉)：解除注销保护期 → 仅当 `enable_background_check` 开启时延迟500ms触发 `monitor::watcher::run_background_check` → 按需启动 `auto_exit` |
| `check_any_adapter_online()` | `commands/login.rs` | **B9-10 并行化**：双适配器 Portal 在线检测从串行改为 `std::thread::scope` 并行，双适配器检测延迟减半；`do_logout` 复用其逐适配器检测结果避免重复 HTTP 请求。**2026-09-05 崩溃修复**：scope 裸子线程无 Tokio runtime context，reqwest `send()` 构造超时计时器时 `Handle::current()` panic（panic=abort 整进程崩溃），子线程闭包已补 `tauri::async_runtime::handle().inner().enter()` 进入 context |

**注销命令** (`commands/login.rs` 委托 `auth/service.rs`):

| 函数 | 所在模块 | 说明 |
|------|----------|------|
| `do_logout(adapter_name?)` | `commands/login.rs` | Tauri 命令，支持可选指定适配器 |
| `full_logout()` | `auth/service.rs` | 注销核心逻辑 (双适配器串行) |
| `logout_adapter_with_log()` | `auth/service.rs` | 单适配器注销+日志 |

**锁语义**: 登录使用 `is_logging_in`，注销使用独立的 `is_logging_out`

**适配器解析** (直接函数调用，无 trait 抽象):

> 注：`auth/traits.rs` 整个文件已在重构中删除（不仅删除 `DefaultAdapterResolver`，连 `AdapterResolver` trait 与 `MockAdapterResolver` 一并移除）。`auth/service.rs` 现直接调用 `crate::network::resolve_adapter_names` 与 `crate::network::find_dual_adapters` 等自由函数，不再经过 trait 抽象。原 `PortalChecker` / `ProtocolClient` trait 及其 impl/mock 早在 v2.2.8 已删除（dead code）。

**双适配器并行执行** (`auth/dual_adapter_executor.rs`，168 行含测试, B9-7 泛型化):

| 项 | 说明 |
|------|------|
| `DualAdapterResult` | 双适配器执行结果结构体 (`primary`/`secondary` 两个 `Option<CommandResult>`) |
| `execute_dual<F1, F2>(a1_action: F1, a2_action: F2, is_quitting)` | 双适配器并行执行器：**B9-7 将 `Box<dyn FnOnce>` 改为泛型 `F1`/`F2` 静态分发**（`F: FnOnce() -> Option<CommandResult> + Send + 'static`），消除堆分配与虚函数调用；适配器1立即 `spawn_blocking`，适配器2通过 10×100ms 轮询 `is_quitting` 实现可中断 1s 错峰；2 处调用点（service.rs 的 `full_login` + `full_logout`）去 `Box::new` 直接传闭包 |

**认证失败计数与 Portal 请求失败容错** (`auth/failure_tracker.rs`, 9c B9-5 合并原 `monitor/portal_failure.rs`):

| 函数 | 说明 |
|------|------|
| `is_auth_failure()` | 判断 CommandResult 是否为认证失败 (`AUTH_FAILURE_CODES: ["ac_auth_failed","1","4"]`) |
| `update_auth_failure_count()` | 单适配器认证失败计数，连续5次触发该登录适配器（调用方 resolve 后传入 `adapter_name`）的 MAC 重置+DHCP 续租（2026-09-03：由重置全部适配器收窄为单个，走 `dhcp_release_renew_single`） |
| `update_dual_adapter_auth_failure()` | 双适配器分别计数，各自5次触发单适配器 MAC 重置 |
| `handle_portal_request_failure()` | **9c 从 portal_failure.rs 迁入**：Portal HTTP 请求失败容错，`PORTAL_REQUEST_FAILURE_THRESHOLD=5`；网关不可达时跳过计数并重置（校园网断网/维护期避免误重置 MAC），达阈值触发 `dhcp_release_renew_single` |
| `reset_all()` | 重置所有认证失败计数器 |

> `AdapterFailureCounter` 枚举 (A1/A2) 统一认证失败与 Portal 请求失败的计数访问器（`get_adapter_failure_count`/`set_adapter_failure_count` 两个访问器；自增经 `NetworkState::update_with_result` 闭包内联完成，无独立 increment 函数）。

**注销成功后状态重置** (v2.2.5 区分全量/单适配器):

- **全量注销**（未指定 `adapter_name`）：重置 `any_adapter_online`/`last_a1_online`/`last_a2_online`/`has_logged_online` 为 false，`disconnect_reconnect_count` 归零，重置 `last_auto_login_attempt` 为当前时间，取消自动退出倒计时，设置 60 秒注销保护期 (`logout_protected_until`)
- **单适配器注销**（指定 `adapter_name`）：仅重置对应适配器的 `last_a1_online` 或 `last_a2_online`，重新计算 `any_adapter_online = a1 || a2`，其余标志保持不变

### 4.8 后台巡检 — `monitor/` (watcher 门面 + background_check 主体 + background_task 调度)

> **重构说明**：原 `watcher.rs` 大文件已按职责拆分。`watcher.rs` 现仅 54 行，作为门面 re-export `background_check`/`background_task` 等，并提供 `run_startup_tasks` 启动聚合入口。检测主体迁移至 `background_check.rs`，任务调度迁移至 `background_task.rs`。Portal 失败容错原位于 `portal_failure.rs`，9c 阶段 (B9-5) 已合并入 `auth/failure_tracker.rs`（统一失败计数入口）。外部调用路径（`monitor::watcher::run_background_check` 等）通过 re-export 保持不变。

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
| `handle_portal_request_failure()` | `auth/failure_tracker.rs` | Portal 请求失败容错（9c 从 portal_failure.rs 迁入，见 4.7） |
| `build_adapter_details()` / `handle_status_change()` / `emit_background_check_result()` / `update_network_state()` / `adapter_status_entry()` 等 | `background_emit.rs` | 适配器详情/状态变更/检测结果事件/网络状态更新/状态条目构建 |
| `check_campus_network()` | `campus_check.rs` | WiFi/有线分别检测校园网状态 |
| `run_quality_check()` | `quality_scheduler.rs` | 质量检测调度；签名含 `cancel: Option<&CancellationToken>`（2026-09-05）：定时测试循环传入循环 token，复核窗口可即时中断；手动检测路径传 None 行为不变 |

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

> 注：`trigger_background_check` 是 `commands/background.rs` 的独立 Tauri 命令（一次性手动触发，spawn 单次 `run_background_check`，不注册跟踪任务、不落盘），并非 `start_background_check_inner` 的别名；`monitor/mod.rs` 仅做 10 个 `pub mod` 声明，无 re-export。

#### 4.8.4 Portal 请求失败容错 — 已迁入 `auth/failure_tracker.rs` (9c B9-5)

> **迁移说明**：原 `monitor/portal_failure.rs` (~90 行) 已在 9c 阶段 (B9-5) 整体合并入 `auth/failure_tracker.rs`，统一失败计数入口。`monitor/portal_failure.rs` 文件已删除，`monitor/mod.rs` 子模块从 11 个降为 10 个。逻辑详见 4.7 章节 `handle_portal_request_failure()`。

**Portal 容错完整链路** (v2.2.5 新增, v2.2.7 增强, 9c 迁入 failure_tracker)：

1. 主/副适配器 Portal 请求失败（`is_request_failed: true`）时，对应适配器 `a1_auth_failure_count`/`a2_auth_failure_count` 自增（CAS 更新 NetworkSnapshot）
2. 失败时先检查网关从该适配器IP是否可达（`check_gateway_reachable_from()`），不可达则跳过计数并重置（校园网断网/维护期避免误重置 MAC）
3. 连续 5 次失败（`PORTAL_REQUEST_FAILURE_THRESHOLD=5`）触发 `dhcp_release_renew_single`（仅对该失败适配器 MAC 重置 + DHCP 续租，仅对校园网子网适配器生效）
4. 触发后重置计数器为 0
5. Portal 检测恢复正常（`Success`）时 CAS 写入 0 重置计数器并记录原值日志

> 9c 合并后，`auth/failure_tracker.rs` 统一管理**认证失败**（`AUTH_FAILURE_CODES: ["ac_auth_failed","1","4"]`）与 **Portal HTTP 请求失败**两类计数，共用 `AdapterFailureCounter` 枚举 (A1/A2) 与计数访问器（`get_adapter_failure_count`/`set_adapter_failure_count`，自增经 `update_with_result` 闭包内联）。

**量化改进** (职责分离重构)：

| 指标 | 重构前 (watcher.rs) | 当前 (background_check.rs) |
|------|---------------------|---------------------------|
| `run_background_check_blocking` 行数 | ~190 行（单文件） | ~290 行（含 Portal 容错调用） |
| 重复 JSON 构建代码 | 3 处 | 0 处 |
| watcher.rs 总行数 | ~337 行 | 54 行（门面） |

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
| `cancel_auto_exit_inner()` | 取消自动退出：清 deadline + cancelled 标志 + **同步 `task_manager.cancel("auto_exit")`**（2026-09-05：取消只清标志不清任务会留下同名残留，20s 内重新登录时 spawn 被拒→假倒计时且 deadline 残留短路后续触发） |
| `start_campus_exit()` | 校园网验证不通过时：30s后最小化到托盘，再30s后强制退出 (受 `campus_exit_on_fail` 控制)。**先 CAS 防止重复触发，成功后再设置 deadline**，避免 `campus_exit_started` 永久卡死 (原实现先 set_deadline 再 CAS 会导致周期性调用时 deadline 被推后、CAS 失败后运行中任务最终校验 deadline 未到期而 return，标志位永久 true) |
| `cancel_campus_exit()` | 取消校园网退出流程：swap 标志 + 清 deadline + **同步 `task_manager.cancel("campus_exit")`**（2026-09-05 同 cancel_auto_exit_inner 理由）。如果自动退出未运行，注销快捷键 |
| `cancel_campus_exit_with_notification()` | 快捷键取消校园网退出 (含通知、快捷键注销与同步 cancel 任务) |
| `shutdown_and_exit()` | 统一退出入口 (async)：设置 `is_quitting` → `task_manager.shutdown()` 清理后台任务（**整体 10s 超时上限**）→ `app_handle.exit(0)`。被 `start_campus_exit` 和 `start_auto_exit` 共同调用 |

**B9-8 TOCTOU 竞态修复** (9c)：`start_campus_exit`/`cancel_campus_exit`/`cancel_campus_exit_with_notification` 3 处 `auto_exit_deadline` 的 check-then-act（`is_none()` 检查 + `try_unregister_cancel_exit_shortcut`）原跨锁边界存在 TOCTOU 竞态。修复：3 处均改为持有 `auto_exit_deadline` 锁覆盖 check-then-act，在同一锁临界区内完成 `is_none()` 检查与 unregister，防止 `start_auto_exit` 在间隙注册快捷键。

### 4.11 延迟测试模块 — `monitor/latency.rs`

| 函数 | 说明 |
|------|------|
| `classify_quality_change()` | 质量档位切换纯判定（不落状态不发通知）：恶化到 poor/bad → `Some("bad")`，从 poor/bad 恢复 → `Some("good")`；`BAD_LEVELS` 常量与复核共用，5 个单测锁定 |
| `record_last_quality()` | 落盘 `last_network_quality` 状态（NetworkSnapshot） |
| `notify_quality_change()` | 发送网络质量通知（bad=网络拥堵 / good=网络恢复）+ `emit_login_log`，通知通道单一化（前端 sendNotification API 已删除） |
| `spawn_latency_test_loop()` | 启动延迟测试循环 (CancellationToken) |

**v2.2.5 改进**:

| 改进项 | 说明 |
|--------|------|
| 启动延迟 1 秒 | 每轮检测前 `sleep(1s)`（首轮等效启动延迟 1s），避免网络未稳定时 HTTPS 测试延迟异常 |
| RAII guard | `is_quality_checking.try_acquire()` 返回 `TaskGuard`，作用域结束自动释放，替代手动 `swap_acquire + force_release` |
| 增量推送 | 传递 `Some(&app_handle)` 给 `check_network_quality_async`，启用 Phase 1 + HTTPS 批次增量推送 |
| 后端统一通知 | 网络质量变化通知由后端统一发送（`notify_quality_change`），前端 sendNotification API 已删除 |
| 移除 15s 冷却 (v2.2.6) | 删除 last_quality_check_time 字段及冷却检查逻辑，首次检测可立即执行 |
| 延迟升高复核确认 (2026-09-04) | 恶化到 poor/bad 不再立即通知：`run_quality_check`（quality_scheduler.rs）以 15s 间隔再连续复核 2 次（`SPIKE_CONFIRM_COUNT=2` / `SPIKE_CONFIRM_INTERVAL_SECS=15`），全部达到 `BAD_LEVELS` 才发"网络拥堵"通知，任一次未达标即放弃——避免瞬时抖动误报。复核检测复用 `perform_quality_check`（信号量抢占 + 检测 + 前端推送），前端数据照常更新仅门控系统通知；复核未执行成功（如与手动检测互斥冲突）按证据不足处理：不通知，下轮周期可重新触发。恢复通知不受影响；落状态语义：确认→落末次 bad 值，任一次复核未达标→落该次实际值，复核未产出→落最近一次已产出的复核值（两次均未产出才保持原值） |
| 未在线跳过 (2026-09-03) | `spawn_latency_test_loop` 每轮检查 `any_adapter_online`，Portal 未认证时跳过自动检测（未认证时外网 HTTPS 必被拦截、全超时且误报"网络拥堵"）；前端手动触发的 `check_network_quality` 命令不受限 |
| 质量驱动者收敛 (2026-09-04) | 全量质量检测（`run_quality_check`）的周期驱动者收敛为定时测试循环一个（`spawn_latency_test_loop`，条件 `enable_network_quality && enable_latency_test`）；后台巡检不再顺带触发质量检测（`run_background_check_blocking` 返回 `()`，删除 quality_info 联动），60s 全局节流（`QUALITY_CHECK_MIN_INTERVAL_MS`）随之移除——定时测试间隔（最小 10s）从此真实生效。信号量 `is_quality_checking` 保留防与手动检测并发。代价：不开"定时测试"则质量面板无周期数据（仅手动检测按钮） |
| 就绪短重试 (2026-09-04) | 修复"启动后几秒内拿不到质量首结果"：未就绪（适配器无 IP / `any_adapter_online=false`）时原先走 `continue`，会立刻耗尽 tokio interval 的即时首 tick（首两个循环连续跳过后），之后干等完整周期（30s+）才轮到下一轮——而 `any_adapter_online` 要等后台巡检首次探测完（数秒）才置 true，天然错过。现改为未就绪时内层循环每 2s 短重试且不消耗周期 tick，就绪后立即检测：启动后首次结果从 30s+ 缩短到数秒 |

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

**config_cmd.rs** — 配置保存/加载 (委托 `config/persist.rs`)，空密码兜底逻辑 (前端未传密码且旧密码存在时保留旧密码)；`save_config` 可选参数 `clear_password`（2026-09-03）：显式为 true 时跳过兜底强制置空密码，供账号面板"清除密码"使用；`clearSelfPassword`（2026-09-06）同语义清除自助服务密码（前端经 `saveConfig(cfg, clearPassword, clearSelfPassword)` / useConfigStore 的 `saveConfigDirect` 透传）

**account.rs** — 多账号管理命令（逻辑自含于本文件；`account/mod.rs` 仅声明 crypto 子模块），使用 `list_account_names()` 共享函数，切换账号仅替换账号相关字段保留启动设置，删除账号前检查并清空 `active_account`

**system.rs** — 系统功能命令，`get_init_data` 复用 `persist::list_account_names()` 获取账号列表，返回字段含 `gpuInfo`/`refreshRate`；登录历史记录 `append_login_history()`（最多100条）定义于 `config/persist.rs`，由 `auth/session.rs` 与 `monitor/auto_auth.rs` 调用

**updater.rs** — 更新命令 (委托 `update/updater.rs`)，SHA256 校验和全 4xx 缺失时**默认拒绝安装**（需 `skipSha256WhenMissing`，无前端开关；5xx/传输错误/哈希不匹配一律拒绝），MSI 安装使用 `raw_arg` 支持含空格路径；`get_mirror_urls` 镜像 URL **原样拼接不做百分号编码**（2026-09-03：gh-proxy.com 对整体编码形式返回 403，与 updater.rs 的 sha256 镜像拼接方式保持一致）

### 4.15 提权辅助子进程 — `helper/` (--helper 模式)

**动机**: 需要管理员权限的操作（改 MAC / 设 DNS+DoH）此前在非管理员下提权执行 PowerShell 脚本（`Set-NetAdapter`/`Set-DnsClientServerAddress`）。自 2.4.0 起改为**提权重启自身**：以管理员身份启动当前 exe 并附加 `--helper <op>`，由 Rust 直调 Win32/winreg 完成操作，彻底移除 PowerShell 依赖（含 `-EncodedCommand` Base64 编码与 `escape_ps_single_quote`）。

**执行流**:
1. **主进程** (`platform/helper_spawn.rs::spawn_elevated_helper`)：`std::env::current_exe()` 取自身路径，生成唯一结果文件路径（`%TEMP%/campus-login-helper-<pid>-<ts>.json`），拼参数 `--helper <op> ... --result <path>`，按现有降级链提权启动（COM ICMLuaUtil 静默 → 失败 ShellExecuteW runas 弹 UAC）
2. **helper 进程** (`main.rs` 顶部拦截)：`helper::parse_helper_args` 解析出 `HelperOp`（`Dns{targets 适配器名单, family}` / `Mac{guid, mac_no_dash}`，`--family` 参数默认 "both"，op 后到首个 `--` 参数前为位置参数），`run_helper` 执行：
   - `Dns` → `network::dns_setup::setup_dns_doh_admin(targets, family)`（枚举活跃适配器 → Win32 设置 → 全局 DoH 注册 → flushdns）
   - `Mac` → 按 GUID 在 `get_adapters_force` 中解析适配器名 → `dhcp::apply_mac_change_via_registry`（写注册表 NetworkAddress + release/disable/enable/renew）→ **`dhcp::remove_mac_from_registry` 在 helper 提权上下文内清除注册表伪装值**（2026-09-05：原清理放在非提升的主进程必然 Access Denied 且仅 log_warn 吞掉，伪装 MAC 每次重启后持续生效；运行中 MAC 不受清除影响，重启后恢复物理 MAC）
3. **结果回传**: helper 把 `HelperResult{success, message, op, logs, details: Option<serde_json::Value>}` 原子写入结果文件（tmp + rename，`details` 透传 DNS 设置明细给前端），主进程 100ms 间隔轮询（DNS 超时 30s / MAC 超时 25s），读取后把 `logs` 并入主进程日志，返回 JSON 结果

**要点**: helper 进程不初始化 logger（避免与主进程跨进程写同一日志文件竞争）；参数仅含 GUID/MAC/结果路径等受控字符，适配器名由 helper 自行枚举，无 shell 拼接注入面。

---

## 五、前端模块详解 (React/TypeScript)

> **架构说明**: 前端采用业务域分目录架构，每个业务域目录包含面板组件、逻辑 Hook、类型定义和模块导出。类型定义分散在各业务域的 `types.ts` 中，而非集中在一个 `types/index.ts` 文件。

### 5.1 状态管理架构 — 领域 store 拆分 (3b 阶段 AM-7)

> **重构说明**：原单体 `useAppStore.ts` 已按领域拆分为 5 个独立 store + 1 个日志/Toast store。`useAppStore.ts` 现仅 3 行 re-export 兼容壳，无任何自身状态。所有 store 基于 zustand ^5.0，`localStorage` 已替换为 `safeStorage`（内存降级封装），避免隐私模式下 localStorage 不可用。

**领域 store 一览**：

| Store | 文件 | 职责 | 关键 state | 关键 action |
|-------|------|------|-----------|-------------|
| `useConfigStore` | `useConfigStore.ts` (~220行) | 配置/账号/语言 + 防抖保存 + 密码掩码 + 脏字段跟踪 + configLoaded 启动信号 | `config`/`passwordSaved`/`selfPasswordSaved`/`accounts`/`activeAccount`/`language`/`api`/`dirtyFields` | `updateConfig`(500ms防抖)/`updateConfigLocal`/`syncPasswordSaved`/`syncSelfPasswordSaved`/`saveConfigDirect`(非空 selfPassword 成功置位 selfPasswordSaved)/`setLanguage`/`saveConfigInFlight`(in-flight 保存等待)/`mergeConfigFromBackend`/`clearDirtyFields` |
| `useAuthStore` | `useAuthStore.ts` (300行) | 登录/注销/在线检测/后台状态 + 登录后 60s 手动质量探测节流 | `isLoggingIn`/`isLoggingOut`/`status`/`bgStatus` | `doLogin`/`doLogout`/`checkOnline`/`setStatus`/`setBgStatus` |
| `useAdapterStore` | `useAdapterStore.ts` (74行) | 适配器列表/详情/面板 | `adapters`/`disabledAdapters`/`adapterDetails`/`isRefreshingAdapters`/`activePanel` | `refreshAdapters`/`setAdapters`/`setActivePanel` (模块级 `refreshAdapterData` 公共函数) |
| `useQualityStore` | `useQualityStore.ts` (90行) | 网络质量/DNS DoH/更新/GPU | `networkQuality`/`dnsDohStatus`/`dnsChecking`/`isRefreshingQuality`/`updateAvailable`/`latestVersion`/`releaseNotes`/`gpuInfo`/`refreshRate` | `refreshQuality`/`setNetworkQuality`/`setDnsDohStatus`/`setUpdateAvailable`/`setGpuInfo` |
| `useThemeStore` | `useThemeStore.ts` (86行) | 主题/亮暗/自定义色 + DOM 副作用 | `themeName`/`isLightMode`/`customThemeColor` | `setThemeName`/`setIsLightMode`/`initTheme`/`setCustomThemeColor` |
| `useLogToastStore` | `useLogToastStore.ts` | 日志/Toast (独立 zustand，MAX_LOG_ENTRIES=300；**`addLog` 连续重复折叠+×N 计数 (2026-09-11)：与末条 message+type 相同不再静默刷时间戳，而是累计 `LogEntry.count` 并刷新 time，RightPanel 渲染 ×N 徽标——对齐 logback DuplicateMessageFilter/tracing-dedup/Chrome DevTools 的"连续重复折叠+计数"惯例，重复次数是诊断信息；跨条 A-B-A 不折叠（会打乱时间序，场景不密集）。后端文件日志保留原始逐条不折叠，审计优先**；Toast 上限 MAX_TOASTS=4，`addToast`/`addToastWithAction` 同 title 去重防重复刷屏) | `logs`/`toasts` | `addLog`/`addToast`/`addToastWithAction`/`removeToast`/`removeToastsByPrefix` |

> **通知单通道规范 (2026-09-03 重构)**：一条通知只有一个来源、一个通道、一个文案源，杜绝双通道重复。
> - `emit_notification`（`infra/notification.rs`）**只发 Windows 系统通知**（用户看不到主窗口即 `!is_visible() || is_minimized()` + `enable_notification` 时，2026-09-05 由 is_focused 改为与 heartbeat 一致的 monitorable 判定：窗口可见但失焦不再误弹打扰；窗口不存在视为看不到继续弹），不再向前端发 `system-notification` 事件（`EventBus.emit_system_notification` 已删除）；系统通知文案为中文硬编码（后端无法感知前端 UI 语言，为已知边界）
> - 应用内 toast/日志由**业务专用事件**负责：`onAutoLoginResult`（登录成功/失败）、`onAutoExitCountdown`（即将退出+取消按钮）、`onAutoExitCancelled`、`onCampusExitCountdown`/`onCampusExitCancelled`（校园网退出/取消+按钮）、`onNetworkQualityResult`→`handleQualityBadAlert`（质量告警）、`onLoginLog`（过程告警日志：检测到断线/重连失败/网络仍断线/网络拥堵/恢复——原 emit_notification 调用点已补 `emit_login_log`）、`onUpdateAvailable`（发现新版本）
> - store 防护：`MAX_TOASTS=4` + 同 title 去重 + 超限淘汰时清理定时器；窗口非前台时普通 toast 不入队（信息由日志兜底），带 action 的 toast 仍入队（承载取消退出操作入口）
> - 通知文案 i18n：专用通道统一走 `i18next.t('notify.*')`；`enable_notification` 开关仅控制系统通知，应用内 toast 不受影响（设置面板描述已注明）
| `useAppStore` | `useAppStore.ts` (3行) | **兼容壳**，仅 re-export `useAppInit`/`hasPendingConfig`/`flushPendingConfig` | 无 | 无 |

**密码处理** (迁移至 `useConfigStore`)：`password === PASSWORD_MASK` 时两层防护——`updateConfig` 合并挂起配置时若旧挂起有真实密码但新 partial 传 MASK，保留旧挂起真实密码；`flushPendingConfig` 最终合并时若 password 仍是 MASK 则 `delete`，让后端识别 MASK 并保留原密码。

**刷新锁统一模式**：各领域 store 均采用模块级 `_xxxLockFlag` 防抖锁模式：
- `useConfigStore`：`saveConfigTimer` + `saveConfigPending`（500ms 防抖保存）+ `dirtyFields`/`dirtyFailureCounts`（后端回写跳过脏字段，连续 3 次失败放弃脏标记）
- `useAuthStore`：`_checkOnlineLockFlag` + `checkOnlineEpoch`（promise settle 时 finally 立即释放，非 setTimeout 延迟释放）
- `useAdapterStore`：`_adapterLockFlag`
- `useQualityStore`：`_qualityLockFlag`

**主题 DOM 副作用** (迁移至 `useThemeStore`)：store 模块底部 `subscribe` 监听 `isLightMode`/`themeName`/`customThemeColor` 变化，toggle `dark` class、添加 `theme-${name}` 类、计算 CSS 变量 `--primary`/`--ring`/`--accent`。

> 注：`activePanel` 归 `useAdapterStore` 管理（历史归位，语义上与适配器关联较弱但实际如此）。`i18next.t()` 在 action 函数体内调用（非 Store 创建时），避免初始化时序问题。

### 5.2 IPC 封装 — `hooks/tauriApi.ts` (原 useIpc.ts)

> **重命名**：`useIpc.ts` 已重命名为 `tauriApi.ts`（commit e06203d），从 hook 风格转向纯 API 模块（无 React 依赖）。

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

- `getCurrentWindow().onCloseRequested` — 拦截关闭，若有 pending config 先 `flushPendingConfig()`，再 await 其返回的保存（2026-09-05：返回值纳入本次 flush 新发出的 `saveConfig` promise 与既有 in-flight 的合并等待，仅 debounce pending 时不再拿到 null 直接关窗丢数据；`Promise.race` 2s 上限保持）后才关闭
- `onBackgroundCheckResult` — 更新 `bgStatus`、记录在线/离线日志（1s 节流 + 5s 在线日志节流）
- `onAdaptersChanged` — 更新 store，500ms 节流（前缘+后缘双重保护）
- `onAdapterDetailsChanged` / `onDisabledAdaptersChanged` / `onAdapterDisabledWarning`
- `onAutoLoginResult` — Toast + 触发 `checkOnline`
- `onLoginLog` — 写入 `useLogToastStore.addLog`
- `onAutoExitCountdown`/`onAutoExitCancelled`/`onCampusExitCountdown`/`onCampusExitCancelled` — 倒计时 Toast
- `onNetworkQualityResult` — 合并到 `useQualityStore.networkQuality`，触发"延迟过高"告警 (`handleQualityBadAlert`)
- `onUpdateAvailable` / `onConfigChanged`（经 `mergeConfigFromBackend` 合并——跳过本地脏字段，不整体覆盖）

**关键策略**：监听器先于数据获取注册（在 `useInitialDataLoad` 之前），避免遗漏初始化期间事件；`mountedRef` 防止 unmount 后写状态；系统通知由后端统一发送（前端 sendNotification API 已删除）；网络质量事件无防抖，增量推送可立即更新 UI。

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

**幂等保护**：`mountedRef` 防止 unmount 后写状态（StrictMode 二次 setup 时恢复 `mountedRef.current = true`，不短路初始化——旧实现的 `initDoneRef` 已移除）；catch 块中 `showWindow` 不受 `mountedRef` 影响（应用级操作）。

**`configLoaded` 确定性信号 (2026-09-06)**：成功路径与失败降级路径都置位 `useConfigStore.configLoaded`（157/165 行）——自助服务面板的自动验证+自动刷新严格等待该信号，配置加载完成且凭据就绪才弹 Hello 并拉取，消除启动加载窗口期的时序竞态；窗口期内面板卡提示"配置加载中..."。后端配套：setup 启动时 gpu-warmup 后台线程预热 GPU/刷新率检测（OnceLock 缓存），`get_init_data` 读缓存即返回，前端更早拿到配置。

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
| `DashboardPanel.tsx` | 总览面板，卡片可拖拽排序（framer-motion Reorder.Group），5张自定义卡（`ALL_CARDS`: QuickActionsCard/AccountManageCard/SelfOnlineCard/SelfLogCard/NetworkQualityCard，质量总开关关闭时隐藏网络质量卡），布局持久化到safeStorage。**2026-09-06 新增"在线信息"与"近期上网记录"两卡**（`SelfOnlineCard`/`SelfLogCard`；共用 hook `useSelfCardReveal`（凭据判断 + 掩码/验证切换）与 `useSelfCardFetch`（自动查询骨架：按学号只查一次、引用变化不重查、错误卡内重试 + toast）定义于本文件；自动查询走 `querySelfDashboard`/`querySelfOnlineLog`，密码传空串由后端回退已保存值，命令仅回传非敏感概览不触发验证；关键信息（IP/登录时间/时长/流量明细）未验证时圆点掩码，点眼睛经 `useHelloGate` 验证后显示、再点切回不再验证；每卡显隐独立，与账号页绑定卡共用门生命周期；无凭据显示引导提示，`selfHelloEnabled` 关闭时眼睛直接放行）。注意 framer-motion 对 Reorder.Item 内联写 `touch-action: pan-x`（axis=y），class 层的 touch-action 会被覆盖，触摸垂直滚动让位于拖拽排序；列表溢出时的滚动可达性由全局细滚动条保证（2026-09-04）。总览滚动末尾（空态块后）渲染低透明度背景看板娘 `mascot-bg-laptop`（2026-09-11，w-64、opacity 0.10/深色 0.06、pointer-events-none，文档流末尾 img 而非 absolute——随滚动被内容自然遮挡） |
| `DashboardPanel.selfCards.test.tsx` | 总览自助两卡单测（2026-09-06）：自动查询+掩码、验证后明细/切回不再验证、验证失败保持掩码、无凭据不查询 |
| `AboutDialog.tsx` | 关于对话框，双栏布局(应用信息+更新仪表盘)，镜像源选择，下载状态机(idle→selecting→downloading→done/error)，Release Notes渲染。**2026-09-03 修复**：`ensureFullUpdateInfo` 在一键下载前确保 updateInfo 完整（系统通知缓存路径构造的对象缺 `sha256Checksum`/`assets`，原样使用会下载 404 且安装被后端拒绝）；安装失败在 done 态显示错误文案（原先静默失败无任何反馈）；兜底下载文件名对齐真实资产命名 `Wxxy-CampusLogin_{v}_x64-setup.exe`。**2026-09-04 布局调整**：一键下载按钮与切换下载源入口从右侧栏顶部移到底部（`mt-auto`），新功能亮点/核心优势卡片置于顶部；核心优势卡片宽度 260px→340px 使"双适配器支持"标题单行；左栏描述文案改为无锡学院专属（`about.appDesc`="无锡学院校园网自动登录助手"、`about.dualAdapterSupportDesc`="适配无锡学院双网卡环境"，zh/en 同步——应用仅支持无锡学院，不再宣称兼容多种校园认证方式）。**赞助入口（2026-09-04）**：左栏底部新增"赞助支持"按钮（`about.sponsor`，rose 风格遵循固定亮色皮肤无 dark: 变体），点击后**右栏原地切换为赞助内嵌页**（`showSponsor` state，标题+双码大图+右下角"返回"，右栏现有更新仪表盘内容用 `contents/hidden` 整体切换——最小 diff 且布局语义不变；对话框关闭时重置回仪表盘），不关闭对话框、不回主界面弹浮层；标题栏 Heart 才打开主界面下拉浮层。**固定亮色皮肤（2026-09-04）**：对话框内 30 处 `dark:` 变体类全部移除，DialogContent 挂 `index.css` 的 `.force-light-dialog`（容器级重定义主题变量为浅色值 + 显式 `color: hsl(var(--foreground))`——`color` 是继承属性，body 按暗色变量算出的颜色会直接继承下来，仅重定义变量不够），修复暗色模式下白底上近白文字几乎不可读的存量缺陷；浅色模式视觉无变化 |
| `useAuth.ts` | 认证逻辑 Hook |
| `types.ts` | 认证类型定义 (PortalStatusResult, CommandResult, LoginResult) |
| `index.ts` | 模块导出 |

#### 5.4.2 账号模块 — `account/`

| 文件 | 说明 |
|------|------|
| `AccountPanel.tsx` | 账号管理面板，两列网格等高布局(左列：登录信息卡+自动化设置开关卡(`flex-1` 撑满与右列底部对齐)；右列：**绑定运营商账号**卡输入框垂直排布+绑定状态区(2026-09-05，query_bind_status 查询：手机号掩码前三后二/查看密码走 Windows Hello 验证后 reveal_operator_credential 临时显示明文，与绑定/查询共用 `useHelloGate` 门 2026-09-06)；两个密码框旁均有**"清除密码"入口**（登录密码 `clearPassword`、自助服务密码 `clearSelfPassword`，仅已保存时显示，`onMouseDown preventDefault` 防夺焦；清除自助密码顺带清空本地草稿防 blur 兜底存回)；下方账号管理卡全宽) |
| `selfServiceState.ts` | 绑定卡与自助服务面板**跨面板共享层**（zustand，非持久化）：`useSelfCredStore`（学号+自助服务密码共用输入，切面板不丢失，退出应用即清空）、`useHelloGate`（绑定/明文查看门：时间戳 `helloGateVerifiedAt` TTL 570s、`gateFresh` 含 `elapsed >= 0` 回拨守卫、`ignoreToggle` 选项使明文查看无视总开关强制验证）、`useSelfServiceVerify`（自助面板会话门 `selfSessionVerifiedAt` 同款 TTL，`resetSelfSessionGate()` 面板卸载重置） |
| `SelfServicePanel.tsx` | "自助服务"独立面板(2026-09-05，协议见 §4.5.4.3；从账户管理页单开，`PanelName`/`PANEL_TITLES`/NAV_ITEMS/设置页默认面板选项四处接入)：**在线信息**表(操作列注销→ConfirmDialog→self_offline_session→本地移除行) + **近期上网记录**卡(2026-09-06：日期范围筛选默认今天 + 汇总数据区8格 + 12列明细表横滚；**金额列 `fmtMoney` 保留原始精度**——历史行此前 parseInt 截断，服务端发 0.50 显示成 0；空值/非数值显示 `-`)；凭据区在"在线信息"卡内顶部(与绑定卡共用 `useSelfCredStore`，学号默认取 config.user，密码 blur 经 `saveConfigDirect` DPAPI 落盘)，密码框旁"清除密码"入口；未填凭据时按钮禁用+提示 |
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
| `useNetwork.ts` | 网络逻辑 Hook；模块级导出 `normalizeDhcpResults`（单条/批量结果归一化）+ `announceDhcpResults`（成功/跳过/失败三类 i18n toast，2026-09-05 起与 NetworkPanel "获取新IP"共用同一实现，替代面板内逐行复制品与 hook 内硬编码中文文案） |
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
| `SettingsPanel.tsx` | 设置面板，页面顺序：外观 → **两列区**(2026-09-06，md 断点起两列：左"启动设置"卡；右列"系统通知"/"安全设置"/"新手指引"三张独立卡垂直排列，grid stretch + `justify-between` 拉伸至与左列等高、剩余空间自动均分卡间隙) → "网络质量检测"大卡整宽收尾；"启动时默认显示"由按钮网格改为 **Radix Select 下拉框**（9 选项压缩为 1 行，"记住上次"用哨兵值映射——Radix Item 不接受空串 value；质量检测关闭时下拉中隐藏 quality 项）；**"安全设置"卡两开关**（2026-09-06）：`selfHelloEnabled`（默认开，Hello 验证总门）与 `selfReverifyEachAction`（默认关，每次操作验证严格模式）——**关闭方向必须先过 Hello 验证**（`handleSecurityDisable`，防绕过界面关闭保护），开启方向免验，总开关关闭时二次验证开关禁用；+7种主题+12色预设+取色器+亮暗模式 |
| `ThemeDialog.tsx` | 主题对话框，2列布局+亮暗模式切换 |
| `OnboardingWizard.tsx` | 5步引导向导(欢迎→绑定运营商账号(可跳过)→账号→适配器→完成)，Framer Motion滑动转场，含语言切换，完成后自动登录；绑定步骤调 `bind_operator` 命令完成自助系统登录+运营商绑定（2026-09-05），**绑定步骤接入 `useHelloGate` 验证门**（2026-09-06，后端 bind 命令补 `ensure_identity_gate` 的配套——不先行验证会被后端拒绝） |
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
| `ToastContainer.tsx` | Toast容器，4种类型(info/success/error/warning)，economy档简单transition替代spring，支持action按钮。带 `mascot` 字段（`'celebrate' | 'offline'`，2026-09-11）时左侧渲染对应看板娘图（裸 img 非 MascotFigure，避免 tailwind 尺寸类冲突），登录成功/失败由 `useAuthStore` 经 `useLogToastStore.addToast` 第 5 参传入 |
| `MascotFigure.tsx` | 看板娘展示组件（双端同构，2026-09-10）。`variant` 对应 `/girl/mascot-{portrait,welcome,empty,celebrate,sponsor,offline}.png`，`size` 三档（sm w-20/md w-28/lg w-40）；tailwind 尺寸类无法被 className 可靠覆盖，特殊尺寸场景用裸 img。**背景装饰娘 4 张**（`mascot-bg-{laptop,nap,lounge,tea}.png`，2026-09-11）：不经组件，低透明度裸 img 直引。**桌面端**：App.tsx 布局层两个 fixed 侧边娘（≥1560px 宽视口显示，left-2 / right-[304px] 避让 w-72 的 RightPanel，z-0 沉于 main z-[1] 卡片下层，占位图待站姿竖版新图替换 src）——主内容四面板（总览/设置/网络质量/日志）滚动末尾的底部娘已由侧边娘替代移除；RightPanel 日志栏保留滚动末尾底部娘（w-56，独立窄栏无侧边空间）。**安卓端**：四面板滚动末尾底部娘保留（窄屏无侧边空间；设置页图加 `pb-72` 撑滚动余量避免被固定底栏遮住）。**坑：`space-y-*` 容器的子元素 margin-bottom 被 `space-y-4 > * + *` 规则锁死为 0（specificity 更高），间距要用 padding 不用 margin**。素材链：AI 原图归档 `assets/ui-girl/original/` → rembg(isnet-anime) 抠图 → `assets/ui-girl/` → 双端 `public/girl/`（2026-09-11 v2 重做：去 WiFi 呆毛改卷曲呆毛、8 张全套替换、图标同步重制） |
| `SponsorCard.tsx` | 赞助下拉浮层 (2026-09-04)。**非模态**：无遮罩、不抢焦点、不阻塞交互，点击浮层外任意处(window pointerdown capture)或 Esc 即关闭。锚定标题栏赞助按钮下方自然向下展开（fixed top-[52px] right-[104px]，带指向按钮的小箭头，z-[60]，高于 DockNav 菜单同级低于 toast z-100），自动弹出与手动入口共用此浮层。内嵌微信/支付宝收款码 (public/sponsor-weixin.png / sponsor-alipay.jpg)。浮层顶部居中渲染赞助看板娘（MascotFigure variant=sponsor，2026-09-11）。文案走 i18n sponsor 段 + about.sponsor |
| `types.ts` | 共享类型定义 (UpdateAvailableData, UpdateInfo, DownloadProgress, MirrorSource 等) |
| `ui-types.ts` | UI 类型定义 (StatusState, PanelName(8个: dashboard/account/network/monitor/quality/settings/log/speedtest), ThemeName(7种), LogType, GpuTier, GpuInfo, LogEntry, ToastMessage, AdapterDisabledWarningData, AutoExitCountdownData, SaveConfigResult 等 10 个导出) |
| `ui-constants.ts` | UI 常量 (MAX_LOG_ENTRIES=300/APP_VERSION='2.3.0'/APP_NAME='校园网登录助手'/PASSWORD_MASK='***'/NAV_ITEMS=8个导航项/Z_INDEX 分层常量) |
| `index.ts` | 模块导出 |

### 5.6 布局组件 — `components/layout/`

| 文件 | 说明 |
|------|------|
| `DockNav.tsx` | 适配器选择浮层 + 注销按钮 (无线蓝色Wifi/有线绿色Cable图标, 300ms延迟关闭/150ms延迟打开)，选择项收敛为主/副适配器（`scopedAdapters`，2026-09-04），GSAP 磁吸效果（MAGNETIC_RANGE=80, MAX_SCALE=1.35, MAX_LIFT=-14），economy档禁用磁吸，RAF节流。tooltip 水平居中用 Tailwind `-translate-x-1/2`（2026-09-03：原 inline `translateX(-50%)` 覆盖 class transform 导致上浮动画失效） |
| `RightPanel.tsx` | 右侧面板，运行日志+网络适配器信息(可展开/折叠，显示IP/子网掩码/网关/DHCP/MAC)，空日志时呼吸动画。清空日志 GSAP 动画 stagger 动态封顶（>8条0.05s/>4条0.1s，2026-09-03：原固定 0.2s/条，日志满 300 条时动画约 60 秒且按钮禁用无法取消），与 LogPanel 同策略 |
| `TitleBar.tsx` | 标题栏，看板娘头像 logo（mascot-portrait 圆形裁剪 w-10，2026-09-11 由 w-7 放大以突出形象）+版本号+更新提示+工具按钮(亮暗/语言/通知/主题/赞助Heart/关于/最小化/最大化/关闭)，双击最大化，拖拽移动窗口 |

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
- **崩溃恢复** (`setupCrashRecovery`): 最多3次自动重载，GPU/WebGL/SharedArrayBuffer 错误触发重载，渲染心跳10秒无响应视为GPU崩溃触发重载（FE-A-11 由 5s 放宽），页面可见性变化时暂停/恢复 GSAP globalTimeline
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
- **面板转场**: `AnimatePresence mode="wait"` + `panelVariants`（createPanelAppleVariants）+ slideDirection；切换锁 60ms（2026-09-04 由 120ms 收紧：锁只需覆盖退出时长，锁内点击仍按设计丢弃）。**内容用 deferredPanel**（2026-09-04：`useDeferredValue(activePanel)`，快速连切跳过中间面板 mount）——面板 switch/转场 key/标题 key/滑动方向全部消费 `deferredPanel`，`activePanel` 仅用于 DockNav 高亮与 storage 恢复
- **quality 面板可见性联动**（2026-09-03 约定）: `enableNetworkQuality === false` 时 App 对 quality 面板渲染 `null`、DockNav 过滤入口。三处必须联动——`useInitialDataLoad` 启动恢复 `defaultPanel`/`savedPanel` 时跳过 quality（否则重启后主区域空白）、`SettingsPanel` 关闭质量开关时清 `defaultPanel` 并把 `activePanel` 切回 dashboard。新增受开关控制的面板时同样需三处联动
- **窗口监听**: `getCurrentWindow().onResized` 监听窗口大小变化
- **引导向导**: 首次启动检测（`safeStorage.get('campus-onboarding-done')`），未完成则弹出 OnboardingWizard
- **赞助下拉浮层自动弹出** (2026-09-04): 已有账号才弹（`configUser` 非空，与 onboarding 的 `!configUser` 条件天然互斥）→ 启动 1s 延迟（`SPONSOR_SHOW_DELAY_MS`，等启动入场动画完成）→ `document.visibilityState === 'visible'` 才弹（静默启动/最小化时挂 visibilitychange 推迟到可见）→ 7 天频控（`sponsor-last-shown` epoch ms 存 localStorage，`SPONSOR_SHOW_INTERVAL_MS`）。弹出瞬间即写时间戳；标题栏 Heart 与关于对话框"赞助支持"两个手动入口不受频控、不写时间戳。频控判断在 `configUser` 短路之后，二者叠加保证首次使用（无账号）阶段完全不打扰。浮层为标题栏按钮下方下拉展开（曾尝试独立外挂子窗口方案，实测体验不佳已废弃，改回窗口内非模态浮层）。
- **ErrorBoundary 嵌套**: 外层 ErrorBoundary（L460）+ 面板内容 ErrorBoundary（L383）+ main.tsx ErrorBoundary
- **useLogToastStore**: 独立 zustand store 用于 Toast 管理

---

## 六、IPC 通信完整清单

### 6.1 请求-响应命令 (v2.3.2: 56个)

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
| `adapter-details-changed` | 适配器详情变更 |
| `campus-exit-countdown` | 校园网退出倒计时 |
| `campus-exit-cancelled` | 校园网退出已取消 |
| `config-changed` | 配置变更 |

**安卓端 IPC 面**：命令与桌面同名对齐（`do_login`/`do_logout`/`check_portal_status`/`check_campus_status`/自助服务/账号管理/`get_init_data`/日志/网络质量/更新全族，前端 `tauriApi` 接口面两端一致）；桌面专属命令（app/helper/monitor 启动等）在安卓 cfg 门控不可见。事件面为桌面子集：`background-check-result`/`login-log`/`auto-login-result`/`network-quality-result`/`update-available` 等；适配器×4、自动退出×2、校园网退出×2、`config-changed` 等桌面事件安卓不 emit。

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

### 7.3 依赖关系图

```
main.rs (二进制入口)
  └── lib.rs (库入口) → app/startup.rs::run()
        │   [build_runtime: Tokio multi-thread, worker=clamp(2,8), max_blocking=clamp(8,64)]
        │   [run: 命令注册 generate_handler!(56个) + WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS ← gpu.rs::build_browser_args]
        │   [plugin 注册: shell/notification/autostart/global-shortcut/single-instance]
        │   [setup_app: 状态管理 + 托盘 + run_startup_tasks(后台检测/延迟循环/自动登录) + 心跳与窗口安全线程]
        │
        └── commands/mod.rs
              ├── config_cmd.rs ← config/, account/crypto.rs, infra/state/
              ├── login.rs ← auth/service.rs, infra/events.rs, infra/lifecycle.rs, monitor/watcher.rs
              │   [do_login + do_logout (两步注销), adapter_name 可选参数]
              ├── background.rs (命令入口，委托 monitor::watcher)
              ├── network_cmd.rs ← network/*, infra/state/, platform/dns_config.rs, platform/elevation.rs, platform/helper_spawn.rs, network/dns_setup.rs, monitor/watcher.rs, monitor/latency.rs, auth/portal.rs
              │   [check_dns_doh_status / setup_dns_doh / check_campus_status / check_portal_status / start_latency_test]
              ├── account.rs ← config/, account/crypto.rs, infra/state/, config_cmd.rs
              ├── self_service.rs ← self_service/（Dr.COM Self 协议）, platform/identity.rs（验证时效）, infra/state/
              │   [bind_operator/query_bind_status/verify_windows_identity/reveal_operator_credential/query_self_dashboard/query_self_online_log/self_offline_session；`ensure_identity_gate`(本文件) 对改变外部状态的命令校验后端 TTL 门]
              ├── system.rs ← infra/state/, config/(model/persist), network/(缓存查询), platform/autostart.rs, platform/gpu.rs, config_cmd.rs
              └── updater.rs ← update/updater.rs

app/ (应用生命周期模块)
  ├── mod.rs (重导出)
  ├── startup.rs — run/build_runtime/setup_app (入口 + 命令注册; panic hook 位于 main.rs)
  ├── tray.rs — 系统托盘 (菜单/事件处理)
  ├── window.rs — 窗口焦点内存调节 (handle_window_focus_event) + show_and_focus_main
  ├── shortcut.rs — 全局快捷键 (Ctrl+Shift+C 取消自动退出)
  ├── heartbeat.rs — 渲染进程心跳检测 + spawn_window_safety_thread (3秒保底显示窗口; reload 恢复动作收敛至 webview_recovery)
  ├── webview_recovery.rs — WebView2 崩溃恢复统一入口 (ProcessFailed 订阅分级处理 / attempt_webview_recovery 5分钟3次限流 / 运行时版本记录, 2026-09-11)
  └── shutdown.rs — graceful_exit + handle_window_close_event (关闭进托盘/退出分流; 统一入口 shutdown_and_exit 在 infra/lifecycle.rs)

infra/
  ├── mod.rs (重导出)
  ├── async_util.rs — block_on_sync (同步上下文驱动 future, 防 async 线程 block_on panic)
  ├── state/ ← config/model.rs, arc-swap, parking_lot, tokio-util (子目录重构自 state.rs)
  │   ├── mod.rs     [TaskLock/TaskFlags/AppState (6 字段 + UpdateStats 子结构体) /CommandResult/AccountResult]
  │   ├── store.rs   [ConfigStore: ArcSwap<Config> CAS 更新]
  │   ├── network.rs [NetworkState + NetworkSnapshot: ArcSwap 快照 CAS 读写]
  │   └── exit.rs    [ExitStateStore]
  ├── task_manager.rs — BackgroundTaskManager (cancel token 统一管理)
  ├── logger.rs — 日志系统 (flush_quick: panic hook 专用 500ms 超时 flush; cleanup_old_logs_by_time: retention_days==0 时永久保留; set_debug_mode/get_debug_mode)
  ├── lifecycle.rs ← infra/state/, infra/notification.rs
  │   [start_auto_exit / cancel_auto_exit_inner / start_campus_exit / cancel_campus_exit / cancel_campus_exit_with_notification / shutdown_and_exit]
  ├── notification.rs — emit_notification 封装
  ├── events.rs — EventBus (15 个 emit_xxx 方法, emit_login_log/emit_network_quality/...)
  └── command_context.rs — CommandContext::from_app (统一访问 ConfigStore/TaskFlags/NetworkState/ExitStateStore)

monitor/ (10 个子模块，portal_failure 已迁入 auth/failure_tracker)
  ├── mod.rs (模块声明 10个 pub mod; trigger_background_check 是 commands/background.rs 的独立命令)
  ├── watcher.rs (门面 54行, re-export background_check/background_task + run_startup_tasks)
  ├── background_check.rs ← auth/failure_tracker/auto_auth/lifecycle/portal_check/campus_check/background_emit
  │   [run_background_check_blocking 检测主体 + run_background_check async 包装]
  ├── background_task.rs ← infra/task_manager [start_background_check_inner + task_manager.spawn]
  ├── auto_auth.rs ← infra/state/, infra/events.rs, infra/notification.rs, infra/lifecycle.rs, auth/service.rs, auth/portal.rs
  ├── latency.rs ← infra/state/, network/*, infra/notification.rs, quality_scheduler.rs [spawn_latency_test_loop]
  ├── adapter_watch.rs ← infra/state/, infra/events.rs, CancellationToken
  ├── campus_check.rs — 校园网检测 (CampusCheckResult 定义于此)
  ├── portal_check.rs — Portal 检测 (PortalCheckResult 定义于此 + check_adapter_portal)
  ├── quality_scheduler.rs — 质量检测调度器
  └── background_emit.rs — 后台事件推送

auth/ (6 个子模块，原 traits.rs 已删除)
  ├── mod.rs (重导出)
  ├── portal.rs ← network/client.rs [block_on_http 同步-异步桥接]
  ├── protocol.rs ← network/client.rs, reqwest, urlencoding, regex [random_v 定义于此]
  │   [两步注销: 2轮循环 MAC解绑+Radius注销, callback 动态生成 dr100{round+1}/dr100{round+2}]
  ├── session.rs ← auth/portal.rs, auth/protocol.rs, network (Adapter 类型, 源自 discovery 经 mod.rs re-export)
  │   [adapter_action_with_log / login_adapter_with_log 通用封装]
  ├── service.rs ← auth/session.rs, auth/protocol.rs, auth/failure_tracker.rs, auth/dual_adapter_executor.rs, network/adapter.rs, infra/lifecycle.rs
  │   [full_login / full_logout 统一入口 + logout_adapter_with_log + post_login_handler (直接调用 network::resolve_adapter_names，无 trait)]
  ├── failure_tracker.rs ← infra/state/, network/dhcp [is_auth_failure / update_auth_failure_count / handle_portal_request_failure / reset_all]
  └── dual_adapter_executor.rs — execute_dual<F1,F2> 双适配器并行执行 (B9-7 泛型静态分发 + tokio spawn_blocking + 可中断错峰)

network/ (9 个业务子模块 + discovery/ 子目录)
  ├── mod.rs (重导出)
  ├── client.rs ← arc-swap, lazy_static, dashmap, reqwest [TLS 1.3+回退, PORTAL_URL/CLIENT_POOL]
  ├── adapter.rs ← config/model, infra/events, discovery, adapter_cache
  │   [适配器选择: resolve_adapter_names/find_dual_adapters/select_adapter/filter_operation_adapters]
  ├── adapter_cache.rs — 适配器查询缓存 (force/cached 双模式, TTL 5s + 4s 后台刷新, validate_adapter_name)
  ├── dhcp.rs — DHCP 操作 (release/renew, MAC 重置, apply_mac_change_via_registry)
  ├── subnet.rs — 子网判定 (/18 校园网子网匹配)
  │   [校园网检测: 网络名称/子网/网关Ping]
  ├── dns.rs — DNS 缓存管理 + 评分系统 + DoH解析 + 智能解析策略
  │   ├── DNS_SERVER_SCORES / DOH_SERVER_SCORES (dashmap 评分表, ServerScore 共用)
  │   ├── resolve_host_smart (三级智能解析)
  │   └── resolve_via_doh (RFC 8484 wire format)
  ├── timing.rs — HTTP 计时
  │   └── measure_https_timing / measure_dns_query / measure_doh_timing / bind_and_connect / do_tls_handshake
  ├── quality.rs ← infra/events.rs, network/timing.rs, network/dns.rs, tauri::AppHandle
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
  │   [set_profile_dns_via_api / set_dns_via_api / clear_adapter_dns_via_api]
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
  ├── model.rs — Config 结构体(40字段) + Default + deserialize_non_empty_or + default_campus_gateway
  ├── persist.rs — atomic_write + list_account_names + get_data_dir + append_login_history + save_config_to_disk_encrypted
  └── validate.rs — 校验逻辑 (枚举值/正则/URL/Portal URL 迁移/校园网关校验)

account/
  ├── mod.rs — 仅声明 crypto 子模块 (多账号命令逻辑在 commands/account.rs)
  └── crypto.rs — Windows DPAPI 加密/解密

update/
  ├── mod.rs (模块声明, 无重导出)
  └── updater.rs ← reqwest, url, sha2
      [VERSION_MIRRORS: 3个镜像源(ghfast.top/gh-proxy.com/ghproxy.net)]
      [start_update_check_loop: 24小时间隔自动更新检查]
      [verify_download_sha256: 分块流式读取计算 SHA256，64KB buffer]
      [SHA256 校验文件支持镜像源 URL 列表]

App.tsx (466行, App + AppInner)
  ├── 领域 store (zustand, useShallow 选择性订阅)
  │   ├── useConfigStore (config/accounts/language + 防抖保存 + 脏字段 + configLoaded 信号)
  │   ├── account/selfServiceState (useSelfCredStore 绑定卡与自助面板共用凭据 + useHelloGate/useSelfServiceVerify 验证门)
  │   ├── useAuthStore (doLogin/doLogout/checkOnline/status/bgStatus)
  │   ├── useAdapterStore (adapters/details/activePanel)
  │   ├── useQualityStore (networkQuality/dnsDoh/gpuInfo)
  │   ├── useThemeStore (themeName/isLightMode/customThemeColor, 由 ThemeDialog/设置侧消费)
  │   └── useLogToastStore (logs/toasts)
  ├── tauriApi.ts ← @tauri-apps/api (原 useIpc.ts，纯模块非 hook)
  └── useAppInit.ts (编排 hook)
        ├── useEventListeners.ts (14 个事件订阅 + 窗口关闭拦截)
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
| 适配器名称校验 | network/adapter_cache.rs::validate_adapter_name，禁止 `&\|;\`$()<>\"'\n\r\0` 等元字符，防命令注入 |
| 敏感操作验证门 (2026-09-05/06) | 查看明文/绑定/踢下线过 Windows Hello 门：前端门 TTL 570s（`account/selfServiceState.ts` 时间戳会话，`gateFresh` 含 `elapsed >= 0` 时钟回拨守卫；明文查看 `ignoreToggle` 无视总开关强制验证）+ 后端真防线（`platform/identity.rs::identity_verified_recently` 600s TTL 含回拨拒绝；`commands/self_service.rs::ensure_identity_gate` 对 bind/offline 等改变外部状态的命令校验）；只读查询命令有意不设门（总览卡自动刷新依赖免验证拉取，Bitwarden reprompt 分级保护/sudo 仅副作用命令需认证同款思路）；`selfHelloEnabled=false` 时门整体放行但明文查看仍强制验证 |
| 出站掩码唯一出口 (2026-09-06) | 所有把 Config 发往前端的路径必经 `Config::masked_for_display()`（password + self_password 双字段掩码，空值=未设置语义保留），回归单测锁死双字段断言（config_cmd.rs）——详见 §4.3 掩码纪律 |
| 自助服务凭据纪律 | 学号/自助服务密码仅内存传递不落盘不写日志（查询命令参数为空/MASK 时后端回退已保存值）；自助服务密码持久化与登录密码同措施（DPAPI + MASK 出站 + 显式清除标志） |

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
| GSAP动画迁移 | 约 12 个 CSS 动画迁移至 GSAP，全局配置 `gsap.defaults({ease:'expo.out'})` + `gsap.config({autoSleep:5})` + `gsap.ticker.lagSmoothing(500,33)` (main.tsx:15-16)，`force3D` 不再全局默认（各 tween 显式声明） | GPU合成层加速，空闲自动暂停，动画结束回收 |
| RAF节流+位置去抖 | Button/DockNav/AnimatedCard鼠标事件节流 | 减少无效getBoundingClientRect调用 |
| transition-all替换 | 10 处替换为显式属性列表（剩余 4 处 AboutDialog/OnboardingWizard/DockNav×2 为有意保留） | 减少不必要的属性过渡计算 |
| WebView2 内存管理 | 前台 NORMAL/后台 LOW (ICoreWebView2_19.SetMemoryUsageTargetLevel) | 后台内存占用显著降低 |
| WebView2 参数精简 (2026-09-03) | `build_browser_args` 由厂商分支 11 参数精简为仅 `--js-flags=--max-old-space-size=512`：`--enable-native-gpu-memory-buffers`/`--gpu-memory-buffer-size-mb`/`UseSkiaRenderer` 已从 Chromium 移除，`--renderer-process-limit` 不再被读取，`--num-raster-threads`/`--enable-gpu-rasterization` Windows 默认即开，`EnableDrDc` 源码标注 Windows NOT SUPPORTED，`--use-angle=d3d12` 非法值回退默认，`SkiaGraphite` 实验性强开有渲染异常风险，`--enable-zero-copy` 收益不可测 | 渲染交还平台默认（测试最充分配置），消除实验组合隐患，厂商分支代码删除 |
| Tokio 线程池动态配置 | 根据 CPU 核心数配置 worker_threads(2-8)/max_blocking_threads(8-64) (app/startup.rs::build_runtime) | 资源利用更合理 |
| CAS 原子配置更新 | `ConfigStore::update` CAS 原子更新 (compare_and_swap 循环) 避免 TOCTOU 竞态 (infra/state/store.rs:35-49) | 配置一致性保证 |
| 流式 SHA256 校验 | 分块流式读取计算 SHA256，64KB buffer | 大文件校验内存占用降低 |
| dual_adapter_executor 泛型化 (B9-7) | `Box<dyn FnOnce>` 改泛型 `F1`/`F2` 静态分发，生产 2 处调用点去 `Box::new`（另有 2 处测试调用）(auth/dual_adapter_executor.rs) | 消除堆分配与虚函数调用 |
| 双适配器在线检测并行化 (B9-10) | `check_any_adapter_online` 串行改 `std::thread::scope` 并行 (commands/login.rs) | 双适配器检测延迟减半 |
| logger shutdown 超时 join (B9-14) | `mpsc` + `recv_timeout(500ms)` 带超时 join 替代固定 `sleep(200ms)` (infra/logger.rs)；main.rs 删除固定 sleep | 避免 logger 线程卡死阻塞退出，同时消除不必要的 200ms 等待 |
| CLIENT_POOL LRU 淘汰 (B9-17) | `client_pool_get` 命中时更新 `Instant`，容量超限 `min_by_key` 剔除最久未访问 (network/client.rs) | 热点连接保活，冷连接及时回收 |
| background_check CAS 合并 (B9-4) | 合并连续 `network.update` 调用，2 处从 2 次 CAS 降为 1 次 (monitor/background_check.rs) | 减少 CAS 循环开销 |
| lifecycle TOCTOU 竞态修复 (B9-8) | 3 处 `auto_exit_deadline` check-then-act 收入同一锁临界区 (infra/lifecycle.rs) | 消除快捷键注册/注销竞态 |
| 质量检测全局节流 (v2.4.0) | ~~`LAST_QUALITY_CHECK_DONE_MS` 全局 60s 最小间隔~~ **已移除 (2026-09-04)**：双定时器叠加的成因随质量驱动者收敛消失，节流反而吞掉 <60s 的自定义定时测试间隔 (monitor/quality_scheduler.rs) | 消除每 15s 全量外网探测的高频重复（原两定时器叠加） |
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
| block_on 统一安全工具 (v2.4.0) | 新增 `infra::async_util::block_on_sync`，protocol/双适配器 6 处迁移 (auth/protocol.rs ×5, dual_adapter_executor.rs ×1) | 防 async worker 线程 block_on panic（前瞻） |
| 客户端池键去 String (v2.4.0) | CLIENT_POOL key 改 `(Option<IpAddr>, u8, u64)` 元组 (network/client.rs) | 热路径零堆分配 |
| MAC 随机化 getrandom (v2.4.0) | generate_random_mac 改 getrandom 填充（低概率失败降级 LCG）(network/dhcp.rs) | 原时间+计数器 LCG 可预测 → 密码学随机 |
| 前端订阅粒度化 (v2.4.0) | App/StatusBar/RightPanel config 全量订阅改最小粒度 selector (App.tsx, StatusBar.tsx, RightPanel.tsx) | 任意字段变化不再级联重渲染外壳与面板 |
| 面板代码分割 (v2.4.0，2026-09-04 修订) | 低频对话框（About/Theme/Onboarding）React.lazy 分包 + 启动预热；**LogPanel 回归静态导入**——NetworkPanel 经 barrel 静态引 SegmentTabs 曾把 LogPanel 连带打进主包使 lazy 失效（vite 警告可查），真 lazy 实测即使 chunk 缓存命中切换仍 ~366ms（React.lazy+Suspense 挂载链），13KB 分包远不值切换延迟；常用面板全部静态导入 | 主包 385KB（vendor 拆分是分包主要收益）；面板切换 100-120ms → 61-68ms，日志面板 366ms → 62ms（生产 preview 实测，无 longtask） |
| 面板切换动画等待链 (2026-09-04) | `createPanelAppleVariants` 退出 0.08s→0.04s；`handlePanelChange` 切换锁 120ms→60ms（锁只需覆盖退出时长，锁内点击仍按设计丢弃） | mode="wait" 每次切换固定多等退出时长；锁过长吞快速连点（"点了没反应"） |
| 面板内容 deferred 渲染 (2026-09-04) | App.tsx 新增 `deferredPanel = useDeferredValue(activePanel)`，面板 switch / AnimatePresence key / 标题+描述 key / slideDirection 全部同步 `deferredPanel`（activePanel 仅保留给 DockNav 高亮与 storage 恢复）。快速连续切换时 React 并发跳过中间面板 mount，重排/栅格化只花在最终面板上，进入动画不再被挂载工作阻塞——vsync 对齐后 120Hz 帧预算仅 8.3ms，此为真机 WebView2 "快速切换短卡顿"的主修 | IAB 生产构建实测：16 连切（120ms 间隔）0 longtask 0 掉帧，max 帧间隔 18.2→12.1ms；逐面板单切与 60ms 极速连点均 0 longtask；单次切换点击→内容提交延迟 10ms（不可感知）。**注意**：新增面板转场相关逻辑时标题/方向/key 必须用 deferredPanel，用 activePanel 会内容与标题错位 |
| panel-content 去 paint containment (2026-09-04) | `PANEL_CONTAINER_STYLE` contain 从 `layout style paint` 改 `layout style`（App.tsx） | paint containment 把后代裁剪到 content box，编辑模式卡片右上角 `-right-1.5` 的删除按钮越出 6px 被裁（hit-test 扫描实测裁剪线=panel-content 右缘 729，按钮右缘 735）；top 方向越界同理。布局/样式隔离保留，微损绘制隔离 |
| 类型检查策略 (2026-09-04) | tsconfig 显式 `types: ["vite/client"]`（node.json 为 `["node"]`）+ 检查命令 `npx tsc --noEmit --incremental`（4.3s 首跑→1.9s 增量，exit 0 零错误）。**禁止跑 `tsc -b`**（tsconfig.node.json 为 composite 项目会 emit 出 vite.config.js/.d.ts 污染产物） | 未设 types 时 TS 自动加载 node_modules 全部 @types 包——工作区根 node_modules 的 @types/yauzl 损坏（二进制文件）导致全量检查必报错 exit 2 |
| 文本输入本地草稿 (v2.4.0) | 用户名/网关/SSID/fixedGateway 改本地 draft + blur 提交；主题色 80ms 节流 (AccountPanel, MonitorPanel, SettingsPanel) | 不再每键写 store + 触发防抖保存 |
| 日志面板渲染优化 (v2.4.0) | 轮询内容未变跳过 setState；叠加窗口可见性门控；单遍解析 (shared/LogPanel.tsx) | 消除四层 useMemo 全量重算与整表重渲染 |
| 事件与动画资源 (v2.4.0) | setStatus 浅比较；usePageIdle interval 化；GSAP/RAF/ripple 清理与 reduced-motion 兜底 (useEventListeners, usePageIdle, AnimatedNumber, button, animated-card, useRipple) | 减少无效 setState 与未清理资源 |
| 动画合成层精细化管理 (v2.5.0) | 移除一次性/瞬态动画的常驻 will-change（card-enter 等）；main.tsx 去全局 force3D；.anim-idle 补 signal-glow-active 暂停 (index.css, main.tsx) | 动画结束即回收合成层，降低持续 GPU 层开销 |
| 设计基础整治 (v2.5.0) | html font-size 15px→16px（对齐 DockNav fallback 修正 dock 偏移）；统一圆角体系（移除按钮 9999px 胶囊化与 w-8 h-8 强转圆形，md=10/lg=12/xl=16）；**移除涟漪动画**（卡片+按钮，删 useRipple.ts）；补全 prefers-reduced-motion；清理 9+ 死代码类；z-index 语义化（DockNav z-50→z-30 低于遮罩，新增 Z_INDEX 表） (index.css, main.tsx, ui-constants.ts, DockNav.tsx) | 视觉层级一致，reduced-motion 全停，减小样式体积与持续 GPU 开销，消除 Dock 与遮罩层级竞态 |
| 动画丢失修复 (v2.5.0) | `LazyMotion features={domAnimation}` → `domMax`（恢复 layout 特性，修复 SegmentTabs 选中背景块 slide 动画）；`.panel-content` `content-visibility: auto`→`visible`（避免含动画元素被跳过渲染合成，修复卡片入场/信号条/数字滚动"数据在但没播"） (main.tsx, index.css) | 恢复面板/子标签切换动画与测试过程动画显示 |
| 子标签切换动画修复 (v2.5.0) | `QualityPanel` 测试详情子标签切换：`TooltipProvider` 从 `m.div(key=activeTab)` 外层移入内层，让 `AnimatePresence mode=wait` 感知到 key 变化从而播进出场动画；`tabContainerVariants` 补全 initial/终态；子元素 `m.div` 补 `custom={tabDirection}` (QualityPanel.tsx) | 恢复测试详情网关/DNS/网站/视频/游戏子标签切换的滑动+淡入过渡 |
| 面板启动预加载 (v2.5.0) | `App.tsx` 对话框 lazy loader 抽出复用（`loadAboutDialog` 等 3 个），新增 `preloadPanels()` 在启动动画播完后经 `requestAnimationFrame` 双帧后直接调用（不等空闲回调）`import()` 预取对话框 chunk (App.tsx) | 切面板时 chunk 已就绪，避免首次切换等待下载导致卡顿 |
| 常用面板静态导入 (v2.5.0) | Account/Network/Monitor/Quality/SpeedTest/Settings 6 个常用面板改静态 `import`（并入主包 285→368KB），切换零等待；About/Theme/Onboarding 对话框保留懒加载 + 启动预取（后续 2026-09-04 LogPanel 亦回归静态导入，现仅 3 个对话框懒加载） (App.tsx) | 消除常用面板切换卡顿，首屏体积仍低于分包前 402KB |
| WebView2 vsync 恢复 (2026-09-03) | `build_browser_args` 移除 `--disable-gpu-vsync`（platform/gpu.rs）；前端 GSAP/Framer/CSS 本就 rAF/vsync 驱动无 JS 上限 | 解除 vsync 后 BeginFrame 不对齐显示器刷新，帧节奏紊乱经 DWM 合并呈撕裂+顿挫（观感"掉帧"）且 GPU 空耗；恢复后管线锁显示器刷新率，120Hz 屏动画最高 120fps |
| 控制台输出 OEM 解码 (2026-09-04) | 新增 `platform/console_output.rs::decode_console_bytes`（严格 UTF-8 优先 → GetOEMCP + MultiByteToWideChar 回退），netsh/ipconfig 输出解析 6 文件 10 处接入（subnet.rs×2、dns_config.rs、adapter_cache.rs、dhcp.rs×4、dns_setup.rs） | GBK 代码页系统（未开系统 UTF-8 的中文 Win，目标用户默认配置）上"配置文件"/"自动升级"关键字匹配与 SSID 解析此前全部失效；本机 UTF-8 模式下开发期无法暴露 |
| ipconfig/netsh 失败如实呈现 (2026-09-04) | dhcp_renew/release 退出码之外按中英错误关键字兜底判定（`ipconfig_output_failed`）；apply_mac_change_via_registry 网卡 enable 失败改 Err（停用状态静默断网）；巡检区 disable/enable 失败记日志 | ipconfig 失败退出码常为 0（假成功）；MAC 重置流程 enable 失败被吞 |
| DoH 响应 chunked 重组 (2026-09-04) | `decode_chunked_body`（RFC 9110 chunk 格式含扩展与畸形拒绝，3 单测），DoH HTTP 解析按 Transfer-Encoding 分支 | 服务器 chunked 响应时 chunk 头会破坏 DNS wire 解析 |
| DoH 注册失败如实上报 (2026-09-04) | netsh dns add encryption 逐条判定，失败取 stderr 记日志并计入 dohFailed；success/message/dohAdded/dohFailed 真实化 | 原实现 let _ 吞错误且 dohFailed 恒空，DoH 未生效仍提示"并启用DoH" |
| 质量总开关联动 (2026-09-04) | start_latency_test 校验 enable_network_quality（关闭则落盘 enable_latency_test=false 经统一路径推前端）；SettingsPanel 关总开关联动 stopLatencyTest | 关总开关后定时测试仍全量外网检测+弹拥堵通知；开关与任务分叉的两个方向都闭环 |
| 落盘统一路径 (2026-09-04) | start_background_check_inner 改走 commands::config_cmd::save_config_to_disk_encrypted（广播 config-changed）；watcher spawn 失败加日志 | 直调 persist 层不广播配置事件，前端/消费方失同步 |
| 全局细滚动条 (2026-09-04) | index.css 滚动条从全局隐藏（display:none!important）改为 6px 半透明细条（thin + webkit），删除 dashboard-editing 特例与 body class effect | 隐藏使"内容可滚动"不可发现（编辑列表溢出一屏卡片看似截断；日志/设置/对话框同类）；6px 半透明与无边框美学兼容，观感经浏览器截图验证 |
| anim-idle 排除 spinner (2026-09-04) | index.css .anim-idle 冻结列表移除 .animate-spin（保留 animate-pulse/signal-glow-active） | 2 秒无输入即全局冻结 loading 指示，用户误读"程序卡死"（登录按钮/检测中 spinner 均中招） |
| 定时测试默认开启 (2026-09-04) | `enable_latency_test` 默认值 false→true（后端 Config::default + 前端 DEFAULT_CONFIG 两处同步）；已存显式 false 的用户配置不受影响（serde 尊重显式值），仅新安装与缺字段配置生效 | 用户预期"装完即有周期质量数据"，默认关闭使质量面板空置（驱动者收敛后周期数据唯一来源就是定时测试循环） |

**已知限制（有意不修，2026-09-04 审计结论）**：① netsh 文本解析依赖中/英关键字，其他系统语言静默失效（目标用户群为中文系统，结构化解析无官方 JSON 接口）；② index.css 的 Tailwind 语义类 !important 劫持（.rounded-xl 等）与全局 border 透明为 v2.5.0 设计系统决策，全局移除会引发不可控视觉回归，维持现状；③ framer-motion 对 Reorder.Item 内联写 touch-action: pan-x（触摸屏垂直滚动让位于拖拽排序，框架行为）；④ v6 栈"空 NameServer 清除"的 API 接受性未经 Win11 实测（失败已降级警告）；⑤ 后端 serde_json::json! 手写返回体与前端类型的字段对齐靠约定，无编译期保证；⑥ auth/session 页面特征误判"已在线"（Portal 页面残留 uid='/v4ip=' 时跳过登录）——判定逻辑需真机实测 Portal 页面后才能改；⑦ 注销占位凭据 drcom/123 为 Dr.COM 惯例，现实 Portal 接受，不为假想故障加真实凭据回退；⑧ config `campusCheckStartHour` alias 在残留旧字段的脏数据下覆盖分钟值；⑨ atomic_write 在 rename 前崩溃的窗口回退旧配置（非丢失）。

**config 健壮性修复 (2026-09-04 补扫)**：Config 加容器级 `#[serde(default)]`（任一字段缺失用 Default 补齐，此前 20+ 字段无 default，缺一个即整体反序列化失败）；加载失败时原文件 `copy` 留档为 `config.json.corrupt-<ts>.bak`（不再静默全量重置无备份）；`save_config` 改先落盘再更新内存（磁盘失败运行态不变）；`get_data_dir` 极端回退加 `campus-login` 子目录。**update 模块**：`compare_versions` 全段比较（`.take(3)` 截断曾使 `2.3.0.1` hotfix 永不提示）；version.json 可选 `asset` 字段声明安装包文件名（改名不再静默断更新）；`extract_checksum` 多 .exe 资产遍历 + BSD 风格 sha256 支持；更新临时目录清理 600s→24h；下载文件名清洗（防路径穿越）；msi 路径含引号拒绝。**auth**：双适配器 `spawn_blocking` panic 由 `unwrap_or(None)` 静默吞掉改为失败 CommandResult + log_error。

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

### 测试与构建基线（2026-09-10 实测）

- **主仓库 `cargo test`**（`tauri-app/src-tauri` 下）：**493 全绿**（lib 246 + bin 同套 246 + repro 1）；改动后不得低于此基线
- **安卓 Rust 验证**：host `cargo check` 在 `android/src-tauri` 基线即失败（mobile-only 插件权限 host 收集不全），靠 `tauri android build` 交叉编译验证
- **前端验证**：`npx tsc --noEmit --incremental`（禁止 `tsc -b`——tsconfig.node.json 是 composite 项目，会 emit 出 vite.config.js/.d.ts 污染文件）
- **安卓构建链**：先手动 `npx vite build`（tauri CLI 不跑 beforeBuildCommand）→ `tauri android build --target aarch64 --apk`；Windows 需开启开发者模式（允许符号链接）
- **安卓产物命名与签名（gradle 内置，2026-09-10）**：`gen/android/app/build.gradle.kts` 配置 release signingConfig（本机 `~/.android/debug.keystore`）+ `applicationVariants` outputFileName——构建直接产出已签名/已对齐的 `Wxxy-CampusLogin_<版本>.apk`（版本号跟随 tauri.conf.json），zipalign/apksigner 后处理整体消失。**CLI 完成报告仍指向旧约定名 `app-universal-release.apk`（预期路径，实际不存在），以输出目录实际文件为准**。签名证书一经发布不可更换（换=用户卸载重装）。一键链 `pwsh android/build-apk.ps1`（含本机 JDK 路径，gitignore 不入库）

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

发布新版本时（以 v2.3.0 为例）：

1. **编辑唯一权威源** — 修改 `tauri-app/src-tauri/tauri.conf.json` 的 `"version"` 字段为 `"2.3.0"`
2. **手动同步 Cargo.toml** — 修改 `tauri-app/src-tauri/Cargo.toml` 的 `version` 字段为 `"2.3.0"`（cargo 强制要求）
3. **同步发布标记** — 修改仓库根 `version.json` 的 `"version"` 为 `"v2.3.0"`（带 v 前缀，是 GitHub release tag 的格式）
4. **同步前端 package.json** — 两个 `package.json` 的 `"version"` 字段（npm 规范要求，无 v 前缀）
5. **同步前端常量** — `tauri-app/frontend/src/shared/ui-constants.ts` 的 `APP_VERSION`（保持当前架构，不改为环境变量注入）
6. **同步静态预览** — `tauri-app/frontend/about-preview.html` 的 `app-version` 和 `status-version` 两个 div（**注意**：此处带 `v` 前缀，如 `v2.3.0`；v2.3.0 升级时曾漏同步此文件，2026-09-05 已修复）
7. **同步徽章** — `README.md` 的 `version-2.3.0` 徽章
8. **同步文档** — `CODE_WIKI.md` 顶部版本号 + 底部元信息

> ⚠️ **Cargo.lock 中的 version**：由 cargo 自动更新，下次 `cargo build` 时自动重写。

> ⚠️ **版本号提交与 Release 发布必须同流程完成**（2026-09-03 教训，v2.3.0 事故）：`version.json` 一旦推送到 main 而对应 tag 的 Release 尚未发布，所有旧版本用户会收到更新通知但下载必然 404。现版本后端已加 HEAD 探测兜底（资产 404 则本轮不提示更新），但仍应把"改 version.json"与"发布 Release"绑在同一次操作里。

> ⚠️ **Release 资产发布检查清单**（2026-09-03 新增，2026-09-04 更新）：
> 1. 上传 `Wxxy-CampusLogin_{ver}_x64-setup.exe`（文件名与硬编码拼接一致；若改名，在 `version.json` 加 `"asset": "<完整文件名>"` 覆盖默认命名，2026-09-04 起支持）
> 2. 同时上传构建产物目录中的 `{安装包名}.sha256`（`build.ps1` 第 [5/5] 步已自动生成）——缺失时应用内更新校验全 4xx，默认拒绝安装且用户无法自救
> 3. `version.json` 可选填 `notes` 字段（字符串，Markdown 列表），将显示为应用内更新日志（release_notes）
> 4. 版本号支持任意段数（`2.3.0.1` hotfix 可正确提示升级，2026-09-04 修复 `.take(3)` 截断）

> ⚠️ **升级检查清单**：建议在发布前对照以下 5 个**必须保持一致**的位置：
> 1. `tauri-app/src-tauri/tauri.conf.json` → `"version": "2.3.0"`
> 2. `tauri-app/src-tauri/Cargo.toml` → `version = "2.3.0"`
> 3. `tauri-app/frontend/src/shared/ui-constants.ts` → `APP_VERSION = '2.3.0'`
> 4. `tauri-app/package.json` + `tauri-app/frontend/package.json` → `"version": "2.3.0"`
> 5. 根 `version.json` → `"version": "v2.3.0"`（带 v 是发布 tag 格式）

### 安卓端 identifier（包名）

- **两端 identifier 相互独立**：桌面 `tauri-app/src-tauri/tauri.conf.json`（`com.campus.login`）、安卓 `android/src-tauri/tauri.conf.json`（`com.campuslogin.client`，2026-09-10 由 `com.campuslogin.app` 迁移）。安卓 identifier 直接决定 `gen/android/app/build.gradle.kts` 的 `namespace`/`applicationId` 与 MainActivity 包路径——改 identifier 必须同步这三处并 `git mv` Kotlin 目录；**改包名=卸载重装**（AndroidKeyStore 密钥按包名隔离，密码密文作废）
- `.app` 结尾的 identifier 会触发 tauri-cli 的 macOS bundle 冲突警告（纯 lint，无 macOS 目标也无碍，迁移后已消除）
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

> v2.2.9 重构清理：`vite.config.ts` 已移除原 `__APP_VERSION__` 注入逻辑（不再 `import tauriConf`、不再 `define` 注入），前端版本号唯一来源为 `ui-constants.ts` 的 `APP_VERSION` 硬编码常量。

> 注：`ui-constants.ts` 的 `APP_VERSION` 保留硬编码是为了在非 Tauri 环境（如纯前端 Storybook / 单元测试 mock）下也能取到合理默认值。**升级时仅需同步 `ui-constants.ts` 一处**。

### 版本号格式约定

- **semver 格式（不带 v）**：`Cargo.toml` / `tauri.conf.json` / `package.json` × 2 / `ui-constants.ts` → `2.3.0`
- **发布 tag 格式（带 v）**：`version.json` / 后端日志（`v{}`）/ `about-preview.html` 的 `app-version` 和 `status-version` div（`v2.3.0`）/ README 徽章（`version-2.3.0` 不带 v，但后端启动日志带 v）

---

*文档版本: v2.3.4 | 基于代码版本: CampusLogin v2.3.4 | 更新日期: 2026-09-10 | 同步 v2.3.4 全端版本号（Windows + 安卓）*

