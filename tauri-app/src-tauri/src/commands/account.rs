use tauri::{AppHandle, State};
use crate::config::model::Config;
use crate::config::persist;
use crate::account::crypto;
use crate::infra::state::{AppState, AccountResult};

#[tauri::command]
pub async fn list_accounts(app_handle: AppHandle) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(persist::list_account_names(&app_handle))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn switch_account(account_name: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<AccountResult, String> {
    let safe_name = match crate::infra::state::validate_account_name(&account_name) {
        Ok(n) => n,
        Err(e) => return Ok(AccountResult::err(&e)),
    };
    let safe_name_log = safe_name.clone();

    let app_h = app_handle.clone();
    let account_config = tauri::async_runtime::spawn_blocking(move || {
        load_account_config_inner(&app_h, &safe_name)
    }).await.map_err(|e| e.to_string())??;

    let config = match account_config {
        Some(c) => c,
        None => return Ok(AccountResult::err("账号不存在")),
    };

    let merged = state.update_config(|c| {
        c.user = config.user.clone();
        c.password = config.password.clone();
        c.operator = config.operator.clone();
        c.adapter1 = config.adapter1.clone();
        c.adapter2 = config.adapter2.clone();
        c.dual_adapter = config.dual_adapter;
        c.active_account = account_name.clone();
    });

    let app_h2 = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || super::config_cmd::save_config_to_disk_encrypted(&app_h2, &merged)).await.map_err(|e| e.to_string())??;

    crate::log_info!("account", "切换账号: {} (用户: {})", safe_name_log, config.user);

    let display_config = state.config.load().masked_for_display();
    Ok(AccountResult::ok(display_config))
}

#[tauri::command]
pub async fn save_current_as_account(account_name: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<AccountResult, String> {
    let safe_name = match crate::infra::state::validate_account_name(&account_name) {
        Ok(n) => n,
        Err(e) => return Ok(AccountResult::err(&e)),
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
        let prev_save_result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
            let accounts_dir = {
                let data_dir = persist::get_data_dir(&app_h_prev);
                persist::get_accounts_dir(&data_dir)
            };
            std::fs::create_dir_all(&accounts_dir).map_err(|e| format!("创建账号目录失败: {}", e))?;
            let account_path = accounts_dir.join(format!("{}.json", prev_name));

            let mut save_prev = if account_path.exists() {
                match std::fs::read_to_string(&account_path) {
                    Ok(content) => {
                        let mut existing = serde_json::from_str::<Config>(&content)
                            .map_err(|e| format!("账号配置文件解析失败(可能已损坏): {}", e))?;
                        // 保留旧账号的非登录字段（主题等），登录字段一律用当前配置覆盖
                        existing.password = String::new();
                        existing
                    }
                    Err(e) => {
                        crate::log_error!("account", "读取旧账号文件失败: {}", e);
                        return Err("读取旧账号配置失败".to_string());
                    }
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
                match crypto::encrypt(&save_prev.password) {
                    Ok(encrypted) => { save_prev.password = encrypted; }
                    Err(e) => {
                        crate::log_error!("account", "密码加密失败: {}", e);
                        return Err(format!("密码加密失败: {}", e));
                    }
                }
            }

            if let Ok(json) = serde_json::to_string_pretty(&save_prev) {
                if let Err(e) = persist::atomic_write(&account_path, &json) {
                    crate::log_error!("account", "保存旧账号文件失败: {}", e);
                }
            } else {
                crate::log_error!("account", "序列化旧账号配置失败");
            }
            Ok(())
        }).await;
        if let Err(e) = prev_save_result {
            crate::log_warn!("account", "保存旧账号配置任务失败: {}", e);
        }
    }

    let app_h = app_handle.clone();
    let password_for_encrypt = config.password.clone();
    let mut account_data = (*config).clone();
    account_data.password = String::new();
    account_data.active_account = account_name.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let accounts_dir = {
            let data_dir = persist::get_data_dir(&app_h);
            persist::get_accounts_dir(&data_dir)
        };
        std::fs::create_dir_all(&accounts_dir).map_err(|e| format!("创建账号目录失败: {}", e))?;

        let account_path = accounts_dir.join(format!("{}.json", safe_name));

        let mut save_account = if account_path.exists() {
            match std::fs::read_to_string(&account_path) {
                Ok(content) => {
                    let mut existing = serde_json::from_str::<Config>(&content)
                        .map_err(|e| format!("账号配置文件解析失败(可能已损坏): {}", e))?;
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
            match crypto::encrypt(&password_for_encrypt) {
                Ok(encrypted) => { save_account.password = encrypted; }
                Err(e) => {
                    crate::log_error!("account", "密码加密失败: {}", e);
                    return Err(format!("密码加密失败: {}", e));
                }
            }
        }

        let json = serde_json::to_string_pretty(&save_account)
            .map_err(|e| format!("序列化账号配置失败: {}", e))?;
        persist::atomic_write(&account_path, &json)?;

        Ok::<(), String>(())
    }).await.map_err(|e| e.to_string())??;

    state.update_config(|c| {
        c.active_account = account_name.clone();
    });

    let display_config = state.config.load().masked_for_display();
    crate::log_info!("account", "保存账号: {}", account_name);
    Ok(AccountResult::ok_with_account(account_name, display_config))
}

#[tauri::command]
pub async fn delete_account(account_name: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<AccountResult, String> {
    let account_name = crate::infra::state::validate_account_name(&account_name)
        .map_err(|e| e.to_string())?;
    let app_h = app_handle.clone();
    let name = account_name.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let accounts_dir = {
            let data_dir = persist::get_data_dir(&app_h);
            persist::get_accounts_dir(&data_dir)
        };
        let account_path = accounts_dir.join(format!("{}.json", name));
        if !account_path.exists() {
            return Err("账号不存在".to_string());
        }
        std::fs::remove_file(&account_path).map_err(|e| format!("删除账号失败: {}", e))?;
        crate::log_info!("account", "删除账号: {}", name);
        Ok(())
    }).await.map_err(|e| e.to_string())??;

    let current_config = state.config.load();
    if current_config.active_account == account_name {
        state.update_config(|c| {
            c.active_account = String::new();
        });
    }

    let display_config = state.config.load().masked_for_display();
    Ok(AccountResult::ok(display_config))
}

#[tauri::command]
pub fn get_active_account(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.load();
    Ok(config.active_account.clone())
}

fn load_account_config_inner(app_handle: &AppHandle, account_name: &str) -> Result<Option<Config>, String> {
    let data_dir = persist::get_data_dir(app_handle);
    let accounts_dir = persist::get_accounts_dir(&data_dir);
    let account_path = accounts_dir.join(format!("{}.json", account_name));

    if !account_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&account_path)
        .map_err(|e| format!("读取账号配置失败: {}", e))?;
    let mut config: Config = serde_json::from_str(&content)
        .map_err(|e| format!("解析账号配置失败: {}", e))?;

    if !config.password.is_empty() {
        match crypto::decrypt(&config.password) {
            Ok(decrypted) => config.password = decrypted,
            Err(e) => {
                crate::log_error!("account", "账号密码解密失败: {}", e);
                return Err("账号密码解密失败".to_string());
            }
        }
    }

    Ok(Some(config))
}
