---
title: "VPN 旁路定调:Network API 层不可绕(netd EPERM),走内核 SO_BINDTODEVICE + 探针回退"
type: decision
source_files:
  - tauri-app/src-tauri/src/network/bound_socket.rs
  - tauri-app/src-tauri/src/auth/protocol.rs
  - tauri-app/src-tauri/src/auth/portal.rs
  - android/src-tauri/src/campus_detect.rs
  - android/src-tauri/src/monitor_loop.rs
tags: [决策, 安卓, VPN, 网络判定, 平台边界]
---

## 背景

2026-09-13 排查「通知常显在线/VPN 下无法登录」时引出的用户期望:应用从安卓底层绕过 VPN 直连 WiFi。动机场景:

- 部分全量 VPN(Clash、UU 加速器等)不支持 per-app 排除,未排除时本应用的 socket 探测与登录请求全部被 tun 劫持——**用户实测:开 Clash 不排除本应用就无法登录**;
- 用户当前 VPN 支持排除本应用,但希望应用侧有兜底。

## 决策

2026-09-13 两轮定调:

**第一轮(Network API 层)**:`bindProcessToNetwork`/`Network.bindSocket` 等 **Network API 路线不可行**——secure 全量 VPN 下被 netd 直接 `-EPERM` 拒绝(见下"理由"),该路线彻底关闭。

**第二轮(内核层,已实施)**:GitHub 调研发现 **`SO_BINDTODEVICE` 内核旁路**——Linux kernel ≥5.7(Android 12+)新建 socket **首次** `setsockopt(SOL_SOCKET, SO_BINDTODEVICE, 接口名)` 不需要 CAP_NET_RAW,且**不经 netd 的 ip rule**(`RULE_PRIORITY_SECURE_VPN=13000` 先于网络绑定规则),由内核直接按设备查路由,可绕过全量 VPN 的 uid 劫持。落地为 `network/bound_socket.rs`(共享 crate,Android cfg 门控):

1. **能力探针**:对 "lo" 一次性 socket 试绑定,Ok→Supported / EPERM|EACCES→Denied / 其他→Unavailable,OnceLock 缓存;非 Supported 一律走原通道。
2. **物理网卡选择**:接口名过滤(排除 tun*/utun*/ppp*/rmnet*/ccmni*/lo/dummy*),wlan0 优先;按名绑定,不要求已有 IP。
3. **接入点**:`campus_detect.rs::portal_reachable`(网关/Portal TCP 探测)直接走旁路 socket;`auth/protocol.rs` 登录/Radius 注销/MAC 解绑、`auth/portal.rs` Portal 页面探测**整体切换到旁路客户端**(reqwest + 本地代理,HTTP 层零改动),桌面路径零变化(cfg 隔离)。
4. **回退语义**:能力缺失/无物理口/socket 创建或绑定层错误→自动回退普通连接;**connect 层失败不回退**(物理网不通重试原通道也必失败,只翻倍超时)。
5. **HTTP 层必须 100% 复用 reqwest——本地转发代理,不手写 HTTP**(2026-09-14 真机教训):首版手写 minimal HTTP(hyper handshake)在真机被学校网关 nginx **400 Bad Request** 拒绝,对照实验矩阵(reqwest 200 / hyper 手写 400;补齐 accept/cache-control/pragma 头、去掉 SO_BINDTODEVICE 均无法消除)证明手写请求与 reqwest 的 wire 级差异无法穷举定位。终态:本地起单请求转发器(127.0.0.1 随机端口,absolute-form → origin-form 重放,注入 `connection: close` 单请求语义),reqwest 以 `Proxy::all` 指向它(`create_bypass_http_client`),登录/注销/探测的客户端整体切换——HTTP 层(头/重定向/charset)100% 复用 reqwest,只有传输层被替换。另修非阻塞 connect 的 **EINPROGRESS(115) 不被 std 映射为 WouldBlock** 导致旁路整体回退的 bug(按 `raw_os_error()==libc::EINPROGRESS` 判定,教训见 `learnings/nonblocking-connect-einprogress-not-wouldblock`)。
6. **提示兜底**:`monitor_loop.rs::with_vpn_hint`——`vpn_tun_present()`(tun*/utun*/ppp* 宽松匹配)为真时,失败类通知 message 追加「检测到 VPN 可能在接管本应用流量:请在 VPN 中排除本应用或临时关闭 VPN」。

## 理由(源码级证据)

- **Network API 路线为何死**:`system/netd/server/RouteController.h` 的 `RULE_PRIORITY_SECURE_VPN = 13000` 数值先于 `RULE_PRIORITY_UID_EXPLICIT_NETWORK = 15000`/`RULE_PRIORITY_EXPLICIT_NETWORK = 16000` 命中;`NetworkController.cpp` `checkUserNetworkAccessLocked()` 对 secure VPN 下的 UID 选网直接返回 `-EPERM`;`FwmarkServer.cpp` 的 `PROTECT_FROM_VPN` 分支 `canProtect(uid, netId)` 仅放行持 VpnService 授权的 uid。ics-openvpn#1058 实证同款报错。
- **SO_BINDTODEVICE 为何可行**:内核 `sock_bindtoindex_locked()`(torvalds/linux net/core/sock.c)——只有"改绑已绑过设备的 socket"才要求 CAP_NET_RAW,首绑免权限;不经 netd,绕开 ip rule 的 uid 劫持。证据:GrapheneOS os-issue-tracker#2381(Pixel 6a,Mullvad 全量隧道下 `--interface wlan0` 拿到真实 WiFi IP);po4yka/RIPDPI(Rust 参考实现,含探针模式);GrapheneOS PR#33 的封堵**仅对 lockdown VPN 生效**。
- **野路子全被堵**:AF_PACKET 被 SELinux 拒绝(untrusted_app 的 net_domain 白名单无 packet_socket);raw socket 仍走路由表被 tun 劫持;IP_TRANSPARENT/SO_MARK 需 CAP_NET_ADMIN;Shizuku 改 ip rule 无先例;自建 VpnService 会顶掉用户现有 VPN 且整机接管,代价不成比例。
- 唯一官方出口(旁路失效时的兜底):VPN 侧 `allowBypass()`(由 VPN 决定,sing-box/FlClash/ics-openvpn 有此选项)或 `addDisallowedApplication`/per-app 排除(由用户配置,WireGuard/v2rayNG/Clash 系均有)。

## 影响与约束

- 旁路是**机会性路径**:ROM 封堵(Denied)或 lockdown VPN 下自动回退原通道,行为不劣于实施前;真机需确认探针结果(logcat `[bind-dev]` 前缀)。
- 已知边界:域名形态的 portal_url 下 DNS 解析不走旁路 socket(getaddrinfo→netd,全量 VPN 下可能被 tun DNS 污染)——当前配置为 IP 直填,风险低。
- `SO_BINDTODEVICE` 与 `bindProcessToNetwork`(ensure_wifi_bound)的 fwmark 交互需真机实测确认。
- **Android 17 兼容预警**:Local Network Protections(targetSdk ≥ 37)默认阻断局域网访问,探 10.x 网关/Portal 需申请 `ACCESS_LOCAL_NETWORK`(Android 16 临时用 `NEARBY_WIFI_DEVICES`);出包升 targetSdk 前必须复查此链路。
- 桌面端零变化:bound_socket 的 socket/HTTP 通道 `#[cfg(target_os = "android")]` 门控,Windows 下的 VPN 场景维持现状(桌面未提需求)。

## 真机验收（2026-09-14 用户实测）

- **核心场景通过**：VPN **不排除本应用**时登录/探测正常工作（旁路代理生效，流量走物理网卡绕过 tun）——本决策的目标场景。
- 无 VPN 场景登录/探测正常（reqwest 原行为不变）。
- 断 WiFi 通知翻转提速已实装（lost 零延迟 + 探测并行 + 绑定快速返回），目标 1-2s。
- 遗留观察点：`SO_BINDTODEVICE` 与 `bindProcessToNetwork` 共存时的 fwmark 交互在不同 ROM 上可能不同，异常时看 logcat `[bind-dev]`；Android 17 LNP 权限预警不变（见上"影响与约束"）。

## Connections

[[android-notify-online-pinned-by-offline-guard]]、[[android-keepalive-fgs-architecture]]、[[background-check-and-auto-login]]
