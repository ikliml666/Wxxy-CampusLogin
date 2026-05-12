use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use arc_swap::ArcSwap;
use crate::config::Config;
use parking_lot::Mutex;

// [架构说明] 状态管理策略：AtomicBool + ArcSwap + Mutex 混合方案
//
// 本模块使用三种并发原语管理全局状态，各有取舍：
//
// 1. AtomicBool / AtomicU32 / AtomicU64
//    - 用于简单的布尔标志和计数器（如 is_logging_in, was_online）
//    - 优点：无锁，零开销读写，适合高频检查（后台循环每 15-60 秒轮询）
//    - 缺点：只能表达单个原子值，无法保证多个 AtomicBool 之间的一致性
//      例如：is_logging_in=false 和 was_online=true 之间没有事务性保证，
//      读取方可能看到"不在登录中"但"曾经在线"的中间状态
//    - Ordering 选择：写操作用 Release，读操作用 Acquire，确保跨线程可见性
//      Relaxed 仅用于计数器递增（disconnect_reconnect_count），因为该值只做近似判断
//
// 2. ArcSwap<T>
//    - 用于需要原子替换的复合值（如 Config, cached_online_status）
//    - 优点：无锁读取，写入时原子替换整个 Arc，读者始终看到完整一致的数据
//    - 缺点：load_full() 会 clone Arc（引用计数 +1），高频调用有轻微开销；
//      写入是整体替换，无法做细粒度字段更新
//    - 注意：ArcSwap<Arc<T>> 是双重间接，load() 返回 arc_swap::Guard<Arc<T>>，
//      Guard drop 前 Arc 不会被释放，保证读取期间数据有效
//
// 3. Mutex<T> (parking_lot)
//    - 用于需要独占访问的复合操作（如 login_timestamps 的"清理+检查+插入"）
//    - 优点：保证复合操作的原子性
//    - 缺点：持锁期间阻塞其他线程，临界区应尽量短
//
// 一致性风险：
//   - AtomicBool 之间没有跨字段一致性保证。例如后台检测循环中：
//       was_online.store(true) 和 is_logging_in.compare_exchange(false, true)
//     之间，另一个线程可能已经修改了 was_online
//   - ArcSwap<Config> 的 store 和 AtomicBool 的 store 之间也不是原子的，
//     可能出现"新配置已生效但旧标志位未更新"的窗口
//   - 当前设计依赖"最终一致性"——所有状态最终会收敛到正确值，
//     但在极端时序下可能出现短暂的逻辑不一致（如重复触发自动登录）
//     实际影响可忽略，因为后台检测周期（15-60秒）远大于状态更新耗时（微秒级）

pub const AUTO_EXIT_DELAY_MS: u64 = 10000;
pub const CANCEL_EXIT_SHORTCUT: &str = "CommandOrControl+Shift+C";
const LOGIN_RATE_LIMIT_SECS: u64 = 10;
const LOGIN_RATE_LIMIT_MAX: u32 = 5;

// TaskLock: 封装 AtomicBool 的互斥获取 + 自动释放模式
// 替代散落在各处的 compare_exchange + atomic_guard 宏组合
// 保证：try_acquire 成功后，Guard drop 时自动释放，不会遗漏
pub struct TaskLock {
    flag: AtomicBool,
}

pub struct TaskGuard<'a> {
    lock: &'a TaskLock,
}

impl TaskLock {
    pub fn new() -> Self {
        Self { flag: AtomicBool::new(false) }
    }

    pub fn try_acquire(&self) -> Option<TaskGuard<'_>> {
        if self.flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            Some(TaskGuard { lock: self })
        } else {
            None
        }
    }

    pub fn is_active(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    pub fn force_release(&self) {
        self.flag.store(false, Ordering::Release);
    }

    pub fn swap_acquire(&self) -> bool {
        self.flag.swap(true, Ordering::Acquire)
    }
}

impl Drop for TaskGuard<'_> {
    fn drop(&mut self) {
        self.lock.flag.store(false, Ordering::Release);
    }
}

// 保留 atomic_guard 宏以兼容现有代码，新代码应使用 TaskLock
macro_rules! atomic_guard {
    ($name:ident, $field:ident) => {
        struct $name<'a>(&'a crate::commands::state::AppState);
        impl Drop for $name<'_> {
            fn drop(&mut self) {
                self.0.tasks.$field.force_release();
            }
        }
    };
}
pub(crate) use atomic_guard;

lazy_static::lazy_static! {
    static ref ACCOUNT_NAME_RE: regex::Regex = regex::Regex::new(r"^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$").unwrap();
    static ref CUSTOM_COLOR_RE: regex::Regex = regex::Regex::new(r"^#[0-9a-fA-F]{6}$").unwrap();
}

pub fn validate_config(config: Config) -> Result<Config, String> {
    let mut config = config;
    if !config.user.is_empty() {
        crate::config::validate_username(&config.user)?;
    }
    if !config.password.is_empty() {
        if config.password == "***" {
            return Err("密码不能为\"***\"".to_string());
        }
        crate::config::validate_password(&config.password)?;
    }
    if config.operator == "@ctcc" {
        config.operator = "@telecom".to_string();
    } else if config.operator == "@cucc" {
        config.operator = "@unicom".to_string();
    }
    config.operator = crate::config::validate_operator(&config.operator).to_string();
    if !config.custom_theme_color.is_empty() {
        if !CUSTOM_COLOR_RE.is_match(&config.custom_theme_color) {
            return Err("自定义主题颜色格式无效，需为#开头的6位十六进制色值".to_string());
        }
    }
    if config.theme_mode != "dark" && config.theme_mode != "light" {
        return Err("主题模式必须为\"dark\"或\"light\"".to_string());
    }
    config.background_check_interval = config.background_check_interval.clamp(10000, 3600000);
    config.latency_test_interval = config.latency_test_interval.clamp(10000, 3600000);
    if config.portal_url == "http://10.1.99.100:801" {
        config.portal_url = "http://10.1.99.100".to_string();
    }
    if config.portal_url.is_empty() {
        config.portal_url = "http://10.1.99.100".to_string();
    }
    match url::Url::parse(&config.portal_url) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                return Err(format!("Portal地址协议不支持: {}，仅允许http/https", scheme));
            }
        }
        Err(e) => {
            return Err(format!("Portal地址格式无效: {}", e));
        }
    }
    if !config.fixed_gateway.is_empty() {
        if config.fixed_gateway.parse::<std::net::IpAddr>().is_err() {
            return Err(format!("固定网关地址无效: {}", config.fixed_gateway));
        }
    }
    Ok(config)
}

pub fn validate_account_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 32 {
        return Err("账号名称长度需在1-32之间".to_string());
    }
    if !ACCOUNT_NAME_RE.is_match(name) {
        return Err("账号名称仅允许字母、数字、下划线、中文和连字符".to_string());
    }
    Ok(name.to_string())
}

// 任务运行标志：使用 TaskLock 封装，统一互斥获取 + 自动释放
// 每个 TaskLock 通过 try_acquire() 获取 Guard，Guard drop 时自动释放
// 消除了散落的 compare_exchange + 手动 guard 模式，保证一致性
pub struct TaskFlags {
    pub background_running: TaskLock,
    pub latency_running: TaskLock,
    // latency_cancel 使用 ArcSwap 而非 TaskLock，因为 CancellationToken
    // 需要整体替换（创建新 token），不能简单翻转布尔值
    pub latency_cancel: ArcSwap<tokio_util::sync::CancellationToken>,
    pub is_checking: TaskLock,
    pub is_logging_in: TaskLock,
    pub is_quality_checking: TaskLock,
    pub login_timestamps: Mutex<Vec<Instant>>,
}

impl TaskFlags {
    pub fn check_login_rate_limit(&self) -> Result<(), String> {
        let mut timestamps = self.login_timestamps.lock();
        let now = Instant::now();
        let cutoff = now - std::time::Duration::from_secs(LOGIN_RATE_LIMIT_SECS);
        timestamps.retain(|t| *t > cutoff);
        if timestamps.len() >= LOGIN_RATE_LIMIT_MAX as usize {
            return Err(format!("登录操作过于频繁，请{}秒后重试", LOGIN_RATE_LIMIT_SECS));
        }
        timestamps.push(now);
        Ok(())
    }
}

// 网络状态：混合使用 AtomicBool/AtomicU32/ArcSwap
// was_online 和 has_logged_online 之间没有原子性保证，
// 但后台检测循环只在单线程中顺序更新它们，实际无竞态
pub struct NetworkStatus {
    pub server_available: AtomicBool,
    pub was_online: AtomicBool,
    pub has_logged_online: AtomicBool,
    pub background_check_count: AtomicU64,
    pub disconnect_reconnect_count: AtomicU32,
    pub consecutive_check_failures: AtomicU32,
    pub last_auto_login_attempt: ArcSwap<std::time::Instant>,
    pub cached_online_status: ArcSwap<Option<serde_json::Value>>,
    pub last_network_quality: ArcSwap<Option<String>>,
}

// 退出状态：is_quitting 使用 Arc<AtomicBool> 而非裸 AtomicBool，
// 因为需要跨线程共享（多个后台线程需要检查退出标志）
pub struct ExitState {
    pub is_quitting: std::sync::Arc<AtomicBool>,
    pub auto_exit_deadline: Mutex<Option<Instant>>,
    pub auto_exit_cancelled: AtomicBool,
}

impl ExitState {
    pub fn deadline(&self) -> Option<Instant> {
        *self.auto_exit_deadline.lock()
    }

    pub fn set_deadline(&self, deadline: Option<Instant>) {
        *self.auto_exit_deadline.lock() = deadline;
    }
}

pub struct NotificationState {
    pub last_disabled_notification_epoch_ms: AtomicU64,
}

impl NotificationState {
    pub fn disabled_notification_elapsed(&self) -> Option<std::time::Duration> {
        let epoch_ms = self.last_disabled_notification_epoch_ms.load(Ordering::Acquire);
        if epoch_ms == 0 {
            return None;
        }
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if epoch_ms >= now_epoch {
            return Some(std::time::Duration::ZERO);
        }
        Some(std::time::Duration::from_millis(now_epoch - epoch_ms))
    }

    pub fn set_disabled_notification(&self) {
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_disabled_notification_epoch_ms.store(now_epoch, Ordering::Release);
    }
}

pub struct AppState {
    pub config: ArcSwap<Config>,
    pub tasks: TaskFlags,
    pub network: NetworkStatus,
    pub exit: ExitState,
    pub notification: NotificationState,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: ArcSwap::from(std::sync::Arc::new(Config::default())),
            tasks: TaskFlags {
                background_running: TaskLock::new(),
                latency_running: TaskLock::new(),
                latency_cancel: ArcSwap::from(Arc::new(tokio_util::sync::CancellationToken::new())),
                is_checking: TaskLock::new(),
                is_logging_in: TaskLock::new(),
                is_quality_checking: TaskLock::new(),
                login_timestamps: Mutex::new(Vec::new()),
            },
            network: NetworkStatus {
                server_available: AtomicBool::new(false),
                was_online: AtomicBool::new(false),
                has_logged_online: AtomicBool::new(false),
                background_check_count: AtomicU64::new(0),
                disconnect_reconnect_count: AtomicU32::new(0),
                consecutive_check_failures: AtomicU32::new(0),
                last_auto_login_attempt: ArcSwap::from(std::sync::Arc::new(std::time::Instant::now())),
                cached_online_status: ArcSwap::from(std::sync::Arc::new(None)),
                last_network_quality: ArcSwap::from(std::sync::Arc::new(None)),
            },
            exit: ExitState {
                is_quitting: std::sync::Arc::new(AtomicBool::new(false)),
                auto_exit_deadline: Mutex::new(None),
                auto_exit_cancelled: AtomicBool::new(false),
            },
            notification: NotificationState {
                last_disabled_notification_epoch_ms: AtomicU64::new(0),
            },
        }
    }

    pub fn check_login_rate_limit(&self) -> Result<(), String> {
        self.tasks.check_login_rate_limit()
    }
}

#[derive(Serialize)]
pub struct CommandResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl CommandResult {
    pub fn ok_msg(msg: &str) -> Self {
        Self { success: true, message: Some(msg.to_string()), data: None }
    }
    pub fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), data: None }
    }
}
