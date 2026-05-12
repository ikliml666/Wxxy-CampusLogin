use tauri::{AppHandle, Manager, State};
use std::sync::Arc;
use crate::config::{Config, get_data_dir, get_config_path};
use crate::crypto_utils;
use super::state::{AppState, validate_config};

fn load_config_from_disk(app_handle: &AppHandle) -> Result<Config, String> {
    let data_dir = get_data_dir(app_handle);
    let config_path = get_config_path(&data_dir);

    if !config_path.exists() {
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {}", e))?;
        let default = Config::default();
        std::fs::write(&config_path, serde_json::to_string_pretty(&default).unwrap_or_default())
            .map_err(|e| format!("写入默认配置失败: {}", e))?;
        return Ok(default);
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置失败: {}", e))?;

    let mut config: Config = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置失败: {}", e))?;

    if !config.password.is_empty() {
        match crypto_utils::decrypt(&config.password) {
            Ok(decrypted) => { config.password = decrypted; }
            Err(e) => {
                crate::log_warn!("config", "密码解密失败: {}, 密码字段将被清空", e);
                config.password = String::new();
            }
        }
    }

    let config = validate_config(config)?;

    Ok(config)
}

pub fn load_config_from_disk_or_default(app_handle: &AppHandle) -> Config {
    load_config_from_disk(app_handle).unwrap_or_default()
}

pub fn save_config_to_disk(app_handle: &AppHandle, config: &Config) -> Result<(), String> {
    crate::log_debug!("config", "保存配置到磁盘");
    let data_dir = get_data_dir(app_handle);
    let config_path = get_config_path(&data_dir);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {}", e))?;

    let is_new_file = !config_path.exists();

    let mut save_config = config.clone();
    if !save_config.password.is_empty() {
        let encrypted = crypto_utils::encrypt(&save_config.password)?;
        save_config.password = encrypted;
    }

    let json = serde_json::to_string_pretty(&save_config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;

    let tmp_path = config_path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("写入临时配置文件失败: {}", e))?;
    std::fs::rename(&tmp_path, &config_path)
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            format!("重命名配置文件失败: {}", e)
        })?;

    if is_new_file {
        restrict_file_permissions(&config_path);
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn restrict_file_permissions(path: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    let path_str = match path.to_str() {
        Some(s) => s,
        None => {
            crate::log_warn!("config", "文件路径包含非UTF-8字符，跳过权限设置");
            return;
        }
    };

    let username = match std::env::var("USERNAME") {
        Ok(u) => u,
        Err(_) => {
            crate::log_warn!("config", "无法获取当前用户名，跳过权限设置");
            return;
        }
    };

    let grant_arg = format!("{}:(R,W,D)", username);

    let output = match std::process::Command::new("icacls")
        .arg(path_str)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(&grant_arg)
        .creation_flags(0x08000000)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            crate::log_warn!("config", "执行icacls失败: {}", e);
            return;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        crate::log_warn!("config", "设置文件权限失败: {}", stderr.trim());
    }
}

#[cfg(not(target_os = "windows"))]
fn restrict_file_permissions(_path: &std::path::Path) {}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.config.load();
    let mut display_config = config.as_ref().clone();
    if !display_config.password.is_empty() {
        display_config.password = "***".to_string();
    }
    Ok(display_config)
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, app_handle: AppHandle, mut config: Config) -> Result<serde_json::Value, String> {
    let (old_adapter1, old_latency_interval, old_latency_enabled, old_enable_network_quality) = {
        let guard = state.config.load_full();
        if config.user.is_empty() && !guard.user.is_empty() {
            config.user = guard.user.clone();
        }
        if config.password == "***" {
            config.password = guard.password.clone();
        }
        let old_vals = (guard.adapter1.clone(), guard.latency_test_interval, guard.enable_latency_test, guard.enable_network_quality);
        drop(guard);

        let config = validate_config(config)?;
        state.config.store(Arc::new(config));
        let config_ref = state.config.load();
        crate::network::update_portal_url(&config_ref.portal_url);
        old_vals
    };

    let app_h = app_handle.clone();
    let config_clone = state.config.load_full();
    tauri::async_runtime::spawn_blocking(move || save_config_to_disk(&app_h, &config_clone)).await.map_err(|e| e.to_string())??;

    let config_ref = state.config.load();
    if config_ref.adapter1 != old_adapter1 {
        crate::network::clear_adapter_cache();
    }

    if !config_ref.enable_network_quality && old_enable_network_quality {
        let s = app_handle.state::<AppState>();
        s.tasks.latency_cancel.load().cancel();
        s.tasks.latency_running.force_release();
    } else if !config_ref.enable_latency_test && old_latency_enabled {
        let s = app_handle.state::<AppState>();
        s.tasks.latency_cancel.load().cancel();
        s.tasks.latency_running.force_release();
    } else if config_ref.enable_latency_test && config_ref.enable_network_quality && (config_ref.latency_test_interval != old_latency_interval || (!old_latency_enabled && config_ref.enable_latency_test)) {
        let s = app_handle.state::<AppState>();
        s.tasks.latency_cancel.load().cancel();
        s.tasks.latency_running.force_release();
        let interval = if config_ref.latency_test_interval < 10000 { 30000 } else { config_ref.latency_test_interval };
        super::latency::spawn_latency_test_loop(&app_handle, interval);
    }

    let mut display_config = state.config.load().as_ref().clone();
    if !display_config.password.is_empty() {
        display_config.password = "***".to_string();
    }

    Ok(serde_json::json!({
        "success": true,
        "config": display_config,
    }))
}

#[tauri::command]
pub fn show_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();       // [忽略错误] 窗口可能已关闭或不可用
        let _ = window.set_focus();  // [忽略错误] 窗口可能已关闭或不可用
    }
    Ok(())
}
