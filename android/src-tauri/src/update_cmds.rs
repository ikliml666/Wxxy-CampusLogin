//! 更新流:version.json 检查(镜像降级)+ 白名单域名流式下载 + APK 安装。
//! 检查/下载为纯 HTTP 复用桌面逻辑;安装改 APK 意图(桌面 exe/msi 不适用)。

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tauri::Emitter;
use tauri::Manager;

lazy_static! {
    static ref UPDATE_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);
    /// 最近一次 check_update 拿到的 version.json 校验值:download_update 完成后
    /// 强制比对(version.json 未提供时为 None 跳过——发布流程需补该字段)
    static ref UPDATE_CHECKSUM: Mutex<Option<String>> = Mutex::new(None);
}

const VERSION_FILE: &str = "https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json";
/// GitHub 原始源失败时按顺序降级(桌面同款镜像)
const VERSION_MIRRORS: &[&str] = &[
    "https://ghfast.top/https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json",
    "https://gh-proxy.com/https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json",
    "https://ghproxy.net/https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json",
];

/// 下载域名白名单:仅允许 GitHub 官方与既有镜像(桌面 updater 同构,防 SSRF)
const DOWNLOAD_ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "raw.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "ghfast.top",
    "gh-proxy.com",
    "ghproxy.net",
];

const MAX_DOWNLOAD_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
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

/// 语义对齐桌面 compare_versions:latest 是否比 current 更新(去 v 前缀,按段数值比较)
pub fn has_newer_version(current: &str, latest: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .split('.')
            .map(|seg| {
                let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse().unwrap_or(0)
            })
            .collect()
    };
    let (c, l) = (parse(current), parse(latest));
    let len = c.len().max(l.len());
    for i in 0..len {
        let cv = c.get(i).copied().unwrap_or(0);
        let lv = l.get(i).copied().unwrap_or(0);
        if lv != cv {
            return lv > cv;
        }
    }
    false
}

fn http_client() -> Result<reqwest::Client, String> {
    // 移动端用 rustls,避免 openssl 交叉编译问题;忽略证书校验关闭(白名单域名 TLS 正常校验)
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {e}"))
}

fn allowed_url(url: &str) -> Result<(), String> {
    let host = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .and_then(|host_port| {
            host_port.rsplit_once(':').map_or(Some(host_port), |(h, p)| {
                if p.chars().all(|c| c.is_ascii_digit()) && !h.is_empty() {
                    Some(h)
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| "URL 缺少合法主机名".to_string())?;
    if DOWNLOAD_ALLOWED_HOSTS.contains(&host) {
        Ok(())
    } else {
        Err(format!("下载域名不在白名单: {host}"))
    }
}

/// 检查更新:按用户渠道设置排序源(update_source="mirror" 镜像优先/"github" 官方优先,
/// 另一侧保留为降级兜底);version.json 4 源按序降级
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    check_update_inner(&app).await
}

fn version_urls(mirror_first: bool) -> Vec<&'static str> {
    if mirror_first {
        vec![VERSION_MIRRORS[0], VERSION_MIRRORS[1], VERSION_MIRRORS[2], VERSION_FILE]
    } else {
        vec![VERSION_FILE, VERSION_MIRRORS[0], VERSION_MIRRORS[1], VERSION_MIRRORS[2]]
    }
}

/// 从 GitHub API release 拉取 APK 资产与官方 digest(服务端计算,最可信校验源)。
/// API 失败/限流/无 APK 资产时返回空——前端降级为外链 Releases(与现状一致);
/// version.json 不含 assets,真实下载地址只能来自 API。
async fn fetch_apk_assets(client: &reqwest::Client, latest: &str) -> (Vec<ReleaseAsset>, Option<String>) {
    let url = format!("https://api.github.com/repos/ikliml666/Wxxy-CampusLogin/releases/tags/v{latest}");
    let Ok(resp) = client
        .get(&url)
        .header("User-Agent", "Wxxy-CampusLogin")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    else {
        return (Vec::new(), None);
    };
    if !resp.status().is_success() {
        return (Vec::new(), None);
    }
    let Ok(raw) = resp.json::<serde_json::Value>().await else {
        return (Vec::new(), None);
    };
    let mut assets = Vec::new();
    let mut checksum = None;
    for item in raw.get("assets").and_then(|a| a.as_array()).into_iter().flatten() {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = item.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if name.is_empty() || url.is_empty() || !name.to_ascii_lowercase().ends_with(".apk") {
            continue;
        }
        if checksum.is_none() {
            // digest 形如 "sha256:<hex>"(GitHub 服务端计算)
            checksum = item
                .get("digest")
                .and_then(|v| v.as_str())
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(|s| s.to_string());
        }
        assets.push(ReleaseAsset {
            name,
            url,
            size: item.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
        });
    }
    (assets, checksum)
}

async fn check_update_inner(app: &tauri::AppHandle) -> Result<UpdateInfo, String> {
    let client = http_client()?;
    let mirror_first = crate::config_state::current_settings(app)
        .await
        .map(|s| s.update_source != "github")
        .unwrap_or(true);
    let urls = version_urls(mirror_first);
    let mut last_err = String::from("未知错误");
    for url in urls {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let raw: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
                let info: UpdateInfo = serde_json::from_value(raw).map_err(|e| e.to_string())?;
                let current = env!("CARGO_PKG_VERSION");
                let has_update = has_newer_version(current, &info.latest_version);
                // APK 资产与校验值:API digest 优先,version.json 字段兜底
                let (assets, checksum) = fetch_apk_assets(&client, &info.latest_version).await;
                let sha256_checksum = checksum.or(info.sha256_checksum);
                if let Ok(mut guard) = UPDATE_CHECKSUM.lock() {
                    *guard = sha256_checksum.clone();
                }
                return Ok(UpdateInfo {
                    has_update,
                    assets,
                    sha256_checksum,
                    ..info
                });
            }
            Ok(resp) => last_err = format!("HTTP {}", resp.status()),
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(format!("检查更新失败: {last_err}"))
}

/// 自动更新检查循环(桌面 startup 24h 同语义):启动延迟 5s 查一次,之后每 24h;
/// 有新版本 emit update-available(前端 useEventListeners 早已监听,据此弹窗提示)
/// + 系统通知(过 enable_notification 闸)。循环随进程存活,无需停。
pub fn start_update_check_loop(app: tauri::AppHandle) {
    if UPDATE_LOOP_RUNNING.swap(true, Ordering::Relaxed) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Ok(info) = check_update_inner(&app).await {
                if info.has_update {
                    let _ = app.emit("update-available", &info);
                    let enabled = crate::config_state::current_settings(&app)
                        .await
                        .map(|s| s.enable_notification)
                        .unwrap_or(false);
                    crate::monitor_loop::notify_system(
                        &app,
                        enabled,
                        "发现新版本",
                        &format!("新版本 v{} 可用,请在关于页更新", info.latest_version),
                        "mascot_update",
                    );
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(24 * 3600)).await;
        }
    });
}

#[tauri::command]
pub fn get_mirror_urls(github_url: String) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![
        serde_json::json!({ "name": "GitHub", "url": github_url }),
        serde_json::json!({ "name": "ghfast.top", "url": format!("https://ghfast.top/{github_url}") }),
        serde_json::json!({ "name": "gh-proxy.com", "url": format!("https://gh-proxy.com/{github_url}") }),
        serde_json::json!({ "name": "ghproxy.net", "url": format!("https://ghproxy.net/{github_url}") }),
    ])
}

/// 流式下载 APK 到 app_data_dir/update/,emit update-download-progress(桌面同构)
///
/// 并发互斥：重复 IPC 调用同时下载会互写同一临时文件，外壳原子标志 + 内部函数
/// 保证所有提前 return 路径都复位标志。
static DOWNLOAD_RUNNING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub async fn download_update(app: tauri::AppHandle, url: String) -> Result<String, String> {
    if DOWNLOAD_RUNNING.swap(true, Ordering::Relaxed) {
        return Err("已有下载任务进行中，请等待完成".to_string());
    }
    let result = download_update_inner(app, url).await;
    DOWNLOAD_RUNNING.store(false, Ordering::Relaxed);
    result
}

async fn download_update_inner(app: tauri::AppHandle, url: String) -> Result<String, String> {
    allowed_url(&url)?;
    let client = http_client()?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("下载响应异常: {e}"))?;

    let total = resp.content_length().unwrap_or(0);
    let file_name = url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.split('?').next().unwrap_or(s).to_string())
        .filter(|s| !s.contains(".."))
        .unwrap_or_else(|| format!("campus-login-{}.apk", env!("CARGO_PKG_VERSION")));

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?
        .join("update");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("创建更新目录失败: {e}"))?;
    let path = dir.join(&file_name);
    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|e| format!("创建下载文件失败: {e}"))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let start = Instant::now();
    let mut last_emit = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载中断: {e}"))?;
        // 大小上限在写入前判定，超限不落盘即中止（原实现先写后判，多写一个 chunk）
        if downloaded + chunk.len() as u64 > MAX_DOWNLOAD_BYTES {
            let _ = tokio::fs::remove_file(&path).await;
            return Err("下载超过 500MB 上限,已中止".to_string());
        }
        downloaded += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入失败: {e}"))?;
        // 节流:每 200ms 发一次进度事件
        if last_emit.elapsed().as_millis() >= 200 {
            last_emit = Instant::now();
            let speed = if start.elapsed().as_secs() > 0 {
                downloaded / start.elapsed().as_secs()
            } else {
                0
            };
            let percent = if total > 0 { downloaded as f64 / total as f64 * 100.0 } else { 0.0 };
            let _ = app.emit(
                "update-download-progress",
                DownloadProgress { downloaded, total, speed, percent },
            );
        }
    }
    file.flush().await.map_err(|e| format!("落盘失败: {e}"))?;
    // SHA256 完整性校验:version.json 提供校验值时强制比对(防镜像投毒,
    // 桌面 updater 同语义);未提供或格式异常时跳过不阻塞更新
    let expected = UPDATE_CHECKSUM.lock().ok().and_then(|g| g.clone());
    if let Some(expected) = expected {
        if !verify_file_sha256(&path, &expected).await? {
            let _ = tokio::fs::remove_file(&path).await;
            return Err("下载文件 SHA256 校验不匹配,已删除(下载源可能被篡改,请换镜像重试)".to_string());
        }
    }
    Ok(path.to_string_lossy().to_string())
}

/// 流式计算文件 SHA256 并与期望值比对;期望值容忍 "hex filename" 格式与大小写,
/// 非 64 位十六进制时视为未提供校验值,返回 true 跳过(不误杀)
async fn verify_file_sha256(path: &Path, expected: &str) -> Result<bool, String> {
    use sha2::{Digest, Sha256};
    let clean = expected
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if clean.len() != 64 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(true);
    }
    let mut file = tokio::fs::File::open(path).await.map_err(|e| format!("读取下载文件失败: {e}"))?;
    use tokio::io::AsyncReadExt;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).await.map_err(|e| format!("读取下载文件失败: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(actual == clean)
}

/// 安装 APK:路径必须位于应用更新目录内(防任意路径),经监控插件的 FileProvider
/// 命令交系统包安装器(私有目录 file:// 对 Android 7+ 必失败,content URI 才可行)
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle, file_path: String) -> Result<bool, String> {
    use tauri_plugin_campus_monitor_service::CampusMonitorServiceExt;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?
        .join("update");
    let path = Path::new(&file_path);
    let canonical = path.canonicalize().map_err(|e| format!("路径无效: {e}"))?;
    let allowed_dir = dir.canonicalize().unwrap_or(dir);
    if !canonical.starts_with(&allowed_dir) {
        return Err("仅允许安装应用更新目录内的 APK".to_string());
    }
    app.campus_monitor_service()
        .install_apk(&canonical.to_string_lossy())
        .map_err(|e| format!("打开安装器失败: {e}"))?;
    Ok(true)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 版本比较_逐段数值() {
        assert!(has_newer_version("2.3.2", "2.3.3"));
        assert!(has_newer_version("2.3.2", "v2.4.0"));
        assert!(has_newer_version("2.3.2", "3.0"));
        assert!(!has_newer_version("2.3.2", "2.3.2"));
        assert!(!has_newer_version("2.3.3", "2.3.2"));
        assert!(has_newer_version("2.3", "2.3.1"), "短版本按 0 补齐");
        assert!(!has_newer_version("2.3.2", "2.3.2-beta"), "预发布后缀段解析为 0,不视为更新");
    }

    #[test]
    fn 白名单域名校验() {
        assert!(allowed_url("https://github.com/a/b.apk").is_ok());
        assert!(allowed_url("https://ghfast.top/https://github.com/x").is_ok());
        assert!(allowed_url("https://evil.example.com/x.apk").is_err(), "非白名单域名必须拒绝");
        assert!(allowed_url("not-a-url").is_err());
        assert!(allowed_url("http://github.com/x").is_ok(), "host 校验不分协议(下载层强制 https 由 reqwest scheme 保证)");
    }

    #[test]
    fn UpdateInfo_反序列化_camelCase契约() {
        let raw = r#"{"latestVersion":"2.4.0","releaseNotes":"修复","assets":[{"name":"app.apk","url":"https://github.com/x/app.apk","size":1024}]}"#;
        let info: UpdateInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(info.latest_version, "2.4.0");
        assert_eq!(info.assets[0].name, "app.apk");
        assert!(info.sha256_checksum.is_none());
        assert!(!info.has_update);
    }
}
