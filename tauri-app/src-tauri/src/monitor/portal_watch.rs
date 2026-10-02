//! Portal 常驻监测：定时从各在网网卡绑源采样 portal 可达性（ICMP + 页面）、
//! chkstatus 会话状态与运营商服务列表，JSONL 落盘日志目录（portal-watch-*.jsonl，
//! 随日志保留天数清理），状态变化写事件日志。观测目标：portal 服务端配置变更
//! （运营商下架/换后缀）、单线路 portal 断联、会话被踢与重连——尤其「无锡学院
//! 线路何时也断」。只读探测，绝不携带凭据请求登录端点。
//!
//! 采样口径与判定/协议路径同源：ICMP 复用 check_gateway_reachable_from（绑源、
//! 须在阻塞线程，见 network/subnet.rs 的线程约束）；页面/会话复用
//! create_safe_http_client（local_address 绑源，客户端超时与 auth::portal 对齐
//! 以共享连接池）+ read_bounded_body（1MB 限长 + charset 解码）+ parse_chkstatus。
//! oltime 停摆不发事件（逐轮 JSONL 可见，避免常驻误报），分析侧自行判读。

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::infra::state::AppState;

/// 采样周期（秒）：5 分钟粒度对「线路何时断/配置何时变」的观测足够，
/// 每卡每轮最多 2 个 HTTP 请求 + 1 次 ICMP，对 portal 无压力。
const SAMPLE_INTERVAL_SECS: u64 = 300;
/// HTTP 客户端创建超时（秒）：与 auth::portal 的 CLIENT_TIMEOUT 对齐，客户端池
/// 按 (local_addr, tls, timeout) 入键，对齐即共享池条目，不翻倍。
const CLIENT_TIMEOUT_SECS: u64 = 8;
/// 单次 HTTP 请求超时（秒）：与 auth::portal 的 REQUEST_TIMEOUT 对齐。
/// 监测是旁路观测，超时本身即数据（记 http=false）。
const REQUEST_TIMEOUT_SECS: u64 = 3;
/// JSONL 文件名前缀（portal-watch-YYYY-MM-DD.jsonl，按天滚动）
const WATCH_FILE_PREFIX: &str = "portal-watch-";

/// 单卡单轮采样。字段即 JSONL 行字段。
#[derive(Debug, Clone, PartialEq)]
struct Sample {
    adapter: String,
    ip: String,
    /// portal 主机绑源 ICMP 可达
    icmp: bool,
    /// portal 页面获取成功（2xx 且非空 body）
    http: bool,
    /// chkstatus 会话在线（result==1）
    online: bool,
    /// chkstatus 在线账号（学号+运营商后缀；离线/解析失败为空串）
    uid: String,
    /// 会话在线时长（秒），chkstatus 缺失该字段时为 None
    oltime: Option<i64>,
    /// 页面运营商服务列表签名（如 "校园用户|校园电信@dx|校园联通@lt|校园其他"）
    carriers: String,
}

pub fn start_portal_watch(app_handle: &AppHandle) -> Result<(), String> {
    let app_h = app_handle.clone();
    app_handle
        .state::<AppState>()
        .task_manager
        .spawn("portal_watch", move |cancel_token| {
            async move {
                // 首拍立即采样建立基线；单轮超周期时不补发，保持固定节奏
                let mut interval_timer =
                    tokio::time::interval(Duration::from_secs(SAMPLE_INTERVAL_SECS));
                interval_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                let mut last: HashMap<String, Sample> = HashMap::new();
                loop {
                    tokio::select! {
                        _ = interval_timer.tick() => {}
                        _ = cancel_token.cancelled() => break,
                    }
                    if app_h
                        .state::<AppState>()
                        .exit
                        .is_quitting
                        .load(Ordering::Acquire)
                    {
                        break;
                    }
                    // 整轮在阻塞线程执行：绑源 ICMP 探测与同步文件 I/O 都不可占 async 线程
                    let app = app_h.clone();
                    match tauri::async_runtime::spawn_blocking(move || run_round(&app)).await {
                        Ok(samples) => apply_events(&mut last, &samples),
                        Err(e) => crate::log_warn!("portal", "portal 监测轮执行失败: {e}"),
                    }
                }
            }
        })
}

/// 一轮采样：所有有 IP 的在网卡逐卡绑源探测。portal 主机解析不出私网 IPv4 时
/// 跳过整轮（域名/公网 portal 不适用绑源探测，见 portal_probe_host）。
fn run_round(app: &AppHandle) -> Vec<Sample> {
    let config = app.state::<AppState>().config.load_full();
    let portal_url = if config.portal_url.is_empty() {
        crate::config::model::default_portal_url()
    } else {
        config.portal_url.clone()
    };
    let portal_host = crate::monitor::outbound_switch::portal_probe_host(&portal_url);
    if portal_host.is_empty() {
        return Vec::new();
    }
    let adapters = crate::network::get_adapters_cached().unwrap_or_default();
    let samples: Vec<Sample> = adapters
        .iter()
        .filter(|a| !a.ip.is_empty())
        .map(|a| sample_adapter(&portal_url, &portal_host, a))
        .collect();
    if !samples.is_empty() {
        write_jsonl(&crate::infra::logger::get_log_dir(app), &samples);
    }
    samples
}

/// 逐卡采样：ICMP（阻塞线程直接调）+ 页面 + chkstatus（block_on 包住 send，
/// body 复用限长解码读取）。任何一步失败记为该步的空值/false，不中断其余
/// 步骤——监测轮的意义就是记录哪一步先断。
fn sample_adapter(portal_url: &str, portal_host: &str, adapter: &crate::network::Adapter) -> Sample {
    let local_addr = adapter.ip.parse::<std::net::IpAddr>().ok();
    let icmp = crate::network::check_gateway_reachable_from(portal_host, Some(&adapter.ip));

    let client = match crate::network::client::create_safe_http_client(
        Duration::from_secs(CLIENT_TIMEOUT_SECS),
        local_addr,
    ) {
        Ok(c) => c,
        Err(e) => {
            crate::log_warn!("portal", "{} 监测客户端创建失败: {e}", adapter.name);
            return Sample {
                adapter: adapter.name.clone(),
                ip: adapter.ip.clone(),
                icmp,
                http: false,
                online: false,
                uid: String::new(),
                oltime: None,
                carriers: String::new(),
            };
        }
    };

    let base = portal_url.trim_end_matches('/');
    let page = fetch_text(
        &client,
        &format!("{base}/"),
        "portal监测页面",
        &adapter.name,
    );
    let chk_body = fetch_text(
        &client,
        &format!("{base}/drcom/chkstatus?callback=dr1003"),
        "portal监测chkstatus",
        &adapter.name,
    );
    let (online, uid, oltime) = match crate::config::night_switch::parse_chkstatus(&chk_body) {
        Some(info) => (info.online, info.uid, info.oltime),
        None => (false, String::new(), None),
    };

    Sample {
        adapter: adapter.name.clone(),
        ip: adapter.ip.clone(),
        icmp,
        http: !page.is_empty(),
        online,
        uid,
        oltime,
        carriers: carriers_signature(&page),
    }
}

/// 绑源 GET：成功（2xx）返回限长解码后的 body，其余（网络错误/非 2xx/空 body）
/// 返回空串。失败只记 debug——监测轮每 5 分钟一次，warn 会刷屏。
fn fetch_text(client: &reqwest::Client, url: &str, label: &str, adapter_name: &str) -> String {
    let resp = tauri::async_runtime::block_on(
        client
            .get(url)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send(),
    );
    match resp {
        Ok(r) if r.status().is_success() => {
            let body = crate::auth::protocol::read_bounded_body(r, label);
            if body.is_empty() {
                crate::log_debug!("portal", "{} {label} 读取为空", adapter_name);
            }
            body
        }
        Ok(r) => {
            crate::log_debug!("portal", "{} {label} 状态异常: {}", adapter_name, r.status());
            String::new()
        }
        Err(e) => {
            crate::log_debug!("portal", "{} {label} 请求失败: {e}", adapter_name);
            String::new()
        }
    }
}

/// 从页面 HTML 提取运营商服务列表签名：`{"id":"1","name":"校园用户","suffix":""}`
/// 序列 → `校园用户|校园电信@dx|…`（后缀直接拼接，空后缀即裸名）。Dr.COM 把
/// 服务类型列表内嵌在页面 JS 里，服务端配置变更（如 @cmcc 下架）立即反映为
/// 签名变化；解析不出返回空串。页面其他 JSON 对象若同样带 id/name 字段会被
/// 一并计入——对本部署的 portal 页实测片段即干净的 4 项运营商列表。
fn carriers_signature(html: &str) -> String {
    html.split("{\"id\"")
        .skip(1)
        .filter_map(|chunk| {
            let name = json_string_field(chunk, "name")?;
            let suffix = json_string_field(chunk, "suffix").unwrap_or_default();
            Some(format!("{name}{suffix}"))
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// 在 JSON 对象片段里取 `"key":"value"` 的 value（页面内嵌 JS 的无转义场景）。
fn json_string_field(chunk: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let pos = chunk.find(&needle)? + needle.len();
    let rest = &chunk[pos..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// 按天落盘 JSONL（append）。文件随 get_log_retention_days 清理过期（0=永久保留，
/// 与 cleanup_old_logs_by_time 同口径）。轮内一次性追加，同步 I/O 已在阻塞线程。
fn write_jsonl(dir: &Path, samples: &[Sample]) {
    let day = chrono::Local::now().format("%Y-%m-%d");
    let path: PathBuf = dir.join(format!("{WATCH_FILE_PREFIX}{day}.jsonl"));
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let mut buf = String::new();
    for s in samples {
        let line = serde_json::json!({
            "ts": ts.to_string(),
            "adapter": s.adapter,
            "ip": s.ip,
            "icmp": s.icmp,
            "http": s.http,
            "online": s.online,
            "uid": s.uid,
            "oltime": s.oltime,
            "carriers": s.carriers,
        });
        buf.push_str(&line.to_string());
        buf.push('\n');
    }
    if std::fs::create_dir_all(dir).is_ok() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(buf.as_bytes());
        }
    }
    cleanup_watch_files(dir);
}

/// 清理过期监测文件：只动本模块前缀的 .jsonl，按修改时间对照日志保留天数。
fn cleanup_watch_files(dir: &Path) {
    let days = crate::infra::logger::get_log_retention_days();
    if days == 0 {
        return; // 永久保留
    }
    let cutoff = match std::time::SystemTime::now()
        .checked_sub(Duration::from_secs(days as u64 * 86400))
    {
        Some(c) => c,
        None => return,
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(WATCH_FILE_PREFIX) || !name.ends_with(".jsonl") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if modified < cutoff {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

/// 与上一轮逐卡对比，状态变化写事件日志（首样只建基线不发事件）。
/// 卡片消失（拔出/禁用/IP 变更）时清出基线，避免陈旧对比。
fn apply_events(last: &mut HashMap<String, Sample>, samples: &[Sample]) {
    for cur in samples {
        let prev = last.insert(cur.adapter.clone(), cur.clone());
        for ev in diff_events(prev.as_ref(), cur) {
            crate::log_info!("portal", "[监测] {}: {ev}", cur.adapter);
        }
    }
    last.retain(|k, _| samples.iter().any(|s| s.adapter == *k));
}

/// 单卡前后两轮的状态差（纯函数便于单测）。
fn diff_events(prev: Option<&Sample>, cur: &Sample) -> Vec<String> {
    let Some(p) = prev else {
        return Vec::new();
    };
    let mut evs = Vec::new();
    if p.http && !cur.http {
        evs.push(format!("portal 页面不可达（http 断，icmp={}）", cur.icmp));
    } else if !p.http && cur.http {
        evs.push("portal 页面恢复可达".to_string());
    }
    if p.online && !cur.online {
        evs.push(format!("会话掉线（uid 空，oltime={:?}）", cur.oltime));
    } else if !p.online && cur.online {
        evs.push(format!("检测到在线会话 {}", cur.uid));
    }
    if let (Some(pv), Some(cv)) = (p.oltime, cur.oltime) {
        if cv < pv {
            evs.push(format!("会话重连（oltime {pv} → {cv}）"));
        }
    }
    // 空签名 = 该轮无数据（页面获取失败），不是「无运营商」，不参与变更比对
    if !p.carriers.is_empty() && !cur.carriers.is_empty() && p.carriers != cur.carriers {
        evs.push(format!("portal 运营商配置变更: {} → {}", p.carriers, cur.carriers));
    }
    evs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-03 portal 首页实测片段（GBK 解码后）
    const PAGE_CARRIERS: &str = r#"data:[{"id":"1","name":"校园用户","suffix":""},{"id":"2","name":"校园电信","suffix":"@dx"},{"id":"3","name":"校园联通","suffix":"@lt"},{"id":"4","name":"校园其他","suffix":""}]"#;

    #[test]
    fn carriers_signature_实测运营商列表() {
        assert_eq!(
            carriers_signature(PAGE_CARRIERS),
            "校园用户|校园电信@dx|校园联通@lt|校园其他"
        );
    }

    #[test]
    fn carriers_signature_无运营商数据为空() {
        assert_eq!(carriers_signature("<html>uid='24385214@cmcc'</html>"), "");
        assert_eq!(carriers_signature(""), "");
    }

    #[test]
    fn carriers_signature_服务项增减反映为签名变化() {
        // @cmcc 下架场景：从含移动项的旧列表变为实测新列表
        let old = r#"[{"id":"5","name":"校园移动","suffix":"@cmcc"},{"id":"1","name":"校园用户","suffix":""}]"#;
        assert_ne!(carriers_signature(old), carriers_signature(PAGE_CARRIERS));
    }

    fn sample(adapter: &str, http: bool, online: bool, oltime: Option<i64>, carriers: &str) -> Sample {
        Sample {
            adapter: adapter.to_string(),
            ip: "10.0.0.1".to_string(),
            icmp: http,
            http,
            online,
            uid: if online { "24385214@cmcc".to_string() } else { String::new() },
            oltime,
            carriers: carriers.to_string(),
        }
    }

    #[test]
    fn diff_events_首样只建基线不发事件() {
        let cur = sample("以太网", true, true, Some(100), "校园用户");
        assert!(diff_events(None, &cur).is_empty());
    }

    #[test]
    fn diff_events_页面断联与恢复() {
        let prev = sample("以太网", true, true, Some(100), "校园用户");
        let down = sample("以太网", false, false, None, "");
        let evs = diff_events(Some(&prev), &down);
        assert_eq!(evs.len(), 2, "http 断 + 会话掉线两条: {evs:?}");
        assert!(evs[0].contains("portal 页面不可达"), "{evs:?}");

        let back = sample("以太网", true, true, Some(160), "校园用户");
        let evs = diff_events(Some(&down), &back);
        assert!(evs.iter().any(|e| e.contains("恢复可达")), "{evs:?}");
        assert!(evs.iter().any(|e| e.contains("检测到在线会话")), "{evs:?}");
    }

    #[test]
    fn diff_events_会话重连与配置变更() {
        let prev = sample("以太网", true, true, Some(186_000), "校园用户|校园电信@dx");
        // oltime 回退 → 重连；carriers 变化 → 配置变更
        let cur = sample("以太网", true, true, Some(300), "校园用户");
        let evs = diff_events(Some(&prev), &cur);
        assert!(evs.iter().any(|e| e.contains("会话重连") && e.contains("186000")), "{evs:?}");
        assert!(evs.iter().any(|e| e.contains("运营商配置变更") && e.contains("校园电信@dx")), "{evs:?}");
    }

    #[test]
    fn diff_events_无变化不发事件() {
        let prev = sample("以太网", true, true, Some(100), "校园用户");
        let cur = sample("以太网", true, true, Some(400), "校园用户");
        assert!(diff_events(Some(&prev), &cur).is_empty());
    }
}
