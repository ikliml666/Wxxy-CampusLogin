### 界面说明文案精简：删除复述布局的辅助标签（2026-11-13，双端）

**为什么**：卡片副标题与设置项说明里有一批只是把标题换个说法（「网络质量 / 实时网络延迟监测」「账号管理 / 切换登录账号」「系统日志 / 查看应用运行日志，定位问题」），或在卡片里已经用开关、列表、图表表达过同一件事（「自动化设置 / 控制登录相关的自动行为」「测试详情 / 各节点延迟检测结果」）。这些文字不增加信息，只增加阅读负担——图标、分组、留白、控件位置已经把关系说清楚了。

**改了什么**：双端共删除 59 处复述式说明，并删除随之失去引用的 i18n 键（两端 zh/en 各 28 个）：

- 首页 5 张卡副标题（快捷操作 / 账号管理 / 网络质量 / 在线信息 / 近期上网记录），安卓自助服务合并卡副标题；
- 质量页 2 张卡、账号页 2 张卡、自助服务页在线信息卡、日志页系统日志卡的副标题；
- 设置页 5 张卡副标题、新手指引横幅副标题、「启动时默认显示」说明，以及「自动登录校园网」「登录成功后退出」「Windows Hello 验证」「可登录时自动登录」等只是复述 label 的字段说明；
- 监控页两张分组卡副标题、检测状态副标题（与右侧开关状态标签重复）、「启动时自动开启检测」「校园网检测时间段」说明；
- 「延迟计算选项」弹窗描述——它描述的是整张设置卡，与弹窗内容不符。

保留的是承载真实信息的说明：时段与时长（夜间出站切换、退出生效时段、定时登录/注销）、后果与约束（轻量化模式、导出含密码、校园网环境验证）、单位与状态（时长/流量单位、未配置提示、绑定凭据来源）。

**验证**：桌面 `npx tsc --noEmit --incremental` 与 `npx vitest run`（23 文件 137 用例）全通过；安卓 `tsc` 通过；两端 `npx vite build` 成功；两端 zh/en 键集合仍一一对应（桌面 855、安卓 821）；`git diff --stat` 确认双端同步改动。

### 登录失败提示补充「运营商终端数量超限」（2026-11-13，双端共享）

**为什么**：虚拟机实测同一账号的第二台设备登录被拒——`dr1003({"result":0,"msg":"运营商终端数量超限！","ret_code":1})`。原解析落到 `unknown_failure` 分支，把服务端原文直接抛给用户：只知道「登录失败」，不知道要先去别处注销。移动运营商对单账号限制在线终端数，旧会话没释放时新设备就挤不进来——这很可能是「切 WiFi 后不能自动登录」的来源之一（旧会话仍挂在 AC 上，新登录被拒，客户端显示的是失败而不是「未登录」）。

**改了什么**：`auth/protocol.rs` 的 `parse_login_result` 增加分支——`msg` 含「终端数量超限」时返回 `code=terminal_limit`、提示「该账号已有设备在线（运营商限制单账号终端数），请先在原设备注销后再登录」、`retryable=false`。**未**加入 `AUTH_FAILURE_CODES`：这不是凭据问题，连续重试触发 MAC 重置解决不了终端数限制，只会白跑五轮。

**验证**：`cargo test --lib parse_login_result` 7 passed / 0 failed（含新增 `parse_login_result_terminal_limit`，断言 code/message/retryable 三项）。

### 登录失败提示补充「运营商终端数量超限」（2026-11-13，双端共享）

**为什么**：虚拟机实测同一账号的第二台设备登录被拒——`dr1003({"result":0,"msg":"运营商终端数量超限！","ret_code":1})`。原解析落到 `unknown_failure` 分支，把服务端原文直接抛给用户：只知道「登录失败」，不知道要先去别处注销。移动运营商对单账号限制在线终端数，旧会话没释放时新设备就挤不进来——这很可能是「切 WiFi 后不能自动登录」的来源之一（旧会话仍挂在 AC 上，新登录被拒，客户端显示的是失败而不是「未登录」）。

**改了什么**：`auth/protocol.rs` 的 `parse_login_result` 增加分支——`msg` 含「终端数量超限」时返回 `code=terminal_limit`、提示「该账号已有设备在线（运营商限制单账号终端数），请先在原设备注销后再登录」、`retryable=false`。**未**加入 `AUTH_FAILURE_CODES`：这不是凭据问题，连续重试触发 MAC 重置解决不了终端数限制，只会白跑五轮。

**验证**：`cargo test --lib parse_login_result` 7 passed / 0 failed（含新增 `parse_login_result_terminal_limit`，断言 code/message/retryable 三项）。

### 安卓注销从 139 秒降到秒级：NAT 跳过无意义的 MAC 解绑（2026-11-13，安卓）

**为什么**：真机点注销要等十几秒到两分钟，最后弹「Radius注销成功，MAC解绑失败」。按代码算得出来：`do_logout_with_retry(..., 3, ...)`（`android/src-tauri/src/core/login.rs:132`）外层 3 次，每次内部 Radius 2 轮（每轮 15s 超时、轮间等 1.5s）+ MAC 解绑 1 次（15s），单次最坏 46.5s，三次共 139.5s；前端 60s 窗口必然先超时，用户看到的就是「卡住」。

**真因不是「服务端不响应」**——虚拟机实测（未认证 IP 10.2.38.149，走同一台 AC）推翻了原先的推断：

| 请求 | 参数 | 耗时 | 响应 |
|---|---|---|---|
| Radius 注销 | `wlan_user_ip=`（空） | **76ms** | `{"result":0,"msg":"Radius注销失败！"}` |
| MAC 解绑 | `wlan_user_ip=` + `wlan_user_mac=000000000000` | **78ms** | `{"result":0,"msg":"获取用户在线信息数据为空！"}` |
| Radius 注销 | `wlan_user_ip=10.2.38.149` | **47ms** | `{"result":0,"msg":"Radius注销失败！"}` |

服务端对任何注销请求都立即回包，空参数也不例外。所以安卓的 15 秒挂起只能是**请求没到达**或**响应没回来**——本地旁路代理（`127.0.0.1` 转发 + `SO_BINDTODEVICE` 重建上游连接）是首要嫌疑，而同批改动已让无 VPN 时不再启用代理。这次的超时收紧是双保险：即使链路异常，也不再让用户干等 139 秒。

**改了什么**

- `auth/protocol.rs`：Radius 注销从「2 轮 + 轮间 1.5s」改为单轮，单次超时 15s → 3s（`LOGOUT_REQUEST_TIMEOUT`）；MAC 解绑超时 15s → 1.5s（`UNBIND_REQUEST_TIMEOUT`）；客户端兜底超时 15s → 6s。
- `auth/protocol.rs`：NAT 场景跳过 MAC 解绑（没有可解的绑定），新增 `unbind_attempted` 标记；`merge_logout_results` 增加该参数，跳过不再显示成「MAC解绑失败」。
- `android/src-tauri/src/core/login.rs:132`：外层重试 3 次 → 1 次（内层已不叠加轮次，外层再重试只会成倍拉长等待）。

**最坏耗时**：NAT 3s、非 NAT 4.5s（原来 139.5s）。

**验证**

- 桌面 `cargo test` 633 passed / 0 failed（含新增「NAT 跳过解绑仍算注销成功」断言）。
- 虚拟机（VMware a8，桥接，未认证 IP 10.2.38.149）实测上表三组请求的耗时与响应，确认服务端不做长等待。
- 同一账号第二台设备登录被拒：`dr1003({"result":0,"msg":"运营商终端数量超限！","ret_code":1})`——移动运营商限制终端数，这可能是「切 WiFi 后登录失败」的另一个来源（旧会话未释放，新设备挤不进去）。
- 安卓真机注销耗时复测待回填（本轮 adb 连接中断，新 APK 未能安装）。

### Portal 状态探测改用结构化接口、旁路代理按 VPN 启用（2026-11-13，双端共享）

**为什么**：`Portal页面请求失败` 反复出现，但同一时刻同一条网络下 Portal 各端点都通——问题在判据选得最脆弱。校园网实测（10.2.38.176）：

| 端点 | 结果 | 耗时 |
|---|---|---|
| `http://10.1.99.100/` | 200 · 2994B · **gb2312** | 132ms |
| `http://10.1.99.100/drcom/chkstatus?callback=dr1003` | 200 · 731B · `dr1003({"result":1,"uid":"..."})` | 42ms |
| `:801/eportal/portal/page/chkstatus` | 200 但 body 是 `404 eportal action chkstatus not found` | 11ms |

原实现探 80 端口首页、靠 `<title>注销页</title>` / `Dr.COMWebLoginID` 特征串判在线：页面是 gb2312 编码的近 3KB 文档，编码偏差、改版、AC 注入任一环节出错就落到 Unknown/Failed；且单发无重试、超时 3s（桌面）/5s（安卓），失败即 `login_available=false` 让自动登录短路，而手动登录走 801 协议端点不受影响——这就是「手动能登、自动不登」的直接来源。

安卓侧还叠加一层：所有请求经本地旁路代理 `127.0.0.1`（`SO_BINDTODEVICE` 转发），代理模式下 `local_address` 不生效（客户端池 key 显示 `local_addr=None`），「本机从哪张卡出去」的判据被抹掉；而未开 VPN 时本就不需要这层代理。

**改了什么**

- `auth/portal.rs`：主判据换 `{origin}/drcom/chkstatus?callback=dr1003`（`result==1` 在线 / `result==0` 离线，`uid` 直接供运营商徽标）；端点不可达或壳解析失败时回落页面特征探测（历史路径保留）。新增 `CHKSTATUS_TIMEOUT=2s`。该端点只有 `callback` 参数、响应无 `Set-Cookie`，是只读查询，与登录端点（:801、带账号密码）不相干；夜切验证自 2026-09 起已在用同一端点。
- `auth/portal.rs`：新增不变量单测，断言探测端点不含 `login`/`user_account`/`password`——防止后续被改成登录端点导致「检测即登录」。
- `network/bound_socket.rs`：`bypass_proxy_addr()` 增加 VPN 门控，无 VPN 直接返回 `None`（直连）；新增 `set_vpn_active_check()` 注入点，未注入时回退接口名匹配。
- `CoreService.kt`：新增 `isVpnActive()`，用 `ConnectivityManager.allNetworks` + `NetworkCapabilities.TRANSPORT_VPN`（Android 官方判据，API 23+）。
- `core/platform.rs`、`core/jni_platform.rs`、`core/runner.rs`：`CorePlatform` 新增 `is_vpn_active()`，`:core` 启动时注入；JNI 调用失败回退 `vpn_tun_present()`，不因一次调用失败就关掉旁路。

**验证**

- 桌面 `cargo test` 633 passed / 0 failed（含新增端点形状断言）。
- 真机探测（本机在校园网 10.2.38.176）：`online=true reachable=true uid=Some("24385214@cmcc") message=已在线 error_kind=None`，耗时 0.16s——`error_kind=None` 即"明确判定"，不再落到 Unknown/Failed。
- `pwsh android/build-apk.ps1` 出包成功（22.46 MB，签名校验通过）并安装到 RMX3300。重启后 logcat 只有 `[bind-dev] 能力探针(lo) = Supported`，**不再出现**「[bind-dev] 旁路代理已启动」——手机未开 VPN 时按新逻辑直连，`local_address` 重新生效。
- 未验证：应用日志页只加载最早 200 行，本轮没有直接翻到 `[http] 客户端池新建: key=(Some(10.2.x.x), 3, 8000, None)`（旧值为 `None`），该项留待下一轮巡检确认。

### 安卓登录卡 15 秒后失败：请求源 IP 改用实时接口地址（2026-11-13，安卓）

**为什么**：真机日志（应用内「更多 → 日志」，`app-2026-11-13.log`）显示登录偶发失败于 `error sending request for url (http://10.1.99.100:801/eportal/portal/login?***)`，单次耗时正好 15 秒：

- `16:19:39.481 登录请求开始 adapterIp=default` → `16:19:39.811 完成(321ms) status=200`
- `16:19:39.922 登录请求开始 adapterIp=192.168.6.203` → `16:19:54.933 失败(15s)`
- `16:28:53.078 登录请求开始 adapterIp=10.2.106.90` → `16:29:08.083 失败(15s)`，`16:29:10.096` 同一地址 49ms 成功

`192.168.6.203` 不是当时 WiFi（i-wxxy，`10.2.106.90`）的地址：它是探测那一刻的旧快照，WiFi 换网后没有任何失效机制。协议层拿它当 `local_addr` 绑定请求源，地址已不属于本机，SYN 发不出去，只能等满 15 秒客户端超时——用户看到的就是「点登录后一直卡住，最后弹登录失败」。

**改了什么**

- `android/src-tauri/src/campus_detect.rs`：新增 `current_source_ip()`（枚举本机接口 → `pick_campus_source_ip`）；网卡展开逻辑抽成 `flatten_v4`，探测与源地址挑选共用一份口径。
- `android/src-tauri/src/core/platform.rs`：`CoreCtx::cached_source_ip()` 改为按当前接口实时计算并回写缓存（登录、注销、巡检的源地址来源）。
- `android/src-tauri/src/protocol_cmds.rs`、`quality_cmds.rs`、`self_service_cmds.rs`：Portal 探测、质量检测、自助服务会话的源地址同样改为实时计算；`android_state.cached_source_ip` 只保留「最近一次结果」，不再作为请求依据。


**第二个问题：手动登录后巡检立刻再登一次（同一次登录里第二条请求挂 15 秒）**

同一天的日志里，每次登录都会在 0.4~0.5 秒内发出两条请求：第一条 50~300ms 成功，第二条挂满 15 秒才失败，2 秒后重试才成功。第二条来自 :core 巡检——巡检的 `online` 是本轮探测（发生在手动登录之前）的结论，探测完成到判定之间手动登录已经成功，巡检仍按「未在线」判定并再登一次，而 portal 对同一 IP 的重复认证请求不回包。用户看到的就是按钮卡十几秒。修法是把「登录成功时间」接进巡检判定：

- `android/src-tauri/src/core/monitor.rs`：`MonitorState` 新增 `last_login_success_ms`；`run_check_once` 记录本拍探测起点 `tick_start_ms`，判定条件再加一道「本拍探测开始后没有新的登录成功」。
- `android/src-tauri/src/core/login.rs`：`LoginAttempt::finish` 成功时写入 `last_login_success_ms`（手动与自动登录共用同一收尾）。

**第三个问题：登录请求偶发挂满 15 秒**

修完上面两处后仍能观测到：应用启动后立刻登录，单条请求挂满 15 秒客户端超时才失败，2 秒后重试 100~200ms 成功；等应用空闲下来再登录则稳定 130ms。启动期后台正在跑 19 项网络质量检测，两者并发时登录请求会被拖到超时。登录是用户点按后干等的操作，把单次超时从 15 秒收紧到 6 秒：3 次重试仍有 18 秒以上总窗口，但单次卡顿减半再减半。

- `tauri-app/src-tauri/src/auth/protocol.rs`：登录请求客户端与单请求超时 15s → 6s。

**第四个问题：一次点击发两条登录请求，第二条必挂满超时**

真机上仍能复现：`18:13:55.741` 第一次请求 132ms 成功，`18:13:56.035`（0.3 秒后）又发一条，挂到 `18:14:02.038` 才失败，重试 241ms 成功。第二条来自与手动登录几乎同时的另一条登录入口（巡检/事件），它拿到所有权时第一条已经完成，互斥锁拦不住。改为在 `run_login` 入口做成功冷却：登录成功后 5 秒内的重复请求直接返回「已登录」，不再发请求。

- `android/src-tauri/src/core/login.rs`：`run_login` 开头按 `MONITOR.last_login_success_ms` 拦 5 秒内的重复请求（只拦成功后的重复，失败重试不受影响）。

**安卓侧三项 Portal 探测适配（2026-11-13，安卓为主）**

**为什么**：`Portal页面请求失败` 是状态探测拿不到响应（`[network]` 模块，`portal.rs` 里 `client.get(page_url).timeout(3s)` 失败），不是页面返回错误。两端共用同一段探测代码，但安卓多一跳本地旁路代理（`127.0.0.1` 转发 + `SO_BINDTODEVICE` 重建上游连接），同一个 3 秒窗口在启动期（19 项质量检测并发、校园网关丢新连接）更容易被打满——真机日志里这条错误和登录的 6 秒超时都集中在这个窗口。

**改了什么**

- **探测超时分档**（`tauri-app/src-tauri/src/auth/portal.rs`）：`REQUEST_TIMEOUT` 安卓 5 秒、桌面保持 3 秒。
- **启动期让路**（`android/src-tauri/src/quality_cmds.rs`、`tauri-app/src-tauri/src/network/quality.rs`）：安卓首轮质量检测延后 15 秒；HTTPS 批次并发安卓 4 → 2、桌面保持 4。
- **失败分类**（`tauri-app/src-tauri/src/auth/portal.rs`）：`PageCheckResult::Failed` 携带 `PortalFailure{Timeout,Connect,Other}`，状态文案与 `error_kind` 区分「Portal 响应超时」/「Portal 不可达」/「Portal页面请求失败」，日志同时带类别，排障不用再靠猜。

**验证**：桌面 `cargo check` exit 0、`cargo test` 632 passed / 0 failed（含 `build_request_failed_status_correct` 对 Timeout/Connect/Other 三类文案与 error_kind 的断言）；安卓 `cargo check --target aarch64-linux-android --all-targets` exit 0（无警告）。

**Portal 探测收归 :core 单点（2026-11-13，安卓）**

**为什么**：UI 的 `check_portal_status` 在 UI 进程自建旁路代理探测，:core 巡检又探一次，两个进程各打一次网关——一次网络抖动被记成两条失败，界面还会在「已在线/不可达」之间闪；启动首拍只检测不登录时也没有任何日志说明原因，用户只看到状态是「未登录」。

**改了什么**

- 新增 `android/src-tauri/src/core/net_state.rs`：Portal 状态单点。成功结果缓存 5 秒、失败结果退避 10 秒、并发调用单飞（同一次探测服务所有调用方）；缓存绑定探测时的源 IP，WiFi 换网后自动失效；`invalidate()` 供「立即检测」强制刷新。
- `android/src-tauri/src/core/monitor.rs`：巡检改用 `net_state::portal_status`，原来的 `portal_probe_on_little_cores`（绑小核 + reactor context 处理）整体搬进 net_state；`trigger_background_check` 先 `invalidate`，用户点「立即检测」不吃缓存。
- `android/src-tauri/src/protocol_cmds.rs`：`check_portal_status` 不再本地探测，改为 Binder 转发 :core 的 `portalStatus` 动作；`core_bridge_cmds::call` 提升为 `pub(crate)`。
- `android/src-tauri/src/core/runner.rs`：新增 `portalStatus` 动作（支持 `force`）。
- `android/src-tauri/src/core/monitor.rs`：启动首拍且「可登录时自动登录」关闭时，输出「启动检测:未开启「可登录时自动登录」,本轮只检测不登录」——用户能直接看到为什么没有动作。

**验证**：安卓 `cargo check --target aarch64-linux-android --all-targets` exit 0（无警告）。真机（RMX3300，i-wxxy `10.2.106.90`）复测：`19:25:10.580 :core 进程核心已初始化` → `19:25:11.036 登录请求开始 adapterIp=10.2.106.90` → `19:25:11.150 登录请求完成(113ms) status=200`（启动首拍检测与自动登录正常，0.5 秒内完成）；同一时段 `[http]` 只有一条 Portal 探测客户端新建（`key=(None,3,8000,…)`），`19:25:13` 之后 `[network]` 无失败记录；首轮质量检测在启动 15 秒后才开始（`19:25:25` 后），19 项全成功、耗时 18.9 秒。
**桌面端是否有同类问题**：没有同构缺陷。桌面登录每次 `full_login` 都重新枚举适配器（缓存 TTL 5 秒，且主适配器缺 IP 时 `should_refresh_adapter_snapshot` 强制刷新），源地址取自登录瞬间的适配器快照，不存在「跨网络切换滞留在内存里」的路径；`connect_timeout` 为 3 秒，也不会像安卓那样空等满 15 秒。

**验证**

- 安卓 `cargo check --target aarch64-linux-android --all-targets` exit 0，无新增警告（三轮改动后各跑一次）。
- `pwsh android/build-apk.ps1 -SkipFrontend` 出包成功、gradle release 签名校验通过。
- 真机 RMX3300（i-wxxy，`10.2.106.90`）复测：
  - 源地址：所有登录请求均为 `adapterIp=10.2.106.90`，不再出现旧网段 `192.168.6.203`。
  - 单次卡顿：应用启动后立刻登录（后台 19 项质量检测进行中）仍会先失败一次，但耗时从 15.0s 降到 6.0s（`18:03:36.145 → 18:03:42.149`），2 秒后重试 `201ms` 成功（`18:03:44.159`）；空闲时登录稳定 130~200ms。
  - 重复登录：同一次操作只发一条登录请求，不再出现「成功后 0.4s 又发一条并挂 15s」。



### 验证时效可配置、开机自启两项合并（2026-11-13，双端）

**为什么**：敏感操作（查看运营商明文密码、绑定与注销会话）的免重复验证窗口在后端 `IDENTITY_VERIFY_TTL_SECS=600`、前端 `VERIFY_TTL_MS=570000` 各硬编码一份，用户无法按使用频率调整；而"每次操作二次验证"开关本质就是 TTL=0 的另一种说法，两套控制面互相牵扯。启动设置里"开机自启（登录 Windows 后启动 GUI）"与"未登录时自动登录（SYSTEM 计划任务跑无头核心）"分列两个开关，很容易只开一半，且后者描述是一整段技术细节。

**改了什么**

- **配置字段**（`tauri-app/src-tauri/src/config/model.rs`、`android/src-tauri/src/config_state.rs`）：新增 `selfHelloTtlMinutes`（分钟，默认 10，`0` = 每次操作都验证），serde 缺省回落默认值；保留 `selfReverifyEachAction` 并把语义收窄为**高危操作二次确认**——注销在线会话、删除账号等操作每次执行都要求验证，不受验证时效影响。
- **后端时效参数化**（桌面 `platform/identity.rs`、安卓 `identity_gate.rs`）：`identity_verified_recently(ttl_secs)` 接收配置时效；命令层 `identity_ttl_secs()` 读取配置并钳制 `0..=1440` 分钟，`validate_config` 同步钳制落盘值——手改配置文件不能把免验证窗口放大到任意长短。
- **前端门**（两端 `account/selfServiceState.ts`）：验证门改为**全局共享**——自助服务面板、首页设备卡、绑定卡共用一个时间戳，切面板或换入口操作不再各自重新验证（此前两套独立时间戳导致刚在自助服务验证过的用户，回首页看设备卡又被要求验证）；面板卸载不再重置门。时效按 `config.selfHelloTtlMinutes` 计算（保留 30s 时钟余量，0 分钟即每次验证）。新增 `useDestructiveVerify()` 供注销会话、删除账号等破坏性操作用，`selfReverifyEachAction` 开启时每次强制验证。
- **设置页**（两端 `settings/SettingsPanel.tsx`）：安全组三项并列——启用验证、高危操作二次验证、验证时效。验证时效为数字输入（分钟），失焦/回车提交；值变化时先过身份验证再落盘，验证失败或取消回滚显示值。删除账号（桌面 `App.tsx`、安卓 `App.tsx`/`TabletShell.tsx`）与注销在线会话（自助服务面板、首页设备卡）接入 `useDestructiveVerify`。桌面端"开机自启"与"未登录时自动登录（系统级）"合并为一个开关：开启先配系统级（需提权），成功后再注册登录后自启，系统级失败即中止、不改任何一边；查询未完成前开关禁用，避免误显示为关闭。
- **i18n**（两端 `i18n/locales/{zh,en}.json`）：新增 `selfHelloTtl`/`selfHelloTtlDesc`/`selfHelloTtlUnit`，删除 `selfReverify`/`coreAutostart` 系列，`securityChangePrompt` 由"关闭"改为"修改"。

**验证**

- 两端 `npx tsc --noEmit --incremental` exit 0。
- 桌面 `cargo test` exit 0；安卓 `cargo check --target aarch64-linux-android --all-targets` exit 0（删除废弃常量后无 dead_code 警告）。
- 桌面前端 `npx vitest run`：23 文件 137 用例全绿。修了 `src/auth/DashboardPanel.selfCards.test.tsx` 的 mock 缺 `selfHelloTtlMinutes` 导致前端门永过期（第二次点击又弹验证）；新增"高危操作二次验证开启时，注销不再复用验证时效、每次都验证"用例（`SelfServicePanel.test.tsx`，12 用例）。
- 浏览器 mock 实测（`vite.mock.config.ts`）：桌面设置页启动设置只剩"开机自启 / 静默启动"，点合并开关后 mock 的 `set_core_autostart`、`set_auto_launch` 均被调用且开关转为开启；"验证时效"10→5 失焦后触发 mock 验证并保持 5（失败会回滚 10）；"分钟"单位不换行。安卓 mock（412px 容器）安全设置卡出现"验证时效"行与说明；安卓真机观感本轮未测。

### 安卓前端 Tauri 包版本对齐（2026-11-12，构建）

**为什么**：`android/frontend` 的 `@tauri-apps/api` 解析到 2.12.3、plugin-opener 2.7.1、notification 2.6.0、biometric 2.4.2，而 Rust crate 停在 tauri 2.11.5 / plugin-opener 2.5.5 / notification 2.4.0 / biometric 2.3.3。`tauri android build` 的版本校验要求两侧主次版本一致，直接拒绝打包（`Found version mismatched Tauri packages`），APK 出不来。

**改了什么**

- `android/frontend/package.json`：把四个包从宽松范围钉到与 crate 同主次的版本——`@tauri-apps/api` 2.11.1（2.11.x 在 npm 上只有 2.11.0/2.11.1）、`plugin-opener` 2.5.5、`plugin-notification` 2.4.0、`plugin-biometric` 2.3.3；`package-lock.json` 随之更新。

**验证**：`npm install` 后四个包版本与 `Cargo.toml` 的 crate 版本一致，`pwsh android/build-apk.ps1 -SkipFrontend` 通过（`Finished 1 APK`），产物安装到真机 RMX3300 正常启动。

### 更多页玻璃页签切换后消失、双端玻璃两侧内缘光（2026-11-12，双端）

**为什么**：更多页在「设置 / 日志」之间切换后，两个玻璃页签整条消失。根因不在切换逻辑，而在就绪标记的写入方式：`useGlassRefraction` 用 `el.classList.add('glass-live')` 命令式标记玻璃就绪，而宿主元素的 `className` 归 React 管理——切页签时 `active` 变化触发重渲染，React 把整串 class 原样写回去，`glass-live` 被抹掉；hook 的 effect 依赖 `[ref, level]` 不重跑，补不回来，`.glass-dock:not(.glass-live)` 的 `opacity: 0` 于是永久生效。实测切换后两个页签 live 标记缺失、opacity 0，3 秒后仍不恢复（DOM 节点同一，排除重挂）。
同一批截图还暴露出：内缘光来自 `::before` 的垂直渐变，亮线只存在于顶部 5px 的水平段，圆角段天然没有光——药丸形的悬浮栏与更多页页签整条边都是弧，观感上就是「两侧边缘没有白光」。

**改了什么**

- **就绪标记改用 data 属性**（`android/frontend/src/lib/glass-dock.tsx`、`tauri-app/frontend/src/lib/glass-dock.tsx`）：`glass-live` / `glass-ready` 改为 `data-glass-live` / `data-glass-ready`。`data-*` 不在 React 管理的 props 里，重渲染不会碰它。
- **样式选择器同步**（两端 `index.css`）：`.glass-dock.glass-ready:not(.glass-live)` → `.glass-dock[data-glass-ready]:not([data-glass-live])`，`:not(.glass-live):not(.glass-ready)` → 属性选择器版本；注释里的旧类名一并更新（含 `useStartupBoost.ts:122` 的说明）。
- **两侧内缘光**（两端 `index.css`）：`.glass-dock` 的 `box-shadow` 追加 `inset 1.3px 0 0` 与 `inset -1.3px 0 0 rgba(255,255,255,calc(var(--glass-shine)*0.34))`。沿边框绘制、在弧上自然收口，强度压到顶边约三分之一，避免读成闭合描边。桌面悬浮栏（`.glass-dock`）与安卓更多页页签（`.glass-dock.glass-chip`）因此都有两侧白光。

**验证**（均为本轮实际输出）

- 安卓 mock（360 CSS px / DPR4）：切换前、切到「日志」、切回「设置」三次采样，两个页签始终带 `data-glass-live`、`opacity: 1`，不再消失；`box-shadow` 含两条 inset。
- 桌面 mock（1280×800）：底栏 `.glass-dock` 597×55、圆角 18px，`data-glass-live` 生效、含两条 inset；依次点击 3 个 dock 项切换面板后仍 live、opacity 1。
- 两端 `npx tsc --noEmit --incremental` exit 0；两端 `npx vite build` 均成功。
- 真机验证（RMX3300，1440×3216，density 640）：更多页在「设置 / 日志」之间往返切换两次，两个玻璃页签始终存在且选中态正确；页签与底栏左右两端各放大 4 倍，可见内缘白线沿圆角走满整圈，两侧不再发黑。


### 质量页「立即检测」右对齐与检测项说明改独立弹窗（2026-11-12，双端）

**为什么**：上一轮为避免 360 CSS px 下标题被按钮挤成竖排，把「帮助 + 立即检测」从 `CardHeader` 下移到 `CardContent` 首行的 `div.ml-auto`。但 `ml-auto` 对块级 flex 容器无效，按钮组实际贴在左边，与左对齐的标题错位，右侧反而留白。帮助按钮仍是 Tooltip：触屏没有 hover，手机上点了没有任何反应。

**改了什么**

- **主操作右对齐**（`android/frontend/src/monitor/QualityPanel.tsx`）：按钮行容器 `ml-auto flex items-center` → `flex items-center justify-end`，按钮组贴右缘。
- **检测项说明改独立弹窗**（同上）：问号按钮不再用 Tooltip、也不在卡片里内联展开，改为打开 `Dialog`（`@/components/ui/dialog`：portal + 全屏遮罩 + 居中卡片 + `z-50`）。标题「检测项说明」、右上角关闭，TLS / TTFB / 内容传输三段在弹窗内展示；按钮带 `aria-haspopup="dialog"`。
- **双端同步**（`tauri-app/frontend/src/monitor/QualityPanel.tsx`）：同一按钮打开同款 Dialog（桌面端也走点击，不再依赖 hover）；两端 `i18n/locales/{zh,en}.json` 各补 `quality.metricHelp`（检测项说明 / Metric details）。

**验证**

- `tauri-app/frontend` 下 `node ./node_modules/typescript/bin/tsc --noEmit --incremental` exit 0；`android/frontend` 同命令 exit 0。
- 浏览器 mock 实测（`android/frontend` 的 `vite.mock.config.ts`，412 CSS px 手机宽度）：质量页「测试详情」卡的按钮组贴右缘；点「检测项说明」打开 `role=dialog` 的居中弹窗（全屏遮罩、标题「检测项说明」、三段说明、右上角关闭），关闭后回到卡片，触屏点击可用。
- 复现前提：`android/frontend` 本机 `node_modules` 为空，需先 `pnpm install`；esbuild 的平台包与二进制从 `tauri-app/frontend/node_modules` 复制，并按 mock 配置注释用重命名副本（`esb-renamed.exe`）绕过杀软。
### 安卓质量页窄屏排版、文案补齐与底栏两侧边缘光（2026-11-12，安卓端）

**为什么**：360 CSS px 窄屏下，「测试详情」卡的标题与说明被右侧「帮助 + 立即检测」按钮组挤成竖排；四个检测标签并排显示文字把整行占满，当前项反而被截断成「网关与D…」；中文界面漏出未翻译的 `quality.runTestNow` 等 5 个键；底栏改成六项后栏高由 70px 收到 64px，药丸半径却仍按 34px 写死，横向 mask 端点错位到圆角弧外，暗色下左右两侧的内缘光被整段吃掉。

**改了什么**

- **主操作下移**（`android/frontend/src/monitor/QualityPanel.tsx`）：帮助按钮与「立即检测」从 `CardHeader` 移到 `CardContent` 首行，标题与说明独占整行，窄屏不再被挤成竖排。
- **标签只显示当前项**（同上）：未选中标签在 `min-[520px]` 以下只留图标（`hidden min-[520px]:inline`）；字号 11→10px、间距 `gap-1.5`→`gap-0.5`、图标 12→10px，选中项文字在 360px 下完整显示。
- **文案补齐**（`android/frontend/src/i18n/locales/{zh,en}.json`）：补 `quality.runTestNow`、`emptyTitle`、`emptyDesc`、`emptyDetailsTitle`、`emptyDetailsDesc` 五键，值取桌面版文案。
- **药丸半径跟随栏高**（`android/frontend/src/index.css`）：`.glass-nav` 的 `--glass-r` 由 `34px` 改为 `calc((var(--dock-item-height) + var(--dock-pad) * 2 + 2px) / 2)`，末尾 2px 是上下各 1px 边框——药丸实际半径是内盒高一半再加边框。
- **两侧内缘光**（同上）：`.glass-nav` 追加 `inset 1.3px 0 0` 与 `inset -1.3px 0 0 rgba(255, 255, 255, calc(var(--glass-shine) * 0.34))`。原内缘光来自 `::before` 的垂直渐变，亮线只存在于顶部 5px 的水平段，圆弧段天然没有光；栏高收紧后弧更短，观感就是「两侧边缘发黑」。强度压到顶边的约三分之一，避免读成闭合描边。

**验证**（均为本轮实际输出）

- `android/frontend` 下 `npx tsc --noEmit --incremental` exit 0；`npx vite build` 通过。
- APK 安装到真机 RMX3300（1440x3216，density 640 → 360 CSS px）：测试详情标题单行、「立即检测」在标题下方、「网关与DNS」完整显示、其余三标签仅图标、界面无 `quality.*` 键外露。
- 底栏两端暗色截图放大 4 倍：沿左上圆角弧线采样，修复前 40–80° 区间亮度 9.1–9.7（与背景齐平，即无光），修复后可见内缘白线沿弧延伸；五项档同步确认观感一致。

### 安卓底栏常驻六项（质量开启时）与更多页玻璃页签（2026-11-12，安卓端）

**为什么**：此前质量开启时「质量」占掉底栏第 4 位、「后台检测」被挤进「更多」的子页签，看一次检测要在两级里点两次；关掉质量又反过来，"检测放哪里"随配置跳动。用户要求质量开启时六个图标共存、检测不再下放，并把「更多」页的标签按钮做成与悬浮栏同一套玻璃。

**改了什么**

- **底栏六项**（`android/frontend/src/components/layout/BottomNav.tsx`）：`useNavTabs` 在质量开启时同时插入 `quality` 与 `monitor`（总览/账号/自助服务/质量/监测/更多），关闭时只插 `monitor`（五项）。新增 `DOCK_METRICS` 两档尺寸：六项时项高 `clamp(46px,14vw,56px)`、图标 `clamp(20px,6.5vw,26px)`、文字 `clamp(9px,2.8vw,11.5px)`，外边距/内边距/项间距与五项档一致；外层 nav 按项数取档，真实项与镜头放大副本共用同一组变量。
- **页签顺序**（`android/frontend/src/App.tsx`）：`tabOrder` 改为六项/五项两个分支；删除「质量开启时把 monitor 归位到 quality」的 effect 与随之无用的 `configLoaded`——检测已是常驻可见页签，不再出现"停在检测页却没有高亮"。
- **更多页**（`android/frontend/src/components/mobile/MobileMore.tsx`）：检测页既已常驻底栏，移除更多页里重复的检测子页与 `qualityOnly` 过滤逻辑，只剩设置/日志两枚页签；随之无引用的 `mobile.moreMonitor` 文案键从 zh/en 两份语言文件删除。
- **玻璃页签**（新增 `android/frontend/src/components/mobile/GlassTabChip.tsx`）：直接复用悬浮栏的 `.glass-dock` 材质与 `useGlassRefraction`（Hyalite 折射），`h-9` 配自带 18px 圆角正好成药丸；选中态在 `index.css` 用 `.glass-chip[data-active='true']` 叠主色膜——走 `background-image` 而非 `background-color`，因为后者的底色约定归 `.glass-dock`，改写会把折射底色一起冲掉。

**验证**（均为本轮实际输出）

- 浏览器 mock（`vite.mock.config.ts` + Playwright，320/360/390/412 CSS px）：六项齐备、无溢出；360px 项宽 49.5px、图标 23.39px、文字 10.08px、栏高 64.39px；320px 图标 20.8px、文字 9px、项高 46px；412px 项高 56px、图标 26px、文字 11.5px。
- 高 DPI 复核：360 CSS px 配 DPR 4（等同 RMX3300 的 1440 物理像素宽）时六项等宽 49.5px，最长文案「自助服务」实测 40.33px，无重叠。
- 临时把 mock 的质量开关置 `false`：回落五项（图标 28.16px、文字 11.93px，与改造前同档），无溢出；验证后已还原 mock 文件。
- 点「监测」页签：`aria-current` 停在监测，面板实际渲染「网络状态检测」，不再被弹回质量页。
- 玻璃页签：`backdrop-filter` 为 `url("#hyalite-3")`、`glass-live` 已生效、尺寸 36×60px，选中项背景叠加 `rgba(72,75,239,0.16)` 主色膜。
- 安卓 `npx tsc --noEmit --incremental` exit 0；`npx vite build` `✓ built in 6.74s`（仅既有大 chunk 与 faceService 动态导入警告）；APK 构建成功（22MB）。
- **真机未复测**：验证期间 RMX3300 已从电脑断开（`adb devices` 为空，本机未安装模拟器），因此没有真机结论；手机接回后需复看六项排布与两枚玻璃页签。

### 新手指引输入完成即识别运营商，安卓悬浮栏适配窄屏（2026-11-12，双端）

**为什么**：此前运营商绑定状态只在账号步点击「下一步」后查询，用户输入完账号密码仍看不到自动选择；若简单增加输入监听，又会与点击下一步、进入绑定步骤形成重复查询。真机（RMX3300，1440×3216、密度 640）上 WebView 宽度为 360 CSS px，底部五个固定 64px 导航项加间距至少需要 364px，而实际内容宽约 324px，导致「更多」被挤到屏幕右缘。

**改了什么**

- **双端输入完成自动判定**：账号密码齐备后采用 500ms 尾沿防抖，输入停止才发起一次 query_bind_status；同一组凭据的自动检测与「下一步」共用单飞 Promise，已绑定运营商在账号步立即选中，点击下一步直接复用结果。输入变化会提升代次并使旧响应失效；未绑定结果进入绑定步骤复用，不再重复检测。
- **安卓悬浮栏按实际视口协调缩放**：`android/frontend/src/components/layout/BottomNav.tsx` 以 360 CSS px 为基准，将栏高、图标、文字、外边距、内边距与项间距统一改为带上下限的 `clamp()`；五项与放大层副本共用同一组变量。小屏不再只有项目变窄而图标/栏高不变，大屏也会在 64px 项高、30px 图标、13px 文字上限内放大；玻璃、镜头、点击与动画逻辑未改。
- **回归测试**：桌面 `OnboardingWizard.autoOperator.test.tsx` 新增「输入停止自动选择且下一步只查一次」「自动未绑定后进入绑定步骤不重复检测」两条用例。

**验证**（均为本轮实际输出）

- `npx tsc --noEmit --incremental`：桌面、安卓均 exit 0，零输出。
- `npx vitest run`（桌面完整套件）：23 个测试文件、136 条用例全部通过；其中新增运营商时机用例在修复前 2 条均失败，修复后全部通过。
- 将安卓真实 `useOnboardingFlow.ts` 原样临时复制到统一依赖测试域驱动：已绑定自动选择且下一步复用、未绑定进入绑定步骤不重复共 2 条均通过；临时文件已删除。
- `npx vite build`：桌面 `✓ built in 5.47s`、安卓 `✓ built in 6.71s`，仅既有 chunk 体积及安卓 faceService 混合导入警告。
- 响应式复测：浏览器 mock 320/360/390/412 CSS px 的导航栏高为 62.47/70.02/75.67/79.75px，图标为 23.09/25.98/28.16/29.73px，文字为 9.792/11.016/11.934/12.607px；五项均完整落在胶囊内，无溢出。安卓 `npx tsc --noEmit --incremental` exit 0；`npx vite build` `✓ built in 7.32s`。
- 完整响应式验证 APK 构建成功，22.43 MB 且签名校验通过；在 RMX3300 覆盖安装输出 `Success`，360 CSS px 视口下胶囊高 284 物理像素、外边距左右各 44px，五个导航项均为 248×228px。临时切换密度后应用停初始化，未作为通过证据；恢复原始 640dpi 并重启 Activity 后，导航几何与功能界面均恢复正常。
- 真机 RMX3300（1440×3216、密度 640）：隔离构建的 `Wxxy-CampusLogin_2.4.3.apk` 签名校验通过、22.43 MB，覆盖安装输出 `Success`。五项导航物理像素宽度由 256/256/256/256/212 改为 232/232/232/232/232，末项右边界由 1440 收至 1364，与首项左边界 76 对称；截图 `AppData\Local\Temp\android-emulator-plugin\screenshots\2026-11-12T06-08-36-412Z-758933b8.png` 目视确认五项均在胶囊内。未在真机输入或改写真实账号密码，运营商时机由上述双端测试验证。

---

### 夜间出站切换改为「提前 1 分钟预判定 + 到点只执行」：把切点的探测耗时前置，缩短断网空窗（2026-10-26，分支 feat/outbound-switch-precompute，桌面后端）

**为什么**：夜间出站切换的切点（周日至周四 23:00、周五周六 23:30）正是运营商断网的时刻，而改造前**所有判定都在到点那一拍才开始做**：逐张候选卡绑源 ICMP 探测（每张卡 × 网关/portal 各两次，用户实测数秒）+ 读跃点 + 算禁用名单，然后才提权写跃点、禁校园网卡。也就是说，用户在这几秒到几十秒里处于"校园网已断、出站还没切过去"的真空期，切得越慢掉线越久。用户要求：**切换前一分钟就开始判定，判定好了等系统时间到点再切**。

**改了什么**（4 个文件，桌面端；安卓端本无网卡切换动作，不受影响）

- **`tauri-app/src-tauri/src/config/outbound_switch.rs`**
  - 新增 `pub const OUTBOUND_PREP_LEAD_MINUTES: u32 = 1`（预判定提前量）。
  - 新增纯函数 `in_outbound_prep_window(enabled, weekday, now_minutes, restore_active, restore_start) -> bool`：判定是否处于 `[t-1min, t)` 这一分钟窗口。三条排除口径——功能关闭不预判定；已处切换态（三份快照任一非空）不预判定（还原侧不做优化，用户明确只优化切换侧）；`now < restore_start` 的**凌晨补切**分支排除（那一支的 Switch 语义是"错过的切换立即补"，没有提前量可言）。
  - 新增 3 条单测：窗口覆盖切点前一分钟（含周五/周六 23:30 口径）、功能关闭/已切换态不成立、与凌晨补切分支在真实取值范围内互斥（顺带守住 `restore_start_minutes(≥1380)` 归一化回落 450 的不变量）。
- **`tauri-app/src-tauri/src/config/scheduled_tick.rs`**
  - 新增 `evaluate_outbound_prep_with_restore(...)` 编排函数：在 `in_outbound_prep_window` 之上叠加与 `evaluate_outbound_action_with_restore` **同源**的「手动切换当日 hold 冻结」。动机是同源复用前置条件——否则会出现「预判定算了但到点因 hold 不切」（白耗一次探测）或反过来「hold 已过预判定没算」（退化成改造前的现场判定）。
  - 新增 3 条单测：窗口内成立、非窗口时刻不成立、hold/关闭/已切换态冻结。
- **`tauri-app/src-tauri/src/monitor/outbound_night.rs`**
  - 新增 `struct OutboundSwitchPlan`（目标卡 name/guid、跃点行快照、待禁用校园网卡名单、空名单告警标志、外网探测结论、生成时刻、当日），全部字段来自**只读**查询，不含任何系统写入。
  - 新增进程内内存槽位 `OUTBOUND_PREPARED_PLAN: Mutex<Option<OutboundSwitchPlan>>`——**不落盘**：计划只是短时效加速器，落盘会让陈旧计划跨重启复活；重启后由现场判定兜底。
  - 抽出 `build_outbound_plan(app, config, notify)`：把改造前 `apply_outbound_switch` 里的选卡/探测/读跃点/算禁用名单整段抽成独立函数，加 `notify` 参数区分告警时机（预判定阶段只记日志，避免提前一分钟弹一次、到点再弹一次）。
  - 新增 `prepare_outbound_switch(app, config)`（预判定入口）：退避窗内跳过；失败**不覆盖**已有计划（窗口内两拍 30s 间隔，后一拍失败时保留前一拍的成功结论）；每拍重算后者覆盖前者（越接近切点越可信）。
  - 新增 `take_prepared_plan()`（一次性 take 消费 + 保鲜期 `OUTBOUND_PLAN_MAX_AGE_MS = 180_000` 与跨日失效）与 `plan_target_still_valid()`（到点复核目标卡是否仍在网、guid 命中且有 IP）。
  - `apply_outbound_switch` 改为三段式：① 优先 `take_prepared_plan`，缺失/过期/目标卡已不在网则回落 `build_outbound_plan(.., true)` 现场判定（**凌晨补切走的正是这条路径**）；② 跃点现值复核——预判定到切点这 1 分钟里跃点可能被外部改动（用户手动设置、虚拟网卡重装协议栈），还原快照以执行时刻现值为准，读不到才回落到计划缓存原值；③ 诊断类提醒（外网不可达/空禁用名单）改在**执行阶段**弹。之后的落盘、写跃点、禁卡、路由验证逻辑不变。
- **`tauri-app/src-tauri/src/monitor/scheduled.rs`**
  - 在 `NightOutboundAction::None` 分支内接入预判定拍：满足窗口就经 `run_outbound_blocking` 调 `prepare_outbound_switch`（阻塞探测不能跑在 async worker 线程上，沿用既有线程约束）。放在 None 分支是正确的——窗口内本就不该有 Switch/Restore 动作，判定函数与窗口判定用的是同一套前置条件。
  - 30s 一拍、窗口 1 分钟 → 窗口内落 2 拍，符合"每拍重算"的设计。

**行为不变的部分**（刻意保留）：切换动作本身的落盘顺序（快照落盘成功才认定切换态成立）、跃点写入、校园网卡临时禁用、路由级验证、退避重放、失败计数、当日提醒节流全部未动；还原侧（`apply_outbound_restore`）一行未改。计划缺失时行为与改造前完全一致（保底走现场判定）。

**验证**（均为当轮实际输出）

- `cargo check --lib`（`tauri-app/src-tauri`）：`Finished dev profile`，exit 0。
- `cargo check --all-targets`（`tauri-app/src-tauri`）：`Finished dev profile ... in 4.08s`，exit 0，**零警告**。
- `cargo test --lib`（`tauri-app/src-tauri`）：`test result: ok. 563 passed; 0 failed; 5 ignored`（新增 6 条用例，基线 557 → 563）。
- `cargo test` 全量：`563 passed / 0 failed / 5 ignored`（lib）+ 集成 `2 passed`、`3 passed`、`1 passed`，全部 exit 0。
- 安卓交叉编译（`android/src-tauri`，NDK 27.0.12077973 的 `aarch64-linux-android24-clang` + `cargo check --target aarch64-linux-android --lib`）：`Finished dev profile ... in 57.01s`，exit 0——确认共享判定层（`campus-login` path 依赖）的桌面侧改动不影响安卓编译。
- **一次偶发失败的如实记录**：改动过程中有 1 次 `cargo test` 全量运行出现 `FAILED. 562 passed; 1 failed`，但随后连续 **48 次**运行（`cargo test --lib` 40 次 + 全量 8 次）全绿、无法复现，且未捕获到失败用例名与 panic 信息。本次新增的 6 条用例均为零 IO / 零时间依赖的纯函数断言，不可能产生非确定性，判断为两个测试目标并行执行时的既有资源竞争而非本次改动引入；如实记录供后续排查，不作为「全绿」结论的一部分。
- 未做真机夜间切换实测（需等到 23:00 断网时刻）；未跑前端类型检查与 `vite build`——本次无任何前端改动。

**安卓端**：未改。安卓的 `night_outbound_restore` 承载的是「注销校园网 → 等系统切移动数据」这一动作，没有桌面端"多网卡选卡 + 绑源 ICMP 探测 + 提权写跃点/禁卡"的耗时环节，因此没有可前置的计算；配置字段未新增、IPC 面无变化，Android 经 Cargo path 依赖继承的共享判定层也未受影响（新增的 `in_outbound_prep_window` / `evaluate_outbound_prep_with_restore` 安卓不调用）。

### 四处视觉/交互缺陷修复：运营商下拉与输入框统一白底、新手指引卡左侧裁切、主题自定义色真正可取、玻璃默认改极致（2026-10-28，分支 fix/ui-dialog-select-onboarding-theme，双端）

**为什么**：用户贴了 4 张截图指出四件事——① 三张卡片（添加登录信息 / 绑定运营商 / 编辑登录信息）里「运营商」下拉是灰块，而同卡片上方的学号、密码是白框，读起来像两套控件；② 新手指引第 2 步「账号信息」卡片的**左边框被略微截断**，右边却有正常留白；③ 主题设置里选了「自定义」配色后**没有任何地方能调颜色**，主色恒为默认蓝紫；④ 玻璃质感的默认档位希望是「极致」而不是「标准」。

**根因**

1. **运营商下拉灰底**：`components/ui/select.tsx` 的 `SelectTrigger` 用 `bg-muted/40`（半透明灰）；而 `input.tsx` 早已统一为 `border border-input bg-card`（白底细边）。浅色模式下 `bg-muted/40` 与弹窗底几乎同明度，视觉上只剩一块灰。
2. **引导卡左侧裁切**：卡内容器原为 `w-[760px] max-w-[calc(100vw-32px)]`，外层是 `fixed inset-0 flex items-center justify-center` **无水平内边距**的容器。`100vw` 按初始包含块计算，在 WebView2 高 DPI/缩放下会与真实布局宽不一致 → `max-w` 失效 → 760px 固定宽超出容器，居中后左右同时溢出，左侧又被 `index.css` 的 `html{overflow:hidden}` 裁掉，于是「左边贴死、右边还留着」。实测（本地 vite + Tauri IPC mock + 无头 Chrome，1069×868）：修复后 host 宽 760、card 左边界 172px > 0；`device-scale-factor` 取 1.25/1.5/1.75 三档复测，card 左边界分别为 173/223/175，均 > 0，不再触及视口左缘。
3. **自定义色不可调**：`ThemeDialog` 只渲染 7 个配色按钮 + 3 档玻璃 + 浅色开关，**从来没有渲染任何颜色控件**——store 的 `setCustomThemeColor`、`useSettings()` 导出的 `customThemeColor`、i18n 键 `settings.customPrimaryColor`（中英文都有）全都存在却无人调用。选中「自定义」后主色就是写死的 `#6366f1`。
4. **玻璃默认档**：`useThemeStore` 初值、首帧脚本 `index.html`、`main.tsx` 的 `initTheme` 三处兜底都写 `'standard'`。

**改了什么**（桌面 `tauri-app/frontend` + 安卓 `android/frontend` 各一份，同一次提交）

- **`components/ui/select.tsx`（两端同改）**：`SelectTrigger` 基类由 `bg-muted/40` 改为 `border border-input bg-card`，`hover:border-input/70`，与 `input.tsx` 同一套语汇；附中文注释记下「半透明灰在同一张卡里读作两套控件」的教训。这一处修完，所有 `Select` 使用点（添加/绑定/编辑登录信息、引导流程、网络适配器、设置页、日志页等）一并统一。
- **`settings/OnboardingWizard.tsx`（两端同改）**：外层 `fixed inset-0 ...` 容器补 `px-4`（原只有 `py-4`）；`DialogPrimitive.Content` 由固定的 `w-[760px] max-w-[calc(100vw-32px)]`（安卓为 `w-[min(760px,94vw)]`）改为 `w-full max-w-[760px]`——宽度上限改由 flex 父容器的真实布局宽决定，任何 DPI/缩放/窄窗下都不会溢出。卡堆高度与层位算式（`deckCardHeight × 1.05 + 64`、`deckTransform`）未动，切步高度不跳的行为不变。
- **`settings/ThemeDialog.tsx`（两端同改）**：新增自定义主色调控件，仅当配色为 `custom` 时展开——左侧是原生 `<input type="color">`（外观由外层 label 决定的 28×28 色块，原生控件缩成 0 尺寸透明覆盖其上）+ 当前 hex 文本，下方 8 个预设色块（`#6366f1 / #3b82f6 / #06b6d4 / #22c55e / #f59e0b / #ef4444 / #ec4899 / #8b5cf6`）。取值统一走 `applyCustomColor(hex)`：先写 store（`useThemeStore.subscribe` 立即重算 `--primary/--ring/--accent` 等 HSL 变量，弹窗内即时预览）再经新增的可选 prop `onSetCustomColor` 回写到 config 持久化。`color input` 只接受 `#rrggbb`，对历史配置里的空串做兜底。
- **`App.tsx`（两端同改）**：给 `ThemeDialog` 接上 `onSetCustomColor={(hex) => updateConfig({ customThemeColor: hex })}`——`useConfigStore.updateConfig` 既做 store 同步又有 500ms 防抖落盘，所以拖一次取色器不会刷爆配置文件。
- **玻璃默认档（两端各三处，共 6 处）**：`hooks/useThemeStore.ts` 初值、`index.html` 首帧脚本、`main.tsx` 的 `initTheme` 兜底全部由 `'standard'` 改为 `'ultra'`；首帧脚本的判断从「只认 off/ultra、其余算 standard」改为「只认 off/standard、其余算 ultra」，老用户显式选过的档位仍然生效。

**验证**（均为当轮实际输出）

- `npx tsc --noEmit --incremental`（`tauri-app/frontend`）：exit 0，零输出。
- `npx tsc --noEmit --incremental`（`android/frontend`）：exit 0，零输出。
- 浏览器实测（本地 vite + Tauri IPC mock + 无头 Chrome，用后已完整还原）：
  - 添加登录信息弹窗：`combobox` 背景 `rgb(255,255,255)`、边框 `rgb(203,207,215)` 1px，与同弹窗 `input` **完全一致**（修复前为半透明灰）。
  - 绑定运营商弹窗：`combobox` `rgb(255,255,255)` / 1px 边框，同样与 `input` 一致。
  - 主题设置（选中自定义）：`input[type=color]` 存在、值 `#6366f1`；点预设 `#22c55e` → `--primary` 由 `239 84% 67%` 变为 `142 71% 45%`，hex 文本与取色器值同步；再模拟拖取色器到 `#ec4899` → `--primary` 变 `330 81% 60%`，两条路径都真正生效。
  - 引导卡几何（1069×868，第 2 步）：`host:144,68,760,634 | card:172,115,702,570`，左边界 172 > 0；`device-scale-factor` 1.25/1.5/1.75 三档复测得 card 左边界 173/223/175，均未触及左缘；窄窗 900/760/720 宽下 `host` 宽度随视口收缩（760/706/666），无溢出。
  - 截图 `build/shots/onboard_step2.png` 目视确认：卡片左侧留白与右侧对称，圆角完整。
  - 首帧 `data-glass=ultra`（清空 `campus-glass-level` 后读取 `document.documentElement` 属性验证）。
- 未做：真机 WebView2 高 DPI 打包实测（无头 Chrome 只能近似），未跑 `npx vite build`。

**安卓端**：四处改动全部同步（`select.tsx`、`OnboardingWizard.tsx`、`ThemeDialog.tsx`、`App.tsx`、`useThemeStore.ts`、`index.html`、`main.tsx`）；`ThemeDialog` 的 `onSetCustomColor` 在移动外壳里同样接到 `useConfigStore.updateConfig`，无 IPC 面变化、无新增配置字段——`customThemeColor` 后端字段（`config/model.rs:137-138`，默认 `#6366f1`，`config/validate.rs:97` 用 `CUSTOM_COLOR_RE` 校验）两端本来就共用。

> 2026-11-13 21:40 本文件在一次整文件重写中被截断（原 904 行）。第 10 条「四处视觉/交互缺陷修复」及其之前的条目已按原样恢复；10-28 之前的更早条目未能从会话或编辑器历史找回，若从备份恢复请补齐。
