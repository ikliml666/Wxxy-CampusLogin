---
title: vpn（深澜 WebVPN 协议核心）
type: module
source_files:
  - tauri-app/src-tauri/src/vpn/mod.rs
  - tauri-app/src-tauri/src/vpn/crypto.rs
  - tauri-app/src-tauri/src/vpn/wrap.rs
  - tauri-app/src-tauri/src/vpn/cas.rs
  - tauri-app/src-tauri/src/vpn/session.rs
  - tauri-app/src-tauri/src/vpn/route.rs
  - tauri-app/src-tauri/src/vpn/rsa.rs
  - tauri-app/src-tauri/src/vpn/captcha.rs
  - tauri-app/src-tauri/src/vpn/jar.rs
  - tauri-app/src-tauri/src/vpn/cache.rs
  - tauri-app/src-tauri/src/vpn/error.rs
tags: [vpn, webvpn, srun, cas, rsa, captcha, 协议, 双端]
---

## Overview

`tauri-app/src-tauri/src/vpn/` 是深澜（Srun）WebVPN 协议核心，2026-10-25 从
Wxxy-CampusHub 移植而来。它把「本校自助服务经 WebVPN 网关校外访问」所需的全部协议
逻辑收敛在一个纯 Rust 模块里：无 Tauri 依赖、无 Windows 专属 API，依赖仅
`aes` / `cfb-mode` / `hex` / `num-bigint-dig` / `image` / `serde` / `reqwest`，因此
Windows 桌面端与 Android 端（经 Cargo path 依赖自动继承，禁止复制到 `android/`）均可编译。

模块分 11 个子模块：

- `crypto`：AES-128-CFB128 host 加解密（网关包装 URL 的核心算子）。
- `wrap`：校内 URL → WebVPN 网关包装 URL（含幂等与端口形态）。
- `cas`：CAS 统一身份认证（kaptcha 自动解题、RSA 账密登录拿 TGT、TGT 换 ST）。
- `session`：WebVPN 会话（CAS TGT 静默登录网关 / 探活 / 业务 client 句柄）。
- `route`：网络归属 × 目标地址 × 会话状态 → 直连/包装/需登录 决策表。
- `rsa`：CAS 密码 / token 的 textbook RSA（无 padding）加密。
- `captcha`：算术验证码自动识别（强度图 → 垂直投影切分 → bbox 锚定 → NCC 最近邻）。
- `jar`：记录型 Cookie Jar（委托 reqwest 内置 Jar，同时记录 `(名, 值)` 供会话检测）。
- `cache`：进程内 WebVPN 会话缓存（惰性复用 / 静默重登）。
- `error`：CAS 协议层错误类型与中文映射。
- `mod`：模块门面 + 协议层统一错误 `WebVpnError`。

核心算法（逆向自 `webvpn.cwxu.edu.cn` 网关，活样本见 Wxxy-CampusHub
`docs/cas-recon/REPORT.md`）：

包装式 `{gateway}/{scheme}/hex(IV)+hex(AES-128-CFB128(host))` + 原 path?query

- `key = IV = 16 字节 ASCII 常量`（`crypto::KEY`，模块内单一来源）；
- AES-128-CFB128 流式，密文长度恒等于明文字节数，无 padding；
- 输出前缀 `hex(IV)` 由 key 派生计算，调用方禁止硬编码该 hex 串；
- scheme 段跟随目标（`https/` / `http/`），非默认端口写作 `{scheme}-{port}`；
- 输入 host 已是网关域时幂等原样返回（`wrap::wrap_url`）。

## Key Components

### mod.rs（模块门面与协议错误）

- `pub enum WebVpnError`（第 46 行）：协议层统一错误，变体：
  - `InvalidUrl(url::ParseError)`（第 48 行，由 URL 解析错误 `[#from]` 透传）；
  - `UnsupportedScheme(String)`（第 50 行，仅 http/https 可包装）；
  - `MissingHost`（第 52 行，URL 无 host）；
  - `InvalidHex(hex::FromHexError)`（第 54 行，解 hex 失败）；
  - `InvalidIv`（第 58 行，IV 段不符，**仅 `decrypt_host` 构造**，标记 `#[allow(dead_code)]`）；
  - `InvalidUtf8(std::string::FromUtf8Error)`（第 60 行）；
  - `Network(reqwest::Error)`（第 63 行，会话层网络错误）；
  - `Sso(CasAuthError)`（第 66 行，CAS 换票失败透明透传）。
- 模块声明 `pub mod …`（第 27–36 行）声明全部 10 个子模块。
- 再导出（第 38–41 行）：`error::CasAuthError`、`route::{route, NetZone, RouteDecision}`、
  `session::WebVpnSession`、`wrap::{wrap_url, GATEWAY}`。

### crypto.rs（AES-128-CFB128 host 加解密）

- `const KEY: [u8; 16]`（第 9 行）：固定 128 位密钥 `b"wrdvpnisthebest!"`；IV 由本密钥
  恒等派生（即 `hex(KEY)`），是 `hex(IV)` 前缀的唯一来源。
- `type Aes128CfbEnc = Encryptor<aes::Aes128>`（第 11 行）：加密类型别名。
- `type Aes128CfbDec = Decryptor<aes::Aes128>`（第 15 行）：解密类型别名，标记
  `#[allow(dead_code)]`——本端只做 URL 包装加密，解密无生产调用方，保留为对称参考实现。
- `pub fn encrypt_host(host: &str) -> String`（第 20 行）：加密 host，返回
  `hex(IV)+hex(密文)`；CFB 流式无 padding，密文长度恒等于明文字节数。
- `pub fn decrypt_host(hex_text: &str) -> Result<String, WebVpnError>`（第 31 行）：解密
  `hex(IV)+hex(密文)` 整段；IV 段须等于固定网关 IV，否则 `InvalidIv`；标记
  `#[allow(dead_code)]`，由测试锚定（golden/roundtrip/坏 IV 拒绝）。
- 测试（第 41–113 行）：`golden_vectors`（第 49 行，6 条向量含本校活样本与社区通行向量）、
  `roundtrip`（第 83 行）、`cfb_stream_length_no_padding`（第 90 行）、
  `decrypt_rejects_bad_iv`（第 97 行）、`decrypt_rejects_bad_hex`（第 107 行）。

### wrap.rs（网关 URL 拼装）

- `pub const GATEWAY: &str`（第 8 行）：本校网关基址 `https://webvpn.cwxu.edu.cn`；
  会话与路由统一引用，禁止别处复制。
- `pub fn wrap_url(raw: &str, gateway: &str) -> Result<String, WebVpnError>`（第 16 行）：
  把任意 http/https URL 包装为网关 URL。逻辑：解析网关与 raw → 校验 scheme（非
  http/https 报 `UnsupportedScheme`）→ 取 host（无 host 报 `MissingHost`）→ 网关 host
  幂等原样返回 → 非默认端口写作 `{scheme}-{port}`（默认 443/80 省略）→ 保留
  path/query/fragment → 拼 `{gateway}/{seg}/{encrypt_host(host)}{tail}`。
- 测试（第 55–131 行）：`wrap_path_and_query`（第 64 行）、`wrap_port_forms`（第 76 行）、
  `wrap_keeps_fragment`（第 95 行）、`wrap_root_path_gets_trailing_slash`（第 103 行）、
  `wrap_is_idempotent_for_gateway_host`（第 111 行）、`wrap_rejects_ftp`（第 117 行）、
  `wrap_rejects_garbage`（第 125 行）。

### cas.rs（CAS 统一身份认证）

- `pub const CAS_BASE: &str`（第 24 行）：`https://wxcas.cwxu.edu.cn/lyuapServer`。
- `pub const PORTAL_SERVICE: &str`（第 26 行）：门户 service `https://my.cwxu.edu.cn/shiro-cas`。
- `pub const WEBVPN_SERVICE: &str`（第 28 行）：WebVPN 在 CAS 端注册的 service，也是
  `session` 层 `sso_ticket` 换票的 service。
- `const USER_AGENT: &str`（第 31 行）：真实 Chrome UA（深澜按 UA 出响应）。
- `pub struct CasClient`（第 36 行，`#[derive(Clone)]`）：字段 `http: reqwest::Client`
  （第 37 行）；构造时挂载 `Arc<RecordingJar>` 作 cookie_provider（`new` 第 87 行）。方法：
  - `new()`（第 87 行）→ `Result<Self, CasAuthError>`；
  - `kaptcha()`（第 97 行）→ `GET {CAS_BASE}/kaptcha`，返回 `CaptchaInfo`；
  - `solved_captcha(templates, max_attempts)`（第 110 行）→ `Option<(uid, code)>`，
    多张识别不达标换一张、绝不硬猜；
  - `login_with_captcha(username, password_rsa_hex, templates, max_attempts)`（第 137 行）
    → `Result<CasLoginOk, CasLoginError>`，识别穷尽或 CODEFALSE 换一张重试；
  - `login(username, password_rsa_hex, captcha_uid, captcha_code)`（第 169 行）→
    `POST {CAS_BASE}/v1/tickets`，头 `token = cas_token_header(毫秒)`；
  - `sso_ticket(tgt, service)`（第 218 行）→ `POST {CAS_BASE}/v1/tickets/{tgt}` 换 ST，
    响应纯文本以 `ST-` 开头校验。
- `pub struct CasLoginOk`（第 42 行，`Debug, Clone`）：字段 `tgt: String`（第 43 行）、
  `ticket: String`（第 47 行，标记 `#[allow(dead_code)]`，当前流程仅消费 tgt）。
- `pub struct CaptchaInfo`（第 52 行）：字段 `uid: String`（第 53 行）、
  `content: String`（第 55 行，`data:image/png;base64,…`）。
- `struct KaptchaResp`（第 59 行，`Deserialize`）：字段 `_kaptcha_type`、`uid`、`content`。
- `pub enum CasLoginError`（第 68 行，`Debug, Clone, PartialEq, Eq`）：变体
  `WrongUserOrPwd`（第 70 行，NOUSER）、`WrongCaptcha`（第 72 行，CODEFALSE）、
  `CaptchaUnsolvable(usize)`（第 75 行）、`UserLocked`（第 77 行，USERLOCK）、
  `NeedTwoVerify(String)`（第 79 行，TWOVERIFY）、`Unknown(String)`（第 81 行）、
  `Network(String)`（第 83 行）。
- `pub fn build_login_body(username, password_rsa_hex, service, captcha_uid, captcha_code)`（第 249 行）：
  构造 7 字段 `application/x-www-form-urlencoded` 请求体（username/password/service/
  loginType/id/code/otpcode），纯函数供离线单测。
- `fn form_encode(s)`（第 273 行）：form 值编码（字母数字与 `*-._` 保留、空格→`+`、其余 `%XX`）。
- `static TEMPLATES: OnceLock<KaptchaTemplates>`（第 290 行）+ `pub fn kaptcha_templates()`（第 291 行）：
  进程内单例，约 330KB JSON 一次解析。
- `pub fn parse_kaptcha(body)`（第 296 行）/ `pub fn parse_login_response(body)`（第 309 行）：
  响应解析纯函数；后者兼容顶层 `{tgt, ticket}`、data 包裹、data 纯字符串三种形态，失败走
  `map_error_code`。
- `fn map_error_code(code, data_raw)`（第 343 行）：16 码全集映射，NOUSER/CODEFALSE/
  USERLOCK/TWOVERIFY 专属，其余 12 码归 `Unknown`。
- 测试（第 353–417 行）：`login_body_seven_fields_and_form_encoding`（第 360 行）、
  `login_response_top_level_ok`（第 373 行）、`login_response_data_wrapped_variants`
  （第 381 行）、`login_error_codes_map`（第 391 行）。

### session.rs（WebVPN 会话层）

- `pub const COOKIE_TICKET: &str`（第 41 行）：深澜 ticket cookie 名
  `wengine_vpn_ticketwebvpn_cwxu_edu_cn`，标记 `#[allow(dead_code)]`——登录态判定走
  302 Location，不读 cookie 名。
- `const MAX_HOPS: usize = 10`（第 44 行）：302 链手动跟随上限。
- `const USER_AGENT: &str`（第 47 行）：与 cas 同款 Chrome UA。
- `pub struct WebVpnSession`（第 54 行，`#[derive(Clone)]`）：字段 `http_manual: reqwest::Client`
  （第 56 行，手动重定向 `Policy::none()`，挂载 `Arc<RecordingJar>`）。方法：
  - `new()`（第 61 行）→ `Result<Self, WebVpnError>`；
  - `login(cas, tgt)`（第 83 行）→ 静默登录网关：GET 网关 `/` 种初始 cookie →
    `sso_ticket` 换 ST → `follow_wrap` 手动跟随 302 链；
  - `is_alive()`（第 102 行）→ 探活：GET 网关 `/`，按 `is_login_redirect` 判定；
  - `wrapped_client_manual()`（第 120 行）→ 业务句柄（手动重定向，与登录共享 jar）；
  - `follow(start, transform)`（第 132 行，私有）→ 逐跳 transform 跟随 302 链；
  - `follow_wrap(start)`（第 167 行）→ 用 `wrap_url` 作 transform 的公开跟随入口。
- `pub fn login_redirect_url(service, ticket)`（第 177 行）：纯函数，拼
  `{service}&ticket=<ST>`（`WEBVPN_SERVICE` 自带 `?cas_login=true`，故用 `&` 追加）。
- `pub fn is_login_redirect(status, location)`（第 188 行）：纯函数，3xx 且 Location 含
  `/login` → 未登录。
- 测试（第 192–337 行）：含 `stub_sequence` 桩服务（第 262 行）、`http_302`（第 286 行）、
  `http_200`（第 290 行）、`follow_applies_transform_per_hop_with_relative_locations`
  （第 296 行）、`follow_stops_at_first_non_redirect`（第 330 行）等。
- 会话持久化取舍（第 13–26 行）：选「不落盘、内存持有、失效重登」，不扩展 `RecordingJar`
  多域恢复。

### route.rs（路由决策表，纯函数无 IO）

- `pub enum NetZone`（第 33 行）：`Campus`（第 35 行）/ `OffCampus`（第 37 行）/
  `Unknown`（第 39 行）。
- `pub enum RouteDecision`（第 44 行）：`Direct`（第 46 行）、`Wrapped(String)`（第 48 行，
  值为包装后 URL）、`NeedLogin`（第 50 行）、`Unreachable(String)`（第 53 行，标记
  `#[allow(dead_code)]` 预留位，`route` 不产生）。
- `const INTERNAL_HOSTS: &[&str]`（第 59 行）：`&["10.1.80.200"]`（显式钉住自助服务 host）。
- `fn is_internal_target(raw_url)`（第 65 行）：内网判据——10/8 IP 字面量，或命中
  网关域/已知内网 host 表。
- `pub fn route(raw_url, zone, has_vpn_session)`（第 87 行）：决策表
  Campus/Unknown→Direct；OffCampus×公网→Direct；OffCampus×内网×有会话→Wrapped；
  OffCampus×内网×无会话→NeedLogin；包装失败→保守 Direct。
- 测试（第 108–250 行）：`self_service_host_wraps_offcampus`（第 121 行）、
  `campus_always_direct`（第 137 行）、`unknown_direct_by_design`（第 147 行）、
  `offcampus_internal_with_session_wraps`（第 156 行）、
  `offcampus_internal_without_session_needs_login`（第 170 行）、
  `offcampus_public_domain_always_direct`（第 181 行）、
  `wrapped_url_routes_back_wrapped_verbatim`（第 195 行）、
  `internal_target_judgement`（第 207 行）、
  `offcampus_internal_bad_scheme_falls_back_to_direct`（第 226 行）、
  `unreachable_is_never_produced_by_route`（第 237 行）。

### rsa.rs（CAS textbook RSA 加密）

- `pub const CAS_RSA_N_HEX: &str`（第 16 行）：CAS 线上 modulus（258 hex 含前导 00）。
- `const RSA_E: u32 = 0x010001`（第 19 行）：公钥指数 65537。
- `const CHUNK_SIZE: usize = 126`（第 22 行）：组块大小。
- `pub fn rsa_encrypt_hex(plaintext)`（第 28 行）：无 padding RSA 加密，块尾补 0x00 至 126
  字节后反转成大端组整数，`c = m^65537 mod n`，输出小写 hex 不补前导零；**非 ASCII 明文
  显式拒绝**（JS `charCodeAt` 语义不等价，计划 P2-11 修正）。
- `pub fn cas_token_header(now_ms)`（第 52 行）：`RSA("lyasp"+毫秒)` 求头 `token` 值。
- `fn parse_modulus()`（第 57 行）：解析固化 modulus。

### captcha.rs（算术验证码自动识别）

- `pub const CANVAS_H: usize = 16`（第 22 行）/ `pub const CANVAS_W: usize = 14`（第 23 行）：
  字符画布尺寸；`const CANVAS_LEN`（第 24 行）= `CANVAS_H*CANVAS_W`。
- `const SEG_MIN: f32 = 0.12`（第 27 行）：切分强度阈值（相对本图峰值）。
- `const NCC_MIN: f32 = 0.88`（第 29 行）/ `const NCC_GAP: f32 = 0.03`（第 30 行）：
  NCC 接受阈值与平局间隙（样本标定类内 min 0.931 / 跨类 max 0.839）。
- `pub struct TemplateEntry`（第 34 行，`Debug, Clone, Serialize, Deserialize`）：字段
  `ch: char`（第 36 行）、`grid: Vec<u8>`（第 37 行，16×14 强度画布）。
- `pub struct TemplateFile`（第 42 行，`Default`）：字段 `templates: Vec<TemplateEntry>`
  （第 44 行）。
- `pub struct KaptchaTemplates`（第 49 行）：字段 `file: TemplateFile`（第 50 行），由编译期
  `include_str!("../../templates/kaptcha-templates.json")`（第 56 行）内嵌。方法：
  `load()`（第 54 行，运行时单例）、`from_json_str(json)`（第 63 行，
  `#[allow(dead_code)]` 仅集成测试锚定）、`templates()`（第 70 行）。
- `pub fn solve(png_bytes, t)`（第 77 行）→ `Option<String>`：PNG → 答案。
- `pub fn png_to_patches(png_bytes)`（第 86 行）→ `Option<Vec<Vec<u8>>>`：PNG → 前 3 片
  16×14 画布；片数 ≠4 返回 None（字符断裂/粘连，片序不可信）。
- `pub fn solve_patches(patches, t)`（第 103 行）：片序 `[d1, op, d2]` → 求值字符串。
- `fn is_digit`（第 119 行）/ `fn is_op`（第 123 行）：分类过滤。
- `pub fn class_scores(patch, t, filter)`（第 128 行）：每类最佳 NCC 降序。
- `fn classify(patch, t, filter)`（第 148 行）：NCC 最大者，阈值/平局双判 None。
- `fn shifted_ncc`（第 164 行）：±1px 位移补偿后最大 NCC。
- `fn ncc(a, b)`（第 185 行）：归一化互相关（长度不等/常量向量 → -1.0）。
- `fn fg_intensity(rgb)`（第 208 行）：颜色不变强度图 `dist = 765 - Σrgb`，除以峰值归一。
- `fn vertical_segments(nm, w, h)`（第 221 行）：垂直投影切分。
- `fn patch_at(nm, w, h, x0, x1)`（第 243 行）：片段 → 左上角锚定 16×14 画布。
- 编译期内嵌资产：`tauri-app/src-tauri/templates/kaptcha-templates.json`
  （`captcha.rs:56` 经 `include_str!` 引入，与 Wxxy-CampusHub 生成器逐字节一致）。

### jar.rs（记录型 Cookie Jar）

- `const RESTORE_URL: &str = "https://my.cwxu.edu.cn/"`（第 20 行）：restore 回填目标域，
  标记 `#[allow(dead_code)]`（预留冻结接口）。
- `pub struct RecordingJar`（第 24 行）：字段 `inner: reqwest::cookie::Jar`（第 25 行）、
  `recorded: Mutex<Vec<(String, String)>>`（第 26 行）。
- `impl Default`（第 29 行）/ `impl RecordingJar`：方法 `new()`（第 36 行）、
  `snapshot()`（第 45 行，`#[allow(dead_code)]`）、`restore(cookies)`（第 52 行，
  `#[allow(dead_code)]`）、`has(name)`（第 72 行，`#[allow(dead_code)]`）、
  `locked()`（第 76 行，私有）。
- `impl CookieStore for RecordingJar`：实现 `set_cookies`（第 82 行，提取首个 `;` 前
  `(名,值)` 记入 `recorded` 并委托内置 Jar）与 `cookies`（第 108 行，转发内置 Jar）。

### cache.rs（进程内会话缓存）

- `lazy_static! VPN_SESSION: RwLock<Option<Arc<WebVpnSession>>>`（第 14–16 行）：进程内单例。
- `pub fn cached()`（第 19 行）→ `Option<Arc<WebVpnSession>>`（不校验活性，避免持锁跨 await）。
- `pub fn reset()`（第 24 行）：清空缓存（保存/清除凭据、切换总开关、失效时调用）。
- `pub async fn ensure_alive(account, password)`（第 31 行）：缓存命中且探活通过则复用，
  否则用保存凭据静默重登一次（无人工交互）。
- `pub async fn login_and_cache(account, password)`（第 42 行）：建全新会话并入库。
- `pub fn store(session)`（第 51 行）：把外部建立的会话入库（连接测试复用登录态）。
- `pub async fn login_new_session(account, password)`（第 57 行）：一次性建全新会话（不入
  缓存）：`CasClient::new` → `rsa_encrypt_hex(password)` → `login_with_captcha`（kaptcha
  自动解题）→ `WebVpnSession::login`（TGT 换 ST + 302 链）。
- 测试（第 70–81 行）：`cache_reset_and_relogin_semantics`（第 75 行）。

### error.rs（CAS 协议错误）

- `pub enum CasAuthError`（第 6 行，`Debug, thiserror::Error`）：变体 `Rsa(String)`
  （第 8 行）、`Http(reqwest::Error)`（第 10 行）、`Parse(String)`（第 12 行）。
- `pub fn map_cas_login_error(e: CasLoginError) -> String`（第 16 行）：CAS 登录错误 → 中文
  （不含凭据），覆盖全部 `CasLoginError` 变体。
- `pub fn map_webvpn_error(e: WebVpnError) -> String`（第 29 行）：WebVPN 错误 → 中文。
- `pub fn map_cas_auth_error(e: CasAuthError) -> String`（第 38 行）：`CasAuthError` → 中文。
- 测试（第 42–58 行）：`cas_error_messages_are_chinese_without_credentials`（第 47 行）。

## Architecture

协议调用关系（UTF-8 box-drawing）：

```text
                ┌─────────────── cas ───────────────┐
   CasClient    │ kaptcha→captcha  login→rsa/jar     │
   login/       │ sso_ticket                       │
   sso_ticket   └───────┬───────────────┬──────────┘
                TGT     │               │
┌──────────┐            │      ┌────────▼───────┐   ┌──────────┐
│  cache   │────────────┼─────►│   session      │   │  jar     │
│ ensure_  │            │      │ login/is_alive/│   │Recording │
│ alive    │            │      │ follow_wrap    │   │          │
└──────────┘            │      └──────┬─────────┘   └──────────┘
                        │             │ wrap_url
┌──────────┐            │      ┌──────▼──────┐   ┌──────────┐
│  route   │────────────┘      │  wrap        │──►│  crypto  │
│  route   │                   │  wrap_url    │   │ encrypt  │
└──────────┘                   └─────────────┘   └──────────┘
```

- `route` 只做静态纯函数决策（zone × 目标 × 会话态），命中 `Wrapped` 时调用 `wrap::wrap_url`。
- `wrap::wrap_url` 每一跳依赖 `crypto::encrypt_host`（key 派生 `hex(IV)`）。
- `cache::ensure_alive` 是命令层入口：缓存/探活/静默重登，内部串起 `cas` + `session`。
- `cas::login` 依赖 `rsa`（token 头 + 密码加密）、`captcha`（kaptcha 解题）、`jar`（cookie 记录）；
  `cas::sso_ticket` 用 TGT 换 ST，交给 `session::login` 走网关 302 链。

## Data Flow

校外访问内网自助服务的两条主线：

1. **路由决策流**（`route::route`）：命令层先探测网络归属得 `NetZone`，传入目标 URL 与
   会话是否有（`cache::cached().is_some()`）。Campus/Unknown→`Direct` 直连；OffCampus×公网
   →`Direct`；OffCampus×内网×有会话→`Wrapped(wrap_url 结果)`；OffCampus×内网×无会话→
   `NeedLogin`（上层提示重新登录）。

2. **静默登录流**（`cache::ensure_alive` → `login_new_session`）：
   - `CasClient::new` 挂载 `RecordingJar`；
   - `rsa_encrypt_hex(password)` 得密码密文（拒绝非 ASCII）；
   - `login_with_captcha`：`GET /kaptcha` 取图 → `captcha::solve` 自动解题 →
     `login`（带 `token = cas_token_header(毫秒)` 与 7 字段 body）→ 解析
     `CasLoginOk.tgt`；CODEFALSE 换图重试，识别穷尽报 `CaptchaUnsolvable`；
   - `WebVpnSession::login(cas, tgt)`：GET 网关 `/` 种初始 cookie →
     `cas.sso_ticket(tgt, WEBVPN_SERVICE)` 换 ST → `follow_wrap`（逐跳 `wrap_url` 后手动
     跟随 302 链）→ 落点 cookie 入 jar；
   - 登录态判定经 `is_alive`：GET 网关 `/` 若 302 到含 `/login` 即失效（不读 cookie 名）。
   - 全程无 Cookie 依赖完成 CAS（服务端不种登录 cookie），会话复用全靠客户端保存的 TGT。

## Connections

- [[campus-auth]]（Wxxy-CampusHub 同源 `campus_auth::jar::RecordingJar` 与 CAS 协议，见
  session.rs:15、cas.rs:1-6、error.rs:1-2 移植说明）。
- [[self-service]]（校外自动回退的路由核心服务于自助服务，路由判据 `INTERNAL_HOSTS` 钉住的
  即自助服务 `SELF_BASE_URL` host，`route.rs:56-59`）。
- [[commands-vpn]]（命令层 `commands/vpn.rs` 已就绪匹配 `RouteDecision::Unreachable` 预留位，
  见 `route.rs:51`；是 `cache`/`route`/`session` 的上层调用方）。
- [[cas-recon-report]]（`docs/cas-recon/REPORT.md`：本模块全部协议事实的活样本来源）。
- [[ag-issues]]（`AGENTS.md` 双端同步第 1 类例外：协议代码单点于桌面 crate，禁止复制到
  `android/`，见 `mod.rs:11-14`）。

## Known Issues

- **解密侧无生产调用方**：`crypto::Aes128CfbDec`（crypto.rs:14-15）与 `decrypt_host`
  （crypto.rs:30）均标记 `#[allow(dead_code)]`；本端只做 URL 包装加密，解密仅为对称参考
  实现，由测试锚定。`WebVpnError::InvalidIv`（mod.rs:56）同理仅由 `decrypt_host` 构造。
- **会话层 dead_code 常量**：`session::COOKIE_TICKET`（session.rs:40）标记 `#[allow(dead_code)]`，
  登录态判定走 302 Location，不读 cookie 名。
- **jar 预留冻结接口未接调用方**：`RESTORE_URL` / `snapshot` / `restore` / `has`
  （jar.rs:19、44、51、71）全部 `#[allow(dead_code)]`；因会话策略定为「不落盘、失效重登」
  （session.rs:13-26），多域恢复暂不启用。
- **route 预留位不产生**：`RouteDecision::Unreachable`（route.rs:52）标记 `#[allow(dead_code)]`，
  `route` 全组合扫描不产生它（route.rs:236-249）；网络观测类「无可用路径」属上层职责。
- **captcha 测试锚定接口**：`KaptchaTemplates::from_json_str`（captcha.rs:62）标记
  `#[allow(dead_code)]`，仅集成测试锚定，`bin` 目标用 `load()`。
- **CAS 登录 ticket 字段未消费**：`CasLoginOk::ticket`（cas.rs:46）标记 `#[allow(dead_code)]`，
  当前流程仅用 `tgt`。
- **RSA 拒绝非 ASCII**：`rsa_encrypt_hex`（rsa.rs:29）显式拒绝非 ASCII 明文（JS `charCodeAt`
  语义不等价），计划 P2-11 修正。
- **Unknown 默认直连**：`route`（route.rs:12-14）对 Unknown 不拦截，且不做「先直连失败再
  自动改走 WebVPN」的二次尝试（写路径触碰资金安全红线，宁可不自动化）；直连失败由上层中文兜底。
- **WebVPN 会话不持久化**：登录态 cookie 由网关一次性 token 换发，不落盘（session.rs:13-26）；
  失效由 `is_alive` 或业务响应暴露，上层凭保存的 TGT 静默重登（TGT 由调用方持久化，如 M1 DPAPI）。
- **验证码低于阈值必重试**：`captcha::solve` 不达 `NCC_MIN` 返回 `None`（captcha.rs:17），
  上层刷新重试；绝不硬猜，避免错答案消耗 CAS 连续错误计数。
