use tauri::{AppHandle, Emitter, Manager, State};
use serde::{Deserialize, Serialize};
use crate::commands::state::AppState;
use std::sync::atomic::Ordering;

const GITHUB_REPO: &str = "ikliml666/Wxxy-CampusLogin";
const UPDATE_CHECK_INTERVAL_SECS: u64 = 86400;

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

#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    let (has_update, latest_tag, data) = fetch_latest_release().await?;

    let release_notes = data["body"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let assets = data["assets"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let name = a["name"].as_str()?.to_string();
                    let url = a["browser_download_url"].as_str()?.to_string();
                    let size = a["size"].as_u64().unwrap_or(0);
                    Some(ReleaseAsset { name, url, size })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let sha256_checksum = data["assets"]
        .as_array()
        .and_then(|arr| {
            arr.iter().find_map(|a| {
                let name = a["name"].as_str().unwrap_or("");
                if name.ends_with(".sha256") || name.ends_with(".sha256sum") {
                    a["browser_download_url"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    Ok(UpdateInfo {
        has_update,
        latest_version: latest_tag,
        release_notes,
        assets,
        sha256_checksum,
    })
}

fn compare_versions(current: &str, latest: &str) -> bool {
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

#[tauri::command]
pub async fn download_update(
    app_handle: AppHandle,
    url: String,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err("仅允许HTTPS协议下载更新包".to_string());
    }

    let parsed = url::Url::parse(&url).map_err(|e| format!("URL解析失败: {}", e))?;
    let host = parsed.host_str().unwrap_or("");
    let allowed_hosts = [
        "github.com",
        "api.github.com",
        "github-releases.githubusercontent.com",
        "objects.githubusercontent.com",
        "ghfast.top",
        "gh-proxy.com",
        "gh-proxy.org",
        "ghproxy.net",
        "gh.llkk.cc",
        "gh.ddlc.top",
        "ghproxy.homeboyc.cn",
        "githubproxy.cc",
        "ghproxylist.com",
        "moeyy.cn",
    ];
    if !allowed_hosts.iter().any(|h| host == *h || host.ends_with(&format!(".{}", h))) {
        return Err(format!("不允许从该域名下载: {}", host));
    }

    let filename = parsed
        .path_segments()
        .and_then(|seg| seg.last())
        .unwrap_or("update.exe")
        .to_string();

    let temp_dir = std::env::temp_dir().join("campus-login-update");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
    let file_path = temp_dir.join(&filename);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let mut response = client
        .get(&url)
        .header("User-Agent", "CampusLogin-Updater")
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("下载失败: HTTP {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("创建临时文件失败: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut last_downloaded: u64 = 0;

    use std::io::Write;
    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(|e| format!("读取数据失败: {}", e))?;

        match chunk {
            Some(data) => {
                file.write_all(&data)
                    .map_err(|e| format!("写入文件失败: {}", e))?;
                downloaded += data.len() as u64;

                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last_emit);
                if elapsed.as_millis() >= 200 || (total_size > 0 && downloaded == total_size) {
                    let speed = if elapsed.as_secs_f64() > 0.0 {
                        ((downloaded - last_downloaded) as f64 / elapsed.as_secs_f64()) as u64
                    } else {
                        0
                    };
                    let percent = if total_size > 0 {
                        (downloaded as f64 / total_size as f64) * 100.0
                    } else {
                        0.0
                    };

                    if let Err(e) = app_handle.emit("update-download-progress", DownloadProgress {
                        downloaded,
                        total: total_size,
                        speed,
                        percent,
                    }) {
                        crate::log_warn!("updater", "发送下载进度失败: {}", e);
                    }

                    last_emit = now;
                    last_downloaded = downloaded;
                }
            }
            None => break,
        }
    }

    file.flush().map_err(|e| format!("刷新文件失败: {}", e))?;
    drop(file);

    let path_str = file_path
        .to_str()
        .ok_or("文件路径转换失败")?
        .to_string();

    if let Err(e) = app_handle.emit("update-download-progress", DownloadProgress {
        downloaded,
        total: downloaded,
        speed: 0,
        percent: 100.0,
    }) {
        crate::log_warn!("updater", "发送下载完成进度失败: {}", e);
    }

    Ok(path_str)
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

fn schedule_update_cleanup() {
    let temp_dir = std::env::temp_dir().join("campus-login-update");
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(600));
        let _ = std::fs::remove_dir_all(&temp_dir);
        crate::log_debug!("updater", "更新临时目录已清理");
    });
}

#[tauri::command]
pub async fn install_update(file_path: String, checksum_url: Option<String>) -> Result<bool, String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err("安装包文件不存在".to_string());
    }

    if let Some(url) = checksum_url {
        if !url.is_empty() {
            match verify_download_sha256(&file_path, &url).await {
                Ok(true) => {}
                Ok(false) => {
                    let _ = std::fs::remove_file(&file_path);
                    return Err("安装包校验失败：SHA256不匹配，文件可能被篡改".to_string());
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&file_path);
                    return Err(format!("SHA256校验过程失败，安装已阻止: {}", e));
                }
            }
        } else {
            return Err("未提供SHA256校验和，安装已阻止".to_string());
        }
    } else {
        return Err("未提供SHA256校验和，安装已阻止".to_string());
    }

    let temp_dir = std::env::temp_dir().join("campus-login-update");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;

    let canonical_path = path.canonicalize().map_err(|e| format!("无法解析文件路径: {}", e))?;
    let allowed_dir = temp_dir.canonicalize().map_err(|_| "无法解析临时目录路径".to_string())?;
    if !canonical_path.starts_with(&allowed_dir) {
        return Err("安装包路径不在允许的临时目录中".to_string());
    }

    let ext = canonical_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "exe" {
        let result = open::that(canonical_path).map(|_| true).map_err(|e| format!("启动安装程序失败: {}", e));
        if result.is_ok() {
            schedule_update_cleanup();
        }
        result
    } else if ext == "msi" {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let result = std::process::Command::new("msiexec")
                .raw_arg(&format!("/i \"{}\"", canonical_path.display()))
                .creation_flags(0x08000000)
                .spawn()
                .map(|_| true)
                .map_err(|e| format!("启动MSI安装失败: {}", e));
            if result.is_ok() {
                schedule_update_cleanup();
            }
            result
        }
        #[cfg(not(target_os = "windows"))]
        {
            let result = std::process::Command::new("msiexec")
                .args(["/i", &canonical_path.display().to_string()])
                .spawn()
                .map(|_| true)
                .map_err(|e| format!("启动MSI安装失败: {}", e));
            if result.is_ok() {
                schedule_update_cleanup();
            }
            result
        }
    } else {
        Err(format!("不支持的安装包格式: {}", ext))
    }
}

#[tauri::command]
pub fn get_mirror_urls(github_url: String) -> Result<Vec<serde_json::Value>, String> {
    if !github_url.starts_with("https://github.com/") && !github_url.starts_with("http://github.com/") {
        return Err("无效的GitHub URL".to_string());
    }
    if github_url.contains("..") || github_url.contains('\\') {
        return Err("URL包含非法字符".to_string());
    }

    let encoded = urlencoding::encode(&github_url);
    let mirrors = vec![
        serde_json::json!({
            "name": "GitHub 官方",
            "url": github_url.clone(),
            "description": "官方源，海外网络推荐"
        }),
        serde_json::json!({
            "name": "ghfast.top",
            "url": format!("https://ghfast.top/{}", encoded),
            "description": "国内加速，速度较快"
        }),
        serde_json::json!({
            "name": "gh-proxy.com",
            "url": format!("https://gh-proxy.com/{}", encoded),
            "description": "国内加速，多区域节点"
        }),
        serde_json::json!({
            "name": "ghproxy.net",
            "url": format!("https://ghproxy.net/{}", encoded),
            "description": "国内加速镜像"
        }),
        serde_json::json!({
            "name": "gh.llkk.cc",
            "url": format!("https://gh.llkk.cc/{}", encoded),
            "description": "国内加速镜像"
        }),
    ];

    Ok(mirrors)
}

pub fn start_update_check_loop(app_handle: &AppHandle) {
    let app_h = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_h.state::<AppState>();
        let last_epoch = state.last_update_check_epoch_ms.load(Ordering::Acquire);
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let elapsed_secs = if last_epoch == 0 { UPDATE_CHECK_INTERVAL_SECS + 1 } else { (now_epoch - last_epoch) / 1000 };

        if elapsed_secs >= UPDATE_CHECK_INTERVAL_SECS {
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

        let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(UPDATE_CHECK_INTERVAL_SECS));
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

async fn fetch_latest_release() -> Result<(bool, String, serde_json::Value), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))?;

    let resp = client
        .get(format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "CampusLogin-UpdateChecker")
        .send()
        .await
        .map_err(|e| format!("请求GitHub API失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        if status.as_u16() == 403 {
            return Err("GitHub API 请求频率受限，请稍后再试".to_string());
        }
        return Err(format!("GitHub API返回错误: {}", status));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析GitHub响应失败: {}", e))?;

    let latest_tag = data["tag_name"]
        .as_str()
        .unwrap_or("")
        .replace("v", "");

    if latest_tag.is_empty() {
        return Err("无法获取最新版本号".to_string());
    }

    let current = env!("CARGO_PKG_VERSION");
    let has_update = compare_versions(current, &latest_tag);

    Ok((has_update, latest_tag, data))
}

async fn check_update_inner() -> Result<UpdateInfo, String> {
    let (has_update, latest_tag, data) = fetch_latest_release().await?;
    let release_notes = data["body"]
        .as_str()
        .unwrap_or("")
        .to_string();
    Ok(UpdateInfo {
        has_update,
        latest_version: latest_tag,
        release_notes,
        assets: vec![],
        sha256_checksum: None,
    })
}
