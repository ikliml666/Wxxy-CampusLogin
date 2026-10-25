---
title: "校园网用户自助服务系统（Dr.COM Self）协议实现"
type: learning
source_files:
  - tauri-app/src-tauri/src/self_service/mod.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
tags: [教训, 协议逆向, 运营商, eportal, 自助服务]
---

## Overview

本模块 (`tauri-app/src-tauri/src/self_service/mod.rs`) 实现了无锡学院校园网“Dr.COM Self”自助服务系统的完整 HTTP 协议交互。核心业务是**自动化绑定运营商账号**（手机号 + 短信密码），这是校园网账号正常上网的前置条件；原流程需用户手动浏览器操作，此模块将其自动化。此外封装了登录会话管理（含直连与 WebVPN 双通道 `SelfServiceChannel`）、Dashboard 数据查询（在线设备、近期记录）、上网记录账单查询、修改密码、注销指定会话等功能。所有接口基于 2026-09-05 至 2026-10-10 的 curl 与浏览器实测逆向得出（见 `mod.rs:1-52` 模块注释）。

`tauri-app/src-tauri/src/auth/protocol.rs` 负责 Portal 认证层的登录/注销请求构建与响应解析，包含 JSONP 切片（`jsonp_json_slice` `protocol.rs:76-110`）、重试逻辑（`do_login_with_retry` `protocol.rs:195-228`、`do_logout_with_retry` `protocol.rs:449-481`）、凭据脱敏及安全 HTTP 客户端调用，为上层提供可靠网络通信基础，自助服务登录会话构建亦参考其超时/本地绑定 IP 模式（但使用独立客户端，见 `build_session_client` `mod.rs:145-157`）。

## Key Components

### 常量与正则表达式 (`self_service/mod.rs`)

*   `SELF_BASE_URL`: `mod.rs:62` — 自助服务系统基地址 (`http://10.1.80.200:8080/Self`)，仅校园网内网可达。
*   `RE_CHECKCODE`: `mod.rs:65-66` — 提取登录页 hidden `checkcode` (4位数字)。
*   `RE_CSRFTOKEN`: `mod.rs:67-68` — 提取绑定页/改密页 hidden `csrftoken` (UUID 36位)。
*   `RE_FLD_VALUE`: `mod.rs:70-71` — 提取绑定页 FLDEXTRA1..6 预填值（value 属性在 name 之前，可能跨行）。
*   `RE_SWAL_MSG`: `mod.rs:73` — 提取页面内嵌 swal 提示文本 (`})('消息');`)。

### 数据结构

*   `BindParams<'a>`: `mod.rs:77-88` — 绑定请求参数（凭据仅内存传递，不得落盘/写日志，见 `mod.rs:76`）。
    *   `account`: `mod.rs:79` — 学号（自助服务系统登录账号，与 Portal 登录账号相同）。
    *   `password`: `mod.rs:81` — 自助服务系统登录密码（明文，协议内部 MD5 后提交；默认为身份证后 6 位）。
    *   `operator`: `mod.rs:83` — 运营商后缀（`@cmcc` / `@telecom` / `@unicom`）。
    *   `phone`: `mod.rs:85` — 运营商账号（用户办理套餐的手机号）。
    *   `sms_password`: `mod.rs:87` — 运营商账户密码（办理套餐时由运营商短信下发）。
*   `OperatorBinding`: `mod.rs:278-283` — 已绑定的运营商凭据状态（供状态展示）。
    *   `masked_account`: `mod.rs:280` — 掩码后的运营商账号（手机号前三后二）。
    *   `password_set`: `mod.rs:282` — 运营商账户密码是否已设置。
*   `SelfServiceChannel`: `mod.rs:169-177` — 自助服务执行通道（直连或 WebVPN 包装），保持会话 cookie 并禁用自动重定向。
    *   `base`: `mod.rs:172` — 业务 base URL（末尾带 `/Self`：直连原样，WebVPN 为网关包装形态）。
    *   `client`: `mod.rs:174` — 会话 client（cookie 会话保持，禁自动重定向）。
    *   `local_addr`: `mod.rs:176` — 直连时的本地绑定地址（WebVPN 通道为 None）。
    *   方法 `direct(local_addr)` `mod.rs:181-187` — 构建直连通道。
    *   方法 `via_webvpn(session)` `mod.rs:190-198` — 构建 WebVPN 通道。
    *   方法 `fresh_session_client(&self)` `mod.rs:203-209` — 重试用：直连重建 client，WebVPN 原样返回。
    *   方法 `url(&self, path)` `mod.rs:212-214` — 业务 path 拼接。

### 公开函数 (`self_service/mod.rs`)

*   `operator_fld_pair(operator: &str) -> Option<(usize, usize)>`: `mod.rs:91-98` — 运营商后缀映射到绑定表单 FLDEXTRA 字段序号（移动=1/2，电信=3/4，联通=5/6）。
*   `extract_checkcode(html: &str) -> Option<String>`: `mod.rs:101-103` — 提取 checkcode。
*   `extract_csrftoken(html: &str) -> Option<String>`: `mod.rs:106-108` — 提取 csrftoken。
*   `extract_fld_values(html: &str) -> [String; 6]`: `mod.rs:113-123` — 提取 FLDEXTRA1..6 预填值。**关键逻辑**：绑定表单整体保存，提交时必须带回非目标运营商预填原值，填空串会清掉已有绑定（`mod.rs:110-112`）。
*   `extract_swal_msg(html: &str) -> Option<String>`: `mod.rs:126-128` — 提取 swal 提示文本。
*   `is_bind_success(msg: &str) -> bool`: `mod.rs:131-133` — 判定绑定成功（包含"绑定运营商账号信息成功"）。
*   `md5_hex(input: &str) -> String`: `mod.rs:136-141` — 标准小写 MD5 hex。
*   `bind_operator(params: &BindParams<'_>, local_addr: Option<IpAddr>) -> Result<String, String>`: `mod.rs:219-225` — 完整绑定流程（直连通道入口）。
*   `bind_operator_via(params: &BindParams<'_>, channel: &SelfServiceChannel) -> Result<String, String>`: `mod.rs:228-275` — 通道化绑定流程：登录 → 取绑定表单 → 提交绑定（含整体保存逻辑 `mod.rs:241-252`）。
*   `query_bind_status(account, password, local_addr) -> Result<[Option<OperatorBinding>; 3], String>`: `mod.rs:287-294` — 查询当前绑定状态（掩码显示，不返回明文凭据）。
*   `query_bind_status_via(account, password, channel) -> Result<[Option<OperatorBinding>; 3], String>`: `mod.rs:297-309` — 通道化查询绑定状态。
*   `reveal_credential(account, password, operator, local_addr) -> Result<(String, String), String>`: `mod.rs:313-321` — 查看某运营商的明文凭据（调用方必须先完成 Windows 本地身份验证，见 `mod.rs:312`）。
*   `reveal_credential_via(account, password, operator, channel) -> Result<(String, String), String>`: `mod.rs:324-340` — 通道化查看明文凭据。
*   `change_password(account, password, new_password, local_addr) -> Result<String, String>`: `mod.rs:364-372` — 修改自助服务系统密码（= 校园网登录密码，凭据统一语义）。成功后服务端强制下线，调用方必须同步更新本地密码。
*   `change_password_via(account, password, new_password, channel) -> Result<String, String>`: `mod.rs:375-420` — 通道化改密流程。
*   `mask_account(account: &str) -> String`: `mod.rs:424-439` — 账号隐私掩码（11位手机号前3后2，其他格式≥8位前2后2、5~7位前1后1，过短整体打码）。
*   `query_dashboard(account, password, local_addr) -> Result<(serde_json::Value, serde_json::Value), String>`: `mod.rs:608-615` — 查询 Dashboard 数据（在线信息 + 近期上网记录）。
*   `query_dashboard_via(account, password, channel) -> Result<(serde_json::Value, serde_json::Value), String>`: `mod.rs:618-627` — 通道化 Dashboard 查询。
*   `query_online_log(account, password, start_time, end_time, local_addr) -> Result<serde_json::Value, String>`: `mod.rs:632-641` — 查询上网记录账单页数据（日期范围，pageSize=500 拉全，返回 `{rows, summary, total}`）。
*   `query_online_log_via(account, password, start_time, end_time, channel) -> Result<serde_json::Value, String>`: `mod.rs:644-672` — 通道化上网记录查询。
*   `offline_session(account, password, session_id, local_addr) -> Result<(), String>`: `mod.rs:683-691` — 注销指定上网会话（GET tooffline?sessionid=）。
*   `offline_session_via(account, password, session_id, channel) -> Result<(), String>`: `mod.rs:694-719` — 通道化注销会话。

### 私有辅助函数 (`self_service/mod.rs`)

*   `build_session_client(local_addr)`: `mod.rs:145-157` — 构建独立于 CLIENT_POOL 的会话客户端（cookie_store 保持 JSESSIONID，禁用自动重定向）。
*   `parse_change_password_response(text)`: `mod.rs:345-359` — 解析改密 JSON 响应（`{"state":"success"}` / `{"state":"fail","data":原因}`）。
*   `binding_from(acct, pwd)`: `mod.rs:441-449` — 解析单个运营商绑定状态。
*   `login_session(channel, account, password)`: `mod.rs:453-510` — 登录自助服务系统（含预热验证码步骤），在 channel 会话中种下登录态。
*   `is_retryable_transport_error(err: &str) -> bool`: `mod.rs:516-525` — 传输层可重试症状判定（仅网络/读取类错误标记）。
*   `login_session_with_retry(channel, account, password)`: `mod.rs:534-558` — login_session 整体重试包装（最多3次，间隔1s，仅传输层错误重试）。
*   `login_and_fetch_bind_page(channel, account, password)`: `mod.rs:562-582` — 登录并获取绑定表单页 HTML。
*   `fetch_dashboard_json(channel, path, label)`: `mod.rs:586-604` — 请求 dashboard JSON 并解析。
*   `parse_offline_success(text)`: `mod.rs:675-680` — 解析 tooffline 响应 success 判定（非 JSON/缺字段按失败）。

### auth/protocol.rs 关键组件

*   `LOGOUT_PLACEHOLDER_ACCOUNT` / `LOGOUT_PLACEHOLDER_PASSWORD`: `protocol.rs:3-4` — 注销请求占位账号密码。
*   `MAX_HTTP_BODY`: `protocol.rs:14` — 响应体读取上限 1MB。
*   `random_v() -> String`: `protocol.rs:6-11` — 生成登录/注销 URL 中的随机 `v` 参数（防缓存）。
*   `read_bounded_body(resp, label) -> String`: `protocol.rs:21-41` — 同步读响应体限1MB，失败/超限返回空串+告警。
*   `read_bounded_body_async(resp, label) -> Result<String, String>`: `protocol.rs:46-63` — async 版，限长与 charset 解码一致，失败返回 Err（自助服务模块调用此版本）。
*   `jsonp_json_slice(response: &str) -> &str`: `protocol.rs:76-110` — 按花括号平衡（字符串/转义感知）截取 JSONP 中首个完整 JSON 对象。
*   `do_login_request(user, password, operator, adapter_ip) -> Result<Value, String>`: `protocol.rs:124-193` — 构建并发送 Portal 登录请求（密码明文 GET 参数，安全审计 A18）。
*   `do_login_with_retry(user, password, operator, adapter_ip, max_retries, is_quitting) -> Result<Value, String>`: `protocol.rs:195-228` — 登录重试与退出标志检查。
*   `parse_login_result(response: &str) -> Result<Value, String>`: `protocol.rs:230-279` — 解析登录响应，区分 result 码与 msg 关键词。
*   `do_logout_request(user, adapter_ip, is_quitting) -> Result<Value, String>`: `protocol.rs:281-385` — Radius 注销先行 + MAC 解绑收尾。
*   `radius_logout_round(client, logout_url, round) -> Result<bool, String>`: `protocol.rs:390-406` — 单轮 Radius 注销请求。
*   `mac_unbind_request(client, unbind_url) -> Result<bool, String>`: `protocol.rs:411-427` — MAC 解绑请求（best-effort）。
*   `merge_logout_results(any_radius_ok, any_unbind_ok) -> &'static str`: `protocol.rs:433-440` — 合并注销与解绑结果消息。
*   `ip_to_eportal_int(ip: &str) -> Option<u32>`: `protocol.rs:445-447` — 点分 IPv4 → ePortal 整数形式。
*   `do_logout_with_retry(user, adapter_ip, max_retries, is_quitting) -> Result<Value, String>`: `protocol.rs:449-481` — 注销重试。
*   `parse_logout_result(response: &str) -> Result<Value, String>`: `protocol.rs:483-530` — 解析注销响应，HTML 含特定词亦判成功。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   self_service/mod.rs                       │
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │ Bind Flow    │    │ Query Flow   │    │ Change Pass  │  │
│  │ bind_operator│    │ query_*      │    │ change_pass  │  │
│  │ (mod.rs:219) │    │ (mod.rs:287+ │    │ (mod.rs:364) │  │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘  │
│         │                   │                   │           │
│  ┌──────▼───────────────────▼───────────────────▼───────┐  │
│  │   login_session_with_retry() (mod.rs:534)            │  │
│  │  -> login_session (mod.rs:453):                     │  │
│  │  1. GET /Self/login/        → extract_checkcode     │  │
│  │  2. GET /Self/login/randomCode?t=1 → 预热验证码     │  │
│  │  3. POST /Self/login/verify → 302 /dashboard (OK)   │  │
│  └──────┬──────────────────────────────────────────────┘  │
│         │ (SelfServiceChannel: direct / via_webvpn)        │
│  ┌──────▼──────────────────────────────────────────────┐  │
│  │              Session Operations                     │  │
│  │  • GET /Self/service/operatorId → extract_fld_values│  │
│  │  • POST /Self/service/bind-operator (mod.rs:253)    │  │
│  │  • GET /Self/dashboard/getOnlineList (mod.rs:624)   │  │
│  │  • GET /Self/dashboard/tooffline?sessionid=         │  │
│  │  • GET /Self/bill/getUserOnlineLog (mod.rs:654)     │  │
│  │  • POST /Self/setting/changePasswordMethod          │  │
│  └─────────────────────────────────────────────────────┘  │
│   ↓ 依赖底层 HTTP/解析模式                                  │
│  auth/protocol.rs (Portal 登录/注销, JSONP 解析, 限长读)    │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 绑定运营商账号流程

1.  **登录预热与验证**（`login_session` `mod.rs:453-510`，由 `login_and_fetch_bind_page` `mod.rs:562-582` 调用）：
    *   `GET /Self/login/` 获取 `checkcode`（`mod.rs:466-474`）。
    *   `GET /Self/login/randomCode?t=1` 隐式预热验证码状态（`mod.rs:470-477`，跳过则 verify 必失败）。
    *   `POST /Self/login/verify` 提交 `account + md5(password) + checkcode + code=`（`mod.rs:480-490`），成功 302 到 `/Self/dashboard`。
2.  **获取绑定页**：`GET /Self/service/operatorId` 获取 `csrftoken` 和 FLDEXTRA1..6 预填值（`mod.rs:570-581`）。
3.  **提交绑定**（`bind_operator_via` `mod.rs:228-275`）：构建表单，非目标运营商字段必须带回预填原值（`mod.rs:241-252`），`POST /Self/service/bind-operator`（`mod.rs:253-259`）。
4.  **结果判定**：HTTP 200 重渲染绑定页，通过 `extract_swal_msg` 提取内嵌 swal 消息（`mod.rs:267`），检查是否包含"绑定运营商账号信息成功"（`is_bind_success` `mod.rs:131`）。

### Dashboard 查询流程

1.  **登录会话**：`login_session_with_retry` 建立 cookie 会话（`mod.rs:623`）。
2.  **请求接口**：`fetch_dashboard_json` 依次请求 `getOnlineList` 与 `getLoginHistory`（`mod.rs:624-625`）。
3.  **响应结构**（见模块注释 `mod.rs:22-27`）：
    *   `getOnlineList`：对象数组（loginTime 字符串、ip、mac 12位hex、useTime 秒、downFlow/upFlow KB、hostName、terminalType "#PC" 带#前缀、sessionId 供注销）。
    *   `getLoginHistory`：数组的数组（上线/注销时间 epoch 毫秒、ip、mac、时长分、流量 M、计费方式 1时长/2流量/3包月、金额、主机名、终端类型）。

### 上网记录账单查询流程

1.  **登录会话**：复用 `login_session_with_retry`（`mod.rs:651`）。
2.  **请求接口**：`GET /Self/bill/getUserOnlineLog?startTime=YYYY-MM-DD&endTime=YYYY-MM-DD&pageNumber=1&pageSize=500`（`mod.rs:652-660`）。
3.  **响应结构**（见 `mod.rs:33-41`）：`{"rows":[...],"summary":{...},"total":N}`。
    *   `rows` 行字段 loginTime/logoutTime（epoch 毫秒）、time（分钟）、flow/internetUpFlow/internetDownFlow/chinanetUpFlow/chinanetDownFlow/costMoney（数值，MB/元）、userIp/nasIp/nasPort、macAddress/userName 等。
    *   `summary` 键大写下划线（INTERNETUPFLOW/INTERNETDOWNFLOW/CHINANETUPFLOW/CHINANETDOWNFLOW/FLOW/TIME/COSTMONEY/COU=记录数）。

### 修改密码流程

1.  **登录会话**：`login_session_with_retry`（`mod.rs:381`）。
2.  **获取改密页**：`GET /Self/setting/changePassword`（GBK 页面），获取 `csrftoken`（`mod.rs:384-397`）。
3.  **提交改密**：`POST /Self/setting/changePasswordMethod` 提交 `csrftoken + oldPassword(即登录密码) + newPassword + confirmPassword`（`mod.rs:400-411`）。
4.  **结果判定**：解析 JSON（`parse_change_password_response` `mod.rs:345-359`，调用处 `mod.rs:419`）`{"state":"success"}` 成功；`{"state":"fail","data":原因}` 失败。
5.  **副作用**：成功时服务端强制下线（2秒后跳 `/Self/login/logout`），调用方必须同步更新本地保存的登录密码（`mod.rs:47-48, 361-363`）。

## Connections

*   [[Portal 认证协议]] (`auth/protocol.rs`) — 底层登录/注销请求构建与解析，自助服务登录复用类似 HTTP 客户端超时/本地地址绑定模式，但使用独立 cookie 客户端。
*   [[Config 配置模块]] — 运营商后缀配置同源（`operator_fld_pair` `mod.rs:91` 与 Config.operator 同源，见 `mod.rs:82` 注释）。
*   [[WebVPN 会话模块]] — `SelfServiceChannel::via_webvpn` 依赖 `crate::vpn::WebVpnSession` 提供包装 URL 与共享 cookie 的 client（`mod.rs:190-198`）。

## Known Issues

1.  **验证码依赖预热**：`/Self/login/randomCode` 是必要隐式预热步骤（浏览器 `<img>` 自动加载）。跳过此步 verify 必然 302 回登录页并提示"验证码错误！"（`mod.rs:8-11, 470-477`）。
2.  **绑定表单整体保存缺陷**：绑定表单提交时，非目标运营商字段必须带回预填原值。如果填空串，会把已有绑定清掉（2026-09-05 实测缺陷，`mod.rs:110-112, 241-252`）。
3.  **改密后节流**：改密成功后短时间内重新登录会被服务端节流，属预期自愈（`mod.rs:52, 361-363`）。
4.  **注销宽松处理**：`tooffline` 对不存在的 sessionid 也返回 `{"success":true}`，服务端宽松处理（`mod.rs:28-29, 675-680`）。
5.  **GBK 编码**：改密页为 GBK 编码，需注意 charset 解码（`mod.rs:44`，`read_bounded_body_async` 按 Content-Type charset 解码 `protocol.rs:54,60`）。
6.  **传输层偶发断开**：服务器偶发连接中途断开/chunked 截断（"error decoding response body"），新增 `login_session_with_retry` 与 `is_retryable_transport_error` 仅对传输层错误重试最多3次（`mod.rs:516-558`）。
7.  **明文凭据内存限制**：`BindParams` 等明文密码仅内存传递，调用方不得落盘/写日志（`mod.rs:76, 313-319`）；`reveal_credential` 要求先完成本地身份验证（`mod.rs:312`）。