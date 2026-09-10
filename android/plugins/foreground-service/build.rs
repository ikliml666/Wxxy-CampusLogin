const COMMANDS: &[&str] = &["startMonitor", "stopMonitor", "updateNotification", "setBootAutostart", "isBootAutostartEnabled", "installApk"];

fn main() {
    let result = tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .try_build();
    if !(cfg!(docsrs) && std::env::var("TARGET").unwrap().contains("android")) {
        result.unwrap();
    }
}
