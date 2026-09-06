use tauri::{AppHandle, State};
use crate::infra::command_context::AppHandleExt;
use crate::config::model::Config;
use crate::config::persist;
use crate::config::validate::{validate_config, validate_config_lenient};
use crate::account::crypto;
use crate::infra::state::{AppState, CommandResult};

pub fn save_config_to_disk_encrypted(app_handle: &AppHandle, config: &Config) -> Result<(), String> {
    let data_dir = persist::get_data_dir(app_handle);
    persist::save_config_to_disk_encrypted(&data_dir, config)?;

    // 统一发射 config-changed 事件：必须 mask 密码后再发射，避免泄露加密后的真实密码
    // 所有调用方（save_config/switch_account/set_auto_launch 等）都通过此路径统一通知前端
    let mut emit_cfg = config.clone();
    if !emit_cfg.password.is_empty() {
        emit_cfg.password = crate::config::model::PASSWORD_MASK.to_string();
    }
    mask_self_password(&mut emit_cfg);
    let _ = app_handle.notify_config_changed(&emit_cfg);
    Ok(())
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

    if !config.self_password.is_empty() && config.self_password != crate::config::model::PASSWORD_MASK {
        match crypto::decrypt(&config.self_password) {
            Ok(decrypted) => config.self_password = decrypted,
            Err(e) => {
                crate::log_warn!("config", "自助服务密码解密失败，清除保留其他配置: {}", e);
                config.self_password = String::new();
            }
        }
    }

    Ok(config)
}

pub fn load_config_from_disk_or_default(app_handle: &AppHandle) -> Config {
    match load_config_from_file(app_handle) {
        Ok(config) => validate_config_lenient(config),
        Err(e) => {
            // 解析/解密失败（非"文件不存在"）时把原文件留档，用户数据不全量丢失，
            // 可手工恢复；文件不存在属首次启动，无需备份
            let config_path = crate::config::persist::get_config_path(&crate::config::persist::get_data_dir(app_handle));
            let mut backup_note = String::new();
            if config_path.exists() {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let bak = config_path.with_extension(format!("json.corrupt-{stamp}.bak"));
                backup_note = match std::fs::copy(&config_path, &bak) {
                    Ok(_) => format!("，原文件已备份至 {:?}", bak),
                    Err(be) => format!("，且备份失败: {}", be),
                };
            }
            crate::log_warn!("config", "加载配置失败: {}{}，使用默认配置", e, backup_note);
            Config::default()
        }
    }
}

#[tauri::command]
pub fn show_window(app_handle: AppHandle) -> Result<(), String> {
    crate::app::window::show_and_focus_main(&app_handle);
    Ok(())
}

/// 自助服务密码出站掩码（所有把 Config 发往前端的命令必须调用）：
/// state 内是解密后的明文；空值保留（未设置语义），非空一律替换为 MASK。
/// 历史缺陷：get_init_data/get_config 漏掩码，明文经 IPC 泄露到 webview，
/// 且前端 selfPasswordSaved 永远 false → 重启后密码框显示空、自动验证不弹。
pub fn mask_self_password(cfg: &mut crate::config::model::Config) {
    if !cfg.self_password.is_empty() {
        cfg.self_password = crate::config::model::PASSWORD_MASK.to_string();
    }
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.config.load();
    let mut cfg = config.as_ref().clone();
    cfg.password = crate::config::model::PASSWORD_MASK.to_string();
    mask_self_password(&mut cfg);
    Ok(cfg)
}

#[tauri::command]
pub fn save_config(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    config: Config,
    clear_password: Option<bool>,
) -> Result<CommandResult, String> {
    let validated = match validate_config(config) {
        Ok(c) => c,
        Err(e) => {
            crate::log_warn!("config", "配置验证失败: {}", e);
            return Ok(CommandResult::err(&format!("配置验证失败: {e}")));
        }
    };

    let mut config = validated;
    // 显式清除密码（用户在账号面板点击"清除密码"）：跳过兜底直接置空
    if clear_password == Some(true) {
        config.password = String::new();
    } else if config.password.is_empty() || config.password == crate::config::model::PASSWORD_MASK {
        // 空密码或 mask 占位符：保留当前密码，避免前端未传密码时旧密码被覆盖
        let current = state.config.load();
        config.password = current.password.clone();
    }
    // 自助服务密码同规则：空/MASK 占位符时保留已保存值（前端仅在用户重输时传新值）
    if config.self_password.is_empty() || config.self_password == crate::config::model::PASSWORD_MASK {
        let current = state.config.load();
        config.self_password = current.self_password.clone();
    }

    // 历史缺陷：修改 Portal URL 仅存配置，不更新进程全局 PORTAL_URL，
    // 运行期后台巡检/登录仍用旧地址直到重启。此处持久化前先同步全局。
    crate::network::update_portal_url(&config.portal_url);

    // 日志保留天数同步应用到运行期 logger（否则需重启或重进日志面板才生效）
    crate::infra::logger::set_log_retention_days(config.log_retention_days);

    // 先落盘再更新内存：磁盘失败（满/只读）时命令返回 Err 且运行态不变，
    // 避免"保存失败但内存已生效、重启后回退"的错位
    save_config_to_disk_encrypted(&app_handle, &config)?;
    state.config.store(config.clone());
    crate::log_info!("config", "配置保存成功, 用户: {}", config.user);

    Ok(CommandResult::ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 出站掩码回归锁：非空自助服务密码必须替换为 MASK（空=未设置语义保留）。
    /// 历史缺陷：get_init_data/get_config 漏调掩码，解密后的明文经 IPC 发到
    /// webview（隐私泄露），且前端 selfPasswordSaved 永远 false → 重启后
    /// 密码框显示空、切入自助服务面板的自动 Hello 验证永不触发。
    #[test]
    fn mask_self_password_masks_non_empty_only() {
        let mut cfg = crate::config::model::Config::default();
        // 空值 = 未设置，保留（前端据此显示"未保存"）
        mask_self_password(&mut cfg);
        assert_eq!(cfg.self_password, "");
        // 非空（明文）必须掩码
        cfg.self_password = "plain-secret".to_string();
        mask_self_password(&mut cfg);
        assert_eq!(cfg.self_password, crate::config::model::PASSWORD_MASK);
    }
}
