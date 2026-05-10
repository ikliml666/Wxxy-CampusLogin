# CampusLogin 校园网登录助手 — Code Wiki

> **版本**: v2.0.0 | **架构**: Tauri 2 (Rust 后端 + React/TypeScript 前端)
> **目标平台**: Windows (主要), macOS/Linux (部分兼容)
> **通信方式**: Tauri IPC (`invoke` / `listen`)

---

## 一、项目概览

CampusLogin 是一款校园网自动登录助手桌面应用，面向无锡学院校园网认证系统（锐捷 ePortal），提供一键登录、自动重连、网络质量监测、多账号管理等功能。v2.0 从 Electron 架构迁移至 Tauri 2，以 Rust 替代 Node.js 主进程，显著降低资源占用与安装包体积。

### 核心特性

| 特性 | 说明 |
|------|------|
| 一键登录 | 自动检测适配器、DHCP续租、重试登录 |
| 自动重连 | 后台巡检断线检测，最多3次自动重连 |
| 网络质量检测 | 网关/DNS/HTTP/游戏服务器延迟并发测试 |
| 多账号管理 | 加密存储、快速切换 |
| 系统托盘 | 最小化到托盘、快速登录 |
| 开机自启 | 注册表写入 / Tauri 插件 |
| 自动退出 | 登录成功后倒计时退出，快捷键取消 |
| 主题系统 | 7种主题 + 明暗模式 + 自定义颜色 |

---

## 二、项目目录结构

```
CampusLogin/
├── .github/
│   └── workflows/
│       └── ci.yml                    # GitHub Actions CI 配置
├── tauri-app/                        # ★ Tauri 应用主目录
│   ├── package.json                  # 根层依赖 (含 @tauri-apps/cli)
│   ├── build.ps1                     # Windows 构建脚本
│   ├── frontend/                     # React 前端
│   │   ├── package.json              # 前端依赖
│   │   ├── vite.config.ts            # Vite 构建配置
│   │   ├── tailwind.config.js        # Tailwind CSS 配置
│   │   ├── tsconfig.json             # TypeScript 配置
│   │   ├── index.html                # HTML 入口
│   │   ├── public/                   # 静态资源
│   │   └── src/
│   │       ├── main.tsx              # React 入口
│   │       ├── App.tsx               # 根组件
│   │       ├── index.css             # 全局样式
│   │       ├── constants/
│   │       │   └── index.ts          # 常量定义 (ISP/导航/主题/质量等级)
│   │       ├── hooks/
│   │       │   ├── useAppStore.ts    # ★ 核心状态管理 Hook
│   │       │   └── useIpc.ts         # ★ Tauri IPC 封装
│   │       ├── types/
│   │       │   └── index.ts          # TypeScript 类型定义
│   │       ├── lib/
│   │       │   ├── utils.ts          # 工具函数 (safeStorage等)
│   │       │   ├── color.ts          # HEX→HSL 颜色转换
│   │       │   ├── latency.ts        # 延迟等级/颜色/百分比计算
│   │       │   ├── qualityFilter.ts  # 网络质量异常值过滤
│   │       │   └── animations.ts     # Framer Motion 动画变体
│   │       ├── components/
│   │       │   ├── dialogs/          # 对话框 (关于/主题/确认)
│   │       │   ├── layout/           # 布局组件 (标题栏/状态栏/导航/日志)
│   │       │   ├── panels/           # 面板组件 (总览/账号/网络/监控/质量/设置)
│   │       │   ├── shared/           # 共享组件 (延迟组件/刷新按钮)
│   │       │   ├── ui/               # 基础 UI 组件 (shadcn/ui)
│   │       │   └── ErrorBoundary.tsx # 错误边界
│   │       └── vite-env.d.ts
│   └── src-tauri/                    # Rust 后端
│       ├── Cargo.toml                # Rust 依赖
│       ├── tauri.conf.json           # ★ Tauri 应用配置
│       ├── build.rs                  # Tauri 构建脚本
│       ├── capabilities/
│       │   └── default.json          # Tauri 权限声明
│       ├── icons/                    # 应用图标 (多平台多尺寸)
│       └── src/
│           ├── main.rs               # ★ 应用入口
│           ├── lib.rs                # 库模块声明
│           ├── config.rs             # ★ 配置模型与路径
│           ├── network.rs            # ★ 网络核心逻辑
│           ├── crypto_utils.rs       # ★ 加密工具
│           └── commands/
│               ├── mod.rs            # 命令模块声明与导出
│               ├── state.rs          # ★ 全局状态 (AppState)
│               ├── config_cmd.rs     # 配置相关命令
│               ├── login.rs          # ★ 登录命令
│               ├── background.rs     # ★ 后台巡检/适配器监控
│               ├── network_cmd.rs    # 网络相关命令
│               ├── account.rs        # 多账号管理命令
│               └── system.rs         # 系统功能命令
├── config.json                       # 根目录空配置 (遗留)
├── FLOW_DOC.md                       # v1 Electron 流程文档 (参考)
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
│                 状态管理层 (State Layer)              │
│  useAppStore Hook — 集中管理所有前端状态              │
│  useIpc Hook — Tauri IPC 通信封装                    │
├─────────────────────────────────────────────────────┤
│                IPC 通信层 (Bridge Layer)              │
│  Tauri invoke (请求-响应) | Tauri listen (事件推送)   │
│  前端 ←→ Rust 后端                                   │
├─────────────────────────────────────────────────────┤
│                 业务逻辑层 (Logic Layer)              │
│  login.rs — 登录认证                                 │
│  background.rs — 后台巡检/断线重连                    │
│  network.rs — 网络检测/延迟测试                       │
│  account.rs — 多账号管理                              │
│  crypto_utils.rs — 加密解密                          │
│  config.rs — 配置管理                                │
├─────────────────────────────────────────────────────┤
│                 系统交互层 (System Layer)             │
│  PowerShell / cmd.exe — 适配器查询、DHCP续租、ping    │
│  Windows Registry — 开机自启                         │
│  reqwest — HTTP 请求 (校园网认证API)                  │
│  tokio — 异步运行时                                  │
│  rayon — 并行计算 (双适配器登录)                      │
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
                              AppState (共享状态) + 业务逻辑
                                         ↓
                              PowerShell / HTTP / 文件系统
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

1. **线程池配置**: 根据 CPU 核心数动态分配 Rayon (计算线程) 和 Tokio (IO线程) 的工作线程数
2. **Tauri 插件注册**:
   - `tauri-plugin-shell` — 执行外部命令
   - `tauri-plugin-notification` — 系统通知
   - `tauri-plugin-autostart` — 开机自启
   - `tauri-plugin-global-shortcut` — 全局快捷键 (Ctrl+Shift+C 取消自动退出)
   - `tauri-plugin-single-instance` — 单实例锁
3. **Setup 钩子**:
   - 创建数据目录
   - 加载配置 (含密码解密)
   - 根据 `--autostart` 参数和 `hiddenStart` 配置决定是否显示窗口
   - 创建系统托盘 (显示窗口/快速登录/退出)
   - 并行初始化 CryptoKeys 和适配器缓存
   - 启动适配器监控和启动任务 (自动登录/后台检测/延迟测试)
4. **窗口关闭事件**: `minimizeToTray` 为 true 时隐藏而非关闭，退出时清零加密密钥
5. **命令注册**: 28个 `#[tauri::command]` 函数

**关键函数**:

| 函数 | 说明 |
|------|------|
| `main()` | 入口，配置线程池后调用 `run_app()` |
| `run_app()` | 构建 Tauri 应用，注册插件/状态/命令/事件处理 |

### 4.2 全局状态 — `commands/state.rs`

**核心结构体 `AppState`**:

```rust
pub struct AppState {
    pub config: RwLock<Arc<Config>>,           // 应用配置 (读写锁+引用计数)
    pub crypto: PlMutex<Option<CryptoKeys>>,    // 加密密钥 (互斥锁)
    pub crypto_ready: AtomicBool,               // 加密初始化完成标志
    pub crypto_condvar: Condvar,                // 加密就绪条件变量
    pub background_running: AtomicBool,         // 后台检测运行中
    pub background_check_count: AtomicU64,      // 后台检测次数
    pub server_available: AtomicBool,           // 服务器可达
    pub was_online: AtomicBool,                 // 曾经在线
    pub disconnect_reconnect_count: AtomicU32,  // 断线重连计数
    pub latency_running: AtomicBool,            // 延迟测试运行中
    pub auto_exit_deadline: PlMutex<Option<Instant>>, // 自动退出截止时间
    pub is_checking: AtomicBool,                // 正在检测中
    pub is_logging_in: AtomicBool,              // 正在登录中
    pub has_logged_online: AtomicBool,          // 已记录在线状态
    pub last_network_quality: PlMutex<Option<String>>, // 上次网络质量等级
    pub is_quitting: AtomicBool,                // 应用正在退出
    pub cached_online_status: PlMutex<Option<Value>>, // 在线状态缓存
    pub last_login_time: PlMutex<Option<Instant>>,    // 上次登录时间
    pub last_disabled_notification: PlMutex<Option<Instant>>, // 上次禁用通知时间
    pub start_time: Instant,                    // 应用启动时间
    pub rate_limiters: DashMap<String, Instant>, // 命令频率限制器
    pub last_saved_config: PlMutex<Option<String>>, // 上次保存的配置JSON
}
```

**关键常量**:

| 常量 | 值 | 说明 |
|------|----|------|
| `AUTO_EXIT_DELAY_MS` | 5000 | 自动退出倒计时 (毫秒) |
| `CANCEL_EXIT_SHORTCUT` | `CommandOrControl+Shift+C` | 取消退出快捷键 |
| `LOGIN_RATE_LIMIT_MS` | 3000 | 登录频率限制 (毫秒) |

**辅助函数**:

| 函数 | 说明 |
|------|------|
| `check_rate_limit()` | 检查命令调用频率是否超限 |
| `validate_config()` | 校验配置字段合法性 (长度/枚举值/正则) |
| `validate_account_name()` | 校验账号名 (1-32字符, 字母数字下划线中文连字符) |
| `write_file_restricted()` | 写入文件并设置权限 (Windows: DACL, Unix: 0o600) |

**`CommandResult` 结构体**:

```rust
pub struct CommandResult {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}
```

### 4.3 配置管理 — `config.rs`

**`Config` 结构体** (20个字段):

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `user` | String | `""` | 学号 |
| `password` | String | `""` | 密码 (内存中明文, 磁盘上加密) |
| `passwordHmac` | Option\<String\> | None | 密码HMAC校验值 |
| `operator` | String | `""` | 运营商后缀 (`""/@ctcc/@cucc/@cmcc`) |
| `adapter1` | String | `"自动检测"` | 主适配器名称 |
| `adapter2` | String | `""` | 副适配器名称 |
| `dualAdapter` | bool | false | 双适配器模式 |
| `autoLoginOnStart` | bool | false | 启动时自动登录 |
| `autoExitAfterLogin` | bool | false | 登录后自动退出 |
| `minimizeToTray` | bool | true | 关闭时最小化到托盘 |
| `hiddenStart` | bool | false | 静默启动 (不显示窗口) |
| `autoLaunch` | bool | false | 开机自启 |
| `enableBackgroundCheck` | bool | false | 启用后台检测 |
| `backgroundCheckInterval` | u64 | 60000 | 后台检测间隔 (ms) |
| `autoLoginOnPreparation` | bool | false | 登录准备模式 (可登录时自动登录) |
| `autoExitOnOnline` | bool | false | 检测到在线后自动退出 |
| `themeMode` | String | `"dark"` | 主题模式 |
| `enableNotification` | bool | true | 启用通知 |
| `activeAccount` | String | `""` | 当前活跃账号名 |
| `enableLatencyTest` | bool | false | 启用延迟测试 |
| `latencyTestInterval` | u64 | 30000 | 延迟测试间隔 (ms) |
| `customThemeColor` | String | `"#6366f1"` | 自定义主题颜色 |
| `defaultPanel` | String | `""` | 默认面板 |
| `enableNetworkQuality` | bool | true | 启用网络质量检测 |

**路径函数**:

| 函数 | 返回路径 | 说明 |
|------|----------|------|
| `get_data_dir()` | `%APPDATA%/campus-login/` | 应用数据目录 |
| `get_config_path()` | `{data_dir}/config.json` | 配置文件 |
| `get_accounts_dir()` | `{data_dir}/accounts/` | 账号存储目录 |
| `get_login_history_path()` | `{data_dir}/login-history.json` | 登录历史 |

### 4.4 加密工具 — `crypto_utils.rs`

**加密方案**: AES-256-CBC + PBKDF2 密钥派生 + HMAC-SHA256 完整性校验

**`CryptoKeys` 结构体**:

| 字段 | 说明 |
|------|------|
| `hmac_key` | HMAC签名密钥 (SHA256派生) |
| `pbkdf2_key` | PBKDF2加密密钥 (惰性初始化) |

**密钥派生链路**:

```
hostname + username + os_name + machine_id + salt
    ↓ SHA256
encryption_key
    ↓ SHA256(key_hex + "-hmac-" + salt)
hmac_key

hostname + username + salt + salt
    ↓ PBKDF2-HMAC-SHA256 (1000次迭代)
pbkdf2_key (32字节)
```

**`machine_id` 获取**:
- Windows: PowerShell `(Get-CimInstance Win32_ComputerSystemProduct).UUID`
- Linux: `/etc/machine-id` 或 `/var/lib/dbus/machine-id`

**加密/解密流程**:

```
加密:
  plaintext → 随机IV(16字节) → AES-256-CBC-PKCS7加密 → IV+ciphertext
  → HMAC-SHA256签名 → Base64(IV+ciphertext) + HMAC

解密:
  Base64解码 → HMAC校验 → 提取IV → AES-256-CBC-PKCS7解密 → plaintext
```

**安全措施**:
- `Drop` trait 实现密钥清零 (`zero_keys()`)
- Salt 文件使用 `write_file_restricted()` 写入 (限制权限)
- HMAC 校验防止数据篡改

**`EncryptedData` 结构体**:

```rust
pub struct EncryptedData {
    pub data: String,    // Base64(IV + ciphertext)
    pub hmac: String,    // HMAC-SHA256 hex
}
```

### 4.5 网络核心 — `network.rs`

**职责**: 适配器管理、门户检测、登录请求、延迟测试、DHCP续租

**核心数据结构**:

| 结构体 | 字段 | 说明 |
|--------|------|------|
| `Adapter` | name, ip, wireless | 网络适配器 |
| `AdapterDetail` | name, ip, wireless, subnet_mask, gateway, dhcp_server | 适配器详情 |
| `DisabledAdapter` | name, status, description | 已禁用适配器 |
| `PortalStatus` | reachable, login_available, online, message, data_length | 门户认证状态 |
| `NetworkQualityResult` | gateway_latency, external_latency, average_external_latency, gateway, quality, timestamp, details, metrics | 网络质量结果 |

**缓存机制** (lazy_static + RwLock):

| 缓存 | TTL | 说明 |
|------|-----|------|
| `ADAPTERS_CACHE` | 60s | 适配器列表 |
| `ADAPTER_DETAILS_CACHE` | 90s | 适配器详情 |
| `DISABLED_ADAPTERS_CACHE` | 30s | 已禁用适配器 |
| `GATEWAY_CACHE` | 120s | 网关IP |
| `HTTP_CLIENT_CACHE` | 无过期 | HTTP客户端 (按local_addr缓存) |

**适配器过滤规则**:

- **黑名单正则**: Hyper-V, VMware, Docker, WSL, VPN, Bluetooth, Tailscale, ZeroTier 等
- **虚拟后缀**: 包含 `#数字` 或 ` 数字` 的适配器名
- **无效IP**: 169.254.x.x (APIPA) 地址被过滤

**关键函数**:

| 函数 | 说明 |
|------|------|
| `get_adapters_cached()` | 获取适配器列表 (带缓存) |
| `get_adapters_powershell()` | 通过 PowerShell `Get-NetAdapter` 查询适配器 |
| `get_disabled_adapters_cached()` | 获取已禁用适配器 (带缓存) |
| `enable_adapter_powershell()` | 通过 PowerShell `Enable-NetAdapter` 启用适配器 |
| `get_adapter_details_cached()` | 获取适配器详情 (含子网掩码/网关/DHCP) |
| `check_portal_full()` | ★ 检测门户认证状态 (访问 `http://10.1.99.100/`) |
| `do_login_request()` | ★ 同步登录请求 |
| `do_login_request_async()` | ★ 异步登录请求 |
| `do_login_with_retry()` | 带重试的登录 (指数退避: 1s, 2s) |
| `parse_login_result()` | 解析登录响应 (dr1003 JSON回调) |
| `dhcp_renew()` | DHCP续租单个适配器 |
| `dhcp_renew_wired_only()` | DHCP续租所有有线适配器 |
| `select_adapter()` | 根据配置选择适配器 (优先指定→有线→任意) |
| `resolve_adapter_names()` | 解析主/副适配器名称 |
| `get_gateway_ip_cached()` | 获取网关IP (ipconfig→PowerShell→推断.254) |
| `ping_host()` | ICMP ping (通过 cmd.exe) |
| `check_network_quality_async()` | ★ 网络质量检测 (并发多目标) |
| `race_latency_async()` | ICMP/TCP竞速延迟检测 |
| `check_tcp_latency_async()` | TCP连接延迟检测 |
| `check_http_latency_async()` | HTTP请求延迟检测 |
| `check_dns_latency_async()` | DNS解析延迟检测 |
| `wait_for_adapter()` | 等待适配器就绪 (最多30s, 3次连续空则放弃) |
| `validate_adapter_ip()` | 校验适配器IP (排除回环/组播/APIPA) |
| `sanitize_shell_arg()` | Shell参数消毒 (仅允许字母数字连字符下划线点中文) |
| `is_allowed_external_host()` | 外部域名白名单检查 |

**门户检测逻辑** (`check_portal_full`):

```
GET http://10.1.99.100/ (绑定adapter_ip)
  ├─ 不可达 → { reachable: false }
  ├─ 重定向到非白名单主机 → { reachable: false, message: "重定向目标不被允许" }
  └─ 可达 → 分析响应内容:
       ├─ login_available = 包含 eportal/login/portal/dr1003
       ├─ online = 包含 uid='xxx' 且 oltime= 且 uid!=''
       └─ 返回 PortalStatus
```

**登录请求URL**:

```
http://10.1.99.100:801/eportal/portal/login?
  callback=dr1003
  &login_method=1
  &user_account={学号}{运营商后缀}
  &user_password={密码}
  &wlan_user_mac=000000000000
  &jsVersion=4.1.3
  &terminal_type=1
  &lang=zh-cn
```

**网络质量检测目标**:

| 类型 | 目标 | 端口/URL | 超时 |
|------|------|----------|------|
| 网关 | 默认网关 | 80, 53 | 1000ms |
| DNS | 阿里DNS 223.5.5.5 | 53 | 1500ms |
| DNS | 腾讯DNS 119.29.29.29 | 53 | 1500ms |
| HTTP | 百度 | http://www.baidu.com/ | 1500ms |
| HTTP | 腾讯 | http://www.qq.com/ | 1500ms |
| HTTP | 京东 | http://www.jd.com/ | 1500ms |
| HTTP | 哔哩哔哩 | http://www.bilibili.com/ | 2000ms |
| HTTP | 抖音 | http://www.douyin.com/ | 2000ms |
| HTTP | 腾讯游戏 | http://game.qq.com/ | 2000ms |
| HTTP | 网易游戏 | http://game.163.com/ | 2000ms |
| DNS解析 | www.baidu.com | - | 2000ms |

**质量等级判定**:

| 延迟范围 | 等级 | 标签 |
|----------|------|------|
| 1~50ms | excellent | 良好通畅 |
| 51~200ms | fair | 一般正常 |
| >200ms | bad | 拥挤卡顿 |

**外部域名白名单** (`is_allowed_external_host`):

```
10.1.99.100, 10.1.99.99, localhost, 127.0.0.1,
www.baidu.com, www.bilibili.com, www.douyin.com,
www.taobao.com, www.jd.com, www.163.com, weibo.com,
www.zhihu.com, www.qq.com, www.douban.com,
game.qq.com, game.163.com
+ 所有私有IP地址
```

### 4.6 登录模块 — `commands/login.rs`

**核心函数**:

| 函数 | 签名 | 说明 |
|------|------|------|
| `full_login_inner()` | `(state, app_handle) → CommandResult` | ★ 同步完整登录流程 |
| `full_login_inner_async()` | `(state, app_handle) → CommandResult` | ★ 异步完整登录流程 |
| `login_adapter_with_log()` | `(adapter, config, app_handle, skip_portal_check) → Option<CommandResult>` | 单适配器登录 (同步) |
| `login_adapter_with_log_async()` | 同上 (异步) | 单适配器登录 (异步) |
| `do_login` | `#[tauri::command]` | Tauri 命令入口 |

**完整登录流程** (`full_login_inner`):

```
1. 校验用户名/密码非空
2. 获取适配器列表 (缓存→等待→失败)
3. 选择主适配器 (select_adapter)
4. 解析副适配器 (dual_adapter模式)
5. 双适配器模式:
   ├─ rayon::join 并行登录两个适配器
   └─ 任一成功即返回成功
6. 单适配器模式:
   ├─ 无线适配器 → 跳过门户检测，直接登录
   └─ 有线适配器:
       ├─ check_portal_full → 已在线 → 返回成功
       ├─ check_portal_full → 可登录 → do_login_with_retry(3次)
       ├─ check_portal_full → 不可达/不可登录:
       │   ├─ dhcp_renew_wired_only()
       │   ├─ 等待IP变化 (最多6秒)
       │   ├─ 重新选择适配器
       │   ├─ 再次门户检测 → 已在线 → 返回成功
       │   └─ do_login_with_retry(3次)
       └─ 记录登录历史
```

**登录命令** (`do_login`):

```
1. 等待加密初始化就绪
2. 原子交换 is_logging_in (防重入)
3. 登录频率限制检查 (3秒)
4. 清除适配器缓存
5. 调用 full_login_inner_async()
6. 成功后:
   ├─ 清除在线状态缓存
   ├─ 5秒后清除适配器缓存
   ├─ 0.5秒后触发后台检测
   └─ autoExitAfterLogin → 1.5秒后退出
```

### 4.7 后台巡检 — `commands/background.rs`

**关键函数**:

| 函数 | 说明 |
|------|------|
| `run_background_check_blocking()` | ★ 同步后台检测核心逻辑 |
| `run_background_check()` | 异步包装 + 网络质量检测 |
| `start_background_check_inner()` | 启动后台检测循环 |
| `stop_background_check()` | 停止后台检测 |
| `trigger_background_check()` | 手动触发一次检测 (带频率限制) |
| `start_auto_exit()` | 启动自动退出倒计时 |
| `cancel_auto_exit_inner()` | 取消自动退出 |
| `start_adapter_watch()` | ★ 适配器状态监控 (15秒间隔) |
| `spawn_latency_test_loop()` | 延迟测试循环 |
| `run_auto_login_on_start()` | 启动时自动登录 (延迟1.5秒) |
| `run_startup_tasks()` | ★ 启动任务编排 |

**后台检测流程** (`run_background_check_blocking`):

```
1. 防重入检查 (is_checking + is_quitting)
2. 获取适配器列表
3. 解析主/副适配器
4. 并行门户检测 (rayon::join)
5. 发送 background-check-result 事件
6. 更新在线状态缓存
7. 自动登录逻辑:
   ├─ 条件: login_available && !online && autoLoginOnPreparation
   └─ 调用 full_login_inner()
8. 断线重连逻辑:
   ├─ 条件: was_online && any_offline && reachable && autoLoginOnPreparation
   ├─ 计数 ≤ 3 → full_login_inner() 重连
   └─ 计数 > 3 → 通知用户手动处理
9. 首次在线 → start_auto_exit()
10. 返回质量检测信息 (如果需要)
```

**适配器监控** (`start_adapter_watch`):

- 15秒轮询间隔
- 检测适配器列表变化 (名称/IP)
- 检测已禁用适配器变化
- 发送 `adapters-changed` / `disabled-adapters-changed` / `adapter-disabled-warning` 事件
- 通知频率限制 (60秒内不重复通知)

**启动任务** (`run_startup_tasks`):

```
1. enableBackgroundCheck → start_background_check_inner()
2. enableNetworkQuality && enableLatencyTest → spawn_latency_test_loop()
3. autoLoginOnStart → run_auto_login_on_start()
```

### 4.8 配置命令 — `commands/config_cmd.rs`

| 命令 | 说明 |
|------|------|
| `get_config` | 获取配置 (密码脱敏为 `***`) |
| `save_config` | 保存配置 (校验→合并→加密→写盘) |
| `show_window` | 显示并聚焦主窗口 |

**配置保存流程**:

```
1. validate_config() 校验
2. 合并策略:
   ├─ 空用户名 → 保留旧值
   ├─ 密码为 "***" → 保留旧密码
   └─ 空密码且旧密码非空 → 保留旧密码
3. 更新内存中的 config (Arc原子替换)
4. spawn_blocking → save_config_to_disk()
   ├─ 变更检测 (JSON序列化比较)
   ├─ 密码加密 (PBKDF2 + AES-256-CBC)
   ├─ write_file_restricted() 写入
   └─ 更新 last_saved_config
5. 适配器变更 → 清除缓存
6. 延迟测试配置变更 → 重启/停止延迟循环
```

### 4.9 多账号管理 — `commands/account.rs`

| 命令 | 说明 |
|------|------|
| `list_accounts` | 列出所有账号 (扫描 accounts/*.json) |
| `switch_account` | 切换账号 (加载→解密→合并→保存) |
| `save_current_as_account` | 保存当前配置为账号 (加密密码) |
| `delete_account` | 删除账号文件 |
| `load_account_config` | 加载账号配置 (密码脱敏) |
| `get_active_account` | 获取当前活跃账号名 |

**账号存储**: `{data_dir}/accounts/{accountName}.json`

### 4.10 系统功能 — `commands/system.rs`

| 命令 | 说明 |
|------|------|
| `minimize_window` | 最小化窗口 |
| `close_window` | 关闭窗口 (minimizeToTray→隐藏, 否则退出) |
| `window_move` | 移动窗口 (Δ≤5000px, 位置钳制) |
| `open_external` | 打开外部链接 (域名白名单) |
| `get_auto_launch` | 获取开机自启状态 (Windows: 注册表) |
| `set_auto_launch` | 设置开机自启 (Windows: 注册表写入 `--autostart` 参数) |
| `get_notification_enabled` | 获取通知开关 |
| `set_notification_enabled` | 设置通知开关 |
| `send_notification` | 发送系统通知 (Tauri Notification插件) |
| `cancel_auto_exit` | 取消自动退出 |
| `get_login_history` | 获取登录历史 (最多100条) |
| `clear_login_history` | 清空登录历史 |
| `get_perf_metrics` | 获取性能指标 (运行时间/内存/CPU/状态) |

**开机自启实现**:
- Windows: 写入 `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\CampusLogin`
- 其他: 使用 `tauri-plugin-autostart`

**登录历史记录格式**:

```json
{
  "time": "2026-05-07 10:30:00",
  "success": true,
  "message": "以太网 登录成功",
  "adapter": "以太网",
  "user": "20240001",
  "type": "login"
}
```

`type` 取值: `login` (手动登录) / `reconnect` (断线重连)

### 4.11 网络命令 — `commands/network_cmd.rs`

| 命令 | 说明 |
|------|------|
| `get_adapters` | 获取适配器列表 |
| `get_disabled_adapters` | 获取已禁用适配器 |
| `enable_adapter` | 启用适配器 |
| `get_adapter_details` | 获取适配器详情 |
| `is_online` | 检测指定IP是否在线 |
| `check_portal_status` | 检测门户认证状态 |
| `dhcp_renew_all` | DHCP续租所有有线适配器 |
| `check_network_quality` | 网络质量检测 (带频率限制) |
| `start_latency_test` | 启动延迟测试循环 |
| `stop_latency_test` | 停止延迟测试 |
| `get_latency_test_status` | 获取延迟测试状态 |

---

## 五、前端模块详解 (React/TypeScript)

### 5.1 入口 — `main.tsx`

- 初始化主题 (从 localStorage 读取明暗模式和主题名)
- 使用 `LazyMotion` + `domAnimation` 优化 Framer Motion 性能
- 开发模式使用 `React.StrictMode`，生产模式不使用
- 包裹 `ErrorBoundary` 错误边界

### 5.2 IPC 通信层 — `hooks/useIpc.ts`

**`TauriApi` 接口**: 定义了 40+ 个方法，分为两类:

**请求-响应 (invoke)**:

| 方法 | 对应 Rust 命令 | 说明 |
|------|----------------|------|
| `getConfig()` | `get_config` | 获取配置 |
| `saveConfig(config)` | `save_config` | 保存配置 |
| `getAdapters()` | `get_adapters` | 获取适配器 |
| `getDisabledAdapters()` | `get_disabled_adapters` | 获取禁用适配器 |
| `enableAdapter(name)` | `enable_adapter` | 启用适配器 |
| `getAdapterDetails()` | `get_adapter_details` | 获取适配器详情 |
| `checkPortalStatus(ip)` | `check_portal_status` | 门户状态检测 |
| `doLogin()` | `do_login` | 执行登录 |
| `listAccounts()` | `list_accounts` | 列出账号 |
| `switchAccount(name)` | `switch_account` | 切换账号 |
| `saveCurrentAsAccount(name)` | `save_current_as_account` | 保存为账号 |
| `deleteAccount(name)` | `delete_account` | 删除账号 |
| `getActiveAccount()` | `get_active_account` | 获取活跃账号 |
| `startBackgroundCheck()` | `start_background_check` | 启动后台检测 |
| `stopBackgroundCheck()` | `stop_background_check` | 停止后台检测 |
| `triggerBackgroundCheck()` | `trigger_background_check` | 触发一次检测 |
| `getBackgroundStatus()` | `get_background_status` | 获取后台状态 |
| `dhcpRenewAll()` | `dhcp_renew_all` | DHCP续租 |
| `checkNetworkQuality(bypass)` | `check_network_quality` | 网络质量检测 |
| `startLatencyTest()` | `start_latency_test` | 启动延迟测试 |
| `stopLatencyTest()` | `stop_latency_test` | 停止延迟测试 |
| `getLatencyTestStatus()` | `get_latency_test_status` | 延迟测试状态 |
| `openExternal(url)` | `open_external` | 打开外部链接 |
| `getAutoLaunch()` | `get_auto_launch` | 获取自启状态 |
| `setAutoLaunch(enabled)` | `set_auto_launch` | 设置自启 |
| `getNotificationEnabled()` | `get_notification_enabled` | 获取通知开关 |
| `setNotificationEnabled(enabled)` | `set_notification_enabled` | 设置通知开关 |
| `sendNotification(title, body)` | `send_notification` | 发送通知 |
| `cancelAutoExit()` | `cancel_auto_exit` | 取消自动退出 |
| `minimizeWindow()` | `minimize_window` | 最小化窗口 |
| `closeWindow()` | `close_window` | 关闭窗口 |
| `windowMove(dx, dy)` | `window_move` | 移动窗口 |
| `showWindow()` | `show_window` | 显示窗口 |

**事件监听 (listen)**:

| 方法 | 事件名 | 说明 |
|------|--------|------|
| `onBackgroundCheckResult(cb)` | `background-check-result` | 后台检测结果 |
| `onAutoLoginResult(cb)` | `auto-login-result` | 自动登录结果 |
| `onAdaptersChanged(cb)` | `adapters-changed` | 适配器变化 |
| `onDisabledAdaptersChanged(cb)` | `disabled-adapters-changed` | 禁用适配器变化 |
| `onAdapterDisabledWarning(cb)` | `adapter-disabled-warning` | 适配器禁用警告 |
| `onLoginLog(cb)` | `login-log` | 登录日志 |
| `onNetworkQualityResult(cb)` | `network-quality-result` | 网络质量结果 |
| `onAutoExitCountdown(cb)` | `auto-exit-countdown` | 自动退出倒计时 |
| `onAutoExitCancelled(cb)` | `auto-exit-cancelled` | 自动退出取消 |
| `onSystemNotification(cb)` | `system-notification` | 系统通知 |

### 5.3 状态管理 — `hooks/useAppStore.ts`

**核心状态**:

| 状态 | 类型 | 说明 |
|------|------|------|
| `config` | Config | 应用配置 |
| `adapters` | Adapter[] | 网络适配器列表 |
| `disabledAdapters` | DisabledAdapter[] | 已禁用适配器 |
| `adapterDetails` | AdapterDetail[] | 适配器详情 |
| `accounts` | string[] | 账号列表 |
| `activeAccount` | string | 当前活跃账号 |
| `activePanel` | PanelName | 当前面板 |
| `status` | { text, state } | 状态栏文本和状态 |
| `logs` | LogEntry[] | 运行日志 (最多300条) |
| `isLoggingIn` | boolean | 登录进行中 |
| `notificationEnabled` | boolean | 通知开关 |
| `themeName` | ThemeName | 主题名称 |
| `isLightMode` | boolean | 明暗模式 |
| `autoLaunch` | boolean | 开机自启 |
| `bgStatus` | BackgroundStatus | 后台检测状态 |
| `networkQuality` | NetworkQuality | 网络质量数据 |
| `toasts` | ToastMessage[] | Toast通知队列 |
| `sidebarCollapsed` | boolean | 侧栏折叠 |
| `isRefreshingQuality` | boolean | 质量刷新中 |
| `passwordSaved` | boolean | 密码已保存 |

**核心方法**:

| 方法 | 说明 |
|------|------|
| `loadConfig()` | 初始化加载 (配置/适配器/账号/自启/通知/后台状态/质量) |
| `doLogin()` | 执行登录 (保存配置→调用API→更新状态→触发质量检测) |
| `updateConfig(partial)` | 更新配置 (防抖500ms自动保存) |
| `checkOnline(cfg?, adps?)` | 检测在线状态 |
| `refreshQuality()` | 刷新网络质量 (带锁防重入) |
| `addLog(message, type)` | 添加日志 |
| `addToast(title, type, desc, duration)` | 添加Toast通知 |
| `applyQualityFilter(data)` | 应用网络质量异常值过滤 |

**事件监听器** (useEffect):

| 事件 | 处理 |
|------|------|
| `background-check-result` | 更新日志+后台状态 |
| `auto-login-result` | 记录自动登录结果 |
| `adapters-changed` | 更新适配器列表+详情 |
| `disabled-adapters-changed` | 更新禁用适配器 |
| `adapter-disabled-warning` | Toast+日志警告 |
| `login-log` | 添加登录日志 |
| `network-quality-result` | 过滤+更新网络质量+拥堵告警 |
| `auto-exit-countdown` | 日志+Toast倒计时 |
| `auto-exit-cancelled` | 日志+Toast取消 |
| `system-notification` | Toast+日志 |
| `Ctrl+Shift+C` 键盘 | 取消自动退出 |

### 5.4 类型定义 — `types/index.ts`

| 类型 | 说明 |
|------|------|
| `Config` | 应用配置 (20字段) |
| `Adapter` | 网络适配器 |
| `DisabledAdapter` | 已禁用适配器 |
| `AdapterDetail` | 适配器详情 (含子网/网关/DHCP) |
| `LogEntry` | 日志条目 |
| `NetworkQuality` | 网络质量 |
| `NetworkQualityDetail` | 质量检测详情 |
| `NetworkQualityMetrics` | 质量检测指标 |
| `BackgroundStatus` | 后台检测状态 |
| `AdapterOnlineStatus` | 适配器在线状态 |
| `ToastMessage` | Toast通知 |
| `StatusState` | 状态枚举 (loading/online/offline/error) |
| `PanelName` | 面板名 (dashboard/account/network/monitor/quality/settings) |
| `ThemeName` | 主题名 (default/vibrant/forest/midnight/ocean/cherry/custom) |
| `NetworkQualityLevel` | 质量等级 (excellent/fair/bad/unknown) |

### 5.5 工具库

| 文件 | 关键函数 | 说明 |
|------|----------|------|
| `lib/utils.ts` | `safeStorage` | localStorage 安全封装 (try-catch) |
| `lib/color.ts` | `hexToHsl()` | HEX颜色转HSL |
| `lib/latency.ts` | `getLatencyLevel()`, `getLatencyColor()`, `getLatencyPercent()`, `extractGatewayLatency()`, `extractExternalLatency()` | 延迟等级/颜色/百分比计算 |
| `lib/qualityFilter.ts` | `filterNetworkQuality()` | 异常值过滤 (网关>800ms/外网>5000ms 使用前值) |
| `lib/animations.ts` | `containerVariants`, `itemVariants`, `listItemVariants` | Framer Motion 动画变体 |

### 5.6 UI 组件

**布局组件** (`components/layout/`):

| 组件 | 说明 |
|------|------|
| `TitleBar` | 自定义标题栏 (通知/主题/关于/明暗/最小化/关闭) |
| `StatusBar` | 状态栏 (在线状态+网络质量摘要+门户链接+刷新) |
| `NavSidebar` | 导航侧栏 (6个面板入口+折叠) |
| `LogSidebar` | 日志侧栏 (日志列表+登录按钮+适配器信息) |
| `ToastContainer` | Toast通知容器 |

**面板组件** (`components/panels/`):

| 组件 | 说明 |
|------|------|
| `DashboardPanel` | 总览面板 (在线状态/适配器/账号/后台检测/质量) |
| `AccountPanel` | 账号管理 (学号/密码/运营商/适配器/多账号) |
| `NetworkPanel` | 网络适配器 (适配器列表/详情/禁用适配器/DHCP) |
| `MonitorPanel` | 网络状态检测 (后台检测开关/间隔/手动触发) |
| `QualityPanel` | 网络质量 (延迟仪表盘/详细指标/延迟测试) |
| `SettingsPanel` | 系统设置 (主题/明暗/自启/通知/退出行为) |

**UI 基础组件** (`components/ui/`): 基于 shadcn/ui + Radix UI

| 组件 | 基础 |
|------|------|
| animated-card | Card + Framer Motion |
| badge | CVA variant |
| button | Radix Slot + CVA |
| card | HTML div |
| dialog | Radix Dialog |
| input | HTML input + CVA |
| label | Radix Label |
| select | Radix Select |
| separator | Radix Separator |
| switch | Radix Switch |
| tooltip | Radix Tooltip |

---

## 六、IPC 通信完整清单

### 6.1 请求-响应命令 (invoke → #[tauri::command])

| 命令名 | 前端方法 | 参数 | 返回值 | 说明 |
|--------|----------|------|--------|------|
| `get_config` | `getConfig()` | - | Config | 获取配置 (密码脱敏) |
| `save_config` | `saveConfig(config)` | Partial\<Config\> | {success, config} | 保存配置 |
| `show_window` | `showWindow()` | - | void | 显示窗口 |
| `do_login` | `doLogin()` | - | CommandResult | 执行登录 |
| `get_adapters` | `getAdapters()` | - | Adapter[] | 获取适配器 |
| `get_disabled_adapters` | `getDisabledAdapters()` | - | DisabledAdapter[] | 获取禁用适配器 |
| `enable_adapter` | `enableAdapter(name)` | adapterName | CommandResult | 启用适配器 |
| `get_adapter_details` | `getAdapterDetails()` | - | AdapterDetail[] | 获取适配器详情 |
| `is_online` | - | adapterIp | boolean | 检测在线 |
| `check_portal_status` | `checkPortalStatus(ip)` | adapterIp | {online, message} | 门户状态 |
| `dhcp_renew_all` | `dhcpRenewAll()` | - | {success, results} | DHCP续租 |
| `check_network_quality` | `checkNetworkQuality(bypass)` | bypassCache | NetworkQuality | 网络质量 |
| `start_latency_test` | `startLatencyTest()` | - | CommandResult | 启动延迟测试 |
| `stop_latency_test` | `stopLatencyTest()` | - | CommandResult | 停止延迟测试 |
| `get_latency_test_status` | `getLatencyTestStatus()` | - | {enabled, isRunning, interval} | 延迟测试状态 |
| `list_accounts` | `listAccounts()` | - | string[] | 列出账号 |
| `switch_account` | `switchAccount(name)` | accountName | {success, config} | 切换账号 |
| `save_current_as_account` | `saveCurrentAsAccount(name)` | accountName | {success, activeAccount, config} | 保存为账号 |
| `delete_account` | `deleteAccount(name)` | accountName | boolean | 删除账号 |
| `load_account_config` | - | accountName | Option\<Config\> | 加载账号配置 |
| `get_active_account` | `getActiveAccount()` | - | string | 获取活跃账号 |
| `start_background_check` | `startBackgroundCheck()` | - | CommandResult | 启动后台检测 |
| `stop_background_check` | `stopBackgroundCheck()` | - | CommandResult | 停止后台检测 |
| `trigger_background_check` | `triggerBackgroundCheck()` | - | CommandResult | 触发一次检测 |
| `get_background_status` | `getBackgroundStatus()` | - | BackgroundStatus | 获取后台状态 |
| `enter_login_preparation` | - | - | CommandResult | 进入登录准备模式 |
| `get_auto_launch` | `getAutoLaunch()` | - | {enabled} | 获取自启状态 |
| `set_auto_launch` | `setAutoLaunch(enabled)` | enabled | {success, message} | 设置自启 |
| `get_notification_enabled` | `getNotificationEnabled()` | - | boolean | 获取通知开关 |
| `set_notification_enabled` | `setNotificationEnabled(enabled)` | enabled | boolean | 设置通知开关 |
| `send_notification` | `sendNotification(title, body)` | title, body | boolean | 发送通知 |
| `cancel_auto_exit` | `cancelAutoExit()` | - | CommandResult | 取消自动退出 |
| `get_login_history` | - | - | Vec\<Value\> | 获取登录历史 |
| `clear_login_history` | - | - | boolean | 清空登录历史 |
| `get_perf_metrics` | - | - | Value | 性能指标 |
| `minimize_window` | `minimizeWindow()` | - | void | 最小化窗口 |
| `close_window` | `closeWindow()` | - | void | 关闭窗口 |
| `window_move` | `windowMove(dx, dy)` | deltaX, deltaY | void | 移动窗口 |
| `open_external` | `openExternal(url)` | url | boolean | 打开外部链接 |

### 6.2 事件推送 (emit → listen)

| 事件名 | 方向 | 数据类型 | 触发时机 |
|--------|------|----------|----------|
| `background-check-result` | Rust→前端 | {serverAvailable, loginAvailable, online, message, secondaryOnline, secondaryMessage, timestamp, checkCount} | 每次后台检测完成 |
| `auto-login-result` | Rust→前端 | {success, message, skipped?} | 自动登录完成 |
| `adapters-changed` | Rust→前端 | Adapter[] | 适配器列表变化 |
| `disabled-adapters-changed` | Rust→前端 | DisabledAdapter[] | 禁用适配器变化 |
| `adapter-disabled-warning` | Rust→前端 | {name, message} | 配置的适配器被禁用 |
| `login-log` | Rust→前端 | {message, type} | 登录过程日志 |
| `network-quality-result` | Rust→前端 | NetworkQuality | 网络质量检测完成 |
| `auto-exit-countdown` | Rust→前端 | {delay, shortcut} | 自动退出倒计时开始 |
| `auto-exit-cancelled` | Rust→前端 | {} | 自动退出被取消 |
| `system-notification` | Rust→前端 | {title, body} | 系统通知 |
| `dhcp-renew-result` | Rust→前端 | {adapters: string[]} | DHCP续租完成 |

---

## 七、依赖关系

### 7.1 Rust 依赖 (Cargo.toml)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tauri` | 2 | 应用框架 (tray-icon, image-png, image-ico features) |
| `tauri-plugin-shell` | 2 | 执行外部命令 |
| `tauri-plugin-notification` | 2 | 系统通知 |
| `tauri-plugin-autostart` | 2 | 开机自启 |
| `tauri-plugin-global-shortcut` | 2 | 全局快捷键 |
| `tauri-plugin-single-instance` | 2 | 单实例锁 |
| `serde` / `serde_json` | 1 | 序列化/反序列化 |
| `tokio` | 1 (full) | 异步运行时 |
| `rayon` | 1.10 | 并行计算 (双适配器登录) |
| `dashmap` | 6 | 并发 HashMap (频率限制器/HTTP客户端缓存) |
| `parking_lot` | 0.12 | 高性能同步原语 (RwLock/Mutex/Condvar) |
| `reqwest` | 0.12 (json, blocking) | HTTP 客户端 |
| `hex` | 0.4 | 十六进制编解码 |
| `dirs` | 6 | 系统目录路径 |
| `lazy_static` | 1.5 | 静态变量延迟初始化 |
| `base64` | 0.22 | Base64 编解码 |
| `hmac` | 0.12 | HMAC 签名 |
| `sha2` | 0.10 | SHA-256 哈希 |
| `aes` | 0.8 | AES 加密 |
| `cbc` | 0.1 | CBC 模式 |
| `pbkdf2` | 0.12 | PBKDF2 密钥派生 |
| `rand` | 0.8 | 随机数生成 |
| `whoami` | 1.5 | 获取主机名/用户名/平台 |
| `regex` | 1 | 正则表达式 |
| `urlencoding` | 2 | URL 编码 |
| `chrono` | 0.4 | 时间日期 |
| `open` | 5 | 打开外部链接 |
| `url` | 2 | URL 解析 |
| `sysinfo` | 0.33 | 系统信息 (CPU/内存) |
| `winreg` | 0.52 | Windows 注册表操作 (仅Windows) |

### 7.2 前端依赖 (package.json)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `react` / `react-dom` | 19 | UI 框架 |
| `@tauri-apps/api` | ^2 | Tauri 前端 API |
| `@tauri-apps/plugin-*` | ^2 | Tauri 插件前端绑定 |
| `@radix-ui/react-*` | 各版本 | 无障碍 UI 原语 |
| `framer-motion` | ^12.38 | 动画库 |
| `lucide-react` | ^0.446 | 图标库 |
| `class-variance-authority` | ^0.7 | 组件变体 |
| `clsx` | ^2.1 | 条件类名 |
| `tailwind-merge` | ^2.5 | Tailwind 类名合并 |
| `tailwindcss` | ^3.4 | CSS 框架 |
| `vite` | ^6 | 构建工具 |
| `typescript` | ^5.5 | 类型系统 |

### 7.3 依赖关系图

```
main.rs
  ├── commands/mod.rs
  │     ├── state.rs ← config.rs, crypto_utils.rs, dashmap, parking_lot
  │     ├── config_cmd.rs ← config.rs, crypto_utils.rs, state.rs
  │     ├── login.rs ← network.rs, state.rs, system.rs
  │     ├── background.rs ← network.rs, state.rs, login.rs, system.rs
  │     ├── network_cmd.rs ← network.rs, state.rs
  │     ├── account.rs ← config.rs, crypto_utils.rs, state.rs, config_cmd.rs
  │     └── system.rs ← config.rs, network.rs, state.rs, winreg
  ├── config.rs ← serde
  ├── network.rs ← reqwest, regex, dashmap, parking_lot, urlencoding, tokio
  └── crypto_utils.rs ← aes, cbc, hmac, sha2, pbkdf2, rand, base64, whoami

App.tsx
  ├── hooks/useAppStore.ts
  │     ├── hooks/useIpc.ts ← @tauri-apps/api
  │     ├── lib/qualityFilter.ts
  │     ├── lib/color.ts
  │     ├── lib/utils.ts
  │     └── constants/index.ts
  ├── components/layout/* ← shadcn/ui, framer-motion
  ├── components/panels/* ← shadcn/ui, framer-motion, lucide-react
  └── components/dialogs/* ← shadcn/ui
```

---

## 八、安全体系

### 8.1 输入验证层级

| 层级 | 位置 | 措施 |
|------|------|------|
| 前端 | `useAppStore.updateConfig()` | 用户名≤64字符, 密码≤128字符 |
| Rust命令 | `validate_config()` | 字段长度/枚举值/正则校验 |
| 业务函数 | `validate_username()` | 仅允许字母数字_@. |
| 业务函数 | `validate_operator()` | 白名单: `""/@ctcc/@cucc/@cmcc` |
| 业务函数 | `validate_adapter_ip()` | IPv4严格校验, 排除回环/组播/APIPA |
| 业务函数 | `validate_adapter_name_input()` | 禁止 `/\:*?"<>|\0` |
| 业务函数 | `sanitize_shell_arg()` | 仅允许字母数字-_.中文 |
| 业务函数 | `validate_ping_host()` | 主机名/IP格式校验 |

### 8.2 数据安全

| 措施 | 实现 |
|------|------|
| 密码加密存储 | AES-256-CBC + PBKDF2 密钥派生 (1000次迭代) |
| 完整性校验 | HMAC-SHA256 签名, 解密时验证 |
| 文件权限 | Windows: DACL 限制, Unix: 0o600 |
| 密钥清零 | `Drop` trait + 退出时 `crypto = None` |
| 密码脱敏 | 前端获取配置时密码替换为 `***` |
| URL白名单 | `open_external` 和 HTTP 重定向检查 |
| 单实例锁 | `tauri-plugin-single-instance` |
| 频率限制 | `DashMap<命令, 上次调用时间>` |

### 8.3 CSP 策略 (tauri.conf.json)

```
default-src 'self'; 
script-src 'self'; 
style-src 'self' 'unsafe-inline'; 
connect-src 'self' ipc: http://ipc.localhost https://tauri.localhost 
            tauri://localhost http://10.1.99.100 http://10.1.99.99;
```

---

## 九、性能优化

| 优化项 | 实现 | 效果 |
|--------|------|------|
| 线程池调优 | CPU核心数动态分配 Rayon/Tokio 线程数 | 避免过度竞争 |
| 线程亲和性 | `SetThreadIdealProcessor` 绑核 | 减少上下文切换 |
| HTTP连接池 | `pool_max_idle_per_host=2`, `pool_idle_timeout=60s` | 连接复用 |
| 适配器缓存 | `RwLock<Cache>` + TTL (30~90秒) | 避免重复PowerShell查询 |
| 网关缓存 | TTL 120秒 | 避免重复ipconfig |
| HTTP客户端缓存 | `DashMap<IpAddr, Client>` | 按源IP复用客户端 |
| 配置变更检测 | JSON序列化比较后再写盘 | 无变更时跳过加密+IO |
| 并行登录 | `rayon::join` / `tokio::join` | 双适配器同时登录 |
| 并发延迟测试 | `tokio::JoinSet` | 多目标同时检测 |
| 响应大小限制 | `MAX_HTTP_RESPONSE_SIZE=1MB` | 防OOM |
| 创建无窗口 | PowerShell/cmd `CREATE_NO_WINDOW` flag | 无控制台闪烁 |
| WebView优化 | `--enable-gpu --disable-background-timer-throttling` | 渲染性能 |
| 前端防抖 | 配置自动保存 500ms 防抖 | 减少IPC调用 |
| 质量过滤 | 异常值使用前值替换 | 避免UI闪烁 |

---

## 十、项目运行方式

### 10.1 开发环境要求

| 工具 | 版本要求 | 说明 |
|------|----------|------|
| Rust | stable (2021 edition) | `rustup` 安装 |
| Node.js | 18.x / 20.x | 前端构建 |
| npm | 随 Node.js | 包管理 |
| Windows SDK | - | WebView2 / 注册表操作 |

### 10.2 开发模式

```bash
# 进入 Tauri 应用目录
cd tauri-app

# 安装依赖
npm install
cd frontend && npm install && cd ..

# 启动开发服务器 (前端热更新 + Rust 自动重编译)
npx tauri dev
```

开发模式下:
- 前端 Vite 开发服务器运行在 `http://localhost:5173`
- Rust 后端修改会自动重编译
- 前端修改会热更新

### 10.3 生产构建

```powershell
# 使用构建脚本
cd tauri-app
.\build.ps1

# 可选参数:
.\build.ps1 -SkipFrontend       # 跳过前端构建
.\build.ps1 -SkipCopy           # 跳过产物复制
.\build.ps1 -TargetDir "C:\short\target"  # 自定义构建目录 (避免长路径问题)
```

构建流程:
1. 检查环境 (rustc, node, cargo)
2. 安装依赖 (npm install)
3. 构建前端 (vite build → `frontend/dist/`)
4. 构建 Tauri 应用 (cargo build --release + 打包 NSIS 安装程序)
5. 复制产物到 `tauri-release/`

### 10.4 CI/CD

GitHub Actions (`.github/workflows/ci.yml`):
- 触发: push/PR 到 main/develop
- 矩阵: Node.js 18.x / 20.x
- 步骤: 依赖安装 → lint → 测试 → 覆盖率 → 构建 → 上传产物

---

## 十一、关键配置文件

### 11.1 tauri.conf.json

| 配置项 | 值 | 说明 |
|--------|----|------|
| `productName` | 校园网登录助手 | 产品名 |
| `identifier` | com.campus.login | 应用标识 |
| `windows[0].width/height` | 960×680 | 默认窗口尺寸 |
| `windows[0].minWidth/minHeight` | 800×600 | 最小窗口尺寸 |
| `windows[0].decorations` | false | 无系统标题栏 (自定义) |
| `windows[0].visible` | false | 初始隐藏 (setup中按需显示) |
| `bundle.targets` | ["nsis"] | 打包为 NSIS 安装程序 |
| `bundle.windows.nsis.installMode` | "both" | 当前用户/所有用户安装 |

### 11.2 Cargo.toml

| 配置项 | 值 | 说明 |
|--------|----|------|
| `package.name` | campus-login | 包名 |
| `lib.name` | campus_login_lib | 库名 |
| `lib.crate-type` | staticlib, cdylib, rlib | 多种编译目标 |

### 11.3 capabilities/default.json

Tauri 2 权限声明，定义前端可调用的命令和可监听的事件。

---

## 十二、异常处理

| 场景 | 处理方式 |
|------|----------|
| 适配器全部无IP | `wait_for_adapter()` 等待最多30s, 3次连续空则放弃 |
| 门户不可达 | 返回 `{reachable: false}`, 后台检测继续尝试 |
| HTTP响应过大 (>1MB) | 截断并返回错误 |
| 登录频率过高 | 3秒冷却期, 返回"请求过于频繁" |
| 密码解密失败 | 清空密码字段, 日志警告 |
| 应用重复启动 | 单实例锁, 第二实例激活已有窗口 |
| 重定向到非白名单 | 拒绝请求, 返回"重定向目标不被允许" |
| 断线重连超限 | 3次后停止, 通知用户手动处理 |
| PowerShell执行失败 | 返回错误信息, 不崩溃 |
| 网络质量异常值 | 前端过滤器使用前值替换 |

---

*文档版本: v2.0 | 基于代码版本: CampusLogin v2.0.0 (Tauri)*
