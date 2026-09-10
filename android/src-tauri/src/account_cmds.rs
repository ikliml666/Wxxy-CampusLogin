//! 多账号命令面:与桌面 commands/account.rs 同构(一账号一 JSON、密码密文落盘、
//! 切换合并登录字段)。账号文件与主配置同格式(EncodedSettings),仅目录不同。

use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

use crate::config_state::{self, CryptoBridge, Settings};

lazy_static! {
    static ref ACCOUNT_NAME_RE: Regex =
        Regex::new(r"^[a-zA-Z0-9_\u{4e00}-\u{9fff}-]+$").expect("ACCOUNT_NAME_RE compilation failed");
    /// 配置 IO 串行锁:防 save_config / switch / delete 并发读改写竞态
    /// (tokio 异步锁:guard 需跨 await 持有,覆盖 load→merge→save 全序列)
    static ref CONFIG_IO_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

/// 主配置/账号文件读改写互斥(跨命令共用)
pub async fn config_io_lock() -> tokio::sync::MutexGuard<'static, ()> {
    CONFIG_IO_LOCK.lock().await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

impl AccountResult {
    fn ok(config: serde_json::Value) -> Self {
        Self { success: true, message: None, active_account: None, config: Some(config) }
    }
    fn ok_with_account(account: String, config: serde_json::Value) -> Self {
        Self { success: true, message: None, active_account: Some(account), config: Some(config) }
    }
    fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), active_account: None, config: None }
    }
}

/// 账号名消毒:1-32 字符,仅字母/数字/下划线/中文/连字符(桌面同款正则,防路径穿越)
pub fn validate_account_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.chars().count() > 32 {
        return Err("账号名称长度需在1-32之间".to_string());
    }
    if !ACCOUNT_NAME_RE.is_match(name) {
        return Err("账号名称仅允许字母、数字、下划线、中文和连字符".to_string());
    }
    Ok(name.to_string())
}

pub fn accounts_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?
        .join("accounts"))
}

pub fn list_account_names_sync(dir: &std::path::Path) -> Vec<String> {
    let mut accounts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) {
                    if name.starts_with('.') || name.is_empty() {
                        continue;
                    }
                    accounts.push(name.to_string());
                }
            }
        }
    }
    accounts.sort();
    accounts
}

#[tauri::command]
pub async fn list_accounts(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let dir = accounts_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || Ok(list_account_names_sync(&dir)))
        .await
        .map_err(|e| e.to_string())?
}

async fn load_account_file(path: &PathBuf, bridge: &CryptoBridge) -> Result<Option<Settings>, String> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(config_state::load_file(path, bridge).await?))
}

async fn persist_current(app: &tauri::AppHandle, merged: &Settings) -> Result<serde_json::Value, String> {
    let bridge = CryptoBridge::from_app(app);
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {e}"))?;
    config_state::save_to(&dir, &bridge, merged).await?;
    if let Ok(mut cache) = app.state::<crate::android_state::AndroidState>().config.lock() {
        *cache = Some(merged.clone());
    }
    Ok(config_state::masked_for_display(merged))
}

#[tauri::command]
pub async fn switch_account(
    app: tauri::AppHandle,
    account_name: String,
) -> Result<AccountResult, String> {
    let safe_name = match validate_account_name(&account_name) {
        Ok(n) => n,
        Err(e) => return Ok(AccountResult::err(&e)),
    };
    let dir = accounts_dir(&app)?;
    let bridge = CryptoBridge::from_app(&app);
    let path = dir.join(format!("{safe_name}.json"));

    let _io = config_io_lock().await;
    let Some(account_cfg) = load_account_file(&path, &bridge).await? else {
        return Ok(AccountResult::err("账号不存在"));
    };

    // 合并登录字段到当前配置(adapter/双适配器为桌面专属,安卓不存在)
    let mut merged = config_state::current_settings(&app).await?;
    merged.user = account_cfg.user;
    merged.password = account_cfg.password;
    merged.operator = account_cfg.operator;
    merged.active_account = safe_name.clone();

    let display = persist_current(&app, &merged).await?;
    Ok(AccountResult::ok_with_account(safe_name, display))
}

#[tauri::command]
pub async fn save_current_as_account(
    app: tauri::AppHandle,
    account_name: String,
) -> Result<AccountResult, String> {
    let safe_name = match validate_account_name(&account_name) {
        Ok(n) => n,
        Err(e) => return Ok(AccountResult::err(&e)),
    };
    let dir = accounts_dir(&app)?;
    let bridge = CryptoBridge::from_app(&app);
    let path = dir.join(format!("{safe_name}.json"));

    let _io = config_io_lock().await;
    let current = config_state::current_settings(&app).await?;
    config_state::save_file(&path, &bridge, &current).await?;

    // active_account 更新回主配置
    let mut updated = current;
    updated.active_account = safe_name.clone();
    let display = persist_current(&app, &updated).await?;
    Ok(AccountResult::ok_with_account(safe_name, display))
}

#[tauri::command]
pub async fn delete_account(
    app: tauri::AppHandle,
    account_name: String,
) -> Result<AccountResult, String> {
    let safe_name = match validate_account_name(&account_name) {
        Ok(n) => n,
        Err(e) => return Ok(AccountResult::err(&e)),
    };
    let dir = accounts_dir(&app)?;
    let path = dir.join(format!("{safe_name}.json"));

    let _io = config_io_lock().await;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("删除账号文件失败: {e}"))?;
    }
    // 删除的是当前账号 → 清 active_account 并落盘
    let mut current = config_state::current_settings(&app).await?;
    if current.active_account == safe_name {
        current.active_account.clear();
        let display = persist_current(&app, &current).await?;
        return Ok(AccountResult { success: true, message: None, active_account: Some(String::new()), config: Some(display) });
    }
    Ok(AccountResult::ok(config_state::masked_for_display(&current)))
}

#[tauri::command]
pub async fn get_active_account(app: tauri::AppHandle) -> Result<String, String> {
    Ok(config_state::current_settings(&app).await?.active_account)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 账号名消毒_放行合法字符() {
        assert_eq!(validate_account_name("我的账号-01").unwrap(), "我的账号-01");
        assert_eq!(validate_account_name("a_b-2").unwrap(), "a_b-2");
    }

    #[test]
    fn 账号名消毒_拒绝路径穿越与非法字符() {
        assert!(validate_account_name("").is_err());
        assert!(validate_account_name("../etc/passwd").is_err(), "路径分隔必须拒绝");
        assert!(validate_account_name("a/b").is_err());
        assert!(validate_account_name("a\\b").is_err());
        assert!(validate_account_name(".hidden").is_err(), "点号不在白名单");
        assert!(validate_account_name(&"x".repeat(33)).is_err());
    }

    #[test]
    fn 账号名消毒_长度边界() {
        let name_32 = "名".repeat(32);
        assert!(validate_account_name(&name_32).is_ok());
    }

    #[test]
    fn 列举账号_过滤隐藏与排序() {
        let dir = std::env::temp_dir().join(format!("campus-accounts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.json"), "{}").unwrap();
        std::fs::write(dir.join("a.json"), "{}").unwrap();
        std::fs::write(dir.join(".hidden.json"), "{}").unwrap();
        std::fs::write(dir.join("c.txt"), "{}").unwrap();
        let names = list_account_names_sync(&dir);
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
