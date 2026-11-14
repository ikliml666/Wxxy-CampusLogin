use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Sender, SyncSender, TryRecvError};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering as AtomicOrdering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use arc_swap::ArcSwap;
use chrono::Local;
use parking_lot::{Mutex, RwLock};

const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;
/// 文件数安全阀：正常靠保留天数清理，这里只防"一天内疯狂轮转"把目录撑爆
const MAX_LOG_FILES: usize = 50;
const FLUSH_INTERVAL_MS: u64 = 2000;
const CHANNEL_CAPACITY: usize = 1024;
/// 审计 A17：日志通道改为有界（原为无界 channel——写盘卡顿时队列无上限增长，
/// 潜在内存放大）。容量 8192 条（每条约 200B，满载约 1.6MB 上限）。
const LOG_CHANNEL_CAPACITY: usize = 8192;
/// 审计 A17：发送端丢弃计数——通道满时 try_send 失败即丢弃（不阻塞调用方），
/// worker 周期性汇总。丢弃报告条件：距上次报告 ≥30s 或累计 ≥1000 条。
const DROP_REPORT_INTERVAL: Duration = Duration::from_secs(30);
const DROP_REPORT_THRESHOLD: u64 = 1000;
/// 跨进程等级同步周期：安卓 UI 与 :core 各自持有一份等级，靠等级文件对齐。
/// 每 30s 读一次几十字节的小文件，开销可忽略。
const LEVEL_SYNC_INTERVAL: Duration = Duration::from_secs(30);
static LOG_DROPPED: AtomicU64 = AtomicU64::new(0);

static LOG_RETENTION_DAYS: AtomicU32 = AtomicU32::new(7);
/// init_logger 记录的实际日志目录，供 set_log_retention_days 应用即清理用
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
/// 进程角色：桌面主进程 "app"、安卓 UI "ui"、安卓 :core "core"。用于日志文件名
/// 分段，避免两个进程向同一个文件交错写入
static LOG_TAG: OnceLock<String> = OnceLock::new();
/// 跨进程共享的等级文件（单行等级字符串），UI 切换等级时写入，各进程 worker 轮询同步
static LEVEL_FILE_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 启动期覆盖（环境变量 / 命令行），优先级高于配置文件与界面设置
static ENV_LEVEL: OnceLock<Option<LogLevel>> = OnceLock::new();
/// 环境变量里的模块过滤是否已生效：生效时忽略配置里的同名字段，避免被覆盖
static ENV_FILTER_APPLIED: OnceLock<bool> = OnceLock::new();
static LEVEL_FILE_STAMP: Mutex<Option<std::time::SystemTime>> = Mutex::new(None);

pub fn set_log_retention_days(days: u32) {
    LOG_RETENTION_DAYS.store(days, AtomicOrdering::Relaxed);
    // 应用即清理一次：启动接线（桌面 startup.rs / 安卓 run_startup_tasks）、配置导入与
    // 日志面板改保留天数都立即生效，不等 worker 首个 1 小时周期——重装/升级后旧日志
    // 首次启动即被清掉（此前启动路径无任何清理）
    if let Some(dir) = LOG_DIR.get() {
        cleanup_old_logs_by_time(dir, days);
    }
}

pub fn get_log_retention_days() -> u32 {
    LOG_RETENTION_DAYS.load(AtomicOrdering::Relaxed)
}

/// 日志等级（由低到高）。Silent 表示完全关闭日志输出，与界面/配置口径的
/// silent 对应——它必须排在最高位，log() 的"低于最小等级即丢弃"才能一票过滤。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Silent,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Warn
    }
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Silent => "SILENT",
        }
    }

    /// 配置与界面的唯一字符串口径（与日志面板下拉项一致）
    pub fn as_config_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warning",
            LogLevel::Error => "error",
            LogLevel::Silent => "silent",
        }
    }

    /// 解析配置/界面口径，大小写不敏感；warn/off/none 作为旧习惯别名接受。
    /// 未知值返回 None，由调用方（配置校验/命令层）决定兜底或报错。
    pub fn from_config_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warn" | "warning" => Some(LogLevel::Warn),
            "error" => Some(LogLevel::Error),
            "off" | "none" | "silent" => Some(LogLevel::Silent),
            _ => None,
        }
    }
}

/// 配置字符串 → 等级，未知/空值回落默认 Warn。配置校验已负责归一化，
/// 这里只承接手改配置文件、旧版本缺字段等边界输入。
pub fn log_level_from_config(raw: &str) -> LogLevel {
    LogLevel::from_config_str(raw).unwrap_or(LogLevel::Warn)
}

/// 解析模块级过滤规格：login=debug,monitor=info；空串合法（表示不过滤）。
pub fn parse_module_levels(spec: &str) -> Result<Vec<(String, LogLevel)>, String> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, raw_level) = part
            .split_once('=')
            .ok_or_else(|| format!("模块等级项缺少 '=': {part}"))?;
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("模块等级项模块名为空: {part}"));
        }
        let level = LogLevel::from_config_str(raw_level)
            .ok_or_else(|| format!("模块等级项等级未知: {part}"))?;
        out.push((name.to_string(), level));
    }
    Ok(out)
}

/// 应用模块级过滤（配置字段 logModuleLevels / 环境变量 CAMPUS_LOG_FILTER）。
/// 环境变量已生效时忽略配置值：环境变量是启动期诊断开关，语义上优先级最高。
pub fn apply_module_levels(spec: &str) {
    if ENV_FILTER_APPLIED.get().copied().unwrap_or(false) {
        return;
    }
    match parse_module_levels(spec) {
        Ok(levels) => MODULE_LEVELS.store(std::sync::Arc::new(levels)),
        Err(e) => eprintln!("[logger] 忽略非法的模块级过滤配置: {e}"),
    }
}

enum LogMessage {
    Entry { line: String },
    Flush { ack: Sender<()> },
    Shutdown,
}

struct LoggerState {
    log_dir: PathBuf,
    current_writer: Option<BufWriter<File>>,
    current_date: String,
    last_flush: Instant,
}

lazy_static::lazy_static! {
    static ref LOGGER_SENDER: ArcSwap<Option<SyncSender<LogMessage>>> = ArcSwap::from(std::sync::Arc::new(None));
    static ref CLEAR_LOGS_MUTEX: Mutex<()> = Mutex::new(());
    static ref LOGGER_THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
    /// 当前生效的最低等级（已合并环境变量覆盖）
    static ref MIN_LOG_LEVEL: ArcSwap<LogLevel> = ArcSwap::from(std::sync::Arc::new(LogLevel::Warn));
    /// 配置/界面设定的等级原值（不含环境变量覆盖），用于与等级文件比较
    static ref CONFIG_LEVEL: ArcSwap<LogLevel> = ArcSwap::from(std::sync::Arc::new(LogLevel::Warn));
    /// 模块级覆盖表（模块名 → 最低等级）；为空时 log() 只查全局阈值
    static ref MODULE_LEVELS: ArcSwap<Vec<(String, LogLevel)>> = ArcSwap::from(std::sync::Arc::new(Vec::new()));
    /// 敏感值黑名单：register_secret 登记后，任何日志消息里的该值都会被替换为星号
    static ref SECRETS: RwLock<Vec<String>> = RwLock::new(Vec::new());
}

fn current_tag() -> &'static str {
    LOG_TAG.get().map(|s| s.as_str()).unwrap_or("app")
}

/// 日志文件名：桌面主进程沿用旧格式 app-<日期>.log，其余进程/核心加角色段，
/// 避免安卓 UI 与 :core 两个进程往同一文件交错写
fn log_file_name(tag: &str, date: &str) -> String {
    if tag == "app" {
        format!("app-{date}.log")
    } else {
        format!("app-{tag}-{date}.log")
    }
}

fn level_file_path() -> Option<&'static PathBuf> {
    LEVEL_FILE_PATH.get()
}

/// 环境变量 CAMPUS_LOG_LEVEL 或命令行 --log-level 的启动期覆盖
fn startup_level_override() -> Option<LogLevel> {
    if let Ok(raw) = std::env::var("CAMPUS_LOG_LEVEL") {
        if let Some(level) = LogLevel::from_config_str(&raw) {
            return Some(level);
        }
    }
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if let Some(v) = args[i].strip_prefix("--log-level=") {
            return LogLevel::from_config_str(v);
        }
        if args[i] == "--log-level" {
            return args.get(i + 1).and_then(|v| LogLevel::from_config_str(v));
        }
        i += 1;
    }
    None
}

fn env_level() -> Option<LogLevel> {
    ENV_LEVEL.get().copied().flatten()
}

/// tag 为进程角色（"app" / "ui" / "core"）：决定日志文件名分段，并让各进程
/// 共享同一个等级文件实现跨进程等级同步。重复调用保留首次 tag。
pub fn init_logger_with_tag(log_dir: PathBuf, tag: &str) -> Result<(), String> {
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("创建日志目录失败: {e}");
    }
    let _ = LOG_DIR.set(log_dir.clone());
    let _ = LOG_TAG.set(tag.to_string());
    let _ = LEVEL_FILE_PATH.set(log_dir.join(".log-level"));

    // 启动期覆盖：环境变量/命令行 > 等级文件 > 配置文件（配置文件在 startup 后应用）
    let _ = ENV_LEVEL.set(startup_level_override());
    if let Ok(filter) = std::env::var("CAMPUS_LOG_FILTER") {
        match parse_module_levels(&filter) {
            Ok(levels) => {
                MODULE_LEVELS.store(std::sync::Arc::new(levels));
                let _ = ENV_FILTER_APPLIED.set(true);
            }
            Err(e) => eprintln!("[logger] 忽略非法的 CAMPUS_LOG_FILTER: {e}"),
        }
    }
    if let Some(level) = read_level_file() {
        set_log_level(level);
    }

    let today = Local::now().format("%Y-%m-%d").to_string();
    let log_path = log_dir.join(log_file_name(current_tag(), &today));

    let writer = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(BufWriter::new);

    if writer.is_none() {
        eprintln!("[WARN] 无法打开日志文件: {log_path:?}");
    }

    // 审计 A17：有界通道——写盘慢时上游不堆积，超限由发送端丢弃并计数
    let (sender, receiver) = sync_channel::<LogMessage>(LOG_CHANNEL_CAPACITY);

    {
        let old = LOGGER_SENDER.swap(std::sync::Arc::new(Some(sender)));
        if let Some(old_sender) = old.as_ref() {
            let _ = old_sender.send(LogMessage::Shutdown);
        }
        drop(old);
    }

    let state = LoggerState {
        log_dir,
        current_writer: writer,
        current_date: today,
        last_flush: Instant::now(),
    };

    let handle = std::thread::Builder::new()
        .name("logger-worker".to_string())
        .spawn(move || {
            logger_worker(state, receiver);
        })
        .map_err(|e| format!("Failed to spawn logger thread: {e}"))?;

    {
        let mut thread_lock = LOGGER_THREAD.lock();
        if let Some(old_handle) = thread_lock.take() {
            let _ = old_handle.join();
        }
        *thread_lock = Some(handle);
    }

    Ok(())
}

fn logger_worker(mut state: LoggerState, receiver: std::sync::mpsc::Receiver<LogMessage>) {
    let mut buffer: Vec<LogMessage> = Vec::with_capacity(64);
    let mut last_cleanup = std::time::Instant::now();
    // 审计 A17：丢弃计数周期汇总的计时起点
    let mut last_drop_report = std::time::Instant::now();
    let mut last_level_sync = std::time::Instant::now();
    const CLEANUP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);
    // BE-C-01: 批量落盘阈值——累计到该条数或距上次落盘超过 FLUSH_INTERVAL_MS 才写盘，
    // 避免低速时每条日志都触发一次 write+flush 使 BufWriter 形同虚设。
    const BATCH_FLUSH_THRESHOLD: usize = 32;
    const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(FLUSH_INTERVAL_MS);

    loop {
        match receiver.recv_timeout(FLUSH_INTERVAL) {
            Ok(LogMessage::Shutdown) => {
                drain_channel(&receiver, &mut buffer);
                flush_messages(&mut state, &mut buffer);
                let _ = state.current_writer.as_mut().map(|w| w.flush());
                return;
            }
            Ok(msg) => buffer.push(msg),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                drain_channel(&receiver, &mut buffer);
                flush_messages(&mut state, &mut buffer);
                let _ = state.current_writer.as_mut().map(|w| w.flush());
                return;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_cleanup.elapsed() >= CLEANUP_INTERVAL {
                    let retention_days = get_log_retention_days();
                    cleanup_old_logs_by_time(&state.log_dir, retention_days);
                    last_cleanup = std::time::Instant::now();
                }
            }
        }

        // 跨进程等级同步：UI 进程切换等级后写等级文件，:core 进程在这里跟随
        if last_level_sync.elapsed() >= LEVEL_SYNC_INTERVAL {
            sync_level_from_file();
            last_level_sync = std::time::Instant::now();
        }

        drain_channel(&receiver, &mut buffer);

        // 审计 A17：周期汇总发送端丢弃的日志条数（≥30s 或 ≥1000 条触发一次），
        // 取 swap(0) 原子清零，报告本身走通道，若因通道满被丢弃自然并入下一轮计数
        let dropped_pending = LOG_DROPPED.load(AtomicOrdering::Relaxed);
        if dropped_pending > 0
            && (last_drop_report.elapsed() >= DROP_REPORT_INTERVAL
                || dropped_pending >= DROP_REPORT_THRESHOLD)
        {
            let dropped = LOG_DROPPED.swap(0, AtomicOrdering::Relaxed);
            if dropped > 0 {
                crate::log_warn!(
                    "logger",
                    "日志通道已满，已丢弃 {} 条日志（容量 {} 条，通常意味着写盘持续阻塞）",
                    dropped,
                    LOG_CHANNEL_CAPACITY
                );
            }
            last_drop_report = std::time::Instant::now();
        }

        // 批量落盘条件：Flush{ack} 必须立即刷（flush() 等待 ack，超时 5s）；
        // 其余消息累计到阈值或距上次落盘超过间隔才刷。
        let needs_immediate_flush = buffer.iter().any(|m| matches!(m, LogMessage::Flush { .. }));
        if needs_immediate_flush
            || buffer.len() >= BATCH_FLUSH_THRESHOLD
            || state.last_flush.elapsed() >= FLUSH_INTERVAL
        {
            flush_messages(&mut state, &mut buffer);
        }
    }
}

fn drain_channel(receiver: &std::sync::mpsc::Receiver<LogMessage>, buffer: &mut Vec<LogMessage>) {
    loop {
        match receiver.try_recv() {
            Ok(msg) => {
                match msg {
                    LogMessage::Shutdown => {}
                    _ => buffer.push(msg),
                }
                if buffer.len() >= CHANNEL_CAPACITY {
                    break;
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

fn flush_messages(state: &mut LoggerState, buffer: &mut Vec<LogMessage>) {
    if buffer.is_empty() {
        return;
    }

    rotate_if_needed(state);

    if let Some(ref mut writer) = state.current_writer {
        // 写盘失败（磁盘满/只读/句柄失效）此前被 let _ 静默吞掉，日志"消失"却
        // 无任何痕迹。日志系统自身已不可用，只能用 stderr 报告；同一批最多报一次，
        // 避免每条日志都往 stderr 刷屏
        let mut write_failed = false;
        for msg in buffer.drain(..) {
            match msg {
                LogMessage::Entry { line } => {
                    if let Err(e) = writer.write_all(line.as_bytes()) {
                        if !write_failed {
                            eprintln!("[logger] 日志写盘失败（本批后续失败不再重复报告）: {e}");
                            write_failed = true;
                        }
                    }
                }
                LogMessage::Flush { ack } => {
                    if let Err(e) = writer.flush() {
                        eprintln!("[logger] 日志刷盘失败: {e}");
                    }
                    let _ = ack.send(());
                }
                LogMessage::Shutdown => {}
            }
        }
        if let Err(e) = writer.flush() {
            eprintln!("[logger] 日志刷盘失败: {e}");
        }
    } else {
        for msg in buffer.drain(..) {
            if let LogMessage::Flush { ack } = msg {
                let _ = ack.send(());
            }
        }
    }
    state.last_flush = Instant::now();
}

fn rotate_if_needed(state: &mut LoggerState) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    if today != state.current_date {
        let previous = std::mem::replace(&mut state.current_date, today.clone());
        // 跨天：关掉旧 writer 后把前一天的文件压缩归档（.log.gz），
        // 之后按保留天数清理时同时处理 .log 与 .log.gz
        if let Some(mut writer) = state.current_writer.take() {
            let _ = writer.flush();
        }
        let prev_path = state.log_dir.join(log_file_name(current_tag(), &previous));
        if prev_path.exists() {
            if let Err(e) = compress_log_file(&prev_path) {
                eprintln!("[logger] 日志归档压缩失败({}): {e}", prev_path.display());
            }
        }
        let log_path = state.log_dir.join(log_file_name(current_tag(), &today));
        state.current_writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok()
            .map(BufWriter::new);
        state.last_flush = Instant::now();
        return;
    }

    if let Some(ref writer) = state.current_writer {
        let file = writer.get_ref();
        if let Ok(meta) = file.metadata() {
            if meta.len() > MAX_LOG_SIZE {
                let timestamp = Local::now().format("%Y%m%d%H%M%S");
                let tag = current_tag();
                let rotated = state.log_dir.join(if tag == "app" {
                    format!("app-{timestamp}.log")
                } else {
                    format!("app-{tag}-{timestamp}.log")
                });
                let current = state.log_dir.join(log_file_name(tag, &state.current_date));
                if fs::rename(&current, &rotated).is_ok() {
                    cleanup_old_logs(&state.log_dir);
                } else {
                    crate::log_warn!("logger", "日志轮转rename失败，尝试直接截断");
                    if let Ok(f) = OpenOptions::new().write(true).truncate(true).open(&current) {
                        let _ = f;
                    }
                }

                let new_path = state.log_dir.join(log_file_name(tag, &state.current_date));
                state.current_writer = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&new_path)
                    .ok()
                    .map(BufWriter::new);
                state.last_flush = Instant::now();
            }
        }
    }
}

/// 把已结束的日志文件压成 .log.gz（原始文件删除）。压缩失败时保留原文件，
/// 调用方只记 stderr——归档失败不影响继续写日志。
fn compress_log_file(path: &Path) -> std::io::Result<()> {
    let src = File::open(path)?;
    let gz_path = PathBuf::from(format!("{}.gz", path.display()));
    let out = File::create(&gz_path)?;
    let mut encoder = flate2::write::GzEncoder::new(BufWriter::new(out), flate2::Compression::default());
    std::io::copy(&mut BufReader::new(src), &mut encoder)?;
    encoder.finish()?.flush()?;
    fs::remove_file(path)?;
    Ok(())
}

/// 文件数安全阀：正常情况下按保留天数清理；只有单日高频轮转时才靠这里兜底
fn cleanup_old_logs(log_dir: &Path) {
    if let Ok(entries) = fs::read_dir(log_dir) {
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
            .flatten()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                is_app_log_file(&name)
            })
            .filter_map(|e| {
                let path = e.path();
                let modified = e.metadata().ok()?.modified().ok()?;
                Some((modified, path))
            })
            .collect();
        files.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, path) in files.iter().skip(MAX_LOG_FILES) {
            let _ = fs::remove_file(path);
        }
    }
}

fn cleanup_old_logs_by_time(log_dir: &Path, retention_days: u32) {
    if retention_days == 0 {
        return; // 永久保留
    }
    let cutoff = match std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(retention_days as u64 * 86400))
    {
        Some(c) => c,
        None => {
            crate::log_warn!("logger", "日志保留天数 {} 过大，跳过清理", retention_days);
            return;
        }
    };
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_app_log_file(&name) {
                continue;
            }
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

/// 模块级覆盖优先于全局阈值；模块表通常为空，空表时只多一次 is_empty 判断
fn module_threshold(module: &str) -> LogLevel {
    let table = MODULE_LEVELS.load();
    if !table.is_empty() {
        if let Some((_, level)) = table.iter().find(|(name, _)| name == module) {
            return *level;
        }
    }
    **MIN_LOG_LEVEL.load()
}

pub fn log(level: LogLevel, module: &str, message: &str) {
    if level < module_threshold(module) {
        return;
    }

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f%:z");
    let message = redact(message);
    let line = format!("[{}] [{}] [{}] {}\n", timestamp, level.as_str(), module, message);

    if let Some(sender) = LOGGER_SENDER.load().as_ref().clone() {
        // 审计 A17：try_send 非阻塞——通道满时丢弃本条并计数（worker 周期性汇总），
        // 绝不阻塞调用方（原 send 在有界通道满时会卡死任意调用线程）。
        // Warn 及以上仍落 stderr 兜底，高优先级日志不因通道满而彻底丢失。
        if let Err(e) = sender.try_send(LogMessage::Entry { line }) {
            LOG_DROPPED.fetch_add(1, AtomicOrdering::Relaxed);
            if level >= LogLevel::Warn {
                if let LogMessage::Entry { line } = match e {
                    std::sync::mpsc::TrySendError::Full(m)
                    | std::sync::mpsc::TrySendError::Disconnected(m) => m,
                } {
                    eprint!("{line}");
                }
            }
        }
    } else if level >= LogLevel::Warn {
        eprint!("{line}");
    }
}

/// 登记敏感值（密码、密文等）：此后任何日志消息里出现该值都会被替换成三个星号。
/// 短于 4 个字符的值不登记——太短会误伤正常文本。上限 8 条，超出淘汰最旧的。
pub fn register_secret(value: &str) {
    let value = value.trim();
    if value.len() < 4 {
        return;
    }
    let mut secrets = SECRETS.write();
    if secrets.iter().any(|s| s == value) {
        return;
    }
    if secrets.len() >= 8 {
        secrets.remove(0);
    }
    secrets.push(value.to_string());
}

fn redact(message: &str) -> Cow<'_, str> {
    let secrets = SECRETS.read();
    if secrets.is_empty() {
        return Cow::Borrowed(message);
    }
    let mut out: Option<String> = None;
    for secret in secrets.iter() {
        if out.is_none() && message.contains(secret.as_str()) {
            out = Some(message.to_string());
        }
        if let Some(buf) = out.as_mut() {
            if buf.contains(secret.as_str()) {
                *buf = buf.replace(secret.as_str(), "***");
            }
        }
    }
    match out {
        Some(s) => Cow::Owned(s),
        None => Cow::Borrowed(message),
    }
}

/// 结构化字段值转义：引号/反斜杠/换行都必须转义，否则 k="v" 无法被解析回原值
pub fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

/// 带结构化字段的日志：msg key="value" key2="value2"；前缀四段格式不变，
/// 前端解析与旧日志兼容。字段为空时等价于普通 log()。
pub fn log_fields(level: LogLevel, module: &str, message: &str, fields: &[(&str, String)]) {
    if fields.is_empty() {
        log(level, module, message);
        return;
    }
    let mut buf = String::with_capacity(message.len() + fields.len() * 32);
    buf.push_str(message);
    for (key, value) in fields {
        buf.push(' ');
        buf.push_str(key);
        buf.push_str("=\"");
        buf.push_str(&escape_field(value));
        buf.push('"');
    }
    log(level, module, &buf);
}

pub fn set_log_level(level: LogLevel) {
    CONFIG_LEVEL.store(std::sync::Arc::new(level));
    let effective = env_level().unwrap_or(level);
    MIN_LOG_LEVEL.store(std::sync::Arc::new(effective));
}

pub fn get_log_level() -> LogLevel {
    **MIN_LOG_LEVEL.load()
}

/// 当前等级的配置口径字符串，供 get_init_data 一次性出站（原 debugMode 布尔
/// 字段只区分 debug/info 两态，无法表达 warning/error/silent）
pub fn get_log_level_name() -> &'static str {
    get_log_level().as_config_str()
}

fn read_level_file() -> Option<LogLevel> {
    let path = level_file_path()?;
    let content = fs::read_to_string(path).ok()?;
    content.lines().next().and_then(LogLevel::from_config_str)
}

/// 写跨进程等级文件（单行等级）。失败只报 stderr，不影响命令结果
fn persist_level_file(level: LogLevel) {
    let Some(path) = level_file_path() else {
        return;
    };
    if let Err(e) = fs::write(path, format!("{}\n", level.as_config_str())) {
        eprintln!("[logger] 写等级文件失败({}): {e}", path.display());
    }
}

/// 按配置文件应用等级：写回等级文件，保证下次其它进程启动读到同一值
pub fn apply_config_level(raw: &str) {
    let level = log_level_from_config(raw);
    set_log_level(level);
    // 环境变量覆盖生效时不写文件，避免把诊断用临时等级固化成跨进程默认值
    if env_level().is_none() {
        persist_level_file(level);
    }
}

/// 界面切换等级：运行期立即生效，并写等级文件供其它进程（安卓 :core）跟随
pub fn set_log_level_persisted(level: LogLevel) {
    set_log_level(level);
    if env_level().is_none() {
        persist_level_file(level);
    }
}

/// worker 周期调用：等级文件被其它进程改写后应用到本进程
fn sync_level_from_file() {
    let Some(path) = level_file_path() else {
        return;
    };
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let Ok(modified) = metadata.modified() else {
        return;
    };
    {
        let mut stamp = LEVEL_FILE_STAMP.lock();
        if stamp.as_ref() == Some(&modified) {
            return;
        }
        *stamp = Some(modified);
    }
    if let Some(level) = read_level_file() {
        if level != **CONFIG_LEVEL.load() {
            set_log_level(level);
        }
    }
}

#[macro_export]
macro_rules! log_debug {
    ($module:expr, $($arg:tt)*) => {
        $crate::infra::logger::log($crate::infra::logger::LogLevel::Debug, $module, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($module:expr, $($arg:tt)*) => {
        $crate::infra::logger::log($crate::infra::logger::LogLevel::Info, $module, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($module:expr, $($arg:tt)*) => {
        $crate::infra::logger::log($crate::infra::logger::LogLevel::Warn, $module, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($module:expr, $($arg:tt)*) => {
        $crate::infra::logger::log($crate::infra::logger::LogLevel::Error, $module, &format!($($arg)*))
    };
}

/// 带结构化字段的日志：crate::log_kv!(LogLevel::Info, "login", "登录完成", user = u, elapsed_ms = n)
#[macro_export]
macro_rules! log_kv {
    ($level:expr, $module:expr, $msg:expr $(, $key:ident = $val:expr)* $(,)?) => {{
        let fields: Vec<(&str, String)> = vec![$((stringify!($key), $val.to_string())),*];
        $crate::infra::logger::log_fields($level, $module, $msg, &fields)
    }};
}

#[cfg(target_os = "android")]
pub fn get_log_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    // 安卓上 current_exe 指向只读的 APK 安装目录,日志必须落应用私有数据目录
    app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("logs")
}

#[cfg(not(target_os = "android"))]
pub fn get_log_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    let install_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            app_handle.path().app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
        });
    install_dir.join("logs")
}

/// 从文件尾部按块倒读，只解析所需尾部 N 行（BE-C-01）。窗口按 2 倍增长向后扩展
/// （从 8KB 起），直到尾部字节行数满足要求或覆盖整个文件；跨块行边界由连续字节流
/// 自然拼接还原，仅窗口起点可能截断首行（该行在取尾部 N 行时被丢弃）。
fn read_tail_lines(log_path: &Path, lines: usize) -> Result<Vec<String>, String> {
    if lines == 0 || !log_path.exists() {
        return Ok(Vec::new());
    }
    let mut file = File::open(log_path).map_err(|e| format!("读取日志失败: {e}"))?;
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file_len == 0 {
        return Ok(Vec::new());
    }

    const BASE_WINDOW: u64 = 8192;
    let mut window = BASE_WINDOW;
    let mut tail: Vec<u8> = Vec::new();
    loop {
        let read_start = file_len.saturating_sub(window);
        let len = (file_len - read_start) as usize;
        file.seek(SeekFrom::Start(read_start)).map_err(|e| format!("读取日志失败: {e}"))?;
        tail.clear();
        tail.resize(len, 0u8);
        file.read_exact(&mut tail).map_err(|e| format!("读取日志失败: {e}"))?;

        let text = String::from_utf8_lossy(&tail);
        let count = text.lines().count();
        if count >= lines.saturating_add(1) || read_start == 0 {
            let all_lines: Vec<&str> = text.lines().collect();
            let start = all_lines.len().saturating_sub(lines);
            return Ok(all_lines[start..].iter().map(|s| s.to_string()).collect());
        }
        window = (window * 2).min(file_len.max(1));
    }
}

fn line_timestamp(line: &str) -> &str {
    line.strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .map(|(ts, _)| ts)
        .unwrap_or("")
}

/// 读取当天所有进程日志（桌面 app-<日期>.log、安卓 app-ui-/app-core-），
/// 各取尾部 N 行后按时间戳合并排序，返回最近 N 行
pub fn read_recent_logs(app_handle: &tauri::AppHandle, lines: usize) -> Result<String, String> {
    let log_dir = get_log_dir(app_handle);
    let today = Local::now().format("%Y-%m-%d").to_string();
    let suffix = format!("-{today}.log");
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if is_app_log_file(&name) && name.ends_with(&suffix) {
                paths.push(entry.path());
            }
        }
    }
    if paths.is_empty() {
        return Ok(String::new());
    }
    paths.sort();

    let mut rows: Vec<String> = Vec::new();
    for path in &paths {
        rows.extend(read_tail_lines(path, lines)?);
    }
    rows.sort_by(|a, b| line_timestamp(a).cmp(line_timestamp(b)));
    let start = rows.len().saturating_sub(lines);
    Ok(rows[start..].join("\n"))
}

pub fn flush() {
    let sender_arc = LOGGER_SENDER.load();
    if let Some(sender) = sender_arc.as_ref() {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel::<()>();
        // 审计 A17：Flush 是控制消息，必须送达——用阻塞 send（通道满时等 worker
        // 排空腾位，worker 每轮循环都在 drain，等待有界）；Entry 才用 try_send 丢弃。
        let _ = sender.send(LogMessage::Flush { ack: ack_tx });
        let _ = ack_rx.recv_timeout(std::time::Duration::from_secs(5));
    }
}

/// panic hook 专用 flush：超时缩短为 500ms，避免 panic=abort 模式下阻塞进程终止 5s
pub fn flush_quick() {
    let sender_arc = LOGGER_SENDER.load();
    if let Some(sender) = sender_arc.as_ref() {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel::<()>();
        // 同 flush()：控制消息必须送达，阻塞 send（注释见上）
        let _ = sender.send(LogMessage::Flush { ack: ack_tx });
        let _ = ack_rx.recv_timeout(std::time::Duration::from_millis(500));
    }
}

pub fn shutdown() {
    let old = LOGGER_SENDER.swap(std::sync::Arc::new(None));
    if let Some(sender) = old.as_ref() {
        let _ = sender.send(LogMessage::Shutdown);
    }
    let mut thread_lock = LOGGER_THREAD.lock();
    if let Some(handle) = thread_lock.take() {
        // 显式 join logger 线程，带 500ms 超时避免 logger 线程卡死时阻塞进程退出
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = handle.join();
            let _ = tx.send(());
        });
        let _ = rx.recv_timeout(std::time::Duration::from_millis(500));
    }
}

/// 判断文件名是否为本应用日志文件（app- 前缀 + .log / .log.gz 后缀）。
/// clear_logs 只删应用自身日志，避免误删日志目录内的其他文件；
/// export_diagnostics 复用同一判断挑选需打包的日志。
pub fn is_app_log_file(name: &str) -> bool {
    name.starts_with("app-") && (name.ends_with(".log") || name.ends_with(".log.gz"))
}

pub fn clear_logs(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let lock = CLEAR_LOGS_MUTEX.lock();

    // 先删除日志文件，再 swap 新 sender，避免新线程创建当天日志文件导致 Windows 下无法删除
    let log_dir = get_log_dir(app_handle);
    if log_dir.exists() {
        if let Ok(entries) = fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                if entry.file_name().to_str().map(is_app_log_file).unwrap_or(false) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    type OldLoggerHandle = (Option<std::sync::Arc<Option<SyncSender<LogMessage>>>>, Option<std::thread::JoinHandle<()>>);
    let (old_sender, old_thread): OldLoggerHandle = {
        // 审计 A17：有界通道（同 init_logger）
        let (new_sender, new_receiver) = sync_channel::<LogMessage>(LOG_CHANNEL_CAPACITY);
        let new_state = LoggerState {
            log_dir: get_log_dir(app_handle),
            current_writer: None,
            current_date: String::new(),
            last_flush: Instant::now(),
        };
        let new_handle = match std::thread::Builder::new()
            .name("logger-worker".to_string())
            .spawn(move || {
                logger_worker(new_state, new_receiver);
            })
        {
            Ok(h) => h,
            Err(e) => {
                crate::log_warn!("logger", "无法创建日志线程: {}", e);
                return Err(format!("无法创建日志线程: {e}"));
            }
        };

        let old = LOGGER_SENDER.swap(std::sync::Arc::new(Some(new_sender)));
        let old_handle = (*LOGGER_THREAD.lock()).replace(new_handle);
        (if old.as_ref().is_some() { Some(old) } else { None }, old_handle)
    };

    if let Some(ref arc_sender) = old_sender {
        if let Some(sender) = arc_sender.as_ref() {
            let _ = sender.send(LogMessage::Shutdown);
        }
    }

    if let Some(handle) = old_thread {
        let _ = handle.join();
    }

    drop(lock);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_config_roundtrip() {
        let levels = [
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
            LogLevel::Silent,
        ];
        for level in levels {
            assert_eq!(LogLevel::from_config_str(level.as_config_str()), Some(level));
        }
    }

    #[test]
    fn log_level_accepts_legacy_aliases() {
        assert_eq!(LogLevel::from_config_str("WARN"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_config_str(" warning "), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_config_str("off"), Some(LogLevel::Silent));
        assert_eq!(LogLevel::from_config_str("none"), Some(LogLevel::Silent));
        assert_eq!(LogLevel::from_config_str("nonsense"), None);
        assert_eq!(log_level_from_config(""), LogLevel::Warn);
        assert_eq!(log_level_from_config("nonsense"), LogLevel::Warn);
    }

    #[test]
    fn default_level_is_warning_and_silent_filters_everything() {
        assert_eq!(LogLevel::default(), LogLevel::Warn);
        assert!(LogLevel::Debug < LogLevel::Warn);
        assert!(LogLevel::Error < LogLevel::Silent);
    }

    #[test]
    fn module_level_spec_parses_and_rejects_bad_items() {
        let parsed = parse_module_levels("login=debug, monitor=info").unwrap();
        assert_eq!(parsed, vec![
            ("login".to_string(), LogLevel::Debug),
            ("monitor".to_string(), LogLevel::Info),
        ]);
        assert!(parse_module_levels("").unwrap().is_empty());
        assert!(parse_module_levels("login").is_err());
        assert!(parse_module_levels("=debug").is_err());
        assert!(parse_module_levels("login=loud").is_err());
    }

    #[test]
    fn log_file_name_keeps_legacy_desktop_name() {
        assert_eq!(log_file_name("app", "2026-11-14"), "app-2026-11-14.log");
        assert_eq!(log_file_name("ui", "2026-11-14"), "app-ui-2026-11-14.log");
        assert_eq!(log_file_name("core", "2026-11-14"), "app-core-2026-11-14.log");
    }

    #[test]
    fn escape_field_escapes_quotes_and_newlines() {
        assert_eq!(escape_field("a\"b\\c\nd"), "a\\\"b\\\\c d");
    }

    #[test]
    fn secrets_are_redacted_from_messages() {
        register_secret("S3cretPass!");
        register_secret("ab");
        assert_eq!(redact("user=alice pass=S3cretPass! ok"), "user=alice pass=*** ok");
        assert_eq!(redact("nothing sensitive"), "nothing sensitive");
    }

    #[test]
    fn is_app_log_file_positive() {
        assert!(is_app_log_file("app-2026-09-13.log"));
        assert!(is_app_log_file("app-20260913120000.log"));
        assert!(is_app_log_file("app-ui-2026-09-13.log"));
        assert!(is_app_log_file("app-2026-09-13.log.gz"));
    }

    #[test]
    fn is_app_log_file_negative() {
        assert!(!is_app_log_file("other.txt"));
        assert!(!is_app_log_file("app-.log.bak"));
        assert!(!is_app_log_file("config.json"));
        assert!(!is_app_log_file("app-2026-09-13"));
        assert!(!is_app_log_file("client-2026-09-13.log"));
    }
}
