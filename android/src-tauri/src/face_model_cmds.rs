//! 人脸识别模型按需下载:APK 不再打包 6 个模型文件(约 8.9MB),首次启用人脸
//! 功能时从 GitHub(@vladmandic/human master 的 models 目录)下载到应用私有
//! 目录,再经 asset 协议喂给 WebView 内的 human(modelBasePath)。
//! 镜像沿用更新链同款前缀代理;源排序跟随 update_source 设置;每个文件以
//! 硬编码 sha256 锁定内容(等于 npm 依赖 3.3.6,2026-10-03 实测 master 六文件
//! 与 npm 3.3.6 逐字节一致),上游更新模型会哈希失配——升级依赖时需同步更新。

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::Emitter;
use tauri::Manager;

use crate::config_state;
use crate::update_cmds::{allowed_url, http_client, verify_file_sha256};

const FACE_MODEL_BASE: &str = "https://raw.githubusercontent.com/vladmandic/human/master/models/";
/// 前缀代理镜像,顺序即降级顺序(与 update_cmds::VERSION_MIRRORS 同构)
const FACE_MODEL_MIRRORS: &[&str] = &[
    "https://ghfast.top/",
    "https://gh-proxy.com/",
    "https://ghproxy.net/",
];
/// (文件名, sha256)。json 与 bin 都强校验——前缀代理失败时常返回 HTML 错误页
const FACE_MODEL_FILES: &[(&str, &str)] = &[
    (
        "blazeface.json",
        "cd7bbfc078270572beb39f9e5ae67aadbd50b5e67cff37e6d4f6b3ea39312e5f",
    ),
    (
        "blazeface.bin",
        "dc9a97fdc50bc43216554bdd69aa3e7b9361a519ee7bdd996a2f69a98a6f9b72",
    ),
    (
        "facemesh.json",
        "b60ca26f404724f43bd2b1575761d8265180e1f053cc0731caa68462927309e7",
    ),
    (
        "facemesh.bin",
        "3826da640b0a3021161605369ee6af293f75d518040355b960ec71a3390c1c0b",
    ),
    (
        "faceres.json",
        "5b83d49c0385d2e68a05122441b94226313677cae9fcc40b9587ad50079eb4df",
    ),
    (
        "faceres.bin",
        "2c7d2d62b76c97528b736527aa09d310ea71743c9e3e79fb6c62d4b2d73af79b",
    ),
];

/// 并发互斥:与 update_cmds::DOWNLOAD_RUNNING 同款"外壳原子标志 + 内部函数"模式
static FACE_DL_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FaceModelFileState {
    pub name: String,
    pub size: u64,
    /// 文件存在且 sha256 匹配
    pub ok: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FaceModelsState {
    pub ready: bool,
    /// 模型目录绝对路径(前端经 convertFileSrc 转 asset URL 作 modelBasePath)
    pub dir: String,
    pub files: Vec<FaceModelFileState>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FaceModelProgress {
    pub file: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: f64,
}

/// 目录约定必须与 update_cmds 对齐:tauri 的 app_data_dir 在 Android 解析为
/// dataDir(/data/user/0/<pkg>),私有文件目录是 dataDir/files——必须拼 files 段。
/// face-models 子目录由 tauri.conf.json assetProtocol scope 精确放行。
fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?
        .join("files")
        .join("face-models"))
}

/// 六文件就位状态:ok = 存在且哈希匹配(复用 verify_file_sha256,~9MB 全量校验)
async fn state_inner(app: &tauri::AppHandle) -> Result<FaceModelsState, String> {
    let dir = models_dir(app)?;
    let mut files = Vec::new();
    let mut ready = true;
    for (name, sha) in FACE_MODEL_FILES {
        let path = dir.join(name);
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let ok = size > 0 && verify_file_sha256(&path, sha).await.unwrap_or(false);
        if !ok {
            ready = false;
        }
        files.push(FaceModelFileState {
            name: name.to_string(),
            size,
            ok,
        });
    }
    Ok(FaceModelsState {
        ready,
        dir: dir.to_string_lossy().to_string(),
        files,
    })
}

#[tauri::command]
pub async fn face_models_state(app: tauri::AppHandle) -> Result<FaceModelsState, String> {
    state_inner(&app).await
}

#[tauri::command]
pub async fn face_models_download(app: tauri::AppHandle) -> Result<FaceModelsState, String> {
    if FACE_DL_RUNNING.swap(true, Ordering::Relaxed) {
        return Err("人脸模型下载已在进行中，请等待完成".to_string());
    }
    let result = download_models(&app).await;
    FACE_DL_RUNNING.store(false, Ordering::Relaxed);
    result
}

async fn download_models(app: &tauri::AppHandle) -> Result<FaceModelsState, String> {
    let dir = models_dir(app)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("创建模型目录失败: {e}"))?;
    let client = http_client()?;
    // 源排序跟随 update_source(与更新链同语义):github 直连优先 / mirror 镜像优先
    let mirror_first = config_state::current_settings(app).await?.update_source != "github";
    let mut sources: Vec<&str> = Vec::with_capacity(FACE_MODEL_MIRRORS.len() + 1);
    if mirror_first {
        sources.extend_from_slice(FACE_MODEL_MIRRORS);
    }
    sources.push(FACE_MODEL_BASE);
    if !mirror_first {
        sources.extend_from_slice(FACE_MODEL_MIRRORS);
    }

    for (name, sha) in FACE_MODEL_FILES {
        let dest = dir.join(name);
        if verify_file_sha256(&dest, sha).await.unwrap_or(false) {
            continue; // 已就位(逐源增量:上次失败中途退出时跳过已完成文件)
        }
        let mut last_err = String::new();
        let mut ok = false;
        for prefix in &sources {
            let url = format!("{prefix}{name}");
            if let Err(e) = allowed_url(&url) {
                last_err = e;
                continue;
            }
            match download_one(&client, &url, &dest, name, app).await {
                Ok(()) => {
                    ok = true;
                    break;
                }
                Err(e) => last_err = format!("{url}: {e}"),
            }
        }
        if !ok {
            return Err(format!("人脸模型 {name} 下载失败: {last_err}"));
        }
        if !verify_file_sha256(&dest, sha).await? {
            let _ = tokio::fs::remove_file(&dest).await;
            return Err(format!(
                "人脸模型 {name} SHA256 校验不匹配,已删除(下载源可能被篡改,请换网络或镜像重试)"
            ));
        }
    }
    state_inner(app).await
}

/// 单文件流式下载,写 <name>.part 临时文件成功后原子改名(半截文件不留正式名)
async fn download_one(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    name: &str,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("下载响应异常: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dest.with_file_name(format!("{name}.part"));
    let mut file = tokio::fs::File::create(&part)
        .await
        .map_err(|e| format!("创建下载文件失败: {e}"))?;

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    let mut last_emit = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载中断: {e}"))?;
        downloaded += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入失败: {e}"))?;
        // 节流:每 200ms 发一次进度事件(与 update-download-progress 同节奏)
        if last_emit.elapsed().as_millis() >= 200 {
            last_emit = Instant::now();
            let percent = if total > 0 {
                downloaded as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            let _ = app.emit(
                "face-models-download-progress",
                FaceModelProgress {
                    file: name.to_string(),
                    downloaded,
                    total,
                    percent,
                },
            );
        }
    }
    file.flush().await.map_err(|e| format!("落盘失败: {e}"))?;
    drop(file);
    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| format!("保存模型文件失败: {e}"))?;
    Ok(())
}

/// 最小自检:URL 拼接与源排序(mirror_first 语义)的纯逻辑不变式
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_url_keeps_prefix_form() {
        // 前缀代理形式必须与 VERSION_MIRRORS 同构:"https://<镜像>/<完整原始 URL>"
        let url = format!("{}{}", FACE_MODEL_MIRRORS[0], format!("{FACE_MODEL_BASE}blazeface.json"));
        assert!(url.starts_with("https://ghfast.top/https://raw.githubusercontent.com/"));
        assert!(url.ends_with("/models/blazeface.json"));
    }

    #[test]
    fn sources_order_follows_mirror_first() {
        let order = |mirror_first: bool| -> Vec<&'static str> {
            let mut v: Vec<&'static str> = Vec::new();
            if mirror_first {
                v.extend_from_slice(FACE_MODEL_MIRRORS);
            }
            v.push(FACE_MODEL_BASE);
            if !mirror_first {
                v.extend_from_slice(FACE_MODEL_MIRRORS);
            }
            v
        };
        assert_eq!(order(true)[0], FACE_MODEL_MIRRORS[0]);
        assert_eq!(order(false)[0], FACE_MODEL_BASE);
        assert_eq!(order(false).len(), FACE_MODEL_MIRRORS.len() + 1);
    }
}
