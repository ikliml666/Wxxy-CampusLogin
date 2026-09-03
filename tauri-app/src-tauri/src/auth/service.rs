use tauri::{AppHandle, Manager};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::config::model::Config;
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

pub fn full_login(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult {
    let config = {
        let guard = state.config.load();
        if guard.user.is_empty() || guard.password.is_empty() {
            crate::log_warn!("login", "登录失败: 用户名或密码为空");
            return CommandResult::err("用户名或密码为空");
        }
        guard.clone()
    };

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
                let result = login_adapter_with_log(a, &config, app_handle, state.exit.is_quitting.as_ref())
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
        let config_shared1 = std::sync::Arc::new(config);
        let config_shared2 = config_shared1.clone();
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

    let result = login_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
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
    let adapter_if_index = adapter.if_index;
    let adapter_mac = adapter.mac.clone();

    adapter_action_with_log(
        adapter, config, app_handle,
        "注销", "logout", "logout",
        || do_logout_with_retry(&config.user, Some(adapter_ip.as_str()), adapter_if_index, &adapter_mac, 2, is_quitting),
    )
}

pub fn full_logout(state: &AppState, app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult {
    let config = {
        let guard = state.config.load();
        if guard.user.is_empty() {
            crate::log_warn!("logout", "注销失败: 用户名为空");
            return CommandResult::err("用户名为空，无法注销");
        }
        guard.clone()
    };

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
        let config_shared1 = std::sync::Arc::new(config);
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
