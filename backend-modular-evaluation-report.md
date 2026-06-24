# Wxxy-CampusLogin 后端模块化评估与优化报告

> 评估范围：`tauri-app/src-tauri/src`（Rust 后端）  
> 评估日期：2026-06-24  
> 优化优先级：可维护性、性能与资源  
> 兼容性策略：允许必要的破坏性调整

---

## 一、现状分析

### 1.1 项目概述

Wxxy-CampusLogin 后端基于 **Tauri v2 + Rust** 构建，负责校园网登录、注销、后台状态监控、网络适配器管理、自动重连、通知推送、配置持久化等职责。当前后端由 9 个顶层模块组成，核心代码约 7,000–8,000 行 Rust。

### 1.2 模块结构与代码规模

| 顶层模块 | 主要职责 | 核心文件 | 代码行数 |
|---|---|---|---|
| `commands` | Tauri Command 入口，对接前端 | `login.rs`, `network_cmd.rs`, `config_cmd.rs`, `system.rs`, `background.rs`, `account.rs`, `updater.rs` | 约 2,200 行 |
| `auth` | 登录/注销协议与认证会话 | `session.rs`, `protocol.rs`, `portal.rs` | 约 900 行 |
| `network` | 适配器发现、HTTP Client、DNS、测速、时延 | `adapter.rs`, `client.rs`, `dns.rs`, `quality.rs`, `timing.rs` | 约 3,000 行 |
| `monitor` | 后台检测、自动登录、延迟测试、适配器监听 | `watcher.rs`, `auto_auth.rs`, `latency.rs`, `adapter_watch.rs` | 约 1,500 行 |
| `infra` | 全局状态、生命周期、日志、通知 | `state.rs`, `lifecycle.rs`, `logger.rs`, `notification.rs` | 约 900 行 |
| `config` | 配置模型、持久化、校验 | `model.rs`, `persist.rs`, `validate.rs` | 约 420 行 |
| `account` | 账号加密 | `crypto.rs` | 约 110 行 |
| `platform` | 平台相关能力（自动启动、DNS、提权、GPU） | `autostart.rs`, `dns_config.rs`, `elevation.rs`, `gpu.rs` | 约 850 行 |
| `update` | 应用更新 | `updater.rs` | 约 260 行 |

> 行数统计基于 2026-06-24 代码快照；`adapter.rs`（1,453 行）、`watcher.rs`（857 行）为当前最大的两个文件。

### 1.3 当前模块职责梳理

```
main.rs
├── commands/          # 前端调用入口
├── auth/              # 认证协议与会话编排
├── network/           # 网络基础设施
├── monitor/           # 后台任务与自动策略
├── infra/             # 全局状态与通用基础设施
├── config/            # 配置模型与持久化
├── account/           # 账号加密
├── platform/          # 平台能力封装
└── update/            # 更新检查与下载
```

从模块命名看，职责边界已基本划分清楚；但在实现层面，**多个模块通过直接读写 `AppState` 的字段形成强耦合**，导致边界模糊。

---

## 二、问题诊断

### 2.1 核心架构问题（P0）

#### P0-1：`AppState` 演变为“上帝对象”

- **位置**：`infra/state.rs`
- **现象**：`AppState` 同时承载配置、任务锁、网络状态、退出状态、更新状态、心跳时间戳等 20+ 字段。
- **影响**：
  - 任何模块都可通过 `app.state::<AppState>()` 访问并修改几乎所有状态。
  - `NetworkStatus`、`ExitState` 被 `auth`、`commands`、`monitor`、`infra::lifecycle` 等多个模块直接修改，违反单一职责原则。
  - 状态变更无统一事件总线，模块间依赖隐式且难以追踪。

#### P0-2：登录/注销逻辑高度重复

- **位置**：`auth/session.rs`、`commands/login.rs`
- **现象**：
  - `full_login_inner` 与 `full_logout_inner` 在适配器解析、双适配器并行、结果合并、失败计数等逻辑上高度相似。
  - `login_adapter_with_log` / `logout_adapter_with_log` 均通过 `adapter_action_with_log` 完成日志、前端事件、历史记录，但函数名与调用路径分散。
  - 双适配器结果合并代码（`a1_success || a2_success`、消息拼接）在登录、注销中各出现一次。
- **影响**：修改登录流程时容易遗漏注销侧，Bug 风险高。

#### P0-3：后台任务管理缺乏统一框架

- **位置**：`monitor/watcher.rs`、`monitor/latency.rs`、`monitor/adapter_watch.rs`、`infra/state.rs`
- **现象**：
  - 每个后台任务都独立维护 `TaskLock` + `CancellationToken` + `running` 标志（`background_running`、`latency_running`、`adapter_watch_running`）。
  - 启动/停止/状态查询逻辑分散在三处，取消信号传播方式不一致。
  - `latency.rs` 中通过 `Arc::ptr_eq` 判断 token 归属后再释放 `latency_running`，属于“补丁式”正确性保证。
- **影响**：新增后台任务成本高；任务泄漏、重复启动、停止后状态不一致的风险大。

### 2.2 模块粒度与耦合问题（P1）

#### P1-1：`network/adapter.rs` 过于庞大

- **位置**：`network/adapter.rs`（1,453 行）
- **现象**：单文件混合适配器发现、状态分类、缓存、DHCP 续租、子网计算、SSID 获取、Windows 注册表遍历、黑名单过滤等。
- **影响**：
  - 单一文件职责过多，阅读和维护成本高。
  - Windows 平台相关代码（注册表、`GetAdaptersAddresses` 等）与通用网络逻辑混在一起，跨平台扩展困难。

#### P1-2：`monitor/watcher.rs` 职责过重

- **位置**：`monitor/watcher.rs`（857 行）
- **现象**：同时负责校园网检测、Portal 检测、状态更新、自动登录触发、断线重连、MAC 重置、网络质量检测调度。
- **影响**：
  - 函数间调用链长，单测困难。
  - 自动登录/重连策略与状态检测耦合，调整策略会影响核心检测逻辑。

#### P1-3：`main.rs` 初始化逻辑臃肿

- **位置**：`main.rs`（402 行）
- **现象**：`setup` 闭包中集中了托盘菜单、全局快捷键、后台任务启动、心跳监控、窗口兜底显示、WebView2 内存控制等。
- **影响**：
  - 模块初始化逻辑未下沉到各自模块，新增插件/后台任务都需要修改 `main.rs`。
  - 启动顺序和依赖关系不清晰。

#### P1-4：命令层存在大量重复样板代码

- **位置**：`commands/login.rs`、`commands/network_cmd.rs`、`commands/config_cmd.rs`、`commands/system.rs` 等
- **现象**：
  - 重复调用 `state.config.load_full()`、`app_h.state::<AppState>()`。
  - 重复执行 `app_handle.emit("login-log", ...)` 和 `emit_notification`。
  - `CommandResult` 的构造与错误包装分散在各命令中。
- **影响**：命令层代码冗长，新增命令需要复制大量样板。

### 2.3 性能与资源问题（P1-P2）

#### P1-5：HTTP Client 同步/异步混用

- **位置**：`network/client.rs`、`auth/protocol.rs`、`auth/portal.rs`、`network/quality.rs`
- **现象**：
  - 使用 `reqwest::blocking::Client` 并在 async 上下文中通过 `tauri::async_runtime::spawn_blocking` 调用。
  - 项目中同时依赖 `tokio-rustls`、`hickory-resolver` 等异步生态，但核心 HTTP 路径却是阻塞式。
- **影响**：
  - 阻塞线程池占用资源，大量并发时可能导致线程数膨胀。
  - 异步运行时与阻塞 Client 的组合增加了心智负担。

#### P1-6：适配器缓存失效策略简单

- **位置**：`network/adapter.rs`
- **现象**：缓存使用 `Mutex<Option<(..., Instant)>>`，TTL 5 秒；强制刷新通过 `ADAPTER_CACHE = None` 实现。
- **影响**：
  - 无并发读取优化（所有读取走同一把锁）。
  - 强制刷新会导致并发请求中的读取方拿到 `None` 后重新查询，存在抖动。

#### P2-1：日志/通知未抽象

- **位置**：`auth/session.rs`、`monitor/watcher.rs`、`monitor/auto_auth.rs`、`commands/login.rs`
- **现象**：`emit("login-log", ...)` 与 `crate::log_*!` 成对出现，散落在业务代码中。
- **影响**：UI 通知与业务逻辑耦合，未来需要更换通知通道或新增埋点时代价高。

#### P2-2：自定义 Tokio Runtime 与 Tauri 默认运行时混合

- **位置**：`main.rs`
- **现象**：手动创建 `tokio::runtime::Builder`，再 `tauri::async_runtime::set(handle)`。
- **影响**：
  - 增加了运行时配置的复杂度。
  - 需要手动处理 runtime shutdown 顺序，容易在退出时引入时序 Bug。

---

## 三、优化方案

### 3.1 设计目标

1. **单一职责**：每个模块/结构体只负责一个清晰的概念。
2. **依赖向内**：业务模块依赖基础设施，基础设施不依赖业务模块。
3. **可测试性**：核心逻辑不依赖 `AppHandle`，可通过 trait 注入依赖。
4. **可扩展性**：新增后台任务、认证协议、平台适配的成本低。
5. **性能合理**：减少阻塞线程使用，统一异步边界，避免重复资源创建。

### 3.2 目标架构

```
src/
├── app/                       # 应用生命周期与模块组装（替代 main.rs 中臃肿 setup）
│   ├── startup.rs             # 启动流程编排
│   ├── tray.rs                # 托盘菜单与事件
│   ├── shortcut.rs            # 全局快捷键
│   └── shutdown.rs            # 优雅退出
│
├── commands/                  # Tauri Command 入口（瘦身，只保留参数解析与转发）
│   ├── login.rs
│   ├── network.rs
│   ├── config.rs
│   ├── system.rs
│   └── ...
│
├── infra/
│   ├── state/
│   │   ├── store.rs           # ConfigStore：配置原子读写
│   │   ├── network.rs         # NetworkState：网络状态快照
│   │   ├── exit.rs            # ExitState：退出/自动退出状态
│   │   └── mod.rs             # AppState 组装（字段大幅精简）
│   ├── tasks.rs               # BackgroundTaskManager：统一后台任务注册/取消/状态
│   ├── events.rs              # EventBus：前端事件抽象
│   ├── notifications.rs       # NotificationService
│   ├── logger.rs
│   └── lifecycle.rs           # 仅保留高层生命周期协调
│
├── auth/
│   ├── service.rs             # AuthService：登录/注销编排
│   ├── adapter_resolver.rs    # 适配器解析（自动检测、双适配器）
│   ├── failure_tracker.rs     # 认证失败计数与 MAC 重置策略
│   ├── portal.rs              # Portal 检测
│   └── protocol.rs            # 校园网认证协议
│
├── network/
│   ├── discovery/             # 适配器发现（拆分自 adapter.rs）
│   │   ├── windows.rs
│   │   └── mod.rs
│   ├── adapter_cache.rs       # 缓存策略
│   ├── dhcp.rs                # DHCP 续租
│   ├── subnet.rs              # 子网/SSID 计算
│   ├── client.rs              # 异步 HTTP Client Pool（统一为 async）
│   ├── dns.rs
│   ├── quality.rs
│   └── timing.rs
│
├── monitor/
│   ├── campus_check.rs        # 校园网环境检测
│   ├── portal_check.rs        # Portal 在线检测
│   ├── auto_login.rs          # 自动登录/重连策略
│   ├── quality_scheduler.rs   # 网络质量调度
│   └── adapter_watch.rs
│
├── config/
├── account/
├── platform/
└── update/
```

### 3.3 关键优化措施

#### 3.3.1 状态管理重构

- **拆分 `AppState`**：
  - `ConfigStore`：只暴露 `load()`、`update(f)`、`subscribe()`。
  - `NetworkState`：只暴露原子读方法和受控写方法（如 `set_online`）。
  - `ExitState`：只由 `lifecycle` 模块操作。
- **引入事件总线**：状态变更后通过 `EventBus` 统一向前端发射事件，业务代码不再直接调用 `app_handle.emit`。
- **禁止跨模块直接写字段**：除 `infra` 内部外，其他模块通过受控 API 修改状态。

#### 3.3.2 统一后台任务框架

```rust
pub struct BackgroundTaskManager {
    registry: DashMap<String, TaskHandle>,
}

pub struct TaskHandle {
    cancel: CancellationToken,
    running: TaskLock,
}

impl BackgroundTaskManager {
    pub async fn start<F>(&self, name: &str, interval: Duration, task: F) -> Result<(), TaskError>
    where F: Fn(CancellationToken) -> BoxFuture<'static, ()> + Send + 'static;

    pub fn stop(&self, name: &str);
    pub fn is_running(&self, name: &str) -> bool;
}
```

- 将 `background_check`、`latency_test`、`adapter_watch` 统一注册到 `BackgroundTaskManager`。
- 删除 `TaskFlags` 中分散的字段，改为通过名称查询任务状态。

#### 3.3.3 认证服务层重构

- 提取 `AuthService`：
  - 接收 `LoginRequest` / `LogoutRequest`。
  - 内部调用 `AdapterResolver`、`PortalChecker`、`ProtocolClient`、`FailureTracker`。
  - 返回 `AuthResult`，由命令层负责转换为 `CommandResult` 和前端事件。
- 提取 `AdapterResolver`：统一处理“自动检测”、双适配器解析、IP 过滤。
- 提取 `FailureTracker`：统一单/双适配器失败计数和 MAC 重置策略，消除 `update_auth_failure_count` 与 `update_dual_adapter_auth_failure` 的重复。
- 登录与注销共享“双适配器执行器”：
  ```rust
  pub async fn execute_on_dual_adapters<F>(a1, a2, action: F) -> DualAdapterResult;
  ```

#### 3.3.4 网络层拆分

- `network/adapter.rs` 拆分为：
  - `network/discovery/mod.rs` + `network/discovery/windows.rs`
  - `network/adapter_cache.rs`
  - `network/dhcp.rs`
  - `network/subnet.rs`
- `network/client.rs`：
  - 逐步迁移到 `reqwest::Client`（异步），在真正需要阻塞的 Windows API 调用处再使用 `spawn_blocking`。
  - Client Pool 使用 `tokio::sync::RwLock` 或 `moka` 等具备 TTL/容量管理的缓存。

#### 3.3.5 命令层瘦身

- 引入 `CommandContext`：
  ```rust
  pub struct CommandContext<'a> {
      pub app: &'a AppHandle,
      pub state: &'a AppState,
  }
  ```
- 提取公共 trait：
  - `AppHandleExt::notify_login_log(msg, level)`
  - `AppHandleExt::notify_adapter_changed(...)`
- 命令函数只负责：参数解析 → 调用 Service → 返回 `CommandResult`。

#### 3.3.6 `main.rs` 初始化下沉

- 将托盘、快捷键、启动任务、心跳监控分别迁移到 `app::tray`、`app::shortcut`、`app::startup`、`app::heartbeat`。
- `main.rs` 只保留：运行时创建 → 插件注册 → 模块初始化 → `app.run()`。

---

## 四、实施步骤

建议分 5 个阶段实施，每个阶段都有独立的编译/运行验证点，降低回归风险。

### Phase 1：低风险重构与公共抽象（1 周）

| 任务 | 目标 | 验证方式 |
|---|---|---|
| T1.1 提取 `EventBus` / `NotificationService` | 将 `emit("login-log", ...)`、`emit_notification` 集中到 `infra` | 全局搜索 `emit("login-log"` 确认只剩一处实现 |
| T1.2 提取 `CommandContext` 与公共 trait | 减少 commands 中重复的状态访问和事件发射 | 统计 commands 中 `state.config.load_full()` 重复次数下降 50% |
| T1.3 统一 `CommandResult` 构造 helper | 减少 `CommandResult { success, message, data }` 手写 | clippy / 编译通过 |
| T1.4 将 `main.rs` 中托盘/快捷键代码下沉到 `app::tray`、`app::shortcut` | `main.rs` 行数降至 150 行以内 | 行数统计 + 功能回归 |

### Phase 2：状态管理与任务框架抽象（1–1.5 周）

| 任务 | 目标 | 验证方式 |
|---|---|---|
| T2.1 拆分 `AppState` 为 `ConfigStore`、`NetworkState`、`ExitState` | 各模块只访问自己需要的状态 | 编译通过 + 检查 `infra/state.rs` 不再被 `auth` 直接写字段 |
| T2.2 设计并实现 `BackgroundTaskManager` | 统一后台任务生命周期 | `cargo test` 新增任务注册/取消/重复启动测试 |
| T2.3 将 `watcher`、`latency`、`adapter_watch` 迁移到新框架 | 删除 `TaskFlags` 中分散的字段 | 运行后通过前端启停后台任务验证 |

### Phase 3：认证服务层重构（1.5–2 周）

| 任务 | 目标 | 验证方式 |
|---|---|---|
| T3.1 提取 `AdapterResolver` | 统一登录/注销的适配器解析 | 登录/注销单测覆盖单/双适配器场景 |
| T3.2 提取 `FailureTracker` | 统一失败计数与 MAC 重置 | 模拟连续失败验证 MAC 重置触发 |
| T3.3 提取 `DualAdapterExecutor` | 登录/注销共享双适配器执行逻辑 | 登录/注销代码重复度下降 |
| T3.4 引入 `AuthService` | `commands/login.rs` 只保留命令入口 | `auth/session.rs` 不再被 commands 直接调用 |

### Phase 4：网络层拆分与异步化（2 周）

| 任务 | 目标 | 验证方式 |
|---|---|---|
| T4.1 拆分 `network/adapter.rs` 为 discovery / cache / dhcp / subnet | 单文件行数 < 400 | 行数统计 |
| T4.2 将 `network/client.rs` 迁移到异步 `reqwest::Client` | 减少 `spawn_blocking` 使用 | 压力测试连接创建次数 |
| T4.3 优化适配器缓存：读写锁 + 后台刷新 | 降低缓存抖动 | 并发读取测试 |
| T4.4 平台相关代码隔离到 `network/discovery/windows.rs` | 非 Windows 平台可替换实现 | 条件编译通过 |

### Phase 5：监控模块职责拆分（1 周）

| 任务 | 目标 | 验证方式 |
|---|---|---|
| T5.1 拆分 `monitor/watcher.rs` 为 campus_check / portal_check / auto_login | 单文件行数 < 400 | 行数统计 |
| T5.2 将 `auto_login` 与 `try_disconnect_reconnect` 策略独立 | 调整自动策略不影响检测核心 | 单测覆盖 |
| T5.3 统一质量检测调度到 `quality_scheduler.rs` | 消除 `latency.rs` 与 `watcher.rs` 中的重复质量检测代码 | 全局搜索 `check_network_quality_async` 调用点 |

---

## 五、风险评估与收益

### 5.1 风险

| 风险 | 级别 | 说明 | 缓解措施 |
|---|---|---|---|
| 前端命令签名变更 | 中 | 若重构 `CommandResult` 或命令参数，前端需同步调整 | Phase 1 先保持命令签名不变，仅内部抽象 |
| 异步化引入运行时 Bug | 中 | `reqwest::blocking` 改 async 可能改变时序 | 逐步迁移，每次只改一个调用点，充分测试 |
| 状态拆分引入竞态 | 高 | `AppState` 拆分后，跨状态一致性需要仔细设计 | 引入状态变更事件总线，所有写操作集中验证 |
| 校园网环境依赖，测试困难 | 中 | 部分逻辑依赖真实校园网 | 增加 mock adapter / mock portal 的测试桩 |
| 跨字段网络状态非原子更新 | 高 | `any_online`、`last_a1_online` 等独立原子变量更新顺序无保障，当前已存在半一致快照 | 拆分后用 `NetworkSnapshot` + `ArcSwap` 保证快照一致性 |
| PowerShell/MAC 操作命令注入与系统影响 | 高 | DHCP/MAC 重置通过 PowerShell 字符串拼接执行，涉及注册表与网卡禁用/启用 | 使用参数数组或平台 API；隔离到独立模块并补充权限/回滚测试 |
| 自定义 Runtime shutdown 时序错误 | 中 | 手动管理 runtime shutdown，后台任务可能未优雅退出 | shutdown 阶段显式等待后台任务 join，或评估使用 Tauri 默认 runtime |

### 5.2 预期收益

| 维度 | 收益 |
|---|---|
| 可维护性 | 单文件职责清晰，代码重复度下降 30% 以上，新增功能成本降低 |
| 可测试性 | 核心服务可脱离 `AppHandle` 进行单元测试，测试覆盖率可显著提升 |
| 性能 | 减少阻塞线程使用，统一连接池，降低后台任务资源占用 |
| 扩展性 | 新增运营商/协议只需扩展 `AuthService`，新增后台任务只需注册到 `BackgroundTaskManager` |
| 稳定性 | 状态变更入口集中，减少竞态和遗漏更新导致的在线状态误判 |

---

## 六、优先级总结

| 优先级 | 问题 | 建议实施阶段 |
|---|---|---|
| P0 | `AppState` 上帝对象 | Phase 2 |
| P0 | 登录/注销逻辑重复 | Phase 3 |
| P0 | 后台任务管理混乱 | Phase 2 |
| P0–P1 | HTTP Client 同步/异步混用（`reqwest::blocking` 内部自建 runtime） | Phase 4 |
| P1 | 跨字段网络状态非原子更新 | Phase 2 |
| P1 | PowerShell/MAC 重置命令注入与系统影响 | Phase 4 |
| P1 | `network/adapter.rs` 过大 | Phase 4 |
| P1 | `monitor/watcher.rs` 职责过重 | Phase 5 |
| P1 | `main.rs` 初始化臃肿 | Phase 1 |
| P1 | 命令层重复样板（含托盘快速登录与 `do_login` 重复） | Phase 1 |
| P1 | 网络质量检测重复触发 | Phase 5 |
| P2 | 日志/通知未抽象 | Phase 1 |
| P2 | 自定义 Tokio Runtime | Phase 1/2 |
| P2 | “自动检测”等中文业务常量硬编码 | Phase 3/4 |
| P2 | 注销未重置认证失败计数 | Phase 3 |
| P2 | 注册表遍历重复，可缓存 | Phase 4 |

---

## 七、结论

当前 Wxxy-CampusLogin 后端在功能层面已经较为完整，但架构层面存在**状态集中、职责边界模糊、代码重复、后台任务管理分散**等系统性问题。经代码审查员复核，报告中的问题诊断基本准确，但低估了以下风险：

- `reqwest::blocking::Client` 内部自建 tokio runtime，极端情况下会加剧线程资源膨胀；
- 当前 `any_online` / `last_a1_online` / `last_a2_online` 等独立原子变量已存在非原子更新的半一致快照风险；
- MAC/DHCP 重置通过 PowerShell 字符串拼接执行，存在命令注入与系统影响风险；
- 后台任务的 `running` flag 释放点分散，停止后立刻启动可能出现新旧任务并发。

建议按照“先抽象公共能力，再拆分状态与任务，最后重构业务核心”的顺序推进优化，并在 Phase 1 优先建立可复现的回归测试基线。

最优先处理的三项是：

1. **拆分 `AppState` 为 `ConfigStore` / `NetworkState` / `ExitState`，并用 `NetworkSnapshot` 保证网络状态快照一致性**；
2. **建立统一的后台任务管理框架（`TaskHandle` 持有 `JoinHandle`，停止时 `cancel + await`）**；
3. **将登录/注销流程抽象为 `AuthService` + `AdapterResolver` + `FailureTracker`，并消除 `spawn_blocking` 内嵌 `std::thread::scope` 的线程资源浪费**。

完成上述三项后，再对 `network` 和 `monitor` 进行粒度拆分与异步化改造，将显著改善代码的可维护性、稳定性和长期演进能力。

---

## 八、代码审查员补充意见

> 审查范围：本报告及 `infra/state.rs`、`auth/session.rs`、`commands/login.rs`、`monitor/watcher.rs`、`monitor/latency.rs`、`network/adapter.rs`、`network/client.rs`、`main.rs` 等核心文件。

### 8.1 审查总体判断

- 报告对现状的诊断与优化方向总体可行；
- 实施计划略显乐观，建议先建立回归测试基线，再按“小步快跑、接口兼容、逐步替换”的节奏推进；
- 部分安全、运行时资源和跨字段一致性问题需要提升优先级。

### 8.2 严重问题补充

#### 8.2.1 后台任务 `running` flag 释放点分散

- **位置**：`commands/background.rs`、`monitor/watcher.rs`、`main.rs`、`monitor/latency.rs`
- **问题**：`stop_background_check` 先 cancel 再立即 `force_release`；`latency.rs` 用 `Arc::ptr_eq` 判断 token 归属再释放 flag。停止后立刻启动可能让旧任务与新任务并发。
- **建议**：`TaskHandle` 同时持有 `cancel: CancellationToken` 与 `handle: JoinHandle<()>`；停止时 `cancel.cancel()` 并 `handle.await`，确认旧任务退出后再允许新任务获得锁。

#### 8.2.2 嵌套 `spawn_blocking` + `std::thread::scope` 导致线程膨胀

- **位置**：`commands/login.rs` → `auth/session.rs`；`commands/login.rs` → 注销双适配器；`monitor/watcher.rs`
- **问题**：async 命令 → `spawn_blocking` → 内部再用 `std::thread::scope` 开新线程做双适配器并行。blocking 线程池已占用，又额外创建 OS 线程。
- **建议**：双适配器动作直接表达为 `tokio::spawn` + `tokio::join!`，避免在 `spawn_blocking` 内部再开 scoped thread。

#### 8.2.3 `reqwest::blocking::Client` 内部自建 tokio runtime

- **位置**：`network/client.rs`
- **问题**：`reqwest::blocking::Client` 会自建内部 runtime。`CLIENT_POOL` 最多缓存 32 个 client，极端情况下可能同时存在数十个内部 runtime。
- **建议**：尽快迁移到单个异步 `reqwest::Client`；按 `local_address` 分 client 时也使用 async client。

#### 8.2.4 跨字段网络状态更新非原子

- **位置**：`commands/login.rs`、`monitor/watcher.rs`
- **问题**：`any_adapter_online`、`last_a1_online`、`last_a2_online`、`has_logged_online` 等分别用独立原子变量存储，更新顺序无保障。并发读取可能看到不一致快照。
- **建议**：将高度相关的网络状态收拢到 `NetworkSnapshot`，用 `ArcSwap` 保证读写都看到一致快照。

#### 8.2.5 MAC/DHCP 流程拼接 PowerShell，存在命令注入与系统影响风险

- **位置**：`network/adapter.rs`
- **问题**：`escape_ps_single_quote` 防护有限，适配器名仍可能包含反引号、`$` 等 PowerShell 特殊字符；MAC 重置会修改注册表、禁用/启用网卡。
- **建议**：对 `adapter_name` 使用更严格白名单；优先使用带参数数组的 `PowerShell -EncodedCommand` 或平台 API（WMI/COM）；重构后把 DHCP/MAC 操作隔离到独立模块并补充权限/回滚测试。

### 8.3 建议性问题补充

| 问题 | 位置 | 建议 |
|---|---|---|
| 注销未重置认证失败计数 | `commands/login.rs` | 在 logout 成功路径统一清零 `portal_failure_count` / `a1_auth_failure_count` / `a2_auth_failure_count` |
| 托盘“快速登录”与 `do_login` 逻辑重复 | `main.rs`、`commands/login.rs` | 托盘菜单只触发 `commands::login::do_login`，或提取公共 `post_login_handler` |
| “自动检测”中文常量硬编码 | `network/adapter.rs` | 定义为常量 `AUTO_DETECT_ADAPTER`，或改用 `enum AdapterSelection { Auto, Named(String) }` |
| 注册表遍历重复且可缓存 | `network/adapter.rs` | 发现阶段一次性构建 `guid -> class subkey` 映射 |
| 网络质量检测事件可能重复触发 | `monitor/watcher.rs`、`monitor/latency.rs` | 将质量检测调度统一到单一入口 `quality_scheduler.rs` |
| 自定义 Tokio Runtime 增加退出复杂度 | `main.rs` | 评估是否可直接使用 Tauri 默认 runtime；若保留，shutdown 阶段显式等待后台任务 join |

### 8.4 优先级调整建议

| 问题 | 报告优先级 | 审查员建议 | 说明 |
|---|---|---|---|
| HTTP Client 同步/异步混用 | P1 | **P0–P1** | `blocking` client 内部自建 runtime，资源消耗比预期更严重 |
| 跨字段网络状态非原子更新 | 未单独列出 | **P1** | 当前已存在，需在拆分前解决 |
| PowerShell/MAC 操作安全 | 未单独列出 | **P1** | 涉及权限、系统网卡状态 |
| 托盘快速登录重复 | 未单独列出 | **P1** | 与命令层重复同性质 |
| 后台任务 `running` flag 释放 | P0 | 保持 P0 | 需在 `BackgroundTaskManager` 中通过 `JoinHandle` 统一解决 |
| 自定义 Tokio Runtime | P2 | 保持 P2 或与 async 迁移合并 | 非核心瓶颈 |

### 8.5 实施节奏建议

1. **Phase 1 增加“建立回归测试基线”**：为 login/logout、background check、adapter discovery 写 mock 测试，确保后续每一步都有绿测试。
2. **Phase 2 拆分 `AppState` 分两步**：先为每个字段增加受控 API（禁止直接 `store`），再物理移动结构体。
3. **Phase 4 异步化逐个调用点迁移**，不要一次性替换整个 `network/client.rs`。
4. **整体时间估算**：报告原估算 6–7.5 周偏乐观，建议预留 **8–10 周**并设置多个内部 Release 验证点。

### 8.6 下一步可展开方向

- A. 针对 **后台任务框架** 给出更具体的状态机设计与代码示例；
- B. 针对 **async HTTP Client 迁移** 给出调用点梳理与逐步替换方案；
- C. 针对 **AppState 拆分与跨字段一致性** 给出 `NetworkSnapshot` 设计；
- D. 针对 **MAC/DHCP 安全改造** 给出 PowerShell 参数化与权限降级建议；
- E. 针对 **测试基线** 给出 mock portal / mock adapter 的 trait 设计。
