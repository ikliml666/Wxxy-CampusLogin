---
title: 安全体系（凭据出站、加密与验证门）
type: concept
source_files:
  - tauri-app/src-tauri/src/config/model.rs
  - tauri-app/src-tauri/src/config/persist.rs
  - tauri-app/src-tauri/src/commands/config_cmd.rs
  - tauri-app/src-tauri/src/commands/system.rs
  - tauri-app/src-tauri/src/commands/self_service.rs
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/src-tauri/src/platform/identity.rs
  - tauri-app/src-tauri/src/auth/portal.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/infra/events.rs
  - tauri-app/src-tauri/src/infra/logger.rs
  - android/src-tauri/src/config_state.rs
  - android/src-tauri/src/identity_gate.rs
  - android/src-tauri/src/self_service_cmds.rs
  - android/src-tauri/src/account_cmds.rs
  - android/src-tauri/src/login_history.rs
  - android/plugins/keystore/src/lib.rs
  - android/plugins/keystore/android/src/main/java/com/campuslogin/plugin/keystore/KeystorePlugin.kt
tags: [概念, 安全, 掩码, dpapi, keystore, 验证门, 凭据]
---

## Overview

安全体系围绕三条线：**凭据出站的唯一出口**（所有把配置发往前端的路径必须经掩码方法）、**落盘加密**（桌面 DPAPI / 安卓 AndroidKeyStore AES-256-GCM）、**敏感操作验证门**（后端 TTL 时间戳，webview 无法伪造）。掩码语义统一为 `PASSWORD_MASK = "***"`：空串表示"未设置"原样保留，非空一律替换；`clear` 标志用于显式清除。违反任一条的后果是明文密码经 IPC、日志或事件流泄漏。

## 机制说明

### 敏感信息出站唯一出口

**桌面**：`Config::masked_for_display()` / `mask_in_place()`（`tauri-app/src-tauri/src/config/model.rs:217-230`），掩掉 `password` 与 `self_password` 两个字段，空值保留。

```rust
pub fn mask_in_place(&mut self) {
    if !self.password.is_empty() { self.password = PASSWORD_MASK.to_string(); }
    if !self.self_password.is_empty() { self.self_password = PASSWORD_MASK.to_string(); }
}
```

`model.rs:214-215` 的注释把它定义为唯一出口约定："不得手工逐字段打码——漏一个字段就是一次明文泄露，account 三命令即前车之鉴。"

全部调用点（桌面）：

| 出口 | 位置 |
|---|---|
| `get_config` | `commands/config_cmd.rs:88` |
| `get_init_data` | `commands/system.rs:117`（注释说明漏掩会同时导致前端 `selfPasswordSaved` 永假） |
| `switch_account` 返回 | `commands/account.rs:47` |
| `save_current_as_account` 返回 | `commands/account.rs:188` |
| `delete_account` 返回 | `commands/account.rs:227` |
| `config-changed` 事件载荷 | `commands/config_cmd.rs:15-16` |

**安卓**：`config_state::masked_for_display(&Settings) -> serde_json::Value`（`android/src-tauri/src/config_state.rs:273-282`），语义与桌面同构。调用点：`get_config`（`config_state.rs:308`）、`account_cmds::persist_current`（`account_cmds.rs:111`）、`delete_account`（`account_cmds.rs:190`）。

**锁死它的回归测试**：

- 桌面 `commands/config_cmd.rs:148-164` `masked_for_display_masks_both_password_fields`：同时断言两个字段被掩、空值保留为空串、原 struct 不被就地修改。测试注释（`:143-147`）记录历史缺陷 `fe000de`：修 `get_init_data`/`get_config` 时漏掉 account 三命令，明文 `selfPassword` 随 IPC 出站。
- 安卓 `config_state.rs:548-558` `掩码出口_非空变星号_空保持空`：断言 `v["password"] == "***"`、`v["selfPassword"] == "***"`、整体 JSON 不含明文。

### 密码掩码语义：MASK 常量 / 空串回退 / clear 标志

常量：桌面 `model.rs:3` `pub const PASSWORD_MASK: &str = "***"`；安卓 `config_state.rs:10` 同值。

**桌面 `save_config`**（`commands/config_cmd.rs:92-139`）：

| 输入 | 行为 |
|---|---|
| `clear_password == Some(true)` | 跳过兜底直接置空（`:109-110`） |
| `password` 为空串或 `"***"` | 回退 `state.config.load().password`，避免前端未传密码时旧密码被覆盖（`:111-115`） |
| 其他 | 采用新值 |
| `clear_self_password` / `self_password` | 同规则（`:118-123`） |

**安卓 `save_config`**：同语义收在纯函数里。

```rust
// config_state.rs:285-294
pub fn resolve_password_field(incoming: &str, current: &str, clear: bool) -> String {
    if clear { return String::new(); }
    if incoming.is_empty() || incoming == PASSWORD_MASK { current.to_string() } else { incoming.to_string() }
}
```

调用点 `config_state.rs:325-331`；回归测试 `config_state.rs:560-572` `密码字段回退语义_与桌面同构` 覆盖 5 种组合。

**落盘加密不排除 MASK**：桌面 `config/persist.rs:153-168` 只判断"非空即加密"（`persist.rs:155-158` 注释明确不得排除 `"***"`，否则真实密码恰为 `***` 的用户会明文落盘）；回归测试 `persist.rs:181-197` `literal_star_password_is_encrypted_on_disk` 断言磁盘内容不含 `"password":"***"`，与 `persist.rs:199-210` `empty_password_stays_empty`。

### 落盘加密：DPAPI 与 Android Keystore

**桌面 DPAPI**（`tauri-app/src-tauri/src/account/crypto.rs`）：

- 直接 `#[link(name = "crypt32")]` 声明 `CryptProtectData` / `CryptUnprotectData`（`:13-31`），flags 传 `0`（当前用户作用域，不带 `CRYPTPROTECT_LOCAL_MACHINE`）。
- 通用 helper `call_dpapi`（`:41-68`）统一 DataBlob 构造、返回码检查、`LocalFree` 释放。
- 对外 `encrypt(&str) -> base64`（`:110-117`）、`decrypt(base64) -> String`（`:120-125`）。
- 非 Windows 平台返回 `Err("加密存储仅桌面端支持")`（`:128-136`）。

**安卓 Keystore**（`android/plugins/keystore/`）：

| 项 | 值 | 位置 |
|---|---|---|
| key alias | `campus_login_master` | `KeystorePlugin.kt:34` |
| 算法 | `AES/GCM/NoPadding` | `KeystorePlugin.kt:60/79` |
| 密钥长度 | 256 位 | `KeystorePlugin.kt:50` |
| IV 长度 | 12 字节（`IV_LEN`） | `KeystorePlugin.kt:35` |
| 密文格式 | `Base64(iv + ciphertext)` | `KeystorePlugin.kt:65` |
| 认证标签 | GCM 128 位（`GCMParameterSpec(128, raw, 0, IV_LEN)`） | `KeystorePlugin.kt:80` |

Rust 侧桥 `CryptoBridge { encrypt, decrypt }`（`config_state.rs:126-163`）：`#[cfg(mobile)]` 取插件句柄，`#[cfg(not(mobile))]` 返回 Err 便于 host 测试注入 `fake_bridge()`（`config_state.rs:367-376`，base64 假加密）。

**磁盘形态**：安卓 `EncodedSettings { settings, password_cipher, self_password_cipher }`（`config_state.rs:166-172`），`save_file`（`:192-223`）把密码字段从 `settings` 里清空后单独取密文写两个 `*_cipher` 位；加密失败直接 `return Err`（`:204/210`），**绝不落明文**。桌面则是就地把两个字段替换为密文后整体 `atomic_write`（`persist.rs:159-167`）。

**失败降级语义**（都不清空整份配置）：

- 桌面：解密失败仅清空该密码字段（`commands/config_cmd.rs:34-38`、`:45-49`）。
- 安卓：`load_file` 解密失败置空（`config_state.rs:182-188`），回归测试 `解密失败时置空_配置仍可加载`（`config_state.rs:442-456`）。

### 验证门分级（bind_operator / self_offline_session / 明文查看）

**桌面**（`commands/self_service.rs`）：

```rust
// :15-23
fn ensure_identity_gate(state: &AppState) -> Option<String> {
    if !state.config.load().self_hello_enabled { return None; }
    if crate::platform::identity::identity_verified_recently() { return None; }
    Some("Windows 身份验证已过期，请重新验证后再操作".to_string())
}
```

分级结果：

| 命令 | 是否设门 | 位置 |
|---|---|---|
| `bind_operator` | 是（受 `self_hello_enabled` 开关） | `self_service.rs:82-84` |
| `self_offline_session` | 是（受开关） | `self_service.rs:231-233` |
| `reveal_operator_credential` | **强制**，不受开关影响 | `self_service.rs:300-304` |
| `query_bind_status` / `query_self_dashboard` / `query_self_online_log` | 有意不设门 | 注释 `self_service.rs:11-14`：总览卡自动刷新依赖免验证拉取，数据本存本机 |

**安卓**（`self_service_cmds.rs`）：`ensure_identity_gate`（`:38-47`）结构同构；`bind_operator`（`:87-89`）与 `self_offline_session`（`:227-229`）设门；`reveal_operator_credential`（`:257-259`）强制验证，注释明确"无论总开关如何都强制验证门（桌面同构,防 webview 侧绕过）"。

**TTL 值（两端都是 600 秒）**：

| 端 | 常量 | 判定函数 | 边界语义 |
|---|---|---|---|
| 桌面 | `platform/identity.rs:31` `IDENTITY_VERIFY_TTL_SECS = 600` | `is_within_ttl(verified_at, now, ttl)`（`:45-47`） | `0` 哨兵（从未验证）拒绝；`now < verified_at`（时钟回拨）拒绝；`now - verified_at <= 600` 通过 |
| 安卓 | `identity_gate.rs:8` `IDENTITY_VERIFY_TTL_SECS = 600` | `identity_verified_recently()`（`:23-33`） | 同上三条（`:24-32`） |

时间戳写入：桌面在 `verify_windows_identity` 成功后由后端自己写（`commands/self_service.rs:274` → `identity.rs:34-36`），命令内部亲自调 WinRT `UserConsentVerifier`（`identity.rs:78-117`）；安卓 `verify_biometric_identity`（`self_service_cmds.rs:62-66`）**只记时间戳**，实际 BiometricPrompt 由前端 `@tauri-apps/plugin-biometric` 调用（`android/frontend/src/hooks/tauriApi.ts:180-211`），后端信任前端调用成功。

### 日志与事件载荷禁止携带密码

- **HTTP 错误串脱敏**：`auth/portal.rs:69-80` `redact_credentials(msg, url, base_url, password)` 做三次替换——完整 URL → `{base_url}?***`、URL 编码后的密码 → `***`、明文密码 → `***`。这是唯一出口，唯一调用点 `auth/protocol.rs:127-131`（登录请求失败路径）。
- **登录日志只打脱敏 URL**：`protocol.rs:119-120` 打 `user` / `operator` / `adapterIp`，`:145-146` 打 `safe_url`（`protocol.rs:111` 构造为 `{base_url}?***`）；请求 URL 本身从不进日志。
- **命令返回值不含凭据**：`query_bind_status` 只回 `masked_account` 与 `password_set` 布尔（`commands/self_service.rs:132-140`，安卓 `self_service_cmds.rs:130-138`）；`reveal_operator_credential` 是唯一返回明文的命令，且必须过强制验证门。
- **凭据只过内存**：`bind_operator` / `self_offline_session` / `reveal_operator_credential` 的文档注释三处明写"凭据仅本次请求内存传递，不写入配置、不落盘、不写日志"（`commands/self_service.rs:64/218/284`）。
- **登录历史不带密码**：安卓回归测试 `记录字段_序列化形状对齐桌面` 里 `assert!(!raw.contains("password"), "历史不得含密码字段")`（`android/src-tauri/src/login_history.rs:139`）。
- **加密过程日志不含明文/密文**：安卓只打长度（`config_state.rs:200-211`，`eprintln!("[save_config] password encrypt ok len={}", c.len())`）。
- **`EventBus` 是事件唯一封装**：`infra/events.rs:6` 与 `commands/config_cmd.rs:13-14` 规定配置事件必须掩码后发。

## 关键约束

- **所有 Config 出站必须走 `masked_for_display()`**，不得手工逐字段打码；新增敏感字段必须同时改 `mask_in_place`（`model.rs:223-230`）与安卓 `masked_for_display`（`config_state.rs:273-282`），否则回归测试不会覆盖到它。
- **空值语义不可改**：空串 = "未设置"，必须原样透传（前端据此显示"未保存"占位）；把空串也替换成 `"***"` 会让前端误判"已保存"。
- **`clear` 标志优先级高于掩码回退**：`clear_password == Some(true)` 必须先于 MASK 判断（`config_cmd.rs:109`、`config_state.rs:286`）。
- **落盘"非空即加密"，不得排除 `"***"`**（`persist.rs:155-158`）。
- **`reveal_operator_credential` 的门不受 `self_hello_enabled` 影响**（`self_service.rs:300`、`self_service_cmds.rs:257`），注释说明"防止一键关闭保护后明文裸奔"。
- **TTL 判定必须拒绝时钟回拨**（`identity.rs:46` 的 `now >= verified_at`、`identity_gate.rs:29-31`），否则回拨可无限延长验证有效期。
- **新增 IPC 出口必须自问掩码**：`get_init_data` / `get_config` / account 三命令 / `config-changed` 是现有六个出口，任何新的"把配置发给前端"的路径都属于同一类风险。
- **Windows Hello 不回退凭据对话框**：`platform/identity.rs:3-5` 明确设备未配置 Hello 时直接返回错误引导，不回退 CredUI 输密码（2026-09-05 用户要求）。

## Data Flow

```text
前端输入明文密码
  → save_config(config, clearPassword, clearSelfPassword)     IPC
    ├─ clear 标志 → 置空
    ├─ 空 / "***"  → 回退 state.config 内存明文
    └─ 新值 → 采用
      → update_portal_url / set_log_retention_days 等运行期同步（config_cmd.rs:127-130）
      → 加密落盘
         桌面: persist::save_config_to_disk_encrypted → crypto::encrypt(DPAPI) → atomic_write
         安卓: config_state::save_file → CryptoBridge → KeystorePlugin AES-GCM → tmp+rename
      → 内存态更新（state.config.store / AndroidState.config 缓存）
      → 掩码后广播 config-changed（桌面 config_cmd.rs:15-16；安卓当前不广播）

前端读配置
  → get_config / get_init_data / account 三命令 / switch_account
    → masked_for_display()  ← 唯一出口
      → password / selfPassword 均为 "***" 或 ""

敏感操作
  → verify_windows_identity | verify_biometric_identity
    → 后端记 LAST_VERIFY_EPOCH_SECS（TTL 600s，时钟回拨=过期）
  → bind_operator / self_offline_session / reveal_operator_credential
    → ensure_identity_gate（前两者受 self_hello_enabled 开关；reveal 强制）
      → 未过门：返回 CommandResult::err，不发起网络请求
```

## Connections

- [[desktop-config]] — `Config` 字段定义与 `masked_for_display` 所在模块
- [[desktop-account-selfservice]] — 六条自助服务命令与 DPAPI 加密的调用方
- [[android-backend]] — `config_state.rs` 的 Keystore 桥与验证门实现
- [[android-plugins]] — Keystore 插件 Kotlin/Rust 双侧细节
- [[ipc-command-surface]] — 命令返回值与事件载荷的传输通道
- [[config-and-persistence]] — 掩码出口与落盘加密在持久化链路中的位置
- [[dual-platform-sharing]] — DPAPI 与 Keystore 的对应关系、`self_hello_enabled` 双端同步

## Known Issues

- **`self_reverify_each_action` 无后端消费方**：字段定义于 `config/model.rs:23-24` 与 `android/src-tauri/src/config_state.rs:21`，但后端从未读取；只有前端 `tauri-app/frontend/src/account/selfServiceState.ts:102` / `android/frontend/src/account/selfServiceState.ts:130` 使用。也就是说"每次操作都二次验证"纯属前端编排，绕过 webview 直调 `invoke` 即失效。
- **安卓验证门的时间戳信任前端**：`verify_biometric_identity`（`android/src-tauri/src/self_service_cmds.rs:62-66`）无条件写时间戳，不校验生物识别是否真的通过。webview 内直接 `invoke('verify_biometric_identity')` 即可拿到 600s 窗口——桌面端在同一位置是命令内亲自调 WinRT（`platform/identity.rs:78-117`），强度不对等。
- **`redact_credentials` 只覆盖登录请求一条路径**：`auth/portal.rs:69` 是唯一定义、`auth/protocol.rs:129` 是唯一调用点。Portal 探测把 reqwest 的 `Display`（可能含完整请求 URL）原样写进日志与 `login-log` 事件（`monitor/portal_check.rs:56/74`），自助服务把同类错误串原样塞进 `CommandResult.message` 返回前端（`self_service/mod.rs:179/186/308` 等十余处 `format!("...: {e}")`），均无脱敏。
- **密码长度异常只在前后端校验**：`validate_password`（`config/validate.rs:31-39`）只查非空与 ≤128，无复杂度要求；安卓 `config_state.rs` 完全不做密码校验。
- **安卓 `query_bind_status` 无验证门但会带密码发请求**：`self_service_cmds.rs:114-143` 未设门，而密码通过 `resolve_self_password` 逐个尝试回退（`:125-127`），与桌面同设计（桌面注释 `self_service.rs:11-14` 明确有意为之），但当用户已保存明文自助密码时，任意 webview 脚本可高频触发对自助服务系统的认证。
- **`Config::mask_in_place` 是 `pub`**：`model.rs:223` 对外公开，任何调用方都可以就地破坏内存中的明文配置对象；类型系统不阻止误用（`masked_for_display` 走 clone 路径是安全的）。
- **日志无统一脱敏钩子**：`infra/logger.rs:304` 的 `log(level, module, message)` 与 `:348/355/362/369` 的四个宏对内容零过滤，只有调用方纪律；新增日志语句若直接打 `config.password` 不会有任何拦截。
