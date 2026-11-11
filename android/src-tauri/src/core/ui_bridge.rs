//! UI 进程侧的事件桥:UI 的 Kotlin 收到 core 广播后调用这里,再 emit 给 WebView。
//!
//! 只有 UI 进程有 `AppHandle`;:core 进程里 `APP` 恒为 None,调用即静默丢弃。

use std::sync::{Mutex, OnceLock};

static APP: OnceLock<Mutex<Option<tauri::AppHandle>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<tauri::AppHandle>> {
    APP.get_or_init(|| Mutex::new(None))
}

pub fn set_app_handle(app: tauri::AppHandle) {
    if let Ok(mut guard) = slot().lock() {
        *guard = Some(app);
    }
}

pub fn emit_to_ui(event: &str, payload: serde_json::Value) {
    use tauri::Emitter;
    if let Ok(guard) = slot().lock() {
        if let Some(app) = guard.as_ref() {
            let _ = app.emit(event, payload);
        }
    }
}

#[cfg(target_os = "android")]
mod jni_export {
    use jni::objects::{JClass, JString};
    use jni::JNIEnv;

    fn read(env: &mut JNIEnv, value: &JString) -> String {
        env.get_string(value)
            .map(|s| s.into())
            .unwrap_or_default()
    }

    /// UI 进程 Kotlin 收到 core 事件后转发给 WebView
    #[no_mangle]
    pub extern "system" fn Java_com_campuslogin_plugin_monitorservice_CoreEvents_nativeEmitToUi(
        mut env: JNIEnv,
        _class: JClass,
        event: JString,
        payload: JString,
    ) {
        let event = read(&mut env, &event);
        let payload = read(&mut env, &payload);
        let value: serde_json::Value =
            serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
        super::emit_to_ui(&event, value);
    }
}