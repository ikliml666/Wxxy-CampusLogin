const AUTOSTART_REG_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_REG_VALUE: &str = "CampusLogin";

pub fn get_auto_launch_enabled() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(AUTOSTART_REG_KEY) {
        if let Ok(val) = key.get_value::<String, _>(AUTOSTART_REG_VALUE) {
            return !val.is_empty();
        }
    }
    false
}

pub fn set_auto_start(exe_path: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(AUTOSTART_REG_KEY, KEY_SET_VALUE)
        .map_err(|e| format!("打开注册表失败: {}", e))?;

    let value = format!("\"{}\" --autostart", exe_path);
    key.set_value(AUTOSTART_REG_VALUE, &value)
        .map_err(|e| format!("写入注册表失败: {}", e))?;

    Ok(())
}

pub fn remove_auto_start() -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(AUTOSTART_REG_KEY, KEY_SET_VALUE)
        .map_err(|e| format!("打开注册表失败: {}", e))?;

    if let Err(e) = key.delete_value(AUTOSTART_REG_VALUE) {
        crate::log_warn!("system", "删除自启动注册表项失败: {}", e);
    }

    Ok(())
}
