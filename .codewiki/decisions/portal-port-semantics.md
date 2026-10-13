---
title: Portal 端口语义与协议交互决策
type: decision
source_files:
  - tauri-app/src-tauri/src/auth/portal.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
tags: [决策, portal, 协议, 端口, 安全]
---

## 背景

同一 Portal 服务器（Dr.COM/ePortal）上，`:80` 网关页与 `:801` 管理前端（SPA）承载完全不同的功能与状态特征。历史缺陷（v2.2.x）曾强制页面探测使用 `:801`，导致在已在线状态下因 SPA 始终渲染登录表单而无法识别在线状态，退化为 `need_manual_check`。此决策记录当前代码中两端口的严格分工、实现机制及安全约束。

## 决策

**Portal 页面探测用 `:80`（配置原始地址），协议请求（登录/注销）强制 `:801`。**

- **`:80` 网关页**：GBK 编码的 HTML 页面，内嵌可匹配的登录态特征（如 `Dr.COMWebLoginID_*`、`uid='...'`、`v4ip='...'`），用于 `check_portal_page` 判定在线状态。
- **`:801` 管理前端**：ePortal SPA 应用，已在线状态下仍渲染登录页，**无状态特征**；但提供 `/eportal/portal/login` 和 `/eportal/portal/logout` 等 API 接口。

## 理由

1. **状态判定的可靠性**：`:80` 页面包含明确的在线/离线指示器（见 `portal.rs:30-35` 常量定义）。`:801` SPA 无法提供此类特征，探测必然失败。
2. **协议接口的可用性**：登录/注销 API 仅在 `:801` 路径下可用。`ensure_portal_port` 函数（`portal.rs:68-79`）确保所有协议请求 URL 均对齐 `:801` 端口。
3. **安全约束**：状态探测是只读操作，绝不允许携带用户密码。当页面检测无法判断时（`Unknown`），系统返回 `need_manual_check` 状态，由用户手动确认，避免静默发送凭据触发账号锁定或误报（`portal.rs:138-155`）。

## 实现细节

### 端口处理逻辑

`ensure_portal_port` (`portal.rs:68-79`) 负责规范化协议请求 URL：
- 解析输入 URL，若未显式指定端口，则追加 `:801`。
- 正确处理带路径的 URL（避免将端口拼接到路径后产生非法 URL）。
- 示例：`http://10.0.0.1` → `http://10.0.0.1:801`；`http://10.0.0.1:8080` → `http://10.0.0.1:8080`（保留已有端口）。

### 页面探测流程

`check_portal_full` (`portal.rs:115-135`) 执行完整状态检查：
1. 构建页面探测 URL：使用配置的原始地址（默认 `:80`），拼接尾部 `/`（`portal.rs:206`）。
2. 发起 HTTP GET 请求，超时 `3s`（`portal_config::REQUEST_TIMEOUT`）。
3. 读取响应体（上限 1MB，按 Content-Type charset 解码，支持 GBK）。
4. `analyze_portal_page_content` (`portal.rs:247-267`) 匹配特征字符串：
   - **已在线**：`Dr.COMWebLoginID_1` 或 `<title>注销页</title>`。
   - **未登录**：`Dr.COMWebLoginID_0`、`Dr.COMWebLoginID_2` 或 `<title>登录页</title>`。
   - **会话信息**：`uid='xxx'`（非空）、`v4ip='xxx'`（非 `0.` 且非空）+ `oltime=xxx`（非 0）。
   - **未知**：无匹配特征，返回 `PageCheckResult::Unknown`。

### Android 旁路优先

在 `check_portal_page` (`portal.rs:216-229`) 和 `do_login_request` (`protocol.rs:108-118`) 中，Android 平台优先尝试创建旁路 HTTP 客户端（`create_bypass_http_client`），通过 `SO_BINDTODEVICE` 直连物理网卡。旁路不可用时回退到普通 reqwest 客户端。

### 错误信息与凭据脱敏

`redact_credentials` (`portal.rs:88-100`) 防止凭据泄漏：
- 替换完整请求 URL 为 `{base_url}?***`。
- 替换 URL 编码后的密码与明文密码为 `***`。
- 在 `do_login_request` (`protocol.rs:133-137`) 的错误处理中调用，确保 reqwest 异常中的完整 URL 被脱敏。

## 备选方案

**合并两端口处理（已回退）**：v2.2.x 曾强制页面探测使用 `:801`，导致状态判定始终为 `Unknown`。2026-09-03 经浏览器实测 80/801 渲染结果后，回退为原始地址探测（`portal.rs:206-213` 注释记录了此次修复）。

## 影响与约束

1. **配置归一化陷阱**：`config::validate::normalize_portal_url` (`validate.rs:81-85`) 仅归一化字面值 `"http://10.1.99.100:801"` 为空端口形式。若用户自定义其他带 `:801` 的地址（如 `http://10.2.1.3:801`），页面探测会打到 EPortal SPA 前端，必然返回 `need_manual_check`。
2. **特征字符串硬编码**：所有页面特征（`portal.rs:30-35`）均为 Dr.COM 文案硬编码。若学校 Portal 改版，所有探测将整体退化为 `Unknown`，需同步更新常量。
3. **NAT 环境处理**：`do_logout_request` (`protocol.rs:186-191`) 检测 NAT 内网 IP（`is_nat_private_ip`, `portal.rs:157-178`），在 NAT 环境下不发送 `wlan_user_ip` 参数，与 `portal.rs` 行为一致。
4. **注销顺序优化**：`do_logout_request` (`protocol.rs:198-260`) 改为先执行 Radius 注销（最多 2 轮），成功后补一次 MAC 解绑（best-effort）。旧实现"先 MAC 解绑再 Radius 注销"会诱发僵尸状态（`protocol.rs:198-207` 注释详述了社区 POC 利用 unbind 断网的风险）。

## Connections

[[portal-page-detection-mechanism]]、[[logout-radius-first-strategy]]、[[credential-redaction-security]]