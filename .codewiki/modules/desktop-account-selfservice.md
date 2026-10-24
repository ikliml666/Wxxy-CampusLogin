---
title: 桌面端账号与自助服务（DPAPI 加密 / Dr.COM Self 协议）
type: module
source_files:
  - tauri-app/src-tauri/src/account/mod.rs
  - tauri-app/src-tauri/src/account/crypto.rs
  - tauri-app/src-tauri/src/self_service/mod.rs
tags: [账号, DPAPI, 加密, 自助服务, Dr.COM Self, 运营商绑定, 跨平台]
---

## Overview

本模块由两块组成：`account::crypto` 是 Windows DPAPI（`CryptProtectData`/`CryptUnprotectData`）的直接 FFI 封装，提供用户作用域与机器作用域的“明文 ↔ base64 密文”字符串级加解密，供密码落盘；`self_service` 是 Dr.COM 自助服务系统（`/Self`）的完整 HTTP 协议实现，包含登录会话建立（带传输层重试）、运营商账号绑定、绑定状态查询、明文凭据查看、修改密码、dashboard 在线信息/上网记录查询、账单页上网记录查询、在线会话注销。两者在 `account/mod.rs:1` 声明 `pub mod crypto`，`self_service/mod.rs:1-53` 逆向记录 11 步协议。非 Windows 平台 `account::crypto` 提供返回 `Err` 的桩；`self_service` 逻辑与平台无关但依赖内网地址。

## Key Components

### account/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `account/mod.rs:1` | pub | `mod crypto` | 唯一的子模块，DPAPI 加解密 |

### account/crypto.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `crypto.rs:1-219` | 私有 mod（`#[cfg(target_os = "windows")]`） | `dpapi` | DPAPI FFI 封装，非 Windows 不编译 |
| `crypto.rs:5-9` | 私有 struct | `DataBlob { cb_data: u32, pb_data: *mut u8 }` | 与 Win32 `DATA_BLOB` 二进制布局一致的 `#[repr(C)]` 结构 |
| `crypto.rs:11-32` | 私有 extern（`#[link(name = "crypt32")]`） | `CryptProtectData`（`crypto.rs:13-21`）、`CryptUnprotectData`（`crypto.rs:23-31`） | DPAPI 加/解密，`extern "system"` |
| `crypto.rs:34-37` | 私有 extern（`#[link(name = "kernel32")]`） | `LocalFree(h_mem) -> *mut c_void` | 释放 DPAPI 返回的输出缓冲 |
| `crypto.rs:39-42` | 私有 const | `DPAPI_ENTROPY: &[u8] = b"Wxxy-CampusLogin::dpapi-v1"` | 审计 A10：DPAPI additional entropy，叠加用户+机器作用域 |
| `crypto.rs:44-78` | dpapi 内私有 fn | `call_dpapi<F>(input_data: &[u8], dpapi_call: F, error_msg: &str, empty_error_msg: &str) -> Result<Vec<u8>, String>` | 统一 DataBlob 构造、返回值检查、输出读取与 `LocalFree` |
| `crypto.rs:80-86` | dpapi 内私有 fn | `entropy_blob() -> DataBlob` | 用 `DPAPI_ENTROPY` 构造 entropy DataBlob |
| `crypto.rs:88-106` | dpapi 内 pub fn | `encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String>` | 调 `CryptProtectData` 带 entropy，失败文案 `"DPAPI加密失败"` / `"DPAPI加密返回空数据"` |
| `crypto.rs:108-147` | dpapi 内 pub fn | `decrypt(data: &[u8]) -> Result<Vec<u8>, String>` | 先试带 entropy 解密，失败回退无 entropy 兼容旧密文 |
| `crypto.rs:149-152` | 私有 const | `CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x0400` | 机器作用域标志 |
| `crypto.rs:154-174` | dpapi 内 pub fn | `encrypt_machine(plaintext: &[u8]) -> Result<Vec<u8>, String>` | 机器作用域加密（带 entropy） |
| `crypto.rs:176-197` | dpapi 内 pub fn（`#[allow(dead_code)]`） | `decrypt_machine(data: &[u8]) -> Result<Vec<u8>, String>` | 机器作用域解密，供无头核心 bin |
| `crypto.rs:199-218` | dpapi 内 pub fn（`#[cfg(test)]`） | `encrypt_legacy_no_entropy(plaintext: &[u8]) -> Result<Vec<u8>, String>` | 测试用：无 entropy 旧格式加密 |
| `crypto.rs:221-229` | pub fn（`#[cfg(target_os = "windows")]`） | `encrypt(plaintext: &str) -> Result<String, String>` | 明文 → DPAPI → 标准 base64 |
| `crypto.rs:231-237` | pub fn（`#[cfg(target_os = "windows")]`） | `decrypt(encrypted_base64: &str) -> Result<String, String>` | base64 → DPAPI 解密 → UTF-8；错误文案 `"Base64解码失败: {e}"` / `"DPAPI解密失败，可能需要重新输入密码"` / `"DPAPI解密返回空数据"` / `"UTF8转换失败: {e}"` |
| `crypto.rs:239-243` | pub fn（`#[cfg(not(target_os = "windows"))]`） | `encrypt(_plaintext: &str) -> Result<String, String>` | 桩，恒返回 `Err("加密存储仅桌面端支持")` |
| `crypto.rs:245-255` | pub fn（`#[cfg(target_os = "windows")]`） | `encrypt_machine(plaintext: &str) -> Result<String, String>` | 字符串级机器作用域加密 |
| `crypto.rs:257-264` | pub fn（`#[cfg(target_os = "windows")]`, `#[allow(dead_code)]`） | `decrypt_machine(encrypted_base64: &str) -> Result<String, String>` | 字符串级机器作用域解密 |
| `crypto.rs:266-269` | pub fn（`#[cfg(not(target_os = "windows"))]`） | `encrypt_machine(_plaintext: &str) -> Result<String, String>` | 桩 |
| `crypto.rs:271-274` | pub fn（`#[cfg(not(target_os = "windows"))]`） | `decrypt_machine(_encrypted_base64: &str) -> Result<String, String>` | 桩 |
| `crypto.rs:276-279` | pub fn（`#[cfg(not(target_os = "windows"))]`） | `decrypt(_encrypted_base64: &str) -> Result<String, String>` | 桩 |

### self_service/mod.rs

模块级文档（`self_service/mod.rs:1-53`）逆向记录了 11 步协议；下表为代码项清单。

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `self_service/mod.rs:61-62` | pub const | `SELF_BASE_URL = "http://10.1.80.200:8080/Self"` | 无锡学院部署的自助服务基地址（仅校园网内网可达） |
| `self_service/mod.rs:64-74` | 私有 static | `RE_CHECKCODE` (`65-66`)、`RE_CSRFTOKEN` (`67-68`)、`RE_FLD_VALUE` (`70-71`)、`RE_SWAL_MSG` (`73`) | 各 HTML/JS 片段提取正则 |
| `self_service/mod.rs:76-88` | pub struct | `BindParams<'a>` | 绑定请求参数（见“结构体与字段”） |
| `self_service/mod.rs:90-98` | pub fn | `operator_fld_pair(operator: &str) -> Option<(usize, usize)>` | 运营商后缀 → `FLDEXTRA` 账号/密码字段序号：`@cmcc`→(1,2)、`@telecom`→(3,4)、`@unicom`→(5,6)，其余 `None` |
| `self_service/mod.rs:100-103` | pub fn | `extract_checkcode(html: &str) -> Option<String>` | 登录页 `checkcode` 提取 |
| `self_service/mod.rs:105-108` | pub fn | `extract_csrftoken(html: &str) -> Option<String>` | 绑定页/改密页 `csrftoken` 提取 |
| `self_service/mod.rs:110-123` | pub fn | `extract_fld_values(html: &str) -> [String; 6]` | FLDEXTRA1..6 预填值（未绑定为空串；越界序号 `continue` 跳过） |
| `self_service/mod.rs:125-128` | pub fn | `extract_swal_msg(html: &str) -> Option<String>` | swal 提示文本提取（登录失败原因 / 绑定结果） |
| `self_service/mod.rs:130-133` | pub fn | `is_bind_success(msg: &str) -> bool` | 判定 `msg` 是否含 `"绑定运营商账号信息成功"` |
| `self_service/mod.rs:135-141` | pub fn | `md5_hex(input: &str) -> String` | 标准小写 MD5 十六进制（登录密码提交格式） |
| `self_service/mod.rs:143-157` | 私有 fn | `build_session_client(local_addr: Option<IpAddr>) -> Result<reqwest::Client, String>` | 构造一次性会话客户端：15s 超时 / 3s 连接超时 / `no_proxy` / 禁用自动重定向 / `cookie_store(true)` / 伪装 UA / 可选 `local_address` |
| `self_service/mod.rs:159-208` | pub async fn | `bind_operator(params: &BindParams<'_>, local_addr: Option<IpAddr>) -> Result<String, String>` | 绑定运营商账号全流程，成功返回服务端提示文本 |
| `self_service/mod.rs:210-216` | pub struct | `OperatorBinding` | 绑定状态展示（见“结构体与字段”） |
| `self_service/mod.rs:218-232` | pub async fn | `query_bind_status(account, password, local_addr) -> Result<[Option<OperatorBinding>; 3], String>` | 查询三家运营商绑定状态；数组顺序为 `[移动, 电信, 联通]` |
| `self_service/mod.rs:234-252` | pub async fn | `reveal_credential(account, password, operator, local_addr) -> Result<(String, String), String>` | 返回该运营商（手机号, 运营商账户密码）明文；双空返回 `Err("该运营商尚未绑定")`；调用方须先做本地身份验证 |
| `self_service/mod.rs:254-271` | 私有 fn | `parse_change_password_response(text: &str) -> Result<String, String>` | 解析 changePasswordMethod 的 JSON 响应 |
| `self_service/mod.rs:273-319` | pub async fn | `change_password(account, password, new_password, local_addr) -> Result<String, String>` | 修改自助服务系统密码（= 校园网登录密码），成功后强制下线 |
| `self_service/mod.rs:321-338` | pub fn | `mask_account(account: &str) -> String` | 账号掩码：长度 ≥8 → 前 3 后 2；>4 → 前 1 后 1；否则整体 `*` 重复 |
| `self_service/mod.rs:340-348` | 私有 fn | `binding_from(acct: &str, pwd: &str) -> Option<OperatorBinding>` | 账号为空 → `None`；否则掩码账号 + `password_set` |
| `self_service/mod.rs:350-407` | 私有 async fn | `login_session(account, password, local_addr) -> Result<reqwest::Client, String>` | 建立登录会话 3 步：取 checkcode → 预热 randomCode → POST verify（密码 MD5） |
| `self_service/mod.rs:409-422` | 私有 fn | `is_retryable_transport_error(err: &str) -> bool` | 传输层错误标记判定（仅网络/读取类重试） |
| `self_service/mod.rs:424-451` | 私有 async fn | `login_session_with_retry(account, password, local_addr) -> Result<reqwest::Client, String>` | `login_session` 整体重试包装：最多 3 次，间隔 1s，仅传输层症状重试 |
| `self_service/mod.rs:453-475` | 私有 async fn | `login_and_fetch_bind_page(account, password, local_addr) -> Result<(reqwest::Client, String), String>` | 在 `login_session_with_retry` 之上取绑定表单页 HTML（含第 4 步） |
| `self_service/mod.rs:477-496` | 私有 async fn | `fetch_dashboard_json(client, path, label) -> Result<serde_json::Value, String>` | 请求 dashboard 下 JSON 接口（302 判会话失效） |
| `self_service/mod.rs:498-509` | pub async fn | `query_dashboard(account, password, local_addr) -> Result<(Value, Value), String>` | 一次会话连拉 `getOnlineList` 与 `getLoginHistory`，原始 JSON 透传 |
| `self_service/mod.rs:511-541` | pub async fn | `query_online_log(account, password, start_time, end_time, local_addr) -> Result<serde_json::Value, String>` | 拉 `/Self/bill/getUserOnlineLog`（固定 `pageNumber=1&pageSize=500`），返回 `{rows, summary, total}` |
| `self_service/mod.rs:543-549` | 私有 fn | `parse_offline_success(text: &str) -> bool` | 解析 `{"success":bool}`；非 JSON/缺字段/非布尔 → `false` |
| `self_service/mod.rs:551-576` | pub async fn | `offline_session(account, password, session_id, local_addr) -> Result<(), String>` | `/Self/dashboard/tooffline?sessionid=` 踢会话下线 |

## 结构体与字段

### `BindParams<'a>`（`self_service/mod.rs:76-88`）

| 位置 | 字段 | 类型 | 含义 |
| --- | --- | --- | --- |
| `self_service/mod.rs:78` | `account` | `&'a str` | 学号（自助服务系统登录账号，与 Portal 登录账号相同） |
| `self_service/mod.rs:80` | `password` | `&'a str` | 自助服务系统登录密码（明文，协议内 MD5 后提交；默认身份证后 6 位） |
| `self_service/mod.rs:82` | `operator` | `&'a str` | 运营商后缀（与 `Config.operator` 同源：`@cmcc`/`@telecom`/`@unicom`） |
| `self_service/mod.rs:84` | `phone` | `&'a str` | 运营商账号（用户办理套餐的手机号） |
| `self_service/mod.rs:86` | `sms_password` | `&'a str` | 运营商账户密码（办理套餐时运营商短信下发） |

凭据仅内存传递，调用方不得落盘/写日志（`self_service/mod.rs:76` 前注释）。

### `OperatorBinding`（`self_service/mod.rs:210-216`）

| 位置 | 字段 | 类型 | 含义 |
| --- | --- | --- | --- |
| `self_service/mod.rs:212` | `masked_account` | `String` | 掩码后的运营商账号（`mask_account` 产物） |
| `self_service/mod.rs:214` | `password_set` | `bool` | 运营商账户密码是否已设置（从不返回明文） |

### `crypto::dpapi::DataBlob`（`crypto.rs:5-9`，私有）

| 位置 | 字段 | 类型 | 含义 |
| --- | --- | --- | --- |
| `crypto.rs:7` | `cb_data` | `u32` | 缓冲区字节数 |
| `crypto.rs:8` | `pb_data` | `*mut u8` | 缓冲区指针（输出由 `LocalFree` 释放） |

### 常量与正则汇总

| 项 | 位置 | 值/语义 |
| --- | --- | --- |
| `SELF_BASE_URL` | `self_service/mod.rs:61-62` | `http://10.1.80.200:8080/Self`，硬编码不可配置 |
| `RE_CHECKCODE` | `self_service/mod.rs:65-66` | `name="checkcode"[^>]*value="(\d+)"` |
| `RE_CSRFTOKEN` | `self_service/mod.rs:67-68` | `name="csrftoken"[^>]*value="([0-9a-fA-F-]{36})"` |
| `RE_FLD_VALUE` | `self_service/mod.rs:70-71` | `value="([^"]*)"\s+name="FLDEXTRA(\d)"`（`value` 在 `name` 之前，可跨行） |
| `RE_SWAL_MSG` | `self_service/mod.rs:73` | `\}\)\('((?:[^'\\]|\\.)*)'\);` |
| `DPAPI_ENTROPY` | `crypto.rs:39-42` | `b"Wxxy-CampusLogin::dpapi-v1"` |
| `CRYPTPROTECT_LOCAL_MACHINE` | `crypto.rs:149-152` | `0x0400` |
| 固定表单参数 | `self_service/mod.rs:378-387` | `account`、`password=md5_hex(pwd)`、`checkcode`、`code=""` |
| 固定查询参数 | `self_service/mod.rs:522-529` | `pageNumber=1`、`pageSize=500` |
| `Referer` 头 (绑定) | `self_service/mod.rs:189` | `{SELF_BASE_URL}/service/operatorId` |
| `Referer` 头 (改密) | `self_service/mod.rs:302` | `{SELF_BASE_URL}/setting/changePassword` |

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Tauri Commands                         │
│  (commands/self_service.rs / commands/config_cmd.rs)        │
└──────────┬───────────────────────────────┬──────────────────┘
           │                               │
           ▼                               ▼
┌─────────────────────┐       ┌───────────────────────────────┐
│   self_service      │       │      account::crypto          │
│   (self_service/    │       │      (account/crypto.rs)      │
│    mod.rs)          │       │                               │
│                     │       │ Windows: DPAPI (crypt32.dll)  │
│ • bind_operator     │       │ Non-Windows: Err("仅桌面端")  │
│ • query_bind_status │       └───────────────┬───────────────┘
│ • reveal_credential │                       │ (used by persist)
│ • change_password   │                       │
│ • query_dashboard   │                       ▼
│ • query_online_log  │         ┌───────────────────────────────┐
│ • offline_session   │         │   config::persist             │
└──────────┬──────────┘         │   (persist.rs)                │
           │                    │   save/load accounts          │
           ▼                    └───────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│              Dr.COM Self Server (/Self)                     │
│  http://10.1.80.200:8080/Self                               │
│  /Self/login/ → /Self/login/randomCode → /Self/login/verify │
│  /Self/service/operatorId → /Self/service/bind-operator     │
│  /Self/dashboard/{getOnlineList,getLoginHistory,tooffline}  │
│  /Self/bill/getUserOnlineLog                                │
│  /Self/setting/changePasswordMethod                         │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 账号密码落盘加密（DPAPI）

```
外部调用 account::crypto::encrypt / decrypt (crypto.rs:221,231 windows; 非 windows 桩 crypto.rs:239,276)
  → dpapi::encrypt / decrypt (crypto.rs:88,108)
      → call_dpapi (crypto.rs:44) → CryptProtectData/CryptUnprotectData
         - 用户作用域 flags=0 (crypto.rs:93-101, 115-123)
         - 机器作用域 flags=CRYPTPROTECT_LOCAL_MACHINE (crypto.rs:161-169, 183-191)
         - 带 DPAPI_ENTROPY (crypto.rs:42,92,114,160,183)
         - decrypt 回退无 entropy 旧密文 (crypto.rs:131-147)
      → base64 编解码 (crypto.rs:225,233)
```

### 绑定运营商账号

```
self_service::bind_operator (self_service/mod.rs:159)
  ├─ operator_fld_pair (mod.rs:90)
  ├─ login_and_fetch_bind_page (mod.rs:453) → login_session_with_retry (mod.rs:424) → login_session (mod.rs:350)
  │     1. GET /Self/login/ → extract_checkcode (mod.rs:100, 行366)
  │     2. GET /Self/login/randomCode?t=1 (mod.rs:370-376)
  │     3. POST /Self/login/verify form: account, password=md5_hex, checkcode, code="" (mod.rs:378-387)
  │       成功判据 Location 含 "/Self/dashboard" (mod.rs:389-394)
  │     4. GET /Self/service/operatorId (mod.rs:463)
  ├─ extract_csrftoken (mod.rs:105), extract_fld_values (mod.rs:110)
  ├─ 组装 form：csrftoken + FLDEXTRA1..6，目标填 phone/sms_password，其余回填 prefilled (mod.rs:175-186)
  ├─ POST /Self/service/bind-operator (mod.rs:187-207, Referer mod.rs:189)
  └─ extract_swal_msg → is_bind_success ? Ok(msg) : Err(msg)
```

### 查询与注销

```
query_bind_status    → login_and_fetch_bind_page → extract_fld_values → [cmcc, telecom, unicom] 掩码 (mod.rs:218-232)
reveal_credential    → login_and_fetch_bind_page → extract_fld_values[acct-1],[pwd-1] 明文出站 (mod.rs:234-252)
query_dashboard      → login_session_with_retry → fetch_dashboard_json(getOnlineList,getLoginHistory) (mod.rs:498-509)
query_online_log     → login_session_with_retry → GET /Self/bill/getUserOnlineLog (mod.rs:511-541)
offline_session      → login_session_with_retry → GET /Self/dashboard/tooffline?sessionid= (mod.rs:551-576)
change_password      → login_session_with_retry → GET/POST changePassword (mod.rs:273-319)
```

## Connections

- [[desktop-config]] — `config::persist` 调用 `account::crypto::{encrypt, decrypt}`；`self_hello_enabled` 与 `self_reverify_each_action` 是自助服务门禁配置面。
- [[desktop-commands]] — 账号命令经 `config::persist` 读写档案，crypto 调用收敛在 persist 层；`commands/self_service.rs` 七个自助服务命令 + `verify_windows_identity`、`commands/config_cmd.rs` 使用 `crypto::decrypt`。
- [[desktop-auth]] — `Config.operator` 取值域与 `account::crypto` 落盘登录密码同份配置；自助服务绑定是校园网登录前置条件（`self_service/mod.rs:4-5`）。
- [[desktop-infra]] — `infra::state::AppState` / `CommandResult`、`platform::identity`、`platform::console_output`。
- 安卓端（`android/src-tauri/src/`）：`self_service_cmds.rs` 全量复用 `campus_login_lib::self_service`；`account_cmds.rs` 不使用 `campus_login_lib::account::crypto`（改用 `tauri-plugin-campus-keystore`）。

## Known Issues

1. **自助服务地址硬编码**：`self_service/mod.rs:61-62` 的 `SELF_BASE_URL` 写死在代码里，不是 `Config` 字段；该模块被安卓端复用，换学校部署必须改代码并双端同步。
2. **`mask_account` 的文档注释与实现不一致**：`self_service/mod.rs:321-322` 注释称“其他格式 ≥8 位前 2 后 2”，实现（`self_service/mod.rs:331-337`）对 `n >= 8` 一律 `head(3) + "******" + tail(2)`，测试 `self_service/mod.rs:663-665` 断言 `mask_account("19720520238") == "197******38"` 证实是前 3 后 2。
3. **所有页面解析都依赖精确的 HTML/JS 形状**：`RE_SWAL_MSG`（`self_service/mod.rs:73`）要求提示形如 `})('...')` 且“整页唯一”；`RE_FLD_VALUE`（`:70-71`）要求 `value` 属性出现在 `name` 之前；`RE_CHECKCODE`（`:65-66`）要求 `value` 紧跟 `name` 且为纯数字。任一格式变动都会静默返回 `None`/空串。
4. **预填值解析失败会清空其他运营商的绑定**：绑定表单是整体保存（`self_service/mod.rs:110-123`、`:175-186`），非目标运营商字段必须回填 `prefilled[i-1]`。若解析失败导致 `prefilled` 全为空串，提交即会把已有绑定清掉——代码层无“预填值为空则跳过提交”的保护。
5. **验证码流程依赖特定部署**：`self_service/mod.rs:370-376` 固定提交 `code=""`，并强制预热 `randomCode`。注释（模块文档 `:9-11`）说明本部署验证码输入框隐藏；若部署强制校验验证码，登录会 302 回登录页并报“验证码错误！”。
6. **登录成功判据是重定向 Location 子串**：`self_service/mod.rs:389-394` 用 `loc.contains("/Self/dashboard")`；改动 dashboard 路径即失效。同理，所有“会话失效”判定都用 `is_redirection()`（`:467`、`:489`、`:533`、`:564` 等），依赖服务端固定 302 而非 401/403。
7. **注销结果不可信**：模块文档 `self_service/mod.rs:28-29` 注释记录“实测对不存在的 sessionid 也返回 true”，`parse_offline_success`（`:543-549`）只透传 `success` 字段，因此 `offline_session` 返回 `Ok(())` 不代表目标会话真的下线。
8. **每条命令都重新登录一次**：`query_dashboard`（`:505`）、`query_online_log`（`:521`）、`offline_session`（`:558`）各自调用 `login_session_with_retry`，而 `build_session_client`（`:145-157`）每次新建客户端（`cookie_store` 不跨调用保留）。一次面板刷新 = N ×（登录往返 + 业务请求），且每次都要提交一次 MD5 密码。
9. **`query_bind_status` 丢弃会话客户端**：`self_service/mod.rs:225` 绑定 `let (_client, op_html)`，函数返回后 cookie 会话即被释放。
10. **重试仅限登录会话建立，业务请求无重试**：`login_session_with_retry`（`:424-451`）对传输层错误最多重试 3 次间隔 1s（`is_retryable_transport_error` `:409-422`），但 `bind_operator` 内 POST 绑定（`:187`）、`change_password` 提交（`:300`）等无重试；超时固定 15s/3s（`:147-148`）。
11. **客户端构造与 `network::client` 不一致**：`self_service/mod.rs:145-157` 不设 `min_tls_version`、不走客户端池、不设默认 `Cache-Control` 头。当前基地址明文 http，暂无实际影响，但两套客户端策略并存。
12. **DPAPI 调用未设 `CRYPTPROTECT_UI_FORBIDDEN`**：`crypto.rs:93-101`、`crypto.rs:115-123`、`crypto.rs:161-169`、`crypto.rs:183-191` 的 `flags` 参数均为 `0` 或 `CRYPTPROTECT_LOCAL_MACHINE`（无 UI 禁止标志）。按 Win32 语义，缺少该 flag 时 `CryptUnprotectData` 在特定场景可能弹 UI，无界面后台场景或阻塞线程。
13. **DPAPI 失败信息不含系统错误码**：`crypto.rs:62-69` 只在 `result == 0` 时返回固定文案，未调 `GetLastError`，排障无法区分“凭据不匹配/内存不足/参数非法”。
14. **空明文加密必然失败**：`crypto.rs:50-54` 把入参复制为 `DataBlob{cb_data: len, pb_data: ptr}`；DPAPI 不接受 `cb_data == 0`，因此 `encrypt("")` 会返回 `Err("DPAPI加密失败")`（`:103`）。现有调用方需先判空。
15. **非 Windows 是硬失败桩**：`crypto.rs:239-243`、`crypto.rs:266-269`、`crypto.rs:271-274`、`crypto.rs:276-279` 四个函数恒返回 `Err("加密存储仅桌面端支持")`。这意味着非 Windows target 上密码落盘必然失败；安卓端因此不使用 `config::persist`，改由 Keystore 实现。
16. **`extract_fld_values` 的越界序号被静默丢弃**：`self_service/mod.rs:115-122` 对 `cap[2]` 解析失败或不在 `1..=6` 的匹配 `continue`，若页面引出 `FLDEXTRA7` 等新字段会被无声忽略（与“整体保存”语义叠加时风险同上第 4 条）。
17. **没有对 `pageSize=500` 的分页兜底**：`self_service/mod.rs:528` 硬编码 `pageSize=500`，返回 `total > 500` 时后端只透传首页数据（前端不做翻页）。
18. **日期合法性校验在命令层而非协议层**：`self_service::query_online_log`（`:511-541`）不校验 `start_time`/`end_time` 格式，把关的是命令层 `is_iso_date` 与先后比较；安卓端需自行实现同等校验。
19. **机器作用域解密函数未被主 bin 引用**：`crypto.rs:176-197` `decrypt_machine` 标记 `#[allow(dead_code)]`，仅供无头核心 bin 使用；主 bin 仅用用户作用域加解密。
20. **旧格式密文兼容回退仅用户作用域**：`crypto.rs:108-147` `decrypt` 先试 entropy 再回退无 entropy；机器作用域 `decrypt_machine`（`:179-196`）无回退，若历史机器密文以无 entropy 生成会失败（但机器密文始终带 entropy 生成，见 `:156-174`）。
