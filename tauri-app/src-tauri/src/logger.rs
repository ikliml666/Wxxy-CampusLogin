use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write as IoWrite};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use arc_swap::ArcSwap;
use parking_lot::Mutex;
use std::time::Instant;
use chrono::Local;

const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024;
const MAX_LOG_FILES: usize = 5;
const FLUSH_INTERVAL_MS: u64 = 2000;
const CHANNEL_CAPACITY: usize = 1024;

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

pub fn init_logger(log_dir: PathBuf) {
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("创建日志目录失败: {}", e);
    }
    let today = Local::now().format("%Y-%m-%d").to_string();
    let log_path = log_dir.join(format!("app-{}.log", today));

    let writer = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok()
        .map(BufWriter::new);

    if writer.is_none() {
        eprintln!("[WARN] 无法打开日志文件: {:?}", log_path);
    }

    let (sender, receiver) = channel::<LogMessage>();

    {
        let old = LOGGER_SENDER.swap(std::sync::Arc::new(Some(sender)));
        if let Some(old_sender) = old.as_ref() {
            let _ = old_sender.send(LogMessage::Shutdown); // [忽略错误] 旧日志线程可能已退出
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
        .expect("Failed to spawn logger thread");

    *LOGGER_THREAD.lock() = Some(handle);
}

fn logger_worker(mut state: LoggerState, receiver: std::sync::mpsc::Receiver<LogMessage>) {
    let mut buffer: Vec<LogMessage> = Vec::with_capacity(64);

    loop {
        match receiver.recv_timeout(std::time::Duration::from_millis(FLUSH_INTERVAL_MS)) {
            Ok(LogMessage::Shutdown) => {
                drain_channel(&receiver, &mut buffer);
                flush_messages(&mut state, &mut buffer);
                let _ = state.current_writer.as_mut().map(|w| w.flush()); // [忽略错误] 关闭时 flush 失败无法恢复
                return;
            }
            Ok(msg) => buffer.push(msg),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                drain_channel(&receiver, &mut buffer);
                flush_messages(&mut state, &mut buffer);
                let _ = state.current_writer.as_mut().map(|w| w.flush()); // [忽略错误] 通道断开时 flush 失败无法恢复
                return;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }

        drain_channel(&receiver, &mut buffer);

        if !buffer.is_empty() {
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
                    let _ = writer.write_all(line.as_bytes()); // [忽略错误] 日志写入失败无法恢复，继续处理下一条
                }
                LogMessage::Flush { ack } => {
                    let _ = writer.flush(); // [忽略错误] flush 失败无法恢复
                    let _ = ack.send(());   // [忽略错误] 通知方可能已超时放弃
                }
                LogMessage::Shutdown => {}
            }
        }
        let _ = writer.flush(); // [忽略错误] 批量写入后 flush 失败无法恢复
    } else {
        for msg in buffer.drain(..) {
            if let LogMessage::Flush { ack } = msg {
                let _ = ack.send(()); // [忽略错误] 无写入器时仍需回复 flush 请求
            }
        }
    }
    state.last_flush = Instant::now();
}

fn rotate_if_needed(state: &mut LoggerState) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    if today != state.current_date {
        state.current_date = today.clone();
        let log_path = state.log_dir.join(format!("app-{}.log", today));
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
                let rotated = state.log_dir.join(format!("app-{}.log", timestamp));
                let current = state.log_dir.join(format!("app-{}.log", state.current_date));
                if fs::rename(&current, &rotated).is_ok() {
                    cleanup_old_logs(&state.log_dir);
                } else {
                    crate::log_warn!("logger", "日志轮转rename失败，尝试直接截断");
                    if let Ok(f) = OpenOptions::new().write(true).truncate(true).open(&current) {
                        let _ = f; // [忽略错误] 截断后立即丢弃文件句柄，仅用于清空文件内容
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
        let mut files: Vec<(String, PathBuf)> = entries
            .flatten()
            .filter(|e| {
                e.path().extension().and_then(|e| e.to_str()) == Some("log")
            })
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                Some((name, e.path()))
            })
            .collect();
        files.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, path) in files.iter().skip(MAX_LOG_FILES) {
            let _ = fs::remove_file(path); // [忽略错误] 旧日志删除失败不影响新日志写入
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
        if sender.send(LogMessage::Entry { line: line.clone() }).is_err() && level >= LogLevel::Warn {
            eprint!("{}", line);
        }
    } else if level >= LogLevel::Warn {
        eprint!("{}", line);
    }
}

pub fn set_log_level(level: LogLevel) {
    MIN_LOG_LEVEL.store(std::sync::Arc::new(level));
}

pub fn get_log_level() -> LogLevel {
    (**MIN_LOG_LEVEL.load()).clone()
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
        $crate::logger::log($crate::logger::LogLevel::Debug, $module, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($module:expr, $($arg:tt)*) => {
        $crate::logger::log($crate::logger::LogLevel::Info, $module, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($module:expr, $($arg:tt)*) => {
        $crate::logger::log($crate::logger::LogLevel::Warn, $module, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($module:expr, $($arg:tt)*) => {
        $crate::logger::log($crate::logger::LogLevel::Error, $module, &format!($($arg)*))
    };
}

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
    let log_dir = get_log_dir(app_handle);
    let today = Local::now().format("%Y-%m-%d").to_string();
    let log_path = log_dir.join(format!("app-{}.log", today));

    if !log_path.exists() {
        return Ok(String::new());
    }

    let content = fs::read_to_string(&log_path)
        .map_err(|e| format!("读取日志失败: {}", e))?;

    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(lines);
    Ok(all_lines[start..].join("\n"))
}

pub fn flush() {
    let sender_arc = LOGGER_SENDER.load();
    if let Some(sender) = sender_arc.as_ref() {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel::<()>();
        let _ = sender.send(LogMessage::Flush { ack: ack_tx }); // [忽略错误] 日志通道可能已关闭
        let _ = ack_rx.recv_timeout(std::time::Duration::from_secs(5)); // [忽略错误] 等待 flush 超时，日志可能未完全落盘
    }
}

pub fn shutdown() {
    let old = LOGGER_SENDER.swap(std::sync::Arc::new(None));
    if let Some(sender) = old.as_ref() {
        let _ = sender.send(LogMessage::Shutdown);
    }
    let mut thread_lock = LOGGER_THREAD.lock();
    if let Some(handle) = thread_lock.take() {
        let _ = handle.join();
    }
}

pub fn clear_logs(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let lock = CLEAR_LOGS_MUTEX.lock();

    let (old_sender, old_thread): (Option<std::sync::Arc<Option<Sender<LogMessage>>>>, Option<std::thread::JoinHandle<()>>) = {
        let (new_sender, new_receiver) = channel::<LogMessage>();
        let new_state = LoggerState {
            log_dir: get_log_dir(app_handle),
            current_writer: None,
            current_date: String::new(),
            last_flush: Instant::now(),
        };
        let new_handle = std::thread::Builder::new()
            .name("logger-worker".to_string())
            .spawn(move || {
                logger_worker(new_state, new_receiver);
            })
            .expect("Failed to spawn logger thread");

        let old = LOGGER_SENDER.swap(std::sync::Arc::new(Some(new_sender)));
        let old_handle = std::mem::replace(&mut *LOGGER_THREAD.lock(), Some(new_handle));
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

    let log_dir = get_log_dir(app_handle);
    if log_dir.exists() {
        if let Ok(entries) = fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    drop(lock);

    Ok(())
}
