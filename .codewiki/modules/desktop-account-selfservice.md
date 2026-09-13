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

本模块由两块组成：`account::crypto` 是 Windows DPAPI（`CryptProtectData`/`CryptUnprotectData`）的直接 FFI 封装，提供"明文 ↔ base64 密文"的字符串级加解密，供 `config::persist` 与 `commands::account` 落盘账号密码；`self_service` 是 Dr.COM 自助服务系统（`/Self`）的完整 HTTP 协议实现，包含登录会话建立、运营商账号绑定、绑定状态查询、明文凭据查看、dashboard 在线信息/上网记录查询、账单页上网记录查询与在线会话注销。

两者都在 `tauri-app/src-tauri/src/lib.rs:2` 与 `lib.rs:7` 被列为**跨平台协议核心**（安卓端经 `campus-login` path 依赖可见）：`self_service` 在安卓端被 `android/src-tauri/src/self_service_cmds.rs` 逐命令复用；`account::crypto` 的 Windows 分支只在 Windows 编译，非 Windows 平台是返回 `Err` 的桩（安卓端改用 `tauri-plugin-campus-keystore`）。

## Key Components

### account/mod.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `account/mod.rs:1` | pub | `mod crypto` | 唯一的子模块，DPAPI 加解密 |

### account/crypto.rs

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `crypto.rs:1-107` | 私有 mod（`#[cfg(target_os = "windows")]`） | `dpapi` | DPAPI FFI 封装，非 Windows 不编译 |
| `crypto.rs:5-9` | 私有 struct | `DataBlob { cb_data: u32, pb_data: *mut u8 }` | 与 Win32 `DATA_BLOB` 二进制布局一致的 `#[repr(C)]` 结构 |
| `crypto.rs:11-32` | 私有 extern（`#[link(name = "crypt32")]`） | `CryptProtectData`（`crypto.rs:13-21`）、`CryptUnprotectData`（`crypto.rs:23-31`） | DPAPI 加/解密，`extern "system"` |
| `crypto.rs:34-37` | 私有 extern（`#[link(name = "kernel32")]`） | `LocalFree(h_mem) -> *mut c_void` | 释放 DPAPI 返回的输出缓冲 |
| `crypto.rs:41-68` | dpapi 内私有 fn | `call_dpapi<F>(input_data: &[u8], dpapi_call: F, error_msg: &str, empty_error_msg: &str) -> Result<Vec<u8>, String>` | 统一 DataBlob 构造、返回值检查、输出读取与 `LocalFree` |
| `crypto.rs:70-87` | dpapi 内 pub fn | `encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String>` | 调 `CryptProtectData`，失败文案 `"DPAPI加密失败"` / `"DPAPI加密返回空数据"` |
| `crypto.rs:89-106` | dpapi 内 pub fn | `decrypt(data: &[u8]) -> Result<Vec<u8>, String>` | 调 `CryptUnprotectData`，失败文案 `"DPAPI解密失败，可能需要重新输入密码"` / `"DPAPI解密返回空数据"` |
| `crypto.rs:109-117` | pub fn（`#[cfg(target_os = "windows")]`） | `encrypt(plaintext: &str) -> Result<String, String>` | 明文 → DPAPI → 标准 base64 |
| `crypto.rs:119-125` | pub fn（`#[cfg(target_os = "windows")]`） | `decrypt(encrypted_base64: &str) -> Result<String, String>` | base64 → DPAPI 解密 → UTF-8；错误文案 `"Base64解码失败: {e}"` / `"DPAPI解密失败…"` / `"UTF8转换失败: {e}"` |
| `crypto.rs:128-131` | pub fn（`#[cfg(not(target_os = "windows"))]`） | `encrypt(_plaintext: &str) -> Result<String, String>` | 桩，恒返回 `Err("加密存储仅桌面端支持")` |
| `crypto.rs:133-136` | pub fn（`#[cfg(not(target_os = "windows"))]`） | `decrypt(_encrypted_base64: &str) -> Result<String, String>` | 桩，恒返回 `Err("加密存储仅桌面端支持")` |

本模块未定义任何 `trait`。

### self_service/mod.rs

模块级文档（`self_service/mod.rs:1-41`）逆向记录了 9 步协议；下表为代码项清单。

| 位置 | 可见性 | 项 | 用途 |
| --- | --- | --- | --- |
| `self_service/mod.rs:49` | pub const | `SELF_BASE_URL = "http://10.1.80.200:8080/Self"` | 无锡学院部署的自助服务基地址（仅校园网内网可达） |
| `self_service/mod.rs:52-53` | 私有 static | `RE_CHECKCODE = name="checkcode"[^>]*value="(\d+)"` | 登录页 hidden 验证码提取 |
| `self_service/mod.rs:54-55` | 私有 static | `RE_CSRFTOKEN = name="csrftoken"[^>]*value="([0-9a-fA-F-]{36})"` | 绑定页 hidden CSRF token（UUID）提取 |
| `self_service/mod.rs:57-58` | 私有 static | `RE_FLD_VALUE = value="([^"]*)"\s+name="FLDEXTRA(\d)"` | 绑定页 FLDEXTRA1..6 预填值提取（要求 `value` 在 `name` 之前） |
| `self_service/mod.rs:60` | 私有 static | `RE_SWAL_MSG = \}\)\('((?:[^'\\]\|\\.)*)'\);` | 内嵌 swal 提示文本提取（整页唯一） |
| `self_service/mod.rs:64-75` | pub struct | `BindParams<'a>` | 绑定请求参数（见"结构体与字段"） |
| `self_service/mod.rs:78-85` | pub fn | `operator_fld_pair(operator: &str) -> Option<(usize, usize)>` | 运营商后缀 → `FLDEXTRA` 账号/密码字段序号：`@cmcc`→(1,2)、`@telecom`→(3,4)、`@unicom`→(5,6)，其余 `None` |
| `self_service/mod.rs:88-90` | pub fn | `extract_checkcode(html: &str) -> Option<String>` | 登录页 `checkcode` 提取 |
| `self_service/mod.rs:93-95` | pub fn | `extract_csrftoken(html: &str) -> Option<String>` | 绑定页 `csrftoken` 提取 |
| `self_service/mod.rs:100-110` | pub fn | `extract_fld_values(html: &str) -> [String; 6]` | FLDEXTRA1..6 预填值（未绑定为空串；越界序号 `continue` 跳过） |
| `self_service/mod.rs:113-115` | pub fn | `extract_swal_msg(html: &str) -> Option<String>` | swal 提示文本提取（登录失败原因 / 绑定结果） |
| `self_service/mod.rs:118-120` | pub fn | `is_bind_success(msg: &str) -> bool` | 判定 `msg` 是否含 `"绑定运营商账号信息成功"` |
| `self_service/mod.rs:123-128` | pub fn | `md5_hex(input: &str) -> String` | 标准小写 MD5 十六进制（自助服务密码提交格式） |
| `self_service/mod.rs:132-144` | 私有 fn | `build_session_client(local_addr: Option<IpAddr>) -> Result<reqwest::Client, String>` | 构造一次性会话客户端：15s 超时 / 3s 连接超时 / `no_proxy` / 禁用自动重定向 / `cookie_store(true)` / 伪装 UA / 可选 `local_address` |
| `self_service/mod.rs:148-195` | pub async fn | `bind_operator(params: &BindParams<'_>, local_addr: Option<IpAddr>) -> Result<String, String>` | 绑定运营商账号全流程，成功返回服务端提示文本 |
| `self_service/mod.rs:198-203` | pub struct | `OperatorBinding` | 绑定状态展示（见"结构体与字段"） |
| `self_service/mod.rs:207-219` | pub async fn | `query_bind_status(account, password, local_addr) -> Result<[Option<OperatorBinding>; 3], String>` | 查询三家运营商绑定状态；数组顺序为 `[移动, 电信, 联通]` |
| `self_service/mod.rs:223-239` | pub async fn | `reveal_credential(account, password, operator, local_addr) -> Result<(String, String), String>` | 返回该运营商（手机号, 运营商账户密码）明文；双空返回 `Err("该运营商尚未绑定")`；调用方须先做本地身份验证 |
| `self_service/mod.rs:243-258` | pub fn | `mask_account(account: &str) -> String` | 账号掩码：长度 ≥8 → 前 3 后 2；>4 → 前 1 后 1；否则整体 `*` 重复 |
| `self_service/mod.rs:260-268` | 私有 fn | `binding_from(acct: &str, pwd: &str) -> Option<OperatorBinding>` | 账号为空 → `None`；否则掩码账号 + `password_set` |
| `self_service/mod.rs:272-328` | 私有 async fn | `login_session(account, password, local_addr) -> Result<reqwest::Client, String>` | 建立登录会话 3 步：取 checkcode → 预热 randomCode → POST verify（密码 MD5） |
| `self_service/mod.rs:332-352` | 私有 async fn | `login_and_fetch_bind_page(account, password, local_addr) -> Result<(reqwest::Client, String), String>` | 在 `login_session` 之上取绑定表单页 HTML（含第 4 步） |
| `self_service/mod.rs:356-373` | 私有 async fn | `fetch_dashboard_json(client, path, label) -> Result<serde_json::Value, String>` | 请求 dashboard 下 JSON 接口（302 判会话失效） |
| `self_service/mod.rs:377-386` | pub async fn | `query_dashboard(account, password, local_addr) -> Result<(Value, Value), String>` | 一次会话连拉 `getOnlineList` 与 `getLoginHistory`，原始 JSON 透传 |
| `self_service/mod.rs:391-420` | pub async fn | `query_online_log(account, password, start_time, end_time, local_addr) -> Result<serde_json::Value, String>` | 拉 `/Self/bill/getUserOnlineLog`（固定 `pageNumber=1&pageSize=500`），返回 `{rows, summary, total}` |
| `self_service/mod.rs:423-428` | 私有 fn | `parse_offline_success(text: &str) -> bool` | 解析 `{"success":bool}`；非 JSON/缺字段/非布尔 → `false` |
| `self_service/mod.rs:431-455` | pub async fn | `offline_session(account, password, session_id, local_addr) -> Result<(), String>` | `/Self/dashboard/tooffline?sessionid=` 踢会话下线 |

## 结构体与字段

### `BindParams<'a>`（`self_service/mod.rs:64-75`）

| 位置 | 字段 | 类型 | 含义 |
| --- | --- | --- | --- |
| `self_service/mod.rs:66` | `account` | `&'a str` | 学号（自助服务系统登录账号，与 Portal 登录账号相同） |
| `self_service/mod.rs:68` | `password` | `&'a str` | 自助服务系统登录密码（明文，协议内 MD5 后提交；默认身份证后 6 位） |
| `self_service/mod.rs:70` | `operator` | `&'a str` | 运营商后缀（与 `Config.operator` 同源：`@cmcc`/`@telecom`/`@unicom`） |
| `self_service/mod.rs:72` | `phone` | `&'a str` | 运营商账号（用户办理套餐的手机号） |
| `self_service/mod.rs:74` | `sms_password` | `&'a str` | 运营商账户密码（办理套餐时运营商短信下发） |

凭据仅内存传递，调用方不得落盘/写日志（`self_service/mod.rs:63` 注释）。

### `OperatorBinding`（`self_service/mod.rs:198-203`）

| 位置 | 字段 | 类型 | 含义 |
| --- | --- | --- | --- |
| `self_service/mod.rs:200` | `masked_account` | `String` | 掩码后的运营商账号（`mask_account` 产物） |
| `self_service/mod.rs:202` | `password_set` | `bool` | 运营商账户密码是否已设置（从不返回明文） |

### `crypto::dpapi::DataBlob`（`crypto.rs:5-9`，私有）

| 位置 | 字段 | 类型 | 含义 |
| --- | --- | --- | --- |
| `crypto.rs:7` | `cb_data` | `u32` | 缓冲区字节数 |
| `crypto.rs:8` | `pb_data` | `*mut u8` | 缓冲区指针（输出由 `LocalFree` 释放） |

### 常量与正则汇总

| 项 | 位置 | 值/语义 |
| --- | --- | --- |
| `SELF_BASE_URL` | `self_service/mod.rs:49` | `http://10.1.80.200:8080/Self`，硬编码不可配置 |
| `RE_CHECKCODE` | `self_service/mod.rs:52-53` | 4 位数字验证码（`\d+`） |
| `RE_CSRFTOKEN` | `self_service/mod.rs:54-55` | 36 位 UUID（`[0-9a-fA-F-]{36}`） |
| `RE_FLD_VALUE` | `self_service/mod.rs:57-58` | `value="..."` 出现在 `name="FLDEXTRA{n}"` 之前，中间可跨行空白 |
| `RE_SWAL_MSG` | `self_service/mod.rs:60` | 匹配 `})('消息');`，消息体允许 `\` 转义 |
| 固定表单参数 | `self_service/mod.rs:301-306` | `account`、`password=md5_hex(pwd)`、`checkcode`、`code=""` |
| 固定查询参数 | `self_service/mod.rs:404-405` | `pageNumber=1`、`pageSize=500` |
| `Referer` 头 | `self_service/mod.rs:176` | `{SELF_BASE_URL}/service/operatorId` |

## Data Flow

### 账号密码落盘加密（DPAPI）

```
commands/config_cmd.rs::save_config / commands/account.rs::save_account
  → config::persist::save_config_to_disk_encrypted(data_dir, config)   (persist.rs:153-168)
      ├─ password 非空 → account::crypto::encrypt(&password)           (persist.rs:160)
      │     → base64::Engine::decode/encode(STANDARD) + dpapi::encrypt
      │       → call_dpapi → CryptProtectData（flags=0）→ LocalFree
      └─ self_password 非空 → 同上                                      (persist.rs:163)
  → persist::atomic_write(config.json)

读取方向（commands/config_cmd.rs:32,43 与 commands/account.rs:256）
  → account::crypto::decrypt(base64) → CryptUnprotectData → String::from_utf8
     失败 → 桌面端：仅清空该字段（config_cmd.rs:36-38）/ 账号文件报错（account.rs:259-260）
     非 Windows：恒 Err("加密存储仅桌面端支持") → 同失败分支
```

### 绑定运营商账号

```
commands/self_service.rs::bind_operator（命令层已校验 11 位手机号、凭据非空）
  → self_service::bind_operator(&BindParams, local_addr)
      ├─ operator_fld_pair(operator)  → (acct_fld, pwd_fld)               (mod.rs:152-153)
      ├─ login_and_fetch_bind_page(account, password, local_addr)
      │     ├─ login_session:
      │     │     1. GET  /Self/login/           → extract_checkcode      (mod.rs:280-287)
      │     │     2. GET  /Self/login/randomCode?t=1（隐式预热，必需）      (mod.rs:290-296)
      │     │     3. POST /Self/login/verify  form: account,
      │     │            password=md5_hex(pwd), checkcode, code=""
      │     │        成功判据：Location 含 "/Self/dashboard"               (mod.rs:309-314)
      │     │        失败 → 再 GET 登录页 → extract_swal_msg 作为错误文案   (mod.rs:316-326)
      │     └─ 4. GET /Self/service/operatorId → 302 判会话失效，否则取 HTML
      ├─ extract_csrftoken(op_html) → 缺则 Err("绑定页解析失败（csrftoken 缺失）")
      ├─ extract_fld_values(op_html) → prefilled[0..6]
      ├─ 组装 form：csrftoken + FLDEXTRA1..6
      │     目标运营商字段填 phone / sms_password，
      │     其余字段回填 prefilled[i-1]（整体保存，填空串会清掉已有绑定）    (mod.rs:162-173)
      ├─ POST /Self/service/bind-operator（带 Referer，明文 form）
      │     302 → Err("登录会话失效，请重试")
      └─ extract_swal_msg(bind_html) → is_bind_success ?
            是 → Ok(msg)；空 → Err("绑定失败：服务端未返回结果，请稍后重试")；否则 → Err(msg)
```

### 查询与注销

```
query_bind_status    → login_and_fetch_bind_page → extract_fld_values → [cmcc, telecom, unicom] 掩码
reveal_credential    → login_and_fetch_bind_page → extract_fld_values[acct-1], [pwd-1]  ← 明文出站
query_dashboard      → login_session → fetch_dashboard_json(getOnlineList) + (getLoginHistory)
query_online_log     → login_session → GET /Self/bill/getUserOnlineLog（startTime/endTime/pageNumber/pageSize）
offline_session      → login_session → GET /Self/dashboard/tooffline?sessionid= → parse_offline_success
```

命令层门禁（桌面，`commands/self_service.rs`）：`ensure_identity_gate`（`:15-23`）对**改变外部状态**的命令（`bind_operator`、`self_offline_session`）要求 `self_hello_enabled` 且后端 TTL 内验证通过；`reveal_operator_credential`（`:300-304`）无条件要求 `identity_verified_recently()`；查询类命令有意不设门。`resolve_self_password`（`:50-61`）实现"前端明文优先、MASK/空回退已保存密码"。安卓端同构实现在 `android/src-tauri/src/self_service_cmds.rs`（`:100,107,128,160,208,236,263` 逐一映射本文的函数）。

## Connections

- [[desktop-config]] — `config::persist` 调用 `account::crypto::{encrypt, decrypt}`；`Config.self_password`（`config/model.rs:16`）承载自助服务密码、`self_hello_enabled`（`model.rs:21`）与 `self_reverify_each_action`（`model.rs:24`）是自助服务门禁与面板的配置面。
- [[desktop-commands]] — `commands/account.rs`（`crypto::encrypt/decrypt` 用于账号档案：`:103`、`:161`、`:256`）、`commands/self_service.rs`（六个自助服务命令 + `verify_windows_identity`）、`commands/config_cmd.rs`（`crypto::decrypt`：`:32`、`:43`）。
- [[desktop-auth]] — `Config.operator` 的取值域（`""`/`@telecom`/`@unicom`/`@cmcc`）与 `account::crypto` 落盘的登录密码是同一份配置；自助服务绑定是校园网登录的前置条件（`self_service/mod.rs:4-5`）。
- [[desktop-infra]] — `infra::state::AppState` / `CommandResult`、`infra::command_context::AppHandleExt`、`platform::identity`（`verify_identity` / `note_identity_verified` / `identity_verified_recently`）、`platform::console_output`（同层的平台能力实现）。
- 安卓端（`android/src-tauri/src/`）：`self_service_cmds.rs` 全量复用 `campus_login_lib::self_service`；`account_cmds.rs` **不使用** `campus_login_lib::account::crypto`（改用 `tauri-plugin-campus-keystore`，见 `android/src-tauri/Cargo.toml` 依赖注释）。

## Known Issues

1. **自助服务地址硬编码**：`self_service/mod.rs:49` 的 `SELF_BASE_URL = "http://10.1.80.200:8080/Self"` 写死在代码里，不是 `Config` 字段；该模块被安卓端复用，换学校部署必须改代码并双端同步。
2. **`mask_account` 的文档注释与实现不一致**：`self_service/mod.rs:241-242` 注释称"其他格式 ≥8 位前 2 后 2"，实现（`self_service/mod.rs:251-257`）对 `n >= 8` 一律 `head(3) + "******" + tail(2)`，测试 `self_service/mod.rs:517` 断言 `mask_account("abc12345") == "abc******45"` 证实是前 3 后 2。
3. **所有页面解析都依赖精确的 HTML/JS 形状**：`RE_SWAL_MSG`（`self_service/mod.rs:60`）要求提示形如 `})('...')` 且"整页唯一"；`RE_FLD_VALUE`（`:57-58`）要求 `value` 属性出现在 `name` 之前；`RE_CHECKCODE`（`:52-53`）要求 `value` 紧跟 `name` 且为纯数字。任一格式变动都会静默返回 `None`/空串（错误只在后续步骤以"csrftoken 缺失"/"bind 失败"等形式浮现）。
4. **预填值解析失败会清空其他运营商的绑定**：绑定表单是整体保存（`self_service/mod.rs:97-99`、`:162-173`），非目标运营商字段必须回填 `prefilled[i-1]`。若第 3 条的解析失败导致 `prefilled` 全为空串，提交即会把已有绑定清掉——代码层没有任何"预填值为空则跳过提交"的保护。
5. **验证码流程依赖特定部署**：`self_service/mod.rs:301-306` 固定提交 `code=""`，并在 `:290-296` 强制预热 `randomCode`。注释（`:9-11`）说明本部署验证码输入框隐藏；若部署强制校验验证码，登录会 302 回登录页并在 `:324-325` 报"验证码错误！"。
6. **登录成功判据是重定向 Location 子串**：`self_service/mod.rs:309-314` 用 `loc.contains("/Self/dashboard")`；改动 dashboard 路径即失效。同理，所有"会话失效"判定都用 `is_redirection()`（`:180`、`:344`、`:365`、`:410`、`:443`），依赖服务端固定 302 而非 401/403。
7. **注销结果不可信**：`self_service/mod.rs:28-29` 注释记录"实测对不存在的 sessionid 也返回 true"，`parse_offline_success`（`:423-428`）只透传 `success` 字段，因此 `offline_session` 返回 `Ok(())` 不代表目标会话真的下线。
8. **每条命令都重新登录一次**：`query_dashboard`（`:382`）、`query_online_log`（`:398`）、`offline_session`（`:437`）各自调用 `login_session`，而 `build_session_client`（`:132-144`）每次新建客户端（`cookie_store` 不跨调用保留）。一次面板刷新 = N ×（3 次登录往返 + 1~2 次业务请求），且每次都要提交一次 MD5 密码，存在被服务端风控/锁定计数的风险。
9. **`query_bind_status` 丢弃会话客户端**：`self_service/mod.rs:212` 绑定 `let (_client, op_html)`，函数返回后 cookie 会话即被释放（同一函数内的后续请求不受影响）。
10. **无重试、无退避、无并发限制**：所有请求单发（`self_service/mod.rs:174-179`、`:280-285`，等等），失败即返回错误文案给用户；超时固定 15s/3s（`:134-135`）。
11. **客户端构造与 `network::client` 不一致**：`self_service/mod.rs:132-144` 不设 `min_tls_version`、不走客户端池、不设默认 `Cache-Control` 头（对比 `network/client.rs:43-67` 的 `build_client`）。当前基地址是明文 http，暂无实际影响，但两套客户端策略并存。
12. **DPAPI 调用未设 `CRYPTPROTECT_UI_FORBIDDEN`**：`crypto.rs:80` 与 `crypto.rs:99` 的 `flags` 参数均为 `0`。按 Win32 语义，缺少该 flag 时 `CryptUnprotectData` 在特定场景（凭据上下文不匹配）理论上可能弹出 UI 提示；本应用是无界面后台/托盘场景，建议关注是否会阻塞调用线程。
13. **DPAPI 失败信息不含系统错误码**：`crypto.rs:57-59` 只在 `result == 0` 时返回固定文案，未调 `GetLastError`，排障时无法区分"凭据不匹配/内存不足/参数非法"。
14. **空明文加密必然失败**：`crypto.rs:45-49` 把入参复制为 `DataBlob{cb_data: len, pb_data: ptr}`；DPAPI 不接受 `cb_data == 0`，因此 `encrypt("")` 会返回 `Err("DPAPI加密失败")`。现有调用方都先判空（`persist.rs:159,162`、`commands/account.rs:102,158`），但函数契约未体现该约束。
15. **非 Windows 是硬失败桩**：`crypto.rs:128-136` 两个函数恒返回 `Err("加密存储仅桌面端支持")`。这意味着同一份 `config/persist.rs:153-168` 在非 Windows target 上必然失败；安卓端因此**不使用** `config::persist`，改由 `android/src-tauri/src/config_state.rs` + `tauri-plugin-campus-keystore` 自行实现（`android/src-tauri/Cargo.toml` 注释："AndroidKeyStore AES-GCM 加解密(密码落盘加密,替代桌面 DPAPI)"）。
16. **`extract_fld_values` 的越界序号被静默丢弃**：`self_service/mod.rs:103-106` 对 `cap[2]` 解析失败或不在 `1..=6` 的匹配 `continue`，若页面引出 `FLDEXTRA7` 等新字段会被无声忽略（与"整体保存"语义叠加时风险同上第 4 条）。
17. **没有对 `pageSize=500` 的分页兜底**：`self_service/mod.rs:405` 硬编码 `pageSize=500`，返回 `total > 500` 时后端只透传首页数据（前端不做翻页，见 `self_service/mod.rs:388-390` 注释）。
18. **日期合法性校验在命令层而非协议层**：`self_service::query_online_log`（`:391-420`）不校验 `start_time`/`end_time` 格式，把关的是 `commands/self_service.rs:37-45` 的 `is_iso_date` 与 `:203-205` 的先后比较；安卓端需自行实现同等校验（`android/src-tauri/src/self_service_cmds.rs:208` 直接透传字符串）。
