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
};
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

    ensure_ethernet_ip_for_login(app_handle, &adapters, &config, state.exit.is_quitting.as_ref());

    // DHCP 续租可能改变了适配器 IP，绕过缓存重新获取，
    // 避免后续 find 仍用续租前的旧快照（IP 为空）导致登录失败
    let adapters = match crate::network::get_adapters_force() {
        Ok(a) => a,
        Err(_) => adapters,
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
                update_auth_failure_count(state, app_handle, &result, &config.campus_gateway, name);
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

        // 双适配器错峰并行登录：适配器2延迟1s启动，避免同时登录触发系统封禁
        // 使用 DualAdapterExecutor 统一并发执行与结果合并
        // BE-A-09: Config 以 Arc 共享给两个闭包，替代完整 clone 两份
        //（spawn_blocking 要求 'static，闭包需持有所有权；Arc 仅复制引用计数）
        let a1_clone = a1_ref.clone();
        let a2_clone = a2_ref.clone();
        let campus_gateway = config.campus_gateway.clone();
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
            state, app_handle, &dual_result.primary, &dual_result.secondary,
            &adapter1_name, &adapter2_name, &campus_gateway,
        );
        return result;
    }

    let a1_ref = a1;

    // R1: 主适配器路径同样按角色取绑定账号
    let effective_config = adapter_credentials(&config, app_handle, &config.adapter1_account);
    let result = login_adapter_with_log(a1_ref, &effective_config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| CommandResult::err("登录请求失败"));
    update_auth_failure_count(state, app_handle, &result, &config.campus_gateway, &adapter1_name);
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
                        crate::log_warn!("logout", "注销请求失败");
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

        // 双适配器注销并行，适配器2延迟1s错峰（与登录侧策略一致）
        // 使用 DualAdapterExecutor 统一并发执行与结果合并，修复原 logout 不可中断 bug
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
            crate::log_warn!("logout", "注销请求失败");
            CommandResult::err("注销请求失败")
        })
}

/// 登录成功后的公共后处理：解除注销保护期、延迟后台检测、按需触发自动退出。
/// AM-13: 从 commands/login.rs 下沉到 auth/service.rs，供 commands/login.rs 与 app/tray.rs 共享调用，消除跨层依赖。
pub fn post_login_handler(app_handle: &AppHandle, state: &AppState) {
    crate::log_info!("login", "登录成功");
    // 手动/快速登录成功后解除注销保护期，避免后台检测强制 online=false 覆盖登录状态
    // 保护期仅用于阻止注销后自动登录立即触发，手动登录不受影响
    state.network.update(|s| s.logout_protected_until = std::time::Instant::now());
    crate::log_debug!("login", "已解除注销保护期");

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
            let cancel_token = s.task_manager
                .cancel_token("background_check")
                .unwrap_or_else(|| Arc::new(tokio_util::sync::CancellationToken::new()));
            crate::monitor::watcher::run_background_check(&app_h_bg, cancel_token).await;
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
}
