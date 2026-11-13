//! Portal 状态单点:UI 命令与后台巡检共用同一次探测结果。
//!
//! 此前 UI 的 `check_portal_status` 在 UI 进程自己探测、巡检在 `:core` 再探一次,
//! 两个进程各自新建旁路代理连接打同一个网关:一次网络抖动会被记成两条失败,
//! 界面还会在「已在线/不可达」之间闪。这里把探测收成单点——缓存窗口内复用、
//! 失败后短暂退避、并发调用共享同一次探测。

use campus_login_lib::auth::portal::PortalStatus;
use tokio::sync::Mutex;

use super::platform::CoreCtx;

/// 成功结果缓存时长:窗口内的重复请求直接复用,不再打网关
const SUCCESS_TTL_MS: u64 = 5_000;
/// 失败结果退避时长:网络抖动期间连环探测只会把同一次失败记成多条
const FAILURE_BACKOFF_MS: u64 = 10_000;

struct Cached {
    status: PortalStatus,
    at_ms: u64,
    failed: bool,
    /// 探测时的源地址:WiFi 换网后旧结论必须失效,不能拿上一张网的在线状态冒充
    source_ip: String,
}

static CACHE: Mutex<Option<Cached>> = Mutex::const_new(None);

/// 取 Portal 状态。`force` 跳过缓存(用户手动刷新/立即检测)。
pub async fn portal_status(ctx: &CoreCtx, force: bool) -> Result<PortalStatus, String> {
    let ip = ctx.cached_source_ip().unwrap_or_default();
    if !force {
        if let Some(cached) = fresh(&ip) {
            return Ok(cached);
        }
    }
    // 单飞:探测期间持锁,并发调用排队后重新检查缓存,不会重复打网关
    let mut guard = CACHE.lock().await;
    if !force {
        if let Some(cached) = fresh(&ip) {
            return Ok(cached);
        }
    }
    let status = probe_on_little_cores(ip.clone()).await?;
    *guard = Some(Cached {
        failed: !status.reachable,
        status: status.clone(),
        at_ms: super::platform::now_ms(),
        source_ip: ip,
    });
    Ok(status)
}

/// 丢弃缓存(立即检测/网络环境变化后强制重新探测)
pub async fn invalidate() {
    *CACHE.lock().await = None;
}

/// 缓存仍有效时返回副本;探测进行中(try_lock 失败)按未命中处理,由单飞锁兜住
fn fresh(ip: &str) -> Option<PortalStatus> {
    let guard = CACHE.try_lock().ok()?;
    let cached = guard.as_ref()?;
    if cached.source_ip != ip {
        return None;
    }
    let age = super::platform::now_ms().saturating_sub(cached.at_ms);
    let ttl = if cached.failed { FAILURE_BACKOFF_MS } else { SUCCESS_TTL_MS };
    (age < ttl).then(|| cached.status.clone())
}

/// Portal 全量探测:专用短命线程执行并绑定小核(检测是 60s 一拍的稳态周期任务,
/// 线程创建 ~1ms 可忽略)。tokio worker 与 spawn_blocking 池都是共享的,直接绑
/// 会把登录等前台任务一并拖到小核——所以用独立线程。绑核失败(节点缺失/无大小核
/// 拓扑)线程照常运行,仅回落内核自动调度。
async fn probe_on_little_cores(ip: String) -> Result<PortalStatus, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let ip_for_thread = ip.clone();
    // 裸线程不在 Tokio runtime 上下文内:check_portal_full 内部经 block_on_sync 驱动的
    // reqwest,构造超时计时器(tokio::time::sleep → Handle::current)时必须有 reactor
    // context,缺失即 panic "there is no reactor running"(2026-09-11 真机每拍必现)。
    // 先在本 async 上下文取 handle,线程内 enter——与桌面 commands/login.rs 的
    // scope 裸线程同款处理(2026-09-05 注销流程崩溃事故根因);绑小核意图不变。
    let probe_handle = tokio::runtime::Handle::try_current().ok();
    let spawned = std::thread::Builder::new()
        .name("portal-probe".to_string())
        .spawn(move || {
            let _guard = probe_handle.as_ref().map(|h| h.enter());
            crate::cpu_affinity::pin_current_thread_to_little_cores();
            // panic 兜底:裸线程 panic 直接 unwind 时 rx.await 只会得到"探测线程提前
            // 退出",真实原因(reactor context 缺失)就此丢失、每拍静默失败。捕获后
            // 转为 Err 结果回传,默认 panic hook 仍会把消息打到 logcat。
            let probed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                campus_login_lib::auth::portal::check_portal_full(&ip_for_thread, None)
            }));
            let result = probed.unwrap_or_else(|_| Err("探测线程 panic".to_string()));
            let _ = tx.send(result);
        });
    match spawned {
        Ok(_) => rx.await.map_err(|e| format!("探测线程提前退出: {e}"))?,
        Err(e) => {
            eprintln!("[portal_probe] 线程创建失败,降级共享阻塞池: {e}");
            // 线程创建失败降级回共享阻塞池(仅保正确性;原始 ip 未被 move)
            tokio::task::spawn_blocking(move || {
                campus_login_lib::auth::portal::check_portal_full(&ip, None)
            })
            .await
            .unwrap_or_else(|join| Err(format!("探测任务失败: {join}")))
        }
    }
}