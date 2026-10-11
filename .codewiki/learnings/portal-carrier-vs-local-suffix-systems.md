---
title: "校园网用户自助服务系统（Dr.COM Self）协议实现"
type: learning
source_files:
  - tauri-app/src-tauri/src/self_service/mod.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
tags: [教训, 协议逆向, 运营商, eportal, 自助服务]
---

## Overview

本模块 (`tauri-app/src-tauri/src/self_service/mod.rs`) 实现了无锡学院校园网“Dr.COM Self”自助服务系统的完整 HTTP 协议交互。该系统的核心业务是**自动化绑定运营商账号**（手机号 + 短信密码），这是校园网账号正常上网的前置条件；原流程需用户手动在浏览器操作，此模块将其自动化。

此外，模块还封装了登录会话管理、Dashboard 数据查询（在线设备、近期记录）、上网记录账单查询以及修改密码等功能。所有接口均基于 2026-09-05 至 2026-10-10 的 curl 与浏览器实测逆向得出。

## Key Components

### 常量与正则表达式

*   `SELF_BASE_URL`: `tauri-app/src-tauri/src/self_service/mod.rs:38` — 自助服务系统基地址 (`http://10.1.80.200:8080/Self`)，仅校园网内网可达。
*   `RE_CHECKCODE`: `mod.rs:41` — 提取登录页 hidden `checkcode` (4位数字)。
*   `RE_CSRFTOKEN`: `mod.rs:43` — 提取绑定页/改密页 hidden `csrftoken` (UUID)。
*   `RE_FLD_VALUE`: `mod.rs:46` — 提取绑定页 FLDEXTRA1..6 预填值（value 属性在 name 之前，可能跨行）。
*   `RE_SWAL_MSG`: `mod.rs:48` — 提取页面内嵌 swal 提示文本 (`})('消息');`)。

### 数据结构

*   `BindParams<'a>`: `mod.rs:73–83` — 绑定请求参数。
    *   `account`: 学号（自助服务系统登录账号）。
    *   `password`: 自助服务系统登录密码（明文，内部 MD5）。
    *   `operator`: 运营商后缀 (`@cmcc` / `@telecom` / `@unicom`)。
    *   `phone`: 运营商账号（手机号）。
    *   `sms_password`: 运营商账户密码（短信下发）。
*   `OperatorBinding`: `mod.rs:162–166` — 已绑定的运营商凭据状态。
    *   `masked_account`: 掩码后的手机号（前三后二）。
    *   `password_set`: 密码是否已设置。

### 公开函数

*   `operator_fld_pair(operator: &str) -> Option<(usize, usize)>`: `mod.rs:86–92` — 运营商后缀映射到绑定表单 FLDEXTRA 字段序号（移动=1/2，电信=3/4，联通=5/6）。
*   `extract_checkcode(html: &str) -> Option<String>`: `mod.rs:95–97` — 提取 checkcode。
*   `extract_csrftoken(html: &str) -> Option<String>`: `mod.rs:100–102` — 提取 csrftoken。
*   `extract_fld_values(html: &str) -> [String; 6]`: `mod.rs:109–119` — 提取 FLDEXTRA1..6 预填值。**关键逻辑**：绑定表单是整体保存，提交时必须带回非目标运营商的预填原值，填空串会清掉已有绑定。
*   `extract_swal_msg(html: &str) -> Option<String>`: `mod.rs:122–124` — 提取 swal 提示文本。
*   `is_bind_success(msg: &str) -> bool`: `mod.rs:127–129` — 判定绑定成功（包含"绑定运营商账号信息成功"）。
*   `md5_hex(input: &str) -> String`: `mod.rs:132–136` — 标准小写 MD5 hex。
*   `bind_operator(params: &BindParams<'_>, local_addr: Option<IpAddr>) -> Result<String, String>`: `mod.rs:169–213` — **完整绑定流程**：登录 → 取绑定表单 → 提交绑定。成功返回服务端提示文本。
*   `query_bind_status(account, password, local_addr) -> Result<[Option<OperatorBinding>; 3], String>`: `mod.rs:227–237` — 查询当前绑定状态（掩码显示，不返回明文凭据）。
*   `reveal_credential(account, password, operator, local_addr) -> Result<(String, String), String>`: `mod.rs:241–254` — 查看某运营商的明文凭据（需 Windows 本地身份验证前置）。
*   `change_password(account, password, new_password, local_addr) -> Result<String, String>`: `mod.rs:283–328` — 修改自助服务系统密码（= 校园网登录密码）。成功后服务端强制下线。
*   `mask_account(account: &str) -> String`: `mod.rs:331–344` — 账号隐私掩码（11位手机号前3后2，其他长度规则不同）。
*   `query_dashboard(account, password, local_addr) -> Result<(serde_json::Value, serde_json::Value), String>`: `mod.rs:387–393` — 查询 Dashboard 数据（在线信息 + 近期上网记录）。
*   `query_online_log(account, password, start_time, end_time, local_addr) -> Result<serde_json::Value, String>`: `mod.rs:398–421` — 查询上网记录账单页数据（日期范围，大 pageSize 拉全）。
*   `offline_session(account, password, session_id, local_addr) -> Result<(), String>`: `mod.rs:437–456` — 注销指定上网会话。

### 私有辅助函数

*   `build_session_client(local_addr)`: `mod.rs:139–154` — 构建独立于 CLIENT_POOL 的会话客户端（cookie_store 保持 JSESSIONID，禁用自动重定向）。
*   `parse_change_password_response(text)`: `mod.rs:257–279` — 解析改密 JSON 响应。
*   `binding_from(acct, pwd)`: `mod.rs:347–353` — 解析单个运营商绑定状态。
*   `login_session(account, password, local_addr)`: `mod.rs:356–403` — 登录自助服务系统并返回会话客户端（含预热验证码步骤）。
*   `login_and_fetch_bind_page(account, password, local_addr)`: `mod.rs:406–428` — 登录并获取绑定表单页 HTML。
*   `fetch_dashboard_json(client, path, label)`: `mod.rs:431–447` — 请求 dashboard JSON 并解析。
*   `parse_offline_success(text)`: `mod.rs:450–455` — 解析 tooffline 响应 success 判定。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   self_service/mod.rs                       │
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │ Bind Flow    │    │ Query Flow   │    │ Change Pass  │  │
│  │ bind_operator│    │ query_*      │    │ change_pass  │  │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘  │
│         │                   │                   │           │
│  ┌──────▼───────────────────▼───────────────────▼───────┐  │
│  │              login_session()                         │  │
│  │  1. GET /Self/login/        → extract_checkcode     │  │
│  │  2. GET /Self/login/randomCode → 预热验证码          │  │
│  │  3. POST /Self/login/verify → 302 /dashboard (OK)   │  │
│  └──────┬──────────────────────────────────────────────┘  │
│         │ (reqwest::Client with Cookie Store)              │
│  ┌──────▼──────────────────────────────────────────────┐  │
│  │              Session Operations                     │  │
│  │  • GET /Self/service/operatorId → extract_fld_values│  │
│  │  • POST /Self/service/bind-operator                 │  │
│  │  • GET /Self/dashboard/getOnlineList                │  │
│  │  • GET /Self/dashboard/tooffline?sessionid=         │  │
│  │  • GET /Self/bill/getUserOnlineLog                  │  │
│  │  • POST /Self/setting/changePasswordMethod          │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 绑定运营商账号流程

1.  **登录预热**：`GET /Self/login/` 获取 `checkcode`；`GET /Self/login/randomCode` 隐式预热验证码状态（跳过则 verify 必失败，302 回登录页）。
2.  **验证登录**：`POST /Self/login/verify`，提交 `account + md5(password) + checkcode + code=`（code 为空）。成功 302 到 `/Self/dashboard`。
3.  **获取绑定页**：`GET /Self/service/operatorId`，获取 `csrftoken` 和 FLDEXTRA1..6 预填值。
4.  **提交绑定**：`POST /Self/service/bind-operator`，提交 `csrftoken` 及 FLDEXTRA1..6。**关键约束**：非目标运营商字段必须带回预填原值，填空串会清空已有绑定。
5.  **结果判定**：HTTP 200 重渲染绑定页，通过 `extract_swal_msg` 提取内嵌 swal 消息，检查是否包含"绑定运营商账号信息成功"。

### Dashboard 查询流程

1.  **登录会话**：复用 `login_session` 建立 cookie 会话。
2.  **并发请求**：`GET /Self/dashboard/getOnlineList` 和 `GET /Self/dashboard/getLoginHistory`。
3.  **响应结构**：
    *   `getOnlineList`：对象数组（loginTime, ip, mac, useTime, downFlow/upFlow, hostName, terminalType, sessionId）。
    *   `getLoginHistory`：数组的数组（epoch 毫秒时间戳, ip, mac, 时长分, 流量 M, 计费方式, 金额, 主机名, 终端类型）。

### 上网记录账单查询流程

1.  **登录会话**：复用 `login_session`。
2.  **请求接口**：`GET /Self/bill/getUserOnlineLog?startTime=YYYY-MM-DD&endTime=YYYY-MM-DD&pageNumber=1&pageSize=500`。
3.  **响应结构**：`{rows:[...], summary:{...}, total:N}`。
    *   `rows` 字段：loginTime/logoutTime (epoch 毫秒), time (分钟), flow/internetUpFlow/internetDownFlow/chinanetUpFlow/chinanetDownFlow/costMoney (MB/元), userIp/nasIp/nasPort, macAddress/userName。
    *   `summary` 键：大写下划线（INTERNETUPFLOW, FLOW, TIME, COSTMONEY, COU 等），与页面顶部汇总数据卡一致。

### 修改密码流程

1.  **登录会话**：复用 `login_session`。
2.  **获取改密页**：`GET /Self/setting/changePassword`，获取 `csrftoken`。
3.  **提交改密**：`POST /Self/setting/changePasswordMethod`，提交 `csrftoken + oldPassword + newPassword + confirmPassword`。
4.  **结果判定**：解析 JSON `{"state":"success"}` 或 `{"state":"fail","data":"原因"}`。
5.  **副作用**：成功时服务端强制下线（2秒后跳 `/Self/login/logout`），调用方必须同步更新本地保存的登录密码。

## Known Issues

1.  **验证码依赖预热**：`/Self/login/randomCode` 是必要隐式预热步骤。浏览器打开登录页时 `<img>` 会自动加载该地址，服务端在会话中记录验证码已发放状态。跳过此步 verify 必然 302 回登录页并提示"验证码错误！"。
2.  **绑定表单整体保存缺陷**：绑定表单提交时，非目标运营商字段必须带回预填原值。如果填空串，会把已有绑定清掉（2026-09-05 实测缺陷）。
3.  **改密后节流**：改密成功后短时间内重新登录会被服务端节流，属预期自愈。
4.  **注销宽松处理**：`tooffline` 对不存在的 sessionid 也返回 true，服务端宽松处理。
5.  **GBK 编码**：改密页为 GBK 编码，需注意 charset 解码。