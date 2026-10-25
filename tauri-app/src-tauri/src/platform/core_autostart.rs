//! 未登录 Windows 时的系统级自动登录（无头核心）——GUI 侧供给与开关
//!
//! 现有「开机自启」是 HKCU Run 键，用户登录后才启动。本模块把当前账号凭据
//! 组装成核心配置（密码用 **机器作用域 DPAPI** 加密——用户作用域密文 SYSTEM
//! 进程解不开），经现有提权通道（`helper_spawn`/`task_proxy` op 协议）交给
//! SYSTEM worker：
//! - 写 `%ProgramData%\CampusLoginCore\core-config.json` 并收紧 ACL 到
//!   SYSTEM+Administrators；
//! - 注册计划任务 `CampusLoginCoreBoot`（BOOT 触发 + SYSTEM + Highest），
//!   开机即拉起同目录的 `campus-login-core.exe` 完成登录（见 `src/bin/`）。
//!
//! 安全面如实告知用户：机器作用域密文本机管理员可解密（DPAPI 机器作用域的
//! 固有属性），文件 ACL 只是访问控制而非保密边界。

use crate::config::model::{Config, AUTO_DETECT_ADAPTER};

/// 系统级启动计划任务名（根文件夹下）。
pub const CORE_TASK_NAME: &str = "CampusLoginCoreBoot";
/// 无头核心二进制文件名（与桌面主程序同目录发布）。
pub const CORE_EXE_NAME: &str = "campus-login-core.exe";

/// 核心配置目录（`%ProgramData%\CampusLoginCore`，与提权代理目录平级）。
pub fn core_dir() -> std::path::PathBuf {
    crate::helper::proxy_base_dir()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
        .join("CampusLoginCore")
}

/// 核心配置文件路径。
pub fn core_config_path() -> std::path::PathBuf {
    core_dir().join("core-config.json")
}

/// 核心日志文件路径（core 自行追加写）。
#[allow(dead_code)] // 供无头核心 bin 使用，主 bin 不引用
pub fn core_log_path() -> std::path::PathBuf {
    core_dir().join("core.log")
}

/// 无头核心 exe 路径（主程序同目录）。
pub fn core_exe_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取当前exe路径失败: {e}"))?;
    Ok(exe
        .parent()
        .ok_or("当前exe无父目录")?
        .join(CORE_EXE_NAME))
}

/// 无头核心所需的精简配置（`core-config.json` 的模型）。
///
/// 只拷贝登录真正需要的字段；密码为机器作用域 DPAPI 密文（base64）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CoreConfig {
    pub user: String,
    /// 机器作用域 DPAPI 密文（base64），core 以 CRYPTPROTECT_LOCAL_MACHINE 产出可解
    #[serde(rename = "passwordEnc")]
    pub password_enc: String,
    pub operator: String,
    #[serde(default)]
    pub portal_url: String,
    #[serde(default)]
    pub adapter1: String,
    #[serde(default)]
    pub adapter2: String,
    #[serde(default)]
    pub dual_adapter: bool,
    /// 校园检查静默窗（分钟数，0 = 无窗口，语义同 monitor::campus_check::is_campus_check_silent）
    #[serde(default)]
    pub campus_check_start_minutes: u16,
    #[serde(default)]
    pub campus_check_end_minutes: u16,
}

/// 从内存明文配置组装核心配置：密码在此时此刻用机器作用域 DPAPI 加密。
/// user/password 为空返回 Err（开关打开前由调用方提示补全）。
pub fn build_core_config(cfg: &Config) -> Result<CoreConfig, String> {
    if cfg.user.trim().is_empty() {
        return Err("账号为空，无法开启未登录自动登录".to_string());
    }
    if cfg.password.is_empty() {
        return Err("密码为空，无法开启未登录自动登录".to_string());
    }
    let password_enc = crate::account::crypto::encrypt_machine(&cfg.password)?;
    Ok(CoreConfig {
        user: cfg.user.clone(),
        password_enc,
        operator: cfg.operator.clone(),
        portal_url: cfg.portal_url.clone(),
        adapter1: cfg.adapter1.clone(),
        adapter2: cfg.adapter2.clone(),
        dual_adapter: cfg.dual_adapter,
        campus_check_start_minutes: cfg.campus_check_start_minutes,
        campus_check_end_minutes: cfg.campus_check_end_minutes,
    })
}

/// 核心配置序列化为 JSON 再 base64（作为提权 op 的单个位置参数：
/// base64 字符集不含空格/引号/`--` 前缀，命令行与请求文件协议双通道安全）。
pub fn encode_core_config_arg(core: &CoreConfig) -> Result<String, String> {
    let json = serde_json::to_string(core).map_err(|e| format!("序列化核心配置失败: {e}"))?;
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        json.as_bytes(),
    ))
}

/// 提权 worker 侧反解（helper 收到的是 base64 位置参数）。
pub fn decode_core_config_arg(arg: &str) -> Result<CoreConfig, String> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, arg)
        .map_err(|e| format!("核心配置Base64解码失败: {e}"))?;
    let json = String::from_utf8(bytes).map_err(|e| format!("核心配置UTF8转换失败: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("核心配置JSON解析失败: {e}"))
}

/// 系统级自启当前是否开启（只读，按计划任务存在性判定，无需提权）。
pub fn core_autostart_enabled() -> bool {
    let output = crate::network::discovery::new_command("schtasks")
        .args(["/query", "/tn", CORE_TASK_NAME])
        .output();
    matches!(&output, Ok(o) if o.status.success())
}

/// 开启系统级自启：组装配置（机器作用域加密）→ 提权 worker 写配置文件 +
/// 注册 BOOT 触发的 SYSTEM 计划任务。全程复用现有三级提权通道（代理优先，
/// 无 UAC；首次注册代理时可能弹一次 UAC，属既有机制）。
pub fn enable_core_autostart(cfg: &Config) -> Result<(), String> {
    let core = build_core_config(cfg)?;
    let arg = encode_core_config_arg(&core)?;
    let result_name = crate::platform::helper_spawn::new_result_name();
    let v = crate::platform::helper_spawn::spawn_elevated_helper(
        "register_boot_task",
        &[arg.as_str()],
        &result_name,
        std::time::Duration::from_secs(90),
        true,
    )?;
    if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        Err(v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("注册系统级自启结果缺少 message")
            .to_string())
    }
}

/// 关闭系统级自启：提权 worker 删除计划任务 + 配置文件（密码密文随配置一并清除）。
pub fn disable_core_autostart() -> Result<(), String> {
    let result_name = crate::platform::helper_spawn::new_result_name();
    let v = crate::platform::helper_spawn::spawn_elevated_helper(
        "remove_boot_task",
        &[],
        &result_name,
        std::time::Duration::from_secs(90),
        true,
    )?;
    if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        Err(v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("移除系统级自启结果缺少 message")
            .to_string())
    }
}

/// 无头核心凭据跟随：配置落盘后检测核心关心的关键字段（账号/密码/运营商/
/// 门户/适配器/静默窗）是否与上次写入 core-config.json 时一致，不一致则重写
/// 配置并重注册任务（COM 注册幂等覆盖）。指纹文件存用户配置目录（仅 FNV-1a
/// 摘要，不含明文）。账号或密码为空时无法重建核心配置，跳过（保持旧配置，
/// 待用户在设置页重新保存完整凭据）。
pub fn refresh_core_config_if_changed(app_handle: &tauri::AppHandle, cfg: &Config) -> Option<Result<(), String>> {
    if !core_autostart_enabled() {
        return None;
    }
    if cfg.user.trim().is_empty() || cfg.password.is_empty() {
        return None;
    }
    let fingerprint = core_config_fingerprint(cfg);
    let marker = core_config_marker_path(app_handle);
    if let Ok(prev) = std::fs::read_to_string(&marker) {
        if prev.trim() == fingerprint {
            return None;
        }
    }
    crate::log_info!("core_autostart", "核心凭据关键字段变化，重写无头核心配置");
    let result = enable_core_autostart(cfg);
    if result.is_ok() {
        let _ = std::fs::write(&marker, &fingerprint);
    }
    Some(result)
}

/// 凭据指纹文件路径（用户配置目录，非 ProgramData——用户态可写）。
fn core_config_marker_path(app_handle: &tauri::AppHandle) -> std::path::PathBuf {
    crate::config::persist::get_data_dir(app_handle).join("core-config-fingerprint.txt")
}

/// 开启系统级自启成功后写入指纹，避免下一次 save_config 因指纹缺失多做一次重写。
pub fn write_core_config_marker(app_handle: &tauri::AppHandle, cfg: &Config) {
    let _ = std::fs::write(core_config_marker_path(app_handle), core_config_fingerprint(cfg));
}

/// 关闭系统级自启后清除指纹（下次重新开启时会重新写入）。
pub fn clear_core_config_marker(app_handle: &tauri::AppHandle) {
    let _ = std::fs::remove_file(core_config_marker_path(app_handle));
}

/// 核心配置关键字段指纹（FNV-1a 64）：仅做变更检测，非安全用途。
/// 密码以明文参与摘要——DPAPI 机器作用域密文每次加密都不同，不能直接比密文。
fn core_config_fingerprint(cfg: &Config) -> String {
    let parts: [&str; 10] = [
        "v1",
        &cfg.user,
        &cfg.password,
        &cfg.operator,
        &cfg.portal_url,
        &cfg.adapter1,
        &cfg.adapter2,
        &format!("{}", cfg.dual_adapter),
        &format!("{}", cfg.campus_check_start_minutes),
        &format!("{}", cfg.campus_check_end_minutes),
    ];
    let src = parts.join("");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in src.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}
/// core 选卡：dual_adapter 时按 adapter1/adapter2 名单匹配；否则取任一有 IP 的适配器。
/// 「自动检测」/空名不参与名单匹配（交给自动检测语义）。
#[allow(dead_code)] // 供无头核心 bin 使用，主 bin 不引用
pub fn select_core_adapters(
    all: &[crate::network::Adapter],
    adapter1: &str,
    adapter2: &str,
    dual_adapter: bool,
) -> Vec<crate::network::Adapter> {
    let named = |n: &str| {
        !n.is_empty() && n != AUTO_DETECT_ADAPTER
    };
    let mut picked: Vec<crate::network::Adapter> = Vec::new();
    if dual_adapter || named(adapter1) || named(adapter2) {
        for name in [adapter1, adapter2] {
            if named(name) {
                if let Some(a) = all.iter().find(|a| a.name == name && !a.ip.is_empty()) {
                    picked.push(a.clone());
                }
            }
        }
    }
    if picked.is_empty() {
        // 未命中名单（或自动检测）：退化为任意有 IP 的适配器
        picked = all
            .iter()
            .filter(|a| !a.ip.is_empty())
            .cloned()
            .collect();
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> Config {
        let mut cfg = Config::default();
        cfg.user = "20230001".to_string();
        cfg.password = "plain-pw".to_string();
        cfg.operator = "@telecom".to_string();
        cfg
    }

    /// 组装 + base64 往返（机器作用域 DPAPI 在测试进程内可直接跑）。
    #[test]
    #[cfg(target_os = "windows")]
    fn core_config_roundtrip_through_base64() {
        let cfg = base_config();
        let core = build_core_config(&cfg).unwrap();
        assert_ne!(core.password_enc, cfg.password, "密文不得等于明文");
        let arg = encode_core_config_arg(&core).unwrap();
        assert!(!arg.contains(' ') && !arg.contains('"'), "base64 参数须命令行安全");
        let decoded = decode_core_config_arg(&arg).unwrap();
        assert_eq!(decoded, core);
        // 密文可被机器作用域解密回明文（core 进程路径）
        assert_eq!(crate::account::crypto::decrypt_machine(&core.password_enc).unwrap(), "plain-pw");
    }

    /// 账号/密码缺失时拒绝组装。
    #[test]
    fn core_config_requires_credentials() {
        let mut cfg = base_config();
        cfg.password = String::new();
        assert!(build_core_config(&cfg).is_err());
        let mut cfg2 = base_config();
        cfg2.user = String::new();
        assert!(build_core_config(&cfg2).is_err());
    }

    /// 选卡：名单命中优先、空名单退化为任意有 IP 适配器。
    #[test]
    fn select_core_adapters_prefers_named() {
        let mk = |name: &str, ip: &str| crate::network::Adapter {
            name: name.to_string(),
            ip: ip.to_string(),
            wireless: false,
            guid: String::new(),
            mac: String::new(),
            if_index: 0,
            status: crate::network::discovery::AdapterStatus::Connected,
            link_speed: 0,
        };
        let all = vec![mk("以太网", "10.1.0.5"), mk("WLAN", "192.168.1.2"), mk("空IP", "")];
        let picked = select_core_adapters(&all, "WLAN", "以太网", true);
        assert_eq!(picked.len(), 2);
        // 单卡 + 自动检测：退化为全部有 IP 适配器
        let auto = select_core_adapters(&all, AUTO_DETECT_ADAPTER, "", false);
        assert_eq!(auto.len(), 2);
        // 名单存在但未命中（未插卡）：同样退化
        let miss = select_core_adapters(&all, "不存在的卡", "", false);
        assert_eq!(miss.len(), 2);
    }
}
