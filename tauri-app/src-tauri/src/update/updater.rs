use serde::{Deserialize, Serialize};
use tauri::Manager;
use crate::infra::state::AppState;
use crate::infra::events::EventBus;
use crate::infra::notification::emit_notification;
use std::sync::atomic::Ordering;

const GITHUB_REPO: &str = "ikliml666/Wxxy-CampusLogin";
const AUTO_CHECK_INTERVAL_SECS: u64 = 86400;
const VERSION_FILE_URL: &str = "https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json";

/// version.json 镜像源列表（GitHub 原始源失败时按顺序降级）
const VERSION_MIRRORS: &[&str] = &[
    "https://ghfast.top/https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json",
    "https://gh-proxy.com/https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json",
    "https://ghproxy.net/https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json",
];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub has_update: bool,
    pub latest_version: String,
    pub release_notes: String,
    pub assets: Vec<ReleaseAsset>,
    pub sha256_checksum: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
    pub percent: f64,
}

pub fn compare_versions(current: &str, latest: &str) -> bool {
    let parse_version = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .split('.')
            .map(|seg| {
                // 提取段中的数字部分（处理 "2-beta" 等后缀），解析失败按 0
                let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse::<u32>().unwrap_or(0)
            })
            .collect()
    };
    let cur = parse_version(current);
    let lat = parse_version(latest);

    // 全段比较：截断为 3 段时 hotfix（如 2.3.0.1）与 2.3.0 判等，升级提示永不出现
    let len = cur.len().max(lat.len());
    for i in 0..len {
        let c = cur.get(i).copied().unwrap_or(0);
        let l = lat.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

/// 短超时 HTTP 客户端（模块级缓存，首次构建后复用）。
/// 不接入 network/client.rs 的 CLIENT_POOL：池内客户端强制 .no_proxy()，
/// 会改变更新下载对系统代理的语义；此处保持原生代理行为。
/// 更新检查与 SHA256 校验超时同为 10s，reqwest::Client 为可并发复用的句柄，
/// 共享同一实例安全（连接池按 host 复用）。
static SHORT_TIMEOUT_CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> = std::sync::OnceLock::new();

fn build_short_timeout_http_client() -> Result<&'static reqwest::Client, String> {
    // 用 get_or_init 缓存 Result（工具链下 get_or_try_init 未稳定）。
    // Client 构建失败属确定性配置错误，缓存失败结果不影响正确性。
    match SHORT_TIMEOUT_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("创建HTTP客户端失败: {e}"))
    }) {
        Ok(c) => Ok(c),
        Err(e) => Err(e.clone()),
    }
}

/// 所有校验和源均失败时的决策。
///
/// - 所有响应均为 4xx（文件不存在/权限受限）且无传输错误：
///   `allow_skip` 为 true 时降级通过（发布流程未上传 .sha256），否则拒绝安装。
/// - 存在 5xx/传输错误：一律视为系统异常，拒绝安装。
pub fn decide_checksum_missing(all_client_errors: bool, had_transport_error: bool, allow_skip: bool) -> Result<(), String> {
    if all_client_errors && !had_transport_error {
        if allow_skip {
            crate::log_warn!(
                "updater",
                "所有 SHA256 校验源均返回 4xx，视为发布流程未上传 .sha256 文件（用户已显式开启跳过校验），降级通过"
            );
            Ok(())
        } else {
            Err("校验和源全部不可用（4xx），且未开启 skipSha256WhenMissing，拒绝安装未校验的安装包".to_string())
        }
    } else {
        Err("所有校验和源均失败（含 5xx/网络错误），拒绝安装".to_string())
    }
}

/// 从校验源响应文本中提取 64 位小写 SHA256。
/// GitHub API 响应为 JSON：取 assets 中首个 .exe 资产的 digest 字段（"sha256:hex"）；
/// .sha256 文件响应为纯文本：支持 shasum 风格（`<hash>  <file>`，取首 token）
/// 与 BSD 风格（`SHA256 (file) = <hash>`，取等号后 token）。
/// 提取失败返回 None，调用方继续尝试下一校验源。
fn extract_checksum(url: &str, text: &str) -> Option<String> {
    if url.contains("api.github.com") {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let assets = value.get("assets")?.as_array()?;
        for asset in assets {
            let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.ends_with(".exe") {
                // 多架构 release 可能有多个 .exe 资产：无有效 digest 时继续找下一个
                if let Some(digest) = asset.get("digest").and_then(|d| d.as_str()) {
                    if let Some(hex) = digest.strip_prefix("sha256:") {
                        if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                            return Some(hex.to_lowercase());
                        }
                    }
                }
            }
        }
        None
    } else {
        let is_hex64 = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
        let first = text.split_whitespace().next().unwrap_or("");
        if is_hex64(first) {
            return Some(first.to_lowercase());
        }
        // BSD 风格：SHA256 (file) = <hash>
        let after_eq = text.rsplit('=').next().unwrap_or("").trim();
        is_hex64(after_eq).then(|| after_eq.to_lowercase())
    }
}

pub async fn verify_download_sha256(file_path: &str, checksum_urls: &[String], allow_skip_missing: bool) -> Result<bool, String> {
    if checksum_urls.is_empty() {
        return Err("未提供校验和URL".to_string());
    }

    let client = build_short_timeout_http_client()?;

    // 按顺序尝试所有 URL（GitHub API digest + GitHub 原始源 + 镜像源），任一成功即用
    let mut last_err = String::new();
    let mut all_client_errors = true; // 是否所有响应都是 4xx（文件不存在/权限问题）
    let mut had_transport_error = false; // 是否有网络/超时等传输错误
    let expected_hash = {
        let mut hash: Option<String> = None;
        for url in checksum_urls {
            match client.get(url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.text().await {
                        Ok(text) => {
                            if let Some(h) = extract_checksum(url, &text) {
                                hash = Some(h);
                                break;
                            }
                            last_err = format!("校验源响应中未找到有效校验和: {url}");
                        }
                        Err(e) => {
                            last_err = format!("读取校验和内容失败: {e}");
                        }
                    }
                }
                Ok(resp) => {
                    let status = resp.status();
                    last_err = format!("HTTP {status}");
                    if !status.is_client_error() {
                        // 5xx 等服务端错误 → 不再视为"文件不存在"
                        all_client_errors = false;
                    }
                }
                Err(e) => {
                    last_err = format!("获取校验和文件失败: {e}");
                    // 网络/超时等传输错误：4xx-only 条件不成立
                    all_client_errors = false;
                    had_transport_error = true;
                }
            }
        }
        match hash {
            Some(h) => h,
            None => {
                // 所有源都失败：按显式开关决策（默认拒绝，需用户在设置中确认跳过）
                decide_checksum_missing(all_client_errors, had_transport_error, allow_skip_missing)
                    .map_err(|e| format!("{e}（最后错误: {last_err}）"))?;
                return Ok(true);
            }
        }
    };

    // 分块流式读取并计算 SHA256，避免一次性 std::fs::read 大文件（如 50MB+ 安装包）造成内存峰值
    // 仍在 spawn_blocking 内执行，不阻塞 async 线程；64KB buffer 使内存占用恒定
    use sha2::Digest;
    let actual_hash = tokio::task::spawn_blocking({
        let path = file_path.to_string();
        move || -> Result<String, String> {
            use std::io::Read;
            let mut file = std::fs::File::open(&path)
                .map_err(|e| format!("打开下载文件失败: {e}"))?;
            let mut hasher = sha2::Sha256::new();
            let mut buf = [0u8; 65536]; // 64KB buffer
            loop {
                let n = file.read(&mut buf)
                    .map_err(|e| format!("读取下载文件失败: {e}"))?;
                if n == 0 { break; }
                hasher.update(&buf[..n]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        }
    }).await
    .map_err(|e| format!("计算哈希任务失败: {e}"))??;

    Ok(actual_hash == expected_hash)
}

pub fn schedule_update_cleanup() {
    let temp_dir = std::env::temp_dir().join("campus-login-update");
    tauri::async_runtime::spawn(async move {
        // 24h 后清理：原 600s 内用户可能尚未点击安装（UAC 等待/稍后安装），
        // 安装包会被提前删除
        tokio::time::sleep(std::time::Duration::from_secs(24 * 3600)).await;
        let _ = tauri::async_runtime::spawn_blocking(move || {
            std::fs::remove_dir_all(&temp_dir)
        }).await;
        crate::log_debug!("updater", "更新临时目录已清理");
    });
}

pub fn start_update_check_loop(app_handle: &tauri::AppHandle) {
    let app_h = app_handle.clone();
    let task_manager = app_handle.state::<AppState>().task_manager.clone();
    if let Err(e) = task_manager.spawn("update_check_loop", move |cancel_token| async move {
        let state = app_h.state::<AppState>();
        let last_epoch = state.update_stats.last_update_check_epoch_ms.load(Ordering::Acquire);
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let elapsed_secs = if last_epoch == 0 { AUTO_CHECK_INTERVAL_SECS + 1 } else { (now_epoch - last_epoch) / 1000 };

        // 首次检查：若距上次检查超过间隔则立即检查（退出中跳过）
        if elapsed_secs >= AUTO_CHECK_INTERVAL_SECS
            && !cancel_token.is_cancelled()
            && !state.exit.is_quitting.load(Ordering::Acquire)
        {
            do_update_check(&app_h, &state).await;
        }

        // 计算到下次检查的剩余时间，避免频繁重启导致检查被持续推迟
        let remaining_secs = if elapsed_secs < AUTO_CHECK_INTERVAL_SECS {
            AUTO_CHECK_INTERVAL_SECS - elapsed_secs
        } else {
            AUTO_CHECK_INTERVAL_SECS
        };
        // 拆分为 5s 步进循环，同时监听取消令牌，避免 24h sleep 期间无法响应退出
        let mut elapsed = 0u64;
        let step = 5u64;
        while elapsed < remaining_secs {
            let wait = std::cmp::min(step, remaining_secs - elapsed);
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(wait)) => {}
                _ = cancel_token.cancelled() => return,
            }
            elapsed += wait;
            if state.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }
        }

        // 后续固定间隔检查
        loop {
            if cancel_token.is_cancelled() || state.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }
            do_update_check(&app_h, &state).await;
            // 拆分为 5s 步进等待，同时监听取消令牌
            let mut waited = 0u64;
            while waited < AUTO_CHECK_INTERVAL_SECS {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                    _ = cancel_token.cancelled() => return,
                }
                if state.exit.is_quitting.load(Ordering::Acquire) {
                    return;
                }
                waited += 5;
            }
        }
    }) {
        crate::log_warn!("updater", "注册 update_check_loop 跟踪任务失败: {}", e);
    }
}

/// 执行一次更新检查并发送通知
async fn do_update_check(app_h: &tauri::AppHandle, state: &AppState) {
    let mirror_first = state.config.load().update_source != "github";
    match check_update_inner(mirror_first).await {
        Ok(info) => {
            if let Err(e) = EventBus::new(app_h).emit_update_available(
                info.has_update,
                &info.latest_version,
                &info.release_notes,
            ) {
                crate::log_warn!("updater", "发送更新通知失败: {}", e);
            }
            if info.has_update && state.update_stats.update_notified.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                emit_notification(app_h, "发现新版本", &format!("新版本 v{} 可用，请在关于页面查看", info.latest_version));
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            state.update_stats.last_update_check_epoch_ms.store(now, Ordering::Release);
        }
        Err(e) => {
            crate::log_warn!("updater", "更新检查失败: {}", e);
        }
    }
}

/// 检查更新:按用户渠道设置(update_source="github" 官方优先/其余镜像优先)排序源,
/// 未选中一侧保留为降级兜底
pub async fn fetch_latest_release(mirror_first: bool) -> Result<(bool, String, String, Option<String>), String> {
    let ordered: Vec<&str> = if mirror_first {
        vec![VERSION_MIRRORS[0], VERSION_MIRRORS[1], VERSION_MIRRORS[2], VERSION_FILE_URL]
    } else {
        vec![VERSION_FILE_URL, VERSION_MIRRORS[0], VERSION_MIRRORS[1], VERSION_MIRRORS[2]]
    };
    let mut primary_err = String::from("未知错误");
    for (i, url) in ordered.iter().enumerate() {
        match fetch_version_from_url(url).await {
            Ok(result) => {
                if i > 0 {
                    crate::log_info!("updater", "更新源 {} 检查成功(降级生效)", url);
                }
                return Ok(result);
            }
            Err(e) => {
                if i == 0 {
                    crate::log_info!("updater", "首选更新源检查失败: {}，按序降级...", e);
                    primary_err = e;
                } else {
                    crate::log_debug!("updater", "更新源 {} 失败: {}", url, e);
                }
            }
        }
    }
    Err(format!("所有更新源均失败（首选: {primary_err}）"))
}

async fn fetch_version_from_url(url: &str) -> Result<(bool, String, String, Option<String>), String> {
    let client = build_short_timeout_http_client()?;

    let resp = client
        .get(url)
        .header("User-Agent", "CampusLogin-UpdateChecker")
        .send()
        .await
        .map_err(|e| format!("获取version.json失败: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("version.json不可用: HTTP {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析version.json失败: {e}"))?;

    let latest_tag = data["version"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    if latest_tag.is_empty() {
        return Err("version.json中缺少版本号".to_string());
    }

    let current = env!("APP_VERSION");
    let has_update = compare_versions(current, &latest_tag);

    // notes 字段可选：发布流程在 version.json 中随版本号一起维护更新日志
    let notes = data["notes"].as_str().unwrap_or("").to_string();
    // asset 字段可选：安装包文件名由发布流程在 version.json 中声明，
    // 未声明时回退历史命名约定 Wxxy-CampusLogin_{tag}_x64-setup.exe
    let asset = data["asset"].as_str().map(|s| s.to_string());

    Ok((has_update, latest_tag, notes, asset))
}



pub async fn check_update_inner(mirror_first: bool) -> Result<UpdateInfo, String> {
    let (has_update, latest_tag, notes, asset) = fetch_latest_release(mirror_first).await?;

    let exe_name = asset.unwrap_or_else(|| format!("Wxxy-CampusLogin_{latest_tag}_x64-setup.exe"));
    let github_exe_url = format!(
        "https://github.com/{GITHUB_REPO}/releases/download/v{latest_tag}/{exe_name}"
    );

    // version.json 先行而 Release 尚未发布时（发布时序问题），下载必然 404。
    // 探测官方源资产：确认不存在则本轮不提示更新，避免用户收到通知后下载必败；
    // 探测本身网络失败时保守维持 has_update，交由下载阶段报错并允许走镜像重试。
    let mut has_update = has_update;
    if has_update {
        let client = build_short_timeout_http_client()?;
        match client
            .head(&github_exe_url)
            .header("User-Agent", "CampusLogin-UpdateChecker")
            .send()
            .await
        {
            Ok(resp) if resp.status().as_u16() == 404 => {
                crate::log_warn!("updater", "version.json 声明 v{latest_tag} 但 Release 资产不存在，本轮不提示更新");
                has_update = false;
            }
            Ok(_) => {}
            Err(e) => {
                crate::log_debug!("updater", "更新资产探测失败（保守视为存在）: {e}");
            }
        }
    }

    // SHA256 校验源优先级：GitHub API 官方 digest（服务端计算，发布者漏传 .sha256
    // 文件时兜底）→ GitHub 原始 .sha256 → 镜像源，任一成功即用
    let sha256_urls: Vec<String> = {
        let mut urls = vec![format!("https://api.github.com/repos/{GITHUB_REPO}/releases/tags/v{latest_tag}")];
        urls.push(format!("{github_exe_url}.sha256"));
        for mirror_prefix in &[
            "https://ghfast.top/",
            "https://gh-proxy.com/",
            "https://ghproxy.net/",
        ] {
            urls.push(format!("{mirror_prefix}{github_exe_url}.sha256"));
        }
        urls
    };

    Ok(UpdateInfo {
        has_update,
        latest_version: latest_tag,
        release_notes: notes,
        assets: vec![
            ReleaseAsset {
                name: exe_name.clone(),
                url: github_exe_url,
                size: 0,
            },
            ReleaseAsset {
                name: format!("{exe_name}.sha256"),
                url: sha256_urls.first().cloned().unwrap_or_default(),
                size: 0,
            },
        ],
        sha256_checksum: Some(serde_json::to_string(&sha256_urls).unwrap_or_default()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_4xx_without_explicit_optin_is_hard_error() {
        // 所有校验源 4xx 且用户未开启跳过 → 必须拒绝安装（原实现返回 Ok(true) 直接通过）
        let result = decide_checksum_missing(true, false, false);
        assert!(result.is_err(), "未显式开启跳过时必须拒绝未校验安装");
        assert!(result.unwrap_err().contains("skipSha256WhenMissing"));
    }

    // ===== compare_versions =====

    #[test]
    fn hotfix_four_segment_version_is_detected() {
        // 历史 bug：take(3) 截断使 2.3.0.1 与 2.3.0 判等，hotfix 永不提示
        assert!(compare_versions("2.3.0", "2.3.0.1"));
        assert!(!compare_versions("2.3.0.1", "2.3.0"));
    }

    #[test]
    fn version_compare_numeric_and_v_prefix() {
        assert!(compare_versions("2.9.0", "2.10.0"));
        assert!(compare_versions("v2.3.0", "v2.4.0"));
        assert!(!compare_versions("2.3.0", "2.3.0"));
    }

    #[test]
    fn extract_checksum_supports_bsd_style() {
        let bsd = "SHA256 (Wxxy-CampusLogin_2.3.0_x64-setup.exe) = ABCD1234ABCD1234ABCD1234ABCD1234ABCD1234ABCD1234ABCD1234ABCD1234";
        assert_eq!(
            extract_checksum("https://example.com/pkg.sha256", bsd).unwrap(),
            "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
        );
        let shasum = "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234  pkg.exe";
        assert_eq!(
            extract_checksum("https://example.com/pkg.sha256", shasum).unwrap(),
            "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"
        );
    }

    #[test]
    fn all_4xx_with_explicit_optin_is_allowed() {
        let result = decide_checksum_missing(true, false, true);
        assert!(result.is_ok(), "用户显式开启跳过后允许降级通过");
    }

    #[test]
    fn server_error_or_transport_error_always_blocks() {
        // 5xx：即使开启跳过也拒绝
        assert!(decide_checksum_missing(false, false, true).is_err());
        assert!(decide_checksum_missing(false, false, false).is_err());
        // 传输错误：即使开启跳过也拒绝
        assert!(decide_checksum_missing(true, true, true).is_err());
        assert!(decide_checksum_missing(false, true, false).is_err());
    }

    const HASH_LOWER: &str = "3c91004d01826e0211f6e95512192262baa7aeabc863317a6e7a1910531d5271";

    #[test]
    fn extract_checksum_from_github_api_json() {
        let body = format!(
            r#"{{"tag_name":"v2.3.0","assets":[{{"name":"Wxxy-CampusLogin_2.3.0_x64-setup.exe","digest":"sha256:{HASH_LOWER}"}},{{"name":"source.zip"}}]}}"#
        );
        assert_eq!(
            extract_checksum("https://api.github.com/repos/x/y/releases/tags/v2.3.0", &body),
            Some(HASH_LOWER.to_string())
        );
    }

    #[test]
    fn extract_checksum_api_json_without_exe_asset_is_none() {
        let body = r#"{"tag_name":"v2.3.0","assets":[{"name":"source.zip"}]}"#;
        assert_eq!(
            extract_checksum("https://api.github.com/repos/x/y/releases/tags/v2.3.0", body),
            None
        );
    }

    #[test]
    fn extract_checksum_from_sha256_file_text() {
        // shasum 风格 "<hash>  <filename>"，大写 hash 也归一化为小写
        let upper: String = HASH_LOWER.to_uppercase();
        assert_eq!(
            extract_checksum("https://github.com/x/y/releases/download/v/.exe.sha256", &format!("{upper}  app.exe")),
            Some(HASH_LOWER.to_string())
        );
        // 非 64 位 token 视为无效
        assert_eq!(extract_checksum("https://mirror/a.sha256", "not-a-hash"), None);
        // API 响应损坏（非 JSON）→ None 而非 panic
        assert_eq!(extract_checksum("https://api.github.com/x", "rate limited"), None);
    }
}
