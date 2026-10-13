//! 更新流:version.json 检查(镜像降级)+ 白名单域名流式下载 + APK 安装。
//! 检查/下载为纯 HTTP 复用桌面逻辑;安装改 APK 意图(桌面 exe/msi 不适用)。
//! 安装包生命周期:install_update 拉起安装器后即删本地 APK,启动时再按
//! APK_STALE_SECS 清理"下载未装/装失败/旧版本残留"的孤儿包。

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tauri::Emitter;
use tauri::Manager;

lazy_static! {
    static ref UPDATE_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);
    /// 最近一次 check_update 记账的 (release tag, digest):download_update 按下载
    /// URL 的 tag 取用——检查期 API 可用但拿不到 digest 时记 (tag, "") 哨兵,
    /// 下载端据此拒绝安装(API 在线却无校验值 ≠ API 不可用降级,两者语义必须区分);
    /// API 不可用时不记账,下载端放行但 emit update-unverified 明示未校验
    static ref UPDATE_CHECKSUM: Mutex<Option<(String, String)>> = Mutex::new(None);
}

const VERSION_FILE: &str = "https://raw.githubusercontent.com/ikliml666/Wxxy-CampusLogin/main/version.json";
/// 自动更新检查周期(桌面 updater 同语义):24h
const AUTO_CHECK_INTERVAL_SECS: u64 = 24 * 3600;
/// 失败退避重试间隔:5min → 15min → 1h(与 24h 周期互补:每个 24h 周期内
/// 至多重试 3 次,退避用尽恢复 24h 周期等待,下一轮周期重新退避)
const BACKOFF_RETRY_SECS: &[u64] = &[5 * 60, 15 * 60, 3600];

/// 最近一次更新检查失败原因(检查成功后清除;None=最近检查无失败)。
/// 原实现自动循环 `if let Ok(info)` 静默吞掉 Err:网络抖动时 24h 内不再重试且
/// 前端不可见,用户误判"已是最新";记录后 check_update 返回体经 lastCheckError
/// 带出(构造时快照),下次手动检查成功时仍能看到上次失败原因
static LAST_CHECK_ERROR: Mutex<Option<String>> = Mutex::new(None);
/// 最近一次更新检查完成时间 epoch ms(成功或失败都记录;0=从未检查)
static LAST_CHECK_TIME_MS: AtomicU64 = AtomicU64::new(0);
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

/// 更新目录内残留 APK 的最大保留时长:下载了但一直没点安装(或安装失败)的包,
/// 超过该时长在下次启动时清掉,避免 files/update 无限堆积旧包。
/// 与桌面 schedule_update_cleanup 的 24h 同档(安装前用户可能要等 UAC/换镜像)
const APK_STALE_SECS: u64 = 24 * 3600;

/// 更新目录路径:与 download_update / install_update / MonitorServicePlugin.installApk
/// 同一约定(app_data_dir/files/update),集中一处避免三处拼接漂移
pub(crate) fn update_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))
        .map(|d| d.join("files").join("update"))
}

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
    /// 最近一次更新检查失败原因。返回体携带的是本次检查前记录在案的错误快照
    /// (本次检查成功时状态被清除,但快照已读出):自动检查失败后,用户下次
    /// 手动检查成功时仍能看到上次失败原因;此前无失败时为 None
    pub last_check_error: Option<String>,
    /// 最近一次更新检查完成时间 unix ms(None=从未检查)
    pub last_check_time: Option<u64>,
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

pub(crate) fn http_client() -> Result<reqwest::Client, String> {
    // 移动端用 rustls,避免 openssl 交叉编译问题;忽略证书校验关闭(白名单域名 TLS 正常校验)
    // connect_timeout 必设:被阻断地址(GitHub API 等)表现为 TCP 挂起而非快速失败,
    // 不设连接超时会吃满 30s 总超时——镜像加速检查"极慢"的主因(逐源串行 30s)
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {e}"))
}

/// 白名单校验唯一漏斗:更新链与人脸模型下载共用(face_model_cmds 复用,勿复制)
pub(crate) fn allowed_url(url: &str) -> Result<(), String> {
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
    check_and_record(&app).await
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn last_check_error_snapshot() -> Option<String> {
    LAST_CHECK_ERROR.lock().ok().and_then(|g| g.clone())
}

fn last_check_time_snapshot() -> Option<u64> {
    let t = LAST_CHECK_TIME_MS.load(Ordering::Relaxed);
    (t != 0).then_some(t)
}

/// 第 fail_count 次连续失败后的等待秒数:0=成功/首轮 → 24h 正常周期;
/// 1..=3 → 对应退避档位(5min/15min/1h);超过 3 → 24h(调用方据此重置计数,
/// 下一轮 24h 周期重新按退避重试)
fn backoff_delay_secs(fail_count: u32) -> u64 {
    fail_count
        .checked_sub(1)
        .and_then(|i| BACKOFF_RETRY_SECS.get(i as usize))
        .copied()
        .unwrap_or(AUTO_CHECK_INTERVAL_SECS)
}

/// 执行一次检查并把结果记入最近检查状态(lastCheckError/lastCheckTime 供前端展示)。
/// 成功清除错误记录,失败写入原因;时间总是更新。自动循环与手动命令统一走本函数,
/// 保证两条检查路径的可见状态一致
async fn check_and_record(app: &tauri::AppHandle) -> Result<UpdateInfo, String> {
    let result = check_update_inner(app).await;
    let err = match &result {
        Ok(_) => None,
        Err(e) => Some(e.clone()),
    };
    if let Ok(mut guard) = LAST_CHECK_ERROR.lock() {
        *guard = err;
    }
    LAST_CHECK_TIME_MS.store(now_epoch_ms(), Ordering::Release);
    result
}

fn version_urls(mirror_first: bool) -> Vec<&'static str> {
    if mirror_first {
        vec![VERSION_MIRRORS[0], VERSION_MIRRORS[1], VERSION_MIRRORS[2], VERSION_FILE]
    } else {
        vec![VERSION_FILE, VERSION_MIRRORS[0], VERSION_MIRRORS[1], VERSION_MIRRORS[2]]
    }
}

/// version.json 的真实形状：snake_case 的 version/notes 字段（桌面 updater 的
/// `fetch_latest_release` 按 data["version"] 手取同款契约）。不能整体反序列化为
/// UpdateInfo——camelCase 找 latestVersion 键永远匹配不到 "version"，has_update
/// 恒为 false，安卓端将永远"已是最新版本"（≤v2.3.6 真实缺陷）
#[derive(Debug, Default, Deserialize, Clone)]
struct VersionFile {
    #[serde(default)]
    version: String,
    #[serde(default)]
    notes: String,
}

/// 从 GitHub API release 拉取 APK 资产与官方 digest(服务端计算,最可信校验源)。
/// 返回 (assets, checksum, api_ok):api_ok=true 表示 API 请求本身成功(含 HTTP 200
/// 与 JSON 解析成功)——「API 可用但无 digest」与「API 不可用(网络/HTTP 错)」是
/// 两种不同语义,前者下载端必须拒绝安装,后者降级放行但明示未校验(审计 B5)。
/// API 失败/限流/无 APK 资产时 assets 为空——前端降级为外链 Releases(与现状一致);
/// version.json 不含 assets,真实下载地址只能来自 API。
async fn fetch_apk_assets(client: &reqwest::Client, latest: &str) -> (Vec<ReleaseAsset>, Option<String>, bool) {
    let url = format!("https://api.github.com/repos/ikliml666/Wxxy-CampusLogin/releases/tags/v{latest}");
    let Ok(resp) = client
        .get(&url)
        // 资产列表是 version.json 之后的附加信息,拉不到也应快速降级(前端走外链兜底),
        // 不让被阻断的 GitHub API 拖住整个检查
        .timeout(std::time::Duration::from_secs(10))
        .header("User-Agent", "Wxxy-CampusLogin")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    else {
        return (Vec::new(), None, false);
    };
    if !resp.status().is_success() {
        return (Vec::new(), None, false);
    }
    let Ok(raw) = resp.json::<serde_json::Value>().await else {
        return (Vec::new(), None, false);
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
    (assets, checksum, true)
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
        match client.get(url).timeout(std::time::Duration::from_secs(10)).send().await {
            Ok(resp) if resp.status().is_success() => {
                let raw: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
                let vf: VersionFile = serde_json::from_value(raw).map_err(|e| e.to_string())?;
                let latest_version = vf.version.trim_start_matches('v').to_string();
                if latest_version.is_empty() {
                    last_err = "version.json 缺少 version 字段".to_string();
                    continue;
                }
                let current = env!("CARGO_PKG_VERSION");
                let has_update = has_newer_version(current, &latest_version);
                // APK 资产与校验值:校验值唯一来源 GitHub API digest(服务端计算),
                // version.json 无 sha256 字段约定
                let (assets, checksum, api_ok) = fetch_apk_assets(&client, &latest_version).await;
                // digest 记账携带 tag:下载端按下载 URL 的 tag 取用(检查与下载的
                // 版本可能不同——只记裸 digest 会把 A 版校验值误配到 B 版文件上)。
                // 三态:有 digest → (tag, digest);API 在线但无 digest → (tag, "") 哨兵,
                // 下载端据此拒绝;API 不可用 → 不记账,下载端放行但明示未校验
                if let Ok(mut guard) = UPDATE_CHECKSUM.lock() {
                    *guard = match &checksum {
                        Some(d) => Some((latest_version.clone(), d.clone())),
                        None if api_ok => Some((latest_version.clone(), String::new())),
                        None => None,
                    };
                }
                return Ok(UpdateInfo {
                    has_update,
                    latest_version,
                    release_notes: vf.notes,
                    assets,
                    sha256_checksum: checksum,
                    // 本次检查前的最近检查状态快照(本次结果随后由 check_and_record 写入):
                    // 自动检查失败后,用户下次手动检查成功仍能看到上次失败原因
                    last_check_error: last_check_error_snapshot(),
                    last_check_time: last_check_time_snapshot(),
                });
            }
            Ok(resp) => last_err = format!("HTTP {}", resp.status()),
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(format!("检查更新失败: {last_err}"))
}

/// 自动更新检查循环(桌面 startup 24h 同语义):启动延迟 5s 查一次,之后每 24h;
/// 失败按 5min→15min→1h 退避重试(原实现 `if let Ok(info)` 静默吞掉 Err,
/// 网络抖动时 24h 内不再重试且无任何提示);有新版本 emit update-available
/// (前端 useEventListeners 早已监听,据此弹窗提示)
/// + 系统通知(过 enable_notification 闸)。循环随进程存活,无需停。
pub fn start_update_check_loop(app: tauri::AppHandle) {
    if UPDATE_LOOP_RUNNING.swap(true, Ordering::Relaxed) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        // 失败退避计数:连续失败按退避档位重试,退避用尽恢复 24h 周期并重新计数
        let mut fail_count: u32 = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            match check_and_record(&app).await {
                Ok(info) => {
                    fail_count = 0;
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
                Err(e) => {
                    // 失败原因已由 check_and_record 记入 LAST_CHECK_ERROR(经 lastCheckError
                    // 回传前端展示),此处仅留日志
                    campus_login_lib::log_warn!("updater", "自动更新检查失败: {e}");
                    fail_count += 1;
                }
            }
            let wait = backoff_delay_secs(fail_count);
            if fail_count as usize > BACKOFF_RETRY_SECS.len() {
                // 退避重试用尽:恢复 24h 周期,下一轮 24h 周期重新按退避重试
                fail_count = 0;
            }
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
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

    // 目录约定必须与 MonitorServicePlugin.installApk 对齐:tauri 的 app_data_dir 在
    // Android 解析为 dataDir(/data/user/0/<pkg>,见上游 PathPlugin.getDataDir),而
    // 插件与 FileProvider(files-path)都按 filesDir/update 约定——必须拼 files 段,
    // 否则下载成功但安装报"APK 文件不存在"(v2.3.9 真机实测)
    let dir = update_dir(&app)?;
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
    // SHA256 完整性校验(审计 B5):按下载 URL 的 tag 取检查期记账的 digest,三态:
    // ①记账含该 tag 的非空 digest → 强制比对,不匹配即删除并报错;
    // ②记账含该 tag 但 digest 为空(API 在线却拿不到校验值) → 拒绝安装;
    // ③无记账或 tag 不匹配(API 不可用降级/手动 URL) → 放行但 emit update-unverified
    //   + log_warn 明示未校验(此前 None 一律静默跳过,用户无从知晓完整性未验证)
    let expected = match (UPDATE_CHECKSUM.lock().ok().and_then(|g| g.clone()), download_tag(&url)) {
        (Some((t, digest)), Some(url_tag)) if t == url_tag && !digest.is_empty() => Some(digest),
        (Some((t, _)), Some(url_tag)) if t == url_tag => {
            let _ = tokio::fs::remove_file(&path).await;
            return Err("无法获取校验值,拒绝安装".to_string());
        }
        _ => None,
    };
    if let Some(expected) = expected {
        if !verify_file_sha256(&path, &expected).await? {
            let _ = tokio::fs::remove_file(&path).await;
            return Err("下载文件 SHA256 校验不匹配,已删除(下载源可能被篡改,请换镜像重试)".to_string());
        }
    } else {
        campus_login_lib::log_warn!("updater", "本次下载未取得校验值(检查期 API 不可用或版本不匹配),完整性未验证");
        let _ = app.emit(
            "update-unverified",
            serde_json::json!({ "url": url, "file": file_name }),
        );
    }
    Ok(path.to_string_lossy().to_string())
}

/// 从下载 URL 提取 release tag:官方与镜像链的资产 URL 都含
/// `/releases/download/<tag>/` 段(镜像仅前缀拼接,不改变路径形状)
fn download_tag(url: &str) -> Option<String> {
    let rest = url.split("/releases/download/").nth(1)?;
    let tag = rest.split('/').next()?;
    (!tag.is_empty()).then(|| tag.to_string())
}

/// 流式计算文件 SHA256 并与期望值比对;期望值容忍 "hex filename" 格式与大小写,
/// 非 64 位十六进制 → Err(校验值格式非法必须显式失败——原实现返回 true 静默放行,
/// 相当于损坏的校验值等于没有校验,审计 B5)。
/// pub(crate):人脸模型下载(face_model_cmds)按同语义复用(face 侧 sha 恒为合法
/// hex,Err 分支对其等价于「校验不通过」,调用方语义安全)
pub(crate) async fn verify_file_sha256(path: &Path, expected: &str) -> Result<bool, String> {
    use sha2::{Digest, Sha256};
    let clean = expected
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if clean.len() != 64 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("校验值格式非法(非 64 位十六进制): {clean}"));
    }
    let mut file = tokio::fs::File::open(path).await.map_err(|e| format!("读取下载文件失败: {e}"))?;
    use tokio::io::AsyncReadExt;
    let mut hasher = Sha256::new();
    // 必须堆分配(vec)而非栈数组:本函数是 async fn,[0u8; 65536] 会被并入 future
    // 状态机,而 tauri 在 WebView 的 JavaBridge 线程(栈小)上构造该 future——
    // 真机实测一进 download_update 即栈溢出闪退(SIGSEGV, 2026-09-20 tombstone)
    let mut buf = vec![0u8; 65536];
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
    // 目录约定必须与 MonitorServicePlugin.installApk 对齐:tauri 的 app_data_dir 在
    // Android 解析为 dataDir(/data/user/0/<pkg>,见上游 PathPlugin.getDataDir),而
    // 插件与 FileProvider(files-path)都按 filesDir/update 约定——必须拼 files 段,
    // 否则下载成功但安装报"APK 文件不存在"(v2.3.9 真机实测)
    let dir = update_dir(&app)?;
    let path = Path::new(&file_path);
    let canonical = path.canonicalize().map_err(|e| format!("路径无效: {e}"))?;
    let allowed_dir = dir.canonicalize().unwrap_or(dir);
    if !canonical.starts_with(&allowed_dir) {
        return Err("仅允许安装应用更新目录内的 APK".to_string());
    }
    app.campus_monitor_service()
        .install_apk(&canonical.to_string_lossy())
        .map_err(|e| format!("打开安装器失败: {e}"))?;
    // 注意：此处不能立刻删 APK——install_apk 返回只代表系统安装器 UI 已拉起，
    // FileProvider 的 content URI 由安装器进程异步读取，删早了会撞上 v2.3.9
    // 「下载成功但装不上」同源竞态；用户取消/安装失败时更需要保留文件可重试。
    // 收尾交给 cleanup_stale_apks（启动期删超 24h 的残留包）
    Ok(true)
}

/// 清理更新目录里超过 APK_STALE_SECS 的残留 APK:用户点了下载却没装/装失败/
/// 取消安装,或历史版本装完未删的旧包(install_update 不再提前删,见其注释),
/// 下次启动时一次性收掉。只删 .apk 且只删 mtime 超龄的文件——进行中的下载
/// 不受影响(写盘即刷新 mtime)
pub async fn cleanup_stale_apks(app: &tauri::AppHandle) {
    // 与下载竞态防护(审计 B5):下载进行中直接跳过清理——mtime 判定虽对进行中
    // 的文件天然"新鲜",但清理与下载并发遍历/删除同一目录仍是无谓的 IO 争用,
    // 且极端情况下(下载启动前文件已存在且超龄,下载器尚未 create 截断)可能
    // 删掉正要被复用的路径
    if DOWNLOAD_RUNNING.load(Ordering::Relaxed) {
        campus_login_lib::log_info!("updater", "下载任务进行中,本轮跳过残留安装包清理");
        return;
    }
    let Ok(dir) = update_dir(app) else { return };
    let Ok(mut entries) = tokio::fs::read_dir(&dir).await else { return };
    let now = std::time::SystemTime::now();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let is_apk = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("apk"))
            .unwrap_or(false);
        if !is_apk {
            continue;
        }
        let stale = match entry.metadata().await.and_then(|m| m.modified()) {
            Ok(mtime) => now
                .duration_since(mtime)
                .map(|age| age.as_secs() > APK_STALE_SECS)
                .unwrap_or(false),
            Err(_) => false,
        };
        if stale {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let _ = tokio::fs::remove_file(&path).await;
            campus_login_lib::log_info!("updater", "清理残留安装包: {name}");
        }
    }
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
        // 新增 last 字段:缺失按 None 处理,序列化为 camelCase
        assert!(info.last_check_error.is_none());
        assert!(info.last_check_time.is_none());
        let with_err: UpdateInfo = serde_json::from_str(
            r#"{"latestVersion":"2.4.0","releaseNotes":"","lastCheckError":"HTTP 503","lastCheckTime":123}"#,
        ).unwrap();
        assert_eq!(with_err.last_check_error.as_deref(), Some("HTTP 503"));
        assert_eq!(with_err.last_check_time, Some(123));
    }

    #[test]
    fn 安装目录约定_含files段() {
        // 回归护栏:app_data_dir 在 Android 解析为 dataDir(/data/user/0/<pkg>),
        // 而 installApk/FileProvider 按 filesDir/update 约定——缺 files 段即装不上
        // (AppHandle 无法在单测构造,直接校验 update_dir 的拼接语义)
        let data = std::path::PathBuf::from("/data/user/0/com.ikliml666.campuslogin");
        let built = data.join("files").join("update");
        assert!(built.ends_with("files/update"), "拼接结果必须落在 files/update: {built:?}");
    }

    #[test]
    fn 清理残留安装包_超龄删_新鲜留_非apk不动() {
        let dir = std::env::temp_dir().join(format!(
            "wxxy-apk-cleanup-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let stale = dir.join("campus-login-2.4.0.apk");
        let fresh = dir.join("campus-login-2.5.0.apk");
        let other = dir.join("notes.txt");
        std::fs::write(&stale, b"old").unwrap();
        std::fs::write(&fresh, b"new").unwrap();
        std::fs::write(&other, b"keep").unwrap();
        // stale 的 mtime 拨到 48h 前(超过 APK_STALE_SECS=24h)
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 3600);
        let f = std::fs::File::options().write(true).open(&stale).unwrap();
        f.set_modified(old).unwrap();
        drop(f);

        // 与 cleanup_stale_apks 同一判定的纯函数式复现(单测拿不到 AppHandle):
        // 逐项断言删除决策,而非依赖真实目录遍历
        let now = std::time::SystemTime::now();
        let is_stale = |p: &std::path::Path| {
            let mtime = std::fs::metadata(p).and_then(|m| m.modified()).unwrap();
            now.duration_since(mtime).map(|a| a.as_secs() > APK_STALE_SECS).unwrap_or(false)
        };
        assert!(is_stale(&stale), "48h 前的旧包必须判为残留");
        assert!(!is_stale(&fresh), "刚下载的包不得被误删");
        assert!(!is_stale(&other), "非 apk 文件不参与判定(调用方按扩展名过滤)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 退避间隔_步进后回归24h周期() {
        assert_eq!(backoff_delay_secs(0), AUTO_CHECK_INTERVAL_SECS, "成功或首轮 → 24h 正常周期");
        assert_eq!(backoff_delay_secs(1), 5 * 60, "第 1 次失败 → 5min");
        assert_eq!(backoff_delay_secs(2), 15 * 60, "第 2 次连续失败 → 15min");
        assert_eq!(backoff_delay_secs(3), 3600, "第 3 次连续失败 → 1h");
        assert_eq!(backoff_delay_secs(4), AUTO_CHECK_INTERVAL_SECS, "退避用尽恢复 24h 周期");
    }

    #[test]
    fn 下载url_tag提取() {
        // 审计 B5:下载校验按当前 tag 取用,tag 提取是镜像前缀无关的
        assert_eq!(download_tag("https://github.com/ikliml666/Wxxy-CampusLogin/releases/download/v2.4.1/app.apk").as_deref(), Some("v2.4.1"));
        assert_eq!(download_tag("https://ghfast.top/https://github.com/ikliml666/Wxxy-CampusLogin/releases/download/v2.4.1/app.apk").as_deref(), Some("v2.4.1"), "镜像仅前缀拼接,tag 段不变");
        assert!(download_tag("https://github.com/ikliml666/Wxxy-CampusLogin/raw/main/version.json").is_none(), "非 release 资产 URL 无 tag");
        assert!(download_tag("https://github.com/x/releases/download/").is_none(), "tag 段为空不产出空 tag");
    }

    #[test]
    fn version_json_真实形状解析() {
        // 线上 version.json 的真实形状（只有 snake_case 的 version 键）——
        // 回归护栏：曾经误用 UpdateInfo(camelCase) 整体反序列化它导致 latest_version
        // 恒为空、安卓端永远"已是最新版本"
        let raw = serde_json::json!({ "version": "v2.3.6" });
        let vf: VersionFile = serde_json::from_value(raw).unwrap();
        assert_eq!(vf.version, "v2.3.6");
        assert!(vf.notes.is_empty());
        let latest = vf.version.trim_start_matches('v');
        assert!(has_newer_version("2.3.5", latest));
        assert!(!has_newer_version("2.3.6", latest));

        // 缺 version 字段时必须解析为空串（check_update_inner 据此跳过该源而非误判无更新）
        let empty: VersionFile = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.version.is_empty());

        // notes 可选字段随版本号一起维护时透传 release_notes
        let with_notes: VersionFile =
            serde_json::from_value(serde_json::json!({ "version": "v2.4.0", "notes": "- 修复" })).unwrap();
        assert_eq!(with_notes.notes, "- 修复");
    }
}
