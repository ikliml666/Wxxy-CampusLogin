//! 系统通知 WinRT Toast（通用通知 + 带点击回调的更新提醒）
//!
//! tauri-plugin-notification 桌面端（notify-rust → tauri-winrt-notification）
//! 不暴露 Activated 回调，点击通知只有"激活应用"的系统默认行为；
//! 更新提醒需要"点击通知 → 唤起主窗口并跳转关于界面"，因此单独经
//! windows crate 自组 toast XML 并注册 Activated 处理器。
//!
//! AUMID：未打包应用默认借用 PowerShell 的 AUMID，但通知中心来源会显示
//! "Windows PowerShell"。首次发通知前向 HKCU\Software\Classes\AppUserModelId\
//! 注册自有 AUMID（DisplayName + IconUri，HKCU 免提权），注册成功后通知来源
//! 显示应用名与应用图标；注册失败（策略封锁等）回退 PowerShell AUMID，
//! 保证通知仍能弹出。

use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

/// 自有 AUMID：与应用 identifier 一致，注册表键 Software\Classes\AppUserModelId\ 下同名
const AUMID: &str = "com.campus.login";
/// 兜底 AUMID：自有 AUMID 注册失败时沿用（旧版行为，来源显示 PowerShell）
const FALLBACK_APP_ID: &str =
    "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";
const APP_DISPLAY_NAME: &str = "校园网登录助手";
/// 通知中心来源角标图标：随 bundle.resources 安装到 resource_dir
const AUMID_ICON_RESOURCE: &str = "icons/128x128.png";

/// 进程内缓存注册结果（成功/失败都只探一次），返回实际可用的 notifier AUMID
static NOTIFIER_AUMID: OnceLock<&'static str> = OnceLock::new();

fn notifier_aumid(app_handle: &AppHandle) -> &'static str {
    NOTIFIER_AUMID.get_or_init(|| {
        match register_aumid(app_handle) {
            Ok(()) => AUMID,
            Err(e) => {
                crate::log_warn!("system", "自有 AUMID 注册失败，通知来源回退系统默认: {e}");
                FALLBACK_APP_ID
            }
        }
    })
}

/// 向 HKCU\Software\Classes\AppUserModelId\<AUMID> 写 DisplayName 与 IconUri。
/// IconUri 可选：dev 模式 resource_dir 下没有安装图标，缺失只影响来源角标，
/// 来源名仍显示应用名。
fn register_aumid(app_handle: &AppHandle) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(format!(r"Software\Classes\AppUserModelId\{AUMID}"))
        .map_err(|e| e.to_string())?;
    key.set_value("DisplayName", &APP_DISPLAY_NAME)
        .map_err(|e| e.to_string())?;
    if let Ok(dir) = app_handle.path().resource_dir() {
        let icon = dir.join(AUMID_ICON_RESOURCE);
        if icon.is_file() {
            // IconUri 用本地绝对路径（toast XML 的 src 才需要 file:/// 前缀）
            let path = icon.to_string_lossy().replace('\\', "/");
            key.set_value("IconUri", &path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

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
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(notifier_aumid(app_handle)))
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

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(notifier_aumid(app_handle)))
        .map_err(|e| e.to_string())?;
    notifier.Show(&toast).map_err(|e| e.to_string())
}
