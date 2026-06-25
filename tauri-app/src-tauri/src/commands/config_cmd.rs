use tauri::{AppHandle, Manager, State};
use crate::infra::command_context::AppHandleExt;
use crate::config::model::Config;
use crate::config::persist;
use crate::config::validate::{validate_config, validate_config_lenient};
use crate::account::crypto;
use crate::infra::state::{AppState, CommandResult};

pub fn save_config_to_disk(app_handle: &AppHandle, config: &Config) -> Result<(), String> {
    let data_dir = persist::get_data_dir(app_handle);
    let config_path = persist::get_config_path(&data_dir);
    let json = serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {e}"))?;
    persist::atomic_write(&config_path, &json)?;
    let _ = app_handle.notify_config_changed_empty();
    Ok(())
}

pub fn save_config_to_disk_encrypted(app_handle: &AppHandle, config: &Config) -> Result<(), String> {
    let mut disk_config = config.clone();
    if !disk_config.password.is_empty() && disk_config.password != crate::config::model::PASSWORD_MASK {
        disk_config.password = crypto::encrypt(&disk_config.password)?;
    }
    save_config_to_disk(app_handle, &disk_config)
}

fn load_config_from_file(app_handle: &AppHandle) -> Result<Config, String> {
    let data_dir = persist::get_data_dir(app_handle);
    let config_path = persist::get_config_path(&data_dir);
    if !config_path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置文件失败: {e}"))?;
    let mut config: Config = serde_json::from_str(&content)
        .map_err(|e| format!("解析配置文件失败: {e}"))?;

    if !config.password.is_empty() && config.password != crate::config::model::PASSWORD_MASK {
        match crypto::decrypt(&config.password) {
            Ok(decrypted) => config.password = decrypted,
            Err(e) => {
                // 解密失败时仅清空密码，保留其他配置，避免全量配置丢失
                crate::log_warn!("config", "密码解密失败，清除密码保留其他配置: {}", e);
                config.password = String::new();
            }
        }
    }

    Ok(config)
}

pub fn load_config_from_disk_or_default(app_handle: &AppHandle) -> Config {
    match load_config_from_file(app_handle) {
        Ok(config) => validate_config_lenient(config),
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
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.config.load();
    let mut cfg = config.as_ref().clone();
    cfg.password = crate::config::model::PASSWORD_MASK.to_string();
    Ok(cfg)
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, app_handle: AppHandle, config: Config) -> Result<CommandResult, String> {
    let validated = match validate_config(config) {
        Ok(c) => c,
        Err(e) => {
            crate::log_warn!("config", "配置验证失败: {}", e);
            return Ok(CommandResult::err(&format!("配置验证失败: {e}")));
        }
    };

    let mut config = validated;
    // 空密码或 mask 占位符：保留当前密码，避免前端未传密码时旧密码被覆盖
    if config.password.is_empty() || config.password == crate::config::model::PASSWORD_MASK {
        let current = state.config.load();
        config.password = current.password.clone();
    }

    state.config.store(config.clone());
    save_config_to_disk_encrypted(&app_handle, &config)?;
    crate::log_info!("config", "配置保存成功, 用户: {}", config.user);

    Ok(CommandResult::ok())
}

#[tauri::command]
pub fn reset_config(state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    crate::log_info!("config", "重置配置为默认值");
    let default_config = Config::default();
    state.config.store(default_config.clone());
    save_config_to_disk(&app_handle, &default_config)?;
    Ok(CommandResult::ok())
}

#[tauri::command]
pub fn export_config(_state: State<'_, AppState>, app_handle: AppHandle) -> Result<CommandResult, String> {
    let data_dir = persist::get_data_dir(&app_handle);
    let config_path = persist::get_config_path(&data_dir);
    if !config_path.exists() {
        return Ok(CommandResult::err("配置文件不存在"));
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取配置文件失败: {e}"))?;
    // 脱敏：将密码字段替换为占位符，防止导出文件包含可还原的加密密码
    let content = match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(mut json) => {
            if let Some(obj) = json.as_object_mut() {
                obj.insert("password".to_string(), serde_json::json!(crate::config::model::PASSWORD_MASK));
            }
            serde_json::to_string(&json).unwrap_or_else(|_| {
                crate::log_warn!("config", "脱敏后序列化失败，返回空配置");
                "{}".to_string()
            })
        }
        Err(_) => {
            crate::log_warn!("config", "配置文件JSON格式异常，拒绝导出");
            return Ok(CommandResult::err("配置文件格式异常，无法脱敏导出"));
        }
    };
    crate::log_info!("config", "导出配置成功");
    Ok(CommandResult::ok_data(serde_json::json!({ "content": content })))
}

#[tauri::command]
pub fn import_config(state: State<'_, AppState>, app_handle: AppHandle, config_json: String) -> Result<CommandResult, String> {
    let config: Config = serde_json::from_str(&config_json)
        .map_err(|e| format!("解析配置失败: {e}"))?;

    let validated = match validate_config(config) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResult::err(&format!("配置验证失败: {e}"))),
    };

    let mut config = validated;
    // mask 占位符或空密码：保留当前密码，避免密码被清空
    if config.password.is_empty() || config.password == crate::config::model::PASSWORD_MASK {
        let current = state.config.load();
        config.password = current.password.clone();
    } else if !config.password.is_empty() {
        match crypto::decrypt(&config.password) {
            Ok(decrypted) => {
                config.password = decrypted;
            }
            Err(e) => {
                crate::log_warn!("config", "导入配置密码解密失败: {}", e);
                return Ok(CommandResult::err(&format!("密码解密失败，请重新输入密码: {e}")));
            }
        }
    }

    state.config.store(config.clone());
    save_config_to_disk_encrypted(&app_handle, &config)?;
    crate::log_info!("config", "导入配置成功, 用户: {}", config.user);
    Ok(CommandResult::ok())
}
