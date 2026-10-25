//! WebVPN 会话层：CAS TGT 静默登录深澜网关 + 会话探活 + 业务 HTTP 句柄。
//!
//! 协议事实全部来自 `docs/cas-recon/REPORT.md` 三·二节（webvpn.js 实测，2026-09-17）：
//! 1. GET 网关 `/` → 302 `/login` + Set-Cookie [`COOKIE_TICKET`]（初始值，回跳必带）；
//! 2. `CasClient::sso_ticket(tgt, WEBVPN_SERVICE)` 用已有 TGT 换 WebVPN service 的 ST
//!    （ST 一次性且绑定 service，[`login`] 每次现换，不做缓存）；
//! 3. GET `{WEBVPN_SERVICE}&ticket=<ST>` 手动跟随 302 链：
//!    `wengine-vpn-token-login?token=<一次性 token>` → 200，登录态写入
//!    [`COOKIE_TICKET`]（同名覆盖初始值）+ show_vpn / show_fast / heartbeat / show_faq；
//! 4. [`WebVpnSession::is_alive`]：GET 网关 `/`，未登录 302 → `/login`（REPORT 步骤 4：
//!    已登录 302 到门户的 WebVPN 代理页，路径为 `/https/<hex>/…` 形态，不含 `/login`）。
//!
//! # Cookie 持久化取舍（任务批 3 第 5 点）
//!
//! 选「**不落盘、内存持有，失效重登**」，不扩展 [`campus_auth::jar::RecordingJar`]
//! 的多域恢复。理由：
//! - WebVPN 登录态 cookie 由网关一次性 token 换发，持久化恢复价值低；而 TGT 由调用方
//!   持久化（M1 已有 DPAPI 方案），静默重登 = 一次换票 + 一次 302 链，**无验证码、
//!   无用户交互**，成本低；
//! - 多域恢复须把 RecordingJar 的 (名, 值) 快照模型扩成 (名, 值, 域) 三元组，牵动 M1
//!   门户 session.json 格式与 `restore` 的单域回填语义（jar.rs:15 `RESTORE_URL`），
//!   破坏面大于收益。
//! 本会话仍复用 RecordingJar 作 cookie 存储与 (名, 值) 记录（[`WebVpnSession::jar`]），
//! 不改变其现有 recording/回填语义；会话失效由 [`WebVpnSession::is_alive`] 或业务响应
//! 暴露，上层凭 TGT 重新 [`WebVpnSession::login`]（静默重登）。

use super::wrap::{wrap_url, GATEWAY};
use super::WebVpnError;
use super::cas::{CasClient, WEBVPN_SERVICE};
use super::jar::RecordingJar;
use reqwest::header::LOCATION;
use std::sync::Arc;
use url::form_urlencoded;

/// 深澜网关 ticket cookie 名（初始值与登录态同名，REPORT 三·二步骤 1/3 实测）。
pub const COOKIE_TICKET: &str = "wengine_vpn_ticketwebvpn_cwxu_edu_cn";

/// 302 链手动跟随上限（webvpn.js 用 6 跳，这里留余量到 10，与 cas.rs sso_follow 一致）。
const MAX_HOPS: usize = 10;

/// 真实 Chrome UA（与 cas.rs 同款；深澜按 UA 出响应，不额外模拟）。
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// WebVPN 会话：reqwest::Client + 记录型 Cookie Jar。
///
/// Clone 廉价（reqwest::Client 与 Arc<RecordingJar> 均为引用计数），clone 与原实例
/// 共享同一 cookie jar。
#[derive(Clone)]
pub struct WebVpnSession {
    jar: Arc<RecordingJar>,
    /// 自动跟随重定向（业务请求句柄，[`WebVpnSession::wrapped_client`]）。
    http: reqwest::Client,
    /// 手动跟随重定向（登录 302 链与探活用；链中存在 body 中断的跳，默认策略会整链失败）。
    http_manual: reqwest::Client,
}

impl WebVpnSession {
    /// 全新空会话（无 cookie）。
    pub fn new() -> Result<Self, WebVpnError> {
        let jar = Arc::new(RecordingJar::new());
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .cookie_provider(jar.clone())
            .build()?;
        // 手动跟随用：深澜 302 链存在响应 body 中断的跳（hyper IncompleteMessage，
        // cas.rs sso_follow 同款教训），手动跟随只要 Location 与 Set-Cookie 即可继续。
        let http_manual = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .cookie_provider(jar.clone())
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            jar,
            http,
            http_manual,
        })
    }

    /// CAS TGT 静默登录 WebVPN 网关（REPORT 三·二步骤 1→2→3）。
    ///
    /// - `cas`：CAS 客户端（仅用其 `sso_ticket` 换票；与它共享 jar 与否无所谓，
    ///   CAS 服务端不种登录 cookie，两 jar 独立不影响流程）；
    /// - `tgt`：调用方持有的 CAS TGT（`CasLoginOk.tgt`，失效时换票报
    ///   [`WebVpnError::Sso`]，上层引导重新 CAS 登录）。
    ///
    /// 成功的最终判定交 [`WebVpnSession::is_alive`]：深澜登录态与初始 cookie 同名
    /// （[`COOKIE_TICKET`]），jar 层无法区分「初始值」与「登录态」，故本方法只保证
    /// 302 链走完、cookie 已入 jar。
    pub async fn login(cas: &CasClient, tgt: &str) -> Result<Self, WebVpnError> {
        let session = Self::new()?;
        // 1. 初始访问：种 wengine_vpn_ticket（302 /login；cookie 在响应头，body 不关心）
        let resp = session.http_manual.get(format!("{}/", super::wrap::GATEWAY)).send().await?;
        // 尽力读 body：某些跳 body 会中断（不影响 Set-Cookie 已入 jar）
        let _ = resp.bytes().await;
        // 2. TGT 换 WebVPN service 的 ST（ST- 前缀校验在 campus_auth::sso_ticket 内）
        let st = cas.sso_ticket(tgt, WEBVPN_SERVICE).await?;
        // 3. ticket 回跳，手动跟随 302 链建立会话（链全在网关域，wrap 幂等无害）
        let url = login_redirect_url(WEBVPN_SERVICE, &st);
        session.follow_wrap(&url).await?;
        Ok(session)
    }

    /// 会话探活：GET 网关 `/` 未被弹回 `/login`（纯判定 [`is_login_redirect`]）。
    ///
    /// - 302 且 Location 含 `/login` → 失效（未登录）；
    /// - 302 到 WebVPN 代理页（`/https/<hex>/…`）或 200 → 有效（REPORT 步骤 4）；
    /// - 网络错误 / 302 无 Location → 按 false 处理（宁可触发一次多余重登）。
    pub async fn is_alive(&self) -> bool {
        let Ok(resp) = self.http_manual.get(format!("{}/", super::wrap::GATEWAY)).send().await
        else {
            return false;
        };
        let status = resp.status().as_u16();
        let loc = resp
            .headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let _ = resp.bytes().await; // 归还连接，body 不参与判定
        !is_login_redirect(status, loc.as_deref())
    }

    /// 业务 HTTP 句柄（自动跟随重定向，与登录共用同一 cookie jar）：
    /// 批 4 路由接入用 `wrap_url` 包装目标 URL 后经本 client 请求。
    pub fn wrapped_client(&self) -> &reqwest::Client {
        &self.http
    }

    /// 业务 HTTP 句柄（**手动**跟随重定向，与登录共用同一 cookie jar）：
    /// 供按 302 Location 自行判定的协议使用（自助服务登录 verify 成功/失败、
    /// 会话失效均以 Location 区分），禁自动跟随才拿得到中间跳的 Location。
    pub fn wrapped_client_manual(&self) -> &reqwest::Client {
        &self.http_manual
    }

    /// 会话 Jar 句柄（调试 / 日志用：snapshot 只含 (名, 值)，不落盘）。
    pub fn jar(&self) -> Arc<RecordingJar> {
        self.jar.clone()
    }

    /// 手动跟随 302 链到落点，**每一跳 URL 先经 `transform`**（M4 路由接入用）。
    ///
    /// - 相对 Location 先 `join` 成绝对 URL 再 transform（`wrap_url` 需要绝对 URL）；
    /// - transform 预期幂等（如 [`wrap_url`]：已包装原样、内网明文包装）；Location 可能
    ///   已是 webvpn 包装形态或内网明文，先判后包的幂等性由 transform 自己保证；
    /// - 其余容忍语义（body 中断、MAX_HOPS 上限）与既有实现一致。
    ///
    /// 私有：公共入口只暴露 [`WebVpnSession::follow_wrap`]（固定 [`wrap_url`] 包装器）。
    async fn follow(
        &self,
        start: &str,
        transform: impl Fn(&str) -> String,
    ) -> Result<reqwest::Url, WebVpnError> {
        let mut url = reqwest::Url::parse(&transform(start))?;
        for _ in 0..MAX_HOPS {
            let resp = self.http_manual.get(url.clone()).send().await?;
            let status = resp.status();
            let loc = resp
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            // 尽力读 body：某些跳 body 会中断（不影响 Set-Cookie 已入 jar 与下一跳）
            let _ = resp.bytes().await;
            if status.is_redirection() {
                if let Some(loc) = loc {
                    let joined = url
                        .join(&loc)
                        .map_err(WebVpnError::InvalidUrl)?;
                    url = reqwest::Url::parse(&transform(joined.as_str()))
                        .map_err(WebVpnError::InvalidUrl)?;
                    continue;
                }
            }
            break;
        }
        Ok(url)
    }

    /// 手动跟随 302 链到落点，每一跳 URL 先经 [`wrap_url`]（幂等包装；包装失败原样直连）。
    ///
    /// M4 SSO 桥 WebVPN 链路用：入口与每跳 Location 可能是内网明文（包装）或已包装
    /// 形态（原样），落点 URL 的 query 原样保留（token 提取方不受包装影响）。
    pub async fn follow_wrap(&self, start: &str) -> Result<reqwest::Url, WebVpnError> {
        self.follow(start, |u| wrap_url(u, GATEWAY).unwrap_or_else(|_| u.to_string()))
            .await
    }
}

/// ticket 回跳 URL 构造（纯函数供离线单测）：`{service}&ticket=<ST>`。
///
/// WEBVPN_SERVICE 自带 query（`?cas_login=true`），故用 `&` 追加；ticket 经
/// `url::form_urlencoded` 编码（ST 实测为 `ST-<可见字符>`，编码是防御性处理）。
pub fn login_redirect_url(service: &str, ticket: &str) -> String {
    let pair = form_urlencoded::Serializer::new(String::new())
        .append_pair("ticket", ticket)
        .finish();
    format!("{service}&{pair}")
}

/// is_alive 判定核心（纯函数供离线单测）：3xx 且 Location 落 `/login` = 未登录。
///
/// 匹配的是 Location 中的 `/login` 子串（含 `/login?cas_login=true` 形态）；已登录
/// 的落点是 WebVPN 代理路径 `/https/<hex>/…`（hex 字符集不含 `/`，无误匹配面）。
pub fn is_login_redirect(status: u16, location: Option<&str>) -> bool {
    (300..400).contains(&status) && location.is_some_and(|l| l.contains("/login"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_cookie_name_matches_recon() {
        // REPORT 三·二步骤 1/3 实测 cookie 名；防手滑改名
        assert_eq!(COOKIE_TICKET, "wengine_vpn_ticketwebvpn_cwxu_edu_cn");
    }

    // ---------- login_redirect_url ----------

    #[test]
    fn redirect_url_appends_ticket_with_ampersand() {
        assert_eq!(
            login_redirect_url(
                "https://webvpn.cwxu.edu.cn/login?cas_login=true",
                "ST-abc123"
            ),
            "https://webvpn.cwxu.edu.cn/login?cas_login=true&ticket=ST-abc123"
        );
    }

    #[test]
    fn redirect_url_encodes_special_chars() {
        assert_eq!(
            login_redirect_url("https://gw.example/login?x=1", "ST a+b"),
            "https://gw.example/login?x=1&ticket=ST+a%2Bb"
        );
    }

    // ---------- is_login_redirect ----------

    #[test]
    fn alive_when_probe_hits_proxy_page() {
        // REPORT 步骤 4：已登录 302 到门户的 WebVPN 代理页（非 /login）
        assert!(!is_login_redirect(
            302,
            Some("https://webvpn.cwxu.edu.cn/https/77726476706e69737468656265737421fdee0f9f30287d1e7b0c9ce29b5b/")
        ));
        assert!(!is_login_redirect(200, None));
    }

    #[test]
    fn dead_when_probe_bounces_to_login() {
        assert!(is_login_redirect(302, Some("/login")));
        assert!(is_login_redirect(
            302,
            Some("https://webvpn.cwxu.edu.cn/login?cas_login=true")
        ));
        assert!(is_login_redirect(301, Some("/login?next=%2Fx")));
    }

    #[test]
    fn dead_on_edge_cases_defaults_to_alive_redirect() {
        // 302 无 Location / 5xx / 4xx：无法判定为登录跳转，按未弹回处理
        // （失效的兜底在业务请求侧，宁可触发一次多余重登）
        assert!(!is_login_redirect(302, None));
        assert!(!is_login_redirect(503, Some("/login")));
        assert!(!is_login_redirect(404, Some("/login")));
    }

    // ---------- follow（逐跳 transform）----------

    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// 依序回放 `responses` 的桩服务：每个连接消费一条响应，收到的请求首行逐条记录。
    /// （campus-synjones client.rs 桩同款思路：Content-Length 读满 + Connection: close。）
    fn stub_sequence(responses: &[&str]) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口失败");
        let addr = listener.local_addr().expect("取本地地址失败");
        let script: Vec<String> = responses.iter().map(|s| s.to_string()).collect();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_clone = seen.clone();
        std::thread::spawn(move || {
            for resp in &script {
                let Ok((mut sock, _)) = listener.accept() else { return };
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf); // 请求行 enough（GET 无 body）
                let first = String::from_utf8_lossy(&buf)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                seen_clone.lock().unwrap().push(first);
                sock.write_all(resp.as_bytes()).ok();
                sock.flush().ok();
            }
        });
        (format!("http://{addr}"), seen)
    }

    fn http_302(location: &str) -> String {
        format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    }

    fn http_200() -> String {
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
    }

    /// 两跳链：相对 Location join 后逐跳过 transform；transform 每跳各收到一次绝对 URL。
    #[tokio::test]
    async fn follow_applies_transform_per_hop_with_relative_locations() {
        let (base, seen) = stub_sequence(&[&http_302("/hop2?a=1"), &http_200()]);
        let s = WebVpnSession::new().expect("会话构造失败");

        let transformed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let t_clone = transformed.clone();
        let landing = s
            .follow(
                &format!("{base}/start"),
                move |u| {
                    // 可观测的幂等变换：记下每次调用，并给 path 前缀加标记
                    let marked = u.replace("/start", "/T-start").replace("/hop2", "/T-hop2");
                    t_clone.lock().unwrap().push(marked.clone());
                    marked
                },
            )
            .await
            .expect("跟随应成功");

        assert_eq!(landing.path(), "/T-hop2", "落点应是第二跳（transform 已生效）");
        assert_eq!(landing.query(), Some("a=1"), "query 原样保留");
        let calls = transformed.lock().unwrap();
        assert_eq!(calls.len(), 2, "入口 + 每跳 Location 各 transform 一次：{calls:?}");
        assert!(calls[0].contains("/T-start"), "入口被 transform：{calls:?}");
        assert!(calls[1].ends_with("/T-hop2?a=1"), "相对 Location 先 join 再 transform：{calls:?}");
        // 请求侧：两跳都真实发出（相对 Location 由 join 解析成绝对 URL）
        let sent = seen.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert!(sent[0].starts_with("GET /T-start"), "首跳按 transform 后的 URL 请求：{sent:?}");
        assert!(sent[1].starts_with("GET /T-hop2?a=1"), "次跳按 transform 后的 URL 请求：{sent:?}");
    }

    /// 非 3xx 即停：落点是首个非跳转响应的 URL。
    #[tokio::test]
    async fn follow_stops_at_first_non_redirect() {
        let (base, seen) = stub_sequence(&[&http_200()]);
        let s = WebVpnSession::new().expect("会话构造失败");
        let landing = s.follow(&format!("{base}/only"), str::to_string).await.expect("应成功");
        assert_eq!(landing.path(), "/only");
        assert_eq!(seen.lock().unwrap().len(), 1, "200 不再跟随");
    }
}
