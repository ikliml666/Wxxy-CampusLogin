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
//! 断开的判据与 manual 化一致，都是 `ssid_blocked`（命中断开名单且不在放行
//! 名单）——只看「不在放行名单」就断会误伤一切未入名单的网络（2026-11-19
//! 修复：名单为空时用户连着的热点被每拍踢一次）。切换到放行网络附带失败
//! 抑制：`netsh wlan connect` 对不可达目标同样会断开当前连接去尝试，每个
//! 候选本窗口只试一次，全部失败即停，连上放行网络或出窗自动重置。
//!
//! 位置服务关闭的机器上 `netsh wlan connect` 被位置隐私管控整条拦截（见
//! [connect_profile] 注释），切换改走 Win32 WlanConnect 直调——profile 模式
//! 不扫描、不在管控清单。`show profiles` / `show profile name=` /
//! `disconnect` 实测不受位置管控影响。
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
///
/// 优先 Win32 WlanConnect 直调：netsh wlan connect 内部先调
/// WlanGetAvailableNetworkList 找目标，位置服务关闭时该 API 被位置隐私管控
/// 拦截（微软 wi-fi-access-location-changes 文档：无位置授权返回
/// ERROR_ACCESS_DENIED，管理员不豁免；2026-11-19 本机实测 netsh connect 直接
/// 失败），而 WlanConnect 不在管控清单——profile 模式不需要扫描列表。
/// FFI 层打不开句柄等失败回落 netsh（位置服务开启的机器上两路等价）。
fn connect_profile(ssid: &str) -> Result<(), String> {
    match connect_profile_via_api(ssid) {
        Ok(()) => Ok(()),
        Err(api_err) => {
            crate::log_debug!("wifi_curfew", "WlanConnect 直调未成({api_err})，回落 netsh");
            connect_profile_via_netsh(ssid)
        }
    }
}

fn connect_profile_via_netsh(ssid: &str) -> Result<(), String> {
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

/// Win32 WlanConnect（profile 模式）直连已保存的无线网络。
///
/// 连接请求是异步受理的：返回成功只代表系统接受了请求，目标不可达时同样
/// 受理成功、随后连接失败——与 netsh 语义一致，由 tick 的 PENDING/FAILED
/// 状态机在下一拍判定真实结果。本函数不做扫描（这正是它不受位置管控的原因）。
fn connect_profile_via_api(ssid: &str) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::NetworkManagement::WiFi::{
        WlanCloseHandle, WlanConnect, WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle,
        WLAN_CONNECTION_PARAMETERS, dot11_BSS_type_any, wlan_connection_mode_profile,
        WLAN_INTERFACE_INFO_LIST,
    };

    unsafe {
        let mut negotiated: u32 = 0;
        let mut handle = windows::Win32::Foundation::HANDLE::default();
        let rc = WlanOpenHandle(2, None, &mut negotiated, &mut handle);
        if rc != 0 {
            return Err(format!("WlanOpenHandle: {rc}"));
        }
        let mut if_list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        // 拿到接口后逐分支算完再统一释放，保证 close/free 一定执行
        let outcome: Result<(), String> = {
            let rc = WlanEnumInterfaces(handle, None, &mut if_list);
            if rc != 0 {
                Err(format!("WlanEnumInterfaces: {rc}"))
            } else if (*if_list).dwNumberOfItems == 0 {
                Err("没有无线接口".to_string())
            } else {
                let guid = (*if_list).InterfaceInfo[0].InterfaceGuid;
                let profile: Vec<u16> = ssid.encode_utf16().chain(std::iter::once(0)).collect();
                let params = WLAN_CONNECTION_PARAMETERS {
                    wlanConnectionMode: wlan_connection_mode_profile,
                    strProfile: PCWSTR(profile.as_ptr()),
                    pDot11Ssid: std::ptr::null_mut(),
                    pDesiredBssidList: std::ptr::null_mut(),
                    dot11BssType: dot11_BSS_type_any,
                    dwFlags: 0,
                };
                let rc = WlanConnect(handle, &guid, &params, None);
                if rc == 0 {
                    Ok(())
                } else {
                    Err(format!("WlanConnect: {rc}"))
                }
            }
        };
        if !if_list.is_null() {
            WlanFreeMemory(if_list.cast());
        }
        WlanCloseHandle(handle, None);
        outcome
    }
}

/// 断开后切到放行名单时的候选轮转下标。
///
/// `netsh wlan connect` 只是"请求已提交"，真正连上要几秒——若每拍都死磕
/// 同一个连不上的网络（信号弱/密码错），名单里的第二个就永远轮不到。
/// 每拍前进一格，窗口结束时归零。
static CONNECT_CURSOR: std::sync::Mutex<usize> = std::sync::Mutex::new(0);

/// 本窗口内切换失败过的放行网络（SSID 小写）。连不上的目标（不在范围内/
/// 凭据失效）同样会让系统断开当前连接去尝试，失败后才重连回来——不记失败
/// 的话每拍重试一次等于每拍把用户正在用的网络踢一次。连上放行网络（目标
/// 达成）与出窗时清空，用户中途把路由器开机后仍有机会重新尝试。
static CONNECT_FAILED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// 上一拍发出连接请求的候选：本拍开头若仍未连上放行网络，即判定它失败
/// 并记入 [CONNECT_FAILED]。
static CONNECT_PENDING: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// 从候选里剔除本窗口已失败的网络（失败记录按 to_lowercase 存储与比对，
/// Unicode 折叠一致，非 ASCII 的 SSID 也能正确剔除）
fn filter_failed(candidates: Vec<String>, failed: &[String]) -> Vec<String> {
    candidates
        .into_iter()
        .filter(|c| !failed.contains(&c.to_lowercase()))
        .collect()
}

fn connect_failed_snapshot() -> Vec<String> {
    CONNECT_FAILED
        .lock()
        .map(|s| s.clone())
        .unwrap_or_else(|e| e.into_inner().clone())
}

/// 计入失败候选（小写存储去重）。锁中毒时沿用仓库惯例 recover 毒锁而不是
/// 静默跳过——抑制状态丢失会让失败候选重新进入每拍重试，等于抑制失效
fn push_failed(ssid: &str) {
    let lower = ssid.to_lowercase();
    let mut failed = CONNECT_FAILED.lock().unwrap_or_else(|e| e.into_inner());
    if !failed.contains(&lower) {
        failed.push(lower);
    }
}

fn set_pending(ssid: &str) {
    *CONNECT_PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(ssid.to_string());
}

/// 命令本身报错：直接计入失败并清掉对应 pending
fn mark_failed(ssid: &str) {
    push_failed(ssid);
    let mut pending = CONNECT_PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if pending.as_deref() == Some(ssid) {
        *pending = None;
    }
}

/// 上一拍发过连接请求：本拍没连上放行网络即失败，本窗口不再试它
fn take_pending_and_mark_failed() {
    let pending = CONNECT_PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(ssid) = pending {
        push_failed(&ssid);
    }
}

/// 连上放行网络或出窗：轮转下标、失败记录、待确认候选全部归零
fn reset_connect_state() {
    *CONNECT_CURSOR.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    CONNECT_FAILED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    *CONNECT_PENDING.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

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
        // 出窗：切换轮转与失败记录归零，下一晚从名单第一个重新试
        reset_connect_state();
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

    // 断开的判据与上面 manual 化循环一致（ssid_blocked：命中断开名单且不在
    // 放行名单）。此前这里只看「不在放行名单」就断——名单都空或只配放行时，
    // 用户连着的热点每拍被踢一次、再被系统按 auto 配置重连，表现为「开启管控
    // 后连着的热点自己断开」（2026-11-19 修复）。
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
    if already_allowed {
        // 已连放行网络：窗口目标达成，切换轮转与失败记录归零——之后网络掉线
        // 再进来，每个候选都还有完整的尝试机会
        reset_connect_state();
        return;
    }
    // 上一拍发过连接请求却仍没连上放行网络：那个候选本窗口内不再试
    take_pending_and_mark_failed();

    let blocked = current.as_deref().is_some_and(|s| {
        crate::config::wifi_curfew::ssid_blocked(
            &config.wifi_curfew_excluded,
            &config.wifi_curfew_allowed,
            s,
        )
    });
    if let Some(ssid) = current.as_deref().filter(|_| blocked) {
        match disconnect_current() {
            Ok(()) => crate::log_info!("wifi_curfew", "就寝守则: 已断开 {ssid}"),
            Err(e) => crate::log_warn!("wifi_curfew", "断开 {ssid} 失败: {e}"),
        }
    }

    // 切到放行名单：只断开不连接的话，机器整晚没有无线可用（校园网一断，
    // 放行名单里那台宿舍路由器本来是用户在用的，得替他连回去）。名单里的
    // 网络必须先在系统里连过一次（有 profile），否则 netsh 连不上。
    let saved_candidates = connect_candidates(&profiles, &config.wifi_curfew_allowed);
    if saved_candidates.is_empty() {
        if !config.wifi_curfew_allowed.is_empty() {
            crate::log_warn!(
                "wifi_curfew",
                "放行名单里的网络在本机都没有保存过，无法自动连接: {:?}",
                config.wifi_curfew_allowed
            );
        }
        return;
    }
    let candidates = filter_failed(saved_candidates, &connect_failed_snapshot());
    if candidates.is_empty() {
        // 都试过且没连上：继续重试只会把用户正在用的网络反复踢断，停手；
        // 连上放行网络或出窗时失败记录自动重置。静息状态（无动作发生）用
        // debug，避免整夜每 30s 一条 info 刷屏
        crate::log_debug!(
            "wifi_curfew",
            "放行网络本窗口内都已尝试且未连上，暂停切换尝试（出窗自动重置）"
        );
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
        Ok(()) => {
            // 请求已受理：下一拍开头若仍未连上放行网络，该候选计入失败
            set_pending(target);
            crate::log_info!("wifi_curfew", "就寝守则: 已请求连接放行网络 {target}");
        }
        Err(e) => {
            mark_failed(target);
            crate::log_warn!("wifi_curfew", "连接放行网络 {target} 失败: {e}");
        }
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
    fn filter_failed_drops_failed_candidates_case_insensitively() {
        let candidates = vec!["Dorm-5G".to_string(), "campus".to_string()];
        let failed = vec!["dorm-5g".to_string()];
        // 已失败的剔除（大小写不敏感），没失败的保留顺序
        assert_eq!(filter_failed(candidates, &failed), vec!["campus".to_string()]);
        assert!(filter_failed(vec!["campus".to_string()], &failed).len() == 1);
    }

    #[test]
    fn filter_failed_matches_non_ascii_ssids() {
        // 非 ASCII SSID：to_lowercase 折叠全角字母，eq_ignore_ascii_case 不折，
        // 比对必须走小写路径否则中文/全角 SSID 的失败记录剔除失效
        let failed = vec!["网络ａ".to_string()];
        assert!(filter_failed(vec!["网络Ａ".to_string()], &failed).is_empty());
        assert_eq!(
            filter_failed(vec!["网络Ｂ".to_string()], &failed),
            vec!["网络Ｂ".to_string()]
        );
    }

    #[test]
    #[ignore = "需 Windows 无线环境：验证位置服务关闭时 WlanConnect 直调不被位置管控拦截"]
    fn wlan_connect_api_works_without_location() {
        // 对已保存 profile 发起 profile 模式连接是幂等无害的：已连着时等于重连，
        // 不可达时只是受理后失败，不动其它网络
        let profiles = list_profiles().expect("netsh show profiles 应可用（不受位置管控）");
        let target = profiles.first().cloned().expect("本机应有已保存的无线 profile");
        connect_profile_via_api(&target)
            .unwrap_or_else(|e| panic!("WlanConnect 直调失败（位置管控或驱动问题）: {e}"));
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
