use std::path::Path;
use tauri::{AppHandle, Manager, State};
use crate::config::model::Config;
use crate::config::persist;
use crate::infra::state::{AppState, AccountItem, AccountResult};

#[tauri::command]
pub async fn list_accounts(app_handle: AppHandle) -> Result<Vec<AccountItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data_dir = persist::get_data_dir(&app_handle);
        Ok(persist::list_account_items(&data_dir))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn switch_account(account_name: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<AccountResult, String> {
    let app_h = app_handle.clone();
    // 阻塞读盘+落盘，走 spawn_blocking；State<'_> 不可跨 await，闭包内经 app_handle 重新获取
    let switch_result = tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        perform_switch_account_sync(&app_h, &s, &account_name)
    }).await.map_err(|e| e.to_string())?;

    match switch_result {
        Ok(safe_name) => {
            // R4 根因修复：成功时 activeAccount 必须出现（Some），否则前端守卫
            // `if (result?.activeAccount)` 永假 → 页面不刷新需手动刷新
            let display_config = state.config.load().masked_for_display();
            Ok(AccountResult::ok_with_account(safe_name, display_config))
        }
        // 与原实现语义一致：校验失败/账号不存在等业务错误以 AccountResult::err 返回
        //（前端对 success=false 有统一的错误 toast 分支）
        Err(e) => Ok(AccountResult::err(&e)),
    }
}

/// switch_account 命令与托盘「切换账号」菜单共享的核心逻辑（含阻塞读盘与落盘，
/// 须在 spawn_blocking 等阻塞上下文调用）。落盘统一经 save_config_to_disk_encrypted，
/// 其内部会发射 config-changed（前端自动同步）并刷新托盘菜单。
/// 返回 Ok(safe_name)：校验后的账号 id（命令层据此回填 activeAccount）。
/// 返回 Err(String)：账号名校验失败 / 账号不存在 / 读盘或落盘失败。
pub(crate) fn perform_switch_account_sync(
    app_handle: &AppHandle,
    state: &AppState,
    account_name: &str,
) -> Result<String, String> {
    let safe_name = crate::infra::state::validate_account_name(account_name)?;

    let data_dir = persist::get_data_dir(app_handle);
    let config = persist::load_account_config(&data_dir, &safe_name)?;

    let merged = state.config.update(|c| {
        merge_account_into_config(c, &config, &safe_name);
    });

    super::config_cmd::save_config_to_disk_encrypted(app_handle, &merged)?;

    crate::log_info!("account", "切换账号: {} (用户: {})", safe_name, config.user);
    Ok(safe_name)
}

/// switch_account 的字段合并规则（可单测）：6 个登录字段（user/password/operator/
/// adapter1/adapter2/dualAdapter）+ display_name + active_account。
/// 明确排除 adapter1_account / adapter2_account（设备级配置，切账号不改变
/// "网卡→账号"映射）。display_name 为空时兜底为账号 id（读取阶段兜底约定）。
fn merge_account_into_config(current: &mut Config, account: &Config, safe_name: &str) {
    current.user = account.user.clone();
    current.password = account.password.clone();
    current.operator = account.operator.clone();
    current.adapter1 = account.adapter1.clone();
    current.adapter2 = account.adapter2.clone();
    current.dual_adapter = account.dual_adapter;
    current.display_name = if account.display_name.is_empty() {
        safe_name.to_string()
    } else {
        account.display_name.clone()
    };
    current.active_account = safe_name.to_string();
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
            let data_dir = persist::get_data_dir(&app_h_prev);
            let mut save_prev = match persist::load_account_config(&data_dir, &prev_name) {
                // 保留旧账号的非登录字段（主题、自定义显示名等），登录字段一律用当前配置覆盖
                Ok(existing) => existing,
                Err(e) if e == "账号不存在" => {
                    // 新回存的旧账号档案：显示名兜底为账号 id
                    let mut fresh = Config::default();
                    fresh.display_name = prev_name.clone();
                    fresh
                }
                Err(e) => {
                    crate::log_error!("account", "读取旧账号文件失败: {}", e);
                    return Err("读取旧账号配置失败".to_string());
                }
            };

            save_prev.user = prev_user;
            save_prev.password = prev_password;
            save_prev.operator = prev_operator;
            save_prev.adapter1 = prev_adapter1;
            save_prev.adapter2 = prev_adapter2;
            save_prev.dual_adapter = prev_dual_adapter;

            persist::save_account_config(&data_dir, &prev_name, &save_prev)
        }).await;
        match prev_save_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => crate::log_warn!("account", "保存旧账号配置失败: {}", e),
            Err(e) => crate::log_warn!("account", "保存旧账号配置任务失败: {}", e),
        }
    }

    let app_h = app_handle.clone();
    let password_for_encrypt = config.password.clone();
    let mut account_data = (*config).clone();
    account_data.password = String::new();
    account_data.active_account = account_name.clone();
    // 另存场景（保存名 != 保存前激活账号）不得把旧激活账号的显示名串写进新档案，
    // 置空后由读取阶段兜底为账号 id；保存当前场景保留主配置的自定义显示名
    if config.active_account != safe_name {
        account_data.display_name = String::new();
    }
    let final_display = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let data_dir = persist::get_data_dir(&app_h);
        let mut save_account = merge_save_as_target(
            persist::load_account_config(&data_dir, &safe_name),
            &account_data,
        )?;

        if password_for_encrypt.is_empty() {
            save_account.password = String::new();
        } else {
            save_account.password = password_for_encrypt;
        }

        // 主配置同步用的最终显示名：档案自带显示名优先，否则兜底账号 id
        let display = if save_account.display_name.trim().is_empty() {
            safe_name.clone()
        } else {
            save_account.display_name.clone()
        };
        persist::save_account_config(&data_dir, &safe_name, &save_account)?;
        Ok::<String, String>(display)
    }).await.map_err(|e| e.to_string())??;

    state.config.update(|c| {
        c.active_account = account_name.clone();
        // 显示名与激活账号同步，避免"高亮 id 与显示名错位"
        c.display_name = final_display.clone();
    });

    // 与 switch_account/delete_account 对齐：active_account 变更必须落盘，
    // 否则重启后回落到旧账号
    let cfg = state.config.load();
    if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
        crate::log_warn!("account", "保存账号后持久化 active_account 失败: {}", e);
    }

    let display_config = state.config.load().masked_for_display();
    crate::log_info!("account", "保存账号: {}", account_name);
    Ok(AccountResult::ok_with_account(account_name, display_config))
}

/// 另存账号时目标档案的合并规则（纯函数，可单测）：
/// - 档案已存在 → 保留非登录字段（自定义显示名、主题等），登录字段用当前配置覆盖；
/// - 档案不存在 → 用当前配置整档写入；
/// - 读盘 IO 失败 / 密码解密失败 → 宽松回退：log_warn 后用当前配置整档覆盖。
///   解密失败这条路径在旧实现（直接 serde_json::from_str，不解密）中不存在，
///   若硬错误中止用户反而无法借"另存"自救——并入宽松分支，与安卓 load_file
///   解密失败置空不 Err 的宽松语义对齐；
/// - JSON 解析失败 → 硬错误中止（档案结构损坏，与旧实现 from_str 失败语义一致）。
fn merge_save_as_target(existing: Result<Config, String>, account_data: &Config) -> Result<Config, String> {
    match existing {
        Ok(mut e) => {
            e.user = account_data.user.clone();
            e.operator = account_data.operator.clone();
            e.adapter1 = account_data.adapter1.clone();
            e.adapter2 = account_data.adapter2.clone();
            e.dual_adapter = account_data.dual_adapter;
            e.active_account = account_data.active_account.clone();
            Ok(e)
        }
        Err(e) if e == "账号不存在"
            || e.starts_with("读取账号配置失败")
            || e == "账号密码解密失败" =>
        {
            if e != "账号不存在" {
                crate::log_warn!("account", "读取已有账号文件失败，使用当前配置覆盖: {}", e);
            }
            Ok(account_data.clone())
        }
        Err(e) => Err(e),
    }
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
        let account_path = accounts_dir.join(format!("{name}.json"));
        if !account_path.exists() {
            return Err("账号不存在".to_string());
        }
        std::fs::remove_file(&account_path).map_err(|e| format!("删除账号失败: {e}"))?;
        crate::log_info!("account", "删除账号: {}", name);
        Ok(())
    }).await.map_err(|e| e.to_string())??;

    let current_config = state.config.load();
    let cleared_active = current_config.active_account == account_name;
    if cleared_active {
        state.config.update(|c| {
            c.active_account = String::new();
        });
        // 历史缺陷：清空 active_account 仅内存更新不落盘，重启后配置仍指向已删除账号。
        // 持久化并发射 config-changed（删除账号后 UI 若不清空，显示将永久不一致）。
        let cfg = state.config.load();
        if let Err(e) = super::config_cmd::save_config_to_disk_encrypted(&app_handle, &cfg) {
            crate::log_warn!("account", "保存删除账号后的配置失败: {}", e);
        }
    }

    let display_config = state.config.load().masked_for_display();
    let mut result = AccountResult::ok(display_config);
    if cleared_active {
        result.active_account = Some(String::new());
    }
    // 托盘「切换账号」子菜单按账号文件列表构建，删除后需重建（删除当前账号时
    // 上方 save_config_to_disk_encrypted 已触发，这里统一刷新一次保证覆盖）
    crate::app::tray::refresh_tray_menu_state(&app_handle);
    Ok(result)
}

#[tauri::command]
pub fn get_active_account(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.load();
    Ok(config.active_account.clone())
}

#[tauri::command]
pub async fn rename_account(account_id: String, display_name: String, app_handle: AppHandle) -> Result<AccountResult, String> {
    let app_h = app_handle.clone();
    // 读盘+落盘走 spawn_blocking；State<'_> 不可跨 await，闭包内经 app_handle 重新获取
    let rename_result = tauri::async_runtime::spawn_blocking(move || {
        let s = app_h.state::<AppState>();
        perform_rename_account_sync(&app_h, &s, &account_id, &display_name)
    }).await.map_err(|e| e.to_string())?;

    // 错误通道与 switch_account 不同：switch 的业务错误走 AccountResult::err
    //（success=false，前端按返回值分支 toast）；rename 的业务错误（校验失败/
    // 重名/账号不存在等）以 IPC Err(String) 返回，前端 useAccount 的 try/catch
    // 捕获后同样 toast——两条通道各自适用，rename 的纯错误场景保持 Err(String)
    rename_result
}

/// rename_account 命令的核心逻辑（含阻塞读盘与落盘，须在阻塞上下文调用）。
/// 只改账号档案的 displayName，不动文件名与 active_account；若被改名账号是
/// 当前激活账号，同步主配置 display_name 并落盘（经 save_config_to_disk_encrypted
/// 广播 config-changed，前端即时可见）。
fn perform_rename_account_sync(
    app_handle: &AppHandle,
    state: &AppState,
    account_id: &str,
    display_name: &str,
) -> Result<AccountResult, String> {
    // 账号 id 直接拼文件路径（accounts/<id>.json），必须先过文件名白名单防路径
    // 穿越——与 perform_switch_account_sync 同款防护，../config 等输入直接拒绝
    let safe_id = crate::infra::state::validate_account_name(account_id)?;
    let data_dir = persist::get_data_dir(app_handle);
    let active_account = state.config.load().active_account.clone();
    let new_name = rename_account_core(&data_dir, &safe_id, display_name)?;

    if safe_id == active_account {
        let merged = state.config.update(|c| {
            c.display_name = new_name.clone();
        });
        super::config_cmd::save_config_to_disk_encrypted(app_handle, &merged)?;
    }

    let display_config = state.config.load().masked_for_display();
    let active_now = state.config.load().active_account.clone();
    let mut result = AccountResult::ok_with_account(active_now, display_config);
    result.display_name = Some(new_name.clone());
    crate::log_info!("account", "账号改名: {} -> {}", safe_id, new_name);
    Ok(result)
}

/// 改名核心（不依赖 AppHandle，可单测）：校验显示名 → 重名检查 → 只改
/// displayName 写回（其余字段一律保留）。返回 trim 后的新显示名。
fn rename_account_core(data_dir: &Path, account_id: &str, display_name: &str) -> Result<String, String> {
    let new_name = validate_display_name(display_name)?;

    // id 直接拼文件路径，过文件名白名单防路径穿越（核心层兜底防护，可单测覆盖；
    // perform_rename_account_sync 入口已先校验，此处对直接调用幂等）
    let account_id = crate::infra::state::validate_account_name(account_id)?;

    // 账号必须存在（不存在/读盘/解析失败均以 Err 返回，不 panic）
    let mut config = persist::load_account_config(data_dir, &account_id)?;

    // 重名检查：与其他账号（按 id 排除自身）的显示名重复则拒绝；
    // list_account_items 已对空显示名兜底为 id，此处直接比较
    for item in persist::list_account_items(data_dir) {
        if item.id == account_id {
            continue;
        }
        if item.display_name == new_name {
            return Err(format!("名称已存在: {new_name}"));
        }
    }

    config.display_name = new_name.clone();
    persist::save_account_config(data_dir, &account_id, &config)?;
    Ok(new_name)
}

/// 显示名校验（R3）：trim 后按字符数 1..=32；不得含控制字符或换行；
/// 允许空格与 emoji。注意：不复用 validate_account_name 的文件名字符集校验
///（显示名与内部 id 分离，任意可见字符合法）。返回 trim 后的名称用于落盘。
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

/// R2 自动建号入口：save_config 落盘成功后调用（不经命令层，避免事件/递归）。
/// 失败仅 log_warn，不得让保存失败。返回是否实际创建/更新了账号档案
///（调用方据此补刷托盘菜单；跳过/幂等短路为 false）。
pub(crate) fn auto_create_account_for_current(state: &AppState, app_handle: &AppHandle) -> bool {
    let data_dir = persist::get_data_dir(app_handle);
    let config = state.config.load();
    match auto_create_account_in(&data_dir, &config) {
        Ok(wrote) => wrote,
        Err(e) => {
            crate::log_warn!("account", "自动创建/同步账号失败: {}", e);
            false
        }
    }
}

/// 自动建号核心（不依赖 AppHandle，可单测），规则见计划 §4.2：
/// 1. user 或 password 为空 → 跳过；
/// 2. id = sanitize(user)，为空 → 跳过；
/// 3. 账号文件不存在 → 以当前配置快照创建，displayName = 原始 user 文本；
/// 4. 已存在且 user 原文不同 → 撞库（不同用户名 sanitize 出同一 id），跳过并
///    log_warn，绝不覆盖他人账号凭据；
/// 5. 已存在且 user 一致 → 读回比对 password/operator：一致则跳过（幂等短路，
///    防止每次防抖保存都写盘）；有差异则只更新这 3 个字段，保留 displayName
///    与其他字段（不覆盖用户自定义显示名）。
/// 返回 Ok(是否实际写盘)。只调用底层落盘函数（persist::save_account_config），
/// 绝不触发 save_config 命令层。
fn auto_create_account_in(data_dir: &Path, config: &Config) -> Result<bool, String> {
    if config.user.is_empty() || config.password.is_empty() {
        return Ok(false);
    }
    let id = crate::infra::state::sanitize_account_id(&config.user);
    if id.is_empty() {
        return Ok(false);
    }

    if !persist::get_account_path(data_dir, &id).exists() {
        let mut snapshot = config.clone();
        snapshot.display_name = config.user.clone();
        snapshot.active_account = id.clone();
        return persist::save_account_config(data_dir, &id, &snapshot).map(|_| true);
    }

    let existing = persist::load_account_config(data_dir, &id)?;
    if existing.user != config.user {
        crate::log_warn!(
            "account",
            "自动建号跳过: 账号 {} 已属于用户 {}，与当前输入用户名不同，不覆盖其凭据",
            id,
            existing.user
        );
        return Ok(false);
    }
    if existing.password == config.password && existing.operator == config.operator {
        return Ok(false);
    }
    let mut updated = existing;
    updated.user = config.user.clone();
    updated.password = config.password.clone();
    updated.operator = config.operator.clone();
    persist::save_account_config(data_dir, &id, &updated).map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::persist::{get_account_path, list_account_items, load_account_config, save_account_config};

    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cl_account_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_account(user: &str, password: &str, display: &str) -> Config {
        Config {
            user: user.to_string(),
            password: password.to_string(),
            operator: "校园".to_string(),
            display_name: display.to_string(),
            ..Default::default()
        }
    }

    // ---------- 显示名校验 ----------

    #[test]
    fn display_name_validation() {
        // 空与纯空格拒绝（trim 后为空）
        assert!(validate_display_name("").is_err());
        assert!(validate_display_name("   ").is_err());
        // 33 字符拒绝（按字符计）
        assert!(validate_display_name("甲".repeat(33).as_str()).is_err());
        // 换行与控制字符拒绝
        assert!(validate_display_name("a\nb").is_err());
        assert!(validate_display_name("a\u{7}b").is_err());
        // 32 字符边界通过
        assert!(validate_display_name("甲".repeat(32).as_str()).is_ok());
        // 空格与 emoji 允许；trim 后返回
        assert_eq!(validate_display_name("  我的 账号😀  ").unwrap(), "我的 账号😀");
        // 不做文件名字符集校验：@ . 等符号允许
        assert!(validate_display_name("a@b.c").is_ok());
    }

    // ---------- rename_account 核心 ----------

    #[test]
    fn rename_account_core_success_keeps_filename_and_other_fields() {
        let dir = temp_data_dir("ren_ok");
        save_account_config(&dir, "acc-a", &sample_account("user-a", "pwd-a", "旧名")).unwrap();

        let new_name = rename_account_core(&dir, "acc-a", "  新名字😀 ").unwrap();
        assert_eq!(new_name, "新名字😀", "返回 trim 后的新显示名");

        // 文件名不变：acc-a.json 仍在
        assert!(get_account_path(&dir, "acc-a").exists());
        // 只改 display_name，其余字段保留（含密码加解密往返、文件内 active_account 字段）
        let loaded = load_account_config(&dir, "acc-a").unwrap();
        assert_eq!(loaded.display_name, "新名字😀");
        assert_eq!(loaded.user, "user-a");
        assert_eq!(loaded.operator, "校园");
        assert_eq!(loaded.password, "pwd-a", "密码经解密读回+加密写回后仍还原");
        assert_eq!(loaded.active_account, "", "账号文件内 active_account 字段不被改名波及");
    }

    #[test]
    fn rename_account_core_rejects() {
        let dir = temp_data_dir("ren_rej");
        save_account_config(&dir, "acc-a", &sample_account("user-a", "pwd-a", "甲")).unwrap();
        save_account_config(&dir, "acc-b", &sample_account("user-b", "pwd-b", "乙")).unwrap();

        // 账号不存在 → Err，不 panic
        let err = rename_account_core(&dir, "no-such", "新名").unwrap_err();
        assert_eq!(err, "账号不存在");
        assert!(!get_account_path(&dir, "no-such").exists(), "失败时不得新建账号文件");

        // 校验拒绝：空 / 33 字符
        assert!(rename_account_core(&dir, "acc-a", "   ").is_err());
        assert!(rename_account_core(&dir, "acc-a", "甲".repeat(33).as_str()).is_err());

        // 重名拒绝（与 acc-b 的显示名“乙”重复）
        let err = rename_account_core(&dir, "acc-a", "乙").unwrap_err();
        assert!(err.contains("名称已存在"), "重名错误需说明名称已存在: {err}");
        // 改名失败后原显示名不被破坏
        assert_eq!(load_account_config(&dir, "acc-a").unwrap().display_name, "甲");

        // 对方空显示名兜底为 id：acc-c 无档案显示名 → 以 id “acc-c” 参与重名比较
        save_account_config(&dir, "acc-c", &Config::default()).unwrap();
        let err = rename_account_core(&dir, "acc-a", "acc-c").unwrap_err();
        assert!(err.contains("名称已存在"), "对方空显示名兜底为 id 后参与重名比较: {err}");

        // 改成与自身当前显示名相同（重名检查排除自身）→ 允许
        assert!(rename_account_core(&dir, "acc-a", "甲").is_ok());
    }

    // ---------- 自动建号（R2）----------

    #[test]
    fn auto_create_skips_without_credentials() {
        let dir = temp_data_dir("auto_skip");
        // user 为空
        assert!(!auto_create_account_in(&dir, &sample_account("", "pwd", "")).unwrap());
        // password 为空
        assert!(!auto_create_account_in(&dir, &sample_account("user", "", "")).unwrap());
        assert!(list_account_items(&dir).is_empty(), "无凭据不得建号");
    }

    #[test]
    fn auto_create_new_account_uses_raw_user_as_display_name() {
        let dir = temp_data_dir("auto_new");
        let config = sample_account("2023@stu.wxxy.edu.cn", "pwd", "");
        assert!(auto_create_account_in(&dir, &config).unwrap(), "新建档案算实际写盘");

        // id = sanitize(user)：@ 与 . 替换为 _
        let items = list_account_items(&dir);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "2023_stu_wxxy_edu_cn");
        // displayName = 原始 user 文本（未 sanitize）
        assert_eq!(items[0].display_name, "2023@stu.wxxy.edu.cn");
        // 账号文件内 active_account 字段 = id
        let loaded = load_account_config(&dir, "2023_stu_wxxy_edu_cn").unwrap();
        assert_eq!(loaded.active_account, "2023_stu_wxxy_edu_cn");
        assert_eq!(loaded.password, "pwd");
    }

    #[test]
    fn auto_create_is_idempotent_no_write_when_identical() {
        let dir = temp_data_dir("auto_idem");
        let config = sample_account("user01", "pwd", "");
        assert!(auto_create_account_in(&dir, &config).unwrap(), "首次创建算写盘");

        let path = get_account_path(&dir, "user01");
        let before_mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let before_bytes = std::fs::read(&path).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(!auto_create_account_in(&dir, &config).unwrap(), "幂等短路不算写盘");

        let after_mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before_mtime, after_mtime, "内容一致时必须幂等短路，不得写盘");
        assert_eq!(before_bytes, std::fs::read(&path).unwrap());
    }

    #[test]
    fn auto_create_updates_credentials_but_keeps_display_name() {
        let dir = temp_data_dir("auto_diff");
        let config = sample_account("user01", "pwd-old", "");
        auto_create_account_in(&dir, &config).unwrap();

        // 用户先自定义了显示名（模拟 rename_account 之后）
        let mut customized = load_account_config(&dir, "user01").unwrap();
        customized.display_name = "我的专属名".to_string();
        save_account_config(&dir, "user01", &customized).unwrap();

        // 修改密码后保存 → 只更新 user/password/operator，displayName 保留
        let changed = sample_account("user01", "pwd-new", "");
        assert!(auto_create_account_in(&dir, &changed).unwrap(), "差异更新算实际写盘");

        let loaded = load_account_config(&dir, "user01").unwrap();
        assert_eq!(loaded.password, "pwd-new", "有差异时更新凭据");
        assert_eq!(loaded.display_name, "我的专属名", "不得覆盖用户自定义显示名");
        assert_eq!(loaded.operator, "校园", "operator 属比对更新字段");
    }

    #[test]
    fn auto_create_all_illegal_chars_still_creates_account() {
        // 锁定全非法字符行为：id 为全下划线，仍会建号（契约行为，
        // 与 sanitize_account_id_rules 用例呼应）
        let dir = temp_data_dir("auto_illegal");
        let config = sample_account("@@#", "pwd", "");
        assert!(auto_create_account_in(&dir, &config).unwrap());
        let items = list_account_items(&dir);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "___");
    }

    /// F7 撞库防护：不同 user 原文 sanitize 出同一 id（2023.1 与 2023/1 同为
    /// 2023_1）时，后到的输入必须跳过并告警，不得覆盖既有账号的凭据
    #[test]
    fn auto_create_skips_on_id_collision_with_different_raw_user() {
        let dir = temp_data_dir("auto_collision");
        auto_create_account_in(&dir, &sample_account("2023.1", "pwd-old", "")).unwrap();
        let path = get_account_path(&dir, "2023_1");
        let before = std::fs::read(&path).unwrap();

        assert!(
            !auto_create_account_in(&dir, &sample_account("2023/1", "pwd-new", "")).unwrap(),
            "撞库命中不得算作写盘"
        );
        assert_eq!(before, std::fs::read(&path).unwrap(), "撞库时不得覆盖既有账号文件");
        let loaded = load_account_config(&dir, "2023_1").unwrap();
        assert_eq!(loaded.user, "2023.1", "既有账号的 user 原文保持不变");
        assert_eq!(loaded.password, "pwd-old", "既有账号的密码不被覆盖");
    }

    /// F1 路径穿越防护：非法 id（../config、a/b）在核心层即被拒绝，
    /// 磁盘上不产生新文件、既有文件不被修改
    #[test]
    fn rename_account_core_rejects_illegal_id_without_disk_writes() {
        let dir = temp_data_dir("ren_trav");
        save_account_config(&dir, "acc-a", &sample_account("user-a", "pwd-a", "甲")).unwrap();
        let target = get_account_path(&dir, "acc-a");
        let before_bytes = std::fs::read(&target).unwrap();
        let before_count = std::fs::read_dir(&dir).unwrap().count();

        assert!(rename_account_core(&dir, "../config", "新名").is_err(), "相对路径穿越必须拒绝");
        assert!(rename_account_core(&dir, "a/b", "新名").is_err(), "路径分隔符必须拒绝");
        assert_eq!(before_bytes, std::fs::read(&target).unwrap(), "既有账号文件不得被改动");
        assert_eq!(before_count, std::fs::read_dir(&dir).unwrap().count(), "不得产生新文件");
    }

    /// F3 另存账号宽松回退：目标档案密码字段为非法密文（解密失败）时，
    /// 另存不硬错误，改用当前配置整档覆盖——用户可借"另存"自救
    #[test]
    fn save_as_falls_back_to_current_config_on_decrypt_failure() {
        let dir = temp_data_dir("saveas_decrypt");
        let accounts_dir = persist::get_accounts_dir(&dir);
        std::fs::create_dir_all(&accounts_dir).unwrap();
        std::fs::write(
            accounts_dir.join("broken.json"),
            r#"{"user":"broken-user","password":"not-a-valid-cipher"}"#,
        )
        .unwrap();
        // 前置：非法密文确实导致解密失败（错误分类为"账号密码解密失败"）
        assert_eq!(
            persist::load_account_config(&dir, "broken").unwrap_err(),
            "账号密码解密失败"
        );

        let account_data = sample_account("user-a", "pwd-a", "甲");
        let merged = merge_save_as_target(persist::load_account_config(&dir, "broken"), &account_data)
            .expect("解密失败必须走宽松回退而非硬错误");
        assert_eq!(merged.user, "user-a", "解密失败时用当前配置覆盖");
        assert_eq!(merged.password, "pwd-a");
        assert_eq!(merged.display_name, "甲");
    }

    /// 对照组：JSON 结构损坏（解析失败）仍硬错误，与旧实现 from_str 失败语义一致
    #[test]
    fn save_as_hard_fails_on_corrupt_json() {
        let dir = temp_data_dir("saveas_json");
        let accounts_dir = persist::get_accounts_dir(&dir);
        std::fs::create_dir_all(&accounts_dir).unwrap();
        std::fs::write(accounts_dir.join("corrupt.json"), "{not json").unwrap();
        let account_data = sample_account("user-a", "pwd-a", "");
        assert!(merge_save_as_target(persist::load_account_config(&dir, "corrupt"), &account_data).is_err());
    }

    /// F3 主路径锁：档案已存在时保留自定义显示名等非登录字段；
    /// 密码由命令体外层经 password_for_encrypt 统一覆盖，merge 本身不动它
    #[test]
    fn save_as_keeps_existing_display_name_when_overwriting() {
        let dir = temp_data_dir("saveas_keep");
        save_account_config(&dir, "b", &sample_account("user-b", "pwd-old", "B的自定义名")).unwrap();

        let account_data = sample_account("user-b", "pwd-new", "");
        let merged = merge_save_as_target(persist::load_account_config(&dir, "b"), &account_data).unwrap();
        assert_eq!(merged.display_name, "B的自定义名", "覆盖写同 id 账号保留档案自定义显示名");
        assert_eq!(merged.password, "pwd-old", "merge 不动密码，由调用方统一覆盖");
        assert_eq!(merged.operator, "校园", "operator 属登录字段，用当前配置覆盖");
    }

    // ---------- switch_account 合并规则 ----------

    #[test]
    fn merge_account_fields_and_exclude_device_level_binding() {
        let mut current = Config::default();
        current.adapter1_account = "kept-1".to_string();
        current.adapter2_account = "kept-2".to_string();
        current.display_name = "旧显示名".to_string();

        let account = sample_account("u2", "p2", "");
        merge_account_into_config(&mut current, &account, "acc-2");

        // 6 个登录字段 + active_account
        assert_eq!(current.user, "u2");
        assert_eq!(current.password, "p2");
        assert_eq!(current.operator, "校园");
        assert_eq!(current.active_account, "acc-2");
        // display_name 为空 → 兜底账号 id
        assert_eq!(current.display_name, "acc-2");
        // 设备级配置不得被账号档案覆盖
        assert_eq!(current.adapter1_account, "kept-1", "切账号不得改变主适配器绑定");
        assert_eq!(current.adapter2_account, "kept-2", "切账号不得改变副适配器绑定");
    }

    #[test]
    fn merge_account_uses_account_display_name_when_present() {
        let mut current = Config::default();
        current.display_name = "旧显示名".to_string();
        let account = sample_account("u2", "p2", "自定义名");
        merge_account_into_config(&mut current, &account, "acc-2");
        assert_eq!(current.display_name, "自定义名");
    }

    /// R4 契约锁：switch_account 成功分支改走 ok_with_account 后，
    /// activeAccount 必须出现在返回值（Some），前端守卫才不会失效
    #[test]
    fn switch_success_result_carries_active_account() {
        let config = Config::default();
        let result = AccountResult::ok_with_account("acc-1".to_string(), config);
        assert_eq!(result.active_account, Some("acc-1".to_string()));

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["activeAccount"], "acc-1", "activeAccount 必须出现在 IPC 返回值中");

        // 对照：err 返回不含 activeAccount（skip_serializing_if）
        let err = AccountResult::err("x");
        assert_eq!(err.active_account, None);
        let err_json = serde_json::to_value(&err).unwrap();
        assert!(err_json.get("activeAccount").is_none());
    }

    /// IPC 出站形状锁：AccountItem 序列化为 { id, displayName }
    #[test]
    fn account_item_json_shape() {
        let item = AccountItem { id: "acc-1".to_string(), display_name: "显示名".to_string() };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["id"], "acc-1");
        assert_eq!(json["displayName"], "显示名");
        assert!(json.get("display_name").is_none(), "不得出现 snake_case 冗余键");
    }
}
