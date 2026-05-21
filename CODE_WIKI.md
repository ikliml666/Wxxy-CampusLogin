# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.1.7 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
> **目标平台**: Windows (x64)
> **通信方式**: Tauri IPC (`invoke` / `listen`)

---

## 一、项目概览

CampusLogin 是一款校园网自动登录助手桌面应用，面向无锡学院校园网认证系统（锐捷 ePortal），提供一键登录/注销、自动重连、DNS 智能解析与优化、网络质量监测、多账号管理等功能。

### 核心特性

| 特性 | 说明 |
|------|------|
| 一键登录 | 自动检测适配器、DHCP续租、智能重试(指数退避+随机抖动) |
| 一键注销 | Portal MAC 解绑端点，支持指定适配器注销或全部注销 |
| 自动重连 | 后台巡检断线检测，最多3次自动重连 |
| DNS 智能解析 | 动态评分选择最优 DNS 服务器，应用级 DoH 解析，三级智能解析策略 |
| DNS 优化 | 检测 DNS/DoH 配置，一键设置推荐 DNS + 启用 DoH 加密 |
| 网络质量检测 | 网关/DNS/DoH/HTTPS/游戏服务器延迟并发测试，DNS 解析专项测试 |
| 多账号管理 | DPAPI 加密存储、快速切换 |
| 双适配器支持 | 有线 + 无线同时管理，Dock 栏适配器选择菜单 |
| 系统托盘 | 最小化到托盘后台运行，支持托盘快速登录 |
| 开机自启 | 注册表写入 / Tauri 插件 |
| 自动退出 | 登录成功后倒计时退出，快捷键取消(Ctrl+Shift+C) |
| 主题系统 | 6种预设主题 + 自定义主题色 + 深浅模式 |
| 用户自助服务 | 一键打开校园网自助服务系统 |

---

## 二、项目目录结构

```
Wxxy-CampusLogin/
├── assets/                          # 截图等资源
├── tauri-app/
│   ├── package.json                 # 根层依赖
│   ├── build.ps1                    # Windows 构建脚本
│   ├── frontend/                    # React 前端
│   │   ├── package.json             # 前端依赖 (含 zustand ^5.0.0, gsap ^3.15.0)
│   │   ├── vite.config.ts           # Vite 构建配置
│   │   ├── tailwind.config.js       # Tailwind CSS 配置
│   │   ├── tsconfig.json            # TypeScript 配置
│   │   ├── index.html               # HTML 入口
│   │   └── src/
│   │       ├── main.tsx             # React 入口
│   │       ├── App.tsx              # 根组件
│   │       ├── index.css            # 全局样式
│   │       ├── constants/
│   │       │   └── index.ts         # 常量 (ISP/导航/主题/质量等级/延迟胶囊背景色)
│   │       ├── hooks/
│   │       │   ├── useAppStore.ts   # 统一状态管理 (zustand) + 密码处理
│   │       │   ├── useIpc.ts        # Tauri IPC 封装 (含 DNS/DoH/注销 API)
│   │       │   └── useAppInit.ts    # 初始化逻辑 + 事件监听 + 在线日志去重
│   │       ├── types/
│   │       │   └── index.ts         # TypeScript 类型定义 (含 DnsDohStatus)
│   │       ├── lib/
│   │       │   ├── utils.ts         # 工具函数
│   │       │   ├── color.ts         # HEX→HSL 颜色转换
│   │       │   ├── latency.ts       # 延迟等级/颜色计算 (显式 borderBg)
│   │       │   └── animations.ts    # Framer Motion 动画变体
│   │       └── components/
│   │           ├── dialogs/         # 对话框 (关于/主题/确认)
│   │           ├── layout/          # 布局组件 (标题栏/状态栏/导航/Toast)
│   │           │   ├── DockNav.tsx  # 适配器选择浮层 + 注销按钮
│   │           │   └── StatusBar.tsx # 用户自助服务按钮 + 交互动画
│   │           ├── panels/          # 面板组件 (总览/账号/网络/监控/质量/设置/速度测试/日志)
│   │           │   └── NetworkPanel.tsx # DNS 优化卡片
│   │           ├── shared/          # 共享组件 (延迟组件/刷新按钮/分段Tab)
│   │           ├── ui/              # 基础 UI 组件 (shadcn/ui)
│   │           └── ErrorBoundary.tsx
│   └── src-tauri/                   # Rust 后端
│       ├── Cargo.toml               # Rust 依赖 (含 hickory-resolver, winreg, sha2)
│       ├── tauri.conf.json          # Tauri 应用配置
│       ├── capabilities/
│       │   └── default.json         # Tauri 权限声明
│       ├── icons/                   # 应用图标
│       └── src/
│           ├── main.rs              # 应用入口
│           ├── lib.rs               # 库模块声明
│           ├── config.rs            # 配置模型 + atomic_write 重试 + list_account_names
│           ├── network/             # 网络模块
│           │   ├── mod.rs           # 重导出 (含 do_logout_with_retry)
│           │   ├── cache.rs         # 缓存基础设施 (NET_CACHE/HTTP客户端/TLS 1.3+回退)
│           │   ├── adapter.rs       # 适配器查询/Win32 API/DHCP/网关/TTL缓存
│           │   ├── portal.rs        # Portal认证状态检测
│           │   ├── login_request.rs # 登录/注销请求/重试/响应解析
│           │   └── quality.rs       # 网络质量并发延迟测试 (两阶段检测)
│           ├── crypto_utils.rs      # 加密工具 (Windows DPAPI)
│           ├── http_timing.rs       # HTTP计时 + DNS智能解析 + DoH + 评分系统
│           ├── logger.rs            # 日志系统 (文件+通道+调试模式切换)
│           └── commands/            # Tauri 命令 (模块化拆分)
│               ├── mod.rs           # 命令模块声明与架构文档
│               ├── state.rs         # 全局状态 (TaskLock/分层AppState/is_logging_out)
│               ├── config_cmd.rs    # 配置相关命令 (空密码兜底)
│               ├── login.rs         # 登录/注销命令
│               ├── background.rs    # 后台检测调度器 (PortalCheckResult 职责分离)
│               ├── auto_login.rs    # 自动登录/断线重连
│               ├── auto_exit.rs     # 自动退出控制
│               ├── latency.rs       # 网络质量通知+延迟测试循环
│               ├── adapter_watch.rs # 适配器状态监控 (15s间隔)
│               ├── network_cmd.rs   # 网络命令 + DNS/DoH 检测与设置 (winreg + ShellExecuteW)
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
│  useAppStore (zustand) — 集中管理所有前端状态         │
│  useIpc Hook — Tauri IPC 通信封装 (单一实例)          │
├─────────────────────────────────────────────────────┤
│                IPC 通信层 (Bridge Layer)              │
│  Tauri invoke (请求-响应) | Tauri listen (事件推送)   │
│  前端 ←→ Rust 后端                                   │
├─────────────────────────────────────────────────────┤
│                 业务逻辑层 (Logic Layer)              │
│  ┌─────────────────────────────────────────────────┐ │
│  │  background.rs (调度器, PortalCheckResult 职责分离) │
│  │    ├─→ auto_login.rs   (自动登录/断线重连)      │ │
│  │    ├─→ auto_exit.rs    (自动退出倒计时)          │ │
│  │    ├─→ latency.rs      (质量通知/延迟循环)       │ │
│  │    └─→ adapter_watch.rs (适配器监控)            │ │
│  │  login.rs — 登录/注销认证                        │ │
│  │  network/ — 网络检测/延迟测试/注销请求 (6个子模块) │ │
│  │  http_timing.rs — DNS智能解析/DoH/评分系统       │ │
│  │  network_cmd.rs — DNS/DoH 检测与设置             │ │
│  │  account.rs — 多账号管理                          │ │
│  │  crypto_utils.rs — Windows DPAPI加密              │ │
│  │  config.rs — 配置管理 (含 atomic_write 重试)     │ │
│  └─────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────┤
│                 系统交互层 (System Layer)             │
│  Win32 API — 适配器查询(GetAdaptersAddresses)        │
│  ShellExecuteW — UAC 提权 (替代 PowerShell)          │
│  winreg — 注册表读写 (DNS/DoH 配置)                  │
│  reqwest — HTTP 请求 (TLS 1.3强制+1.2回退)          │
│  hickory-resolver — 传统 DNS 解析                    │
│  tokio-rustls — DoH TLS 连接 (RFC 8484)             │
│  tokio — 异步运行时                                  │
│  Windows Registry — 开机自启/DNS配置                 │
└─────────────────────────────────────────────────────┘
```

### 3.2 Commands 模块依赖关系 (v2.1.7)

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
//        └──→ network::do_logout_with_retry
//
//   auto_exit ──→ system (emit_notification)
//
//   adapter_watch ─ (无跨模块调用，仅依赖 state + network)
//
//   network_cmd ──→ network (适配器/质量检测)
//               └──→ http_timing (DNS/DoH 测试)
//               └──→ winreg (注册表 DNS/DoH 读写)
//               └──→ ShellExecuteW (UAC 提权)
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
2. **Setup 钩子**:
   - 创建数据目录
   - 加载配置 (含密码DPAPI解密)
   - 根据 `--autostart` 参数和 `hiddenStart` 配置决定是否显示窗口
   - 创建系统托盘
   - 启动适配器监控和启动任务 (通过 `run_startup_tasks`)
3. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭，退出时使用 `force_release()` 清理任务标志
4. **命令注册**: 30个 `#[tauri::command]` 函数

### 4.2 全局状态 — `commands/state.rs`

#### TaskLock 并发原语抽象

```rust
pub struct TaskLock { flag: AtomicBool }

pub struct TaskGuard<'a> { lock: &'a TaskLock }

pub struct TaskReleaseGuard<'a> { lock: &'a TaskLock }

impl TaskLock {
    pub fn new() -> Self { ... }
    pub fn try_acquire(&self) -> Option<TaskGuard<'_>> { ... }
    pub fn is_active(&self) -> bool { ... }
    pub fn force_release(&self) { ... }
    pub fn swap_acquire(&self) -> bool { ... }
    pub fn release_guard(&self) -> TaskReleaseGuard<'_> { ... }
}
```

#### 分层 AppState 结构体

```rust
pub struct TaskFlags {
    pub background_running: TaskLock,
    pub latency_running: TaskLock,
    pub latency_cancel: ArcSwap<tokio_util::sync::CancellationToken>,
    pub is_checking: TaskLock,
    pub is_logging_in: TaskLock,
    pub is_logging_out: TaskLock,      // 注销独立锁
    pub is_quality_checking: TaskLock,
}

pub struct NetworkStatus {
    pub server_available: AtomicBool,
    pub was_online: AtomicBool,
    pub has_logged_online: AtomicBool,
    pub disconnect_reconnect_count: AtomicU32,
    pub last_auto_login_attempt: ArcSwap<std::time::Instant>,
    pub last_network_quality: ArcSwap<Option<String>>,
}

pub struct ExitState {
    pub is_quitting: std::sync::Arc<AtomicBool>,
    pub auto_exit_deadline: Mutex<Option<Instant>>,
    pub auto_exit_cancelled: AtomicBool,
}

pub struct AppState {
    pub config: ArcSwap<Config>,
    pub tasks: TaskFlags,
    pub network: NetworkStatus,
    pub exit: ExitState,
    pub last_update_check_epoch_ms: AtomicU64,
    pub last_disabled_notification_ms: AtomicU64,
}
```

**关键常量**:

| 常量 | 值 | 说明 |
|------|----|------|
| `AUTO_EXIT_DELAY_MS` | 10000 | 自动退出倒计时 (毫秒) |
| `CANCEL_EXIT_SHORTCUT` | `"CommandOrControl+Shift+C"` | 取消快捷键 |

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `validate_config()` | 校验配置字段 (枚举值/正则/URL)，含 Portal URL 迁移逻辑 |
| `validate_account_name()` | 校验账号名 (1-32字符, 字母数字下划线中文连字符) |

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
| `portalUrl` | String | `"http://10.1.99.100"` | Portal地址 (无端口默认值) |
| `fixedGateway` | String | `""` | 固定网关IP |

**关键函数**:

| 函数 | 说明 |
|------|------|
| `atomic_write()` | 原子写入文件，3次重试+100ms间隔，失败保留临时文件 |
| `list_account_names()` | 共享函数，统一账号目录遍历逻辑 |
| `validate_username()` | 校验用户名 |
| `validate_operator()` | 校验运营商后缀 |
| `validate_password()` | 校验密码 |

### 4.4 加密工具 — `crypto_utils.rs`

Windows DPAPI 加密/解密，绑定当前 Windows 用户。

### 4.5 网络模块 — `network/`

#### 4.5.1 缓存基础设施 — `cache.rs`

**`NetworkCache` 结构体** (全局单例 `NET_CACHE`):

```rust
struct NetworkCache {
    pub adapter: ArcSwap<Option<AdapterCache>>,      // 适配器缓存 (TTL=5s)
    pub gateway: ArcSwap<Option<GatewayCacheEntry>>, // 网关缓存
    pub portal: ArcSwap<Option<PortalCacheEntry>>,   // Portal状态缓存
    pub portal_url: ArcSwap<String>,                 // Portal URL
}
```

**关键函数**:

| 函数 | 说明 |
|------|------|
| `clear_adapter_cache_only()` | 仅清除适配器缓存 |
| `clear_portal_cache()` | 仅清除 Portal 状态缓存 |
| `create_safe_http_client(timeout, local_addr)` | 创建 HTTP 客户端 (TLS 1.3 强制 + TLS 1.2 回退) |

#### 4.5.2 适配器查询 — `adapter.rs`

- Win32 API `GetAdaptersAddresses` 查询适配器
- TTL 5秒缓存，`get_adapters_force()` 先清除缓存再查询
- DHCP 续租、网关检测、适配器启用

#### 4.5.3 Portal 检测 — `portal.rs`

- Portal 认证状态检测
- URL `:801` 端口追加逻辑统一处理

#### 4.5.4 登录/注销请求 — `login_request.rs`

**登录函数**:

| 函数 | 说明 |
|------|------|
| `do_login_with_retry()` | 登录请求+重试(最多3次)，重试等待可中断(每100ms检查退出标志) |

**注销函数**:

| 函数 | 说明 |
|------|------|
| `do_logout_request()` | 注销请求 (Portal MAC 解绑端点) |
| `do_logout_with_retry()` | 注销重试(最多2次)，重试等待可中断 |
| `parse_logout_result()` | 注销结果解析 (JSONP) |

**注销 API 端点**:
```
GET /eportal/portal/mac/unbind?callback=dr1002&user_account={学号}&wlan_user_mac=&wlan_user_ip=&jsVersion=4.1.3&v=526&lang=zh
```

**注销成功判定**:
- `msg` 包含 "解绑终端MAC成功" → 注销成功
- `msg` 包含 "获取用户在线信息数据为空" → 当前无在线设备（也视为成功）

#### 4.5.5 网络质量检测 — `quality.rs`

**两阶段检测**:
1. Phase 1: 并行测试网关 + DNS 服务器 + DoH 服务器 → 更新评分表
2. Phase 2: 并行预解析 HTTPS 主机名 → 并行测试 HTTPS 网站

**任务类型** (`LatencyTask` 枚举):

| 变体 | 说明 |
|------|------|
| `Gateway` | 网关 ICMP ping |
| `DnsServer` | DNS 服务器延迟测试 |
| `Doh` | DoH 服务器延迟测试 |
| `Https` | HTTPS 网站延迟测试 |
| `SystemDns` | 系统 DNS 解析延迟测试 (多域名平均) |

### 4.6 DNS 智能解析 — `http_timing.rs`

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

**默认服务器**:

| 类型 | 服务器 |
|------|--------|
| DNS | `223.5.5.5`, `1.12.12.12`, `114.114.114.114` |
| DoH | `dns.alidns.com` (223.5.5.5), `doh.pub` (1.12.12.12) |

#### 应用级 DoH 解析

```rust
pub async fn resolve_via_doh(doh_server: &str, doh_ip: &str, domain: &str) -> Result<IpAddr, String>
```

- 直接 TCP 连接 DoH 服务器 443 端口 → TLS 握手 → 发送 RFC 8484 wire format 查询
- `?dns=<base64url>` + `Accept: application/dns-message`
- 完全绕过系统 DoH API

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `build_dns_query_wire()` | 构建标准 DNS wire format 查询报文 |
| `base64url_encode_no_pad()` | RFC 8484 要求的 base64url 编码 (无填充) |
| `parse_dns_response_wire()` | 解析 DNS wire format 响应，提取 A 记录 IP |

#### 三级智能解析策略

```rust
pub async fn resolve_host_smart(host: &str, bind_addr: Option<IpAddr>) -> Result<IpAddr, String>
```

```
DNS缓存 (TTL 60s) → 传统DNS (动态选最优服务器) → DoH回退 (按延迟排序尝试)
```

- 第一级: 查询 DNS 缓存
- 第二级: 传统 DNS 解析，使用延迟最优的服务器并发查询
- 第三级: DoH 回退，按延迟排序依次尝试各 DoH 服务器
- 自定义 DNS 失败时自动回退到系统 DNS (`ResolverConfig::default()`)

#### HTTP 计时

| 函数 | 说明 |
|------|------|
| `measure_https_timing()` | HTTPS 完整计时 (DNS/TCP/TLS/TTFB/Content)，使用 `resolve_host_smart` |
| `measure_dns_query()` | DNS 查询计时 |
| `measure_doh_timing()` | DoH 查询计时 |

### 4.7 登录/注销模块 — `commands/login.rs`

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

### 4.8 后台巡检调度器 — `commands/background.rs`

**职责**: 纯调度器，职责分离重构

**核心类型**:

```rust
enum PortalCheckResult {
    Success { online: bool, message: String, reachable: bool, login_available: bool },
    Error,
    NotFound,
}
```

**提取的独立函数**:

| 函数 | 说明 |
|------|------|
| `check_adapter_portal()` | 消除主/副适配器检测逻辑重复 |
| `build_adapter_details()` | 适配器详情构建 |
| `handle_status_change()` | 状态变更通知 |
| `emit_background_check_result()` | 统一检测结果 JSON 构建和事件发送 |
| `update_network_state()` | 独立网络状态更新逻辑 |
| `adapter_status_entry()` / `adapter_disabled_entry()` / `adapter_disconnected_entry()` | 适配器状态条目构建 |

**量化改进**:

| 指标 | 重构前 | 重构后 |
|------|--------|--------|
| `run_background_check_blocking` 行数 | ~190 行 | ~88 行 |
| 重复 JSON 构建代码 | 3 处 | 0 处 |
| 主函数圈复杂度 | 15+ | ~5 |

### 4.9 自动登录模块 — `commands/auto_login.rs`

**公开函数**:

| 函数 | 说明 |
|------|------|
| `try_auto_login_on_preparation()` | 准备阶段自动登录 (30秒冷却) |
| `try_disconnect_reconnect()` | 断线重连 (最多3次 + 间隔提醒) |
| `run_auto_login_on_start()` | 启动时自动登录 (1.5s延迟 + Portal预检) |

### 4.10 自动退出模块 — `commands/auto_exit.rs`

| 函数 | 说明 |
|------|------|
| `start_auto_exit()` | 启动自动退出倒计时 + 快捷键注册 + 通知 |
| `cancel_auto_exit_inner()` | 取消自动退出 |

### 4.11 延迟测试模块 — `commands/latency.rs`

| 函数 | 说明 |
|------|------|
| `notify_network_quality_change()` | 网络质量变化通知 (bad/good 级别切换) |
| `spawn_latency_test_loop()` | 启动延迟测试循环 (CancellationToken) |

### 4.12 适配器监控模块 — `commands/adapter_watch.rs`

| 函数 | 说明 |
|------|------|
| `start_adapter_watch()` | 启动适配器状态监控循环 (15s间隔) |

### 4.13 网络命令模块 — `commands/network_cmd.rs`

**DNS/DoH 检测与设置**:

| 函数 | 说明 |
|------|------|
| `check_dns_doh_status()` | 通过 winreg 读取注册表检测 DNS/DoH 状态 |
| `enable_doh_for_dns()` | 启用 DoH (ShellExecuteW 提权) |
| `setup_dns_doh()` | 一键设置推荐 DNS + DoH (netsh + ShellExecuteW) |

**UAC 提权**:

```rust
fn run_elevated(cmd: &str, args: &str) -> Result<(), String> {
    // ShellExecuteW + "runas" 实现UAC提权，耗时约1ms
}
```

**注册表路径**:
- 适配器 DNS: `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{GUID}\NameServer`
- 适配器名称映射: `HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-...}\{GUID}\Connection\Name`
- DoH 配置: `HKLM\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DohWellKnownServers\{IP}`

### 4.14 其他命令模块

**config_cmd.rs** — 配置保存/加载，空密码兜底逻辑 (前端未传密码且旧密码存在时保留旧密码)

**account.rs** — 多账号管理，使用 `list_account_names()` 共享函数

**system.rs** — 系统功能命令，`get_init_data` 使用 `list_account_names()`

**updater.rs** — 更新检查/下载/安装，`update-available` 事件发送失败时记录日志

---

## 五、前端模块详解 (React/TypeScript)

### 5.1 状态管理 — `hooks/useAppStore.ts`

- 基于 zustand ^5.0.0 的全局状态管理
- 密码处理简化：`password === '***'` 时直接 `delete` 密码字段，不再发送占位符
- 注销状态：`isLoggingOut` + `doLogout` action
- 登录/注销均支持 `adapterName` 可选参数

### 5.2 IPC 封装 — `hooks/useIpc.ts`

**新增 API**:

| API | 说明 |
|-----|------|
| `doLogin(adapterName?)` | 登录，可选指定适配器 |
| `doLogout(adapterName?)` | 注销，可选指定适配器 |
| `checkDnsDohStatus()` | 检测 DNS/DoH 状态 |
| `setupDnsDoh()` | 一键设置推荐 DNS + DoH |

### 5.3 初始化逻辑 — `hooks/useAppInit.ts`

- 在线日志去重：5秒内在线日志自动去重
- 主/副适配器状态合并显示：`"已在线（以太网、WLAN）"`
- 移除冗余日志（适配器变更不再重复报告在线状态）

### 5.4 Dock 导航栏 — `components/layout/DockNav.tsx`

- **适配器选择浮层**: 多个适配器时 hover 弹出选择菜单
  - 无线用蓝色 Wifi 图标，有线用绿色 Cable 图标
  - 200ms 延迟关闭，避免误操作
  - 登录中/注销中不弹出浮层
- **注销按钮**: outline 风格，与登录按钮并列

### 5.5 状态栏 — `components/layout/StatusBar.tsx`

- **用户自助服务按钮**: 紫色 UserCircle 图标，点击打开自助服务系统
- **交互动画**: framer-motion 弹性缩放 (hover 1.12x, tap 0.88x, spring 过渡)

### 5.6 网络面板 — `components/panels/NetworkPanel.tsx`

- **DNS 优化卡片**: 检测当前 DNS/DoH 配置状态
- **一键优化按钮**: 始终显示，设置阿里 DNS + 腾讯 DNS + 启用 DoH

### 5.7 延迟颜色 — `lib/latency.ts`

- `QUALITY_CONFIG` 新增显式 `borderBg` 字段，确保 Tailwind JIT 可扫描
- `getLatencyColor()` 使用 `cfg.borderBg` 替代动态字符串替换

### 5.8 类型定义 — `types/index.ts`

**新增类型**:

| 类型 | 说明 |
|------|------|
| `DnsDohStatus` | DNS/DoH 状态 (适配器列表 + DoH 支持情况) |
| `DnsAdapterInfo` | 适配器 DNS 信息 (内部类型) |
| `DnsServerInfo` | DNS 服务器信息 (内部类型) |

---

## 六、IPC 通信完整清单

### 6.1 请求-响应命令 (v2.1.7: 30个)

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
| `enable_doh_for_dns` | 启用 DoH |
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
| `send_notification` | 发送通知 |
| `cancel_auto_exit` | 取消自动退出 |
| `minimize_window` / `close_window` / `window_move` | 窗口控制 |
| `open_external` | 打开外部链接 |
| `get_logs` / `clear_logs` | 日志管理 |
| `get_init_data` | 初始化数据 |
| `check_update` / `download_update` / `install_update` / `get_mirror_urls` | 更新管理 |
| `set_debug_mode` / `get_debug_mode` | 调试模式 |

### 6.2 事件推送

| 事件名 | 说明 |
|--------|------|
| `background-check-result` | 后台检测结果 (含 adapter1Name/adapter2Name) |
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

---

## 七、依赖关系

### 7.1 Rust 依赖 (Cargo.toml)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tauri` | 2 | 应用框架 |
| `tauri-plugin-*` | 2 | shell/notification/autostart/global-shortcut/single-instance |
| `serde` / `serde_json` | 1 | 序列化 |
| `tokio` | 1 (full) | 异步运行时 |
| `tokio-util` | 0.7 (rt) | CancellationToken |
| `reqwest` | 0.12 (json, blocking, http2, rustls-tls, charset) | HTTP客户端 |
| `tokio-rustls` | 0.26 | TLS 连接 (DoH) |
| `rustls-pki-types` | 1 | TLS 类型 |
| `webpki-roots` | 0.26 | TLS 根证书 |
| `hickory-resolver` | 0.24 (tokio-runtime) | DNS 解析 |
| `dashmap` | 6 | 并发 HashMap (DNS 评分/缓存) |
| `parking_lot` | 0.12 | 高性能同步原语 |
| `arc-swap` | 1 | 原子引用交换 |
| `windows` | 0.58 | Win32 API (含 Shell/UI/Threading) |
| `winreg` | 0.52 | Windows 注册表读写 |
| `surge-ping` | 0.8 | ICMP ping |
| `sha2` | 0.10 | SHA-2 哈希 |
| `urlencoding` | 2 | URL 编码 |
| `regex` | 1 | 正则表达式 |
| `url` | 2 | URL 解析验证 |
| `dirs` | 6 | 数据目录 |
| `rand` | 0.8 | 随机数 |
| `lazy_static` | 1.5 | 静态初始化 |
| `base64` | 0.22 | Base64 编解码 |
| `chrono` | 0.4 | 时间处理 |
| `open` | 5 | 打开外部链接 |
| `thiserror` | 2 | 错误类型 |

### 7.2 前端依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `react` / `react-dom` | 19 | UI框架 |
| `@tauri-apps/api` | ^2 | Tauri前端API |
| `framer-motion` | ^12 | 动画 |
| `gsap` | ^3.15 | 动画 |
| `lucide-react` | ^0.446 | 图标 |
| `zustand` | ^5.0 | 状态管理 |
| `tailwindcss` | ^3.4 | CSS框架 |
| `vite` | ^6 | 构建 |
| `typescript` | ^5.5 | 类型系统 |
| Radix UI primitives | 各版本 | 无障碍UI |

### 7.3 依赖关系图

```
main.rs
  ├── lib.rs (Tauri库入口)
  └── commands/mod.rs
        ├── state.rs ← config.rs, arc-swap, dashmap, parking_lot, tokio-util
        │   [TaskLock/TaskGuard/TaskReleaseGuard 抽象]
        │   [ExitState / AppState 分层]
        │   [is_logging_out 独立锁]
        ├── config_cmd.rs ← config.rs, crypto_utils.rs, state.rs
        ├── login.rs ← network/*, state.rs, system.rs
        │   [do_login + do_logout, adapter_name 可选参数]
        ├── background.rs (调度器, PortalCheckResult 职责分离)
        │   ├── auto_login.rs ← state.rs, login.rs, system.rs, auto_exit.rs
        │   ├── auto_exit.rs ← state.rs, system.rs
        │   ├── latency.rs ← state.rs, network/*, system.rs
        │   └── (直接调用上述模块)
        ├── auto_login.rs
        ├── auto_exit.rs
        ├── latency.rs
        ├── adapter_watch.rs
        ├── network_cmd.rs ← network/*, state.rs, http_timing, winreg, ShellExecuteW
        │   [check_dns_doh_status / setup_dns_doh / run_elevated]
        ├── account.rs ← config.rs, crypto_utils.rs, state.rs, config_cmd.rs
        ├── system.rs ← config.rs, network/*, state.rs, winreg
        └── updater.rs ← reqwest, url

network/
  ├── mod.rs (重导出 + do_logout_with_retry)
  ├── cache.rs ← arc-swap, lazy_static, dashmap, reqwest [TLS 1.3]
  ├── adapter.rs ← cache.rs, windows, regex [TTL 5s 缓存]
  ├── portal.rs ← cache.rs, reqwest, url
  ├── login_request.rs ← cache.rs, reqwest, urlencoding, regex, rand
  │   [do_login_with_retry + do_logout_with_retry]
  └── quality.rs ← adapter.rs, cache.rs, surge-ping, tokio-rustls, http_timing
      [两阶段检测: DNS/DoH → HTTPS]

http_timing.rs
  ├── DNS_SERVER_SCORES / DOH_SERVER_SCORES (dashmap 评分表)
  ├── resolve_host_smart (三级智能解析)
  ├── resolve_via_doh (RFC 8484 wire format)
  ├── build_dns_query_wire / parse_dns_response_wire
  └── measure_https_timing / measure_dns_query / measure_doh_timing

App.tsx
  └── useAppStore (zustand)
        ├── useIpc.ts ← @tauri-apps/api
        └── useAppInit.ts
```

---

## 八、安全体系

| 措施 | 实现 |
|------|------|
| 密码加密存储 | Windows DPAPI，绑定当前用户 |
| 密码防意外清空 | 前端空密码不发送，后端空密码+旧密码存在时保留旧密码 |
| TLS 1.3 强制 | HTTP 客户端默认 TLS 1.3，回退 TLS 1.2 |
| DoH 安全 | RFC 8484 wire format，兼容主流 DNS 服务商 |
| CSP 策略 | 限制脚本、插件和表单提交来源 |
| 外部链接验证 | URL 验证 + 本地地址黑名单 |
| 登录频率限制 | 防止滥用 |
| panic=abort | 编译选项减小二进制体积，避免信息泄露 |
| force_release 防死锁 | 退出场景强制释放任务锁 |

---

## 九、性能优化

| 优化项 | 实现 | 效果 |
|--------|------|------|
| PowerShell 消除 | ShellExecuteW 替代 PowerShell UAC 提权 | 200-500ms → 1ms |
| DNS 设置优化 | netsh + ShellExecuteW 替代 PowerShell | 1-3s → 100-300ms |
| DNS 注册表检测 | winreg 替代 PowerShell | 10x+ 速度提升 |
| 适配器 TTL 缓存 | 5秒 TTL，避免频繁调用 Win32 API | 减少 API 调用 |
| codegen-units=1 | LTO 跨单元优化 | +2~5% 性能 |
| panic=abort | 编译选项 | 体积↓5~10% |
| DNS 预取移除 | 删除无效并发 DNS 查询 | 减少资源消耗 |
| 登录重试可中断 | 每100ms检查退出标志 | 退出延迟 2s → 100ms |

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

*文档版本: v2.1.7 | 基于代码版本: CampusLogin v2.1.7 | 更新日期: 2026-05-16*
