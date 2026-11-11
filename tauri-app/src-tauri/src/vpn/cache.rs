//! WebVPN 会话进程内缓存（2026-10-25 新增，自动共存路由支撑）：
//! 命令层路由命中 Wrapped 时惰性建立/复用，失效（[`WebVpnSession::is_alive`] ==
//! false 或凭据/总开关变更）用保存凭据静默重登一次。密码仅内存传递，任何日志
//! 不得出现明文凭据。

use super::cas::{kaptcha_templates, CasClient};
use super::error::{map_cas_login_error, map_webvpn_error};
use super::rsa::rsa_encrypt_hex;
use super::session::WebVpnSession;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::future::Future;
use std::sync::Arc;

struct SessionCache<T> {
    session: RwLock<Option<Arc<T>>>,
    refresh: tokio::sync::Mutex<()>,
}

impl<T> SessionCache<T> {
    fn new() -> Self {
        Self {
            session: RwLock::new(None),
            refresh: tokio::sync::Mutex::new(()),
        }
    }

    fn cached(&self) -> Option<Arc<T>> {
        self.session.read().clone()
    }

    fn reset(&self) {
        *self.session.write() = None;
    }

    fn store(&self, session: T) {
        *self.session.write() = Some(Arc::new(session));
    }

    async fn ensure_alive_with<Probe, ProbeFuture, Login, LoginFuture>(
        &self,
        probe: Probe,
        login: Login,
    ) -> Result<Arc<T>, String>
    where
        Probe: Fn(Arc<T>) -> ProbeFuture,
        ProbeFuture: Future<Output = bool>,
        Login: FnOnce() -> LoginFuture,
        LoginFuture: Future<Output = Result<T, String>>,
    {
        if let Some(session) = self.cached() {
            if probe(session.clone()).await {
                return Ok(session);
            }
        }

        let _guard = self.refresh.lock().await;
        // 等待期间其他调用可能已重登；必须复查最新缓存，不能直接沿用此前结果。
        if let Some(session) = self.cached() {
            if probe(session.clone()).await {
                return Ok(session);
            }
        }
        self.reset();
        let session = Arc::new(login().await?);
        *self.session.write() = Some(session.clone());
        Ok(session)
    }
}

lazy_static! {
    static ref SESSION_CACHE: SessionCache<WebVpnSession> = SessionCache::new();
}

/// 重置缓存（保存/清除凭据、切换总开关、会话失效时调用）。
pub fn reset() {
    SESSION_CACHE.reset();
}

/// 取当前缓存会话克隆（不校验活性：活性判断是 async 调用，不得持锁跨 await）。
/// 保留给既有库调用方；当前双端命令统一走 ensure_alive。
#[allow(dead_code)]
pub fn cached() -> Option<Arc<WebVpnSession>> {
    SESSION_CACHE.cached()
}

/// 建立全新会话并存入缓存。保留原有公共入口；命令热路径使用 ensure_alive 的单飞逻辑。
#[allow(dead_code)]
pub async fn login_and_cache(account: &str, password: &str) -> Result<Arc<WebVpnSession>, String> {
    let session = login_new_session(account, password).await?;
    let arc = Arc::new(session);
    *SESSION_CACHE.session.write() = Some(arc.clone());
    Ok(arc)
}

/// 取有效会话：缓存命中且探活通过直接复用；否则静默重登一次（kaptcha 自动
/// 解题 → CAS 登录 → TGT 换 ST → 网关 302 链，无人工交互）。`account`/`password`
/// 为 CAS 明文凭据，由调用方从加密存储解出，仅内存传递。
pub async fn ensure_alive(account: &str, password: &str) -> Result<Arc<WebVpnSession>, String> {
    SESSION_CACHE
        .ensure_alive_with(
            |session| async move { session.is_alive().await },
            || login_new_session(account, password),
        )
        .await
}

/// 把外部建立的会话存入缓存（连接测试走完整链路成功后复用其登录态，
/// 免去紧接着的业务请求再登一次）。
pub fn store(session: WebVpnSession) {
    SESSION_CACHE.store(session);
}

/// 一次性建立全新 WebVPN 会话（不入缓存）：CAS 账密登录拿 TGT → WebVPN 静默
/// 登录。连接测试的分步展示也走本函数。
pub async fn login_new_session(account: &str, password: &str) -> Result<WebVpnSession, String> {
    let cas = CasClient::new().map_err(|e| format!("CAS 客户端创建失败: {e}"))?;
    let pwd_hex = rsa_encrypt_hex(password).map_err(|e| format!("密码加密失败: {e}"))?;
    let ok = cas
        .login_with_captcha(account, &pwd_hex, kaptcha_templates(), 3)
        .await
        .map_err(map_cas_login_error)?;
    WebVpnSession::login(&cas, &ok.tgt)
        .await
        .map_err(map_webvpn_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn cache_reset_and_relogin_semantics() {
        reset();
        assert!(SESSION_CACHE.cached().is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn 无缓存并发只重登一次且共享同一会话() {
        let cache = Arc::new(SessionCache::<usize>::new());
        let logins = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            let logins = logins.clone();
            tasks.push(tokio::spawn(async move {
                cache
                    .ensure_alive_with(
                        |_| async { true },
                        || async {
                            logins.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(25)).await;
                            Ok::<_, String>(42)
                        },
                    )
                    .await
                    .unwrap()
            }));
        }
        let mut sessions = Vec::new();
        for task in tasks {
            sessions.push(task.await.unwrap());
        }
        assert_eq!(logins.load(Ordering::SeqCst), 1);
        assert!(sessions
            .iter()
            .all(|session| Arc::ptr_eq(session, &sessions[0])));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn 失效并发只重登一次且共享新会话() {
        let cache = Arc::new(SessionCache::<usize>::new());
        cache.store(1);
        let alive = Arc::new(AtomicBool::new(false));
        let logins = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            let alive = alive.clone();
            let logins = logins.clone();
            tasks.push(tokio::spawn(async move {
                let probe_alive = alive.clone();
                cache
                    .ensure_alive_with(
                        move |_| {
                            let alive = probe_alive.clone();
                            async move { alive.load(Ordering::SeqCst) }
                        },
                        || async {
                            logins.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(25)).await;
                            alive.store(true, Ordering::SeqCst);
                            Ok::<_, String>(2)
                        },
                    )
                    .await
                    .unwrap()
            }));
        }
        let mut sessions = Vec::new();
        for task in tasks {
            sessions.push(task.await.unwrap());
        }
        assert_eq!(logins.load(Ordering::SeqCst), 1);
        assert!(sessions.iter().all(|session| **session == 2));
        assert!(sessions
            .iter()
            .all(|session| Arc::ptr_eq(session, &sessions[0])));
    }
}
