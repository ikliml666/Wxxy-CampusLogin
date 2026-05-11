use tauri::{AppHandle, Emitter, State};
use serde::{Deserialize, Serialize};
use crate::commands::state::AppState;

const GITHUB_REPO: &str = "ikliml666/Wxxy-CampusLogin";

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
        return Err(format!("GitHub API返回错误: {}", resp.status()));
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

    Ok(UpdateInfo {
        has_update,
        latest_version: latest_tag,
        release_notes,
        assets,
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
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("无效的下载链接".to_string());
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
                    let speed = if elapsed.as_secs() > 0 {
                        ((downloaded - last_downloaded) as f64 / elapsed.as_secs_f64()) as u64
                    } else {
                        0
                    };
                    let percent = if total_size > 0 {
                        (downloaded as f64 / total_size as f64) * 100.0
                    } else {
                        0.0
                    };

                    let _ = app_handle.emit("update-download-progress", DownloadProgress {
                        downloaded,
                        total: total_size,
                        speed,
                        percent,
                    });

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

    let _ = app_handle.emit("update-download-progress", DownloadProgress {
        downloaded,
        total: downloaded,
        speed: 0,
        percent: 100.0,
    });

    Ok(path_str)
}

#[tauri::command]
pub fn install_update(file_path: String) -> Result<bool, String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err("安装包文件不存在".to_string());
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
        open::that(canonical_path).map(|_| true).map_err(|e| format!("启动安装程序失败: {}", e))
    } else if ext == "msi" {
        let args = format!("/i \"{}\"", canonical_path.display());
        std::process::Command::new("msiexec")
            .args(args.split_whitespace())
            .spawn()
            .map(|_| true)
            .map_err(|e| format!("启动MSI安装失败: {}", e))
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

    let mirrors = vec![
        serde_json::json!({
            "name": "GitHub 官方",
            "url": github_url.clone(),
            "description": "官方源，海外网络推荐"
        }),
        serde_json::json!({
            "name": "ghfast.top",
            "url": format!("https://ghfast.top/{}", github_url),
            "description": "国内加速，速度较快"
        }),
        serde_json::json!({
            "name": "gh-proxy.com",
            "url": format!("https://gh-proxy.com/{}", github_url),
            "description": "国内加速，多区域节点"
        }),
        serde_json::json!({
            "name": "ghproxy.net",
            "url": format!("https://ghproxy.net/{}", github_url),
            "description": "国内加速镜像"
        }),
        serde_json::json!({
            "name": "gh.llkk.cc",
            "url": format!("https://gh.llkk.cc/{}", github_url),
            "description": "国内加速镜像"
        }),
    ];

    Ok(mirrors)
}
