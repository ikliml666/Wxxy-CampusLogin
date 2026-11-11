use tauri::{AppHandle, Manager, State};
use std::sync::atomic::Ordering;
use crate::auth::service::{
    apply_logout_online_snapshot, begin_full_logout, finish_partial_logout,
    verify_logout_and_emit,
};
use crate::infra::state::{AppState, CommandResult};

#[tauri::command]
pub async fn do_login(state: State<'_, AppState>, app_handle: AppHandle, adapter_name: Option<String>) -> Result<CommandResult, String> {
    state.exit.auto_exit_cancelled.store(false, Ordering::Release);
    // 取消可能残留的自动退出倒计时，避免重新登录后被旧倒计时强制退出
    state.exit.set_deadline(None);

    let result = {
        let adapter = adapter_name.clone();
        let app_h = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let s = app_h.state::<AppState>();
            let _guard = match s.tasks.is_logging_in.try_acquire() {
                Some(g) => g,
                None => {
                    crate::log_warn!("login", "登录被拒绝：已有登录任务在进行");
                    return CommandResult::err("登录正在进行中");
                }
            };
            crate::auth::service::full_login(&s, &app_h, adapter.as_deref())
        }).await.map_err(|e| format!("登录任务失败: {e}"))?
    };

    if result.success {
        crate::auth::service::post_login_handler(&app_handle, &state);
    }

    Ok(result)
}

#[tauri::command]
pub async fn do_logout(_state: State<'_, AppState>, app_handle: AppHandle, adapter_name: Option<String>) -> Result<CommandResult, String> {
    let app_h = app_handle.clone();
    // 注销含网络请求与固定 1s 复检延迟，走 spawn_blocking；State<'_> 不可跨 await，
    // 闭包内经 app_handle 重新获取
    let result = tauri::async_runtime::spawn_blocking(move || {
        perform_full_logout_sync(&app_h, adapter_name.as_deref())
    }).await.map_err(|e| format!("注销任务失败: {e}"))?;
    Ok(result)
}

/// do_logout 命令与托盘「快速注销」菜单共享的注销核心（含阻塞网络请求与 1s 在线
/// 复检，须在 spawn_blocking 等阻塞上下文调用）。协议动作复用 auth::service::full_logout，
/// 成功后的状态重置与 do_logout 原实现逐行对齐（取消自动退出/重置检测计数/60s 注销保护期）。
pub(crate) fn perform_full_logout_sync(app_handle: &AppHandle, adapter_name: Option<&str>) -> CommandResult {
    let s = app_handle.state::<AppState>();
    let _guard = match s.tasks.is_logging_out.try_acquire() {
        Some(g) => g,
        None => {
            crate::log_warn!("logout", "注销被拒绝：已有注销任务在进行");
            return CommandResult::err("注销正在进行中，请稍后再试");
        }
    };

    let result = crate::auth::service::full_logout(&s, app_handle, adapter_name);

    if result.success {
        // 保护期必须先于 1s 复检生效，否则复检窗口内断线重连可把刚注销的账号登回。
        let is_full_logout = adapter_name.is_none();
        if is_full_logout {
            begin_full_logout(&s);
        }
        let status = verify_logout_and_emit(app_handle, &s);
        if is_full_logout {
            apply_logout_online_snapshot(&s, status);
        } else {
            finish_partial_logout(&s, status, adapter_name);
        }
    }
    result
}
