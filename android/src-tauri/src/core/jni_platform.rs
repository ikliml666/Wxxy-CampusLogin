//! :core 进程的平台实现:全部经 Kotlin `CoreService` 的实例方法往返。
//!
//! 调用签名严格按 `campus_core_logic::platform_sig` 的映射:
//! String→String 只给加解密用,`(String)V` 给通知,`()V` 给探针/监听,
//! 不再把无参/Unit 方法按 String 签名调用(2026-11-11 NoSuchMethodError 缺陷)。

use std::path::PathBuf;
use std::sync::Arc;

use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::sys::jstring;
use jni::{JNIEnv, JavaVM};

use campus_core_logic::platform_sig::JavaSignature;
use campus_core_logic::status::CoreStatus;

use crate::core::platform::CorePlatform;
use crate::core::status_file;

pub struct JniPlatform {
    vm: JavaVM,
    service: GlobalRef,
    data_dir: PathBuf,
}

impl JniPlatform {
    fn call_str_str(&self, method: &str, arg: &str) -> Result<String, String> {
        let mut env = self.vm.attach_current_thread().map_err(|e| e.to_string())?;
        let jarg = env.new_string(arg).map_err(|e| e.to_string())?;
        let out = env
            .call_method(
                self.service.as_obj(),
                method,
                JavaSignature::StrToStr.descriptor(),
                &[JValue::Object(&jarg)],
            )
            .map_err(|e| format!("{method}: {e}"))?;
        let obj = out.l().map_err(|e| e.to_string())?;
        let jstr = JString::from(obj);
        env.get_string(&jstr)
            .map(|s| s.into())
            .map_err(|e| e.to_string())
    }

    fn call_zero_str(&self, method: &str) -> Result<String, String> {
        let mut env = self.vm.attach_current_thread().map_err(|e| e.to_string())?;
        let out = env
            .call_method(
                self.service.as_obj(),
                method,
                JavaSignature::ZeroToStr.descriptor(),
                &[],
            )
            .map_err(|e| format!("{method}: {e}"))?;
        let obj = out.l().map_err(|e| e.to_string())?;
        let jstr = JString::from(obj);
        env.get_string(&jstr)
            .map(|s| s.into())
            .map_err(|e| e.to_string())
    }

    fn call_str_void(&self, method: &str, arg: &str) -> Result<(), String> {
        let mut env = self.vm.attach_current_thread().map_err(|e| e.to_string())?;
        let jarg = env.new_string(arg).map_err(|e| e.to_string())?;
        env.call_method(
            self.service.as_obj(),
            method,
            JavaSignature::StrToVoid.descriptor(),
            &[JValue::Object(&jarg)],
        )
        .map(|_| ())
        .map_err(|e| format!("{method}: {e}"))
    }

    fn call_zero_void(&self, method: &str) -> Result<(), String> {
        let mut env = self.vm.attach_current_thread().map_err(|e| e.to_string())?;
        env.call_method(
            self.service.as_obj(),
            method,
            JavaSignature::ZeroToVoid.descriptor(),
            &[],
        )
        .map(|_| ())
        .map_err(|e| format!("{method}: {e}"))
    }

    fn call_two_str_void(&self, method: &str, a: &str, b: &str) -> Result<(), String> {
        let mut env = self.vm.attach_current_thread().map_err(|e| e.to_string())?;
        let ja = env.new_string(a).map_err(|e| e.to_string())?;
        let jb = env.new_string(b).map_err(|e| e.to_string())?;
        env.call_method(
            self.service.as_obj(),
            method,
            JavaSignature::TwoStrToVoid.descriptor(),
            &[JValue::Object(&ja), JValue::Object(&jb)],
        )
        .map(|_| ())
        .map_err(|e| format!("{method}: {e}"))
    }

    fn call_json(&self, method: &str) -> Result<serde_json::Value, String> {
        let raw = self.call_zero_str(method)?;
        serde_json::from_str(&raw).map_err(|e| format!("{method} 返回非 JSON: {e}"))
    }
}

impl CorePlatform for JniPlatform {
    fn decrypt(&self, cipher: &str) -> Result<String, String> {
        self.call_str_str("decrypt", cipher)
    }

    fn encrypt(&self, plain: &str) -> Result<String, String> {
        self.call_str_str("encrypt", plain)
    }

    fn emit(&self, event: &str, payload: serde_json::Value) {
        if let Err(e) = self.call_two_str_void("emit", event, &payload.to_string()) {
            eprintln!("[core] emit 失败: {e}");
        }
    }

    fn notify_system(&self, title: &str, body: &str, mascot: &str) {
        let payload = serde_json::json!({ "title": title, "body": body, "mascot": mascot });
        if let Err(e) = self.call_str_void("notifySystem", &payload.to_string()) {
            eprintln!("[core] 系统通知失败: {e}");
        }
    }

    fn update_notification(&self, text: &str) {
        if let Err(e) = self.call_str_void("updateNotification", text) {
            eprintln!("[core] 常驻通知更新失败: {e}");
        }
    }

    fn begin_probe_window(&self) {
        if let Err(e) = self.call_zero_void("beginProbeWindow") {
            eprintln!("[core] 探针窗口开启失败: {e}");
        }
    }

    fn end_probe_window(&self) {
        if let Err(e) = self.call_zero_void("endProbeWindow") {
            eprintln!("[core] 探针窗口释放失败: {e}");
        }
    }

    fn power_state(&self) -> (bool, bool) {
        match self.call_json("powerState") {
            Ok(v) => (
                v["screenOn"].as_bool().unwrap_or(true),
                v["wifiConnected"].as_bool().unwrap_or(true),
            ),
            Err(_) => (true, true),
        }
    }

    fn bind_to_wifi(&self) -> Result<serde_json::Value, String> {
        self.call_json("bindToWifi")
    }

    fn accept_wifi_network(&self) -> Result<serde_json::Value, String> {
        self.call_json("acceptWifiNetwork")
    }

    fn wifi_ssid(&self) -> Option<String> {
        self.call_zero_str("wifiSsid")
            .ok()
            .filter(|s| !s.is_empty())
    }

    fn report_wifi_unusable(&self) -> Result<serde_json::Value, String> {
        self.call_json("reportWifiUnusable")
    }

    fn ensure_avoid_bad_wifi(&self) -> Result<serde_json::Value, String> {
        self.call_json("ensureAvoidBadWifi")
    }

    fn restore_avoid_bad_wifi(&self) -> Result<serde_json::Value, String> {
        self.call_json("restoreAvoidBadWifi")
    }

    fn start_wifi_watcher(&self) -> Result<(), String> {
        // 必须传播失败:静默吞掉会让 WATCHER_FAILED 永不置位、WiFi 即时触发失效
        self.call_zero_void("startWifiWatcher")
    }

    fn stop_wifi_watcher(&self) {
        if let Err(e) = self.call_zero_void("stopWifiWatcher") {
            eprintln!("[core] WiFi 监听注销失败: {e}");
        }
    }

    fn stop_service(&self) {
        if let Err(e) = self.call_zero_void("runIdleStop") {
            eprintln!("[core] 空闲服务退出失败: {e}");
        }
    }

    fn read_status(&self) -> CoreStatus {
        status_file::read(&self.data_dir)
    }

    fn write_status(&self, status: &CoreStatus) {
        status_file::write(&self.data_dir, status);
    }
}

fn read_string(env: &mut JNIEnv, value: &JString) -> String {
    env.get_string(value)
        .map(|s| s.into())
        .unwrap_or_default()
}

#[no_mangle]
pub extern "system" fn Java_com_campuslogin_plugin_monitorservice_CoreService_nativeStart(
    mut env: JNIEnv,
    _class: JClass,
    service: JObject,
    data_dir: JString,
) {
    let dir = read_string(&mut env, &data_dir);
    let Ok(vm) = env.get_java_vm() else { return };
    let Ok(global) = env.new_global_ref(service) else {
        return;
    };
    let platform = Arc::new(JniPlatform {
        vm,
        service: global,
        data_dir: PathBuf::from(&dir),
    });
    crate::core::runner::start(PathBuf::from(dir), platform);
}

#[no_mangle]
pub extern "system" fn Java_com_campuslogin_plugin_monitorservice_CoreService_nativeCommand(
    mut env: JNIEnv,
    _class: JClass,
    action: JString,
    args: JString,
) -> jstring {
    let action = read_string(&mut env, &action);
    let args = read_string(&mut env, &args);
    let out = crate::core::runner::command(&action, &args);
    env.new_string(out)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_campuslogin_plugin_monitorservice_CoreService_nativeWifiEvent(
    mut env: JNIEnv,
    _class: JClass,
    event: JString,
) {
    let event = read_string(&mut env, &event);
    crate::core::runner::wifi_event(&event);
}

#[no_mangle]
pub extern "system" fn Java_com_campuslogin_plugin_monitorservice_CoreService_nativeHeartbeat(
    _env: JNIEnv,
    _class: JClass,
) {
    crate::core::runner::heartbeat();
}

#[no_mangle]
pub extern "system" fn Java_com_campuslogin_plugin_monitorservice_CoreService_nativeStop(
    _env: JNIEnv,
    _class: JClass,
) {
    crate::core::runner::shutdown();
}
