# Wxxy-CampusLogin 后端重构实施计划

> 依据：[`backend-modular-evaluation-report.md`](./backend-modular-evaluation-report.md)  
> 目标：将评估报告中的优化方案转化为可执行、可验证、可跟踪的实施计划  
> 版本：v1.0  
> 日期：2026-06-24

---

## 一、项目背景与目标

### 1.1 背景

Wxxy-CampusLogin Rust 后端经过多轮功能迭代后，形成了 9 个顶层模块、约 7,000–8,000 行代码的规模。当前架构在功能层面已较完整，但存在状态集中、职责边界模糊、代码重复、后台任务管理分散等系统性问题，亟需通过模块化重构提升可维护性、稳定性和长期演进能力。

### 1.2 目标

| 维度 | 目标 |
|---|---|
| 可维护性 | 单文件职责清晰，代码重复度下降 30% 以上 |
| 可测试性 | 核心服务可脱离 `AppHandle` 进行单元测试 |
| 性能 | 减少阻塞线程与重复资源创建，统一异步边界 |
| 稳定性 | 状态变更入口集中，消除半一致快照与任务并发风险 |
| 安全性 | 消除 PowerShell 命令注入风险，降低系统网卡误操作概率 |

### 1.3 核心原则

1. **接口兼容优先**：Phase 1–2 不修改 Tauri command 签名和前端事件 payload。
2. **小步快跑**：每个 Phase 结束都能编译通过、运行验证、独立回滚。
3. **测试先行**：每个 Phase 开始前先补充/完善对应模块的单元测试。
4. **依赖向内**：业务模块依赖基础设施，基础设施不依赖业务模块。

---

## 二、关键问题清单（按优先级排序）

| 编号 | 问题 | 优先级 | 关联文件 | 所属 Phase |
|---|---|---|---|---|
| P0-1 | `AppState` 上帝对象 | P0 | `infra/state.rs` | Phase 2 |
| P0-2 | 登录/注销逻辑重复 | P0 | `auth/session.rs`, `commands/login.rs` | Phase 3 |
| P0-3 | 后台任务管理混乱，`running` flag 释放点分散 | P0 | `monitor/watcher.rs`, `latency.rs`, `adapter_watch.rs` | Phase 2 |
| P0-P1-4 | `reqwest::blocking::Client` 内部自建 runtime，且混用 async/blocking | P0–P1 | `network/client.rs`, `auth/*.rs`, `network/quality.rs` | Phase 4 |
| P1-5 | 跨字段网络状态非原子更新 | P1 | `infra/state.rs`, `commands/login.rs`, `monitor/watcher.rs` | Phase 2 |
| P1-6 | PowerShell/MAC 重置命令注入与系统影响 | P1 | `network/adapter.rs` | Phase 4 |
| P1-7 | `network/adapter.rs` 过大（1,453 行） | P1 | `network/adapter.rs` | Phase 4 |
| P1-8 | `monitor/watcher.rs` 职责过重（857 行） | P1 | `monitor/watcher.rs` | Phase 5 |
| P1-9 | `main.rs` 初始化逻辑臃肿 | P1 | `main.rs` | Phase 1 |
| P1-10 | 命令层重复样板，托盘快速登录与 `do_login` 重复 | P1 | `commands/*.rs`, `main.rs` | Phase 1 |
| P1-11 | 网络质量检测可能重复触发 | P1 | `monitor/watcher.rs`, `monitor/latency.rs` | Phase 5 |
| P2-12 | 日志/通知未抽象 | P2 | 多处 | Phase 1 |
| P2-13 | 自定义 Tokio Runtime 增加退出复杂度 | P2 | `main.rs` | Phase 1/2 |
| P2-14 | “自动检测”中文常量硬编码 | P2 | `network/adapter.rs` | Phase 3/4 |
| P2-15 | 注销未重置认证失败计数 | P2 | `commands/login.rs` | Phase 3 |
| P2-16 | 注册表遍历重复，可缓存 | P2 | `network/adapter.rs` | Phase 4 |

---

## 三、目标架构总览

```
src/
├── app/                       # 应用生命周期与模块组装
│   ├── startup.rs             # 启动流程编排
│   ├── tray.rs                # 托盘菜单与事件
│   ├── shortcut.rs            # 全局快捷键
│   ├── heartbeat.rs           # 前端心跳与 WebView 兜底显示
│   └── shutdown.rs            # 优雅退出
│
├── commands/                  # Tauri Command 入口（只保留参数解析与转发）
│   ├── login.rs
│   ├── network.rs
│   ├── config.rs
│   ├── system.rs
│   ├── background.rs
│   ├── account.rs
│   └── updater.rs
│
├── infra/
│   ├── state/
│   │   ├── store.rs           # ConfigStore
│   │   ├── network.rs         # NetworkState + NetworkSnapshot
│   │   ├── exit.rs            # ExitState
│   │   └── mod.rs             # AppState 组装
│   ├── tasks.rs               # BackgroundTaskManager
│   ├── events.rs              # EventBus
│   ├── notifications.rs       # NotificationService
│   ├── logger.rs
│   └── lifecycle.rs           # 高层生命周期协调
│
├── auth/
│   ├── service.rs             # AuthService
│   ├── adapter_resolver.rs    # AdapterResolver
│   ├── failure_tracker.rs     # FailureTracker
│   ├── portal.rs              # PortalChecker
│   └── protocol.rs            # ProtocolClient
│
├── network/
│   ├── discovery/             # 适配器发现
│   │   ├── windows.rs
│   │   └── mod.rs
│   ├── adapter_cache.rs       # 适配器缓存
│   ├── dhcp.rs                # DHCP 续租
│   ├── subnet.rs              # 子网/SSID 计算
│   ├── client.rs              # 异步 HTTP Client Pool
│   ├── dns.rs
│   ├── quality.rs
│   └── timing.rs
│
├── monitor/
│   ├── campus_check.rs        # 校园网环境检测
│   ├── portal_check.rs        # Portal 在线检测
│   ├── auto_login.rs          # 自动登录/重连策略
│   ├── quality_scheduler.rs   # 网络质量调度（唯一入口）
│   └── adapter_watch.rs
│
├── config/
├── account/
├── platform/
└── update/
```

---

## 四、详细实施计划

### Phase 1：公共能力抽象与启动流程下沉（第 1–2 周）

**目标**：降低命令层与 `main.rs` 的重复与复杂度；建立回归测试基线；所有变更保持前端接口兼容。

#### T1.1 建立回归测试基线

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T1.1.1 定义 trait 边界 | 为 `PortalChecker`、`ProtocolClient`、`AdapterResolver` 定义 trait | 编译通过，无业务逻辑变更 |
| T1.1.2 编写 mock 测试 | 使用 mock adapter + stub portal server 测试 login/logout 成功/失败路径 | `cargo test` 新增 ≥ 10 个测试用例 |
| T1.1.3 后台任务测试 | 为现有后台任务编写启动/停止/重复启动测试 | 覆盖 `start_background_check_inner` 与 `spawn_latency_test_loop` |

#### T1.2 抽象 `EventBus` 与 `NotificationService`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T1.2.1 创建 `infra::events::EventBus` | 封装 `app_handle.emit` 调用 | `emit("login-log", ...)` 只剩一处实现 |
| T1.2.2 统一 `NotificationService` | 封装 `emit_notification` | 所有通知调用走 `NotificationService` |
| T1.2.3 迁移 `auth/session.rs` 中的事件发射 | 用 `EventBus` 替代直接 `emit` | 全局搜索 `emit("login-log"` 在业务模块中消失 |

#### T1.3 命令层 `CommandContext` 与公共 trait

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T1.3.1 定义 `CommandContext` | `struct CommandContext<'a> { app: &'a AppHandle, state: &'a AppState }` | 编译通过 |
| T1.3.2 提取 `AppHandleExt` trait | `notify_login_log`, `notify_adapter_changed`, `notify_background_result` 等 | commands 中 `emit` 调用下降 50% |
| T1.3.3 统一 `CommandResult` helper | `CommandResult::ok_msg`, `CommandResult::err` 已存在，新增 `from_json_result` 等 | 减少手写 `CommandResult { ... }` |

#### T1.4 将 `main.rs` 初始化逻辑下沉到 `app` 模块

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T1.4.1 创建 `app::tray` | 托盘菜单、图标、事件处理 | `main.rs` 中 tray 相关代码移除 |
| T1.4.2 创建 `app::shortcut` | 全局快捷键注册/注销 | `main.rs` 中 shortcut 相关代码移除 |
| T1.4.3 创建 `app::startup` | 启动任务调度 | `main.rs` 中后台任务启动代码移除 |
| T1.4.4 创建 `app::heartbeat` | 前端心跳、窗口兜底显示、WebView2 内存控制 | `main.rs` 中对应代码移除 |
| T1.4.5 统一“快速登录”入口 | 托盘菜单只调用 `commands::login::do_login` 或公共 `post_login_handler` | 消除托盘与命令入口的逻辑重复 |

**Phase 1 验收**：
- `main.rs` 行数 ≤ 150 行；
- `cargo test` 全部通过；
- 前端 command 调用与事件接收行为不变；
- 手动验证启动、登录、注销、托盘操作正常。

---

### Phase 2：状态管理与后台任务框架（第 3–5 周）

**目标**：拆分 `AppState`、消除跨字段状态不一致、建立统一的后台任务生命周期管理。

#### T2.1 拆分 `AppState`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T2.1.1 创建 `ConfigStore` | 封装 `ArcSwap<Config>`，提供 `load()`, `update(f)`, `subscribe()` | `state.config.load()` 只出现在 `ConfigStore` 内部 |
| T2.1.2 创建 `NetworkState` + `NetworkSnapshot` | 用 `ArcSwap<NetworkSnapshot>` 替代分散原子变量 | 所有网络状态读取都看到一致快照 |
| T2.1.3 创建 `ExitState` | 封装退出相关字段，仅由 `lifecycle` 操作 | `auth`、`commands`、`monitor` 不再直接修改 `ExitState` 字段 |
| T2.1.4 渐进式迁移 | 先为字段增加受控 API，再物理移动结构体 | 每步编译通过 |

**`NetworkSnapshot` 设计示例**：

```rust
#[derive(Clone, Default)]
pub struct NetworkSnapshot {
    pub server_available: bool,
    pub any_adapter_online: bool,
    pub a1_online: bool,
    pub a2_online: bool,
    pub has_logged_online: bool,
    pub disconnect_reconnect_count: u32,
    pub on_campus_network: bool,
    pub current_ssid: Option<String>,
    pub last_network_quality: Option<String>,
}

pub struct NetworkState {
    snapshot: ArcSwap<NetworkSnapshot>,
}

impl NetworkState {
    pub fn load(&self) -> Arc<NetworkSnapshot> {
        self.snapshot.load_full()
    }

    pub fn update<F>(&self, f: F)
    where F: FnOnce(&mut NetworkSnapshot) {
        loop {
            let current = self.snapshot.load_full();
            let mut new = (*current).clone();
            f(&mut new);
            let new_arc = Arc::new(new);
            if self.snapshot.compare_and_swap(&current, new_arc).ptr_eq(&current) {
                // 触发事件总线通知
                break;
            }
        }
    }
}
```

#### T2.2 设计并实现 `BackgroundTaskManager`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T2.2.1 设计 `TaskHandle` | 持有 `CancellationToken` + `JoinHandle<()>` | 编译通过 |
| T2.2.2 实现 `BackgroundTaskManager` | `start(name, interval, task)` / `stop(name)` / `is_running(name)` | 单元测试覆盖启动、停止、重复启动 |
| T2.2.3 迁移 `background_check` | 用新框架替代 `TaskFlags::background_running` + `bg_check_cancel` | 运行验证启停正常 |
| T2.2.4 迁移 `latency_test` | 用新框架替代 `TaskFlags::latency_running` + `latency_cancel` | 消除 `Arc::ptr_eq` 补丁 |
| T2.2.5 迁移 `adapter_watch` | 用新框架替代 `TaskFlags::adapter_watch_running` + `adapter_watch_cancel` | 运行验证 |
| T2.2.6 删除 `TaskFlags` 中分散字段 | 改为通过名称查询 | `infra/state.rs` 精简 |

**`BackgroundTaskManager` 设计示例**：

```rust
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;
use dashmap::DashMap;
use tokio_util::sync::CancellationToken;
use tokio::task::JoinHandle;

pub struct TaskHandle {
    cancel: CancellationToken,
    handle: JoinHandle<()>,
}

pub struct BackgroundTaskManager {
    registry: DashMap<String, TaskHandle>,
}

pub type TaskFn = Box<dyn Fn(CancellationToken) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

impl BackgroundTaskManager {
    pub fn new() -> Self {
        Self { registry: DashMap::new() }
    }

    pub async fn start(
        &self,
        name: &str,
        interval: Duration,
        task: TaskFn,
    ) -> Result<(), TaskError> {
        if self.registry.contains_key(name) {
            return Err(TaskError::AlreadyRunning);
        }
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            interval_timer.tick().await; // skip immediate first tick if needed
            loop {
                tokio::select! {
                    _ = interval_timer.tick() => {}
                    _ = cancel_clone.cancelled() => break,
                }
                task(cancel_clone.clone()).await;
            }
        });
        self.registry.insert(name.to_string(), TaskHandle { cancel, handle });
        Ok(())
    }

    pub async fn stop(&self, name: &str) {
        if let Some((_, handle)) = self.registry.remove(name) {
            handle.cancel.cancel();
            let _ = handle.handle.await;
        }
    }

    pub fn is_running(&self, name: &str) -> bool {
        self.registry.contains_key(name)
    }
}
```

#### T2.3 生命周期与退出流程统一

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T2.3.1 统一取消信号 | 退出时由 `BackgroundTaskManager` 统一 cancel 所有任务 | 无重复 `force_release` |
| T2.3.2 优雅 shutdown | `app::shutdown` 等待任务 join 后再退出 | 退出时不丢失日志 |
| T2.3.3 评估默认 runtime | 判断是否可移除自定义 Tokio runtime | 形成决策记录 |

**Phase 2 验收**：
- `AppState` 字段精简，各状态由独立 Store 管理；
- `NetworkSnapshot` 保证一致性；
- 后台任务启停 100 次无异常；
- 退出流程稳定，无任务泄漏。

---

### Phase 3：认证服务层重构（第 6–8 周）

**目标**：消除登录/注销重复逻辑，建立清晰的认证服务边界。

#### T3.1 提取 `AdapterResolver`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T3.1.1 定义 `AdapterResolver` trait | `resolve_primary`, `resolve_secondary` | 编译通过 |
| T3.1.2 实现默认 resolver | 迁移 `network::resolve_adapter_names` 逻辑 | 单测覆盖自动检测、双适配器、指定适配器 |
| T3.1.3 替换 `auth/session.rs` 与 `commands/login.rs` 中的解析逻辑 | 两处统一使用 `AdapterResolver` | 代码重复消除 |

#### T3.2 提取 `FailureTracker`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T3.2.1 定义 `FailureTracker` | 统一单/双适配器失败计数 | 编译通过 |
| T3.2.2 迁移 `update_auth_failure_count` 与 `update_dual_adapter_auth_failure` | 消除重复 | 单测覆盖连续 5 次失败触发 MAC 重置 |
| T3.2.3 注销时重置计数 | 在 logout 成功路径清零相关计数 | 单测覆盖 |

#### T3.3 提取双适配器执行器

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T3.3.1 设计 `DualAdapterExecutor` | 支持顺序/并行/错峰策略 | 编译通过 |
| T3.3.2 替换登录/注销中的双适配器逻辑 | 消除 `std::thread::scope` 嵌套 | 用 `tokio::spawn` + `tokio::join!` 实现 |
| T3.3.3 结果合并抽象 | `DualAdapterResult` 自动合并 success 与 message | 单测覆盖 |

**`DualAdapterExecutor` 示例**：

```rust
pub struct DualAdapterResult {
    pub primary: Option<CommandResult>,
    pub secondary: Option<CommandResult>,
}

impl DualAdapterResult {
    pub fn success(&self) -> bool {
        self.primary.as_ref().map(|r| r.success).unwrap_or(false)
            || self.secondary.as_ref().map(|r| r.success).unwrap_or(false)
    }

    pub fn combined_message(&self) -> String {
        // 合并主副适配器消息
    }
}

pub async fn execute_on_dual_adapters<F, Fut>(
    primary: &Adapter,
    secondary: Option<&Adapter>,
    stagger: Duration,
    action: F,
) -> DualAdapterResult
where
    F: Fn(&Adapter) -> Fut + Send + Sync + Clone,
    Fut: Future<Output = Option<CommandResult>> + Send,
{
    let r1 = action(primary);
    let r2 = async {
        if let Some(a) = secondary {
            tokio::time::sleep(stagger).await;
            action(a).await
        } else {
            None
        }
    };
    let (primary, secondary) = tokio::join!(r1, r2);
    DualAdapterResult { primary: Some(primary), secondary }
}
```

#### T3.4 引入 `AuthService`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T3.4.1 定义 `AuthService` | 接收 `LoginRequest` / `LogoutRequest`，返回 `AuthResult` | 编译通过 |
| T3.4.2 迁移 `full_login_inner` / `full_logout_inner` | 业务逻辑下沉到 `AuthService` | `commands/login.rs` 只保留命令入口 |
| T3.4.3 抽象登录历史记录 | `AuthService` 不直接写历史，由事件/通知机制处理 | 解耦 |
| T3.4.4 常量抽取 | `AUTO_DETECT_ADAPTER` 常量化或改为枚举 | 消除中文硬编码 |

**Phase 3 验收**：
- `auth/session.rs` 不再被 `commands` 直接调用；
- 登录/注销代码重复度下降 ≥ 50%；
- 双适配器并发使用 `tokio` 而非 `spawn_blocking` 内嵌 `thread::scope`；
- 单测覆盖单/双适配器登录、注销、失败计数、MAC 重置。

---

### Phase 4：网络层拆分与异步化（第 9–12 周）

**目标**：拆分 `network/adapter.rs`，迁移到异步 HTTP Client，消除安全与资源风险。

#### T4.1 拆分 `network/adapter.rs`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T4.1.1 创建 `network::discovery` | 适配器发现、Windows 注册表操作 | 单文件 < 400 行 |
| T4.1.2 创建 `network::adapter_cache` | TTL 缓存、并发优化 | 使用 `RwLock` 或 `moka` |
| T4.1.3 创建 `network::dhcp` | DHCP 续租、MAC 重置 | 隔离 PowerShell 调用 |
| T4.1.4 创建 `network::subnet` | 子网计算、SSID 处理 | 单测覆盖 |
| T4.1.5 平台隔离 | Windows 专有代码移到 `network::discovery::windows` | 条件编译通过 |

#### T4.2 异步 HTTP Client 迁移

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T4.2.1 创建异步 `HttpClient` | 基于 `reqwest::Client`，按 `local_address` 分池 | 编译通过 |
| T4.2.2 迁移 `auth/protocol.rs` | `do_login_with_retry` / `do_logout_with_retry` 改为 async | 单测通过 |
| T4.2.3 迁移 `auth/portal.rs` | `check_portal_full` 改为 async | 单测通过 |
| T4.2.4 迁移 `network/quality.rs` | `check_network_quality_async` 不再依赖 `spawn_blocking` | 运行时验证 |
| T4.2.5 删除 `reqwest::blocking` 依赖 | 确认 `Cargo.toml` 中不再需要 blocking feature | 编译通过 |

#### T4.3 适配器缓存优化

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T4.3.1 使用读写锁 | 读多写少场景优化 | 并发读取测试 |
| T4.3.2 后台刷新策略 | 避免强制刷新导致并发抖动 | 测试覆盖 |
| T4.3.3 缓存注册表映射 | 发现阶段一次性构建 `guid -> class subkey` | 注册表遍历次数下降 |

#### T4.4 MAC/DHCP 安全改造

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T4.4.1 严格白名单 | `adapter_name` 只允许字母、数字、中文、连字符、下划线 | 单测覆盖非法字符拒绝 |
| T4.4.2 参数化 PowerShell | 使用 `-EncodedCommand` 或参数数组 | 无字符串拼接命令 |
| T4.4.3 评估平台 API | 调研 WMI/COM `Set-NetAdapter` 替代方案 | 形成决策记录 |
| T4.4.4 权限与回滚测试 | 在测试环境验证 MAC 重置不会误伤 | 手动测试记录 |

**Phase 4 验收**：
- `network/adapter.rs` 拆分后每个文件 < 400 行；
- 所有 HTTP 调用使用 async `reqwest::Client`；
- PowerShell/MAC 操作通过参数化或平台 API 执行；
- 适配器缓存并发读取稳定。

---

### Phase 5：监控模块职责拆分（第 13–14 周）

**目标**：拆分 `monitor/watcher.rs`，统一网络质量检测调度。

#### T5.1 拆分 `monitor/watcher.rs`

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T5.1.1 创建 `monitor::campus_check` | 校园网环境检测 | 单文件 < 300 行 |
| T5.1.2 创建 `monitor::portal_check` | Portal 在线检测 | 单文件 < 300 行 |
| T5.1.3 创建 `monitor::auto_login` | 自动登录/断线重连策略 | 单文件 < 400 行 |
| T5.1.4 创建 `monitor::quality_scheduler` | 网络质量检测唯一调度入口 | 消除重复调用 |

#### T5.2 统一质量检测调度

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T5.2.1 `quality_scheduler` 注册到 `BackgroundTaskManager` | 由后台检测或独立定时器触发 | 运行时验证 |
| T5.2.2 删除 `latency.rs` 中的重复检测 | 只保留调度逻辑 | 全局搜索 `check_network_quality_async` 只剩 1–2 处 |
| T5.2.3 防止重复事件 | 同一时刻只保留一个活跃质量检测 | 前端不收到重复 `network-quality-result` |

#### T5.3 策略与检测解耦

| 子任务 | 说明 | 验收标准 |
|---|---|---|
| T5.3.1 `auto_login` 依赖抽象接口 | 不直接依赖 `auth::session::full_login_inner` | 通过 `AuthService` 调用 |
| T5.3.2 策略可配置 | 重连次数、冷却时间等通过配置注入 | 单测覆盖 |

**Phase 5 验收**：
- `monitor/watcher.rs` 拆分后每个文件 < 400 行；
- 网络质量检测只有一个调度入口；
- 自动登录/重连策略与状态检测解耦；
- 全量回归测试通过。

---

## 五、关键设计决策

### 5.1 状态管理：`ArcSwap` 还是 `RwLock`？

- **决策**：配置与网络快照使用 `ArcSwap`，保证无锁读取和高并发快照一致性；退出状态等写少读多的数据使用 `parking_lot::Mutex`。
- **理由**：`ArcSwap` 适合读多写少且需要原子替换整个快照的场景；`Mutex` 适合字段少、更新逻辑复杂的场景。

### 5.2 后台任务：统一框架还是独立管理？

- **决策**：统一 `BackgroundTaskManager`，`TaskHandle` 同时持有 `CancellationToken` 和 `JoinHandle`。
- **理由**：消除 `running` flag 释放点分散的问题，停止任务时确保旧任务真正退出。

### 5.3 HTTP Client：async 还是 blocking？

- **决策**：统一使用 `reqwest::Client`（async），仅在 Win32 API 调用处使用 `spawn_blocking`。
- **理由**：`reqwest::blocking::Client` 内部自建 runtime，多 client 时资源浪费严重；async client 可共享连接池和 runtime。

### 5.4 双适配器执行：`tokio::spawn` 还是 `std::thread::scope`？

- **决策**：使用 `tokio::spawn` + `tokio::join!`。
- **理由**：避免在 `spawn_blocking` 中再创建 OS 线程，减少线程资源占用。

### 5.5 MAC/DHCP：PowerShell 还是平台 API？

- **决策**：短期使用参数化 PowerShell（`-EncodedCommand`）作为安全加固；长期评估 WMI/COM 平台 API。
- **理由**：平台 API 更安全和可控，但开发成本高；参数化 PowerShell 是低风险的中期方案。

---

## 六、测试策略

### 6.1 单元测试

| 模块 | 测试重点 |
|---|---|
| `ConfigStore` | CAS 更新、订阅回调 |
| `NetworkState` | 快照一致性、并发更新 |
| `BackgroundTaskManager` | 启动、停止、重复启动、取消传播 |
| `AdapterResolver` | 自动检测、双适配器、指定适配器 |
| `FailureTracker` | 失败计数、MAC 重置触发、注销清零 |
| `AuthService` | 登录/注销成功失败、双适配器并行 |
| `network::discovery` | 黑名单过滤、可见性判断 |
| `network::subnet` | 子网匹配计算 |

### 6.2 集成测试

- 使用 mock portal server 测试完整登录/注销流程；
- 使用 mock adapter 列表测试后台检测与自动登录；
- 测试校园网检测各种分支（名称匹配、子网匹配、网关可达）。

### 6.3 手动测试

- 真实校园网环境下的登录/注销/断线重连；
- 后台检测启停、延迟测试启停；
- 托盘快速登录与主窗口登录行为一致；
- 自动退出/校园网退出取消流程；
- 应用退出时后台任务优雅关闭。

---

## 七、风险管理

| 风险 | 级别 | 缓解措施 |
|---|---|---|
| 重构过程中前端接口不兼容 | 高 | Phase 1–2 保持 command 签名与事件 payload 不变；后续变更需前端同步并灰度验证 |
| 异步化改变时序导致 Bug | 高 | 逐个调用点迁移，每次都有单元测试和手动回归 |
| 状态拆分引入竞态 | 高 | 使用 `NetworkSnapshot` + `ArcSwap`；所有写操作集中验证 |
| PowerShell/MAC 操作误伤系统 | 高 | 严格白名单、参数化命令、测试环境充分验证 |
| 校园网环境不可复现 | 中 | mock portal / mock adapter 测试桩；关键分支单测覆盖 |
| 重构周期过长导致分支冲突 | 中 | 每个 Phase 独立分支/PR，及时合并；设置多个内部 Release 验证点 |
| 性能回退 | 中 | Phase 4 完成后进行连接创建数、线程数、内存占用基准测试 |

---

## 八、里程碑与验收标准

| 里程碑 | 时间 | 验收标准 |
|---|---|---|
| M1：Phase 1 完成 | 第 2 周末 | `main.rs` ≤ 150 行；EventBus/NotificationService/CommandContext 上线；测试基线建立 |
| M2：Phase 2 完成 | 第 5 周末 | AppState 拆分完成；BackgroundTaskManager 统一所有后台任务；无 `force_release` 散落 |
| M3：Phase 3 完成 | 第 8 周末 | `AuthService` 上线；登录/注销重复度下降 ≥ 50%；双适配器使用 `tokio` 并行 |
| M4：Phase 4 完成 | 第 12 周末 | `network/adapter.rs` 拆分；`reqwest::blocking` 移除；MAC/DHCP 参数化执行 |
| M5：Phase 5 完成 | 第 14 周末 | `monitor/watcher.rs` 拆分；质量检测统一调度；全量回归通过 |

---

## 九、资源估算

| Phase | 预估周期 | 主要投入 |
|---|---|---|
| Phase 1 | 2 周 | 公共抽象、测试基线、启动流程下沉 |
| Phase 2 | 3 周 | 状态拆分、后台任务框架、退出流程统一 |
| Phase 3 | 3 周 | 认证服务层重构、双适配器执行器 |
| Phase 4 | 4 周 | 网络层拆分、异步化、安全改造 |
| Phase 5 | 2 周 | 监控模块拆分、质量检测统一 |
| **总计** | **14 周** | 含测试、联调、回归 |

> 注：若团队只有 1 名 Rust 后端工程师，建议预留 16–18 周；若并行开展 Phase 4 与 Phase 5 部分工作，可压缩至 12–14 周。

---

## 十、下一步建议

1. **召开架构评审会**：与前端、QA、运维一起评审本计划，确认 command/event 兼容策略和测试环境。
2. **准备 Phase 1 开发分支**：从 `main` 切出 `refactor/phase-1-infra`。
3. **补充 mock 基础设施**：优先实现 `MockPortalChecker` 和 `MockAdapterResolver`，作为后续所有 Phase 的测试基线。
4. **制定前端配合计划**：明确哪些 Phase 会调整 command 签名或事件 payload，提前安排前端改造。

---

## 附录 A：Phase 1 快速启动检查清单

- [ ] 创建 `infra::events::EventBus` 和 `infra::notifications::NotificationService`
- [ ] 创建 `CommandContext` 与 `AppHandleExt` trait
- [ ] 创建 `app::tray`、`app::shortcut`、`app::startup`、`app::heartbeat`
- [ ] 将 `main.rs` 初始化逻辑迁移到 `app` 模块
- [ ] 统一托盘快速登录入口到 `commands::login::do_login` 或公共 `post_login_handler`
- [ ] 为 login/logout/background check 编写 mock 测试
- [ ] 运行 `cargo test` 和 `cargo clippy`，确保无新增警告
- [ ] 手动验证启动、登录、注销、托盘操作、自动退出流程
