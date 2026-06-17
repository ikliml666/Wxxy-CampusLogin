use tauri::{AppHandle, Emitter, Manager, State};
use crate::infra::state::AppState;
use crate::update::updater::DownloadProgress;
use std::sync::atomic::Ordering;

#[tauri::command]
pub async fn check_update(app_handle: AppHandle, _state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    crate::log_info!("updater", "手动检查更新");
    let info = crate::update::updater::check_update_inner().await?;

    let state = app_handle.state::<AppState>();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    state.last_update_check_epoch_ms.store(now, Ordering::Release);

    Ok(serde_json::to_value(info).map_err(|e| format!("序列化更新信息失败: {}", e))?)
}

#[tauri::command]
pub async fn download_update(
    app_handle: AppHandle,
    url: String,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    crate::log_info!("updater", "开始下载更新: {}", url);
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

    const MAX_DOWNLOAD_SIZE: u64 = 500 * 1024 * 1024;

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
    if total_size > MAX_DOWNLOAD_SIZE {
        return Err(format!("文件过大({}MB)，超过大小限制{}MB)", total_size / 1024 / 1024, MAX_DOWNLOAD_SIZE / 1024 / 1024));
    }

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

                if downloaded > MAX_DOWNLOAD_SIZE {
                    drop(file);
                    let _ = std::fs::remove_file(&file_path);
                    return Err(format!("下载文件超过大小限制({}MB)", MAX_DOWNLOAD_SIZE / 1024 / 1024));
                }

                let now = std::time::Instant::now();
                let elapsed = now.saturating_duration_since(last_emit);
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

    crate::log_info!("updater", "更新下载完成: {}", path_str);
    Ok(path_str)
}

#[tauri::command]
pub async fn install_update(file_path: String, checksum_url: Option<String>) -> Result<bool, String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err("安装包文件不存在".to_string());
    }

    // 路径校验必须在 SHA256 校验之前，避免校验失败时删除非临时目录的文件
    let temp_dir = std::env::temp_dir().join("campus-login-update");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;

    let canonical_path = path.canonicalize().map_err(|e| format!("无法解析文件路径: {}", e))?;
    let allowed_dir = temp_dir.canonicalize().map_err(|_| "无法解析临时目录路径".to_string())?;
    if !canonical_path.starts_with(&allowed_dir) {
        return Err("安装包路径不在允许的临时目录中".to_string());
    }

    if let Some(url) = checksum_url {
        if !url.is_empty() {
            // checksum_url 可能是 JSON 数组字符串（多镜像 URL）或单个 URL
            let sha256_urls: Vec<String> = if url.starts_with('[') {
                serde_json::from_str(&url).unwrap_or_else(|_| vec![url])
            } else {
                vec![url]
            };

            match crate::update::updater::verify_download_sha256(&file_path, &sha256_urls).await {
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

    let ext = canonical_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "exe" {
        let result = open::that(canonical_path).map(|_| true).map_err(|e| format!("启动安装程序失败: {}", e));
        if result.is_ok() {
            crate::update::updater::schedule_update_cleanup();
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
                crate::update::updater::schedule_update_cleanup();
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
                crate::update::updater::schedule_update_cleanup();
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


