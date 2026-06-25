use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use crate::infra::state::AppState;
use crate::infra::events::EventBus;

/// 构建并注册托盘图标与菜单
pub fn build_tray(app: &tauri::AppHandle, install_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
    let quick_login_item = MenuItemBuilder::with_id("quick-login", "快速登录").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&quick_login_item)
        .separator()
        .item(&quit_item)
        .build()?;

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

    let _ = TrayIconBuilder::new() // [忽略错误] 托盘图标创建失败不影响应用运行
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
                    None => return,
                };
                let result = crate::auth::service::full_login(&s, &app_h, None);
                let _ = EventBus::new(&app_h).emit_auto_login_result(
                    result.success,
                    &result.message.clone().unwrap_or_default(),
                    false,
                );

                if result.success {
                    crate::commands::login::post_login_handler(&app_h, &s);
                }
            });
        }
        "quit" => {
            let s = app.state::<AppState>();
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
