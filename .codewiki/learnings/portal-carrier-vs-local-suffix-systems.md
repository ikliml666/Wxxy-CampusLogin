---
title: "Portal 服务类型（carrier）与本地运营商后缀是两套机制"
type: learning
source_files:
  - tauri-app/src-tauri/src/self_service/mod.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/config/validate.rs
tags: [教训, 协议逆向, 运营商, eportal, 自助服务]
---

## 现象

2026-09-19 按用户要求对校园网认证/自助体系做外部逆向（VMware 虚拟机 10.2.118.248 桥接 + guestcontrol 抓包，页面与 JS 也可从主机直接抓），发现 Portal 认证页（`http://10.1.99.100/a79.htm`）内嵌的"服务类型"（carrier）配置与本项目硬编码的运营商后缀不一致，一度怀疑项目后缀体系失效。

## 服务端事实（实测）

1. **eportal 页面按请求源 IP 判定在线状态并差异下发**：在线 IP 拿到注销页（pc_1）且内嵌 carrier JSON（校园用户''/校园电信@dx/校园联通@lt/校园其他''，defaultID=1）；未在线 IP 拿到的登录页当时**不含** carrier 段（aolno 序号也不同，13445 vs 13395，疑按策略分组）。
2. **前端拼接逻辑**（a40.js）：登录页渲染 `select[name=ISP_select]` 或 `div[name=ISP_radio]`（value=-1 占位"请选择运营商"）；提交时 `user_account = DDDDD + tempAccountSuffix`，suffix 来自选中项 value，未选回落服务端 `account_suffix`（loadConfig 下发，当前为 `''`，另有 `account_prefix`）。
3. **后缀双体系（服务端行为差异）**：对不存在账号试登，空后缀/`@telecom`/`@unicom`/`@cmcc` 一律返回中文"账号不存在"（本地账号体系，同一处理流）；`@dx`/`@lt` 返回英文 "Authentication fail"（另一条认证路径，疑转发运营商）。**项目使用的后缀体系真实有效，与页面下发的 @dx/@lt 是两套并行机制，互不影响**。
4. **自助服务（Self，10.1.80.200:8080）**：登录（MD5+checkcode）、dashboard、账单接口均无运营商参数；运营商区分只存在于绑定页（FLDEXTRA1..6 = 移动/电信/联通三组手机号+短信密码，无"无锡学院"项），与本项目 `self_service/mod.rs` 的 2026-09-05 逆向记录一致。

## 教训

1. 排查运营商相关问题时先分清**两套机制**：认证后缀（@telecom 系，本地账号体系，项目在用）与页面"服务类型"（carrier/@dx/@lt，页面展示层配置，按策略动态下发且当前与本项目无关）。不要用页面 carrier 配置去否定/修改项目后缀。
2. eportal 是"框架 JS（a41）+ 压缩功能 JS（a40）+ 页面模板（extern/<方案>/<页面>/pc_N.js、mobile_3N.js）"三层结构；抓 JS 必须 `--compressed`（gzip）；含 GBK 的 JS/HTML 会让 git-bash grep 静默按二进制跳过，用 python 以 gb18030 读取。
3. eportal 的在线判定以**请求源 IP** 为准，URL 的 `wlanuserip` 参数只是记录；主机与虚拟机（不同 IP/策略）拿到的页面可能不同，跨机抓包结论要标注视角。
4. 未登录的自助服务面全部 302，公开页只有 login/help/forgetPwd，且 help 为空——Self 侧新协议信息只能靠登录会话获取。

## Connections

[[desktop-account-selfservice]]、[[logout-radius-first]]、[[portal-port-semantics]]、[[learnings/binding-operator-select-excludes-campus]]
