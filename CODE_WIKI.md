# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.1.4 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
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
│   │   ├── package.json             # 前端依赖
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
│       ├── Cargo.toml               # Rust 依赖
│       ├── tauri.conf.json          # ★ Tauri 应用配置
│       ├── capabilities/
│       │   └── default.json         # Tauri 权限声明
│       ├── icons/                   # 应用图标
│       └── src/
│           ├── main.rs              # ★ 应用入口
│           ├── lib.rs                # 库模块声明 (Tauri构建所需)
│           ├── config.rs            # ★ 配置模型与路径
│           ├── network/             # ★ 网络模块 (拆分后)
│           │   ├── mod.rs           # 重导出 + NetworkError类型
│           │   ├── cache.rs         # 缓存基础设施 (NET_CACHE/HTTP客户端)
│           │   ├── adapter.rs       # 适配器查询/Win32 API/DHCP/网关
│           │   ├── portal.rs        # Portal认证状态检测
│           │   ├── login_request.rs # 登录请求/重试/响应解析
│           │   └── quality.rs       # 网络质量并发延迟测试
│           ├── crypto_utils.rs      # ★ 加密工具 (Windows DPAPI)
│           ├── http_timing.rs       # HTTP计时 (DoH/DNS/TLS/TCP)
│           ├── logger.rs            # ★ 日志系统 (文件+通道)
│           └── commands/            # Tauri 命令
│               ├── mod.rs           # 命令模块声明与导出
│               ├── state.rs         # ★ 全局状态 (AppState/TaskFlags/NetworkStatus)
│               ├── config_cmd.rs    # 配置相关命令
│               ├── login.rs         # ★ 登录命令
│               ├── background.rs    # ★ 后台巡检/适配器监控/自动退出
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
│  Framer Motion 动画 | shadcn/ui 组件库               │
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
│  login.rs — 登录认证                                 │
│  background.rs — 后台巡检/断线重连                    │
│  network/ — 网络检测/延迟测试 (5个子模块)             │
│  account.rs — 多账号管理                              │
│  crypto_utils.rs — Windows DPAPI加密                  │
│  config.rs — 配置管理                                │
├─────────────────────────────────────────────────────┤
│                 系统交互层 (System Layer)             │
│  Win32 API — 适配器查询(GetAdaptersAddresses)        │
│  cmd.exe / netsh — DHCP续租/适配器启用                │
│  reqwest — HTTP 请求 (校园网认证API)                  │
│  tokio — 异步运行时                                  │
│  Windows Registry — 开机自启                         │
└─────────────────────────────────────────────────────┘
```

### 3.2 数据流

```
用户操作 → React组件 → useAppStore → useIpc.invoke()
                                         ↓
                                    Tauri IPC
                                         ↓
                              #[tauri::command] Rust函数
                                         ↓
                              AppState (TaskFlags + NetworkStatus)
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

**职责**: 应用初始化、线程池配置、Tauri 插件注册、窗口/托盘/事件处理

**关键流程**:

1. **线程池配置**: 根据 CPU 核心数动态分配工作线程数
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
   - 并行初始化 CryptoKeys 和适配器缓存
   - 启动适配器监控和启动任务 (自动登录/后台检测/延迟测试)
4. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭，退出时清零加密密钥
5. **命令注册**: 28个 `#[tauri::command]` 函数

### 4.2 全局状态 — `commands/state.rs`

**核心结构体** (v2.1.4 分组重构):

```rust
pub struct TaskFlags {                    // 6个任务运行标志
    pub background_running: AtomicBool,
    pub latency_running: AtomicBool,
    pub latency_generation: AtomicU32,
    pub is_checking: AtomicBool,
    pub is_logging_in: AtomicBool,
    pub is_quality_checking: AtomicBool,
}

pub struct NetworkStatus {                // 7个网络状态字段
    pub server_available: AtomicBool,
    pub was_online: AtomicBool,
    pub has_logged_online: AtomicBool,
    pub background_check_count: AtomicU64,
    pub disconnect_reconnect_count: AtomicU32,
    pub cached_online_status: ArcSwap<Option<serde_json::Value>>,
    pub last_network_quality: ArcSwap<Option<String>>,
}

pub struct AppState {
    pub config: ArcSwap<Config>,          // 应用配置 (原子引用交换)
    pub tasks: TaskFlags,                  // 任务标志分组
    pub network: NetworkStatus,            // 网络状态分组
    pub is_quitting: Arc<AtomicBool>,     // 应用正在退出
    pub auto_exit_deadline: Mutex<Option<Instant>>,
    pub auto_exit_cancelled: AtomicBool,
    pub login_timestamps: Mutex<Vec<Instant>>, // 登录频率限制
    pub last_disabled_notification_epoch_ms: AtomicU64,
}
```

**关键常量**:

| 常量 | 值 | 说明 |
|------|----|------|
| `AUTO_EXIT_DELAY_MS` | 5000 | 自动退出倒计时 (毫秒) |
| `LOGIN_RATE_LIMIT_SECS` | 3 | 登录频率限制时间窗 (秒) |
| `LOGIN_RATE_LIMIT_MAX` | 3 | 时间窗内最大登录次数 |

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `check_login_rate_limit()` | 检查登录频率 (3秒内最多3次) |
| `validate_config()` | 校验配置字段 (长度/枚举值/正则/URL)，非法返回错误 |
| `validate_account_name()` | 校验账号名 (1-32字符, 字母数字下划线中文连字符) |
| `write_file_restricted()` | 写入文件并设置权限 |

**`CommandResult` 结构体**:

```rust
pub struct CommandResult {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}
```

**`atomic_guard!` 宏**: 用于任务标志的 RAII 守卫，Drop 时自动将对应 AtomicBool 设为 false。所有守卫字段都在 `tasks.` 下。

### 4.3 配置管理 — `config.rs`

**`Config` 结构体** (26个字段):

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `user` | String | `""` | 学号 |
| `password` | String | `""` | 密码 (内存中明文, 磁盘上DPAPI加密) |
| `operator` | String | `""` | 运营商后缀 (`""/@ctcc/@cucc/@cmcc`) |
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
| `portalUrl` | String | `"http://10.1.99.100:801"` | Portal地址 |
| `fixedGateway` | String | `""` | 固定网关IP |

**路径函数**:

| 函数 | 返回路径 | 说明 |
|------|----------|------|
| `get_data_dir()` | `%APPDATA%/campus-login/` | 应用数据目录 |
| `get_config_path()` | `{data_dir}/config.json` | 配置文件 |
| `get_accounts_dir()` | `{data_dir}/accounts/` | 账号存储目录 |
| `get_login_history_path()` | `{data_dir}/login-history.json` | 登录历史 |

### 4.4 加密工具 — `crypto_utils.rs`

**加密方案**: Windows DPAPI (CryptProtectData / CryptUnprotectData)

- 密码绑定当前 Windows 用户，无法在其他账户或机器上解密
- 无需管理密钥派生链或 salt 文件
- 使用 `optional_entropy` 作为额外保护层

**关键函数**:

| 函数 | 说明 |
|------|------|
| `encrypt_string(plaintext)` | DPAPI 加密字符串 → Base64 |
| `decrypt_string(ciphertext)` | Base64 解码 → DPAPI 解密 → 明文 |
| `CryptoKeys::new()` | 初始化加密环境 (当前为空实现，预留扩展) |

### 4.5 网络模块 — `network/` (v2.1.4 拆分后)

#### 4.5.1 缓存基础设施 — `cache.rs`

**`NetworkCache` 结构体** (全局单例 `NET_CACHE`):

```rust
pub(crate) static ref NET_CACHE: NetworkCache = NetworkCache::new();

struct NetworkCache {
    pub adapter: ArcSwap<Option<AdapterCache>>,      // 适配器缓存 (TTL=15s)
    pub gateway: ArcSwap<Option<GatewayCacheEntry>>, // 网关缓存
    pub portal: ArcSwap<Option<PortalCacheEntry>>,   // Portal状态缓存
    pub portal_lock: parking_lot::Mutex<()>,          // Portal检测互斥锁
    pub http_clients: DashMap<String, (Client, Instant)>, // HTTP客户端缓存 (DashMap并发安全)
    pub portal_url: ArcSwap<String>,                 // Portal URL (原独立全局变量)
}
```

**常量**:
- `MAX_RESPONSE_SIZE`: 16384 bytes (HTTP响应截断上限)
- `CACHE_TTL_MS`: 15000ms (适配器缓存有效期)

**关键函数**:
- `create_safe_http_client(timeout, local_addr)` — 创建带超时和本地地址绑定的HTTP客户端，带 DashMap 缓存 (TTL=300s)
- `clear_adapter_cache()` / `clear_adapter_cache_only()` — 清除全部/仅适配器缓存
- `update_portal_url(url)` — 更新 Portal URL

#### 4.5.2 适配器管理 — `adapter.rs`

**核心数据结构**:

| 结构体 | 字段 | 说明 |
|--------|------|------|
| `Adapter` | name, ip, wireless | 网络适配器 |
| `AdapterDetail` | name, ip, wireless, subnet_mask, gateway, dhcp_server | 适配器详情 |
| `DisabledAdapter` | name, status, description | 已禁用适配器 |

**适配器查询方式**: Windows Win32 API (`GetAdaptersAddresses`)，非 PowerShell

**改进点 (v2.1.4)**:
- 3次重试 + 缓冲区动态扩展 (原始代码仅尝试1次)
- `read_pwstr()`: 先尝试 `PCWSTR::to_string()` 安全解析，fallback 用4096字符上限手动遍历
- 黑名单正则覆盖 Hyper-V/VMware/Docker/WSL/VPN/Bluetooth 等

**关键函数**:

| 函数 | 说明 |
|------|------|
| `get_adapters_cached()` | 获取适配器列表 (带15s TTL缓存) |
| `get_adapters_force()` | 强制刷新缓存后获取 |
| `get_all_adapters_cached()` | 获取 (adapters, details, disabled) 元组 |
| `get_disabled_adapters_cached()` | 获取已禁用适配器 |
| `enable_adapter(adapter_name)` | 通过 netsh 启用适配器 |
| `get_adapter_details_cached()` | 获取适配器详情 (含子网掩码/网关/DHCP) |
| `resolve_adapter_names(adapters, config)` | 解析主/副适配器名称 |
| `select_adapter(adapters, config)` | 选择适配器 (优先指定→有线→任意) |
| `get_gateway_ip_cached(adapter_name, adapter_ip)` | 获取网关IP (ipconfig→推断.254) |
| `dhcp_renew_wired_only()` | DHCP续租所有有线适配器 |
| `wait_for_adapter(max_wait_ms, is_quitting)` | 等待适配器就绪 (轮询间隔递增) |

#### 4.5.3 Portal检测 — `portal.rs`

**`PortalStatus` 结构体**:

```rust
pub struct PortalStatus {
    pub reachable: bool,      // Portal可达
    pub login_available: bool, // 可登录 (含eportal/login/dr1003)
    pub online: bool,         // 已在线
    pub message: String,      // 状态描述
    pub data_length: usize,   // 响应大小
}
```

**`check_portal_full(adapter_ip, adapter_name)`**:
- Double-check locking 模式 (先读锁检查 → 写锁检查 → 执行 → 写锁写入 → 释放)
- 并发双请求: GET 百度 + GET Portal (thread::scope 并行)
- 响应分析: 检测 eportal/dr1003/uid/oltime 关键字

**v2.1.4 修复**: 锁释放时机从缓存写入之前移到之后，防止并发写入不一致。

#### 4.5.4 登录请求 — `login_request.rs`

**`do_login_with_retry(user, password, operator, adapter_ip, max_retries, is_quitting)`**:
- 最大重试次数可配置
- 指数退避: `500 * 2^(attempt-1)` ms
- 随机抖动: `0~300ms` (避免惊群效应)
- 每次 retry 前检查 `is_quitting` 标志

**`parse_login_result(response)`**:
- 解析 dr1003 JSONP 回调
- result==0: 成功 (区分"已在线"/"认证成功"/"AC认证失败")
- result==1: 失败 (区分"认证成功"兜底/通用失败)
- result==2: IP冲突/重复登录
- result==3: 流量超限
- result==4: 账号被禁用

**v2.1.4 修复**: AC认证失败时 code 从 "0" 改为 "ac_auth_failed"，消除歧义。

#### 4.5.5 网络质量检测 — `quality.rs`

**`NetworkQualityResult` 结构体**:

```rust
pub struct NetworkQualityResult {
    pub gateway_latency: i64,
    pub external_latency: i64,
    pub average_external_latency: i64,
    pub gateway: String,
    pub quality: String,     // excellent/great/good/fair/poor/bad/unknown
    pub timestamp: u64,
    pub details: serde_json::Value,
    pub metrics: serde_json::Value,
}
```

**检测目标** (20+个并发任务):

| 类型 | 目标 | 数量 |
|------|------|------|
| 网关 | 默认网关 (TCP/ICMP竞速) | 1 |
| DoH | 阿里DoH, 腾讯DoH | 2 |
| DNS解析 | www.baidu.com | 1 |
| DNS服务器 | 阿里DNS, 腾讯DNS, 信风DNS | 3 |
| HTTPS | 百度/京东/必应/12306/LOL/原神/PUBG/永劫无间/B站/B站直播/抖音/抖音直播 | 12 |

**质量等级判定**:

| 延迟范围 | 等级 |
|----------|------|
| ≤20ms | excellent |
| ≤50ms | great |
| ≤100ms | good |
| ≤200ms | fair |
| ≤400ms | poor |
| >400ms | bad |

**v2.1.4 改进**:
- 新增 `fixed_gateway` 参数支持自定义网关
- 新增 `is_quitting` 参数支持提前退出
- 网关过滤保留 `192.168.x.x` 过滤 (用户路由器场景)

### 4.6 登录模块 — `commands/login.rs`

**完整登录流程** (`full_login_inner`):

```
1. 校验用户名/密码非空
2. 获取适配器列表 (缓存→等待最多10s→失败)
3. 选择主适配器 (select_adapter)
4. 解析副适配器 (dual_adapter模式)
5. 双适配器模式:
   ├─ tokio::join! 并行登录两个适配器
   └─ 任一成功即返回成功
6. 单适配器模式:
   ├─ 无线适配器 → 跳过门户检测，直接登录
   └─ 有线适配器:
       ├─ check_portal_full → 已在线 → 返回成功
       ├─ check_portal_full → 可登录 → do_login_with_retry(3次)
       ├─ check_portal_full → 不可达/不可登录:
       │   ├─ dhcp_renew_wired_only()
       │   ├─ 等待IP变化 (最多6s)
       │   ├─ 重新选择适配器
       │   ├─ 再次门户检测 → 已在线 → 返回成功
       │   └─ do_login_with_retry(3次)
       └─ 记录登录历史
```

**登录命令** (`do_login`):
1. 等待加密初始化就绪
2. 原子交换 `is_logging_in` (防重入)
3. 登录频率限制检查 (3秒内最多3次)
4. 清除适配器缓存
5. 调用 full_login_inner_async()
6. 成功后清除在线状态缓存 + 触发后台检测

**v2.1.4 修复**: 移除了登录成功后5秒延迟清除缓存的 spawn 块，避免与后台检测冲突。

### 4.7 后台巡检 — `commands/background.rs`

**关键函数**:

| 函数 | 说明 |
|------|------|
| `run_background_check_blocking()` | ★ 同步后台检测核心逻辑 |
| `start_background_check_inner()` | 启动后台检测循环 (60s间隔) |
| `stop_background_check()` | 停止后台检测 |
| `trigger_background_check()` | 手动触发一次检测 |
| `start_auto_exit()` | 启动自动退出倒计时 (10s/30s) |
| `cancel_auto_exit_inner()` | 取消自动退出 |
| `start_adapter_watch()` | ★ 适配器状态监控 (15s间隔) |
| `spawn_latency_test_loop()` | 延迟测试循环 |
| `run_startup_tasks()` | ★ 启动任务编排 |

**后台检测流程**:
1. 防重入检查 (`tasks.is_checking`)
2. 获取适配器列表
3. 并行门户检测 (双适配器)
4. 发送 `background-check-result` 事件
5. 自动登录逻辑 (login_available && !online)
6. 断线重连逻辑 (was_online && offline, 最多3次)
7. 首次在线 → start_auto_exit()

**v2.1.4 修复**:
- 快捷键注册失败时延长3倍倒计时并重新 emit 事件通知前端
- 所有 `state.xxx` 引用更新为 `state.tasks.xxx` 或 `state.network.xxx`

### 4.8 其他命令模块

**config_cmd.rs** — 配置相关:
- `get_config` — 获取配置 (密码脱敏为 `***`)
- `save_config` — 保存配置 (校验→合并→加密→写盘, 变更检测避免无效IO)
- `show_window` — 显示并聚焦主窗口

**account.rs** — 多账号管理:
- `list_accounts` / `switch_account` / `save_current_as_account` / `delete_account` / `load_account_config` / `get_active_account`

**network_cmd.rs** — 网络命令:
- `get_adapters` / `get_disabled_adapters` / `enable_adapter` / `get_adapter_details`
- `is_online` / `check_portal_status` / `dhcp_renew_all`
- `check_network_quality` / `start_latency_test` / `stop_latency_test` / `get_latency_test_status`
- `http_timing_test` — 单次HTTP计时测试

**system.rs** — 系统功能:
- `minimize_window` / `close_window` / `window_move`
- `open_external` — 打开外部链接 (仅阻止 loopback/link-local, 允许私有IP)
- `get_auto_launch` / `set_auto_launch` (Windows注册表)
- `get_notification_enabled` / `set_notification_enabled` / `send_notification`
- `cancel_auto_exit` / `get_login_history` / `clear_login_history` / `get_perf_metrics`
- `append_login_history` — 追加登录历史记录

**updater.rs** — 更新功能:
- `check_update` — 检查GitHub Release新版本
- `download_update` — 下载更新包
- `install_update` — 安装更新 (路径校验+TOCTOU防护)

---

## 五、前端模块详解 (React/TypeScript)

### 5.1 入口 — `main.tsx`

- 初始化主题 (从 localStorage 读取明暗模式和主题名)
- 使用 `LazyMotion` + `domAnimation` 优化 Framer Motion 性能
- 包裹 `ErrorBoundary` 错误边界
- 包裹 `AppStoreProvider` (单一Provider)

### 5.2 状态管理 — `hooks/AppStoreContext.tsx` (v2.1.4 重构)

**单一 `AppStoreProvider`** 替代原来的三层嵌套:
- ConfigProvider → ~~NetworkProvider~~ → ~~UIProvider~~ → **AppStoreProvider**
- 仅创建一个 `useIpc()` 实例 (原来3个)

**核心状态** (全部集中在一个 Context 中):

| 分组 | 状态 | 类型 |
|------|------|------|
| 配置 | config, configRef, passwordSaved, passwordSavedRef | Config, Ref |
| 网络 | adapters, disabledAdapters, adapterDetails, accounts, activeAccount, bgStatus, networkQuality | 各类数组/对象 |
| UI | activePanel, notificationEnabled, autoLaunch, logs, toasts, themeName, isLightMode | 原始值 |
| 操作 | updateConfig, saveConfigDirect, saveConfigDebounced, flushPendingSave | 函数 |
| 操作 | setAdapters, setDisabledAdapters, setAdapterDetails, setAccounts, setActiveAccount, setBgStatus, setNetworkQuality | setState |
| 操作 | addLog, addToast, addToastWithAction, removeToast, removeToastsByPrefix, setLogs | 日志/Toast |
| 操作 | setActivePanel, setThemeName, setIsLightMode, setNotificationEnabled, setAutoLaunch, initTheme | 设置 |
| 异步 | doLogin, checkOnline, refreshQuality | async函数 |
| 导出 | api (useIpc实例) | IpcApi |

### 5.3 初始化逻辑 — `hooks/useAppInit.ts`

**v2.1.4 改进**:
- `initDoneRef` 守卫确保初始化只执行一次 (解决 stale closure 问题)
- `initCallbacksRef` 存储回调引用，避免 useEffect 依赖不稳定
- `callbacksRef` 存储事件处理回调
- 监听9种后端事件 (background-check-result, auto-login-result, adapters-changed, disabled-adapters-changed, adapter-disabled-warning, login-log, network-quality-result, auto-exit-countdown, auto-exit-cancelled, system-notification)
- Ctrl+Shift+C 键盘监听取消自动退出

### 5.4 IPC 通信层 — `hooks/useIpc.ts`

**请求-响应 (invoke)** — ~30个方法

**事件监听 (listen)** — 11个事件

### 5.5 类型定义 — `types/index.ts`

主要类型: Config, Adapter, DisabledAdapter, AdapterDetail, LogEntry, NetworkQualityDetail, NetworkQualityMetrics, NetworkQuality, BackgroundStatus, AdapterOnlineStatus, ToastMessage, StatusState, PanelName, ThemeName, LogType

### 5.6 工具库

| 文件 | 关键导出 |
|------|----------|
| `lib/utils.ts` | `cn` (Tailwind类合并), `safeStorage` (localStorage封装) |
| `lib/color.ts` | `hexToHsl()` |
| `lib/latency.ts` | `getLatencyLevel`, `getLatencyColor`, `extractGatewayLatency`, `extractExternalLatency` |
| `lib/animations.ts` | `containerVariants`, `itemVariants`, `panelSwitchVariants`, `logEntryVariants` |

### 5.7 UI 组件

**布局组件**: TitleBar, StatusBar, DockNav, RightPanel, ToastContainer

**面板组件** (8个): DashboardPanel, AccountPanel, NetworkPanel, MonitorPanel, QualityPanel, SettingsPanel, SpeedTestPanel, LogPanel

**对话框**: AboutDialog, ThemeDialog, ConfirmDialog

**共享组件**: AnimatedNumber, LatencyTimeline, LatencyPair (LatencyComponents), RefreshButton, SegmentTabs

**基础UI** (shadcn/ui): animated-card, badge, button, card, dialog, input, label, select, separator, switch, tooltip

---

## 六、IPC 通信完整清单

### 6.1 请求-响应命令

| 命令名 | 说明 |
|--------|------|
| `get_config` | 获取配置 (密码脱敏) |
| `save_config` | 保存配置 |
| `show_window` | 显示窗口 |
| `do_login` | 执行登录 |
| `get_adapters` | 获取适配器 |
| `get_disabled_adapters` | 获取禁用适配器 |
| `enable_adapter` | 启用适配器 |
| `get_adapter_details` | 获取适配器详情 |
| `is_online` | 检测在线 |
| `check_portal_status` | 门户状态检测 |
| `dhcp_renew_all` | DHCP续租 |
| `check_network_quality` | 网络质量检测 |
| `start_latency_test` | 启动延迟测试 |
| `stop_latency_test` | 停止延迟测试 |
| `get_latency_test_status` | 延迟测试状态 |
| `list_accounts` | 列出账号 |
| `switch_account` | 切换账号 |
| `save_current_as_account` | 保存为账号 |
| `delete_account` | 删除账号 |
| `load_account_config` | 加载账号配置 |
| `get_active_account` | 获取活跃账号 |
| `start_background_check` | 启动后台检测 |
| `stop_background_check` | 停止后台检测 |
| `trigger_background_check` | 触发一次检测 |
| `get_background_status` | 获取后台状态 |
| `enter_login_preparation` | 进入登录准备模式 |
| `get_auto_launch` | 获取自启状态 |
| `set_auto_launch` | 设置自启 |
| `get_notification_enabled` | 获取通知开关 |
| `set_notification_enabled` | 设置通知开关 |
| `send_notification` | 发送通知 |
| `cancel_auto_exit` | 取消自动退出 |
| `get_login_history` | 获取登录历史 |
| `clear_login_history` | 清空登录历史 |
| `get_perf_metrics` | 性能指标 |
| `minimize_window` | 最小化窗口 |
| `close_window` | 关闭窗口 |
| `window_move` | 移动窗口 |
| `open_external` | 打开外部链接 |
| `http_timing_test` | HTTP计时测试 |
| `check_for_updates` | 检查更新 |
| `download_update` | 下载更新 |
| `install_update` | 安装更新 |

### 6.2 事件推送

| 事件名 | 数据 | 触发时机 |
|--------|------|----------|
| `background-check-result` | 在线状态/消息/计数 | 后台检测完成 |
| `auto-login-result` | success/message/skipped? | 自动登录完成 |
| `adapters-changed` | Adapter[] | 适配器变化 |
| `disabled-adapters-changed` | DisabledAdapter[] | 禁用适配器变化 |
| `adapter-disabled-warning` | {name, message} | 适配器被禁用 |
| `login-log` | {message, type} | 登录过程日志 |
| `network-quality-result` | NetworkQuality | 质量检测完成 |
| `auto-exit-countdown` | {delay, shortcut} | 退出倒计时开始 |
| `auto-exit-cancelled` | {} | 退出被取消 |
| `system-notification` | {title, body} | 系统通知 |
| `dhcp-renew-result` | {success, results} | DHCP续租完成 |
| `update-progress` | DownloadProgress | 更新下载进度 |
| `update-available` | UpdateInfo | 发现新版本 |

---

## 七、依赖关系

### 7.1 Rust 依赖 (Cargo.toml)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tauri` | 2 | 应用框架 |
| `tauri-plugin-*` | 2 | shell/notification/autostart/global-shortcut/single-instance |
| `serde` / `serde_json` | 1 | 序列化 |
| `tokio` | 1 (full) | 异步运行时 |
| `reqwest` | 0.12 (json, blocking) | HTTP客户端 |
| `dashmap` | 6 | 并发HashMap (HTTP客户端缓存) |
| `parking_lot` | 0.12 | 高性能同步原语 |
| `arc-swap` | 1 | 原子引用交换 (Config/缓存) |
| `windows` | 0.58 | Win32 API (适配器查询) |
| `surge-ping` | 0.8 | ICMP ping |
| `tokio-rustls` | 0.26 | TLS连接 (DoH/HTTPS) |
| `urlencoding` | 2 | URL编码 |
| `regex` | 1 | 正则表达式 |
| `url` | 2 | URL解析验证 |
| `whoami` | 1.5 | 主机信息 |
| `sysinfo` | 0.33 | 系统信息 |
| `winreg` | 0.52 | 注册表操作 |
| `rand` | 0.8 | 随机数 (重试抖动) |

### 7.2 前端依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| `react` / `react-dom` | 19 | UI框架 |
| `@tauri-apps/api` | ^2 | Tauri前端API |
| `framer-motion` | ^12 | 动画 |
| `lucide-react` | ^0.4 | 图标 |
| `tailwindcss` | ^3.4 | CSS框架 |
| `vite` | ^6 | 构建 |
| `typescript` | ^5.5 | 类型系统 |
| `class-variance-authority` | ^0.7 | 组件变体 |
| Radix UI primitives | 各版本 | 无障碍UI |

### 7.3 依赖关系图 (v2.1.4)

```
main.rs
  ├── lib.rs (Tauri库入口)
  └── commands/mod.rs
        ├── state.rs ← config.rs, arc-swap, dashmap, parking_lot
        ├── config_cmd.rs ← config.rs, crypto_utils.rs, state.rs
        ├── login.rs ← network/*, state.rs, system.rs
        ├── background.rs ← network/*, state.rs, login.rs, system.rs
        ├── network_cmd.rs ← network/*, state.rs
        ├── account.rs ← config.rs, crypto_utils.rs, state.rs, config_cmd.rs
        ├── system.rs ← config.rs, network/*, state.rs, winreg
        └── updater.rs ← reqwest, url

network/
  ├── mod.rs (重导出)
  ├── cache.rs ← arc-swap, lazy_static, dashmap, reqwest
  ├── adapter.rs ← cache.rs, windows, regex
  ├── portal.rs ← cache.rs, reqwest, url
  ├── login_request.rs ← cache.rs, reqwest, urlencoding, regex, rand
  └── quality.rs ← adapter.rs, cache.rs, surge-ping, tokio-rustls

App.tsx
  └── AppStoreProvider (AppStoreContext.tsx)
        ├── useIpc.ts ← @tauri-apps/api
        ├── useLogToast.ts
        ├── useThemeStore.ts
        └── useAppInit.ts
```

---

## 八、安全体系

### 8.1 输入验证层级

| 层级 | 措施 |
|------|------|
| 前端 | 用户名≤64字符, 密码≤128字符 |
| Rust validate_config() | 字段长度/枚举值/正则/URL合法性校验, 非法返回Err |
| validate_username() | 仅允许字母数字_@. |
| validate_operator() | 白名单: ""/@ctcc/@cucc/@cmcc |
| validate_adapter_name_input() | 禁止 /\:*?"<>|\0 |
| is_restricted_ip() | 仅阻止 loopback + link-local, 允许私有IP (校园网内网) |

### 8.2 数据安全

| 措施 | 实现 |
|------|------|
| 密码加密存储 | Windows DPAPI (CryptProtectData), 绑定当前用户 |
| 密码脱敏 | 前端获取配置时替换为 `***` |
| URL白名单 | open_external 仅阻止 loopback/link-local |
| 单实例锁 | tauri-plugin-single-instance |
| 频率限制 | Vec<Instant> + retain (3秒内最多3次登录) |
| 配置验证 | 非法值返回错误而非静默修正 (v2.1.4改进) |
| 错误处理 | 20处 let _ = 改为适当传播/记录 (v2.1.4改进) |

### 8.3 CSP 策略

```
default-src 'self';
script-src 'self';
style-src 'self' 'unsafe-inline';
connect-src 'self' ipc: http://ipc.localhost https://tauri.localhost
            tauri://localhost http://10.1.99.100;
```

---

## 九、性能优化

| 优化项 | 实现 | 效果 |
|--------|------|------|
| 编译优化 | codegen-units=16 + LTO=thin | 二进制体积↓5-10% |
| 适配器缓存 | ArcSwap + TTL 15s | 避免重复Win32 API调用 |
| HTTP客户端缓存 | DashMap + TTL 300s | 按源IP复用客户端 |
| 配置变更检测 | JSON序列化比较后再写盘 | 无变更时跳过加密+IO |
| 并行登录 | tokio::join! | 双适配器同时登录 |
| 并发延迟测试 | tokio::JoinSet (20+目标) | 多目标同时检测 |
| 响应大小限制 | MAX_HTTP_RESPONSE_SIZE=16KB | 防OOM |
| Win32 API | GetAdaptersAddresses (非PowerShell) | 显著快于shell调用 |
| 创建无窗口 | CREATE_NO_WINDOW flag | 无控制台闪烁 |
| 前端防抖 | saveConfigDebounced 300ms | 减少IPC调用 |
| init守卫 | initDoneRef 确保一次性初始化 | 避免重复getInitData调用 |

---

## 十、三大原则遵守情况

### 不要为了安全而安全

| 原违反 | v2.1.4 改进 |
|---------|-------------|
| `is_restricted_ip` 阻止所有私有IP | 仅阻止 loopback + link-local, 允许校园网内网 |
| `open_external` 阻止私有IP | 同上 |
| `gateway.filter` 过滤192.168 | 保留 (用户有路由器场景) |
| `http_timing_test` 拒绝HTTP | 保留 (HTTPS优先策略合理) |

### 不要为了性能而性能

| 原违反 | v2.1.4 改进 |
|---------|-------------|
| codegen-units=256 抵消LTO | 改为16, 让LTO真正生效 |
| 适配器缓存TTL=60s过长 | 改为15s, UI更及时 |
| HTTP客户端双重检查+清理复杂 | 保留 (DashMap本身已足够高效) |

### 不要为了健壮而健壮

| 原违反 | v2.1.4 改进 |
|---------|-------------|
| validate_config静默修正 | 非法值返回Err, 让前端展示错误 |
| 登录等待10s过长 | 保留 (桌面应用可接受) |
| 重试确定性失败 | 保留 (网络环境可能瞬态恢复) |

---

## 十一、异常处理

| 场景 | 处理方式 |
|------|----------|
| 适配器全部无IP | wait_for_adapter() 等待最多10s |
| 门户不可达 | 返回 {reachable: false}, 后台继续尝试 |
| HTTP响应过大 (>16KB) | 截断并返回错误 |
| 登录频率过高 | 3秒冷却期, 3次限制 |
| 密码解密失败 | 清空密码字段, 日志警告 |
| 应用重复启动 | 单实例锁, 第二实例激活已有窗口 |
| 断线重连超限 | 3次后停止, 通知用户 |
| 快捷键注册失败 | 倒计时延长至30s, 重新通知前端 |
| 目录创建失败 | propagate error / log_warn (按上下文) |
| 配置保存失败 | propagate error / log_warn (按上下文) |
| 登录历史写入失败 | log_warn (不中断登录流程) |
| 后台检测启动失败 | log_warn (不阻塞) |

---

*文档版本: v2.1.4 | 基于代码版本: CampusLogin v2.1.4*
