use tauri::AppHandle;
use std::sync::atomic::AtomicBool;
use crate::config::model::Config;
use crate::network::{
    Adapter, get_adapters_cached,
    ensure_ethernet_ip_for_login,
    wait_for_adapter,
};
use crate::auth::protocol::do_logout_with_retry;
use crate::auth::traits::{AdapterResolver, DefaultAdapterResolver};
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
            Err(e) => return CommandResult::err(&format!("获取适配器失败: {}", e)),
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
        let adapter = adapters.iter().find(|a| a.name == name && !a.ip.is_empty());
        match adapter {
            Some(a) => {
                let result = login_adapter_with_log(a, &config, app_handle, state.exit.is_quitting.as_ref())
                    .unwrap_or_else(|| CommandResult::err("登录请求失败"));
                update_auth_failure_count(state, app_handle, &result, &config.campus_gateway);
                return result;
            }
            None => return CommandResult::err(&format!("未找到适配器: {}", name)),
        }
    }

    let (adapter1_name, adapter2_name) = DefaultAdapterResolver.resolve_adapter_names(&adapters, &config);

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    if a1.is_none() {
        return CommandResult::err("未找到有效IP地址的适配器");
    }

    if config.dual_adapter && !adapter2_name.is_empty() && adapter2_name != adapter1_name {
        let a2 = adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty());
        if let Some(a2_ref) = a2 {
            let a1_ref = a1.unwrap();

            // 双适配器错峰并行登录：适配器2延迟1s启动，避免同时登录触发系统封禁
            // 使用 DualAdapterExecutor 统一并发执行与结果合并
            let a1_clone = a1_ref.clone();
            let a2_clone = a2_ref.clone();
            let config_clone1 = config.clone();
            let config_clone2 = config.clone();
            let app_h1 = app_handle.clone();
            let app_h2 = app_handle.clone();
            let is_quitting1 = state.exit.is_quitting.clone();
            let is_quitting2 = state.exit.is_quitting.clone();
            let dual_result = crate::auth::dual_adapter_executor::execute_dual(
                Box::new(move || login_adapter_with_log(&a1_clone, &config_clone1, &app_h1, is_quitting1.as_ref())),
                Box::new(move || login_adapter_with_log(&a2_clone, &config_clone2, &app_h2, is_quitting2.as_ref())),
                state.exit.is_quitting.clone(),
            );

            let result = dual_result.build_command_result();
            // 双适配器分别计数：对认证失败的适配器单独递增计数，连续5次触发该适配器 MAC 重置
            update_dual_adapter_auth_failure(
                state, app_handle, &dual_result.primary, &dual_result.secondary,
                &adapter1_name, &adapter2_name, &config.campus_gateway,
            );
            return result;
        }
    }

    let a1_ref = a1.unwrap();

    let result = login_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| CommandResult::err("登录请求失败"));
    update_auth_failure_count(state, app_handle, &result, &config.campus_gateway);
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
                return CommandResult::err(&format!("获取适配器失败: {}", e));
            }
        },
    };

    if adapters.is_empty() {
        crate::log_warn!("logout", "未找到可用网络适配器");
        return CommandResult::err("未找到可用网络适配器");
    }

    if let Some(name) = adapter_name {
        let adapter = adapters.iter().find(|a| a.name == name && !a.ip.is_empty());
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
                return CommandResult::err(&format!("未找到适配器: {}", name));
            }
        }
    }

    let (adapter1_name, adapter2_name) = DefaultAdapterResolver.resolve_adapter_names(&adapters, &config);

    let a1 = adapters.iter().find(|a| a.name == adapter1_name && !a.ip.is_empty());
    if a1.is_none() {
        crate::log_warn!("logout", "未找到有效IP地址的适配器");
        return CommandResult::err("未找到有效IP地址的适配器");
    }

    if config.dual_adapter && !adapter2_name.is_empty() && adapter2_name != adapter1_name {
        let a2 = adapters.iter().find(|a| a.name == adapter2_name && !a.ip.is_empty());
        if let Some(a2_ref) = a2 {
            let a1_ref = a1.unwrap();

            // 双适配器注销并行，适配器2延迟1s错峰（与登录侧策略一致）
            // 使用 DualAdapterExecutor 统一并发执行与结果合并，修复原 logout 不可中断 bug
            let a1_clone = a1_ref.clone();
            let a2_clone = a2_ref.clone();
            let config_clone1 = config.clone();
            let config_clone2 = config.clone();
            let app_h1 = app_handle.clone();
            let app_h2 = app_handle.clone();
            let is_quitting1 = state.exit.is_quitting.clone();
            let is_quitting2 = state.exit.is_quitting.clone();
            let dual_result = crate::auth::dual_adapter_executor::execute_dual(
                Box::new(move || logout_adapter_with_log(&a1_clone, &config_clone1, &app_h1, is_quitting1.as_ref())),
                Box::new(move || logout_adapter_with_log(&a2_clone, &config_clone2, &app_h2, is_quitting2.as_ref())),
                state.exit.is_quitting.clone(),
            );

            return dual_result.build_command_result();
        }
    }

    let a1_ref = a1.unwrap();

    logout_adapter_with_log(a1_ref, &config, app_handle, state.exit.is_quitting.as_ref())
        .unwrap_or_else(|| {
            crate::log_warn!("logout", "注销请求失败");
            CommandResult::err("注销请求失败")
        })
}
