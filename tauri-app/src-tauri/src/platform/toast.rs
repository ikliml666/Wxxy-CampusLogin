//! 更新提醒专用 WinRT Toast（带点击回调）
//!
//! tauri-plugin-notification 桌面端（notify-rust → tauri-winrt-notification）
//! 不暴露 Activated 回调，点击通知只有"激活应用"的系统默认行为；
//! 更新提醒需要"点击通知 → 唤起主窗口并跳转关于界面"，因此单独经
//! windows crate 自组 toast XML 并注册 Activated 处理器。
//! AUMID 沿用 notify-rust 默认的 PowerShell APP_ID，与既有系统通知同源
//! （未打包应用借用该 AUMID 才能出现在通知中心）。

use tauri::{AppHandle, Manager};
use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

const APP_ID: &str = "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

/// 解析打包资源文件为 toast 可用的 file:/// URL；资源不存在时返回 None（通知退回无图样式）。
fn resolve_resource_image(app_handle: &AppHandle, relative: &str) -> Option<String> {
    let path = app_handle.path().resource_dir().ok()?.join(relative);
    if !path.is_file() {
        return None;
    }
    let url = path.to_string_lossy().replace('\\', "/");
    Some(format!("file:///{url}"))
}

/// 通用系统通知 toast（带娘头像 appLogoOverride，无点击回调）。
/// `mascot` 为 `resources/mascot-toast/` 下的文件 stem（如 `mascot-alert`）。
pub fn show_system_toast(app_handle: &AppHandle, title: &str, body: &str, mascot: &str) -> Result<(), String> {
    let image_part = resolve_resource_image(app_handle, &format!("resources/mascot-toast/{mascot}.png"))
        .map(|path| format!(r#"<image placement="appLogoOverride" hint-crop="circle" src="{path}"/>"#))
        .unwrap_or_default();
    let xml = format!(
        r#"<toast activationType="foreground"><visual><binding template="ToastGeneric">{image_part}<text>{title}</text><text>{body}</text></binding></visual></toast>"#
    );
    let doc = XmlDocument::new().map_err(|e| e.to_string())?;
    doc.LoadXml(&HSTRING::from(xml)).map_err(|e| e.to_string())?;
    let toast = ToastNotification::CreateToastNotification(&doc).map_err(|e| e.to_string())?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
        .map_err(|e| e.to_string())?;
    notifier.Show(&toast).map_err(|e| e.to_string())
}

/// 发送"发现新版本"toast，点击通知唤起主窗口并向前端发 `update-notification-click`。
/// 任一步失败返回 Err，由调用方降级走 emit_notification。
pub fn show_update_toast(app_handle: &AppHandle, version: &str) -> Result<(), String> {
    // appLogoOverride 头像：未打包应用只能走本地 file:/// 绝对路径，
    // PNG 随 bundle.resources 安装到 resource_dir（webp 不被 toast 支持）。
    let image_part = resolve_resource_image(app_handle, "resources/mascot-update-toast.png")
        .map(|path| format!(r#"<image placement="appLogoOverride" hint-crop="circle" src="{path}"/>"#))
        .unwrap_or_default();
    let xml = format!(
        r#"<toast activationType="foreground"><visual><binding template="ToastGeneric">{image_part}<text>发现新版本</text><text>新版本 v{version} 可用，前往关于界面进行更新</text></binding></visual></toast>"#
    );
    let doc = XmlDocument::new().map_err(|e| e.to_string())?;
    doc.LoadXml(&HSTRING::from(xml)).map_err(|e| e.to_string())?;
    let toast = ToastNotification::CreateToastNotification(&doc).map_err(|e| e.to_string())?;

    let app_h = app_handle.clone();
    toast
        .Activated(&TypedEventHandler::new(move |_, _| {
            if let Some(win) = app_h.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
            if let Err(e) = crate::infra::events::EventBus::new(&app_h).emit_update_notification_click() {
                crate::log_warn!("system", "通知点击事件发送失败: {}", e);
            }
            Ok(())
        }))
        .map_err(|e| e.to_string())?;

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
        .map_err(|e| e.to_string())?;
    notifier.Show(&toast).map_err(|e| e.to_string())
}
