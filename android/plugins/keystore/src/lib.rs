#![cfg(mobile)]
//! 移动插件:AndroidKeyStore AES-GCM 加解密原语。
//! 用途与桌面 DPAPI 同构:password/selfPassword 落盘前加密、读出后解密,前端无感知。

use serde::Deserialize;
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.campuslogin.plugin.keystore";

#[derive(Deserialize)]
struct EncryptResult {
    data: String,
}

#[derive(Deserialize)]
struct DecryptResult {
    text: String,
}

pub struct CampusKeystore<R: Runtime>(PluginHandle<R>);

type Result<T> = std::result::Result<T, tauri::plugin::mobile::PluginInvokeError>;

impl<R: Runtime> CampusKeystore<R> {
    /// 明文 → base64(iv + ciphertext)
    pub fn encrypt(&self, text: &str) -> Result<String> {
        let r: EncryptResult = self.0.run_mobile_plugin("encrypt", serde_json::json!({ "text": text }))?;
        Ok(r.data)
    }

    /// base64(iv + ciphertext) → 明文;密文损坏时由 GCM 认证失败拒绝
    pub fn decrypt(&self, data: &str) -> Result<String> {
        let r: DecryptResult = self.0.run_mobile_plugin("decrypt", serde_json::json!({ "data": data }))?;
        Ok(r.text)
    }
}

pub trait CampusKeystoreExt<R: Runtime> {
    fn campus_keystore(&self) -> &CampusKeystore<R>;
}

impl<R: Runtime, T: Manager<R>> CampusKeystoreExt<R> for T {
    fn campus_keystore(&self) -> &CampusKeystore<R> {
        self.state::<CampusKeystore<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("campus-keystore")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "KeystorePlugin")?;
            app.manage(CampusKeystore(handle));
            Ok(())
        })
        .build()
}
