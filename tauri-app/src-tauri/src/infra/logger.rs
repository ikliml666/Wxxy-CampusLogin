use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};
use arc_swap::ArcSwap;
use parking_lot::Mutex;
use std::time::Instant;
use chrono::Local;

const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;
const MAX_LOG_FILES: usize = 5;
const FLUSH_INTERVAL_MS: u64 = 2000;
const CHANNEL_CAPACITY: usize = 1024;

static LOG_RETENTION_DAYS: AtomicU32 = AtomicU32::new(7);

pub fn set_log_retention_days(days: u32) {
    LOG_RETENTION_DAYS.store(days, AtomicOrdering::Relaxed);
}

pub fn get_log_retention_days() -> u32 {
    LOG_RETENTION_DAYS.load(AtomicOrdering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
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
    static ref LOGGER_SENDER: ArcSwap<Option<Sender<LogMessage>>> = ArcSwap::from(std::sync::Arc::new(None));
    static ref CLEAR_LOGS_MUTEX: Mutex<()> = Mutex::new(());
    static ref LOGGER_THREAD: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
    static ref MIN_LOG_LEVEL: ArcSwap<LogLevel> = ArcSwap::from(std::sync::Arc::new(LogLevel::Info));
}

pub fn init_logger(log_dir: PathBuf) -> Result<(), String> {
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("创建日志目录失败: {e}");
    }
    let today = Local::now().format("%Y-%m-%d").to_string();
    let log_path = log_dir.join(format!("app-{today}.log"));

    let writer = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(BufWriter::new);

    if writer.is_none() {
        eprintln!("[WARN] 无法打开日志文件: {log_path:?}");
    }

    let (sender, receiver) = channel::<LogMessage>();

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

        drain_channel(&receiver, &mut buffer);

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
        for msg in buffer.drain(..) {
            match msg {
                LogMessage::Entry { line, .. } => {
                    let _ = writer.write_all(line.as_bytes());
                }
                LogMessage::Flush { ack } => {
                    let _ = writer.flush();
                    let _ = ack.send(());
                }
                LogMessage::Shutdown => {}
            }
        }
        let _ = writer.flush();
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
        state.current_date = today.clone();
        let log_path = state.log_dir.join(format!("app-{today}.log"));
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
                let rotated = state.log_dir.join(format!("app-{timestamp}.log"));
                let current = state.log_dir.join(format!("app-{}.log", state.current_date));
                if fs::rename(&current, &rotated).is_ok() {
                    cleanup_old_logs(&state.log_dir);
                } else {
                    crate::log_warn!("logger", "日志轮转rename失败，尝试直接截断");
                    if let Ok(f) = OpenOptions::new().write(true).truncate(true).open(&current) {
                        let _ = f;
                    }
                }

                let new_path = state.log_dir.join(format!("app-{}.log", state.current_date));
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

fn cleanup_old_logs(log_dir: &PathBuf) {
    if let Ok(entries) = fs::read_dir(log_dir) {
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
            .flatten()
            .filter(|e| {
                e.path().extension().and_then(|e| e.to_str()) == Some("log")
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

pub fn log(level: LogLevel, module: &str, message: &str) {
    if level < **MIN_LOG_LEVEL.load() {
        return;
    }

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [{}] [{}] {}\n", timestamp, level.as_str(), module, message);

    if let Some(sender) = LOGGER_SENDER.load().as_ref().clone() {
        // BE-C-01: send 失败时 SendError 会把未发送的 LogMessage 归还（e.0），
        // 直接取回其中的 line 打印，避免额外 clone 一份 line。
        if let Err(e) = sender.send(LogMessage::Entry { line }) {
            if level >= LogLevel::Warn {
                if let LogMessage::Entry { line } = e.0 {
                    eprint!("{line}");
                }
            }
        }
    } else if level >= LogLevel::Warn {
        eprint!("{line}");
    }
}

pub fn set_log_level(level: LogLevel) {
    MIN_LOG_LEVEL.store(std::sync::Arc::new(level));
}

pub fn get_log_level() -> LogLevel {
    **MIN_LOG_LEVEL.load()
}

#[tauri::command]
pub fn set_debug_mode(enabled: bool) -> Result<bool, String> {
    let level = if enabled { LogLevel::Debug } else { LogLevel::Info };
    set_log_level(level);
    Ok(enabled)
}

#[tauri::command]
pub fn get_debug_mode() -> Result<bool, String> {
    Ok(get_log_level() == LogLevel::Debug)
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

pub fn read_recent_logs(app_handle: &tauri::AppHandle, lines: usize) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};

    let log_dir = get_log_dir(app_handle);
    let today = Local::now().format("%Y-%m-%d").to_string();
    let log_path = log_dir.join(format!("app-{today}.log"));

    if !log_path.exists() {
        return Ok(String::new());
    }

    let mut file = File::open(&log_path).map_err(|e| format!("读取日志失败: {e}"))?;
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file_len == 0 || lines == 0 {
        return Ok(String::new());
    }

    // BE-C-01: 从文件尾部按块倒读，只解析所需尾部 N 行，替代"整读最多 5MB 文件 + 全量 collect"。
    // 窗口按 2 倍增长向后扩展（从 8KB 起），直到尾部字节行数满足要求或覆盖整个文件；
    // 用 str::lines() 语义解析（与旧实现一致，保留行数截断语义），跨块行边界由
    // 连续字节流自然拼接还原，仅窗口起点可能截断首行（该行在取尾部 N 行时被丢弃）。
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
        // 需要 count >= lines + 1：保证窗口首行的不完整残片能被丢弃，只保留完整行
        if count >= lines.saturating_add(1) || read_start == 0 {
            let all_lines: Vec<&str> = text.lines().collect();
            let start = all_lines.len().saturating_sub(lines);
            return Ok(all_lines[start..].join("\n"));
        }
        window = (window * 2).min(file_len.max(1));
    }
}

pub fn flush() {
    let sender_arc = LOGGER_SENDER.load();
    if let Some(sender) = sender_arc.as_ref() {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel::<()>();
        let _ = sender.send(LogMessage::Flush { ack: ack_tx });
        let _ = ack_rx.recv_timeout(std::time::Duration::from_secs(5));
    }
}

/// panic hook 专用 flush：超时缩短为 500ms，避免 panic=abort 模式下阻塞进程终止 5s
pub fn flush_quick() {
    let sender_arc = LOGGER_SENDER.load();
    if let Some(sender) = sender_arc.as_ref() {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel::<()>();
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

pub fn clear_logs(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let lock = CLEAR_LOGS_MUTEX.lock();

    // 先删除日志文件，再 swap 新 sender，避免新线程创建当天日志文件导致 Windows 下无法删除
    let log_dir = get_log_dir(app_handle);
    if log_dir.exists() {
        if let Ok(entries) = fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    type OldLoggerHandle = (Option<std::sync::Arc<Option<Sender<LogMessage>>>>, Option<std::thread::JoinHandle<()>>);
    let (old_sender, old_thread): OldLoggerHandle = {
        let (new_sender, new_receiver) = channel::<LogMessage>();
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
