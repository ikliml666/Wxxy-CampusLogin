use tauri::{AppHandle, Manager};
use std::sync::Arc;
use crate::config::model::Config;
use crate::config::persist::{get_data_dir, get_accounts_dir, atomic_write, list_account_names};
use crate::config::validate::validate_config;
use super::crypto;
use crate::infra::state::{AppState, validate_account_name, AccountResult};
use crate::commands::config_cmd::save_config_to_disk;

#[allow(dead_code)]
pub fn list_accounts_inner(app_handle: &AppHandle) -> Vec<String> {
    list_account_names(app_handle)
}

#[allow(dead_code)]
pub fn load_account_config(app_handle: &AppHandle, account_name: &str) -> Result<Option<Config>, String> {
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
        match crypto::decrypt(&config.password) {
            Ok(decrypted) => { config.password = decrypted; }
            Err(e) => {
                crate::log_warn!("account", "账号密码解密失败: {}", e);
                return Err(format!("密码解密失败，请重新输入密码: {}", e));
            }
        }
    }

    let config = validate_config(config)?;

    Ok(Some(config))
}

#[allow(dead_code)]
pub fn switch_account_inner(app_handle: &AppHandle, account_name: &str) -> Result<AccountResult, String> {
    let safe_name = match validate_account_name(account_name) {
        Ok(n) => n,
        Err(e) => return Ok(AccountResult::err(&e)),
    };

    let account_config = load_account_config(app_handle, &safe_name)?;

    let config = match account_config {
        Some(c) => c,
        None => return Ok(AccountResult::err("账号不存在")),
    };

    let state = app_handle.state::<AppState>();
    let mut merged = state.config.load().as_ref().clone();
    merged.user = config.user.clone();
    merged.password = config.password.clone();
    merged.operator = config.operator.clone();
    merged.adapter1 = config.adapter1.clone();
    merged.adapter2 = config.adapter2.clone();
    merged.dual_adapter = config.dual_adapter;
    merged.active_account = account_name.to_string();

    save_config_to_disk(app_handle, &merged)?;

    state.config.store(Arc::new(merged));

    let display_config = state.config.load().masked_for_display();
    Ok(AccountResult::ok(display_config))
}

#[allow(dead_code)]
pub fn save_account_inner(app_handle: &AppHandle, account_name: &str) -> Result<AccountResult, String> {
    let safe_name = match validate_account_name(account_name) {
        Ok(n) => n,
        Err(e) => return Ok(AccountResult::err(&e)),
    };

    let state = app_handle.state::<AppState>();
    let config = state.config.load_full();

    if !config.active_account.is_empty() && config.active_account != safe_name {
        let prev_name = config.active_account.clone();
        let prev_user = config.user.clone();
        let prev_password = config.password.clone();
        let prev_operator = config.operator.clone();
        let prev_adapter1 = config.adapter1.clone();
        let prev_adapter2 = config.adapter2.clone();
        let prev_dual_adapter = config.dual_adapter;

        let save_prev_result: Result<(), String> = (|| {
            let accounts_dir = {
                let data_dir = get_data_dir(app_handle);
                get_accounts_dir(&data_dir)
            };
            std::fs::create_dir_all(&accounts_dir).map_err(|e| format!("创建账号目录失败: {}", e))?;
            let account_path = accounts_dir.join(format!("{}.json", prev_name));

            let mut save_prev = if account_path.exists() {
                match std::fs::read_to_string(&account_path) {
                    Ok(content) => {
                        let mut existing = serde_json::from_str::<Config>(&content)
                            .map_err(|e| format!("账号配置文件解析失败(可能已损坏): {}", e))?;
                        if !existing.password.is_empty() {
                            match crypto::decrypt(&existing.password) {
                                Ok(decrypted) => { existing.password = decrypted; }
                                Err(e) => {
                                    crate::log_error!("account", "旧账号密码解密失败: {}", e);
                                    return Err("旧账号密码解密失败，请重新输入密码".to_string());
                                }
                            }
                        }
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
                if let Err(e) = atomic_write(&account_path, &json) {
                    crate::log_error!("account", "保存旧账号文件失败: {}", e);
                }
            } else {
                crate::log_error!("account", "序列化旧账号配置失败");
            }
            Ok(())
        })();

        if let Err(e) = save_prev_result {
            crate::log_warn!("account", "保存旧账号配置任务失败: {}", e);
        }
    }

    let password_for_encrypt = config.password.clone();
    let mut account_data = (*config).clone();
    account_data.password = String::new();
    account_data.active_account = account_name.to_string();

    let accounts_dir = {
        let data_dir = get_data_dir(app_handle);
        get_accounts_dir(&data_dir)
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

    atomic_write(&account_path, &json)
        .map_err(|e| format!("写入账号配置失败: {}", e))?;

    {
        let mut cfg = state.config.load().as_ref().clone();
        cfg.active_account = account_name.to_string();
        state.config.store(Arc::new(cfg));
    }

    if let Err(e) = save_config_to_disk(app_handle, &state.config.load_full()) {
        crate::log_warn!("account", "切换账号后保存配置失败: {}", e);
    }

    let display_config = state.config.load().masked_for_display();

    Ok(AccountResult::ok_with_account(account_name.to_string(), display_config))
}

#[allow(dead_code)]
pub fn delete_account_inner(app_handle: &AppHandle, account_name: &str) -> Result<AccountResult, String> {
    let safe_name = validate_account_name(account_name)
        .map_err(|e| format!("删除失败: {}", e))?;

    let state = app_handle.state::<AppState>();
    let config = state.config.load();
    if config.active_account == safe_name {
        let mut cfg = config.as_ref().clone();
        cfg.active_account = String::new();
        state.config.store(Arc::new(cfg));
    }
    let data_dir = get_data_dir(app_handle);
    let accounts_dir = get_accounts_dir(&data_dir);
    let account_path = accounts_dir.join(format!("{}.json", safe_name));

    if account_path.exists() {
        std::fs::remove_file(&account_path)
            .map_err(|e| format!("删除账号失败: {}", e))?;
        Ok(AccountResult::ok_msg("账号已删除"))
    } else {
        Ok(AccountResult::err("账号不存在"))
    }
}

#[allow(dead_code)]
pub fn get_active_account_inner(state: &AppState) -> String {
    state.config.load().active_account.clone()
}
