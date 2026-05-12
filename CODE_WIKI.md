# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.1.5 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
> **目标平台**: Windows (x64)
> **通信方式**: Tauri IPC (`invoke` / `listen`)

---

## 一、项目概览

CampusLogin 是一款校园网自动登录助手桌面应用，面向无锡学院校园网认证系统（锐捷 ePortal），提供一键登录、自动重连、网络质量监测、多账号管理等功能。

### 核心特性

| 特性 | 说明 |
|------|------|
| 一键登录 | 自动检测适配器、DHCP续租、智能重试(指数退避+随机抖动) |
| 自动重连 | 后台巡检断线检测，最多3次自动重连 |
| 网络质量检测 | 网关/DNS/DoH/HTTPS/游戏服务器延迟并发测试 |
| 多账号管理 | DPAPI 加密存储、快速切换 |
| 双适配器支持 | 有线 + 无线同时管理 |
| 系统托盘 | 最小化到托盘后台运行，支持托盘快速登录 |
| 开机自启 | 注册表写入 / Tauri 插件 |
| 自动退出 | 登录成功后倒计时退出，快捷键取消(Ctrl+Shift+C) |
| 主题系统 | 6种预设主题 + 自定义主题色 + 深浅模式 |

---

## 二、项目目录结构

```
Wxxy-CampusLogin/
├── assets/                          # 截图等资源
├── tauri-app/
│   ├── package.json                 # 根层依赖
│   ├── build.ps1                    # Windows 构建脚本
│   ├── frontend/                    # React 前端
│   │   ├── package.json             # 前端依赖 (含 zustand ^5.0.0)
│   │   ├── vite.config.ts           # Vite 构建配置
│   │   ├── tailwind.config.js       # Tailwind CSS 配置
│   │   ├── tsconfig.json            # TypeScript 配置
│   │   ├── index.html               # HTML 入口
│   │   └── src/
│   │       ├── main.tsx             # React 入口
│   │       ├── App.tsx              # 根组件
│   │       ├── index.css            # 全局样式
│   │       ├── constants/
│   │       │   └── index.ts         # 常量 (ISP/导航/主题/质量等级)
│   │       ├── hooks/
│   │       │   ├── useAppStore.ts   # 统一导出 (AppStoreProvider/useAppStore/useAppInit)
│   │       │   ├── AppStoreContext.tsx  # ★ 单一状态管理 Provider
│   │       │   ├── useIpc.ts        # ★ Tauri IPC 封装
│   │       │   ├── useAppInit.ts    # ★ 初始化逻辑 + 事件监听
│   │       │   ├── useLogToast.ts   # 日志 & Toast 状态
│   │       │   └── useThemeStore.ts # 主题状态管理
│   │       ├── types/
│   │       │   └── index.ts        # TypeScript 类型定义
│   │       ├── lib/
│   │       │   ├── utils.ts        # 工具函数 (safeStorage等)
│   │       │   ├── color.ts        # HEX→HSL 颜色转换
│   │       │   ├── latency.ts      # 延迟等级/颜色计算
│   │       │   └── animations.ts    # Framer Motion 动画变体
│   │       └── components/
│   │           ├── dialogs/        # 对话框 (关于/主题/确认)
│   │           ├── layout/         # 布局组件 (标题栏/状态栏/导航/日志/Toast)
│   │           ├── panels/         # 面板组件 (总览/账号/网络/监控/质量/设置/速度测试/日志)
│   │           ├── shared/         # 共享组件 (延迟组件/刷新按钮/分段Tab)
│   │           ├── ui/             # 基础 UI 组件 (shadcn/ui)
│   │           └── ErrorBoundary.tsx
│   └── src-tauri/                   # Rust 后端
│       ├── Cargo.toml               # Rust 依赖 (含 tokio-util, sha2 新增)
│       ├── tauri.conf.json          # ★ Tauri 应用配置
│       ├── capabilities/
│       │   └── default.json         # Tauri 权限声明
│       ├── icons/                   # 应用图标
│       └── src/
│           ├── main.rs              # ★ 应用入口 (动态线程池)
│           ├── lib.rs                # 库模块声明 (Tauri构建所需)
│           ├── config.rs            # ★ 配置模型与路径 (Portal URL 无端口默认值)
│           ├── network/             # ★ 网络模块
│           │   ├── mod.rs           # 重导出 (含 clear_portal_cache)
│           │   ├── cache.rs         # 缓存基础设施 (NET_CACHE/HTTP客户端/TLS 1.3+回退)
│           │   ├── adapter.rs       # 适配器查询/Win32 API/DHCP/网关
│           │   ├── portal.rs        # Portal认证状态检测
│           │   ├── login_request.rs # 登录请求/重试/响应解析
│           │   └── quality.rs       # 网络质量并发延迟测试
│           ├── crypto_utils.rs      # ★ 加密工具 (Windows DPAPI)
│           ├── http_timing.rs       # HTTP计时 (DoH/DNS/TLS/TCP)
│           ├── logger.rs            # ★ 日志系统 (文件+通道+调试模式切换)
│           └── commands/            # Tauri 命令 (v2.1.5 模块化重构)
│               ├── mod.rs           # ★ 命令模块声明与架构文档 (28行依赖链说明)
│               ├── state.rs         # ★ 全局状态 (TaskLock/分层AppState/82行设计文档)
│               ├── config_cmd.rs    # 配置相关命令
│               ├── login.rs         # ★ 登录命令
│               ├── background.rs    # ★ 后台检测调度器 (从上帝类重构为纯调度)
│               ├── auto_login.rs    # 🆕 自动登录/断线重连 (229行独立模块)
│               ├── auto_exit.rs     # 🆕 自动退出控制 (~80行独立模块)
│               ├── latency.rs       # 🆕 网络质量通知+延迟测试循环 (~100行)
│               ├── adapter_watch.rs  # 🆕 适配器状态监控 (~100行, 15s间隔)
│               ├── network_cmd.rs   # 网络相关命令
│               ├── account.rs       # 多账号管理命令
│               ├── system.rs        # 系统功能命令
│               └── updater.rs       # 更新检查/下载/安装
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
│  Framer Motion 动画 | shadcn/ui 组件库 | zustand 状态  │
├─────────────────────────────────────────────────────┤
│              单一状态管理层 (State Layer)             │
│  AppStoreProvider — 集中管理所有前端状态              │
│  useIpc Hook — Tauri IPC 通信封装 (单一实例)          │
├─────────────────────────────────────────────────────┤
│                IPC 通信层 (Bridge Layer)              │
│  Tauri invoke (请求-响应) | Tauri listen (事件推送)   │
│  前端 ←→ Rust 后端                                   │
├─────────────────────────────────────────────────────┤
│                 业务逻辑层 (Logic Layer)              │
│  ┌─────────────────────────────────────────────────┐ │
│  │  background.rs (调度器)                         │ │
│  │    ├─→ auto_login.rs   (自动登录/断线重连)      │ │
│  │    ├─→ auto_exit.rs    (自动退出倒计时)          │ │
│  │    ├─→ latency.rs      (质量通知/延迟循环)       │ │
│  │    └─→ adapter_watch.rs (适配器监控)            │ │
│  ├─ login.rs — 登录认证                              │ │
│  ├─ network/ — 网络检测/延迟测试 (5个子模块)         │ │
│  ├─ account.rs — 多账号管理                          │ │
│  ├─ crypto_utils.rs — Windows DPAPI加密              │ │
│  └─ config.rs — 配置管理                            │ │
├─────────────────────────────────────────────────────┤
│                 系统交互层 (System Layer)             │
│  Win32 API — 适配器查询(GetAdaptersAddresses)        │
│  cmd.exe / netsh — DHCP续租/适配器启用                │
│  reqwest — HTTP 请求 (TLS 1.3强制+1.2回退)          │
│  tokio — 异步运行时 (动态线程池/CPU自适应)            │
│  Windows Registry — 开机自启                         │
└─────────────────────────────────────────────────────┘
```

### 3.2 Commands 模块依赖关系 (v2.1.5)

```
// [架构说明] commands 模块间耦合关系
//
//  依赖链（箭头表示 "调用/依赖"）：
//
//   background ──→ auto_login ──→ auto_exit
//       │              │              │
//       │              └──→ system (emit_notification)
//       │
//       ├──→ latency ──→ system (emit_notification)
//       │
//       └──→ auto_exit
//
//   login ──→ system (append_login_history)
//
//   auto_exit ──→ system (emit_notification)
//
//   adapter_watch ─ (无跨模块调用，仅依赖 state + network)
//
// 耦合问题：
//   1. background 是核心调度器，同时依赖 auto_login/auto_exit/latency 三个子模块，
//      任何子模块的接口变更都会影响 background
//   2. auto_login 同时调用 auto_exit 和 system，形成 background→auto_login→auto_exit
//      的三层调用链，中间层的变更会向上传播
//   3. emit_notification 被 auto_login/auto_exit/latency 三处调用，是事实上的共享工具，
//      但定义在 system 模块中，语义上不够清晰
//
// 所有模块通过 AppState 共享状态（见 state.rs），状态一致性依赖原子操作和 ArcSwap 保证
```

### 3.3 数据流

```
用户操作 → React组件 → useAppStore → useIpc.invoke()
                                         ↓
                                    Tauri IPC
                                         ↓
                              #[tauri::command] Rust函数
                                         ↓
                              AppState (TaskLock + NetworkStatus + ExitState)
                                         ↓
                              Win32 API / HTTP / 文件系统
                                         ↓
                              结果返回 / 事件推送 (emit)
                                         ↓
                              useIpc.listen() → useAppStore → UI更新
```

---

## 四、后端模块详解 (Rust)

### 4.1 应用入口 — `main.rs`

**职责**: 应用初始化、动态线程池配置、Tauri 插件注册、窗口/托盘/事件处理

**关键流程**:

1. **动态线程池配置** (v2.1.5): 根据 CPU 核心数动态分配工作线程数（至少4个）
   ```rust
   let worker_threads = std::thread::available_parallelism()
       .map(|n| n.get()).unwrap_or(4).max(4);
   let runtime = tokio::runtime::Builder::new_multi_thread()
       .worker_threads(worker_threads)
       .enable_all().build();
   ```
2. **Tauri 插件注册**:
   - `tauri-plugin-shell` — 执行外部命令
   - `tauri-plugin-notification` — 系统通知
   - `tauri-plugin-autostart` — 开机自启
   - `tauri-plugin-global-shortcut` — 全局快捷键 (Ctrl+Shift+C 取消自动退出)
   - `tauri-plugin-single-instance` — 单实例锁
3. **Setup 钩子**:
   - 创建数据目录
   - 加载配置 (含密码DPAPI解密)
   - 根据 `--autostart` 参数和 `hiddenStart` 配置决定是否显示窗口
   - 创建系统托盘
   - **🆕 并行初始化**: CryptoKeys + 适配器缓存预热 (`get_adapters_cached()`)
   - 启动适配器监控和启动任务 (通过 `run_startup_tasks`)
4. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭，退出时使用 `force_release()` 清理任务标志
5. **命令注册**: 30个 `#[tauri::command]` 函数（含新增的 `set_debug_mode`/`get_debug_mode`）

### 4.2 全局状态 — `commands/state.rs` (v2.1.5 重大重构)

#### TaskLock 并发原语抽象 (🆕)

```rust
/// TaskLock: 封装 AtomicBool 的互斥获取 + 自动释放模式
/// 替代散落在各处的 compare_exchange + atomic_guard 宏组合
/// 保证：try_acquire 成功后，Guard drop 时自动释放，不会遗漏
pub struct TaskLock { flag: AtomicBool }

pub struct TaskGuard<'a> { lock: &'a TaskLock }

impl TaskLock {
    pub fn new() -> Self { Self { flag: AtomicBool::new(false) } }
    
    /// CAS操作获取锁，成功返回Guard（RAII模式）
    pub fn try_acquire(&self) -> Option<TaskGuard<'_>> { ... }
    
    /// 查询当前状态
    pub fn is_active(&self) -> bool { ... }
    
    /// 强制释放（防死锁，用于退出清理）
    pub fn force_release(&self) { ... }
    
    /// Swap操作获取（返回旧值）
    pub fn swap_acquire(&self) -> bool { ... }
}

/// Guard drop 时自动释放锁（RAII）
impl Drop for TaskGuard<'_> {
    fn drop(&mut self) { self.lock.flag.store(false, Ordering::Release); }
}
```

#### 分层 AppState 结构体 (🆕)

```rust
// 🆕 退出状态子结构
pub struct ExitState {
    pub is_quitting: std::sync::Arc<AtomicBool>,
    pub auto_exit_deadline: Mutex<Option<Instant>>,
    pub auto_exit_cancelled: AtomicBool,
}
impl ExitState {
    pub fn deadline(&self) -> Option<Instant> { ... }
    pub fn set_deadline(&self, deadline: Option<Instant>) { ... }
}

// 🆕 通知状态子结构
pub struct NotificationState {
    pub last_disabled_notification_epoch_ms: AtomicU64,
}
impl NotificationState {
    pub fn disabled_notification_elapsed(&self) -> Option<Duration> { ... }
}

// 🆕 任务运行标志：使用 TaskLock 封装
pub struct TaskFlags {
    pub background_running: TaskLock,
    pub latency_running: TaskLock,
    // 🆕 使用 CancellationToken 替代 AtomicU32 计数器
    pub latency_cancel: ArcSwap<tokio_util::sync::CancellationToken>,
    pub is_checking: TaskLock,
    pub is_logging_in: TaskLock,
    pub is_quality_checking: TaskLock,
    pub login_timestamps: Mutex<Vec<Instant>>,
}
impl TaskFlags {
    // 🆕 将 check_login_rate_limit 移入 TaskFlags
    pub fn check_login_rate_limit(&self) -> Result<(), String> { ... }
}

// 🆕 网络状态（新增字段）
pub struct NetworkStatus {
    pub server_available: AtomicBool,
    pub was_online: AtomicBool,
    pub has_logged_online: AtomicBool,
    pub background_check_count: AtomicU64,
    pub disconnect_reconnect_count: AtomicU32,
    pub consecutive_check_failures: AtomicU32,     // 🆕 连续失败防抖计数
    pub last_auto_login_attempt: ArcSwap<std::time::Instant>,  // 🆕 冷却时间戳
    pub cached_online_status: ArcSwap<Option<serde_json::Value>>,
    pub last_network_quality: ArcSwap<Option<String>>,
}

// 🆕 最终 AppState (分层结构)
pub struct AppState {
    pub config: ArcSwap<Config>,
    pub tasks: TaskFlags,
    pub network: NetworkStatus,
    pub exit: ExitState,              // 🆕 提取
    pub notification: NotificationState,  // 🆕 提取
}
```

**关键常量** (v2.1.5 变更):

| 常量 | v2.1.4 | v2.1.5 | 说明 |
|------|--------|--------|------|
| `AUTO_EXIT_DELAY_MS` | 5000 | 10000 | 自动退出倒计时 (毫秒) |
| `CANCEL_EXIT_SHORTCUT` | - | `"CommandOrControl+Shift+C"` | 取消快捷键 |
| `LOGIN_RATE_LIMIT_SECS` | 3 | **10** | 登录频率限制时间窗 (秒) |
| `LOGIN_RATE_LIMIT_MAX` | 3 | **5** | 时间窗内最大登录次数 |

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `validate_config()` | 校验配置字段 (枚举值/正则/URL)，含 Portal URL 迁移逻辑 |
| `validate_account_name()` | 校验账号名 (1-32字符, 字母数字下划线中文连字符) |

**`atomic_guard!` 宏** (v2.1.5 改造):
```rust
// 保留 atomic_guard 宏以兼容现有代码，新代码应使用 TaskLock
macro_rules! atomic_guard {
    ($name:ident, $field:ident) => {
        struct $name<'a>(&'a crate::commands::state::AppState);
        impl Drop for $name<'_> {
            fn drop(&mut self) {
                self.0.tasks.$field.force_release();  // 🔄 改用 force_release
            }
        }
    };
}
```

### 4.3 配置管理 — `config.rs`

**`Config` 结构体** (26个字段):

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `user` | String | `""` | 学号 |
| `password` | String | `""` | 密码 (内存中明文, 磁盘上DPAPI加密) |
| `operator` | String | `""` | 运营商后缀 (`""/@telecom/@unicom/@cmcc`) |
| `adapter1` | String | `"自动检测"` | 主适配器名称 |
| `adapter2` | String | `""` | 副适配器名称 |
| `dualAdapter` | bool | false | 双适配器模式 |
| `autoLoginOnStart` | bool | true | 启动时自动登录 |
| `autoExitAfterLogin` | bool | true | 登录后自动退出 |
| `minimizeToTray` | bool | false | 关闭时最小化到托盘 |
| `hiddenStart` | bool | true | 静默启动 |
| `autoLaunch` | bool | true | 开机自启 |
| `enableBackgroundCheck` | bool | true | 启用后台检测 |
| `backgroundCheckInterval` | u64 | 60000 | 后台检测间隔 (ms) |
| `autoLoginOnPreparation` | bool | true | 登录准备模式 |
| `autoExitOnOnline` | bool | true | 检测到在线后自动退出 |
| `themeMode` | String | `"dark"` | 主题模式 |
| `enableNotification` | bool | true | 启用通知 |
| `activeAccount` | String | `""` | 当前活跃账号名 |
| `enableLatencyTest` | bool | false | 启用延迟测试 |
| `latencyTestInterval` | u64 | 30000 | 延迟测试间隔 (ms) |
| `customThemeColor` | String | `"#6366f1"` | 自定义主题颜色 |
| `defaultPanel` | String | `""` | 默认面板 |
| `enableNetworkQuality` | bool | true | 启用网络质量检测 |
| `skipTtfbInLatency` | bool | true | 延迟测试跳过TTFB |
| `skipContentInLatency` | bool | true | 延迟测试跳过内容下载 |
| `portalUrl` | String | **`"http://10.1.99.100"`** | 🆕 Portal地址 (移除 :801 端口) |
| `fixedGateway` | String | `""` | 固定网关IP |

**🆕 Portal URL 迁移逻辑** (validate_config 内):
```rust
// 旧配置自动迁移
if config.portal_url == "http://10.1.99.100:801" {
    config.portal_url = "http://10.1.99.100".to_string();
}
// 空值使用新的无端口默认值
if config.portal_url.is_empty() {
    config.portal_url = "http://10.1.99.100".to_string();
}
```

### 4.4 加密工具 — `crypto_utils.rs`

同 v2.1.4，无变化。

### 4.5 网络模块 — `network/`

#### 4.5.1 缓存基础设施 — `cache.rs` (v2.1.5 增强)

**`NetworkCache` 结构体** (全局单例 `NET_CACHE`):

```rust
struct NetworkCache {
    pub adapter: ArcSwap<Option<AdapterCache>>,      // 适配器缓存 (TTL=15s)
    pub gateway: ArcSwap<Option<GatewayCacheEntry>>, // 网关缓存
    pub portal: ArcSwap<Option<PortalCacheEntry>>,   // Portal状态缓存
    pub portal_url: ArcSwap<String>,                 // Portal URL
}
```

**🆕 新增函数**:

| 函数 | 说明 |
|------|------|
| `clear_adapter_cache_only()` | 仅清除适配器缓存 |
| `clear_portal_cache()` | 仅清除 Portal 状态缓存 |
| `create_safe_http_client(timeout, local_addr)` | 创建 HTTP 客户端 (**TLS 1.3 强制 + TLS 1.2 回退**) |

**TLS 安全增强**:
```rust
fn build_http_client(...) -> Result<Client, String> {
    let mut builder = Client::builder()
        .min_tls_version(Version::TLS_1_3)  // 🆕 强制 TLS 1.3
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(5))
        ...;
    
    match builder.build() {
        Ok(c) => Ok(c),
        Err(_) => {
            // 🆕 回退到 TLS 1.2 兼容旧服务器
            let mut fallback = Client::builder()
                .min_tls_version(Version::TLS_1_2)
                ...;
            fallback.build()
        }
    }
}
```

**mod.rs 导出更新**:
```rust
pub use cache::{
    update_portal_url, clear_adapter_cache, clear_portal_cache,  // 🆕 clear_portal_cache
};
```

#### 4.5.2 ~ 4.5.5 其他网络子模块

同 v2.1.4，无重大变化。

### 4.6 登录模块 — `commands/login.rs`

同 v2.1.4，核心流程不变。注意登录频率限制已放宽至 10秒5次。

### 4.7 后台巡检调度器 — `commands/background.rs` (v2.1.5 重构)

**职责变更**: 从包含所有业务逻辑的"上帝类"变为**纯调度器**

**导入结构** (第9-12行):
```rust
use super::state::{AppState, CommandResult, atomic_guard};
use super::auto_exit::start_auto_exit;                                    // 🆕 从独立模块导入
use super::auto_login::{try_auto_login_on_preparation, try_disconnect_reconnect, run_auto_login_on_start};  // 🆕
use super::latency::{notify_network_quality_change, spawn_latency_test_loop};  // 🆕
```

**关键函数**:

| 函数 | 说明 |
|------|------|
| `run_background_check_blocking()` | ★ 同步后台检测核心逻辑 (精简为调度) |
| `run_background_check()` | 异步包装 + 质量检测触发 |
| `start_background_check_inner()` | 启动后台检测循环 (使用 `force_release()`) |
| `stop_background_check()` | 停止后台检测 (使用 `force_release()`) |
| `trigger_background_check()` | 手动触发一次检测 |
| `get_background_status()` | 获取后台状态 (使用 `is_active()`) |
| `run_startup_tasks()` | ★ 启动任务编排 (调用 `run_auto_login_on_start`) |

**🆕 后台检测流程增强**:
1. 防重入检查 (`tasks.is_checking.swap_acquire()`)
2. 获取适配器列表 (含 user_account/user_password 构建)
3. 并行门户检测 (双适配器)
   - **🆕 错误详情记录**: Portal 异常不再静默吞掉，记录具体错误并通过 `login-log` 事件发送前端
4. **🆕 连续失败防抖**: 单次失败不立即切状态，需连续2次才更新
5. 发送 `background-check-result` 事件 (🆕 含 `adapter1Name`/`adapter2Name`)
6. **🆕 调用独立模块**:
   - `try_auto_login_on_preparation(...)` — 自动登录逻辑
   - `try_disconnect_reconnect(...)` — 断线重连逻辑
7. **🆕 离线通知**: 状态从在线变离线时发送系统通知
8. 首次在线 → start_auto_exit()

### 4.8 🆕 自动登录模块 — `commands/auto_login.rs`

**文件**: 229行独立模块，从 background.rs 提取

**公开函数**:

| 函数 | 说明 |
|------|------|
| `try_auto_login_on_preparation(app, state, login_available, online, config)` | 准备阶段自动登录 (30秒冷却) |
| `try_disconnect_reconnect(app, state, online, secondary_online, a1, ..., config)` | 断线重连 (最多3次 + 间隔提醒) |
| `run_auto_login_on_start(app_handle)` | 启动时自动登录 (1.5s延迟 + Portal预检) |

**常量**:

| 常量 | 值 | 说明 |
|------|----|------|
| `MAX_DISCONNECT_RECONNECT` | 3 | 最大断线重连次数 |
| `RECONNECT_REMINDER_INTERVAL` | 10 | 重连失败后的提醒间隔 (次) |
| `AUTO_LOGIN_COOLDOWN_SECS` | 30 | 自动登录冷却期 (秒) |

**耦合点**: 调用 `auto_exit::start_auto_exit` 和 `system::emit_notification`

### 4.9 🆕 自动退出模块 — `commands/auto_exit.rs`

**文件**: ~80行独立模块，从 background.rs 提取

**公开函数**:

| 函数 | 说明 |
|------|------|
| `start_auto_exit(app_handle, state)` | 启动自动退出倒计时 + 快捷键注册 + 通知 |

**流程**:
1. 检查是否已有 deadline（避免重复启动）
2. 设置 deadline = now + AUTO_EXIT_DELAY_MS
3. emit `auto-exit-countdown` 事件（含 delay 和 shortcut）
4. 发送系统通知
5. 注册 CANCEL_EXIT_SHORTCUT 全局快捷键

### 4.10 🆕 延迟测试模块 — `commands/latency.rs`

**文件**: ~100行独立模块，从 background.rs 提取

**公开函数**:

| 函数 | 说明 |
|------|------|
| `notify_network_quality_change(app, state, quality, enable_notification)` | 网络质量变化通知 (bad/good 级别切换) |
| `spawn_latency_test_loop(app_handle, interval)` | 启动延迟测试循环 (使用 CancellationToken) |

### 4.11 🆕 适配器监控模块 — `commands/adapter_watch.rs`

**文件**: ~100行独立模块，从 background.rs 提取

**公开函数**:

| 函数 | 说明 |
|------|------|
| `start_adapter_watch(app_handle)` | 启动适配器状态监控循环 |

**参数**:

| 常量 | 值 | 说明 |
|------|----|------|
| `ADAPTER_WATCH_INTERVAL` | 15000ms | 监控间隔 (15秒) |

**流程**:
1. 无限循环，每15秒 tick
2. 检查 `is_quitting` 标志
3. 清理过期 DNS 缓存
4. `spawn_blocking` 调用 `get_adapters_force()`
5. 对比上次状态，变化则 emit `adapters-changed` 事件

### 4.12 其他命令模块

**config_cmd.rs** / **account.rs** / **network_cmd.rs** / **system.rs** / **updater.rs** — 同 v2.1.4，无重大变化。

注意 `system.rs` 中新增了 `set_debug_mode` 和 `get_debug_mode` 命令。

---

## 五、前端模块详解 (React/TypeScript)

### 5.1 ~ 5.7 同 v2.1.4

前端架构基本保持一致，主要变化：

- **🆕 新增 zustand ^5.0.0** 依赖（package.json），可能用于替代部分 AppStoreContext 的状态管理
- 其余 hooks/components/lib 保持不变

---

## 六、IPC 通信完整清单

### 6.1 请求-响应命令 (v2.1.5: 30个)

同 v2.1.4，新增：

| 命令名 | 说明 |
|--------|------|
| `set_debug_mode` | 🆕 动态设置调试日志模式 |
| `get_debug_mode` | 🆕 获取当前调试模式状态 |

### 6.2 事件推送 (v2.1.5)

同 v2.1.4，`background-check-result` 事件新增字段：

```typescript
{
    // ...原有字段...
    "adapter1Name": string,    // 🆕 主适配器名称
    "adapter2Name": string,    // 🆕 副适配器名称
}
```

---

## 七、依赖关系

### 7.1 Rust 依赖 (Cargo.toml) (v2.1.5)

| 依赖 | 版本 | 用途 | 变更 |
|------|------|------|------|
| `tauri` | 2 | 应用框架 | - |
| `tauri-plugin-*` | 2 | shell/notification/autostart/global-shortcut/single-instance | - |
| `serde` / `serde_json` | 1 | 序列化 | - |
| `tokio` | 1 (full) | 异步运行时 | - |
| **`tokio-util`** | **0.7 (rt)** | **🆕 CancellationToken (延迟测试取消)** | **新增** |
| `reqwest` | 0.12 (json, blocking, **charset**) | HTTP客户端 | **+charset** |
| `dashmap` | 6 | 并发HashMap | - |
| `parking_lot` | 0.12 | 高性能同步原语 | - |
| `arc-swap` | 1 | 原子引用交换 | - |
| `windows` | 0.58 | Win32 API | - |
| `surge-ping` | 0.8 | ICMP ping | - |
| `tokio-rustls` | 0.26 | TLS连接 | - |
| **`sha2`** | **0.10** | **🆕 SHA-2 哈希 (数据校验)** | **新增** |
| `urlencoding` | 2 | URL编码 | - |
| `regex` | 1 | 正则表达式 | - |
| `url` | 2 | URL解析验证 | - |
| `dirs` | 6 | 数据目录 | - |
| `rand` | 0.8 | 随机数 | - |
| `lazy_static` | 1.5 | 静态初始化 | - |
| `base64` | 0.22 | Base64编解码 | - |
| `chrono` | 0.4 | 时间处理 | - |
| `open` | 5 | 打开外部链接 | - |
| `thiserror` | 2 | 错误类型 | - |

### 7.2 前端依赖 (v2.1.5)

| 依赖 | 版本 | 用途 | 变更 |
|------|------|------|------|
| `react` / `react-dom` | 19 | UI框架 | - |
| `@tauri-apps/api` | ^2 | Tauri前端API | - |
| `framer-motion` | ^12 | 动画 | - |
| `lucide-react` | ^0.446 | 图标 | - |
| **`zustand`** | **^5.0.0** | **🆕 轻量级状态管理** | **新增** |
| `tailwindcss` | ^3.4 | CSS框架 | - |
| `vite` | ^6 | 构建 | - |
| `typescript` | ^5.5 | 类型系统 | - |
| Radix UI primitives | 各版本 | 无障碍UI | - |

### 7.3 依赖关系图 (v2.1.5)

```
main.rs (动态线程池 + 缓存预热)
  ├── lib.rs (Tauri库入口)
  └── commands/mod.rs (28行架构文档)
        ├── state.rs ← config.rs, arc-swap, dashmap, parking_lot, tokio-util
        │   [TaskLock/TaskGuard 抽象]
        │   [ExitState / NotificationState 分层]
        ├── config_cmd.rs ← config.rs, crypto_utils.rs, state.rs
        ├── login.rs ← network/*, state.rs, system.rs
        ├── background.rs (调度器)
        │   ├── auto_login.rs ← state.rs, login.rs, system.rs, auto_exit.rs
        │   ├── auto_exit.rs ← state.rs, system.rs
        │   ├── latency.rs ← state.rs, network/*, system.rs
        │   └── (直接调用上述模块)
        ├── auto_login.rs 🆕
        ├── auto_exit.rs 🆕
        ├── latency.rs 🆕
        ├── adapter_watch.rs 🆕
        ├── network_cmd.rs ← network/*, state.rs
        ├── account.rs ← config.rs, crypto_utils.rs, state.rs, config_cmd.rs
        ├── system.rs ← config.rs, network/*, state.rs, winreg
        └── updater.rs ← reqwest, url

network/
  ├── mod.rs (重导出 + clear_portal_cache)
  ├── cache.rs ← arc-swap, lazy_static, dashmap, reqwest [TLS 1.3]
  ├── adapter.rs ← cache.rs, windows, regex
  ├── portal.rs ← cache.rs, reqwest, url
  ├── login_request.rs ← cache.rs, reqwest, urlencoding, regex, rand
  └── quality.rs ← adapter.rs, cache.rs, surge-ping, tokio-rustls

App.tsx
  └── AppStoreProvider (AppStoreContext.tsx) [可能引入 zustand]
        ├── useIpc.ts ← @tauri-apps/api
        ├── useLogToast.ts
        ├── useThemeStore.ts
        └── useAppInit.ts
```

---

## 八、安全体系

同 v2.1.4，安全基线保持一致。v2.1.5 新增：

| 措施 | 实现 |
|------|------|
| **TLS 1.3 强制** | HTTP客户端默认 TLS 1.3，回退 TLS 1.2 |
| **panic=abort** | 编译选项减小二进制体积，避免信息泄露 |
| **force_release 防死锁** | 退出场景强制释放任务锁 |

---

## 九、性能优化 (v2.1.5)

| 优化项 | v2.1.4 | v2.1.5 | 效果 |
|--------|--------|--------|------|
| codegen-units | 16 | **1** | LTO跨单元优化 +2~5% 性能 |
| panic策略 | 未设置 | **abort** | 体积↓5~10% |
| Tokio线程 | 固定默认 | **CPU自适应** | 多核性能提升 |
| 适配器缓存 | TTL=15s | **启动预热** | 首次检测更快 |
| Portal检测错误 | 静默吞掉 | **详情记录+通知** | 可观测性提升 |

---

## 十、编译配置

```toml
[profile.release]
lto = "thin"
codegen-units = 1       # 🆕 从16降至1
opt-level = 3
strip = true
panic = "abort"         # 🆕 新增
```

---

*文档版本: v2.1.5 | 基于代码版本: CampusLogin v2.1.5 | 更新日期: 2026-05-12*
