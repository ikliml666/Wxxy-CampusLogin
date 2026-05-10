use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write as IoWrite};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use arc_swap::ArcSwap;
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

#[allow(dead_code)]
struct LogEntry {
    level: LogLevel,
    line: String,
}

struct LoggerState {
    log_dir: PathBuf,
    current_writer: Option<BufWriter<File>>,
    current_date: String,
    last_flush: Instant,
}

lazy_static::lazy_static! {
    static ref LOGGER_SENDER: ArcSwap<Option<Sender<LogEntry>>> = ArcSwap::from(std::sync::Arc::new(None));
}

pub fn init_logger(log_dir: PathBuf) {
    let _ = fs::create_dir_all(&log_dir);
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

    let (sender, receiver) = channel::<LogEntry>();

    {
        let old = LOGGER_SENDER.swap(std::sync::Arc::new(Some(sender)));
        drop(old);
    }

    let state = LoggerState {
        log_dir,
        current_writer: writer,
        current_date: today,
        last_flush: Instant::now(),
    };

    std::thread::Builder::new()
        .name("logger-worker".to_string())
        .spawn(move || {
            logger_worker(state, receiver);
        })
        .expect("Failed to spawn logger thread");
}

fn logger_worker(mut state: LoggerState, receiver: std::sync::mpsc::Receiver<LogEntry>) {
    let mut buffer: Vec<LogEntry> = Vec::with_capacity(64);

    loop {
        match receiver.recv_timeout(std::time::Duration::from_millis(FLUSH_INTERVAL_MS)) {
            Ok(entry) => buffer.push(entry),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                drain_channel(&receiver, &mut buffer);
                flush_buffer(&mut state, &mut buffer);
                let _ = state.current_writer.as_mut().map(|w| w.flush());
                return;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }

        drain_channel(&receiver, &mut buffer);

        if !buffer.is_empty() {
            flush_buffer(&mut state, &mut buffer);
        }
    }
}

fn drain_channel(receiver: &std::sync::mpsc::Receiver<LogEntry>, buffer: &mut Vec<LogEntry>) {
    loop {
        match receiver.try_recv() {
            Ok(entry) => {
                buffer.push(entry);
                if buffer.len() >= CHANNEL_CAPACITY {
                    break;
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

fn flush_buffer(state: &mut LoggerState, buffer: &mut Vec<LogEntry>) {
    if buffer.is_empty() {
        return;
    }

    rotate_if_needed(state);

    if let Some(ref mut writer) = state.current_writer {
        for entry in buffer.drain(..) {
            let _ = writer.write_all(entry.line.as_bytes());
        }
        let _ = writer.flush();
    } else {
        buffer.clear();
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
                    crate::log_debug!("logger", "日志轮转rename失败，尝试直接截断");
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
            let _ = fs::remove_file(path);
        }
    }
}

pub fn log(level: LogLevel, module: &str, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [{}] [{}] {}\n", timestamp, level.as_str(), module, message);

    if level >= LogLevel::Warn {
        eprint!("{}", line);
    }

    if let Some(sender) = LOGGER_SENDER.load().as_ref().clone() {
        let _ = sender.send(LogEntry { level, line });
    }
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

pub fn get_log_dir(_app_handle: &tauri::AppHandle) -> PathBuf {
    let install_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
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

pub fn clear_logs(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let log_dir = get_log_dir(app_handle);

    let old_sender_existed = {
        let old = LOGGER_SENDER.swap(std::sync::Arc::new(None));
        old.as_ref().is_some()
    };

    if old_sender_existed {
        std::thread::sleep(std::time::Duration::from_millis(FLUSH_INTERVAL_MS as u64 + 500));
    }

    if log_dir.exists() {
        if let Ok(entries) = fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    init_logger(log_dir);

    Ok(())
}
