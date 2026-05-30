use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use crate::infra::state::AppState;
use std::sync::atomic::Ordering;

const GITHUB_REPO: &str = "ikliml666/Wxxy-CampusLogin";
const AUTO_CHECK_INTERVAL_SECS: u64 = 86400;
#[allow(dead_code)]
const MANUAL_CHECK_COOLDOWN_SECS: u64 = 600;
const VERSION_FILE_URL: &str = "https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json";

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
    let cur: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let lat: Vec<u32> = latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

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

pub async fn verify_download_sha256(file_path: &str, checksum_url: &str) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let resp = client.get(checksum_url).send().await
        .map_err(|e| format!("获取校验和文件失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取校验和失败: HTTP {}", resp.status()));
    }

    let checksum_content = resp.text().await
        .map_err(|e| format!("读取校验和内容失败: {}", e))?;

    let expected_hash = checksum_content
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    if expected_hash.is_empty() || expected_hash.len() != 64 {
        return Err("校验和格式无效".to_string());
    }

    let data = tokio::task::spawn_blocking({
        let path = file_path.to_string();
        move || std::fs::read(&path)
    }).await
    .map_err(|e| format!("读取文件任务失败: {}", e))?
    .map_err(|e| format!("读取下载文件失败: {}", e))?;

    use std::io::Write;
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.write_all(&data).map_err(|e| format!("计算哈希失败: {}", e))?;
    let result = hasher.finalize();
    let actual_hash = format!("{:x}", result);

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

        if elapsed_secs >= AUTO_CHECK_INTERVAL_SECS {
            if let Ok(info) = check_update_inner().await {
                if let Err(e) = app_h.emit("update-available", serde_json::json!({
                    "has_update": info.has_update,
                    "latest_version": info.latest_version,
                    "release_notes": info.release_notes,
                })) {
                    crate::log_warn!("updater", "发送更新通知失败: {}", e);
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                state.last_update_check_epoch_ms.store(now, Ordering::Release);
            }
        }

        let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(AUTO_CHECK_INTERVAL_SECS));
        interval_timer.tick().await;

        loop {
            interval_timer.tick().await;

            if state.exit.is_quitting.load(Ordering::Acquire) {
                break;
            }

            if let Ok(info) = check_update_inner().await {
                if let Err(e) = app_h.emit("update-available", serde_json::json!({
                    "has_update": info.has_update,
                    "latest_version": info.latest_version,
                    "release_notes": info.release_notes,
                })) {
                    crate::log_warn!("updater", "发送更新通知失败: {}", e);
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                state.last_update_check_epoch_ms.store(now, Ordering::Release);
            }
        }
    });
}

pub async fn fetch_latest_release() -> Result<(bool, String, serde_json::Value), String> {
    fetch_version_via_raw().await
}

async fn fetch_version_via_raw() -> Result<(bool, String, serde_json::Value), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let resp = client
        .get(VERSION_FILE_URL)
        .header("User-Agent", "CampusLogin-UpdateChecker")
        .send()
        .await
        .map_err(|e| format!("获取version.json失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("version.json不可用: HTTP {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析version.json失败: {}", e))?;

    let latest_tag = data["version"]
        .as_str()
        .unwrap_or("")
        .replace("v", "");

    if latest_tag.is_empty() {
        return Err("version.json中缺少版本号".to_string());
    }

    let current = env!("CARGO_PKG_VERSION");
    let has_update = compare_versions(current, &latest_tag);

    let body = data["notes"].as_str().unwrap_or("");
    let enriched = serde_json::json!({
        "tag_name": data["version"],
        "body": body,
        "html_url": format!("https://github.com/{}/releases/tag/v{}", GITHUB_REPO, latest_tag),
        "assets": data.get("assets").cloned().unwrap_or(serde_json::json!([])),
        "_source": "raw"
    });

    Ok((has_update, latest_tag, enriched))
}



pub async fn check_update_inner() -> Result<UpdateInfo, String> {
    let (has_update, latest_tag, data) = fetch_latest_release().await?;
    let release_notes = data["body"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let assets: Vec<ReleaseAsset> = data.get("assets")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter().filter_map(|item| {
                let name = item["name"].as_str().unwrap_or("").to_string();
                let url = item["browser_download_url"].as_str().unwrap_or("").to_string();
                let size = item["size"].as_u64().unwrap_or(0);
                if !name.is_empty() && !url.is_empty() {
                    Some(ReleaseAsset { name, url, size })
                } else {
                    None
                }
            }).collect()
        })
        .unwrap_or_default();

    let sha256_checksum = assets.iter()
        .find(|a| a.name.ends_with(".sha256"))
        .map(|a| a.url.clone());

    Ok(UpdateInfo {
        has_update,
        latest_version: latest_tag,
        release_notes,
        assets,
        sha256_checksum,
    })
}
