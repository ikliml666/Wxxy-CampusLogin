use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use crate::config::model::Config;
use crate::config::persist;
use crate::config::validate::validate_config;
use crate::account::crypto;
use crate::infra::state::{AppState, CommandResult};

pub fn save_config_to_disk(app_handle: &AppHandle, config: &Config) -> Result<(), String> {
    let data_dir = persist::get_data_dir(app_handle);
    let config_path = persist::get_config_path(&data_dir);
    let json = serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {}", e))?;
    persist::atomic_write(&config_path, &json)?;
    let _ = app_handle.emit("config-changed", serde_json::json!({}));
    Ok(())
}

fn load_config_from_file(app_handle: &AppHandle) -> Result<Config, String> {
    let data_dir = persist::get_data_dir(app_handle);
    let config_path = persist::get_config_path(&data_dir);
    if !config_path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置文件失败: {}", e))?;
    let mut config: Config = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置文件失败: {}", e))?;

    if !config.password.is_empty() && config.password != crate::config::model::PASSWORD_MASK {
        match crypto::decrypt(&config.password) {
            Ok(decrypted) => config.password = decrypted,
            Err(_) => {}
        }
    }

    Ok(config)
}

pub fn load_config_from_disk_or_default(app_handle: &AppHandle) -> Config {
    match load_config_from_file(app_handle) {
        Ok(config) => {
            match validate_config(config) {
                Ok(validated) => validated,
                Err(e) => {
                    crate::log_warn!("config", "配置验证失败: {}", e);
                    Config::default()
                }
            }
        }
        Err(e) => {
            crate::log_warn!("config", "加载配置失败: {}，使用默认配置", e);
            Config::default()
        }
    }
}

#[tauri::command]
pub fn show_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<CommandResult, String> {
    let config = state.config.load();
    let mut cfg = config.as_ref().clone();
    cfg.password = crate::config::model::PASSWORD_MASK.to_string();
    Ok(CommandResult::ok_data(serde_json::to_value(cfg).map_err(|e| format!("序列化配置失败: {}", e))?))
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, app_handle: AppHandle, config_json: String) -> Result<CommandResult, String> {
    let config: Config = serde_json::from_str(&config_json)
        .map_err(|e| format!("解析配置失败: {}", e))?;

    let validated = match validate_config(config) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResult::err(&format!("配置验证失败: {}", e))),
    };

    let mut config = validated;
    if config.password == crate::config::model::PASSWORD_MASK {
        let current = state.config.load();
        config.password = current.password.clone();
    } else if !config.password.is_empty() {
        config.password = crypto::encrypt(&config.password)?;
    }

    state.config.store(Arc::new(config.clone()));
    save_config_to_disk(&app_handle, &config)?;

    Ok(CommandResult::ok())
}

#[allow(dead_code)]
#[tauri::command]
pub fn reset_config(state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let default_config = Config::default();
    state.config.store(Arc::new(default_config.clone()));
    save_config_to_disk(&app_handle, &default_config)?;
    Ok(CommandResult::ok())
}

#[allow(dead_code)]
#[tauri::command]
pub fn export_config(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let data_dir = persist::get_data_dir(&app_handle);
    let config_path = persist::get_config_path(&data_dir);
    if !config_path.exists() {
        return Ok(CommandResult::err("配置文件不存在"));
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置文件失败: {}", e))?;
    Ok(CommandResult::ok_data(serde_json::json!({ "content": content })))
}

#[allow(dead_code)]
#[tauri::command]
pub fn import_config(state: State<'_, AppState>, app_handle: AppHandle, config_json: String) -> Result<CommandResult, String> {
    let config: Config = serde_json::from_str(&config_json)
        .map_err(|e| format!("解析配置失败: {}", e))?;

    let validated = match validate_config(config) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResult::err(&format!("配置验证失败: {}", e))),
    };

    let mut config = validated;
    if !config.password.is_empty() && config.password != crate::config::model::PASSWORD_MASK {
        match crypto::decrypt(&config.password) {
            Ok(_) => {}
            Err(_) => {
                config.password = crypto::encrypt(&config.password)?;
            }
        }
    }

    state.config.store(Arc::new(config.clone()));
    save_config_to_disk(&app_handle, &config)?;
    Ok(CommandResult::ok())
}
