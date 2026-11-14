use tauri::{AppHandle, Manager};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::config::model::Config;
use crate::config::persist::{get_data_dir, load_account_config};
use crate::network::{
    Adapter, get_adapters_cached,
    ensure_ethernet_ip_for_login,
    wait_for_adapter,
    find_with_valid_ip, find_dual_adapters,
    resolve_adapter_names,
    is_secondary_adapter_enabled,
};
use crate::auth::portal::check_portal_full;
use crate::infra::events::EventBus;
use crate::auth::protocol::do_logout_with_retry;
use crate::infra::state::{AppState, CommandResult};
use crate::auth::session::{login_adapter_with_log, adapter_action_with_log};
use crate::auth::failure_tracker::{
    update_auth_failure_count, update_dual_adapter_auth_failure,
};

/// 空凭据前置校验（纯函数）：仅当全局凭据缺失、且主/副适配器都未指定账号时
/// 才视为凭据缺失（R1：适配器指定账号的凭据在账号档案里，全局为空不再提前失败）
fn credentials_missing(config: &Config) -> bool {
    (config.user.is_empty() || config.password.is_empty())
        && config.adapter1_account.is_empty()
        && config.adapter2_account.is_empty()
}

/// 手动指定网卡登录时的账号匹配规则（纯函数，按 §4.5 规则 4）：
/// 网卡名 == 主适配器 → 主适配器账号 id；== 副适配器 → 副适配器账号 id；
/// 都不匹配 → 空串（跟随全局账号）
fn resolve_manual_adapter_account_id<'a>(name: &str, config: &'a Config) -> &'a str {
    if name == config.adapter1 {
        &config.adapter1_account
    } else if name == config.adapter2 {
        &config.adapter2_account
    } else {
        ""
    }
}

/// 凭据覆盖（纯函数）：account 为 None 时返回 base 的克隆；
/// Some 时仅覆盖 user/password/operator 三个凭据字段，其余字段
/// （含 adapter1_account/adapter2_account 等设备级配置）保持 base 的值
fn credentials_config(base: &Config, account: Option<&Config>) -> Config {
    let mut c = base.clone();
    if let Some(a) = account {
        c.user = a.user.clone();
        c.password = a.password.clone();
        c.operator = a.operator.clone();
    }
    c
}

/// 薄封装：读取适配器指定账号的档案。空 id → None（跟随当前账号）；
/// 文件不存在/读取/解析/解密失败 → log_warn 后返回 None（回退语义，不阻断登录）
fn load_adapter_account_config(app_handle: &AppHandle, account_id: &str) -> Option<Config> {
    if account_id.is_empty() {
        return None;
    }
    adapter_account_from_dir(&get_data_dir(app_handle), account_id)
}

/// 账号档案读取的回退逻辑（纯函数，便于单测）：读取失败 → log_warn + None
fn adapter_account_from_dir(data_dir: &Path, account_id: &str) -> Option<Config> {
    match load_account_config(data_dir, account_id) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            crate::log_warn!("login", "读取适配器指定账号失败，回退当前账号: id={} ({})", account_id, e);
            None
        }
    }
}

/// 按适配器角色组装凭据副本：account_id 非空且账号档案可读时覆盖凭据，否则与 base 凭据一致
fn adapter_credentials(base: &Config, app_handle: &AppHandle, account_id: &str) -> Config {
    credentials_config(base, load_adapter_account_config(app_handle, account_id).as_ref())
}

pub fn full_login(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult {
    // state.config.load_full() 返回 Arc<Config>（引用计数），复用它即可：
    // 免去复制 Config，也免去双适配器分支把 Arc 再包一层的二次包装
    let config = state.config.load_full();
    // R1: 全局凭据缺失时不再直接拒绝——若任一适配器指定了账号，
    // 后续按适配器解析出的凭据仍可完成登录；全部为空才提前失败
    if credentials_missing(&config) {
        crate::log_warn!("login", "登录失败: 用户名或密码为空");
        return CommandResult::err("用户名或密码为空");
    }

    // 日志脱敏兜底：登记本次登录密码，任何误拼进日志的消息都会被自动打码
    crate::infra::logger::register_secret(&config.password);
    crate::log_info!("login", "开始登录, 用户: {}{}, 指定适配器: {:?}", config.user, config.operator, adapter_name);

    let adapters = match get_adapters_cached() {
        Ok(a) => a,
        Err(_) => match wait_for_adapter(10000, state.exit.is_quitting.as_ref()) {
            Ok(a) => a,
            Err(e) => return CommandResult::err(&format!("获取适配器失败: {e}")),
        },
    };

    if adapters.is_empty() {
        return CommandResult::err("未找到可用网络适配器");
    }

    let renewed = ensure_ethernet_ip_for_login(app_handle, &adapters, &config, state.exit.is_quitting.as_ref());

    // 续租后必须重查；快照中主适配器还没有 IP 时也重查，避免缓存掩盖刚完成的 DHCP 分配。
    let adapters = if crate::network::adapter::should_refresh_adapter_snapshot(&adapters, &config, renewed) {
        match crate::network::get_adapters_force() {
            Ok(a) => a,
            Err(_) => adapters,
        }
    } else {
        adapters
    };

    if let Some(name) = adapter_name {
        let adapter = find_with_valid_ip(&adapters, name);
        match adapter {
            Some(a) => {
                // R1: 手动指定的网卡按角色取绑定账号（==主适配器→adapter1Account、
                // ==副适配器→adapter2Account、其他网卡跟随全局账号）；
                // 日志/登录历史由 login_adapter_with_log 从副本的 user 记录实际使用账号
                let account_id = resolve_manual_adapter_account_id(name, &config);
                let effective_config = adapter_credentials(&config, app_handle, account_id);
                let result = login_adapter_with_log(a, &effective_config, app_handle, state.exit.is_quitting.as_ref())
                    .unwrap_or_else(|| CommandResult::err("登录请求失败"));
                update_auth_failure_count(state, &result, name);
                return result;
            }
            None => return CommandResult::err(&format!("未找到适配器: {name}")),
        }
    }

    let (adapter1_name, adapter2_name) = resolve_adapter_names(&adapters, &config);

    let (a1_opt, a2_opt) = find_dual_adapters(&adapters, &config, &adapter1_name, &adapter2_name);
    let a1 = match a1_opt {
        Some(a) => a,
        None => return CommandResult::err("未找到有效IP地址的适配器"),
    };

    if let Some(a2_ref) = a2_opt {
        let a1_ref = a1;

        // 双适配器串行登录：主适配器完成后，副适配器再延迟1s启动，避免并发认证触发系统封禁
        // 使用 DualAdapterExecutor 统一串行执行与结果合并
        // BE-A-09: Config 以 Arc 共享给两个闭包，替代完整 clone 两份
        //（spawn_blocking 要求 'static，闭包需持有所有权；Arc 仅复制引用计数）
        let a1_clone = a1_ref.clone();
        let a2_clone = a2_ref.clone();
        // R1: 每个适配器各持一份按角色解析出的凭据副本（未指定账号时凭据与全局一致，行为不变）；
        // Arc<Config> 满足 execute_dual 闭包的 'static 约束，其余字段保持 base 值。
        // 未指定账号（account id 为空）时直接共享 base 的 Arc（保持 BE-A-09 的
        // 零拷贝优化），仅指定了账号才深拷构造凭据副本
        let config_shared1 = if config.adapter1_account.is_empty() {
            Arc::clone(&config)
        } else {
            Arc::new(adapter_credentials(&config, app_handle, &config.adapter1_account))
        };
        let config_shared2 = if config.adapter2_account.is_empty() {
            Arc::clone(&config)
        } else {
            Arc::new(adapter_credentials(&config, app_handle, &config.adapter2_account))
        };
        let app_h1 = app_handle.clone();
        let app_h2 = app_handle.clone();
        let is_quitting1 = state.exit.is_quitting.clone();
        let is_quitting2 = state.exit.is_quitting.clone();
        let dual_result = crate::auth::dual_adapter_executor::execute_dual(
            move || login_adapter_with_log(&a1_clone, &config_shared1, &app_h1, is_quitting1.as_ref()),
            move || login_adapter_with_log(&a2_clone, &config_shared2, &app_h2, is_quitting2.as_ref()),
            state.exit.is_quitting.clone(),
        );

        let result = dual_result.build_command_result();
        // 双适配器分别计数：对认证失败的适配器单独递增计数，连续5次触发该适配器 MAC 重置
        update_dual_adapter_auth_failure(
            state, &dual_result.primary, &dual_result.secondary,
            &adapter1_name, &adapter2_name,
        );
        return result;
    }

    let a1_ref = a1;

    // R1: 主适配器路径同样按角色取绑定账号
    let effective_config = adapter_credentials(&config, app_handle, &config.adapter1_account);
    let result = login_adapter_with_log(a1_ref, &effective_config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| CommandResult::err("登录请求失败"));
    update_auth_failure_count(state, &result, &adapter1_name);
    result
}

pub fn logout_adapter_with_log(
    adapter: &Adapter,
    config: &Config,
    app_handle: &AppHandle,
    is_quitting: &AtomicBool,
) -> Option<CommandResult> {
    let adapter_ip = adapter.ip.clone();

    adapter_action_with_log(
        adapter, config, app_handle,
        "注销", "logout", "logout",
        || do_logout_with_retry(&config.user, Some(adapter_ip.as_str()), 2, is_quitting),
    )
}

pub fn full_logout(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult {
    // 与 full_login 同理：复用 load_full() 的 Arc，不再复制 Config
    let config = state.config.load_full();
    if config.user.is_empty() {
        crate::log_warn!("logout", "注销失败: 用户名为空");
        return CommandResult::err("用户名为空，无法注销");
    }

    crate::log_info!("logout", "开始注销, 用户: {}, 指定适配器: {:?}", config.user, adapter_name);

    let adapters = match get_adapters_cached() {
        Ok(a) => a,
        Err(_) => match wait_for_adapter(10000, state.exit.is_quitting.as_ref()) {
            Ok(a) => a,
            Err(e) => {
                crate::log_warn!("logout", "获取适配器失败: {}", e);
                return CommandResult::err(&format!("获取适配器失败: {e}"));
            }
        },
    };

    if adapters.is_empty() {
        crate::log_warn!("logout", "未找到可用网络适配器");
        return CommandResult::err("未找到可用网络适配器");
    }

    if let Some(name) = adapter_name {
        let adapter = find_with_valid_ip(&adapters, name);
        match adapter {
            Some(a) => {
                return logout_adapter_with_log(a, &config, app_handle, state.exit.is_quitting.as_ref())
                    .unwrap_or_else(|| {
                        crate::log_warn!("logout", "注销请求失败（指定适配器: {}）", a.name);
                        CommandResult::err("注销请求失败")
                    });
            }
            None => {
                crate::log_warn!("logout", "未找到适配器: {}", name);
                return CommandResult::err(&format!("未找到适配器: {name}"));
            }
        }
    }

    let (adapter1_name, adapter2_name) = resolve_adapter_names(&adapters, &config);

    let (a1_opt, a2_opt) = find_dual_adapters(&adapters, &config, &adapter1_name, &adapter2_name);
    let a1 = match a1_opt {
        Some(a) => a,
        None => {
            crate::log_warn!("logout", "未找到有效IP地址的适配器");
            return CommandResult::err("未找到有效IP地址的适配器");
        }
    };

    if let Some(a2_ref) = a2_opt {
        let a1_ref = a1;

        // 双适配器串行注销：主适配器完成后，副适配器再延迟1s启动（与登录侧策略一致）
        // 使用 DualAdapterExecutor 统一串行执行与结果合并，保留可中断等待
        // BE-A-09: Config 以 Arc 共享给两个闭包，替代完整 clone 两份
        let a1_clone = a1_ref.clone();
        let a2_clone = a2_ref.clone();
        let config_shared1 = config;
        let config_shared2 = config_shared1.clone();
        let app_h1 = app_handle.clone();
        let app_h2 = app_handle.clone();
        let is_quitting1 = state.exit.is_quitting.clone();
        let is_quitting2 = state.exit.is_quitting.clone();
        let dual_result = crate::auth::dual_adapter_executor::execute_dual(
            move || logout_adapter_with_log(&a1_clone, &config_shared1, &app_h1, is_quitting1.as_ref()),
            move || logout_adapter_with_log(&a2_clone, &config_shared2, &app_h2, is_quitting2.as_ref()),
            state.exit.is_quitting.clone(),
        );

        return dual_result.build_command_result();
    }

    let a1_ref = a1;

    logout_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| {
            crate::log_warn!("logout", "注销请求失败（主适配器: {}）", a1_ref.name);
            CommandResult::err("注销请求失败")
        })
}

#[derive(Clone, Copy)]
pub(crate) struct AdapterOnlineStatus {
    pub any_online: bool,
    pub a1_online: bool,
    pub a2_online: bool,
    /// 探测链路是否全部可用；false 时“离线”结论不可信。
    pub probe_ok: bool,
}

/// 并行探测主/副适配器的 Portal 在线状态。
pub(crate) fn check_any_adapter_online(state: &AppState) -> AdapterOnlineStatus {
    let adapters = match get_adapters_cached() {
        Ok(a) => a,
        Err(_) => return AdapterOnlineStatus {
            any_online: false,
            a1_online: false,
            a2_online: false,
            probe_ok: false,
        },
    };
    let config = state.config.load_full();
    let (a1_name, a2_name) = resolve_adapter_names(&adapters, &config);
    let a2_enabled = is_secondary_adapter_enabled(&config, &a2_name);

    let check_one = |name: &str| -> Option<bool> {
        // scope 裸子线程没有 Tokio runtime context：reqwest 的 send() 是同步函数，
        // 构造 per-request 超时计时器需要 Handle::current()，缺失会 panic。
        let _runtime_ctx = tauri::async_runtime::handle().inner().enter();
        find_with_valid_ip(&adapters, name)
            .and_then(|a| check_portal_full(&a.ip, Some(&a.name)).ok().map(|ps| ps.online))
    };

    let (a1_probe, a2_probe) = std::thread::scope(|s| {
        let h1 = s.spawn(|| check_one(&a1_name));
        let a2_probe = if a2_enabled {
            let h2 = s.spawn(|| check_one(&a2_name));
            h2.join().unwrap_or(None)
        } else {
            Some(false)
        };
        (h1.join().unwrap_or(None), a2_probe)
    });

    let a1_online = a1_probe.unwrap_or(false);
    let a2_online = a2_probe.unwrap_or(false);
    AdapterOnlineStatus {
        any_online: a1_online || a2_online,
        a1_online,
        a2_online,
        probe_ok: a1_probe.is_some() && a2_probe.is_some(),
    }
}

/// 注销成功后的 1s 复检与结论日志：只有探测全部成功且离线才断言“已确认离线”。
pub(crate) fn verify_logout_and_emit(app_handle: &AppHandle, state: &AppState) -> AdapterOnlineStatus {
    std::thread::sleep(Duration::from_secs(1));
    let status = check_any_adapter_online(state);
    let event_bus = EventBus::new(app_handle);
    if status.any_online {
        let _ = event_bus.emit_login_log("页面检测仍显示在线，注销可能未完全生效", "warning");
    } else if status.probe_ok {
        let _ = event_bus.emit_login_log("注销成功（页面检测已确认离线）", "success");
    } else {
        crate::log_warn!("logout", "注销后页面检测存在探测异常，离线状态未能确认");
        let _ = event_bus.emit_login_log(
            "注销请求已发送，离线状态未能确认（页面检测异常）",
            "warning",
        );
    }
    status
}

/// 全量注销成功后的立即收尾：取消自动退出、重置失败/重连状态并开启 60s 保护期。
/// 必须紧接注销请求成功执行，不能等 1s Portal 复检——复检窗口内的自动重连
/// 会看到旧的在线/保护期状态，把刚注销的账号重新登回。
pub(crate) fn begin_full_logout(state: &AppState) {
    crate::log_info!("logout", "全量注销成功，已重置网络状态，60秒注销保护期开始");
    state.exit.auto_exit_cancelled.store(true, Ordering::Release);
    state.exit.set_deadline(None);
    crate::auth::failure_tracker::reset_all(state);

    let protected_until = std::time::Instant::now() + Duration::from_secs(60);
    state.network.update(|n| {
        n.has_logged_online = false;
        n.disconnect_reconnect_count = 0;
        n.secondary_disconnect_reconnect_count = 0;
        n.last_auto_login_attempt = std::time::Instant::now();
        n.logout_protected_until = protected_until;
    });
}

/// 注销复检完成后的在线快照回填；不改保护期与登录历史。
pub(crate) fn apply_logout_online_snapshot(state: &AppState, status: AdapterOnlineStatus) {
    let cfg = state.config.load_full();
    state.network.update(|n| {
        n.any_adapter_online = status.any_online;
        n.last_a1_online = status.a1_online;
        n.last_a2_online = if cfg.dual_adapter { status.a2_online } else { false };
    });
}

/// 单适配器注销成功后的状态收尾：只更新该口径的在线快照，不改保护期与登录历史。
pub(crate) fn finish_partial_logout(
    state: &AppState,
    status: AdapterOnlineStatus,
    adapter_name: Option<&str>,
) {
    crate::log_info!("logout", "单适配器注销成功: {:?}", adapter_name);
    apply_logout_online_snapshot(state, status);
}

/// 登录成功的统一状态收尾：解除注销保护期并更新登录历史/失败与重连计数。
/// 在线快照仍由后续后台检测写入（调用方无法在此可靠判断主副卡），避免把
/// `any_adapter_online` 与 `last_a*_online` 写成互相矛盾的状态。
pub(crate) fn mark_login_success(state: &AppState) {
    state.network.update(|s| {
        s.logout_protected_until = std::time::Instant::now();
        s.has_logged_online = true;
        s.prep_login_failures = 0;
        s.disconnect_reconnect_count = 0;
        s.secondary_disconnect_reconnect_count = 0;
    });
}

/// 登录成功后的公共后处理：解除注销保护期、延迟后台检测、按需触发自动退出。
/// AM-13: 从 commands/login.rs 下沉到 auth/service.rs，供 commands/login.rs 与 app/tray.rs 共享调用，消除跨层依赖。
pub fn post_login_handler(app_handle: &AppHandle, state: &AppState) {
    crate::log_info!("login", "登录成功");
    // 手动/快速/定时登录成功后统一解除注销保护期并更新登录历史；在线快照
    // 交由后续后台检测回填，避免在此写出与主副卡状态不一致的快照。
    mark_login_success(state);
    crate::log_debug!("login", "已解除注销保护期并更新登录状态");

    let app_h_bg = app_handle.clone();
    let config = state.config.load_full();
    let auto_exit = config.auto_exit_after_login;
    // 历史缺陷：无论用户是否开启后台巡检，登录后都强制触发一次后台检查。
    // 改为仅当 enable_background_check 开启时才触发，避免违背用户显式关闭的决定。
    let enable_bg = config.enable_background_check;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let s = app_h_bg.state::<AppState>();
        // 退出流程已开始时不再执行后台检查或触发自动退出
        if s.exit.is_quitting.load(Ordering::Acquire) {
            return;
        }
        if enable_bg {
            // 审计 A9：改走统一注册入口——一次性检测经 task_manager 注册运行
            // （"background_check_once"），取消令牌全程可达（旧实现取不到周期任务
            // 令牌时新建孤儿令牌，任务不可取消）
            crate::monitor::background_task::spawn_background_check_once(&app_h_bg);
        }

        if auto_exit && !s.exit.is_quitting.load(Ordering::Acquire) {
            crate::infra::lifecycle::start_auto_exit(&app_h_bg, &s);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 带凭据与适配器账号绑定的测试 Config
    fn test_config() -> Config {
        Config {
            user: "global-user".to_string(),
            password: "global-pass".to_string(),
            operator: String::new(),
            adapter1: "以太网".to_string(),
            adapter2: "WLAN".to_string(),
            adapter1_account: "acc-1".to_string(),
            adapter2_account: "acc-2".to_string(),
            campus_gateway: "10.2.127.254".to_string(),
            ..Default::default()
        }
    }

    fn account_config() -> Config {
        Config {
            user: "acc-user".to_string(),
            password: "acc-pass".to_string(),
            operator: "cmcc".to_string(),
            adapter1_account: "should-not-leak".to_string(),
            ..Default::default()
        }
    }

    /// credentials_config: account 为 None 时返回与 base 凭据/设备字段一致的克隆
    #[test]
    fn credentials_config_none_returns_base_clone() {
        let base = test_config();
        let out = credentials_config(&base, None);
        assert_eq!(out.user, base.user);
        assert_eq!(out.password, base.password);
        assert_eq!(out.operator, base.operator);
        assert_eq!(out.adapter1, base.adapter1);
        assert_eq!(out.adapter1_account, base.adapter1_account);
        assert_eq!(out.adapter2_account, base.adapter2_account);
        assert_eq!(out.campus_gateway, base.campus_gateway);
    }

    /// credentials_config: Some 时仅覆盖 user/password/operator，
    /// adapter1Account/adapter2Account 等设备级字段保持 base 的值
    #[test]
    fn credentials_config_some_overrides_only_credentials() {
        let base = test_config();
        let account = account_config();
        let out = credentials_config(&base, Some(&account));
        assert_eq!(out.user, "acc-user");
        assert_eq!(out.password, "acc-pass");
        assert_eq!(out.operator, "cmcc");
        assert_eq!(out.adapter1_account, "acc-1", "adapter1Account 不得被账号档案覆盖");
        assert_eq!(out.adapter2_account, "acc-2", "adapter2Account 不得被账号档案覆盖");
        assert_eq!(out.adapter1, "以太网");
        assert_eq!(out.campus_gateway, base.campus_gateway);
    }

    /// 手动指定网卡的账号匹配规则：==主适配器→主账号、==副适配器→副账号、否则→全局（空串）
    #[test]
    fn manual_adapter_account_matches_role() {
        let base = test_config();
        assert_eq!(resolve_manual_adapter_account_id("以太网", &base), "acc-1");
        assert_eq!(resolve_manual_adapter_account_id("WLAN", &base), "acc-2");
        assert_eq!(resolve_manual_adapter_account_id("蓝牙", &base), "", "未匹配任何适配器 → 跟随全局账号");
    }

    /// 空凭据校验规则：全局为空但任一适配器指定了账号时不再提前失败
    #[test]
    fn credentials_missing_requires_global_and_no_binding() {
        let cfg = Config::default();
        assert!(credentials_missing(&cfg), "全局凭据全空且无绑定 → 拒绝");

        let mut user_only = Config::default();
        user_only.password = "pwd".to_string();
        assert!(credentials_missing(&user_only), "user 缺失仍视为凭据缺失（保持原语义）");

        let mut bound1 = Config::default();
        bound1.adapter1_account = "acc-1".to_string();
        assert!(!credentials_missing(&bound1), "主适配器指定了账号 → 放行");

        let mut bound2 = Config::default();
        bound2.adapter2_account = "acc-2".to_string();
        assert!(!credentials_missing(&bound2), "副适配器指定了账号 → 放行");

        let mut full = test_config();
        full.adapter1_account = String::new();
        full.adapter2_account = String::new();
        assert!(!credentials_missing(&full), "全局凭据齐全 → 不拒绝");
    }

    /// 薄封装回退语义（纯函数部分）：账号文件不存在 → None（回退），空 id → None
    #[test]
    fn adapter_account_missing_file_falls_back_to_none() {
        let dir = std::env::temp_dir().join(format!("cl_auth_svc_missing_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(adapter_account_from_dir(&dir, "no-such-account").is_none());
        assert!(adapter_account_from_dir(&dir, "").is_none(), "空 id 应直接返回 None");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 已存在的账号档案可读回（密码经 DPAPI 加密落盘、读回解密还原）
    #[cfg(target_os = "windows")]
    #[test]
    fn adapter_account_loads_existing_account() {
        let dir = std::env::temp_dir().join(format!("cl_auth_svc_ok_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut acc = Config::default();
        acc.user = "u-acc".to_string();
        acc.password = "p-acc".to_string();
        crate::config::persist::save_account_config(&dir, "acc-x", &acc).unwrap();
        let loaded = adapter_account_from_dir(&dir, "acc-x").expect("已存在的账号应加载成功");
        assert_eq!(loaded.user, "u-acc");
        assert_eq!(loaded.password, "p-acc");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 登录成功统一更新登录状态并解除保护期() {
        let state = AppState::new();
        let future = std::time::Instant::now() + Duration::from_secs(60);
        state.network.update(|n| {
            n.any_adapter_online = false;
            n.has_logged_online = false;
            n.prep_login_failures = 3;
            n.disconnect_reconnect_count = 2;
            n.logout_protected_until = future;
        });

        mark_login_success(&state);

        let n = state.network.load();
        assert!(!n.any_adapter_online, "在线快照由后续检测回填，避免与 last_a*_online 矛盾");
        assert!(n.has_logged_online);
        assert_eq!(n.prep_login_failures, 0);
        assert_eq!(n.disconnect_reconnect_count, 0);
        assert!(n.logout_protected_until <= std::time::Instant::now(), "登录成功必须解除注销保护期");
    }

    #[test]
    fn 全量注销收尾重置在线与保护期() {
        let state = AppState::new();
        state.config.store(Config { dual_adapter: true, ..Default::default() });
        let future = std::time::Instant::now() + Duration::from_secs(60);
        state.network.update(|n| {
            n.any_adapter_online = true;
            n.has_logged_online = true;
            n.disconnect_reconnect_count = 2;
            n.logout_protected_until = future;
        });
        state.exit.auto_exit_cancelled.store(false, Ordering::Release);

        begin_full_logout(&state);
        // 保护期必须立即生效，不能等 1s 复检。
        let immediate = state.network.load();
        assert!(!immediate.has_logged_online);
        assert_eq!(immediate.disconnect_reconnect_count, 0);
        assert!(immediate.logout_protected_until >= future, "全量注销必须立即重启 60s 保护期");
        assert!(state.exit.auto_exit_cancelled.load(Ordering::Acquire));

        apply_logout_online_snapshot(&state, AdapterOnlineStatus {
            any_online: true,
            a1_online: false,
            a2_online: true,
            probe_ok: true,
        });

        let n = state.network.load();
        assert!(n.any_adapter_online);
        assert_eq!(n.last_a2_online, true, "双卡启用时副卡在线快照必须写入");
    }

    #[test]
    fn 单卡注销不改登录历史与保护期() {
        let state = AppState::new();
        state.config.store(Config { dual_adapter: true, ..Default::default() });
        let protected = std::time::Instant::now() + Duration::from_secs(30);
        state.network.update(|n| {
            n.any_adapter_online = true;
            n.has_logged_online = true;
            n.logout_protected_until = protected;
        });

        finish_partial_logout(&state, AdapterOnlineStatus {
            any_online: true,
            a1_online: false,
            a2_online: true,
            probe_ok: true,
        }, Some("WLAN"));

        let n = state.network.load();
        assert!(n.any_adapter_online);
        assert!(n.has_logged_online);
        assert_eq!(n.last_a1_online, false);
        assert_eq!(n.last_a2_online, true);
        assert_eq!(n.logout_protected_until, protected, "单卡注销不得改动全量保护期");
    }

    #[test]
    fn 单卡配置不回填副卡在线快照() {
        let state = AppState::new();
        state.config.store(Config { dual_adapter: false, ..Default::default() });

        apply_logout_online_snapshot(&state, AdapterOnlineStatus {
            any_online: true,
            a1_online: false,
            a2_online: true,
            probe_ok: true,
        });

        let n = state.network.load();
        assert!(n.any_adapter_online);
        assert!(!n.last_a2_online, "未启用双卡时副卡快照必须归零");
    }
}
