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

在 CampusLogin 项目中，用户密码通过 `config_state.rs` 中的 `CryptoBridge` 桥接层，调用 `tauri-plugin-campus-keystore` 插件进行 AES-GCM 加密后写入磁盘。由于 KeyStore 密钥隔离特性，任何导致 `applicationId` 变化的构建产物都会使存量用户的密码密文变成“乱码”（无法解密），进而导致配置加载时密码字段置空，用户被迫重新输入。

## Key Components

### build.gradle.kts (`android/src-tauri/gen/android/app/build.gradle.kts`)

定义了 Android 应用的标识符与签名策略，是理解隔离边界的入口。

- **namespace** (Line 14): `com.campuslogin.client`。Gradle 命名空间，决定生成的 R 类包路径及最终 APK 的内部标识。
- **applicationId** (Line 17): `com.campuslogin.client`。AndroidManifest 中的 package 属性，直接对应 KeyStore 的密钥别名前缀隔离域。
- **signingConfigs.release** (Lines 26-33): 
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
- **applicationVariants** (Lines 35-40): 自定义输出文件名 `Wxxy-CampusLogin_${variantVersionName}.apk`。

### config_state.rs (`android/src-tauri/src/config_state.rs`)

负责配置的序列化、加密落盘与解密读取。

- **`PASSWORD_MASK`** (Line 12): `const PASSWORD_MASK: &str = "***"`。出站掩码占位符。
- **`CONFIG_FILE`** (Line 13): `const CONFIG_FILE: &str = "config.json"`。配置文件名。
- **`CONFIG_IO_LOCK`** (Line 23): `lazy_static! { static ref CONFIG_IO_LOCK: tokio::sync::Mutex<()> }`。配置 IO 串行锁，防止 save_config / switch / delete 并发竞态。
- **`config_io_lock()`** (Line 28): 获取配置 IO 锁的异步函数。
- **`Settings` struct** (Lines 32-129): 
  主配置结构体，映射前端 IPC 契约 (camelCase)。
  - `user`: String
  - `password`: String (**关键字段**: 落盘前经 AndroidKeyStore AES-GCM 加密)
  - `self_hello_enabled`: bool
  - `self_reverify_each_action`: bool
  - `allow_2d_face_verify`: bool
  - `operator`: String
  - `enable_night_operator_switch`: bool
  - `night_operator_restore`: String
  - `enable_night_outbound_switch`: bool
  - `outbound_priority`: Vec<String>
  - `outbound_metric_restore`: String
  - `outbound_disabled_adapters`: String
  - `outbound_standby_route`: String
  - `night_outbound_restore`: String
  - `dns_optimize_adapters`: Vec<String>
  - `device_aliases`: HashMap<String, String>
  - `auto_login_on_start`: bool
  - `enable_background_check`: bool
  - `background_check_interval`: u64 (默认 60_000ms)
  - `background_check_idle_interval`: u64 (默认 300_000ms)
  - `auto_login_on_preparation`: bool
  - `max_disconnect_reconnect`: u32
  - `auto_login_cooldown_secs`: u64
  - `theme_mode`: String
  - `enable_notification`: bool
  - `custom_theme_color`: String
  - `default_panel`: String
  - `display_name`: String
  - `active_account`: String
  - `enable_boot_autostart`: bool
  - `enable_latency_test`: bool
  - `latency_test_interval`: u64 (默认 600_000ms)
  - `enable_network_quality`: bool
  - `skip_ttfb_in_latency`: bool
  - `skip_content_in_latency`: bool
  - `portal_url`: String
  - `fixed_gateway`: String
  - `required_network_name`: String (默认 "i-wxxy、iwxxy-2、iwxxy-3")
  - `enable_network_name_check`: bool
  - `campus_gateway`: String
  - `campus_check_start_minutes`: u16
  - `campus_check_end_minutes`: u16
  - `scheduled_login_minutes`: u16
  - `scheduled_logout_minutes`: u16
  - `update_source`: String
  - `log_retention_days`: u32
  - `config_schema_version`: u32 (当前版本 7，用于旧默认值迁移)
  
- **`Settings::default()`** (Lines 131-195): 实现 Default trait，定义所有字段的初始值。特别注意 `config_schema_version: 7` 表示新装即当前版本，不触发迁移。
- **`CryptoBridge` struct** (Lines 198-202): 
  ```rust
  pub struct CryptoBridge {
      pub encrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
      pub decrypt: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
  }
  ```
  加解密桥接器，允许在测试环境注入假实现，在真机绑定 KeyStore 插件。
- **`CryptoBridge::from_app()`** (Lines 204-228): 
  根据编译目标绑定真实实现：
  - `#[cfg(mobile)]`: 调用 `tauri_plugin_campus_keystore::CampusKeystoreExt` 的 `encrypt`/`decrypt`。这是真正使用 AndroidKeyStore 的路径。
  - `#[cfg(not(mobile))]`: 返回错误 "Keystore 仅移动端可用"。
- **`EncodedSettings` struct** (Lines 231-235): 
  磁盘存储格式。
  - `settings`: Settings (密码字段为空)
  - `password_cipher`: String (加密后的密码密文)
- **`load_file()`** (Lines 237-253): 
  从磁盘读取并解密配置。
  ```rust
  pub async fn load_file(path: &Path, bridge: &CryptoBridge) -> Result<Settings, String>
  ```
  关键逻辑 (Lines 248-252): 
  ```rust
  if !encoded.password_cipher.is_empty() {
      s.password = (bridge.decrypt)(&encoded.password_cipher).unwrap_or_default();
  }
  ```
  **保护机制**: 如果 `decrypt` 失败（例如因包名改变导致密钥不存在，GCM 认证失败），`unwrap_or_default()` 会将 `s.password` 设为空字符串，而不是报错崩溃。这使得配置其他字段依然可用，但用户处于“未登录/需重输密码”状态。
- **`save_file()`** (Lines 255-288): 
  加密并保存配置。
  ```rust
  pub async fn save_file(path: &Path, bridge: &CryptoBridge, s: &Settings) -> Result<(), String>
  ```
  关键逻辑 (Lines 263-268): 
  ```rust
  if !s.password.is_empty() {
      match (bridge.encrypt)(&s.password) {
          Ok(c) => { /* ... */ password_cipher = c; }
          Err(e) => { /* ... */ return Err(format!("password encrypt: {e}")); }
      }
  }
  ```
  原子写入：先写 `.tmp` 文件，再 rename。
- **`cleanup_stale_config_tmp()`** (Lines 291-304): 清理崩溃遗留的临时配置文件。
- **`load_from()`** (Lines 306-323): 加载配置并执行 `migrate_legacy_defaults` 和 SSID 扩列。
- **`migrate_legacy_defaults()`** (Lines 325-398): 根据 `config_schema_version` 一次性迁移旧默认值（v0→v1 间隔升级, v1→v2 自动化开启, v2→v3 质量关闭, v3→v4 闲时间隔, v4→v5 检测终点, v5→v6 质量间隔, v6→v7 夜切默认开）。
- **`save_to()`** (Lines 400-402): 保存配置的便捷函数。
- **`masked_for_display()`** (Lines 405-411): 将非空密码替换为 `"***"` 后序列化为 JSON Value，作为敏感信息出站唯一出口。
- **`emit_config_changed()`** (Lines 414-419): 广播 `config-changed` 事件，payload 为掩码后的配置。
- **`resolve_password_field()`** (Lines 422-429): 处理密码更新逻辑：空串或掩码视为“未修改”，保留旧值；显式清除标志则置空。
- **`get_config()`** (Lines 432-444): Tauri 命令，加载配置并缓存到 `AndroidState`。
- **`save_config()`** (Lines 447-473): Tauri 命令，持锁合并配置、加密保存、自动建号、广播事件。
- **`current_settings()`** (Lines 476-489): 优先从缓存读取，未命中则读盘。

### 测试代码 (`config_state.rs` 内部 `mod tests`)

- **`fake_bridge()`** (Lines 496-503): 使用 Base64 编码模拟可逆加密，用于单元测试。
- **`base64_encode` / `base64_decode`** (Lines 505-545): 手动实现的 Base64 编解码，避免引入外部依赖。
- **`tmp_dir()`** (Lines 547-551): 创建带 PID 后缀的临时测试目录。
- **`sample_settings()`** (Lines 553-560): 构造包含明文密码的测试用例。
- **`加密落盘_文件不含明文_读回等于原文()`** (Lines 562-573): 验证 Roundtrip 加密解密正确性。
- **`解密失败时置空_配置仍可加载()`** (Lines 575-589): **核心测试**。模拟密钥失效（不同 Bridge），验证 `load_from` 返回空密码而非错误，且其他字段正常。
- **`旧默认单值SSID_载入时扩为三值()`** (Lines 591-602): 验证 SSID 迁移逻辑。
- **`默认值_业务语义()`** (Lines 604-642): 断言所有默认值符合预期。
- **`迁移_旧默认15s升60s且落盘后用户值不被覆盖()`** (Lines 644-687): 验证多阶段迁移逻辑及“用户主动修改后不再覆盖”的原则。
- **`迁移_非旧默认值保持不动()`** (Lines 689-698): 验证非默认值不被迁移脚本篡改。
- **`迁移_v5质量间隔旧默认刷v6且用户值不被覆盖()`** (Lines 700-731): 验证特定版本的迁移边界。
- **`camelCase_序列化契约()`** (Lines 733-739): 验证 JSON 键名格式。
- **`掩码出口_非空变星号_空保持空()`** (Lines 741-748): 验证 `masked_for_display`。
- **`密码字段回退语义_与桌面同构()`** (Lines 750-760): 验证 `resolve_password_field` 的各种边界情况。
- **`空密码不写密文位()`** (Lines 762-771): 验证空密码不产生 `passwordCipher` 字段。

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
│  │                                                             │    │
│  │  ┌──────────────────────────────────────────────────────┐   │    │
│  │  │ tauri-plugin-campus-keystore                          │   │    │
│  │  │  ┌────────────────────────────────────────────────┐   │   │    │
│  │  │  │ AndroidKeyStore                                 │   │   │    │
│  │  │  │  Alias: com.campuslogin.client...               │   │   │    │
│  │  │  │  (Bound by applicationId/package name)          │   │   │    │
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
   - 密钥生成/获取时，Android 系统将该密钥与当前应用的 `applicationId` 绑定。
   - 如果 `applicationId` 改变，新 APK 无法找到旧密钥，`decrypt` 操作失败。

## Data Flow

### 正常加载流程 (Load)

1. `get_config` command 被调用。
2. `load_from` 读取 `config.json`。
3. `serde_json` 解析为 `EncodedSettings`。
4. `CryptoBridge.decrypt` 被调用，传入 `password_cipher`。
5. **分支 A (成功)**: KeyStore 密钥存在且匹配 → 解密成功 → `s.password` 填入原文。
6. **分支 B (失败)**: KeyStore 密钥缺失或不匹配 (如包名变更) → `decrypt` 返回 Error → `unwrap_or_default()` 执行 → `s.password` 变为 `""`。
7. `masked_for_display` 检查 `s.password`：若为空，JSON 中为 `""`；若不为空，为 `"***"`。
8. 前端收到配置，发现密码为空，提示用户重新输入。

### 正常保存流程 (Save)

1. `save_config` command 被调用，携带新配置。
2. `resolve_password_field` 确定最终密码：如果是新输入或非掩码值，则保留；否则沿用旧值。
3. `save_file` 被调用。
4. **分支 A (有密码)**: `CryptoBridge.encrypt` 被调用 → KeyStore 生成/使用密钥加密 → 得到 `password_cipher`。
5. **分支 B (无密码)**: `password_cipher` 保持空字符串。
6. 构造 `EncodedSettings`，序列化 JSON。
7. 原子写入：写入 `.tmp` 文件 → `rename` 覆盖原文件。

### 包名变更后的灾难场景

1. 开发者修改 `build.gradle.kts` 中的 `namespace` 和 `applicationId` (例如从 `com.old.pkg` 改为 `com.new.pkg`)。
2. 重新构建 APK 并安装。
3. 用户打开 App，触发 `get_config`。
4. `CryptoBridge` 尝试访问 KeyStore。Android 系统查找 `com.new.pkg` 对应的密钥别名。
5. 旧密钥存储在 `com.old.pkg` 名下，**不可见**。
6. `decrypt` 调用失败 (通常抛出 `UnrecoverableKeyException` 或 `BadPaddingException`)。
7. `config_state.rs:248-252` 的 `unwrap_or_default()` 捕获错误，返回空字符串。
8. 用户的所有存量密码丢失，必须重新输入。

## Connections

[[android-generated-project-discipline]] (强调 identifier 变更的破坏性)
[[mask-placeholder-persisted-as-plaintext]] (讨论掩码语义，与此处的加密落盘形成对比)