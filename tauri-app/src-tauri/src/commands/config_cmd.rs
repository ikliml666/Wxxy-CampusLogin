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

    // 统一发射 config-changed 事件：必须掩码后再发射，避免泄露真实密码
    // （所有调用方 save_config/switch_account/set_auto_launch 等都经此路径通知前端）
    let emit_cfg = config.masked_for_display();
    let _ = app_handle.notify_config_changed(&emit_cfg);
    // 托盘菜单依赖账号配置（「快速注销」可用性取决于 user 是否已配置、「切换账号」
    // 依赖 active_account 标记），配置落盘后异步重建菜单
    crate::app::tray::refresh_tray_menu_state(app_handle);
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

/// 组装配置导出 payload（纯函数，便于单测锁定"明文不出站"）。
/// - include_password=false：掩码态出站（敏感信息出站唯一出口 masked_for_display）
/// - include_password=true：密码字段以 DPAPI 加密态密文出站（内存明文重新 encrypt），
///   wrapper 带 passwordEncrypted 标志供导入端区分；绝不写明文。
///   空串或 MASK 占位视为"无已存密码"，写空串（MASK 语义：真实密码不会是 "***"，
///   与 save_config 的占位符语义一致）。
fn build_config_export_payload(current: &Config, include_password: bool) -> Result<serde_json::Value, String> {
    let config = if include_password {
        let mut c = current.clone();
        for pwd in [&mut c.password, &mut c.self_password] {
            if pwd.is_empty() || *pwd == crate::config::model::PASSWORD_MASK {
                pwd.clear();
            } else {
                *pwd = crypto::encrypt(pwd)?;
            }
        }
        c
    } else {
        current.masked_for_display()
    };
    Ok(serde_json::json!({
        "type": "campus-login-config",
        "version": 1,
        "exportedAt": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "appVersion": env!("APP_VERSION"),
        "passwordEncrypted": include_password,
        "config": config,
    }))
}

/// 还原导入文件中的一个密码字段（写盘方责任：MASK 占位不得直接落盘，
/// 见 .codewiki/learnings/mask-placeholder-persisted-as-plaintext）。
/// - 空串 / MASK 占位：保留当前已存值
/// - passwordEncrypted=true：先本机 decrypt 还原明文；失败明确报错（含密码导出仅限
///   本机导入），不静默清空避免"以为导入了密码实际没有"的困惑
/// - 其余：视为明文直传（用户手工编辑的文件）
fn restore_imported_password_field(imported: &mut String, encrypted: bool, current: &str) -> Result<(), String> {
    if imported.is_empty() || imported == crate::config::model::PASSWORD_MASK {
        *imported = current.to_string();
        return Ok(());
    }
    if encrypted {
        *imported = crypto::decrypt(imported)
            .map_err(|e| format!("密码密文解密失败（含密码导出仅限本机导入）: {e}"))?;
    }
    Ok(())
}

/// 导出当前配置到 <data_dir>/exports/config-<时间戳>.json，返回文件路径。
/// 默认不含密码（掩码态）；include_password=true 时密码以本机 DPAPI 密文导出。
#[tauri::command]
pub fn export_config(state: State<'_, AppState>, app_handle: AppHandle, include_password: Option<bool>) -> Result<String, String> {
    let current = state.config.load();
    let payload = build_config_export_payload(&current, include_password.unwrap_or(false))?;
    let json = serde_json::to_string_pretty(&payload).map_err(|e| format!("序列化配置失败: {e}"))?;

    let data_dir = persist::get_data_dir(&app_handle);
    let exports_dir = data_dir.join("exports");
    std::fs::create_dir_all(&exports_dir).map_err(|e| format!("创建导出目录失败: {e}"))?;
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let out_path = exports_dir.join(format!("config-{stamp}.json"));
    std::fs::write(&out_path, json).map_err(|e| format!("写入导出文件失败: {e}"))?;

    crate::log_info!("config", "配置导出成功: {:?}", out_path);
    Ok(out_path.to_string_lossy().to_string())
}

/// 从 JSON 文件导入配置。失败分列三类原因：JSON 解析失败 / 配置校验失败 / 落盘失败
/// （另有密码密文解密失败，见 restore_imported_password_field）。
/// 通过后走与 save_config 完全相同的落盘路径：update_portal_url → set_log_retention_days
/// → save_config_to_disk_encrypted（内含 config-changed 事件）→ 更新内存 ConfigStore。
#[tauri::command]
pub fn import_config(state: State<'_, AppState>, app_handle: AppHandle, path: String) -> Result<CommandResult, String> {
    // 大小防呆：配置文件不应超过 1MB，防止误选超大文件整读进内存
    const MAX_IMPORT_SIZE: u64 = 1024 * 1024;
    let meta = std::fs::metadata(&path).map_err(|e| format!("读取配置文件失败: {e}"))?;
    if meta.len() > MAX_IMPORT_SIZE {
        return Err(format!("配置文件过大（{} 字节），超过 1MB 限制", meta.len()));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取配置文件失败: {e}"))?;

    // 失败分列①：JSON 解析失败
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("配置文件不是合法 JSON: {e}"))?;
    // 兼容两种形态：本应用导出的 wrapper（{type, config, passwordEncrypted}）与裸 Config
    let password_encrypted = value.get("passwordEncrypted").and_then(|b| b.as_bool()).unwrap_or(false);
    let config_value = value.get("config").cloned().unwrap_or(value);
    // 失败分列②：结构不符合 Config（serde 錯誤给出首个不合法字段）
    let mut config: Config = serde_json::from_value(config_value)
        .map_err(|e| format!("配置文件结构无效: {e}"))?;

    // 密码语义还原必须先于严格校验：含密码导出的密码字段是 base64 密文，
    // 长度必然超过 validate_password 的 128 上限，先还原成明文再校验
    let current = state.config.load();
    restore_imported_password_field(&mut config.password, password_encrypted, &current.password)?;
    restore_imported_password_field(&mut config.self_password, password_encrypted, &current.self_password)?;

    // 失败分列③：配置校验失败（严格版，与 save_config 同源）
    let config = match validate_config(config) {
        Ok(c) => c,
        Err(e) => return Ok(CommandResult::err(&format!("配置校验失败: {e}"))),
    };

    // 与 save_config 同路径同顺序：全局 Portal URL → logger 保留天数 → 落盘（含
    // config-changed 事件，掩码后发射）→ 更新内存。落盘失败（分列④）时命令返回 Err
    // 且运行态不变，避免"导入成功但内存未生效"的错位
    crate::network::update_portal_url(&config.portal_url);
    crate::infra::logger::set_log_retention_days(config.log_retention_days);
    save_config_to_disk_encrypted(&app_handle, &config)?;
    state.config.store(config.clone());
    crate::log_info!("config", "配置导入成功, 用户: {}", config.user);

    Ok(CommandResult::ok_msg("配置导入成功"))
}

#[tauri::command]
pub fn show_window(app_handle: AppHandle) -> Result<(), String> {
    crate::app::window::show_and_focus_main(&app_handle);
    Ok(())
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.config.load().masked_for_display())
}

#[tauri::command]
pub fn save_config(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    config: Config,
    clear_password: Option<bool>,
    clear_self_password: Option<bool>,
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
    // 自助服务密码同规则：空/MASK 占位符时保留已保存值（前端仅在用户重输时传新值）；
    // 显式清除（clearSelfPassword，与 clear_password 同语义）跳过兜底直接置空
    if clear_self_password == Some(true) {
        config.self_password = String::new();
    } else if config.self_password.is_empty() || config.self_password == crate::config::model::PASSWORD_MASK {
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
    use super::{build_config_export_payload, restore_imported_password_field};

    /// 出站掩码回归锁：masked_for_display 必须同时掩掉 password 与 self_password
    /// 两个敏感字段（空=未设置语义保留）。历史缺陷：fe000de 修 get_init_data/
    /// get_config 漏掩 self_password 时漏掉了 account 三命令（switch/save_as/
    /// delete），它们经 masked_for_display 组装返回值，明文 selfPassword 随 IPC
    /// 出站到 webview——掩码逻辑收敛到 Config 自身后该类遗漏即被类型锁死。
    #[test]
    fn masked_for_display_masks_both_password_fields() {
        let mut cfg = crate::config::model::Config::default();
        // 空值 = 未设置，保留（前端据此显示"未保存"）
        let masked = cfg.masked_for_display();
        assert_eq!(masked.password, "");
        assert_eq!(masked.self_password, "");
        // 非空（明文）必须双双掩码
        cfg.password = "login-secret".to_string();
        cfg.self_password = "self-secret".to_string();
        let masked = cfg.masked_for_display();
        assert_eq!(masked.password, crate::config::model::PASSWORD_MASK);
        assert_eq!(masked.self_password, crate::config::model::PASSWORD_MASK);
        // 原 struct 不被就地修改
        assert_eq!(cfg.password, "login-secret");
        assert_eq!(cfg.self_password, "self-secret");
    }

    /// 导出安全回归锁（P2-30）：无论 include_password 取值如何，导出 payload 都
    /// 不得包含内存明文密码。掩码态走 masked_for_display；含密码态必须是
    /// DPAPI 密文（本机可解回原值），绝不出现明文字符串。
    #[cfg(target_os = "windows")]
    #[test]
    fn export_payload_never_contains_plaintext_password() {
        let cfg = crate::config::model::Config {
            password: "login-secret".to_string(),
            self_password: "self-secret".to_string(),
            ..Default::default()
        };

        // 掩码态：密码字段为 MASK，无明文
        let masked = build_config_export_payload(&cfg, false).unwrap();
        let masked_str = masked.to_string();
        assert!(!masked_str.contains("login-secret"), "掩码导出泄露明文: {masked_str}");
        assert_eq!(masked["config"]["password"], crate::config::model::PASSWORD_MASK);
        assert_eq!(masked["passwordEncrypted"], false);

        // 含密码态：DPAPI 密文（可解回原值），无明文
        let enc = build_config_export_payload(&cfg, true).unwrap();
        let enc_str = enc.to_string();
        assert!(!enc_str.contains("login-secret"), "含密码导出泄露明文: {enc_str}");
        assert!(!enc_str.contains("self-secret"), "含密码导出泄露自助密码明文: {enc_str}");
        assert_eq!(enc["passwordEncrypted"], true);
        let cipher = enc["config"]["password"].as_str().unwrap();
        assert_eq!(crate::account::crypto::decrypt(cipher).unwrap(), "login-secret");
        let self_cipher = enc["config"]["selfPassword"].as_str().unwrap();
        assert_eq!(crate::account::crypto::decrypt(self_cipher).unwrap(), "self-secret");
    }

    /// 导入密码字段还原语义：空/MASK 保留当前已存值（MASK 占位不得落盘变明文 "***"）；
    /// 密文需 decrypt 还原明文；密文解密失败明确报错而非静默清空。
    #[cfg(target_os = "windows")]
    #[test]
    fn import_password_restore_semantics() {
        // 空 / MASK → 保留当前已存值
        let mut imported = String::new();
        restore_imported_password_field(&mut imported, false, "current-pwd").unwrap();
        assert_eq!(imported, "current-pwd");
        let mut imported = crate::config::model::PASSWORD_MASK.to_string();
        restore_imported_password_field(&mut imported, true, "current-pwd").unwrap();
        assert_eq!(imported, "current-pwd");

        // 密文 → decrypt 还原明文
        let cipher = crate::account::crypto::encrypt("real-pwd").unwrap();
        let mut imported = cipher.clone();
        restore_imported_password_field(&mut imported, true, "current-pwd").unwrap();
        assert_eq!(imported, "real-pwd");

        // 明文直传（未加密导出文件）
        let mut imported = "plain-typed".to_string();
        restore_imported_password_field(&mut imported, false, "current-pwd").unwrap();
        assert_eq!(imported, "plain-typed");

        // 伪造密文 → 明确报错
        let mut imported = "not-a-valid-cipher".to_string();
        assert!(restore_imported_password_field(&mut imported, true, "current-pwd").is_err());
    }
}
