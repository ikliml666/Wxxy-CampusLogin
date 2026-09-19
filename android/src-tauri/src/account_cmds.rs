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

/// 账号列表条目(R3,与桌面 AccountItem 同契约):id 为账号文件名 stem(稳定不变),
/// display_name 为可读显示名(持久层空值已在 list_account_items_sync 兜底为 id,
/// 出站一律非空)。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AccountItem {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    /// rename_account 成功时携带:trim 后的新显示名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

impl AccountResult {
    fn ok(config: serde_json::Value) -> Self {
        Self { success: true, message: None, active_account: None, display_name: None, config: Some(config) }
    }
    fn ok_with_account(account: String, config: serde_json::Value) -> Self {
        Self { success: true, message: None, active_account: Some(account), display_name: None, config: Some(config) }
    }
    fn err(msg: &str) -> Self {
        Self { success: false, message: Some(msg.to_string()), active_account: None, display_name: None, config: None }
    }
}

/// 账号 id 消毒(R2,与桌面 sanitize_account_id 同构):把账号文件名白名单
/// (`[a-zA-Z0-9_\u{4e00}-\u{9fff}-]`)之外的字符替换为 `_`,按字符截断 32 位。
/// 空输入 → 空输出(调用方跳过建号);全非法输入 → 全下划线(仍建号)。
pub fn sanitize_account_id(user: &str) -> String {
    user.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || ('\u{4e00}'..='\u{9fff}').contains(&c) {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect()
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

/// 从账号文件 JSON 读 settings.displayName(不解密:display_name 非敏感字段,
/// 列举路径无需走 Keystore)。读取失败返回 Err 由调用方 log_warn 后兜底。
fn read_display_name(path: &std::path::Path) -> Result<String, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(v["settings"]["displayName"].as_str().unwrap_or("").to_string())
}

/// 同步列举账号条目:按 id 排序、过滤隐藏文件;display_name 从账号文件读,
/// 为空/读取失败/解析失败时兜底为 id(读取失败仅 log_warn,不让整次列举失败)。
pub fn list_account_items_sync(dir: &std::path::Path) -> Vec<AccountItem> {
    let mut items = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let path = entry.path();
            let Some(id) = path.file_stem().and_then(|n| n.to_str()) else {
                continue;
            };
            if id.starts_with('.') || id.is_empty() {
                continue;
            }
            let display = match read_display_name(&path) {
                Ok(d) if !d.trim().is_empty() => d,
                Ok(_) => id.to_string(),
                Err(e) => {
                    campus_login_lib::log_warn!("account", "读取账号文件显示名失败,兜底为 id: {id}: {e}");
                    id.to_string()
                }
            };
            items.push(AccountItem { id: id.to_string(), display_name: display });
        }
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

#[tauri::command]
pub async fn list_accounts(app: tauri::AppHandle) -> Result<Vec<AccountItem>, String> {
    let dir = accounts_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || Ok(list_account_items_sync(&dir)))
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

    // 合并登录字段到当前配置(adapter/双适配器为桌面专属,安卓不存在);
    // display_name 档案为空时兜底为账号 id(读取阶段兜底约定,与桌面同构)
    let mut merged = config_state::current_settings(&app).await?;
    merged.user = account_cfg.user;
    merged.password = account_cfg.password;
    merged.operator = account_cfg.operator;
    // 切账号后夜切恢复目标失效:restore 暂存的是旧账号的 ISP 后缀,残留会把
    // A 账号的运营商恢复到 B 账号上,置空使夜切回到未切换态(与桌面同构)
    merged.night_operator_restore = String::new();
    merged.display_name = if account_cfg.display_name.is_empty() {
        safe_name.clone()
    } else {
        account_cfg.display_name
    };
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
    // 目标档案已存在 → 保留自定义显示名等非登录字段(与桌面 save_current_as_account
    // 同构);不存在/解密失败置空 → 整档写入(load_file 解密失败置空不 Err,宽松语义)
    let existing = load_account_file(&path, &bridge).await?;
    let (save_account, final_display) = merge_save_as_target(&current, existing, &safe_name);
    config_state::save_file(&path, &bridge, &save_account).await?;

    // 主配置同步 active_account 与最终显示名(档案自带非空显示名优先,否则兜底 id),
    // 避免"另存后主配置仍挂着旧账号的显示名"造成串号
    let mut updated = current;
    updated.active_account = safe_name.clone();
    updated.display_name = final_display;
    let display = persist_current(&app, &updated).await?;
    Ok(AccountResult::ok_with_account(safe_name, display))
}

/// 另存账号合并规则(纯函数,与桌面 save_current_as_account 同构,可单测)。
/// 返回 (落盘档案, 主配置最终显示名):
/// - 另存场景(current.active_account != safe_name)落盘副本 display_name 置空
///   (读取阶段由 id 兜底),不得把旧激活账号的显示名串写进新档案;
/// - 目标档案已存在 → 保留其非登录字段(自定义显示名等),登录字段用当前配置覆盖;
/// - 主配置最终显示名 = 档案自带非空显示名优先,否则兜底账号 id。
fn merge_save_as_target(
    current: &Settings,
    existing: Option<Settings>,
    safe_name: &str,
) -> (Settings, String) {
    let is_resave = current.active_account != safe_name;
    let mut account_data = current.clone();
    account_data.active_account = safe_name.to_string();
    if is_resave {
        account_data.display_name = String::new();
    }

    let save_account = match existing {
        Some(mut e) => {
            e.user = account_data.user.clone();
            e.password = account_data.password.clone();
            e.operator = account_data.operator.clone();
            e.active_account = account_data.active_account.clone();
            e
        }
        None => account_data,
    };

    let final_display = if save_account.display_name.trim().is_empty() {
        safe_name.to_string()
    } else {
        save_account.display_name.clone()
    };
    (save_account, final_display)
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
        return Ok(AccountResult { success: true, message: None, active_account: Some(String::new()), display_name: None, config: Some(display) });
    }
    Ok(AccountResult::ok(config_state::masked_for_display(&current)))
}

#[tauri::command]
pub async fn get_active_account(app: tauri::AppHandle) -> Result<String, String> {
    Ok(config_state::current_settings(&app).await?.active_account)
}

#[tauri::command]
pub async fn rename_account(
    app: tauri::AppHandle,
    account_id: String,
    display_name: String,
) -> Result<AccountResult, String> {
    // account_id 拼文件名,过文件名白名单防路径穿越(合法 id 均来自
    // sanitize/validate,必过)。错误通道与 switch_account 不同:switch 的业务
    // 错误走 AccountResult::err(success=false,前端按返回值分支 toast);
    // rename 的业务错误(校验失败/重名/账号不存在等)以 IPC Err(String) 返回,
    // 前端 useAccount 的 try/catch 捕获后同样 toast——两条通道各自适用,
    // rename 的纯错误场景保持 Err(String),与桌面同构
    let account_id = validate_account_name(&account_id)?;
    let dir = accounts_dir(&app)?;
    let bridge = CryptoBridge::from_app(&app);

    let _io = config_io_lock().await;
    let new_name = rename_account_core(&dir, &bridge, &account_id, &display_name).await?;

    // 被改名账号是当前激活账号 → 同步主配置 display_name 并落盘(走
    // persist_current:落盘 + 缓存刷新);不动文件名、不动 active_account
    let mut current = config_state::current_settings(&app).await?;
    if current.active_account == account_id {
        current.display_name = new_name.clone();
        persist_current(&app, &current).await?;
    }
    let active = current.active_account.clone();
    let display = config_state::masked_for_display(&current);
    let mut result = AccountResult::ok_with_account(active, display);
    result.display_name = Some(new_name.clone());
    campus_login_lib::log_info!("account", "账号改名: {} -> {}", account_id, new_name);
    Ok(result)
}

/// rename_account 核心(不依赖 AppHandle,可单测):校验显示名 → 账号存在性 →
/// 重名检查 → 只改 displayName 写回(其余字段一律保留)。返回 trim 后的新显示名。
async fn rename_account_core(
    dir: &std::path::Path,
    bridge: &CryptoBridge,
    account_id: &str,
    display_name: &str,
) -> Result<String, String> {
    let new_name = validate_display_name(display_name)?;
    let path = dir.join(format!("{account_id}.json"));
    if !path.exists() {
        return Err("账号不存在".to_string());
    }
    let mut account_cfg = config_state::load_file(&path, bridge).await?;

    // 重名检查:与其他账号(按 id 排除自身)的显示名重复则拒绝;
    // list_account_items_sync 已对空显示名兜底为 id,此处直接比较
    for item in list_account_items_sync(dir) {
        if item.id != account_id && item.display_name == new_name {
            return Err(format!("名称已存在: {new_name}"));
        }
    }

    account_cfg.display_name = new_name.clone();
    config_state::save_file(&path, bridge, &account_cfg).await?;
    Ok(new_name)
}

/// 显示名校验(R3,与桌面 validate_display_name 同构):trim 后按字符数 1..=32;
/// 不得含控制字符或换行;允许空格与 emoji。注意:不复用 validate_account_name
/// 的文件名字符集校验(显示名与内部 id 分离,任意可见字符合法)。返回 trim 后的名称用于落盘。
fn validate_display_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let count = trimmed.chars().count();
    if count == 0 {
        return Err("显示名称不能为空".to_string());
    }
    if count > 32 {
        return Err("显示名称长度需在1-32之间".to_string());
    }
    if trimmed.chars().any(char::is_control) {
        return Err("显示名称不得包含控制字符或换行".to_string());
    }
    Ok(trimmed.to_string())
}

/// R2 自动建号入口:save_config 落盘成功后调用(不经命令层,避免递归)。
/// 失败仅 log_warn,不得让保存失败。
pub async fn auto_create_account_for_current(app: &tauri::AppHandle, current: &Settings) {
    let Ok(dir) = accounts_dir(app) else {
        campus_login_lib::log_warn!("account", "自动创建/同步账号失败: 获取数据目录失败");
        return;
    };
    let bridge = CryptoBridge::from_app(app);
    if let Err(e) = auto_create_account_in(&dir, &bridge, current).await {
        campus_login_lib::log_warn!("account", "自动创建/同步账号失败: {}", e);
    }
}

/// 自动建号核心(不依赖 AppHandle,可单测),规则与桌面 auto_create_account_in 同构:
/// 1. user 或 password 为空 → 跳过;
/// 2. id = sanitize_account_id(user),为空 → 跳过;
/// 3. 账号文件不存在 → 以当前配置快照创建,display_name = 原始 user 文本,
///    文件内 active_account = id;
/// 4. 已存在且 user 原文不同 → 撞库(不同用户名 sanitize 出同一 id),跳过并
///    log_warn,绝不覆盖他人账号凭据;
/// 5. 已存在且 user 一致 → 读回比对 password/operator:一致则跳过(幂等短路,
///    防止每次防抖保存都写盘);有差异则只更新这 3 个字段,保留 display_name
///    等其他字段(不覆盖用户自定义显示名)。
/// 只调用底层 save_file,绝不触发 save_config 命令层。
async fn auto_create_account_in(
    dir: &std::path::Path,
    bridge: &CryptoBridge,
    config: &Settings,
) -> Result<(), String> {
    if config.user.is_empty() || config.password.is_empty() {
        return Ok(());
    }
    let id = sanitize_account_id(&config.user);
    if id.is_empty() {
        return Ok(());
    }
    let path = dir.join(format!("{id}.json"));
    if !path.exists() {
        let mut snapshot = config.clone();
        snapshot.display_name = config.user.clone();
        snapshot.active_account = id;
        return config_state::save_file(&path, bridge, &snapshot).await;
    }
    let existing = config_state::load_file(&path, bridge).await?;
    if existing.user != config.user {
        campus_login_lib::log_warn!(
            "account",
            "自动建号跳过: 账号 {} 已属于用户 {}，与当前输入用户名不同，不覆盖其凭据",
            id,
            existing.user
        );
        return Ok(());
    }
    if existing.password == config.password && existing.operator == config.operator {
        return Ok(());
    }
    let mut updated = existing;
    updated.user = config.user.clone();
    updated.password = config.password.clone();
    updated.operator = config.operator.clone();
    config_state::save_file(&path, bridge, &updated).await
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
    fn 列举账号_过滤隐藏与排序_空显示名兜底为id() {
        let dir = std::env::temp_dir().join(format!("campus-accounts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 无 displayName 字段的文件 → 兜底为 id
        std::fs::write(dir.join("b.json"), "{}").unwrap();
        std::fs::write(dir.join("a.json"), "{}").unwrap();
        std::fs::write(dir.join(".hidden.json"), "{}").unwrap();
        std::fs::write(dir.join("c.txt"), "{}").unwrap();
        let items = list_account_items_sync(&dir);
        assert_eq!(
            items,
            vec![
                AccountItem { id: "a".to_string(), display_name: "a".to_string() },
                AccountItem { id: "b".to_string(), display_name: "b".to_string() },
            ],
            "按 id 排序、过滤隐藏与非 json、空 displayName 兜底为 id"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // ---------- 以下为单元 D 新增:display_name / 自动建号 / 改名 ----------

    use std::sync::Arc;

    /// 可逆假加密桥(与 config_state 测试的 fake_bridge 同思路,输出带前缀非明文)
    fn test_bridge() -> CryptoBridge {
        CryptoBridge {
            encrypt: Arc::new(|t| Ok(format!("enc:{t}"))),
            decrypt: Arc::new(|d| Ok(d.strip_prefix("enc:").unwrap_or(d).to_string())),
        }
    }

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("campus-acct-cmds-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn settings_with(user: &str, password: &str) -> Settings {
        Settings {
            user: user.to_string(),
            password: password.to_string(),
            operator: "@cmcc".to_string(),
            ..Settings::default()
        }
    }

    #[test]
    fn 账号id消毒_边界与桌面同构() {
        // 合法学号原样保留;合法字符类:字母数字下划线中文连字符
        assert_eq!(sanitize_account_id("2023123456"), "2023123456");
        assert_eq!(sanitize_account_id("user_01-甲"), "user_01-甲");
        // @ 与 . 等非法字符替换为 _
        assert_eq!(sanitize_account_id("a@b.c"), "a_b_c");
        // 按字符截断 32(中文按 1 字符计)
        assert_eq!(sanitize_account_id(&"甲".repeat(40)).chars().count(), 32);
        // 全非法字符 → 全下划线(仍会建号)
        assert_eq!(sanitize_account_id("@@#"), "___");
        // 空输入 → 空输出(调用方跳过建号)
        assert_eq!(sanitize_account_id(""), "");
    }

    #[test]
    fn 显示名校验_边界() {
        // 空与纯空格拒绝(trim 后为空)
        assert!(validate_display_name("").is_err());
        assert!(validate_display_name("   ").is_err());
        // 33 字符拒绝、32 字符边界通过(按字符计)
        assert!(validate_display_name(&"甲".repeat(33)).is_err());
        assert!(validate_display_name(&"甲".repeat(32)).is_ok());
        // 换行与控制字符拒绝
        assert!(validate_display_name("a\nb").is_err());
        assert!(validate_display_name("a\u{7}b").is_err());
        // 空格与 emoji 允许;trim 后返回
        assert_eq!(validate_display_name("  我的 账号😀  ").unwrap(), "我的 账号😀");
        // 不做文件名字符集校验:@ . 等符号允许(账号名可自由编辑是本需求核心)
        assert!(validate_display_name("a@b.c").is_ok());
    }

    #[tokio::test]
    async fn 自动建号_无凭据跳过() {
        let dir = tmp_dir("auto_skip");
        let bridge = test_bridge();
        // user 为空
        auto_create_account_in(&dir, &bridge, &settings_with("", "pwd")).await.unwrap();
        // password 为空
        auto_create_account_in(&dir, &bridge, &settings_with("user", "")).await.unwrap();
        assert!(list_account_items_sync(&dir).is_empty(), "无凭据不得建号");
    }

    #[tokio::test]
    async fn 自动建号_新号_原始user作显示名_文件内active为id() {
        let dir = tmp_dir("auto_new");
        let bridge = test_bridge();
        auto_create_account_in(&dir, &bridge, &settings_with("2023@stu.wxxy.edu.cn", "pwd"))
            .await
            .unwrap();

        // id = sanitize(user):@ 与 . 替换为 _
        let items = list_account_items_sync(&dir);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "2023_stu_wxxy_edu_cn");
        // displayName = 原始 user 文本(未 sanitize)
        assert_eq!(items[0].display_name, "2023@stu.wxxy.edu.cn");
        // 文件内 active_account 字段 = id
        let loaded = config_state::load_file(&dir.join("2023_stu_wxxy_edu_cn.json"), &bridge)
            .await
            .unwrap();
        assert_eq!(loaded.active_account, "2023_stu_wxxy_edu_cn");
        assert_eq!(loaded.password, "pwd", "密码经加密落盘+解密读回往返还原");
    }

    #[tokio::test]
    async fn 自动建号_幂等_一致不写盘() {
        let dir = tmp_dir("auto_idem");
        let bridge = test_bridge();
        let config = settings_with("user01", "pwd");
        auto_create_account_in(&dir, &bridge, &config).await.unwrap();

        let path = dir.join("user01.json");
        let before_mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let before_bytes = std::fs::read(&path).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(60));
        auto_create_account_in(&dir, &bridge, &config).await.unwrap();

        assert_eq!(
            before_mtime,
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            "内容一致时必须幂等短路,不得写盘"
        );
        assert_eq!(before_bytes, std::fs::read(&path).unwrap());
    }

    #[tokio::test]
    async fn 自动建号_差异更新凭据_保留显示名() {
        let dir = tmp_dir("auto_diff");
        let bridge = test_bridge();
        auto_create_account_in(&dir, &bridge, &settings_with("user01", "pwd-old"))
            .await
            .unwrap();

        // 用户先自定义了显示名(模拟 rename_account 之后)
        let mut customized =
            config_state::load_file(&dir.join("user01.json"), &bridge).await.unwrap();
        customized.display_name = "我的专属名".to_string();
        config_state::save_file(&dir.join("user01.json"), &bridge, &customized).await.unwrap();

        // 修改密码后保存 → 只更新 user/password/operator,display_name 保留
        auto_create_account_in(&dir, &bridge, &settings_with("user01", "pwd-new"))
            .await
            .unwrap();

        let loaded = config_state::load_file(&dir.join("user01.json"), &bridge).await.unwrap();
        assert_eq!(loaded.password, "pwd-new", "有差异时更新凭据");
        assert_eq!(loaded.display_name, "我的专属名", "不得覆盖用户自定义显示名");
        assert_eq!(loaded.operator, "@cmcc", "operator 属比对更新字段");
    }

    #[tokio::test]
    async fn 改名核心_成功_文件名与凭据保留() {
        let dir = tmp_dir("ren_ok");
        let bridge = test_bridge();
        let mut src = settings_with("user-a", "pwd-a");
        src.display_name = "旧名".to_string();
        config_state::save_file(&dir.join("acc-a.json"), &bridge, &src).await.unwrap();

        let new_name = rename_account_core(&dir, &bridge, "acc-a", "  新名字😀 ").await.unwrap();
        assert_eq!(new_name, "新名字😀", "返回 trim 后的新显示名");

        // 文件名不变:acc-a.json 仍在,未新建其他文件
        assert!(dir.join("acc-a.json").exists());
        // 只改 display_name,其余字段保留(含密码加解密往返)
        let loaded = config_state::load_file(&dir.join("acc-a.json"), &bridge).await.unwrap();
        assert_eq!(loaded.display_name, "新名字😀");
        assert_eq!(loaded.user, "user-a");
        assert_eq!(loaded.operator, "@cmcc");
        assert_eq!(loaded.password, "pwd-a", "密码经解密读回+加密写回后仍还原");
    }

    #[tokio::test]
    async fn 改名核心_拒绝_空_超长_重名_不存在() {
        let dir = tmp_dir("ren_rej");
        let bridge = test_bridge();
        let mut a = settings_with("user-a", "pwd-a");
        a.display_name = "甲".to_string();
        config_state::save_file(&dir.join("acc-a.json"), &bridge, &a).await.unwrap();
        let mut b = settings_with("user-b", "pwd-b");
        b.display_name = "乙".to_string();
        config_state::save_file(&dir.join("acc-b.json"), &bridge, &b).await.unwrap();

        // 账号不存在 → Err,不 panic、不新建文件
        let err = rename_account_core(&dir, &bridge, "no-such", "新名").await.unwrap_err();
        assert_eq!(err, "账号不存在");
        assert!(!dir.join("no-such.json").exists(), "失败时不得新建账号文件");

        // 校验拒绝:纯空格 / 33 字符
        assert!(rename_account_core(&dir, &bridge, "acc-a", "   ").await.is_err());
        assert!(rename_account_core(&dir, &bridge, "acc-a", &"甲".repeat(33)).await.is_err());

        // 重名拒绝(与 acc-b 的显示名"乙"重复)
        let err = rename_account_core(&dir, &bridge, "acc-a", "乙").await.unwrap_err();
        assert!(err.contains("名称已存在"), "重名错误需说明名称已存在: {err}");
        // 改名失败后原显示名不被破坏
        let unchanged = config_state::load_file(&dir.join("acc-a.json"), &bridge).await.unwrap();
        assert_eq!(unchanged.display_name, "甲");

        // 对方空显示名兜底为 id:acc-c 无档案显示名 → 以 id "acc-c" 参与重名比较
        config_state::save_file(&dir.join("acc-c.json"), &bridge, &Settings::default())
            .await
            .unwrap();
        let err = rename_account_core(&dir, &bridge, "acc-a", "acc-c").await.unwrap_err();
        assert!(err.contains("名称已存在"), "对方空显示名兜底为 id 后参与重名比较: {err}");

        // 改成与自身当前显示名相同(重名检查排除自身)→ 允许
        assert!(rename_account_core(&dir, &bridge, "acc-a", "甲").await.is_ok());
    }

    /// IPC 出站形状锁:AccountItem 序列化为 { id, displayName },不得出现 snake_case 冗余键
    #[test]
    fn 账号条目_json形状() {
        let item = AccountItem { id: "acc-1".to_string(), display_name: "显示名".to_string() };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["id"], "acc-1");
        assert_eq!(json["displayName"], "显示名");
        assert!(json.get("display_name").is_none());
    }

    /// F2 另存账号显示名不串号:另存场景落盘档案 display_name 置空(读取兜底为 id),
    /// 主配置最终显示名兜底为新账号 id
    #[tokio::test]
    async fn 另存账号_显示名不串号_主配置兜底新id() {
        let dir = tmp_dir("saveas_new");
        let bridge = test_bridge();
        let mut current = settings_with("user-a", "pwd-a");
        current.display_name = "我的A".to_string();
        current.active_account = "user-a".to_string();

        let (saved, final_display) = merge_save_as_target(&current, None, "b");
        assert_eq!(saved.display_name, "", "另存场景不得把旧激活账号的显示名串写进新档案");
        assert_eq!(saved.active_account, "b");
        assert_eq!(final_display, "b", "主配置最终显示名兜底为新账号 id");

        config_state::save_file(&dir.join("b.json"), &bridge, &saved).await.unwrap();
        let loaded = config_state::load_file(&dir.join("b.json"), &bridge).await.unwrap();
        assert_eq!(loaded.display_name, "", "落盘档案 display_name 为空,由读取阶段兜底为 id");
        assert_eq!(loaded.password, "pwd-a", "凭据随当前配置写入");
    }

    /// F2 对照:覆盖写同 id 账号时保留档案的自定义显示名,主配置同步为该显示名
    #[tokio::test]
    async fn 另存账号_覆盖同id档案_保留自定义显示名() {
        let mut current = settings_with("user-b", "pwd-b");
        current.display_name = "我的B".to_string();
        current.active_account = "b".to_string();
        let mut existing = current.clone();
        existing.display_name = "B档案自定义名".to_string();

        let (saved, final_display) = merge_save_as_target(&current, Some(existing), "b");
        assert_eq!(saved.display_name, "B档案自定义名", "覆盖写同 id 账号保留档案自定义显示名");
        assert_eq!(final_display, "B档案自定义名", "主配置显示名同步为档案显示名");
        assert_eq!(saved.password, "pwd-b", "登录字段用当前配置覆盖");
        assert_eq!(saved.active_account, "b");
    }

    /// F7 撞库防护:不同 user 原文 sanitize 出同一 id(2023.1 与 2023/1 同为
    /// 2023_1)时,后到的输入必须跳过并告警,不得覆盖既有账号的凭据
    #[tokio::test]
    async fn 自动建号_撞库_同id不同user原文_跳过不覆盖() {
        let dir = tmp_dir("auto_collision");
        let bridge = test_bridge();
        auto_create_account_in(&dir, &bridge, &settings_with("2023.1", "pwd-old"))
            .await
            .unwrap();
        let path = dir.join("2023_1.json");
        let before = std::fs::read(&path).unwrap();

        auto_create_account_in(&dir, &bridge, &settings_with("2023/1", "pwd-new"))
            .await
            .unwrap();

        assert_eq!(before, std::fs::read(&path).unwrap(), "撞库时不得覆盖既有账号文件");
        let loaded = config_state::load_file(&path, &bridge).await.unwrap();
        assert_eq!(loaded.user, "2023.1", "既有账号的 user 原文保持不变");
        assert_eq!(loaded.password, "pwd-old", "既有账号的密码不被覆盖");
    }
}
