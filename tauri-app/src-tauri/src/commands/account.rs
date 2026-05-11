use tauri::{AppHandle, Manager, State};
use std::sync::Arc;
use crate::config::{Config, get_data_dir, get_accounts_dir};
use crate::crypto_utils;
use super::state::{AppState, validate_account_name, validate_config};

#[tauri::command]
pub async fn list_accounts(app_handle: AppHandle) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data_dir = get_data_dir(&app_handle);
        let accounts_dir = get_accounts_dir(&data_dir);

        if !accounts_dir.exists() {
            return Ok(vec![]);
        }

        let mut accounts = Vec::new();
        let entries = std::fs::read_dir(&accounts_dir)
            .map_err(|e| format!("读取账号目录失败: {}", e))?;

        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) {
                    accounts.push(name.to_string());
                }
            }
        }

        accounts.sort();
        Ok(accounts)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn switch_account(account_name: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let safe_name = match validate_account_name(&account_name) {
        Ok(n) => n,
        Err(e) => return Ok(serde_json::json!({ "success": false, "message": e })),
    };

    let app_h = app_handle.clone();
    let account_config = tauri::async_runtime::spawn_blocking(move || {
        load_account_config_inner(&app_h, &safe_name)
    }).await.map_err(|e| e.to_string())??;

    let config = match account_config {
        Some(c) => c,
        None => return Ok(serde_json::json!({ "success": false, "message": "账号不存在" })),
    };

    let app_h2 = app_handle.clone();
    let config_clone = config.clone();
    tauri::async_runtime::spawn_blocking(move || super::config_cmd::save_config_to_disk(&app_h2, &config_clone)).await.map_err(|e| e.to_string())??;

    let mut active = config.clone();
    active.active_account = account_name.clone();
    state.config.store(Arc::new(active));

    let mut display_config = config.clone();
    if !display_config.password.is_empty() {
        display_config.password = "***".to_string();
    }
    Ok(serde_json::json!({ "success": true, "config": display_config }))
}

#[tauri::command]
pub async fn save_current_as_account(account_name: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let safe_name = match validate_account_name(&account_name) {
        Ok(n) => n,
        Err(e) => return Ok(serde_json::json!({ "success": false, "message": e })),
    };

    let config = state.config.load_full();

    if !config.active_account.is_empty() && config.active_account != safe_name {
        let prev_name = config.active_account.clone();
        let app_h_prev = app_handle.clone();
        let prev_user = config.user.clone();
        let prev_password = config.password.clone();
        let prev_operator = config.operator.clone();
        let prev_adapter1 = config.adapter1.clone();
        let prev_adapter2 = config.adapter2.clone();
        let prev_dual_adapter = config.dual_adapter;
        let prev_save_result = tauri::async_runtime::spawn_blocking(move || {
            let accounts_dir = {
                let data_dir = get_data_dir(&app_h_prev);
                get_accounts_dir(&data_dir)
            };
            let _ = std::fs::create_dir_all(&accounts_dir).map_err(|e| format!("创建账号目录失败: {}", e));
            let account_path = accounts_dir.join(format!("{}.json", prev_name));

            let mut save_prev = if account_path.exists() {
                match std::fs::read_to_string(&account_path) {
                    Ok(content) => {
                        let mut existing = serde_json::from_str::<Config>(&content).unwrap_or_default();
                        if !existing.password.is_empty() {
                            match crypto_utils::decrypt(&existing.password) {
                                Ok(decrypted) => { existing.password = decrypted; }
                                Err(_) => { existing.password = String::new(); }
                            }
                        }
                        existing
                    }
                    Err(_) => Config::default(),
                }
            } else {
                Config::default()
            };

            save_prev.user = prev_user;
            save_prev.password = prev_password;
            save_prev.operator = prev_operator;
            save_prev.adapter1 = prev_adapter1;
            save_prev.adapter2 = prev_adapter2;
            save_prev.dual_adapter = prev_dual_adapter;

            if !save_prev.password.is_empty() {
                if let Ok(encrypted) = crypto_utils::encrypt(&save_prev.password) {
                    save_prev.password = encrypted;
                }
            }

            if let Ok(json) = serde_json::to_string_pretty(&save_prev) {
                if let Err(e) = std::fs::write(&account_path, &json) {
                    crate::log_error!("account", "保存旧账号文件失败: {}", e);
                }
            } else {
                crate::log_error!("account", "序列化旧账号配置失败");
            }
        }).await;
        if let Err(e) = prev_save_result {
            crate::log_warn!("account", "保存旧账号配置任务失败: {}", e);
        }
    }

    let app_h = app_handle.clone();
    let password_for_encrypt = config.password.clone();
    let account_data = Config {
        user: config.user.clone(),
        password: password_for_encrypt.clone(),
        operator: config.operator.clone(),
        adapter1: config.adapter1.clone(),
        adapter2: config.adapter2.clone(),
        dual_adapter: config.dual_adapter,
        active_account: account_name.clone(),
        auto_login_on_start: config.auto_login_on_start,
        auto_exit_after_login: config.auto_exit_after_login,
        minimize_to_tray: config.minimize_to_tray,
        hidden_start: config.hidden_start,
        auto_launch: config.auto_launch,
        enable_background_check: config.enable_background_check,
        background_check_interval: config.background_check_interval,
        auto_login_on_preparation: config.auto_login_on_preparation,
        auto_exit_on_online: config.auto_exit_on_online,
        theme_mode: config.theme_mode.clone(),
        enable_notification: config.enable_notification,
        enable_latency_test: config.enable_latency_test,
        latency_test_interval: config.latency_test_interval,
        custom_theme_color: config.custom_theme_color.clone(),
        default_panel: config.default_panel.clone(),
        enable_network_quality: config.enable_network_quality,
        skip_ttfb_in_latency: config.skip_ttfb_in_latency,
        skip_content_in_latency: config.skip_content_in_latency,
        portal_url: config.portal_url.clone(),
        fixed_gateway: config.fixed_gateway.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let accounts_dir = {
            let data_dir = get_data_dir(&app_h);
            get_accounts_dir(&data_dir)
        };
        let _ = std::fs::create_dir_all(&accounts_dir).map_err(|e| format!("创建账号目录失败: {}", e));

        let account_path = accounts_dir.join(format!("{}.json", safe_name));

        let mut save_account = if account_path.exists() {
            match std::fs::read_to_string(&account_path) {
                Ok(content) => {
                    let mut existing = serde_json::from_str::<Config>(&content).unwrap_or_default();
                    existing.user = account_data.user.clone();
                    existing.operator = account_data.operator.clone();
                    existing.adapter1 = account_data.adapter1.clone();
                    existing.adapter2 = account_data.adapter2.clone();
                    existing.dual_adapter = account_data.dual_adapter;
                    existing.active_account = account_data.active_account.clone();
                    existing
                }
                Err(_) => account_data.clone(),
            }
        } else {
            account_data.clone()
        };

        if !password_for_encrypt.is_empty() {
            match crypto_utils::encrypt(&password_for_encrypt) {
                Ok(encrypted) => {
                    save_account.password = encrypted;
                }
                Err(e) => return Err(format!("加密密码失败: {}", e)),
            }
        } else {
            save_account.password = String::new();
        }

        let json = serde_json::to_string_pretty(&save_account)
            .map_err(|e| format!("序列化账号配置失败: {}", e))?;

        std::fs::write(&account_path, &json)
            .map_err(|e| format!("写入账号配置失败: {}", e))?;

        Ok::<(), String>(())
    }).await.map_err(|e| e.to_string())??;

    {
        let mut cfg = state.config.load().as_ref().clone();
        cfg.active_account = account_name.clone();
        state.config.store(Arc::new(cfg));
    }

    let app_h_save = app_handle.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let state = app_h_save.state::<AppState>();
        let config = state.config.load_full();
        if let Err(e) = super::config_cmd::save_config_to_disk(&app_h_save, &config) {
            crate::log_warn!("account", "切换账号后保存配置失败: {}", e);
        }
    }).await;

    let updated_config = state.config.load();
    let mut display_config = updated_config.as_ref().clone();
    if !display_config.password.is_empty() {
        display_config.password = "***".to_string();
    }

    Ok(serde_json::json!({ "success": true, "activeAccount": account_name, "config": display_config }))
}

#[tauri::command]
pub async fn delete_account(account_name: String, app_handle: AppHandle) -> Result<bool, String> {
    let safe_name = match validate_account_name(&account_name) {
        Ok(n) => n,
        Err(_) => return Ok(false),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let data_dir = get_data_dir(&app_handle);
        let accounts_dir = get_accounts_dir(&data_dir);
        let account_path = accounts_dir.join(format!("{}.json", safe_name));

        if account_path.exists() {
            std::fs::remove_file(&account_path)
                .map_err(|e| format!("删除账号失败: {}", e))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }).await.map_err(|e| e.to_string())?
}

fn load_account_config_inner(app_handle: &AppHandle, account_name: &str) -> Result<Option<Config>, String> {
    let data_dir = get_data_dir(app_handle);
    let accounts_dir = get_accounts_dir(&data_dir);
    let account_path = accounts_dir.join(format!("{}.json", account_name));

    if !account_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&account_path)
        .map_err(|e| format!("读取账号配置失败: {}", e))?;

    let mut config: Config = serde_json::from_str(&content)
        .map_err(|e| format!("解析账号配置失败: {}", e))?;

    if !config.password.is_empty() {
        match crypto_utils::decrypt(&config.password) {
            Ok(decrypted) => { config.password = decrypted; }
            Err(_) => {
                crate::log_warn!("account", "账号密码解密失败，密码字段将被清空");
                config.password = String::new();
            }
        }
    }

    let config = validate_config(config)?;

    Ok(Some(config))
}

#[tauri::command]
pub fn get_active_account(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.config.load().active_account.clone())
}
