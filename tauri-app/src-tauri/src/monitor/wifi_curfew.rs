//! WiFi 就寝守则执行器（桌面）。
//!
//! 窗口判定在 [`crate::config::wifi_curfew`]（双端共用的纯函数）；本模块只做
//! Windows 侧动作：把被阻断 SSID 的 WLAN profile 连接模式改成「手动」，并断开
//! 当前连接；出窗（或功能关闭）时按快照逐条还原成「自动」。
//!
//! 为什么改 profile 而不是每拍 `netsh wlan disconnect` 了事：系统 WLAN AutoConfig
//! 会在断线后立刻自动重连，光断开等于每 30s 踢一次用户、还残留半分钟在线。
//! 把这个 SSID 的连接模式改成手动后系统不再自动连，恢复时段再设回——用户名下
//! 其余 WiFi 的配置一概不碰。
//!
//! 快照存在配置里（`wifiCurfewProfileRestore`）：程序退出/重启后仍能还原，
//! 与夜切的跃点快照同一模式（只记本功能亲手改过的 SSID，不按当前状态推断）。

use crate::config::Config;
use tauri::{AppHandle, Manager};

/// 本功能亲手改过连接模式的 profile 及改动前的模式。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ProfileSnapshot {
    ssid: String,
    /// 改动前模式："auto" | "manual"
    mode: String,
}

/// 解析快照。字段来自磁盘配置（用户可手工编辑，属系统边界），解析失败只记
/// 告警并按空处理——损坏的快照最多让 profile 停在手动，不会阻断主流程。
fn parse_snapshot(raw: &str) -> Vec<ProfileSnapshot> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            crate::log_warn!("wifi_curfew", "就寝守则快照解析失败({e})，按空处理: {raw}");
            Vec::new()
        }
    }
}

fn snapshot_json(entries: &[ProfileSnapshot]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    serde_json::to_string(entries).unwrap_or_default()
}

/// 解码并合并 netsh 的 stdout/stderr，给失败信息用
fn netsh_detail(output: &std::process::Output) -> String {
    let mut text = crate::platform::console_output::decode_console_bytes(&output.stdout);
    text.push_str(&crate::platform::console_output::decode_console_bytes(&output.stderr));
    text.trim().to_string()
}

/// 已保存的 WLAN profile 名列表（用户配置文件，含组策略之外的普通配置）。
pub(crate) fn list_profiles() -> Result<Vec<String>, String> {
    let output = crate::network::discovery::new_command("netsh")
        .args(["wlan", "show", "profiles"])
        .output()
        .map_err(|e| format!("调用 netsh 失败: {e}"))?;
    if !output.status.success() {
        return Err(netsh_detail(&output));
    }
    let text = crate::platform::console_output::decode_console_bytes(&output.stdout);
    let mut names: Vec<String> = Vec::new();
    // 多块无线网卡时 netsh 会按接口分段列出，同一 SSID 可能重复出现；
    // 名单是勾选用的，重复条目只会让人以为有多条配置
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // 中英两版 netsh 输出的行前缀
        let rest = if let Some(r) = trimmed.strip_prefix("所有用户配置文件") {
            r
        } else if let Some(r) = trimmed.strip_prefix("All User Profile") {
            r
        } else {
            continue;
        };
        if let Some(colon) = rest.find(':') {
            let name = rest[colon + 1..].trim();
            if !name.is_empty() && seen.insert(name.to_lowercase()) {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// 读取某个 profile 当前的连接模式："auto" | "manual"；读不到返回 None。
fn read_connection_mode(ssid: &str) -> Result<Option<String>, String> {
    let output = crate::network::discovery::new_command("netsh")
        .args(["wlan", "show", "profile", &format!("name={ssid}")])
        .output()
        .map_err(|e| format!("调用 netsh 失败: {e}"))?;
    if !output.status.success() {
        return Err(netsh_detail(&output));
    }
    let text = crate::platform::console_output::decode_console_bytes(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        let rest = if let Some(r) = trimmed.strip_prefix("Connection mode") {
            r
        } else if let Some(r) = trimmed.strip_prefix("连接模式") {
            r
        } else {
            continue;
        };
        let Some(colon) = rest.find(':') else { continue };
        let value = rest[colon + 1..].trim().to_lowercase();
        if value.contains("auto") || value.contains("自动") {
            return Ok(Some("auto".to_string()));
        }
        if value.contains("manual") || value.contains("手动") {
            return Ok(Some("manual".to_string()));
        }
    }
    Ok(None)
}

fn set_connection_mode(ssid: &str, mode: &str) -> Result<(), String> {
    let output = crate::network::discovery::new_command("netsh")
        .args([
            "wlan",
            "set",
            "profileparameter",
            &format!("name={ssid}"),
            &format!("connectionmode={mode}"),
        ])
        .output()
        .map_err(|e| format!("调用 netsh 失败: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(netsh_detail(&output))
    }
}

/// 连到指定 profile（name 即 SSID；profile 必须已保存）。
fn connect_profile(ssid: &str) -> Result<(), String> {
    let output = crate::network::discovery::new_command("netsh")
        .args(["wlan", "connect", &format!("name={ssid}")])
        .output()
        .map_err(|e| format!("调用 netsh 失败: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(netsh_detail(&output))
    }
}

/// 断开后切到放行名单时的候选轮转下标。
///
/// `netsh wlan connect` 只是"请求已提交"，真正连上要几秒——若每拍都死磕
/// 同一个连不上的网络（信号弱/密码错），名单里的第二个就永远轮不到。
/// 每拍前进一格，窗口结束时归零。
static CONNECT_CURSOR: std::sync::Mutex<usize> = std::sync::Mutex::new(0);

/// 放行名单里那些「系统已保存、可以直接连」的候选，保持名单顺序。
fn connect_candidates(saved: &[String], allowed: &[String]) -> Vec<String> {
    allowed
        .iter()
        .filter(|a| a.as_str() != "*")
        .filter(|a| saved.iter().any(|s| s.eq_ignore_ascii_case(a)))
        .cloned()
        .collect()
}

fn disconnect_current() -> Result<(), String> {
    let output = crate::network::discovery::new_command("netsh")
        .args(["wlan", "disconnect"])
        .output()
        .map_err(|e| format!("调用 netsh 失败: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(netsh_detail(&output))
    }
}

/// 单拍执行。`now_minutes`/`weekday` 由调用方按本地时间给出（与夜切同一份换算）。
///
/// 阻塞调用（每次最多 4 个 netsh 子进程），调用方须走阻塞线程池。
pub fn tick(app_handle: &AppHandle, config: &Config, weekday: u32, now_minutes: u32) {
    let in_window = crate::config::wifi_curfew::in_curfew_window(
        weekday,
        now_minutes,
        config.night_restore_minutes,
    );
    // 提前准备：切换时刻前 night_prepare_minutes 分钟内就先动手，让到点那一刻
    // 只差出站与跃点。出站切换本身仍按原时刻执行（见 monitor::scheduled）
    let preparing = crate::config::wifi_curfew::in_prepare_window(
        weekday,
        now_minutes,
        config.night_prepare_minutes,
        config.night_restore_minutes,
    );
    let active = config.enable_wifi_curfew && (in_window || preparing);
    if preparing && !in_window {
        crate::log_info!(
            "wifi_curfew",
            "就寝守则: 进入提前准备窗口（切换前 {} 分钟），先把 WLAN 切到放行网络",
            crate::config::wifi_curfew::prepare_lead_minutes(config.night_prepare_minutes)
        );
    }
    let mut snapshot = parse_snapshot(&config.wifi_curfew_profile_restore);

    if !active {
        // 出窗：轮转下标归零，下一晚从名单第一个重新试
        if let Ok(mut cursor) = CONNECT_CURSOR.lock() {
            *cursor = 0;
        }
        if snapshot.is_empty() {
            return;
        }
        restore_all(app_handle, &snapshot);
        return;
    }

    // 窗口内：先确保排除列表里「已保存」的 profile 全部改成手动（覆盖跨设备
    // 切换、用户手动连回的场景），再断开当前连接
    let profiles = match list_profiles() {
        Ok(v) => v,
        Err(e) => {
            crate::log_warn!("wifi_curfew", "读取 WLAN 配置列表失败，本拍跳过: {e}");
            return;
        }
    };
    let mut changed = false;
    for ssid in &profiles {
        if !crate::config::wifi_curfew::ssid_blocked(
            &config.wifi_curfew_excluded,
            &config.wifi_curfew_allowed,
            ssid,
        ) {
            continue;
        }
        if snapshot.iter().any(|entry| &entry.ssid == ssid) {
            continue;
        }
        match read_connection_mode(&ssid) {
            Ok(Some(mode)) if mode == "manual" => {
                // 用户本来就设了手动：不需要改，也不必进快照（我们不负责还原）
                continue;
            }
            Ok(Some(mode)) => {
                if let Err(e) = set_connection_mode(&ssid, "manual") {
                    crate::log_warn!("wifi_curfew", "把 {ssid} 设为手动连接失败: {e}");
                    continue;
                }
                crate::log_info!("wifi_curfew", "就寝守则: {ssid} 改为手动连接（原 {mode}）");
                snapshot.push(ProfileSnapshot { ssid: ssid.clone(), mode });
                changed = true;
            }
            Ok(None) => {
                crate::log_warn!("wifi_curfew", "读不到 {ssid} 的连接模式，跳过");
            }
            Err(e) => {
                crate::log_warn!("wifi_curfew", "读取 {ssid} 配置失败: {e}");
            }
        }
    }
    if changed {
        persist_snapshot(app_handle, &snapshot);
    }

    // 断开并切到放行名单：断开被阻断的当前连接，再主动连一个放行的网络。
    // 只断开不连接的话，机器整晚没有无线可用（校园网一断，放行名单里那台
    // 宿舍路由器本来是用户在用的，得替他连回去）。
    let current = match crate::network::subnet::get_wireless_ssid() {
        Ok(v) => v,
        Err(e) => {
            // 读不到就不知道要切谁，本拍不动——乱切可能把正在用的放行网络踢掉
            crate::log_warn!("wifi_curfew", "查询当前 WiFi 失败，本拍不切换: {e}");
            return;
        }
    };
    let already_allowed = current
        .as_deref()
        .is_some_and(|s| crate::config::wifi_curfew::ssid_allowed(&config.wifi_curfew_allowed, s));

    if let Some(ssid) = current.as_deref().filter(|_| !already_allowed) {
        match disconnect_current() {
            Ok(()) => crate::log_info!("wifi_curfew", "就寝守则: 已断开 {ssid}"),
            Err(e) => crate::log_warn!("wifi_curfew", "断开 {ssid} 失败: {e}"),
        }
    }

    if already_allowed {
        return;
    }
    let candidates = connect_candidates(&profiles, &config.wifi_curfew_allowed);
    if candidates.is_empty() {
        // 没有可连的放行网络：只断不切。名单里的网络必须先在系统里连过一次
        // （有 profile），否则 netsh 连不上，这一点靠日志说清
        if !config.wifi_curfew_allowed.is_empty() {
            crate::log_warn!(
                "wifi_curfew",
                "放行名单里的网络在本机都没有保存过，无法自动连接: {:?}",
                config.wifi_curfew_allowed
            );
        }
        return;
    }
    let index = {
        let mut cursor = CONNECT_CURSOR.lock().unwrap_or_else(|e| e.into_inner());
        let index = *cursor % candidates.len();
        *cursor = cursor.wrapping_add(1);
        index
    };
    let target = &candidates[index];
    match connect_profile(target) {
        Ok(()) => crate::log_info!("wifi_curfew", "就寝守则: 已请求连接放行网络 {target}"),
        Err(e) => crate::log_warn!("wifi_curfew", "连接放行网络 {target} 失败: {e}"),
    }
}

/// 按快照逐条还原连接模式，成功后清空快照。单条失败保留快照等下一拍重试。
fn restore_all(app_handle: &AppHandle, snapshot: &[ProfileSnapshot]) {
    let mut remaining = Vec::new();
    for entry in snapshot {
        match set_connection_mode(&entry.ssid, &entry.mode) {
            Ok(()) => crate::log_info!(
                "wifi_curfew",
                "就寝守则: {} 还原为 {}",
                entry.ssid,
                entry.mode
            ),
            Err(e) => {
                crate::log_warn!("wifi_curfew", "还原 {} 连接模式失败: {e}", entry.ssid);
                remaining.push(entry.clone());
            }
        }
    }
    if remaining.len() != snapshot.len() {
        persist_snapshot(app_handle, &remaining);
    }
}

/// 把快照写回配置并落盘；落盘失败只记日志——本拍的连接模式已经改完，
/// 快照丢失的后果是下次启动少了还原依据，不该打断守则判定
fn persist_snapshot(app_handle: &AppHandle, entries: &[ProfileSnapshot]) {
    let json = snapshot_json(entries);
    let state = app_handle.state::<crate::infra::state::AppState>();
    let result = crate::config::persist::mutate_config_and_save(app_handle, &state, |config| {
        config.wifi_curfew_profile_restore = json;
        Ok(())
    });
    if let Err(e) = result {
        crate::log_warn!("wifi_curfew", "就寝守则快照落盘失败: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_keep_order_and_drop_unknown() {
        let saved = vec!["Dorm-5G".to_string(), "campus".to_string()];
        let allowed = vec![
            "not-saved".to_string(),
            "dorm-5g".to_string(),
            "*".to_string(),
            "campus".to_string(),
        ];
        // 没保存过的连不上、`*` 不是网络名：都跳过，顺序按放行名单
        assert_eq!(connect_candidates(&saved, &allowed), vec!["dorm-5g".to_string(), "campus".to_string()]);
        assert!(connect_candidates(&saved, &[]).is_empty());
    }

    #[test]
    fn snapshot_roundtrip() {
        let entries = vec![
            ProfileSnapshot { ssid: "i-wxxy".to_string(), mode: "auto".to_string() },
            ProfileSnapshot { ssid: "iwxxy-2".to_string(), mode: "manual".to_string() },
        ];
        let json = snapshot_json(&entries);
        assert_eq!(parse_snapshot(&json), entries);
    }

    #[test]
    fn empty_snapshot_serializes_to_empty_string() {
        assert_eq!(snapshot_json(&[]), "");
        assert!(parse_snapshot("").is_empty());
    }

    #[test]
    fn corrupt_snapshot_is_ignored() {
        assert!(parse_snapshot("not json").is_empty());
    }
}
