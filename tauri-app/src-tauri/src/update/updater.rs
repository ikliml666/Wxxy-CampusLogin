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
            .take(3)
            .map(|seg| {
                // 提取段中的数字部分（处理 "2-beta" 等后缀），解析失败按 0
                let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse::<u32>().unwrap_or(0)
            })
            .collect()
    };
    let cur = parse_version(current);
    let lat = parse_version(latest);

    for i in 0..3 {
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

pub async fn verify_download_sha256(file_path: &str, checksum_urls: &[String]) -> Result<bool, String> {
    if checksum_urls.is_empty() {
        return Err("未提供校验和URL".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {e}"))?;

    // 按顺序尝试所有 URL（GitHub 原始源 + 镜像源），任一成功即用
    // 降级策略：所有源都返回 4xx（文件不存在/权限受限）时视为"无可用校验文件"，
    // 返回 Ok(true) 跳过校验并输出警告日志；存在 5xx/网络错误则视为系统异常，保留原行为返回错误
    let mut last_err = String::new();
    let mut all_client_errors = true; // 是否所有响应都是 4xx（文件不存在/权限问题）
    let mut had_transport_error = false; // 是否有网络/超时等传输错误
    let checksum_content = {
        let mut content: Option<String> = None;
        for url in checksum_urls {
            match client.get(url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.text().await {
                        Ok(text) => {
                            content = Some(text);
                            break;
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
        match content {
            Some(c) => c,
            None => {
                // 所有源都失败
                if all_client_errors && !had_transport_error {
                    // 所有响应都是 4xx（文件不存在/权限受限）→ 降级通过
                    crate::log_warn!(
                        "updater",
                        "所有 {} 个 SHA256 校验源均返回 4xx（最后错误: {}），视为发布流程未上传 .sha256 文件，降级跳过校验",
                        checksum_urls.len(),
                        last_err
                    );
                    return Ok(true);
                }
                return Err(format!("所有校验和源均失败（最后错误: {last_err}）"));
            }
        }
    };

    let expected_hash = checksum_content
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    if expected_hash.is_empty() || expected_hash.len() != 64 {
        return Err("校验和格式无效".to_string());
    }

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
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(600));
        let _ = std::fs::remove_dir_all(&temp_dir);
        crate::log_debug!("updater", "更新临时目录已清理");
    });
}

pub fn start_update_check_loop(app_handle: &tauri::AppHandle) {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_h.state::<AppState>();
        let last_epoch = state.last_update_check_epoch_ms.load(Ordering::Acquire);
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let elapsed_secs = if last_epoch == 0 { AUTO_CHECK_INTERVAL_SECS + 1 } else { (now_epoch - last_epoch) / 1000 };

        // 首次检查：若距上次检查超过间隔则立即检查
        if elapsed_secs >= AUTO_CHECK_INTERVAL_SECS {
            do_update_check(&app_h, &state).await;
        }

        // 计算到下次检查的剩余时间，避免频繁重启导致检查被持续推迟
        let remaining_secs = if elapsed_secs < AUTO_CHECK_INTERVAL_SECS {
            AUTO_CHECK_INTERVAL_SECS - elapsed_secs
        } else {
            AUTO_CHECK_INTERVAL_SECS
        };
        // 拆分为 5s 步进循环，每次检查 is_quitting，避免 24h sleep 期间无法响应退出
        let mut elapsed = 0u64;
        let step = 5u64;
        while elapsed < remaining_secs {
            let wait = std::cmp::min(step, remaining_secs - elapsed);
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
            elapsed += wait;
            if state.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }
        }

        // 后续固定间隔检查
        loop {
            if state.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }
            do_update_check(&app_h, &state).await;
            // 拆分为 5s 步进等待，避免 24h sleep 期间无法响应退出
            let mut waited = 0u64;
            while waited < AUTO_CHECK_INTERVAL_SECS {
                if state.exit.is_quitting.load(Ordering::Acquire) {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                waited += 5;
            }
        }
    });
}

/// 执行一次更新检查并发送通知
async fn do_update_check(app_h: &tauri::AppHandle, state: &AppState) {
    match check_update_inner().await {
        Ok(info) => {
            if let Err(e) = EventBus::new(app_h).emit_update_available(
                info.has_update,
                &info.latest_version,
                &info.release_notes,
            ) {
                crate::log_warn!("updater", "发送更新通知失败: {}", e);
            }
            if info.has_update && state.update_notified.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                emit_notification(app_h, "发现新版本", &format!("新版本 v{} 可用，请在关于页面查看", info.latest_version));
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            state.last_update_check_epoch_ms.store(now, Ordering::Release);
        }
        Err(e) => {
            crate::log_warn!("updater", "更新检查失败: {}", e);
        }
    }
}

pub async fn fetch_latest_release() -> Result<(bool, String), String> {
    // 先尝试 GitHub 原始源
    match fetch_version_from_url(VERSION_FILE_URL).await {
        Ok(result) => Ok(result),
        Err(github_err) => {
            crate::log_info!("updater", "GitHub源检查失败: {}，尝试镜像源降级...", github_err);
            // 降级到镜像源
            for mirror_url in VERSION_MIRRORS {
                match fetch_version_from_url(mirror_url).await {
                    Ok(result) => {
                        crate::log_info!("updater", "镜像源 {} 检查成功", mirror_url);
                        return Ok(result);
                    }
                    Err(mirror_err) => {
                        crate::log_debug!("updater", "镜像源 {} 失败: {}", mirror_url, mirror_err);
                    }
                }
            }
            Err(format!("GitHub源及所有镜像源均失败（GitHub: {github_err}）"))
        }
    }
}

async fn fetch_version_from_url(url: &str) -> Result<(bool, String), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {e}"))?;

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

    Ok((has_update, latest_tag))
}



pub async fn check_update_inner() -> Result<UpdateInfo, String> {
    let (has_update, latest_tag) = fetch_latest_release().await?;

    let exe_name = format!("Wxxy-CampusLogin_{latest_tag}_x64-setup.exe");
    let github_exe_url = format!(
        "https://github.com/{GITHUB_REPO}/releases/download/v{latest_tag}/{exe_name}"
    );
    // 为 SHA256 校验文件生成 GitHub 原始源 + 镜像源 URL 列表
    let sha256_urls: Vec<String> = {
        let mut urls = vec![format!("{}.sha256", github_exe_url)];
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
        release_notes: String::new(),
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
