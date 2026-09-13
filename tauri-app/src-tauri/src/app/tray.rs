use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use crate::infra::command_context::CommandContext;
use crate::infra::events::EventBus;
use crate::infra::notification::emit_notification;
use crate::infra::state::AppState;

/// 托盘图标 id：动态刷新菜单时经 `app.tray_by_id` 定位
const TRAY_ID: &str = "main-tray";
/// 「切换账号」子菜单项 id 前缀，后接账号名（账号名经 validate_account_name 校验，
/// 不含冒号，可安全按前缀解析）
const SWITCH_ITEM_PREFIX: &str = "switch-account:";

/// 按当前配置构建托盘菜单：
/// - 「快速注销」：仅在已配置登录账号（config.user 非空，即 full_logout 的硬前置，
///   auth::service::full_logout 对空用户名直接报错）时启用；后端没有可靠的
///   "会话已登录"标志（在线状态为网络检测推论，存在时序盲区），故以账号是否
///   配置作为启用判据，保证启用的项必然可执行（最多因网络原因失败并给出通知）。
/// - 「切换账号」子菜单：列出已保存账号，当前账号标记「（当前）」并禁用；
///   账号列表为空时整个子菜单禁用并显示占位项（避免 Windows 空白子菜单）。
fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let s = app.state::<AppState>();
    let config = s.config.load();
    let has_account = !config.user.is_empty();
    let active_account = config.active_account.clone();
    drop(config);
    let accounts = crate::config::persist::list_account_names(app);

    let show_item = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
    let quick_login_item = MenuItemBuilder::with_id("quick-login", "快速登录").build(app)?;
    let quick_logout_item = MenuItemBuilder::with_id("quick-logout", "快速注销")
        .enabled(has_account)
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    let mut switch_builder = SubmenuBuilder::with_id(app, "switch-account", "切换账号");
    if accounts.is_empty() {
        let placeholder = MenuItemBuilder::with_id("switch-account-empty", "（无已保存账号）")
            .enabled(false)
            .build(app)?;
        switch_builder = switch_builder.item(&placeholder);
    } else {
        for name in &accounts {
            let is_active = *name == active_account;
            let label = if is_active { format!("{name}（当前）") } else { name.clone() };
            let item = MenuItemBuilder::with_id(format!("{SWITCH_ITEM_PREFIX}{name}"), label)
                .enabled(!is_active)
                .build(app)?;
            switch_builder = switch_builder.item(&item);
        }
    }
    let switch_menu = switch_builder.build()?;
    switch_menu.set_enabled(!accounts.is_empty())?;

    MenuBuilder::new(app)
        .item(&show_item)
        .item(&quick_login_item)
        .item(&quick_logout_item)
        .separator()
        .item(&switch_menu)
        .separator()
        .item(&quit_item)
        .build()
}

/// 重建并应用托盘菜单（账号配置/账号列表变化后调用，保持菜单与配置一致）。
/// 内部经 spawn_blocking 异步执行（菜单构建含读账号目录，且 set_menu 需在
/// 非主线程同步等待主线程应用），任何线程可安全调用。
pub fn refresh_tray_menu_state(app: &AppHandle) {
    let app_h = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(tray) = app_h.tray_by_id(TRAY_ID) else { return; };
        match build_tray_menu(&app_h) {
            Ok(menu) => {
                if let Err(e) = tray.set_menu(Some(menu)) {
                    crate::log_warn!("tray", "刷新托盘菜单失败: {}", e);
                }
            }
            Err(e) => crate::log_warn!("tray", "重建托盘菜单失败: {}", e),
        }
    });
}

/// 构建并注册托盘图标与菜单
pub fn build_tray(app: &tauri::AppHandle, install_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_tray_menu(app)?;

    let tray_icon = app.default_window_icon()
        .cloned()
        .or_else(|| {
            let icon_path = install_dir.join("icons").join("icon.ico");
            tauri::image::Image::from_path(&icon_path).ok()
        })
        .unwrap_or_else(|| {
            tauri::image::Image::from_bytes(include_bytes!("../../icons/icon.ico"))
                .unwrap_or_else(|e| {
                    crate::log_error!("main", "加载嵌入图标失败: {}, 使用空图标", e);
                    tauri::image::Image::new(&[], 0, 0)
                })
        });

    let _ = TrayIconBuilder::with_id(TRAY_ID) // [忽略错误] 托盘图标创建失败不影响应用运行
        .icon(tray_icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_tray_menu_event)
        .tooltip("校园网登录助手")
        .on_tray_icon_event(handle_tray_icon_event)
        .build(app);

    Ok(())
}

/// 托盘菜单事件处理
fn handle_tray_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "show" => {
            crate::app::window::show_and_focus_main(app);
        }
        "quick-login" => {
            let app_h = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let s = app_h.state::<AppState>();
                let _guard = match s.tasks.is_logging_in.try_acquire() {
                    Some(g) => g,
                    // 锁被占时不能静默吞掉：用户点击后无任何反馈表现为"点了没反应"
                    None => {
                        let _ = EventBus::new(&app_h).emit_auto_login_result(
                            false,
                            "登录正在进行中，请稍候",
                            false,
                        );
                        return;
                    }
                };
                let result = crate::auth::service::full_login(&s, &app_h, None);
                let _ = EventBus::new(&app_h).emit_auto_login_result(
                    result.success,
                    &result.message.clone().unwrap_or_default(),
                    false,
                );

                if result.success {
                    crate::auth::service::post_login_handler(&app_h, &s);
                }
            });
        }
        "quick-logout" => {
            let app_h = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                // 复用 do_logout 的完整注销编排（互斥锁/在线复检/状态重置），不复制协议逻辑
                let result = crate::commands::login::perform_full_logout_sync(&app_h, None);
                let message = result.message.clone().unwrap_or_default();
                let _ = EventBus::new(&app_h).emit_login_log(
                    &format!("托盘快速注销: {message}"),
                    if result.success { "success" } else { "error" },
                );
                // 反馈走系统通知（托盘操作时主窗口通常不可见，通知可达）；
                // 失败信息非空才弹，避免无意义的空通知
                if result.success {
                    emit_notification(&app_h, "快速注销成功", "已注销校园网登录", "mascot-portrait");
                } else if !message.is_empty() {
                    emit_notification(&app_h, "快速注销失败", &message, "mascot-alert");
                }
            });
        }
        id if id.starts_with(SWITCH_ITEM_PREFIX) => {
            let Some(name) = id.strip_prefix(SWITCH_ITEM_PREFIX).map(str::to_string) else { return; };
            let app_h = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let s = app_h.state::<AppState>();
                // 复用 switch_account 命令的核心逻辑（校验/读账号配置/落盘），不复制实现；
                // 落盘经 save_config_to_disk_encrypted，内部已发射 config-changed 同步前端
                let result = crate::commands::account::perform_switch_account_sync(&app_h, &s, &name);
                match result {
                    Ok(()) => {
                        let _ = EventBus::new(&app_h).emit_login_log(
                            &format!("已切换到账号「{name}」，可点击快速登录"),
                            "success",
                        );
                        emit_notification(
                            &app_h,
                            "切换账号成功",
                            &format!("已切换到「{name}」，可点击快速登录"),
                            "mascot-portrait",
                        );
                    }
                    Err(e) => {
                        let _ = EventBus::new(&app_h).emit_login_log(
                            &format!("切换账号失败: {e}"),
                            "error",
                        );
                        emit_notification(&app_h, "切换账号失败", &e, "mascot-alert");
                    }
                }
            });
        }
        "quit" => {
            let s = CommandContext::from_app(app);
            crate::app::shutdown::graceful_exit(app, &s);
        }
        _ => {}
    }
}

/// 托盘图标点击事件处理
fn handle_tray_icon_event(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click { button, .. } = event {
        if button == tauri::tray::MouseButton::Left {
            let app = tray.app_handle();
            crate::app::window::show_and_focus_main(app);
        }
    }
}
