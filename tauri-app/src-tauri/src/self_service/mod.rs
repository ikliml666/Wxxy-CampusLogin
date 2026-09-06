//! 校园网用户自助服务系统（Dr.COM Self）协议实现。
//!
//! 逆向于 2026-09-05（curl + 浏览器实测），供新手教程"绑定运营商账号"使用。
//! 绑定运营商账号（手机号 + 办理套餐时运营商短信下发的运营商账户密码）是校园网账号
//! 正常上网的前置条件，原流程需用户手动浏览器操作，此模块将其自动化：
//!
//! 1. `GET  /Self/login/`              取 hidden `checkcode`（会话级 4 位数字）
//! 2. `GET  /Self/login/randomCode`    **必要隐式预热**：浏览器打开登录页时
//!    `<img>` 会自动加载该地址，服务端在会话中记录验证码已发放状态；跳过此步
//!    verify 必然 302 回登录页并提示"验证码错误！"（本部署验证码输入框隐藏，
//!    提交空 `code` 即可通过，用户无需输入）
//! 3. `POST /Self/login/verify`        `account + md5(password) + checkcode + code=`
//!    → 成功 302 `/Self/dashboard`；失败 302 回 `/Self/login/`，重新 GET 登录页
//!    可从内嵌 `})('提示文本')` 提取失败原因
//! 4. `GET  /Self/service/operatorId`  绑定表单页，取 hidden `csrftoken`（UUID）
//! 5. `POST /Self/service/bind-operator` `csrftoken + FLDEXTRA1..6`
//!    （中国移动=1/2，中国电信=3/4，中国联通=5/6，账号/密码**明文**提交）
//!    → HTTP 200 重渲染绑定页，内嵌 swal msg 含"绑定运营商账号信息成功"判成功
//!
//! dashboard 卡片协议（逆向于 2026-09-05 dashboard 页内嵌 JS + 实测响应结构，
//! 仅需登录会话 cookie，无需额外参数）：
//! 6. `GET /Self/dashboard/getOnlineList`  在线设备列表（对象数组：loginTime 字符串、
//!    ip、mac 12位hex、useTime 秒、downFlow/upFlow KB、hostName、terminalType
//!    "#PC" 带#前缀、sessionId 供注销）
//! 7. `GET /Self/dashboard/getLoginHistory` 近期上网记录（数组的数组：上线/注销时间
//!    epoch 毫秒、ip、mac、时长分、流量 M、计费方式 1时长/2流量/3包月、金额、主机名、
//!    终端类型）
//! 8. `GET /Self/dashboard/tooffline?sessionid=` 踢指定会话下线 → `{"success":bool}`
//!    （实测对不存在的 sessionid 也返回 true，服务端宽松处理）
//!
//! 上网记录账单页协议（逆向于 2026-09-06 /Self/bill/userOnlineLog 页内嵌
//! bootstrapTable 配置 + 会话内 fetch 实测）：
//! 9. `GET /Self/bill/getUserOnlineLog?startTime=YYYY-MM-DD&endTime=YYYY-MM-DD&`
//!    `pageNumber=1&pageSize=N`（GET，仅需登录会话；服务端分页，pageSize 大值有效）
//!    → `{"rows":[...],"summary":{...},"total":N}`：
//!    rows 行字段 loginTime/logoutTime（epoch **毫秒**）、time（分钟）、
//!    flow/internetUpFlow/internetDownFlow/chinanetUpFlow/chinanetDownFlow/costMoney
//!    （数值，页面 toFixed(2)；MB/元）、userIp/nasIp/nasPort、macAddress/userName 等
//!    （页面未展示）；summary 键大写下划线（INTERNETUPFLOW/INTERNETDOWNFLOW/
//!    CHINANETUPFLOW/CHINANETDOWNFLOW/FLOW/TIME/COSTMONEY/COU=记录数），与页面
//!    顶部"汇总数据"卡一致。原始 JSON 透传前端，展示格式化不做在后端

use std::net::IpAddr;

use lazy_static::lazy_static;
use regex::Regex;

/// 自助服务系统基地址（无锡学院部署，仅校园网内网可达）
pub const SELF_BASE_URL: &str = "http://10.1.80.200:8080/Self";

lazy_static! {
    static ref RE_CHECKCODE: Regex =
        Regex::new(r#"name="checkcode"[^>]*value="(\d+)""#).unwrap();
    static ref RE_CSRFTOKEN: Regex =
        Regex::new(r#"name="csrftoken"[^>]*value="([0-9a-fA-F-]{36})""#).unwrap();
    // 绑定页 FLDEXTRA 预填值（value 在 name 之前，中间可跨行；密码框 value 为明文）
    static ref RE_FLD_VALUE: Regex =
        Regex::new(r#"value="([^"]*)"\s+name="FLDEXTRA(\d)""#).unwrap();
    // swal 提示注入段：`})('消息');`（整页唯一一处，消息文本可能含 \n 等 JS 转义）
    static ref RE_SWAL_MSG: Regex = Regex::new(r#"\}\)\('((?:[^'\\]|\\.)*)'\);"#).unwrap();
}

/// 绑定请求参数（凭据仅内存传递，调用方不得落盘/写日志）
pub struct BindParams<'a> {
    /// 学号（自助服务系统登录账号，与 Portal 登录账号相同）
    pub account: &'a str,
    /// 自助服务系统登录密码（明文，协议内部 MD5 后提交；默认为身份证后 6 位）
    pub password: &'a str,
    /// 运营商后缀（与 Config.operator 同源：@cmcc / @telecom / @unicom）
    pub operator: &'a str,
    /// 运营商账号（用户办理套餐的手机号）
    pub phone: &'a str,
    /// 运营商账户密码（办理套餐时由运营商短信下发）
    pub sms_password: &'a str,
}

/// 运营商后缀 → 绑定表单 FLDEXTRA 账号/密码字段序号（1-based）
pub fn operator_fld_pair(operator: &str) -> Option<(usize, usize)> {
    match operator {
        "@cmcc" => Some((1, 2)),
        "@telecom" => Some((3, 4)),
        "@unicom" => Some((5, 6)),
        _ => None,
    }
}

/// 登录页 hidden checkcode 提取
pub fn extract_checkcode(html: &str) -> Option<String> {
    RE_CHECKCODE.captures(html)?.get(1).map(|m| m.as_str().to_string())
}

/// 绑定页 hidden csrftoken（UUID）提取
pub fn extract_csrftoken(html: &str) -> Option<String> {
    RE_CSRFTOKEN.captures(html)?.get(1).map(|m| m.as_str().to_string())
}

/// 绑定页 FLDEXTRA1..6 预填值（已绑定运营商的账号/密码明文回显；未绑定为空串）。
/// 绑定表单是**整体保存**：提交时必须带回其他运营商的预填原值，只发目标运营商
/// 字段、其余填空串会把已有绑定清掉（2026-09-05 实测缺陷）。
pub fn extract_fld_values(html: &str) -> [String; 6] {
    let mut values: [String; 6] = std::array::from_fn(|_| String::new());
    for cap in RE_FLD_VALUE.captures_iter(html) {
        let idx: usize = match cap[2].parse() {
            Ok(n) if (1..=6).contains(&n) => n,
            _ => continue,
        };
        values[idx - 1] = cap[1].to_string();
    }
    values
}

/// 页面内嵌 swal 提示文本提取（登录失败原因 / 绑定结果，整页唯一）
pub fn extract_swal_msg(html: &str) -> Option<String> {
    RE_SWAL_MSG.captures(html)?.get(1).map(|m| m.as_str().to_string())
}

/// 绑定结果判定：成功 msg 形如"中国移动账号:xxx,绑定运营商账号信息成功"
pub fn is_bind_success(msg: &str) -> bool {
    msg.contains("绑定运营商账号信息成功")
}

/// 标准小写 MD5 hex（自助服务系统登录密码提交格式）
pub fn md5_hex(input: &str) -> String {
    use md5::Digest;
    let mut hasher = md5::Md5::new();
    hasher.update(input.as_bytes());
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// 构建自助服务会话客户端：独立于 CLIENT_POOL（一次性会话，cookie_store 保持
/// JSESSIONID；禁用自动重定向以便按 Location 区分 verify 成功/失败）
fn build_session_client(local_addr: Option<IpAddr>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .connect_timeout(std::time::Duration::from_secs(3))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 CampusLogin");
    if let Some(ip) = local_addr {
        builder = builder.local_address(ip);
    }
    builder.build().map_err(|e| format!("创建自助服务HTTP客户端失败: {e}"))
}

/// 完整绑定流程：登录自助服务系统 → 取绑定表单 → 提交绑定。
/// 成功返回服务端提示文本；失败返回可展示给用户的原因（服务端原文或本地判断）。
pub async fn bind_operator(
    params: &BindParams<'_>,
    local_addr: Option<IpAddr>,
) -> Result<String, String> {
    let (acct_fld, pwd_fld) =
        operator_fld_pair(params.operator).ok_or_else(|| "请选择要绑定的运营商".to_string())?;
    let (client, op_html) =
        login_and_fetch_bind_page(params.account, params.password, local_addr).await?;
    let csrftoken = extract_csrftoken(&op_html)
        .ok_or_else(|| "绑定页解析失败（csrftoken 缺失）".to_string())?;
    let prefilled = extract_fld_values(&op_html);

    // 提交绑定（账号/密码明文，按运营商映射 FLDEXTRA 序号）。
    // 绑定表单整体保存：非目标运营商字段必须带回预填原值，填空串会清掉已有绑定
    let mut form: Vec<(String, String)> = Vec::with_capacity(7);
    form.push(("csrftoken".to_string(), csrftoken));
    for i in 1..=6usize {
        let value = if i == acct_fld {
            params.phone.to_string()
        } else if i == pwd_fld {
            params.sms_password.to_string()
        } else {
            prefilled[i - 1].clone()
        };
        form.push((format!("FLDEXTRA{i}"), value));
    }
    let bind_resp = client
        .post(format!("{}/service/bind-operator", SELF_BASE_URL))
        .header(reqwest::header::REFERER, format!("{}/service/operatorId", SELF_BASE_URL))
        .form(&form)
        .send().await
        .map_err(|e| format!("提交绑定请求失败: {e}"))?;
    if bind_resp.status().is_redirection() {
        return Err("登录会话失效，请重试".to_string());
    }
    let bind_html = bind_resp
        .error_for_status()
        .map_err(|e| format!("提交绑定请求失败: {e}"))?
        .text().await.map_err(|e| format!("读取绑定结果失败: {e}"))?;
    let msg = extract_swal_msg(&bind_html).unwrap_or_default();
    if is_bind_success(&msg) {
        Ok(msg)
    } else if msg.is_empty() {
        Err("绑定失败：服务端未返回结果，请稍后重试".to_string())
    } else {
        Err(msg)
    }
}

/// 已绑定的运营商凭据（供状态展示：手机号已掩码、密码只回是否设置）
pub struct OperatorBinding {
    /// 掩码后的运营商账号（手机号前三后二）
    pub masked_account: String,
    /// 运营商账户密码是否已设置
    pub password_set: bool,
}

/// 查询当前绑定状态：登录自助系统 → 解析绑定表单预填值。
/// 未绑定的运营商为 None；已绑定返回掩码账号（不返回任何明文凭据）。
pub async fn query_bind_status(
    account: &str,
    password: &str,
    local_addr: Option<IpAddr>,
) -> Result<[Option<OperatorBinding>; 3], String> {
    let (_client, op_html) = login_and_fetch_bind_page(account, password, local_addr).await?;
    let fld = extract_fld_values(&op_html);
    Ok([
        binding_from(&fld[0], &fld[1]),
        binding_from(&fld[2], &fld[3]),
        binding_from(&fld[4], &fld[5]),
    ])
}

/// 查看某运营商的明文凭据（手机号 + 运营商账户密码）。
/// 调用方必须先完成 Windows 本地身份验证（platform::identity::verify_identity）。
pub async fn reveal_credential(
    account: &str,
    password: &str,
    operator: &str,
    local_addr: Option<IpAddr>,
) -> Result<(String, String), String> {
    let (acct_fld, pwd_fld) =
        operator_fld_pair(operator).ok_or_else(|| "请选择要查看的运营商".to_string())?;
    let (_client, op_html) = login_and_fetch_bind_page(account, password, local_addr).await?;
    let fld = extract_fld_values(&op_html);
    let phone = fld[acct_fld - 1].clone();
    let sms_password = fld[pwd_fld - 1].clone();
    if phone.is_empty() && sms_password.is_empty() {
        return Err("该运营商尚未绑定".to_string());
    }
    Ok((phone, sms_password))
}

/// 账号隐私掩码：11 位手机号前 3 后 2（中间 6 位打码）；其他格式 ≥8 位前 2 后 2、
/// 5~7 位前 1 后 1；过短整体打码。明文凭据不出协议模块。
pub fn mask_account(account: &str) -> String {
    let chars: Vec<char> = account.chars().collect();
    let n = chars.len();
    if n == 0 {
        return String::new();
    }
    let head = |k: usize| chars[..k].iter().collect::<String>();
    let tail = |k: usize| chars[n - k..].iter().collect::<String>();
    if n >= 8 {
        format!("{}******{}", head(3), tail(2))
    } else if n > 4 {
        format!("{}****{}", head(1), tail(1))
    } else {
        "*".repeat(n)
    }
}

fn binding_from(acct: &str, pwd: &str) -> Option<OperatorBinding> {
    if acct.is_empty() {
        return None;
    }
    Some(OperatorBinding {
        masked_account: mask_account(acct),
        password_set: !pwd.is_empty(),
    })
}

/// 登录自助服务系统并返回会话客户端（绑定/dashboard 查询共用前 3 步）。
/// 客户端须保持 cookie 会话供后续请求使用。
async fn login_session(
    account: &str,
    password: &str,
    local_addr: Option<IpAddr>,
) -> Result<reqwest::Client, String> {
    let client = build_session_client(local_addr)?;

    // 1. 登录页取 checkcode
    let login_html = client
        .get(format!("{}/login/", SELF_BASE_URL))
        .send().await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("无法连接自助服务系统: {e}"))?
        .text().await.map_err(|e| format!("读取登录页失败: {e}"))?;
    let checkcode = extract_checkcode(&login_html)
        .ok_or_else(|| "自助服务登录页解析失败（checkcode 缺失）".to_string())?;

    // 2. 预热会话验证码状态（见模块注释，跳过则 verify 必失败）
    client
        .get(format!("{}/login/randomCode", SELF_BASE_URL))
        .query(&[("t", "1")])
        .send().await
        .map_err(|e| format!("预热验证码失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("预热验证码失败: {e}"))?;

    // 3. 登录（密码 MD5 后提交）
    let verify_resp = client
        .post(format!("{}/login/verify", SELF_BASE_URL))
        .form(&[
            ("account", account),
            ("password", &md5_hex(password)),
            ("checkcode", checkcode.as_str()),
            ("code", ""),
        ])
        .send().await
        .map_err(|e| format!("登录请求失败: {e}"))?;
    let is_login_ok = verify_resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(|loc| loc.contains("/Self/dashboard"))
        .unwrap_or(false);
    if !is_login_ok {
        // 失败：重新拉登录页提取服务端错误原文（如"账号或密码错误！"）
        let fail_html = client
            .get(format!("{}/login/", SELF_BASE_URL))
            .send().await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("登录失败（{e}）"))?
            .text().await
            .map_err(|e| format!("登录失败（读取错误信息失败: {e}）"))?;
        let msg = extract_swal_msg(&fail_html).unwrap_or_default();
        return Err(if msg.is_empty() { "登录失败，请检查学号与自助服务密码".to_string() } else { msg });
    }
    Ok(client)
}

/// 登录自助服务系统并返回绑定表单页 HTML（绑定/查询共用前 4 步）。
/// 返回 (会话客户端, 绑定页 HTML)——客户端须保持 cookie 会话供后续请求使用。
async fn login_and_fetch_bind_page(
    account: &str,
    password: &str,
    local_addr: Option<IpAddr>,
) -> Result<(reqwest::Client, String), String> {
    let client = login_session(account, password, local_addr).await?;

    // 4. 绑定表单页 HTML（302 = 登录会话失效）
    let op_resp = client
        .get(format!("{}/service/operatorId", SELF_BASE_URL))
        .send().await
        .map_err(|e| format!("打开绑定页失败: {e}"))?;
    if op_resp.status().is_redirection() {
        return Err("登录会话失效，请重试".to_string());
    }
    let op_html = op_resp
        .error_for_status()
        .map_err(|e| format!("打开绑定页失败: {e}"))?
        .text().await.map_err(|e| format!("读取绑定页失败: {e}"))?;
    Ok((client, op_html))
}

/// 请求 dashboard 查询接口并解析 JSON（getOnlineList / getLoginHistory 共用）。
/// 302 = 登录会话失效；非 JSON = 服务端异常页。
async fn fetch_dashboard_json(
    client: &reqwest::Client,
    path: &str,
    label: &str,
) -> Result<serde_json::Value, String> {
    let resp = client
        .get(format!("{}/dashboard/{path}", SELF_BASE_URL))
        .send().await
        .map_err(|e| format!("请求{label}失败: {e}"))?;
    if resp.status().is_redirection() {
        return Err("登录会话失效，请重试".to_string());
    }
    let text = resp
        .error_for_status()
        .map_err(|e| format!("请求{label}失败: {e}"))?
        .text().await.map_err(|e| format!("读取{label}失败: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{label}解析失败: {e}"))
}

/// 查询自助服务 dashboard 数据（在线信息 + 近期上网记录）。
/// 一次登录会话连拉两个接口，返回原始 JSON（数组结构），展示格式化由前端完成。
pub async fn query_dashboard(
    account: &str,
    password: &str,
    local_addr: Option<IpAddr>,
) -> Result<(serde_json::Value, serde_json::Value), String> {
    let client = login_session(account, password, local_addr).await?;
    let online = fetch_dashboard_json(&client, "getOnlineList", "在线信息").await?;
    let history = fetch_dashboard_json(&client, "getLoginHistory", "上网记录").await?;
    Ok((online, history))
}

/// 查询自助服务"上网记录"账单页数据（协议见模块注释第 9 条）。
/// 日期范围 YYYY-MM-DD（含端点）；一次请求大 pageSize 拉全（前端不做翻页），
/// 返回原始 `{ rows, summary, total }`。
pub async fn query_online_log(
    account: &str,
    password: &str,
    start_time: &str,
    end_time: &str,
    local_addr: Option<IpAddr>,
) -> Result<serde_json::Value, String> {
    let client = login_session(account, password, local_addr).await?;
    let resp = client
        .get(format!("{}/bill/getUserOnlineLog", SELF_BASE_URL))
        .query(&[
            ("startTime", start_time),
            ("endTime", end_time),
            ("pageNumber", "1"),
            ("pageSize", "500"),
        ])
        .send()
        .await
        .map_err(|e| format!("请求上网记录失败: {e}"))?;
    if resp.status().is_redirection() {
        return Err("登录会话失效，请重试".to_string());
    }
    let text = resp
        .error_for_status()
        .map_err(|e| format!("请求上网记录失败: {e}"))?
        .text()
        .await
        .map_err(|e| format!("读取上网记录失败: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("上网记录解析失败: {e}"))
}

/// tooffline 响应 success 判定（非 JSON / 缺字段按失败处理）
fn parse_offline_success(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("success").and_then(|s| s.as_bool()))
        .unwrap_or(false)
}

/// 注销指定上网会话（dashboard 在线信息卡片操作列，GET tooffline?sessionid=）。
pub async fn offline_session(
    account: &str,
    password: &str,
    session_id: &str,
    local_addr: Option<IpAddr>,
) -> Result<(), String> {
    let client = login_session(account, password, local_addr).await?;
    let resp = client
        .get(format!("{}/dashboard/tooffline", SELF_BASE_URL))
        .query(&[("sessionid", session_id)])
        .send().await
        .map_err(|e| format!("请求注销失败: {e}"))?;
    if resp.status().is_redirection() {
        return Err("登录会话失效，请重试".to_string());
    }
    let text = resp
        .error_for_status()
        .map_err(|e| format!("请求注销失败: {e}"))?
        .text().await.map_err(|e| format!("读取注销结果失败: {e}"))?;
    if parse_offline_success(&text) {
        Ok(())
    } else {
        Err("注销失败，请稍后重试".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_hex_standard_vectors() {
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(md5_hex("123456"), "e10adc3949ba59abbe56e057f20f883e");
    }

    #[test]
    fn extract_checkcode_from_login_page() {
        // 取自实测登录页 HTML（value 在 name 之后，跨行属性）
        let html = r#"<input type="hidden" name="checkcode" value="7151">"#;
        assert_eq!(extract_checkcode(html).as_deref(), Some("7151"));
        assert_eq!(extract_checkcode("no form here"), None);
    }

    #[test]
    fn extract_csrftoken_from_bind_page() {
        let html = r#"<input type="hidden" name="csrftoken" value="e5d2e4bd-2b8f-478b-b65e-11189dd90399"/>"#;
        assert_eq!(
            extract_csrftoken(html).as_deref(),
            Some("e5d2e4bd-2b8f-478b-b65e-11189dd90399")
        );
        assert_eq!(extract_csrftoken("<div>empty</div>"), None);
    }

    #[test]
    fn extract_swal_msg_success_and_empty() {
        // 成功样例（实测响应，msg 内含字面 \n 转义）
        let html = r#"    })('中国移动账号:19720520238,绑定运营商账号信息成功 \n');"#;
        assert_eq!(
            extract_swal_msg(html).as_deref(),
            Some(r#"中国移动账号:19720520238,绑定运营商账号信息成功 \n"#)
        );
        // 空提示（正常渲染页）
        assert_eq!(extract_swal_msg("    })('');"), Some("".to_string()));
        // 登录失败样例
        assert_eq!(
            extract_swal_msg(r#"        })('账号或密码错误！');"#).as_deref(),
            Some("账号或密码错误！")
        );
        assert_eq!(extract_swal_msg("nothing"), None);
    }

    #[test]
    fn bind_success_judgement() {
        assert!(is_bind_success("中国移动账号:19720520238,绑定运营商账号信息成功 \\n"));
        assert!(!is_bind_success("密码错误，请重试"));
        assert!(!is_bind_success(""));
    }

    #[test]
    fn mask_account_phone_front3_back2() {
        // 11 位手机号：前 3 后 2，中间 6 位打码
        assert_eq!(mask_account("19720520238"), "197******38");
        assert_eq!(mask_account("13800000000"), "138******00");
        assert_eq!(mask_account(""), "");
        assert_eq!(mask_account("abc12345"), "abc******45");
        // 5~7 位：前 1 后 1
        assert_eq!(mask_account("12345"), "1****5");
        // 过短：整体打码
        assert_eq!(mask_account("123"), "***");
    }

    #[test]
    fn operator_fld_mapping() {
        assert_eq!(operator_fld_pair("@cmcc"), Some((1, 2)));
        assert_eq!(operator_fld_pair("@telecom"), Some((3, 4)));
        assert_eq!(operator_fld_pair("@unicom"), Some((5, 6)));
        assert_eq!(operator_fld_pair("@unknown"), None);
        assert_eq!(operator_fld_pair(""), None);
    }

    #[test]
    fn offline_success_parsing() {
        // 实测响应格式 {"success":bool}；非 JSON / 缺字段 / 非布尔按失败处理
        assert!(parse_offline_success(r#"{"success":true}"#));
        assert!(!parse_offline_success(r#"{"success":false}"#));
        assert!(!parse_offline_success(r#"{"result":1}"#));
        assert!(!parse_offline_success("<html>error</html>"));
        assert!(!parse_offline_success(""));
    }

    #[test]
    fn extract_fld_values_prefilled_kept_for_other_operators() {
        // 模拟实测绑定页格式：value 属性在 name 之前且跨行；移动预填、电信/联通为空
        let html = r#"<input type="text" class="form-control" value="12345678901"
                                                           name="FLDEXTRA1"
                                                           maxlength="20">
                                                <input type="password" class="form-control" value="abc123"
                                                           name="FLDEXTRA2" maxlength="20">
                                                <input type="text" class="form-control" value=""
                                                           name="FLDEXTRA3"
                                                           maxlength="20">
                                                <input type="password" class="form-control" value=""
                                                           name="FLDEXTRA4" maxlength="20">
                                                <input type="text" class="form-control" value=""
                                                           name="FLDEXTRA5"
                                                           maxlength="20">
                                                <input type="password" class="form-control" value=""
                                                           name="FLDEXTRA6" maxlength="20">"#;
        let values = extract_fld_values(html);
        assert_eq!(values[0], "12345678901");
        assert_eq!(values[1], "abc123");
        assert!(values[2].is_empty() && values[3].is_empty() && values[4].is_empty() && values[5].is_empty());

        // 绑定联通（5/6）时，移动 1/2 必须带回预填值而非空串
        let (acct_fld, pwd_fld) = operator_fld_pair("@unicom").unwrap();
        let mut form = vec![("csrftoken".to_string(), "x".to_string())];
        for i in 1..=6usize {
            let value = if i == acct_fld {
                "13900000000".to_string()
            } else if i == pwd_fld {
                "sms123".to_string()
            } else {
                values[i - 1].clone()
            };
            form.push((format!("FLDEXTRA{i}"), value));
        }
        assert_eq!(form[1], ("FLDEXTRA1".to_string(), "12345678901".to_string()));
        assert_eq!(form[2], ("FLDEXTRA2".to_string(), "abc123".to_string()));
        assert_eq!(form[5], ("FLDEXTRA5".to_string(), "13900000000".to_string()));
        assert_eq!(form[6], ("FLDEXTRA6".to_string(), "sms123".to_string()));
    }
}
