---
title: AndroidKeyStore 密钥按包名隔离导致改 identifier 后密码失效
type: learning
source_files:
  - android/src-tauri/gen/android/app/build.gradle.kts
  - android/src-tauri/src/config_state.rs
tags: [教训, 安卓, 包名, keystore, 破坏性变更]
---

## Overview

AndroidKeyStore 的密钥生成与访问严格绑定应用的 `applicationId`（包名）。一旦应用 identifier 发生变更（如重构包名），旧版本 APK 生成的 KeyStore 密钥在新包名下不可见或无法使用，导致所有依赖该密钥加密落盘的密文数据（如用户密码）永久失效。

在 CampusLogin 项目中，用户密码通过 `config_state.rs` 中的 `CryptoBridge` 桥接层，调用 `tauri-plugin-campus-keystore` 插件进行 AES-GCM 加密后写入磁盘（真机路径见 `CryptoBridge::from_app` 行211-230，实际加密调用行217-222、223-228）。由于 KeyStore 密钥隔离特性，任何导致 `applicationId` 变化的构建产物都会使存量用户的密码密文变成“乱码”（无法解密），进而导致配置加载时密码字段置空（保护逻辑见 `load_file` 行259-261），用户被迫重新输入。

当前 `build.gradle.kts` 中 `namespace` (行18) 与 `applicationId` (行21) 均为 `com.campuslogin.client`；`signingConfigs.release` (行29-36) 使用本机 debug.keystore，注释明确“签名证书一经发布不可更换(换证书=用户必须卸载重装)”(行27-28)。这两项共同构成密钥隔离边界与数据持久性约束。

## Key Components

### build.gradle.kts (`android/src-tauri/gen/android/app/build.gradle.kts`)

定义了 Android 应用的标识符与签名策略，是理解隔离边界的入口。

- **namespace** (行18): `com.campuslogin.client`。Gradle 命名空间，决定生成的 R 类包路径及最终 APK 的内部标识。
- **applicationId** (行21): `com.campuslogin.client`。AndroidManifest 中的 package 属性，直接对应 KeyStore 的密钥别名前缀隔离域。
- **signingConfigs.release** (行29-36, 注释见行27-28): 
  ```kotlin
  signingConfigs {
      create("release") {
          storeFile = file(System.getProperty("user.home") + "/.android/debug.keystore")
          storePassword = "android"
          keyAlias = "androiddebugkey"
          keyPassword = "android"
      }
  }
  ```
  使用本机 debug.keystore 进行签名。注释明确：“签名证书一经发布不可更换(换证书=用户必须卸载重装)”。虽然 KeyStore 主要按包名隔离，但换签名通常伴随 uninstall/install 周期，进一步加剧了数据丢失风险。
- **applicationVariants** (行38-44): 自定义输出文件名 `Wxxy-CampusLogin_${variantVersionName}.apk`（行41-42），与桌面安装包命名同源。

### config_state.rs (`android/src-tauri/src/config_state.rs`)

负责配置的序列化、加密落盘与解密读取。

- **`PASSWORD_MASK`** (行10): `pub const PASSWORD_MASK: &str = "***"`。出站掩码占位符。
- **`CONFIG_FILE`** (行11): `const CONFIG_FILE: &str = "config.json"`。配置文件名。
- **`CONFIG_IO_LOCK`** (行17-19): `lazy_static! { static ref CONFIG_IO_LOCK: tokio::sync::Mutex<()> = ... }`。配置 IO 串行锁，防止 save_config / switch / delete 并发竞态。
- **`config_io_lock()`** (行22-24): 获取配置 IO 锁的异步函数，返回 `MutexGuard`。
- **`Settings` struct** (行26-128): 
  主配置结构体，映射前端 IPC 契约 (camelCase, 行27)。全部字段如下：
  - `user: String` (行30)
  - `password: String` (行31) (**关键字段**: 落盘前经 AndroidKeyStore AES-GCM 加密)
  - `self_hello_enabled: bool` (行32)
  - `self_reverify_each_action: bool` (行33)
  - `allow_2d_face_verify: bool` (行35, 注释行34)
  - `operator: String` (行36)
  - `enable_night_operator_switch: bool` (行41, 注释行38-40)
  - `night_operator_restore: String` (行42)
  - `enable_night_outbound_switch: bool` (行47, 注释行44-46)
  - `outbound_priority: Vec<String>` (行50, 注释行48-49)
  - `outbound_metric_restore: String` (行53, 注释行51-52)
  - `outbound_disabled_adapters: String` (行56, 注释行54-55)
  - `outbound_standby_route: String` (行59, 注释行57-58)
  - `night_outbound_restore: String` (行64, 注释行60-63)
  - `dns_optimize_adapters: Vec<String>` (行67, 注释行65-66)
  - `device_aliases: std::collections::HashMap<String, String>` (行72, 注释行68-71, 含 `#[serde(default)]` 行71)
  - `auto_login_on_start: bool` (行74)
  - `enable_background_check: bool` (行75)
  - `background_check_interval: u64` (行76)
  - `background_check_idle_interval: u64` (行81, 注释行77-80)
  - `auto_login_on_preparation: bool` (行82)
  - `max_disconnect_reconnect: u32` (行83)
  - `auto_login_cooldown_secs: u64` (行84)
  - `theme_mode: String` (行86)
  - `enable_notification: bool` (行87)
  - `custom_theme_color: String` (行88)
  - `default_panel: String` (行89)
  - `display_name: String` (行93, 注释行90-92)
  - `active_account: String` (行94)
  - `enable_boot_autostart: bool` (行95)
  - `enable_latency_test: bool` (行97)
  - `latency_test_interval: u64` (行98)
  - `enable_network_quality: bool` (行99)
  - `skip_ttfb_in_latency: bool` (行100)
  - `skip_content_in_latency: bool` (行101)
  - `portal_url: String` (行103)
  - `fixed_gateway: String` (行104)
  - `required_network_name: String` (行105)
  - `enable_network_name_check: bool` (行106)
  - `campus_gateway: String` (行107)
  - `campus_check_start_minutes: u16` (行109, 注释行108)
  - `campus_check_end_minutes: u16` (行111, 注释行110)
  - `scheduled_login_minutes: u16` (行113, 注释行112)
  - `scheduled_logout_minutes: u16` (行115, 注释行114)
  - `update_source: String` (行118, 注释行117)
  - `log_retention_days: u32` (行120)
  - `config_schema_version: u32` (行127, 注释行121-126)
  
- **`Settings::default()`** (行130-200): 实现 Default trait，定义所有字段初始值。特别注意 `config_schema_version: 7` (行197) 表示新装即当前版本，不触发迁移。默认值细节：如 `background_check_interval: 60_000` (行155), `background_check_idle_interval: 300_000` (行157), `latency_test_interval: 600_000` (行170), `required_network_name: "i-wxxy、iwxxy-2、iwxxy-3"` (行181), `enable_night_operator_switch: false` (行141, 2026-10-02 起默认关闭), `scheduled_login_minutes: 1440` (行192) 等。
- **`CryptoBridge` struct** (行202-207): 
  ```rust
  #[derive(Clone)]
  pub struct CryptoBridge {
      pub encrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
      pub decrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
  }
  ```
  加解密桥接器，允许在测试环境注入假实现，在真机绑定 KeyStore 插件。
- **`CryptoBridge::from_app()`** (行209-240): 
  根据编译目标绑定真实实现：
  - `#[cfg(mobile)]` (行211-230): 调用 `tauri_plugin_campus_keystore::CampusKeystoreExt` 的 `encrypt`/`decrypt` (行213, 219, 225)。这是真正使用 AndroidKeyStore 的路径。
  - `#[cfg(not(mobile))]` (行231-238): 返回错误 "Keystore 仅移动端可用"。
- **`EncodedSettings` struct** (行242-248): 
  磁盘存储格式 (`#[serde(rename_all = "camelCase")]` 行244)。
  - `settings: Settings` (行246, 密码字段为空)
  - `password_cipher: String` (行247, 加密后的密码密文)
- **`load_file()`** (行250-263): 
  ```rust
  pub async fn load_file(path: &Path, bridge: &CryptoBridge) -> Result<Settings, String>
  ```
  从磁盘读取并解密配置。关键逻辑 (行259-261): 
  ```rust
  if !encoded.password_cipher.is_empty() {
      s.password = (bridge.decrypt)(&encoded.password_cipher).unwrap_or_default();
  }
  ```
  **保护机制**: 如果 `decrypt` 失败（例如因包名改变导致密钥不存在，GCM 认证失败），`unwrap_or_default()` 会将 `s.password` 设为空字符串，而不是报错崩溃。这使得配置其他字段依然可用，但用户处于“未登录/需重输密码”状态。
- **`password_saved_at()`** (行265-276): 
  ```rust
  pub fn password_saved_at(path: &Path) -> bool
  ```
  账号卡“密码是否已保存”判据。必须落在磁盘形态密文位 `password_cipher` (行270-275)，而非外露 `settings.password`（落盘时被清空）。读取失败返回 false。
- **`save_file()`** (行278-308): 
  ```rust
  pub async fn save_file(path: &Path, bridge: &CryptoBridge, s: &Settings) -> Result<(), String>
  ```
  加密并保存配置。关键逻辑 (行286-291): 
  ```rust
  if !s.password.is_empty() {
      match (bridge.encrypt)(&s.password) {
          Ok(c) => { eprintln!("[save_config] password encrypt ok len={}", c.len()); password_cipher = c; }
          Err(e) => { eprintln!("[save_config] password encrypt FAILED: {e}"); return Err(format!("password encrypt: {e}")); }
      }
  }
  ```
  构造 `EncodedSettings` 时刻意清空 `settings.password` (行293)。原子写入：先写带 pid 的 `.tmp` 文件 (行300-305)，再 rename (行306)。
- **`cleanup_stale_config_tmp()`** (行310-324): 清理进程崩溃遗留的配置临时文件（匹配 `*.json.<pid>.tmp` 形状）。
- **`load_from()`** (行326-336): 加载配置并执行 `migrate_legacy_defaults` (行328) 和旧默认单值 SSID 扩三值 (行332-334)。
- **`migrate_legacy_defaults()`** (行338-417, 私有): 根据 `config_schema_version` 一次性迁移旧默认值（v0→v1 间隔升级, v1→v2 自动化开启, v2→v3 质量关闭, v3→v4 闲时间隔, v4→v5 检测终点, v5→v6 质量间隔, v6→v7 夜切默认开及定时禁用值 0→1440）。迁移结果落盘 (行371,380,389,398,413)，失败静默。
- **`save_to()`** (行419-421): 保存配置的便捷函数，调用 `save_file`。
- **`masked_for_display()`** (行423-430): 将非空密码替换为 `"***"` 后序列化为 JSON Value，作为敏感信息出站唯一出口。
- **`emit_config_changed()`** (行432-439): 广播 `config-changed` 事件，payload 为 `{ "config": 掩码后配置 }` (行438)，前端依赖该事件同步 store。
- **`resolve_password_field()`** (行441-451): 处理密码更新逻辑：空串或掩码视为“未修改”，保留旧值；显式清除标志则置空。
- **`get_config()`** (行453-466): `#[tauri::command]` 加载配置并缓存到 `AndroidState`，返回掩码后配置。
- **`save_config()`** (行468-499): `#[tauri::command]` 持锁合并配置 (行477)、加密保存、自动建号 (行494)、广播事件 (行497)。
- **`current_settings()`** (行501-520): 优先从缓存读取，未命中则读盘，供登录等命令回退已存凭据。

### 测试代码 (`config_state.rs` 内部 `mod tests`, 行522-827)

- **`fake_bridge()`** (行528-537): 使用 Base64 编码模拟可逆加密，用于单元测试。
- **`base64_encode`** (行539-551) / **`base64_decode`** (行553-568): 手动实现 Base64 编解码。
- **`tmp_dir()`** (行570-574): 创建带 PID 后缀的临时测试目录。
- **`sample_settings()`** (行576-584): 构造包含明文密码的测试用例。
- **`加密落盘_文件不含明文_读回等于原文()`** (行586-598): 验证 Roundtrip 加密解密正确性。
- **`解密失败时置空_配置仍可加载()`** (行600-614): **核心测试**。模拟密钥失效（不同 Bridge），验证 `load_from` 返回空密码而非错误，且其他字段正常。
- **`旧默认单值SSID_载入时扩为三值()`** (行616-630): 验证 SSID 迁移逻辑。
- **`默认值_业务语义()`** (行632-664): 断言所有默认值符合预期（含 `config_schema_version == 7` 行646, `enable_night_operator_switch == false` 行647 等）。
- **`迁移_旧默认15s升60s且落盘后用户值不被覆盖()`** (行666-711): 验证多阶段迁移逻辑及“用户主动修改后不再覆盖”原则。
- **`迁移_非旧默认值保持不动()`** (行713-725): 验证非默认值不被迁移脚本篡改。
- **`迁移_v5质量间隔旧默认刷v6且用户值不被覆盖()`** (行727-756): 验证特定版本迁移边界。
- **`camelCase_序列化契约()`** (行758-766): 验证 JSON 键名格式。
- **`掩码出口_非空变星号_空保持空()`** (行768-777): 验证 `masked_for_display`。
- **`密码字段回退语义_与桌面同构()`** (行779-791): 验证 `resolve_password_field` 边界。
- **`空密码不写密文位()`** (行793-804): 验证空密码不产生 `passwordCipher` 字段。
- **`密码已保存判据_落盘后仍为已保存()`** (行806-826): 验证 `password_saved_at` 基于密文位判据正确。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                      Android App (Tauri)                           │
│                                                                     │
│  ┌──────────────────┐      ┌──────────────────────┐                │
│  │  Frontend (Vue)  │─────►│  Tauri Commands      │                │
│  │                  │      │  (config_state.rs)   │                │
│  │  get_config      │      │  ┌────────────────┐  │                │
│  │  save_config     │      │  │ CryptoBridge   │  │                │
│  └──────────────────┘      │  │ (Encrypt/Decrypt)│  │                │
│                            │  └───────┬────────┘  │                │
│                            │          │           │                │
│                            │  ┌───────▼────────┐  │                │
│                            │  │ EncodedSettings│  │                │
│                            │  │ (JSON + Cipher)│  │                │
│                            │  └───────┬────────┘  │                │
│                            │          │           │                │
│                            │  ┌───────▼────────┐  │                │
│                            │  │ File System    │  │                │
│                            │  │ (config.json)  │  │                │
│                            │  └────────────────┘  │                │
│                             └──────────────────────┘                │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  Android Native Layer                                       │    │
│  │  ┌──────────────────────────────────────────────────────┐   │    │
│  │  │ tauri-plugin-campus-keystore (行213,219,225)         │   │    │
│  │  │  ┌────────────────────────────────────────────────┐   │   │    │
│  │  │  │ AndroidKeyStore                                 │   │   │    │
│  │  │  │  Alias bound by applicationId                   │   │   │    │
│  │  │  │  (com.campuslogin.client, build.gradle 行21)    │   │   │    │
│  │  │  └────────────────────────────────────────────────┘   │   │    │
│  │  └──────────────────────────────────────────────────────┘   │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

1. **Frontend**: 发起 `get_config` 或 `save_config` 请求。
2. **Tauri Commands (`config_state.rs`)**: 
   - `get_config`: 调用 `load_from` → `load_file` → `CryptoBridge.decrypt` → `masked_for_display`。
   - `save_config`: 调用 `save_to` → `save_file` → `CryptoBridge.encrypt` → 写入 JSON。
3. **CryptoBridge**: 
   - 在移动端 (`#[cfg(mobile)]`)，委托给 `tauri-plugin-campus-keystore`。
   - 该插件底层调用 Android API 访问 KeyStore。
4. **AndroidKeyStore**: 
   - 密钥生成/获取时，Android 系统将该密钥与当前应用的 `applicationId` (行21) 绑定。
   - 如果 `applicationId` 改变，新 APK 无法找到旧密钥，`decrypt` 操作失败。

## Data Flow

### 正常加载流程 (Load)

1. `get_config` command (行453) 被调用。
2. `load_from` (行326) 读取 `config.json`。
3. `serde_json` 解析为 `EncodedSettings` (行255-256)。
4. `CryptoBridge.decrypt` 被调用，传入 `password_cipher` (行260)。
5. **分支 A (成功)**: KeyStore 密钥存在且匹配 → 解密成功 → `s.password` 填入原文。
6. **分支 B (失败)**: KeyStore 密钥缺失或不匹配 (如包名变更) → `decrypt` 返回 Error → `unwrap_or_default()` 执行 (行260) → `s.password` 变为 `""`。
7. `masked_for_display` 检查 `s.password`：若为空，JSON 中为 `""`；若不为空，为 `"***"` (行426-428)。
8. 前端收到配置，发现密码为空，提示用户重新输入。

### 正常保存流程 (Save)

1. `save_config` command (行468) 被调用，携带新配置。
2. `resolve_password_field` (行485-486) 确定最终密码：如果是新输入或非掩码值，则保留；否则沿用旧值。
3. `save_file` (行278) 被调用。
4. **分支 A (有密码)**: `CryptoBridge.encrypt` 被调用 (行287) → KeyStore 生成/使用密钥加密 → 得到 `password_cipher`。
5. **分支 B (无密码)**: `password_cipher` 保持空字符串 (行284)。
6. 构造 `EncodedSettings` (行292-295)，序列化 JSON。
7. 原子写入：写入 `.tmp` 文件 (行305) → `rename` 覆盖原文件 (行306)。

### 包名变更后的灾难场景

1. 开发者修改 `build.gradle.kts` 中的 `namespace` (行18) 和 `applicationId` (行21) (例如从 `com.campuslogin.client` 改为 `com.new.pkg`)。
2. 重新构建 APK 并安装。
3. 用户打开 App，触发 `get_config`。
4. `CryptoBridge` 尝试访问 KeyStore。Android 系统查找 `com.new.pkg` 对应的密钥别名。
5. 旧密钥存储在 `com.campuslogin.client` 名下，**不可见**。
6. `decrypt` 调用失败 (通常抛出 `UnrecoverableKeyException` 或 `BadPaddingException`)。
7. `config_state.rs` 行259-261 的 `unwrap_or_default()` 捕获错误，返回空字符串。
8. 用户的所有存量密码丢失，必须重新输入；且因签名证书不可更换 (行27-28)，若同时换证书需卸载重装。

## Connections

[[android-generated-project-discipline]] (强调 identifier 变更的破坏性)
[[mask-placeholder-persisted-as-plaintext]] (讨论掩码语义，与此处的加密落盘形成对比)

## Known Issues

- **包名/签名变更破坏性**：当前 `applicationId` 为 `com.campuslogin.client` (行21)，一旦修改，存量用户密码密文因 KeyStore 隔离不可解密，只能置空重输 (行259-261)。签名证书同样不可更换 (行27-28)，否则需卸载重装。此限制要求在产品生命周期内冻结 identifier 与签名。
- **缓解而非根治**：代码通过 `unwrap_or_default()` 保证配置其他字段可用，但不会尝试密钥迁移或跨标识符解密。若确需变更包名，需配套提供用户密码导出/导入或重新登录引导。
- **测试覆盖**：`解密失败时置空_配置仍可加载()` (行600-614) 验证了密钥失效时的降级行为，但未模拟真实包名变更后的 KeyStore 别名丢失场景（仅用假桥模拟），真机回归需手动验证。
- **配置迁移不涉及密钥**：`migrate_legacy_defaults` (行338-417) 仅处理 `config_schema_version` 字段默认值，与 KeyStore 密钥无关，无法修复因包名变更导致的密文失效。