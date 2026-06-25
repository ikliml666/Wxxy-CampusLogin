//! 适配器缓存与访问 API
//!
//! 提供 TTL 缓存（5 秒）与公共访问函数，避免重复调用 Win32 GetAdaptersAddresses。
//! 缓存优化（读写锁、后台刷新）将在 T4.3 实施。

use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::time::Instant;
use std::sync::atomic::AtomicBool;
use crate::network::discovery::{
    Adapter, AdapterDetail, AdapterQueryResult, DisabledAdapter, new_command,
};

/// 缓存条目：(adapters, details, disabled, timestamp)
type AdapterCacheEntry = (Vec<Adapter>, Vec<AdapterDetail>, Vec<DisabledAdapter>, Instant);

lazy_static! {
    static ref ADAPTER_CACHE: RwLock<Option<AdapterCacheEntry>> = RwLock::new(None);
}

const ADAPTER_CACHE_TTL_SECS: u64 = 5;

fn query_adapters_cached_inner() -> AdapterQueryResult {
    {
        let cache = ADAPTER_CACHE.read();
        if let Some((adapters, details, disabled, ts)) = cache.as_ref() {
            if ts.elapsed().as_secs() < ADAPTER_CACHE_TTL_SECS {
                return Ok((adapters.clone(), details.clone(), disabled.clone()));
            }
        }
    }
    let result = crate::network::discovery::query_adapters_addresses()?;
    {
        let mut cache = ADAPTER_CACHE.write();
        *cache = Some((result.0.clone(), result.1.clone(), result.2.clone(), Instant::now()));
    }
    Ok(result)
}

pub fn get_all_adapters_force() -> AdapterQueryResult {
    ADAPTER_CACHE.write().take();
    query_adapters_cached_inner()
}

pub fn get_adapters_cached() -> Result<Vec<Adapter>, String> {
    let (adapters, _, _) = query_adapters_cached_inner()?;
    Ok(adapters)
}

/// 异步版本的 get_adapters_cached，供 async 上下文调用。
///
/// 快速路径：缓存命中时仅 Mutex lock + clone（非阻塞），直接返回，避免 spawn_blocking 开销。
/// 慢路径：缓存未命中时，通过 spawn_blocking 把阻塞的 Win32 GetAdaptersAddresses 调用
///        转移到阻塞线程池，避免阻塞 async 运行时。
pub async fn get_adapters_cached_async() -> Result<Vec<Adapter>, String> {
    // 快速路径：缓存命中直接返回（仅 Mutex lock + clone，非阻塞）
    {
        let cache = ADAPTER_CACHE.read();
        if let Some((adapters, _details, _disabled, ts)) = cache.as_ref() {
            if ts.elapsed().as_secs() < ADAPTER_CACHE_TTL_SECS {
                return Ok(adapters.clone());
            }
        }
    }
    // 慢路径：缓存未命中，spawn_blocking 执行阻塞的 Win32 GetAdaptersAddresses 调用
    tokio::task::spawn_blocking(get_adapters_cached)
        .await
        .map_err(|e| format!("适配器查询任务失败: {e}"))?
}

pub fn get_disabled_adapters_cached() -> Result<Vec<DisabledAdapter>, String> {
    let (_, _, disabled) = query_adapters_cached_inner()?;
    Ok(disabled)
}

pub fn get_adapters_force() -> Result<Vec<Adapter>, String> {
    ADAPTER_CACHE.write().take();
    get_adapters_cached()
}

pub fn get_adapter_details_cached() -> Result<Vec<AdapterDetail>, String> {
    let (_, details, _) = query_adapters_cached_inner()?;
    Ok(details)
}

pub fn validate_adapter_name(name: &str) -> Result<(), String> {
    if name.is_empty() { return Err("适配器名称不能为空".to_string()); }
    if name.len() > 128 { return Err("适配器名称过长".to_string()); }
    const FORBIDDEN: &[char] = &['&', '|', ';', '`', '$', '(', ')', '<', '>', '"', '\'', '\n', '\r', '\0'];
    if name.chars().any(|c| FORBIDDEN.contains(&c)) { return Err("适配器名称包含非法字符".to_string()); }
    Ok(())
}

pub fn enable_adapter(adapter_name: &str) -> Result<(), String> {
    validate_adapter_name(adapter_name)?;

    // netsh 命令行参数（适配器名含空格时需双引号包裹）
    let netsh_args = format!("interface set interface \"{adapter_name}\" enable");

    if crate::platform::elevation::is_admin() {
        // 管理员：直接执行 netsh
        crate::log_info!("adapter", "管理员直写启用适配器: {}", adapter_name);
        let output = new_command("netsh")
            .args(["interface", "set", "interface", adapter_name, "enable"])
            .output()
            .map_err(|e| format!("启用适配器失败: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr_trimmed = stderr.trim();
            return Err(if stderr_trimmed.is_empty() {
                "启用适配器失败：netsh 返回非零退出码但未输出错误信息".to_string()
            } else {
                format!("启用适配器失败: {stderr_trimmed}")
            });
        }
    } else {
        // 非管理员：COM 静默提权执行 netsh（不弹 UAC）
        crate::log_info!("adapter", "非管理员运行，COM ShellExec 提权启用适配器: {}", adapter_name);
        match crate::platform::elevation::shell_exec_elevated("netsh", &netsh_args, true) {
            Ok(()) => {
                crate::log_info!("adapter", "COM ShellExec 提权启用适配器成功: {}", adapter_name);
            }
            Err(com_err) => {
                // COM 失败：降级 ShellExecuteW runas（会弹 UAC）
                crate::log_warn!("adapter", "COM ShellExec 失败: {}，降级到 ShellExecuteW runas", com_err);
                crate::platform::elevation::run_elevated("netsh", &netsh_args)
                    .map_err(|e| format!("提权启用适配器失败（COM 和 UAC 均失败）: COM错误={com_err}; UAC错误={e}"))?;
                crate::log_info!("adapter", "ShellExecuteW runas 启用适配器成功: {}", adapter_name);
            }
        }
    }

    // 启用后强制清缓存，让下次查询拿到最新状态
    ADAPTER_CACHE.write().take();
    // 同步刷新注册表缓存，避免 is_admin_disabled_via_registry 返回过时结果
    #[cfg(target_os = "windows")]
    crate::network::discovery::registry::refresh_class_subkey_cache();
    crate::log_info!("adapter", "已清空适配器缓存");

    Ok(())
}

pub fn wait_for_adapter(max_wait_ms: u64, is_quitting: &std::sync::atomic::AtomicBool) -> Result<Vec<Adapter>, String> {
    let start = std::time::Instant::now();
    let mut delay_ms: u64 = 1000;

    while start.elapsed().as_millis() < max_wait_ms as u128 {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(vec![]);
        }

        let adapters = get_adapters_force()?;
        if !adapters.is_empty() {
            return Ok(adapters);
        }

        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        delay_ms = (delay_ms * 2).min(5000);
    }

    get_adapters_cached()
}

pub fn poll_adapter_ip_quick(adapter_name: &str, timeout_ms: u64, is_quitting: &AtomicBool) -> bool {
    let start = std::time::Instant::now();
    let interval = std::time::Duration::from_millis(100);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    // 记录初始 IP，只有 IP 变为非空且与初始值不同时才认为续租成功
    let initial_ip = get_adapters_force()
        .ok()
        .and_then(|list| list.iter().find(|a| a.name == adapter_name).map(|a| a.ip.clone()))
        .unwrap_or_default();
    while start.elapsed() < timeout {
        if is_quitting.load(std::sync::atomic::Ordering::Acquire) {
            return false;
        }
        std::thread::sleep(interval);
        if let Ok(adapters) = get_adapters_force() {
            if let Some(a) = crate::network::find_by_name(&adapters, adapter_name) {
                if !a.ip.is_empty() && a.ip != initial_ip {
                    return true;
                }
            }
        }
    }
    false
}

/// 适配器缓存后台刷新间隔（秒）。
/// 略短于 TTL（5 秒），保证缓存始终新鲜。
const CACHE_REFRESH_INTERVAL_SECS: u64 = 4;

/// 启动适配器缓存后台刷新任务。
///
/// 每 4 秒主动刷新一次适配器缓存，保证缓存始终新鲜，避免调用方在缓存过期后同步等待。
/// 刷新失败时记录日志并保留旧缓存，避免缓存雪崩。
/// 通过 BackgroundTaskManager 统一管理生命周期，shutdown 时自动取消。
pub fn start_cache_refresh_task(
    task_manager: &crate::infra::task_manager::BackgroundTaskManager,
) -> Result<(), String> {
    task_manager.spawn("adapter_cache_refresh", |cancel_token| async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(CACHE_REFRESH_INTERVAL_SECS));
        interval.tick().await; // 跳过第一次立即触发
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // 在 spawn_blocking 中执行阻塞的 Win32 GetAdaptersAddresses 调用
                    let result = tokio::task::spawn_blocking(query_adapters_cached_inner).await;
                    match result {
                        Ok(Ok(_)) => {
                            crate::log_debug!("adapter", "后台刷新适配器缓存成功");
                        }
                        Ok(Err(e)) => {
                            crate::log_warn!("adapter", "后台刷新适配器缓存失败（保留旧缓存）: {}", e);
                        }
                        Err(e) => {
                            crate::log_warn!("adapter", "后台刷新任务 join 异常: {}", e);
                        }
                    }
                }
                _ = cancel_token.cancelled() => {
                    crate::log_info!("adapter", "适配器缓存后台刷新任务已停止");
                    break;
                }
            }
        }
    })
}
