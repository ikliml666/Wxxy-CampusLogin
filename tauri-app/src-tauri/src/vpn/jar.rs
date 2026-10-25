//! 会话 Cookie Jar：委托 reqwest 内置 Jar 的同时记录 (名, 值) 供会话检测与持久化。
//!
//! reqwest 0.12 的 Client 无法读回内部 CookieStore（私有类型），故自实现
//! `reqwest::cookie::CookieStore`（set_cookies / cookies 两方法，0.12.28 均为 `&self`），
//! 委托内置 `reqwest::cookie::Jar` 并记录 (名, 值) 供 check_session / session.json 持久化使用
//! （DPAPI 加密由上层做）。

use reqwest::cookie::CookieStore;
use reqwest::header::HeaderValue;
use std::sync::Mutex;

/// restore 回填的目标域：M1 会话恢复的核心是门户会话（customsid / Authorization /
/// rememberMe，REPORT.md 三节实测均种在 my.cwxu.edu.cn）。
/// 多域恢复（WebVPN 等）待 M4 按需扩展；(名, 值) 快照本身不含域信息（计划冻结接口）。
const RESTORE_URL: &str = "https://my.cwxu.edu.cn/";

/// 委托内置 Jar 的记录型 CookieStore（`CasClient::new` 内经
/// `.cookie_provider(Arc<RecordingJar>)` 挂载）。
pub struct RecordingJar {
    inner: reqwest::cookie::Jar,
    recorded: Mutex<Vec<(String, String)>>,
}

impl Default for RecordingJar {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingJar {
    pub fn new() -> Self {
        Self {
            inner: reqwest::cookie::Jar::default(),
            recorded: Mutex::new(Vec::new()),
        }
    }

    /// 全部已见 cookie（供 session.json 持久化）。
    pub fn snapshot(&self) -> Vec<(String, String)> {
        self.locked().clone()
    }

    /// 启动回填：把快照 cookies 重新塞回内置 Jar（按门户域，使请求真正携带）并计入记录。
    /// 同名 cookie 覆盖旧值（幂等，重复 restore 不累积）。
    pub fn restore(&self, cookies: &[(String, String)]) {
        {
            let mut recorded = self.locked();
            for (name, value) in cookies {
                recorded.retain(|(n, _)| n != name);
                recorded.push((name.clone(), value.clone()));
            }
        }
        let url: reqwest::Url = RESTORE_URL.parse().expect("RESTORE_URL 恒为合法 URL");
        for (name, value) in cookies {
            // 头格式同 Set-Cookie；非法字符（非可见 ASCII）的头跳过，不使回填整体失败
            if let Ok(head) = HeaderValue::from_str(&format!("{name}={value}; Path=/")) {
                let mut iter = std::iter::once(&head);
                self.inner.set_cookies(&mut iter, &url);
            }
        }
    }

    /// 浅检测：是否存在指定 cookie（如 check_session 用 customsid）。
    pub fn has(&self, name: &str) -> bool {
        self.locked().iter().any(|(n, _)| n == name)
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, Vec<(String, String)>> {
        self.recorded.lock().expect("RecordingJar 互斥锁中毒")
    }
}

impl CookieStore for RecordingJar {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &HeaderValue>,
        url: &reqwest::Url,
    ) {
        let heads: Vec<&HeaderValue> = cookie_headers.collect();
        {
            let mut recorded = self.locked();
            for head in &heads {
                let Ok(s) = head.to_str() else { continue };
                // Set-Cookie: NAME=VALUE; 属性… → 取第一个 ';' 前的键值对，
                // 按第一个 '=' 切分（值可含 '='，如 base64 形态的 rememberMe）
                let pair = s.split(';').next().unwrap_or_default();
                if let Some((name, value)) = pair.split_once('=') {
                    let name = name.trim();
                    if !name.is_empty() {
                        // (名, 值) 模型不含域信息，同名后到覆盖先到（与浏览器单域语义一致）
                        recorded.retain(|(n, _)| n != name);
                        recorded.push((name.to_string(), value.trim().to_string()));
                    }
                }
            }
        }
        self.inner.set_cookies(&mut heads.into_iter(), url);
    }

    fn cookies(&self, url: &reqwest::Url) -> Option<HeaderValue> {
        self.inner.cookies(url)
    }
}
