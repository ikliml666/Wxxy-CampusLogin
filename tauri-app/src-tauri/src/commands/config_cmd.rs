use tauri::{AppHandle, State};
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
    crate::app::window::show_and_focus_main(&app_handle);
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
