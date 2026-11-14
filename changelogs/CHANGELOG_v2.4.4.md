### 网络适配器页：适配器/账号与检测参数各收进弹窗，公网探测目标可配（2026-11-14，桌面）

**为什么**：网络适配器卡片里「适配器选择 + 账号绑定」是一整块常驻表单，和卡片主体（适配器清单）抢版面——用户打开这个页面主要是看卡，改主/副适配器是低频操作。同时公网可达性检测目标此前硬编码为 `www.baidu.com` / `cn.bing.com`（另加 223.5.5.5:53 一路），代理/TUN 把域名解析成 fake-ip 或学校出口针对特定域名异常时，用户没有换目标的入口，只能等默认目标碰运气。

**改了什么**

- 前端（`tauri-app/frontend`）
  - 新增 `network/AdapterSettingsDialog.tsx`：主/副适配器与各自登录账号从卡片内联表单搬进弹窗，选择即时生效，底部只留「关闭」。
  - 新增 `network/ProbeSettingsDialog.tsx`：公网可达性检测目标（主机名，最多 4 个，逗号/换行分隔，留空=内置默认）与状态检测间隔（10–3600 秒，绑定 `backgroundCheckInterval`），带格式校验、打开时重置草稿。
  - `NetworkPanel.tsx`：卡片底部改为两枚入口按钮（适配器与账号 / 检测参数），与账号管理卡卡尾同构；删除内联表单及随之失效的导入。
  - `settings/types.ts`、`settings/constants.ts`：新增 `internetProbeHosts: string[]`（默认空）。
  - i18n 中英文案各 14 行。
- 后端（`tauri-app/src-tauri`）
  - `config/model.rs`：新增 `internetProbeHosts`（`Vec<String>`，默认空 = 内置默认目标）。
  - `config/validate.rs`：新增 `normalize_internet_probe_hosts`（trim / 剔空 / 去重 / 上限 4 项；含协议、端口、斜杠、空白即报错）。严格校验与宽松校验都接入，宽松侧失败回退空列表，避免单个非法项触发兜底的「整份配置换默认」。
  - `network/quality.rs`：`check_internet_access_with_if` 增加 `probe_hosts: &[String]` 参数，空列表回落 `DEFAULT_INTERNET_PROBE_HOSTS`；逐目标绑源 HTTPS 探测逻辑不变。
  - `monitor/outbound_failover.rs`：`probe_internet_access` 把配置目标 clone 进 spawn（`JoinSet` 要求 `'static`）。
  - `network/adapter.rs`：测试用 `Config` 字面量补新字段。

**验证**：桌面 `npx tsc --noEmit --incremental` 通过；`npx vite build` 成功；后端 `cargo test --lib` 641 passed / 0 failed / 3 ignored（含新增 4 个探测目标校验用例）；浏览器实测（`vite --config vite.mock.config.ts` + headless Edge，`?panel=network`）截图确认：卡片底部两枚入口按钮排版正常、「网络检测参数」与「适配器与账号」弹窗渲染正常、副适配器为空时副账号字段正确隐藏。**未实测**：真实系统上自定义探测目标后的夜间/故障切换端到端行为（需新构建）。

**双端说明**：适配器选择与公网探测为桌面专属（安卓无多网卡与出站切换概念）；安卓前端配置类型补 `internetProbeHosts?` 仅为保持两端类型树同构，安卓端不消费该字段。

---

### 出站条目收口：修复跃点编辑态间距、图标改路由语义、三行按钮统一（2026-11-14，桌面）

**为什么**：上一版把跃点做成行内 chip 后有三处不自洽——①编辑态输入框与「切换出站」之间莫名空出一大段；②WLAN 与以太网的操作按钮集合不一致；③跃点用了仪表盘（Gauge）图标，而跃点表示路由开销/优先级，不是速度。

**改了什么**

- 间距根因：共享 `Input` 组件外层固定包了一层 `div.relative.w-full`，放进按钮行会吞掉剩余宽度并把后面的按钮推开。编辑态改用原生 `<input>`（自带边框/聚焦态样式），宽度严格 64px，数值 `text-center` 居中显示。
- 跃点图标 `Gauge` → `Route`（路由路径语义）；同时移除 hover 才浮现的铅笔，chip 本身即编辑入口，说明放在 tooltip。
- 三行按钮统一：只要有 IP 就渲染「切换出站」，当前出站行置灰（title 显示「当前出站」），夜间开关关闭时同样置灰。

**验证**：`npx tsc --noEmit --incremental` 通过；CDP 实测编辑态 input 右边缘 674px、「切换出站」左边缘 682px（间距 8px，即 gap-2）；截图确认三行都含「跃点 chip + 切换出站 + 获取新IP」。

---

### 出站面板精简：删除头部两个按钮、跃点改为行内 chip、条目按钮改名「切换出站」（2026-11-14，桌面）

**为什么**：头部「按优先级切换 / 恢复原出口」与条目内的切换按钮语义重叠，用户要求删除；跃点此前是一个常驻输入框直接排在按钮堆里，既不像接口属性也不像操作控件，整体与条目设计语言不符。

**改了什么**

- 删除头部「按优先级切换」「恢复原出口」两个按钮，同步移除 `useOutboundManual` 里的 `handleOutboundSwitchNow` / `handleOutboundRestoreNow`、`switchingOutbound` / `restoringOutbound` 状态与 `tauriApi` 的两个封装；后端 `outbound_switch_now` / `outbound_restore_now` 命令保留（命令面不变，当前无 UI 入口）。
- 跃点改为与行内按钮同排的轻量 chip（`MetricChip`）：浅底圆角、仪表图标 + `跃点 30`，点击原地变输入框（回车/失焦保存、Esc 取消，1–9999），与「切换出站」「获取新IP」同一视觉层级。
- 条目按钮「立即切换出站」改名「切换出站」。
- 文案同步：`nightOutboundSwitchDesc` 去掉「也可手动还原」，`outboundDragHint` 去掉已删按钮的引用。

**行为影响**：还原只剩夜间自动流程（次日恢复时刻），不再有手动还原入口；切换态下若想提前恢复，目前只能等自动还原。

**验证**：`npx tsc --noEmit --incremental` 通过；`npx vite build` 成功；浏览器实测（`vite --config vite.mock.config.ts` + headless Edge + CDP）确认头部只剩开关、chip 常态排版、点击后原地出现聚焦输入框（值 30）、按钮改名生效。**未实测**：真实系统上的 `set_adapter_metric` 与条目「切换出站」端到端效果（需新构建）。

**双端说明**：桌面专属 UI，安卓端无出站面板，未做双端同步。

---

### 出站优先级可控：按钮改名、条目级「立即切换出站」、跃点显示与编辑、恢复时按排序重排（2026-11-14，桌面）

**为什么**：用户早上发现 07:50 还原后出站是「以太网 2」而不是「以太网」，而网络适配器列表第一项是 WLAN——列表顺序只决定夜间切换目标，不参与出站判定，用户无法通过排序控制出站。同时「切换出站/还原出站」两个头部按钮与要新增的条目级「立即切换出站」语义撞名，且界面上看不到、也改不了网卡当前跃点。

**改了什么**

- `monitor/outbound_night.rs`
  - 新增 `apply_outbound_priority_metrics`：按 `outboundPriority` 顺序把有 IP 的网卡跃点重排为 1..N（IPv4/IPv6 同步写），排第一且有 IP 的卡成为系统出站。
  - `apply_outbound_restore` 收尾调用它；`manual_outbound_restore` 在非切换态下改为执行它（原实现直接报「当前未处于出站切换状态」，而用户此时正是想让排序生效）。
  - `build_outbound_plan` / `apply_outbound_switch_with_health` 增加 `forced_target`：条目「立即切换出站」以指定卡为目标，跳过优先级选卡与全部探测（只要求有 IP 与 GUID）。
  - 新增 `manual_outbound_switch_to`（校验开关/切换态/是否已是当前出口，成功后冻结当日自动动作）与 `manual_set_adapter_metric`（单卡跃点 1–9999；切换态下拒绝——快照原值会在还原时覆盖用户修改）。
- 命令面：新增 `outbound_switch_to` / `get_adapter_metrics` / `set_adapter_metric`（`app/startup.rs` 注册）。
- 前端（`NetworkPanel.tsx` / `useOutboundManual.ts` / `tauriApi.ts` / `types.ts`）
  - 头部按钮改名：「切换出站」→「按优先级切换」，「还原出站」→「恢复原出口」。
  - 每行新增「跃点」输入框（显示当前 IPv4 跃点，回车/失焦提交，1–9999）与「立即切换出站」按钮（有 IP 且非当前出口时显示，开关关闭时禁用）。
  - 拖拽提示改为「排最前且有 IP 的卡在恢复原出口或夜间还原后成为出站（跃点按此顺序重排）」。
- `tauri-mock.js` 补三个命令的 mock（浏览器实测用）。

**语义变化（排查要点）**：还原不再只是「写回切换前快照」——收尾会按列表顺序重排全部有 IP 卡片的跃点，列表第一即出站。拖动排序本身不即时改系统跃点，需点「恢复原出口」或等夜间还原。

**验证**：桌面 `cargo test --lib` 637 passed / 0 failed / 3 ignored；`npx tsc --noEmit --incremental` 通过；`npx vite build` 成功；浏览器实测（`npx vite --config vite.mock.config.ts` + headless Edge 截图 `?panel=network`）确认三行布局、跃点显示、按钮显隐与改名生效。**未实测**：真实系统的「立即切换出站」与还原后按排序重排（需安装新构建；切换态下改跃点会被后端拒绝）。

**双端说明**：本次为桌面专属能力（Windows 接口跃点与出站切换），安卓端无出站切换 UI 与跃点概念，未做双端同步。

---

### 夜间出站切换恢复「到点即切」、切换后断开旧出口连接、切换耗时压到 2s 内（2026-11-14，桌面）

**为什么**：三件事叠在一起，用户实测（2026-11-13 夜）暴露得很清楚：

1. **夜切不执行，只剩还原时间**。`33a452b4`（按出口健康自动切换）把 `NightOutboundAction::Switch` 的动作体改成了空 `{}`，注释写「夜间 Switch 只是额外触发，不再直接强制切卡」——于是 23:00/23:30 到点什么都不做，要等故障探测连续两轮失败（约 1 分钟）才切。日志证据：23:30:19 才开始探测、23:30:52 判「连续 2/2 轮」，23:31 用户手动点切换才真正切走。同一提交还把界面文案从「夜间出站切换 / 周日至周四 23:00、周五六 23:30 …」改成「自动出站切换 / 持续检测各网卡公网连通性…」，界面上一眼看不出还有定时切换。
2. **切了出口，已建立的连接不断**。切换只改接口跃点，既有 TCP 连接的 TCB 仍钉在原网卡上：原卡（校园网）断网后这些连接只会僵死到重传超时，应用层既收不到网络切换信号、也不会重连到新出口——用户看到的就是「出口切了但应用没恢复」。
3. **切换耗时 4141ms**（`[outbound] 夜间出站切换: 动作完成，总耗时 4141ms`）。逐段拆日志：并行绑源探测 590ms + 公网健康探测 **3000ms**（两断网卡各走满 3s HTTPS 超时）+ 快照落盘 340ms + set_metric 150ms + Portal 旁路 180ms。

**改了什么**

- `monitor/scheduled.rs`：`NightOutboundAction::Switch` 恢复执行 `apply_outbound_switch_with_health(.., None)`——到点即切，不等当前出口探测失败；失败（无候选/helper 退避）时快照可能已落盘，下一拍转 None 走补齐分支续写。
- `monitor/outbound_failover.rs`：`evaluate_switch_back` 增加夜间守护窗口门控（复用 `config::outbound_switch::is_night_outbound_guard_window_with_restore`）。不加会把夜切变成「切了又切回」：到点切换后校园网可能还没断（学校断网时刻不精确）或探测偶发误判，两轮「健康」就切回。白天故障切换后原卡恢复仍照常自动切回。
- `platform/tcp_conns.rs`（新增）：`GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)` 枚举本机 IPv4 TCP 连接，对本地地址属于原出口卡的行走 `SetTcpEntry`（`MIB_TCP_STATE_DELETE_TCB`，对两端发 RST）。`SetTcpEntry` 对任意进程的 TCB 生效、需要管理员，因此走提权 helper。
- `helper/mod.rs`：新增 `close_tcp_conns` op（参数为本地 IPv4 列表，逐行删 TCB，单行失败不中断），`--helper` 与 `--helper-task` 两条分发路径同步登记。
- `monitor/outbound_night.rs`：切换动作在路由验证通过后调用 `close_tcp_conns`（失败只告警，不影响切换结果）；`build_outbound_plan` 把绑源网关探测与公网健康探测并行发起（原先串行，切点耗时是两者之和）；切换路径的公网探测超时 3s → 1s（`OUTBOUND_SWITCH_HEALTH_TIMEOUT`，正常卡实测 640ms 内完成，断网卡必然走满超时）。
- `network/quality.rs` / `monitor/outbound_failover.rs`：`check_internet_access_with_if` 与 `probe_internet_access` 增加超时参数——故障切换（30s 一拍）仍用 3s 保证判定准，只有切换动作吃 1s。
- `monitor/outbound_failover.rs` 的登录恢复日志从「当前出口 X 开始登录恢复」改为「登录恢复: X 开始 Portal 复核」：该函数被故障路径与切换态原卡轮转两条路径共用，旧文案会把「正在恢复的原卡」误报成当前出口（排障时被误导）。
- 双端文案对齐：桌面 `nightOutboundSwitch` 恢复为「夜间出站切换」、描述恢复时段（周日至周四 23:00、周五六 23:30）并写明故障兜底，`outboundBadge`/`outboundDragHint` 同步回「夜间出站目标/优先级」；安卓文案本来就是夜间语义，未改。

**耗时预期**：1.0s（探测上限，与 590ms 网关探测并行）+ 0.34s（快照落盘）+ 0.15s（set_metric）+ 0.15s（close_tcp_conns）+ 0.18s（Portal 旁路）≈ 1.8s，较 4141ms 降一半以上。

**验证**：桌面 `cargo test --lib` 637 passed / 0 failed / 3 ignored（新增 `close_tcp_conns_round_trip` 与 `platform::tcp_conns` 的两项真机取表/空列表测试）；`npx tsc --noEmit --incremental` 通过；`npx vite build` 成功。**未实测**：真实夜切端到端（需挂机到 23:00 或次日 07:30）与连接关闭的实际条数——下次切换日志应出现「已关闭原出口 X(ip) 上的旧 TCP 连接，耗时 Yms」与总耗时 <2000ms，可据此复核。

---

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

> 2026-11-13 22:05 以下条目从 `CHANGELOG_v2.4.3.md` 与 `CHANGELOG_v2.4.2.md` 恢复（原 v2.4.4 在一次整文件重写中被截断，未能逐字还原的部分以历史版本文件为准）。
### 校园网判定改为逐卡短路 + 绑源探测：修掉「路由器 NAT 下判非校园网」与「无 IP 卡蹭结论」（2026-10-26，分支 fix/campus-per-adapter-judge，桌面后端+桌面前端）

**为什么**：用户真机反馈两个缺陷，根因同源——`check_campus_network` 是**按类聚合**判定（`wifi`/`wired` 两类各出一个结论），且网关/Portal 探测走 `check_gateway_reachable`（**不绑源**，多网卡下从哪张卡出去不受控）。

1. **路由器 NAT 被误判非校园网**：用户电脑经路由器上网，网卡 IP 是路由器私网 `192.168.6.114`，路由器本身登录了校园网。旧实现只看本机 IP 与 `campus_gateway`（`10.2.127.254`）是否同 `/18` 网段——私网地址必然不同段，于是「网段」这一层就否了；后续网关/Portal 探测又不绑源，探出去的结果无法归因到这张卡，最终判「非校园网」。用户明确要求改成：**名称 / 网段 / 网关 / Portal 任一命中即算校园网，按探测成本从低到高排序逐个判定，网关与 Portal 都失败才判非校园网**。
2. **无 IP 卡蹭同类的结论**：旧实现 `wifi`/`wired` 整类只要有一张卡有 IP 且判定成功，成功文案就被套到该类**每一张**卡上（含无 IP 的卡）。用户截图里「以太网 2 / 无IP地址 / 有线通过网关10.2.127.254连接校园网」的错乱归因就是这么来的。

**改了什么**

- **`tauri-app/src-tauri/src/monitor/campus_check.rs` 整文件重写**：
  - 新增 `AdapterCampusStatus { name, wireless, on_campus, matched_by, message }` 逐卡结果与 `CampusCheckResult.adapters` 明细字段；`matched_by` 记录命中的判定腿（`name`/`subnet`/`gateway`/`portal`，全失败为 `None`），便于排障与前端标注。
  - 新增 `judge_adapter()`：单卡按成本顺序短路——① 无 IP 直接判非（任何探测都绑不上它的源地址，也本就无法通信，杜绝蹭结论）→ ② 名称（netsh 结果已缓存，零网络 IO）→ ③ 网段（纯位运算，零 IO）→ ④ 网关（绑该卡源的一次探测，超时 2s）→ ⑤ Portal（绑该卡源）→ 全失败才判非。
  - **名称腿按卡类型套用**：SSID 只对无线卡生效、有线 profile 只对有线卡生效（`netsh wlan` 只报当前连接的一个 SSID，套到有线卡上是错归因）。
  - **探测改为绑源**：`probe(target, source)` 走 `check_gateway_reachable_from(target, Some(该卡IP))`，同一 `(target, source)` 维度加缓存避免重复探测。这是修掉 NAT 场景的关键——从 `192.168.6.114` 这张卡探 `10.2.127.254`，路由器转发后可达即命中「网关」腿。
  - `wifi`/`wired` 旧字段保留为**兼容聚合**（该类任一卡命中即 `on_campus`，文案优先取命中卡），IPC 契约不破坏；同时新增 `adapter_campus_status`/`adapter_campus_message` 按适配器名精确取用，`background_check.rs` 的 `a1/a2` 归因随之改为逐卡，不再按类推断。
- **IPC 面新增逐卡明细**：`check_campus_status` 增 `adapters` 字段（`commands/network_cmd.rs`），后台巡检事件增 `campusAdapters` 字段（`monitor/background_emit.rs`）。
- **桌面前端消费逐卡结论**（`useAuthStore.ts` / `useEventListeners.ts` / `tauriApi.ts` / `monitor/types.ts`）：卡片文案与 `onCampus` 优先取该卡自己的判定，旧版后端无该字段时才回退整类聚合——避免「无 IP 卡显示成功文案」的错乱。
- **安卓端未改**：安卓 `campus_detect::pick_campus_source_ip` 本就选单一出口 IP 并绑源探测，不存在多卡聚合与不绑源问题。
- **死代码清理**：删掉 `tauri-app/src-tauri/src/network/subnet.rs` 的 `check_gateway_reachable(gateway)` 薄包装（= `check_gateway_reachable_from(gateway, None)`）与 `network/mod.rs` 的对应 re-export——本次重构后全部调用点改为绑源 `check_gateway_reachable_from`，该包装零调用方，触发 `unused import` + `dead_code` 双警告。另把 4 个新增测试函数名里的 ASCII 大写（`IP`/`Portal`/`None`）改小写，消除 `non_snake_case` 警告，中文命名风格不变。

**验证**（均为当轮实际输出）

- 桌面 `cargo test --lib`（`tauri-app/src-tauri`）：`test result: ok. 557 passed; 0 failed; 5 ignored`，exit 0（含本次新增 8 条逐卡判定用例：无 IP 不探测、名称按卡类型套用、网段零探测短路、路由器私网绑源命中、Portal 兜底、全失败判非、禁用名称检查、聚合文案优先级）。
- 桌面 `npx tsc --noEmit --incremental`（`tauri-app/frontend`）：exit 0。
- 桌面 `npx vite build`（`tauri-app/frontend`）：`✓ built in 10.75s`，exit 0（仅既有 chunk >500kB 提示，非新增警告）。
- 桌面 `cargo check --all-targets`（`tauri-app/src-tauri`）：`Finished dev profile`，exit 0，**零警告**（清理前有 `unused import: check_gateway_reachable` + `dead_code` + 4 条 `non_snake_case`）。

**用户场景复核**：`以太网 2`（`192.168.6.114`，路由器 NAT 后接校园网）——网段腿不同段跳过，网关腿绑源探 `10.2.127.254` 可达即判校园网，文案为「通过网关10.2.127.254连接校园网」，不再误判。

---

### 打包脚本只出 Windows 包、版本号按实际产物校验（2026-10-26，分支 fix/campus-per-adapter-judge，构建脚本）

**为什么**：用户反馈「已经升级版本号了，脚本打出来的还是 v2.4.1」。排查后确认脚本本身**没有写死版本号**（`make-release.ps1:36-40` 从 `tauri-app/src-tauri/tauri.conf.json` 的 `version` 读），真实原因有两个：① 版本升级提交 `de9751c8` 只在 `chore/release-v2.4.2` 分支上，main 仍是 2.4.1；② 脚本取安装包用的是「bundle 目录里 LastWriteTime 最新的那个 `*-setup.exe`」，配置版本改了却没重新编译时，会把旧版安装包静默改名成新版本号发布——这是个会发错包的隐患。另用户要求脚本不要再把安卓 APK 一起收进 `release/`。

**改了什么**（`make-release.ps1`）

- **产物按版本号精确匹配**：遍历候选 target 目录的 `release/bundle/nsis`，用 `$_.BaseName -match "_${version}_[A-Za-z0-9]+-setup$"` 只挑版本号对得上的包；匹配不到且目录里只有旧版本时直接报错退出，文案点明「版本号与配置不一致，请重新编译（去掉 -SkipBuild），不要把旧包改名成新版本号发布」。
- **删除安卓 APK 收集**（原 95-110 行整段）：脚本只负责 Windows 桌面端，安卓出包走 `android/build-apk.ps1`；两端产物分属不同 Release 渠道，混在一起会让 Windows 用户下到 APK。
- **收尾清单只列本次三项资产**（安装包 / `.sha256` / notes），目录内历史文件另起一行提示数量、不计入本次资产。
- 头部 `.DESCRIPTION` 重写，把「仅构建 Windows 端」「版本号以实际产物为准并交叉校验」写成显式约定。

**验证**（均为当轮实际输出）

- `[System.Management.Automation.Language.Parser]::ParseFile(...)` → `SYNTAX OK (693 tokens)`。
- `pwsh -File make-release.ps1 -SkipBuild`（版本仍为 2.4.1 时）→ 成功打出 `Wxxy-CampusLogin_2.4.1_x64-setup.exe`（源自 `校园网登录助手_2.4.1_x64-setup.exe`）+ `.sha256`，收尾列 3 项资产并提示「另有 11 个历史文件未计入本次资产」。

---

### 版本号全量升级 v2.4.1 → v2.4.3（2026-10-26，分支 fix/campus-per-adapter-judge，双端）

**为什么**：本版含校园网判定重构（行为变化，用户可感知），按语义化版本走次版本位；2.4.2 的版本号升级提交 `de9751c8` 尚未合入 main，本版直接从 2.4.1 跳到 2.4.3。

**改了什么**（17 个文件，权威源 `tauri-app/src-tauri/tauri.conf.json` 的 `version`）

- 版本字段：`tauri-app/src-tauri/tauri.conf.json`、`tauri-app/src-tauri/Cargo.toml`、`tauri-app/package.json`、`tauri-app/frontend/package.json`、`android/src-tauri/tauri.conf.json`、`android/src-tauri/Cargo.toml`、`android/frontend/package.json`。
- 前端常量：`tauri-app/frontend/src/shared/ui-constants.ts`、`android/frontend/src/shared/ui-constants.ts` 的 `APP_VERSION = '2.4.3'`。
- 锁文件：三份 `package-lock.json` 各 2 处、两份 `Cargo.lock`（`campus-login` / `campus-login-android`）。
- 展示位：`tauri-app/frontend/about-preview.html`（2 处 `v2.4.3`）、`README.md` 徽章、`version.json`（版本 + notes 按本版重写）。
- **保留不动的 2.4.1 残留**：`tauri-app/package.json` 的 `@tauri-apps/plugin-global-shortcut: ^2.4.1`（依赖约束，上次误改已还原）、锁文件里的第三方包自身版本、`THIRD-PARTY-NOTICES.md` 的 fastrand 2.4.1、`android/src-tauri/gen/android/.../assets/tauri.conf.json`（构建产物，gitignore，构建时重生成）。

**验证**（均为当轮实际输出）

- `cargo check --all-targets`（`tauri-app/src-tauri`）：`Finished dev profile ... in 14.15s`，exit 0，零警告（编译日志显示 `Compiling campus-login v2.4.3`，确认版本号已生效）。
- `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0、安卓 `android/frontend` exit 0。
- `npx vite build`（`tauri-app/frontend`）：`✓ built in 5.34s`，exit 0。
- `version.json` JSON 解析校验通过。

---

### 发布说明融合：v2.4.2 + v2.4.3 合并为 `RELEASE_NOTES_v2.4.3.md`，并补齐 19 项遗漏（2026-10-26，分支 main，发布文档）

**为什么**：v2.4.2 从未正式发布——`git tag` 只到 `v2.4.1`，没有 v2.4.2 tag、也没有对应的 GitHub Release，但 `release/RELEASE_NOTES_v2.4.2.md` 已写好且 `changelogs/CHANGELOG_v2.4.2.md` 承载了 2026-10-06 之后几乎全部改动（121 个 `### ` 条目）。若直接发 v2.4.3 而只写 3 条校园判定/版本号内容，用户会漏掉整整一版的更新；用户要求把两份 release 说明融合成一份。

**改了什么**

- **新建 `release/RELEASE_NOTES_v2.4.3.md`**（144 行、11 个章节、99 条 bullet）：以 v2.4.2 notes 的 8 个章节为底稿全文保留，前置新增 `## 🛰️ 校园网判定改为逐卡短路 + 绑源探测（v2.4.3）` 章节，并在顶部加「关于本版本号」说明（v2.4.2 已完成开发但未发布，本次合并发布，从 v2.4.1 升级的用户下面全部是新的）。`release/RELEASE_NOTES_v2.4.2.md` 原文件保留不动，作为历史留存。
- **逐项对账 `changelogs/CHANGELOG_v2.4.2.md` 的 121 条与 v2.4.2 notes 的 8 章节**，凡 changelog 有、notes 完全未提或覆盖过粗的即补。共补 **19 项**，其中 9 项用户可感知缺陷：
  1. 悬浮栏登录/注销按钮点击被镜头手势抢走、标签页被切到最右端（`:1145`，双端，`lib/dock-lens.ts` 新增 `inItemStrip` 以 `[data-dock-strip]` 为边界，配 3 条回归测试）
  2. USB 网卡禁用后重启弹「正在安装设备驱动程序」（`:1345`，RTL8156 停在 PnP problem 22 / 出厂 CD-ROM 模式，Windows 当新大容量存储设备）
  3. 下拉选项悬停无背景高亮（`:1389`，双端，Radix Select 用 `data-highlighted` 而非 DOM focus，`focus:bg-accent` 永不命中）
  4. 定时注销在启动时可能立即触发（`:1789`，双端，`should_fire_scheduled_action` 是过点补触发语义，冷启动首拍必命中）
  5. 切入自助服务面板时验证码预热失败弹错误通知（`:1807`，双端，`fetchDashboard` 增 `opts.silent`）
  6. 时间输入框时钟图标被截断（`:1783`，桌面，`w-20` 80px vs 内容 82px，改 `w-24`）
  7. 浅色模式堆叠卡片顶部阴影包裹缺失（`:1795`，双端，两层 key 阴影 y 偏移全正、spread 全负；像素采样 254/255/255/255 → 248/250/251/252）
  8. 网络质量检测配置适配器全离线时回退活动适配器（`:1813`，桌面，新增 `select_adapter_for_quality`；SSID 读取失败告警）
  9. 安装包残留清理（`:1765`+`:1774`，安卓 `cleanup_stale_apks` 24h / 桌面 `cleanup_stale_update_dir()`）
  另 10 项为整块或细节遗漏：**安全与健壮性修复（全栈审计轮，61 文件，49+10 条发现）整章新增**（`:1988`）、轻量化/关闭按钮三条修复（`:1935`）、新手引导五个后续条目含「卡内去描边线三端 12 处」（`:1410/1438/1454/1505/1515`）、USB 幽灵态恢复加固八项（`:392`）、极致档四角阴影 shade 0.45→0.2 及 8x 灰度实测表（`:610/657/686/713`）、启动期窗口与夜间出站三项（`:1619`）、系统级自动登录的安全加固细节（icacls `/T` + 对 `core-config.json` 本身收紧、`externalBin`、凭据跟随、`CheckOutcome::Skipped`、补试窗口 3→20 次，`:2516-2570`）、`clean-build-cache.ps1` 纳入项目外 cargo-target（`:2655`）、构建/发布链九条（`:1613/2128/2136/2171/2191/2202/2225/2246/2273`）、双二进制构建说明。

**验证**（当轮实际输出）

- `wc -l release/RELEASE_NOTES_v2.4.3.md` → 144 行；`grep -n '^## '` → 11 个章节（v2.4.2 的 8 个全部保留 + 校园判定新增 1 + 安全审计新增 1 + 升级说明）；`grep -c '^- '` → 99 条 bullet（v2.4.2 原 8 章节 bullet 数全部覆盖，无删减）。
- 19 项遗漏逐条比对已落地：对每项在融合版中 `grep` 关键词命中（如「data-dock-strip」「problem 22」「data-highlighted」「静默降级」「w-24」「select_adapter_for_quality」「cleanup_stale_apks」等）。
- `release/` 与 `changelogs/` 均在 `.gitignore:7`、`:59` 中，两处改动不进 git。


### v2.4.2 发布准备：Release Notes 撰写 + 全量版本号升级（2026-10-26，分支 chore/release-v2.4.2，仓库维护）

**为什么**：v2.4.2 功能已累积到封版规模（changelog 覆盖 2026-10-13 至 10-26，含 WebVPN 校外访问、夜间出站整轮重做、悬浮栏玻璃定型、新手引导重做、安卓内存优化等），需要按历代 `release/RELEASE_NOTES_v*.md` 风格产出本版更新说明，并把所有版本号从 2.4.1 升到 2.4.2，供用户编译安装包后发布。

**改了什么**

- **新增 `release/RELEASE_NOTES_v2.4.2.md`**：严格沿用 v2.4.0 / v2.4.1 的结构——标题 → 固定「📥 下载提醒」引用块（GitHub Releases + 夸克网盘两条渠道，链接 `https://pan.quark.cn/s/c46f67bc2658`）→ 一段「本版本最重要的变化：**加粗主打句**」→ 分节 `## <emoji> <主题>`（每节 bullet 以 `**小标题**：` 开头）→ 末尾「升级说明」。内容全部取自 `changelogs/CHANGELOG_v2.4.2.md`，按主题归为 9 节：WebVPN 校外访问（全新功能）/ 夜间出站切换方向修正与整轮重做 / 悬浮栏玻璃质感 / 新手引导重做 / 安卓端内存·平板布局·账号页 / 桌面系统级自动登录与稳定性 / 其他修复 / 仓库维护 / 升级说明。主打句聚焦三件最重磅：WebVPN 校外访问、夜间出站方向修正、悬浮栏玻璃定型。
- **全量版本号升级 2.4.1 → 2.4.2**（16 个文件，与上次 `d570be6b` 的清单一致）：`tauri-app/src-tauri/tauri.conf.json`（权威源）、`tauri-app/src-tauri/Cargo.toml` + `Cargo.lock`、`tauri-app/package.json` + `package-lock.json`、`tauri-app/frontend/package.json` + `package-lock.json` + `src/shared/ui-constants.ts` + `about-preview.html`（2 处）、`android/src-tauri/tauri.conf.json` + `Cargo.toml` + `Cargo.lock`（campus-login 与 campus-login-android 两包）、`android/frontend/package.json` + `package-lock.json` + `src/shared/ui-constants.ts`、`README.md` 徽章。
- **`version.json`**：`v2.4.1` → `v2.4.2`，notes 正文按本版重写（保留固定下载提醒头 + GitHub/夸克两条渠道 + 「### 本版本最重要的变化」五条要点 + 末尾「完整变更见 Release Notes」）。
- **未改生成目录**：`android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json` 含旧版本号，但该文件被 `android/src-tauri/gen/android/app/.gitignore:3` 忽略、属构建产物，构建时由 tauri 重新生成，故不手改。
- **修正一次误替换**：初版脚本对 `tauri-app/package.json` 做了全文替换，误把第三方依赖 `@tauri-apps/plugin-global-shortcut` 的版本约束 `^2.4.1` 改成 `^2.4.2`，已还原为 `^2.4.1`（该插件实际发布版本就是 2.4.1，改掉会破坏 Tauri CLI 的 NPM/Rust 同 minor 校验）。其余 lock 文件中残留的 `2.4.1` 均为第三方包（`@tauri-apps/plugin-global-shortcut`、`@asamuzakjp/nwsapi`、`@tauri-apps/plugin-notification`、`fastrand`、`tauri-plugin-global-shortcut`）自身版本号，按设计保留。

**验证**（均为当轮实际输出）

- 桌面 `npx tsc --noEmit --incremental`（`tauri-app/frontend`）：exit 0。
- 安卓 `npx tsc --noEmit --incremental`（`android/frontend`）：exit 0。
- 桌面 `cargo check --lib`（`tauri-app/src-tauri`）：`Compiling campus-login v2.4.2 … Finished dev profile [unoptimized + debuginfo] target(s) in 11.56s`，exit 0，版本号已生效。
- `version.json` JSON 解析通过：`version= v2.4.2`，notes 长度 618 字符。
- `git diff --stat`：21 个文件（16 个版本号文件 + version.json + release notes 新增 + 本会话安卓排版修复 4 个文件 + `.codewiki/_meta.yaml`）。

**未做/交用户决定**：安装包尚未编译，`.sha256` 与 APK 由用户执行 `make-release.ps1` / `android/build-apk.ps1` 产出；**推送与 GitHub Release 发布须等用户编译完成后再执行**（用户明确要求）。

### 安卓端自助服务排版修复 + 平板布局对齐 Windows 端（2026-10-26，分支 chore/release-v2.4.2，仅安卓端）

**为什么**：用户真机反馈两点。①「安卓端自助服务在线信息卡片的排版修复一下，目前两个按钮导致文字挤压」——真机截图（1280×2772，约 394 CSS px 宽）显示「在线信息」卡头部一行塞下了图标 + 标题 + 描述 + 「刷新」+「VPN 连接」两个按钮，两个按钮约 228px 的固定宽度把描述文字压成三行；②「把安卓平板的布局对齐 windows 端」——平板外壳此前用 `zoom: 0.9` 整体缩小内容区，与桌面观感不一致（zoom 会连带缩小内边距与字体，平板用户反馈比桌面「憋屈」），且内容区宽度固定 720px 不随视口变化、面板懒加载时整块空白。

**改了什么**

- **在线信息卡头部窄屏纵向堆叠**（双端 `SelfServicePanel.tsx`，按 AGENTS.md 第 3 条双端同步）：外层 `flex items-center justify-between gap-3` → `flex flex-wrap items-center justify-between gap-3`，按钮组容器补 `max-sm:basis-full max-sm:justify-end`——窄屏（<640px）下图标+标题占一行、两个操作按钮另起一行右对齐；`sm` 及以上（桌面/平板）维持原横排。桌面端 `tauri.conf.json:18-19` 的 `minWidth: 960` 保证窗口不会窄到 640px 以下，故该改动对桌面仅为防御性同步。
- **在线信息卡堆叠舞台垂直居中**（`android/frontend/src/account/SelfOnlineStack.tsx`）：单卡容器由 `absolute top-0` 补为 `absolute inset-x-0 top-0 bottom-0 my-auto h-fit`，与桌面 `SelfOnlineCards.tsx` 同款——舞台量高取所有卡的最大自然高度，矮卡在舞台内垂直居中，不再被高卡顶到顶端。
- **上网记录卡改为无条件渲染**（`android/frontend/src/account/SelfServicePanel.tsx`）：原 `hasCred && <SelfOnlineRecords .../>` 的整卡隐藏改为无条件渲染，未配凭据时由卡内 `hasCred` 分支给出「去账号管理填登录信息」指引——与桌面同款，比整卡隐藏更能说明功能存在。
- **登录时间不再截取时间部分**（`android/frontend/src/account/SelfServicePanel.tsx` 两处）：`item.loginTime?.split(' ')[1] || item.loginTime || '-'` → `item.loginTime || '-'`，完整显示「日期 时间」，与桌面口径一致（原来只显示 `HH:mm:ss`，丢掉日期后跨天会话无法区分）。
- **未配凭据时的提示文案改用通用键**：`title={hasCred ? undefined : t('account.selfDashboardNeedCred')}` → `t('account.needLoginPassword')`，与其他卡片同一口径。
- **平板内容区去掉 zoom 缩小**（`android/frontend/src/components/tablet/TabletShell.tsx`）：删除 `CONTENT_ZOOM = 0.9` 常量与其 `style={{ zoom: CONTENT_ZOOM }}` 应用，内容区与桌面 1:1；顶部两栏 `TOPBAR_ZOOM = 1.3` 保留（触屏可点性）。
- **平板内容宽度随视口双档**：主区 `max-w-[720px]` → `max-w-[720px] min-[1024px]:max-w-[880px]`，与桌面 `isMaximized ? 880 : 720` 同口径（平板无最大化状态，用视口宽度代替：横屏 ≥1024px 给 880，竖屏维持 720）。
- **平板主区下内边距对齐桌面 dock 口径**：`pb-28` → `paddingBottom: calc(75px + env(safe-area-inset-bottom, 0px))`——75px 即桌面「dock 高 55 + bottom 20」，再叠加安卓导航栏 safe-area 让位；滚到底时最后一条内容刚好贴玻璃上缘。
- **平板面板懒加载补骨架**（`TabletShell.tsx` 新增 `PanelSkeleton`）：`<Suspense fallback={null}>` → `<Suspense fallback={<PanelSkeleton />}>`，与桌面 `App.tsx` 的 `PanelSkeleton` 同款，切换面板时不再整块空白。

**验证**（均为当轮实际输出）

- 桌面 `npx tsc --noEmit --incremental`（`tauri-app/frontend`）：exit 0。
- 安卓 `npx tsc --noEmit --incremental`（`android/frontend`）：exit 0。
- 未做真机截图复核：平板外壳改动（去 zoom、880 双档、骨架、下内边距）需真机/模拟器实测确认观感，本轮仅静态类型检查通过。

### 还原后静默期放行由「一次性」改为「持续到静默期结束」（2026-10-26，分支 fix/outbound-restore-silent-pass，双端后端）

**为什么**：上一版（见下方「夜间出站登录闸方向修正」）为「还原后登录判定必须生效」加了 `FORCE_CHECK_AFTER_RESTORE` 一次性放行标记，只放行紧接着的一轮巡检。用户实测后指出仍然不够——原话「将检测还原后的方案改为，还原后能直接检测，并在之后的检测时段都正常检测。避免现在还原后仍然因为未到检测时段而不检测」。核实成立：还原时刻（默认 07:30）本身就落在校园网检测静默期内（检测时段 07:40 起，`is_campus_check_silent(450, 460, 1380) == true`），一次性放行只覆盖那一轮；若那一轮因校园网尚未恢复而登录失败，后续轮次会被静默期重新挡回，用户看到的仍是「已还原但整段静默期都不再重试」，直到 07:40 才恢复。

**改了什么**

- **放行语义改为「本轮静默期剩余时段持续放行」**（桌面 `monitor/background_check.rs`、安卓 `monitor_loop.rs`）：静态标记 `FORCE_CHECK_AFTER_RESTORE`（`AtomicBool` + `swap(false)` 消费即清）改名为 `RESTORE_SILENT_PASS`，消费方式由「swap 即清」改为「load 判定 + 离开静默期才清除」——`if restore_pass && !campus_silent { store(false); restore_pass = false }`。还原后置位，静默期内每拍都照常执行 Portal 检测/自动登录/断连重连，一旦进入检测时段（静默期结束）标记自动失效，当日 23:00 起的下一个静默期不会再被误放行（不跨日残留）。
- **安卓端同构改造**：`monitor_loop.rs` 原先把时间计算包在 `if settings.campus_check_start_minutes > 0 && !forced_after_restore` 内，无法在清除标记前得知是否处于静默期。改为先算 `minutes_now` 与 `silent_now`（含 `start == 0` 禁用判定），再做标记清理与闸判定；`silent_now && restore_pass` 时额外记一条 `log_debug` 说明本轮为还原后放行。
- **日志降级**：放行期从「1 轮」变成「静默期剩余时段（默认 10 分钟约 20 拍）」，原 `log_info` 每拍一条会刷屏，降为 `log_debug`。

**验证**（均为当轮实际输出）

- 桌面 `cargo test`：`running 554 tests` → `test result: ok. 549 passed; 0 failed; 5 ignored`；集成三套 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿。
- 桌面 `cargo check --lib`：`Finished dev profile in 2.35s`，零 warning。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）：`Finished dev profile in 18.18s`，无 error/warning。
- 前端 `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0；安卓 `android/frontend` exit 0。

**边界说明**：放行期长度等于「还原时刻 → 静默期结束」的剩余时长。默认配置下（还原 07:30 / 静默期 07:40）是 10 分钟；用户若把还原时刻调得很早（如 06:00），放行期会相应变长，期间每拍都会向校园 Portal 发起判定与自动登录尝试——这是用户显式要求的行为（还原后要一直检测到正常检测时段），代价是若校园网在该时段确实未恢复，认证失败日志会持续到静默期结束。

### 出站修复的两轴代码审查整改：静默期判定下沉共享面 + 归一化内聚 + 转发层 test-only 化（2026-10-26，分支 fix/outbound-review-followups，双端后端）

**为什么**：对 `31bc85f0...HEAD`（含 `f9f82183` + `a468dd90`）跑 `/code-review` 双轴审查（Standards 子代理 `fb87bf64`、Spec 子代理 `acd52e9f`）。Standards 轴判出成文标准违背 2 条 + 判据外泄 1 处 + 空泛泛化 1 处，Spec 轴判出 scope creep 3 项。逐条核实后全部成立，本轮整改的是其中「属代码质量问题、不需要产品决策」的部分。

**改了什么**：

- **静默期边界判定下沉双端共享面**（`config/schedule.rs` 新增 `is_campus_check_silent(now, start, end)`）：此前同一段「start==0 禁用 / now<start 静默 / end>start 且 now>=end 静默」在四处各写一份——桌面 `monitor/campus_check.rs` 一份、安卓 `monitor_loop.rs` 两处手写展开 + 无头 `bin/campus-login-core.rs` 一份注释只能声称"同语义"的私有副本。任一处改退化规则就两端漂移。现在四个调用点全部改调共享函数；`monitor/campus_check.rs` 保留 `pub use` 转出以免打断既有 `monitor::campus_check::is_campus_check_silent` 路径引用，三条边界用例随之迁到 `config/schedule.rs` 测试模块（共用一份断言，不再两处维护）。
- **归一化内聚进判定函数**（`config/night_switch.rs::evaluate_night_switch_with_restore`、`config/outbound_switch.rs::evaluate_night_outbound_with_restore` / `is_night_outbound_guard_window_with_restore`）：`restore_start_minutes()` 原先要求 7 个调用点各自先归一化再传参（`adapter_watch.rs:189`、`outbound_night.rs:825`、`scheduled.rs:122`/`:190`、安卓 `monitor_loop.rs:889`/`:980` 等），漏一处就把恢复窗变成 `[0,1380)` 全天还原。改为函数内部归一化（幂等，重复归一化无副作用），七个调用点改传原始配置值。
- **旧签名包装函数 `#[cfg(test)]` 化**（5 个：`evaluate_night_switch`、`evaluate_night_outbound`、`is_night_outbound_guard_window`、`evaluate_outbound_action`、`evaluate_night_switch_gated`）：这些「转发默认 07:30」的包装在恢复时刻可配置后已无生产调用点，留着是"把默认常量写死"的陷阱，同时 `cargo check --all-targets` 报 4 条 dead_code 警告。加 `#[cfg(test)]` 后警告清零，且用例不必逐个改写签名；注释同步改为「生产一律调 `_with_restore`」。
- **删除 `PendingSwitchRow` / `parse_pending_switch`**（`monitor/outbound_switch.rs`）：上一版把登录闸下线后，挂起机制整体作废，`drive_pending_switch` 只剩「清残留」职责，这两个符号已无任何生产使用点（`Serialize` derive 也已无用），保留属空泛泛化。一并删除其单测。
- **双端前端恢复时刻魔法数具名化**（两端 `NetworkPanel.tsx`）：`300` / `1379` / `450` 三个裸数字提为 `RESTORE_TIME_MIN_MINUTES` / `RESTORE_TIME_MAX_MINUTES` / `RESTORE_TIME_DEFAULT_MINUTES`，各自注释说明取值来源（05:00 是凌晨补切窗口的产品口径、22:59 必须早于最早切换时刻 23:00、450 与后端默认同值），并把 `formatRestoreTime` 里的 `minutes < 1380` 与钳制上限对齐到同一个常量，消除「显示口径接受 1380、写入口径钳到 1379」的隐性不一致。

**验证**（均为当轮实际输出）：

- 桌面 `cargo test`：`running 554 tests` → `test result: ok. 549 passed; 0 failed; 5 ignored`（lib 双目标各 549）；集成三套 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿。
- 桌面 `cargo check --lib --all-targets`：`Finished dev profile`，**零 warning**（整改前 7 条，逐轮降到 0）。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973）：`Finished dev profile in 27.32s`，无 error/warning。
- 前端 `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0；安卓 `android/frontend` exit 0。

**未做/交用户决定**（Spec 轴标为「未被要求的行为」，需产品判断，本轮不动）：① 静默期由「仅跳校园网环境验证」改为整体跳过巡检；② 开机自启在静默期一并跳过自动登录；③ 外网探测无响应时的每日一次系统提醒。三者都是上一条修复的附带效果，若用户认为越界可单独收敛。

### 夜间出站登录闸方向修正：夜间不再等登录 + 恢复时刻可配置 + 还原后判定生效（2026-10-26，分支 fix/outbound-login-gate-deadlock，双端前后端）

**为什么**：用户对上一版「登录闸三态化」（见下一条）提出方向性质疑——原话「晚上切换为什么需要登录？晚上校园网关闭需要切到非校园网，早上才需要判定是否登录」。核实后确认用户对：登录闸把「目标网卡必须已在校园 Portal 在线」当作**夜间切换**的前提，而夜间切换的目的恰恰是**切离已经断网的校园网**，两件事方向相反。上一版只解掉了「热点因 Portal 不可达被拦」的死锁，闸本身仍然立在错误的位置——用户环境里只要目标卡 Portal 可达但未登录（例如热点与校园网关同段、或校园侧仍有响应），切换照样被拦。

**改了什么**

- **夜间切换彻底移除登录闸**（`monitor/outbound_night.rs`）：`target_adapter_logged_in` 与 `apply_outbound_switch` 内的闸调用全数删除；改为一次**外网可达探测**（`probe_target_internet`，绑源 ICMP 打 `OUTBOUND_PROBE_HOST = 223.5.5.5`，经 `network::subnet::check_gateway_reachable_from`），探测结果**只用于日志与每日一次提醒（`OUTBOUND_PROBE_WARN_DAY`）、绝不阻断切换**——「探测失败」可能只是目标被拦 ICMP，把推测当成否决条件正是上一版的病根。切换成功与否只取决于候选筛选（`select_outbound_candidate` 的 IP/网关/非校园判定）与动作执行结果。
- **挂起机制随闸一起下线**：不再产生 `outboundPendingSwitch` 新值；`drive_pending_switch` 降级为纯清理入口（历史 `waiting-login` 残留每拍清空，前端徽标随之消失）。前端挂起徽标与三条 i18n 文案（`outboundPendingSwitch` / `outboundPendingWaitLogin` / `outboundPendingSwitchTip`，双端 zh/en）删除。
- **恢复时刻可配置**（新增 `nightRestoreMinutes`，双端）：判定层 `config/night_switch.rs` 新增 `DEFAULT_RESTORE_START_MINUTES = 450`（07:30）与 `restore_start_minutes(configured)` 归一化（0/越界回落默认）；`evaluate_night_outbound` / `evaluate_night_outbound_with_restore` / `is_night_outbound_guard_window` / `is_night_outbound_guard_window_with_restore` / `evaluate_outbound_action` / `evaluate_outbound_action_with_restore` / `evaluate_night_switch_gated` / `evaluate_night_switch_gated_with_restore` 双签名并存（旧签名转发默认 07:30），不再读写常量。桌面 `monitor/scheduled.rs`、`monitor/adapter_watch.rs`、`monitor/outbound_night.rs`（`in_outbound_restore_window` 改收 `&Config`）与安卓 `monitor_loop.rs` 消费该字段。前端双端 NetworkPanel 新增「还原时刻」`<input type="time">`（05:00–22:59 钳制，后端另有归一化兜底），功能描述文案由硬编码「次日 7:30」改为带 `{{time}}` 插值。
- **还原后判定必须生效**（`monitor/background_check.rs` + `monitor/outbound_night.rs`）：还原时刻可配置后可能早于校园网检测静默期起点（默认还原 07:30 / 静默期 07:40），还原后补检会被静默期整体跳过，登录判定要等到静默期结束才发生。新增一次性放行标记 `FORCE_CHECK_AFTER_RESTORE`（`request_check_after_outbound_restore()` 置位、`run_background_check_blocking` 内 `swap(false)` 消费即清），`finish_outbound_restore` 在发起补检前置位。安卓同构（`monitor_loop.rs` 静态标记 + `run_check_once` 闸条件 `&& !forced_after_restore`，还原成功分支置位）。
- 清理：`monitor/outbound_switch.rs` 删除 `monitored_adapter_online`（上一版 `login_gate_allow` 的三态判定随之作废）与对应单测 `monitored_adapter_online_matches_by_name`；`pending_switch_json` 删除、`pending_switch_json_round_trip` 改写为只保留解析侧的用例。`PendingSwitchRow` 注明仅作历史残留解析；`config/model.rs` 的 `outbound_pending_switch` 文档改为「历史残留，本版本不再产生」。（后续账号：这两个解析符号在审查整改中一并删除，见本条上方「出站修复的两轴代码审查整改」。）

**验证**（均为当轮实际输出）

- 桌面 `cargo test`：lib `test result: ok. 550 passed; 0 failed; 5 ignored`；集成三套 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿；`cargo check --lib` Finished 无 error/warning。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973，`CC_aarch64_linux_android=aarch64-linux-android24-clang.cmd`）：`Finished dev profile in 30.05s`，无 error/warning。
- 前端 `npx tsc --noEmit --incremental`：桌面 `tauri-app/frontend` exit 0；安卓 `android/frontend` exit 0。

**遗留说明**：外网可达探测按设计不阻断切换，故「目标出口其实上不了网」时仍会切过去——这是有意的取舍（探测目标被墙/被拦比真断网常见得多），由每日一次的提醒通知告知用户排查。

### 夜间出站切换整夜不执行 + 静默期整夜空转登录（2026-10-25，分支 fix/outbound-login-gate-deadlock，双端后端）

**为什么**：用户实测两处（原话「时间到了还在检测，同时已经切换了网卡却没有更新」→ 追加「不对是根本没有执行网卡切换」）。

① **登录闸死锁（主因）**：`5b9181ed` 引入的登录闸要求「目标网卡必须在校园 Portal 在线」才放行切换，但候选筛选（`is_campus_adapter`）判定「非校园网」的依据恰恰是**校园网关与 portal 主机都不可达**——两个判据互斥，对手机热点这类不经营校园认证的出口条件恒假。用户当晚 WLAN 是手机热点（10.146.34.246），`check_portal_full` 返 `reachable=false`（`build_request_failed_status`），旧代码 `map(|s| s.online).unwrap_or(false)` 一律判未登录 → 卡在「待切换: WLAN（等待该网卡登录校园网）」整夜，落盘配置 `outboundMetricRestore`/`outboundDisabledAdapters`/`outboundStandbyRoute` 三快照全空即为「切换从未执行」的铁证。

② **静默期只管住一半**：`campus_check_start/end` 设置项文案是「仅此时段内执行校园网检测」，但旧实现只跳过「校园网环境验证」这一小段，Portal 登录检测、自动登录、断连重连照跑。校园网 23:00 断网后，用户日志里 23:04 起每分钟一轮「以太网 正在登录… → 登录失败: Authentication fail」整夜空转，`auth/portal.rs:127` 注释已明示这有账号锁定风险。

③ **挂起无通知**：「待切换」徽标只在前端可见，用户不看面板就完全无感，整夜静默失效。

**改了什么**：

- `tauri-app/src-tauri/src/monitor/outbound_switch.rs`：新增纯函数 `login_gate_allow(monitored_online: Option<bool>, portal: Option<&PortalStatus>) -> bool` 承载三态语义——监测已在线放行；Portal 在线放行；**Portal 不可达放行**（与候选筛选同一信号，不经营校园认证）；Portal 可达但未登录**拦截**（切过去就断网，这才是本闸该拦的）；探测发不起来同不可达放行。附回归用例 `登录闸_不可达即放行且可达未登录才拦截`（8 条断言，含热点场景 `gate(Some(false), Some(&status(false, false))) == true`）。
- `monitor/outbound_night.rs::target_adapter_logged_in`：重构为「监测结论已在线直接放行；否则**无论监测结论如何都补一次绑源 Portal 探测**」（旧代码监测卡直接采信 `last_a1_online=false` 就不再探测，把「不可达」与「可达未登录」混为一谈），探测结果交 `login_gate_allow` 判定，四个分支各带可读日志（在线/不可达放行/可达未登录挂起/无法确认挂起）。该探测每拍至多一次且仅在未在线时才发。
- `monitor/outbound_night.rs`：新增 `OUTBOUND_PENDING_LOGIN_WARN_DAY` 去重标记，登录闸挂起时调 `notify_outbound_issue_once_per_day` 推「夜间出站切换未执行」通知（标题+「目标网卡 X 可访问校园 Portal 但尚未登录…登录确认后会自动继续」），每天至多一次不刷屏。
- `monitor/background_check.rs::run_background_check_blocking`：静默期由「只跳过环境验证」改为**整体跳过本轮巡检**（提前 `return CheckOutcome::Skipped`，保留 `cancel_campus_exit` 与 `on_campus_network=true` 的既有语义），Portal 检测/自动登录/断连重连全部不执行。走 `Skipped` 而非 `Online`/`NotOnline`，是为了不累计退避、不把「已登录」刷成离线触发状态变更通知（`dynamic_interval.rs:67` Skipped 沿用上拍间隔）。顺带删掉原静默期分支构造的 `CampusCheckResult` 假结果与相关 import。
- `monitor/auto_auth.rs::run_auto_login_on_start`：静默期分支补 `return`（原来只是跳过环境验证后继续往下走登录流程），日志文案改为「跳过校园网环境验证与自动登录」。
- `android/src-tauri/src/monitor_loop.rs::auto_login_on_start`：双端同步——安卓启动自动登录同样加静默期闸（与桌面同退化规则 `end > start && now >= end`），打 `启动检测:校园网检测静默期(非检测时段),跳过校园网判定与自动登录`。安卓 `run_check_once` 早已有此闸（:1363-1383），本次只补齐启动登录路径。

**验证**（均为当轮实际输出）：

- 桌面 `cargo test`：`test result: ok. 548 passed; 0 failed; 5 ignored`（lib 双目标各 548）+ 集成测试 `2 passed / 1 ignored`、`3 passed`、`1 passed` 全绿；基线原为 547，新增 1 条登录闸回归用例。
- 桌面 `cargo check --lib`：Finished，无 error/warning（首轮 `unused import: CampusCheckResult` 已清理）。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973，`CC_aarch64_linux_android=aarch64-linux-android24-clang.cmd`）：`Finished dev profile` 无 error/warning。
- 前端 `npx tsc --noEmit --incremental`（tauri-app/frontend）exit 0（本次未改前端，作为回归确认）。

**遗留说明**：用户当晚配置 `enableNightOutboundSwitch=false`（总开关关闭）而 UI 显示开启——属另一独立现象，本次未复现确认，未改动；本修复针对「开关开启时到点不切换」的判定链。

### 审查跟进：captcha 测试移植 + CODEFALSE 换图重试 + 两处文档修正（2026-10-25，分支 fix/review-followups，桌面后端）

**为什么**：code-review（origin/main 全部 3 提交）双轴审查发现的最大缺口：kaptcha 修复（62c0eccc）移植了识别器却没移植 Hub 的配套测试；且 CODEFALSE 后不重试——Hub 原版 auth.rs should_retry 语义是「答案被拒→换一张再提」，移植版只在识别不出时换图，答案被服务端拒绝就直接失败。另有两处文档瑕疵：cas.rs 模板单例段插在 parse_login_response 的 doc 注释与函数体之间（rustdoc 挂错对象）；captcha.rs 注释引用本仓库不存在的 tests/captcha_solve.rs（生成器实在 Wxxy-CampusHub 仓库）。

**改了什么**：
- 移植 Hub `crates/campus-auth/tests/captcha_solve.rs` → `tauri-app/src-tauri/tests/captcha_solve.rs`（use 路径改 `campus_login_lib::vpn::captcha`，样本路径改随测试入库的 `tests/captcha-samples`）：`solve_synthetic_roundtrip`（离线合成图 roundtrip，覆盖颜色归一化/NCC/切分/不硬猜）常态运行；`captcha_solve_eval`（真实样本 70/30 划分评测，断言正确率 ≥98% 且自信错误=0——自信错误会消耗 CAS 连续错误计数是红线）由 Hub 的 #[ignore] 改为常态运行；`build_templates`（重写模板 json 的维护动作）保持 #[ignore]。样本 101 张 png + labels.json（201KB）从 Hub `scripts/captcha-samples/` 复制到 `tauri-app/src-tauri/tests/captcha-samples/`。
- `vpn/captcha.rs` 恢复 `KaptchaTemplates::from_json_str`（Hub 原版 3 行，错误类型适配 `CasAuthError::Parse`；仅集成测试使用，#[allow(dead_code)] 标注测试锚定用途）。
- CODEFALSE 换图重试：`vpn/cas.rs` 新增 `CasClient::login_with_captcha(username, password_rsa_hex, templates, max_attempts)` 组合方法——每轮先 `solved_captcha(3)`，登录遇 `WrongCaptcha`（CODEFALSE）换一轮重提，总轮数上限 3；识别穷尽返回新增变体 `CasLoginError::CaptchaUnsolvable(usize)`（从未发起登录，纯识别层失败）。`CasLoginError` 加 `CaptchaUnsolvable(usize)` 变体，`error.rs` 映射文案「CAS 验证码自动识别失败（连续 {n} 张未识别，请稍后重试）」。三个调用点（桌面 commands/vpn.rs run_full_chain、vpn/cache.rs login_new_session、安卓 vpn_cmds.rs run_full_chain）由「solved_captcha + login 两段」改为单调 `login_with_captcha`，识别失败文案统一走 `map_cas_login_error`。
- 文档修正：cas.rs 中 `parse_login_response` 的 3 行 doc 注释移回函数体前（原被模板单例段隔开，rustdoc 挂到单例段上）；captcha.rs:47 注释改为指明模板生成器在 Wxxy-CampusHub 的 `crates/campus-auth/tests/captcha_solve.rs::build_templates`、模板文件逐字节一致。

**验证**（当轮实际输出）：
- `cargo test --test captcha_solve`：3 测试 2 passed 1 ignored；评测实测「模板集: 正确 70/70 (100.0%) 自信错误 0；holdout: 正确 30/30 (100.0%) 自信错误 0」。
- 桌面 `cargo check --all-targets` Finished 无 error/warning；`cargo test` 全量 547 passed / 0 failed / 5 ignored（lib 双目标）+ 集成测试全过。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27，CC 用 `aarch64-linux-android24-clang.cmd`）Finished 无 error/warning。

不修项（审查发现但判断不落本分支）：run_full_chain 双端下沉单点重构（结构性，收益一般）；减号模板缺失（模板集 116 条仅 `* + 0-9`，减法题必拒→换一张兜底，Hub 原版同款，补齐需服务端真实减法样本不可造）；verify 取消后已验证会话留缓存被复用的低危副作用。


### VPN 凭据弹窗：保存前先校验凭据 + 输入框浅色可见性（2026-10-25，双端前端+双端后端）

**为什么**：用户实测反馈两点。①「浅色模式下输入框不明显，容易导致误触」——`Input` 基础类只有 `bg-muted/40` 填充、无边框，而浅色下 `--muted`（220 14% 94%）与弹窗底 `--background`（210 40% 96%）明度几乎相同，输入区与卡面糊成一片，只靠一点填充差看不出边界。②「先不要保存凭据，先判定是否连接上了再保存，避免保存错误密码」——原流程是「保存 → 自动测试连通性（toast 报结果）」，密码错了也照样 DPAPI/Keystore 加密落盘，用户得先存错再改。

**改了什么**：
- 后端新增「用未保存凭据校验」命令（双端同构）：`vpn_verify_credential(account, password)`——复用各端 `run_full_chain`（① CAS 统一身份认证 → ② WebVPN 网关登录 → ③ 网关会话探活），**全程不落盘**；成功时会话入缓存（与 `vpn_test_connection` 一致，后续业务可直接复用）。桌面注册在 `tauri-app/src-tauri/src/app/startup.rs`，安卓注册在 `android/src-tauri/src/lib.rs`；实现分别在 `tauri-app/src-tauri/src/commands/vpn.rs` 与 `android/src-tauri/src/vpn_cmds.rs`（命令面双端对齐，见 CodeWiki 的 `decisions/ipc-command-name-alignment`）。
- 前端弹窗改两段式（`VpnCredentialDialog.tsx`，双端）：点保存 → 先 `vpn_verify_credential` → 通过才 `vpn_save_credential` 落盘。校验期间按钮变「验证中…」+ Loader2 转圈，保存/取消/账号激活按钮与两个输入框全部禁用，弹窗不可点外关闭；校验失败错误就地展示（后端分步 message 原样透出，如「① CAS 统一身份认证失败: 账号或密码错误」），**弹窗保持打开、已输入内容不清空**，用户改完可直接重试——避免把打错的密码存进配置。`SelfServicePanel.tsx` 的 `handleVpnSave` 去掉保存后重复调 `vpn_test_connection`（校验阶段已确认链路可用）。
- 输入框可见性（`components/ui/input.tsx`，双端）：基础类补 `border border-input` + 填充由 `bg-muted/40` 改为 `bg-card`（浅色纯白、深色比底亮一档），hover `border-input/70`、focus `border-ring`；`index.css` 浅色 `--input` 由 `220 14% 90%` 加深到 `220 13% 82%`（深色 `220 15% 18%` 不动）。error 态改为覆盖式 `border-destructive`（基础类已有边框宽度，不再重复给 `border`）。
- i18n 四处（双端 zh/en）新增 `account.vpnVerifying`（验证中… / Verifying…）与 `account.vpnVerifyFailed`（凭据校验未通过，请检查账号密码 / Credential verification failed...）。
- 浏览器实测用 mock 补上 6 条 WebVPN 命令（`tauri-app/frontend/tauri-mock.js`，仅 `vite.mock.config.ts` 注入、不进构建产物）：`vpn_credential_status`（`?vpnsaved=1` 覆盖已保存态）、`vpn_verify_credential`（`?vpnfail=1` 覆盖校验失败）、`vpn_save_credential`、`vpn_clear_credential`、`set_self_service_via_vpn`、`vpn_test_connection`。
- 测试 `tauri-app/frontend/src/account/SelfServicePanel.test.tsx`：原「保存后自动测连通性」用例改写为「先校验后保存」；新增「校验通过才写凭据且不再重复测连通性」（断言 `vpnTestConnection` 零调用）与「校验失败不保存、错误就地展示、密码保留」（断言 `vpnSaveCredential` 零调用、弹窗仍在、错误文案与输入值）。

**验证**（均为当轮实际输出）：
- 桌面 `npx tsc --noEmit --incremental`（tauri-app/frontend）exit 0；安卓同命令（android/frontend）exit 0。
- 桌面 `cargo check --all-targets` 无 error/warning（exit 0）；`cargo test` 547 passed / 0 failed / 5 ignored。
- 安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27 注入 `ANDROID_NDK_HOME` + `CC_aarch64_linux_android` 用 `aarch64-linux-android24-clang.cmd`）Finished，无 error/warning。
- `npx vitest run src/account/SelfServicePanel.test.tsx` 11 passed（含新增 2 例）。
- 浏览器实测（Chrome + vite.mock，浅/深双主题）：浅色下输入框计算样式由「无边框 + bg rgba(238,239,242,.6)」变为「边框 rgb(203,207,215) + 填充 rgb(255,255,255)」，弹窗底 rgb(241,245,249)，边界一眼可辨（截图确认）；点击保存后实测按钮文本变「验证中…」、全部按钮/输入框 `disabled=true`；`?vpnfail=1` 下弹窗保持打开、错误红字「① CAS 统一身份认证失败: 账号或密码错误（mock）」、密码框值仍为 `wrong-password`；通过场景下验证结束弹窗数 0（已关闭）。深色主题下输入框（border rgb(39,44,53) / bg rgb(29,32,38) / 弹窗底 rgb(17,19,23)）层级正常，未被本次改动破坏。

### CAS 登录补回 kaptcha 自动解题（2026-10-25，分支 fix/cas-kaptcha，双端后端）

**为什么**：用户实测报错「① CAS 统一身份认证失败: 服务端要求验证码」（CODEFALSE）。移植 WebVPN 时按「本校实测无验证码」裁掉了 kaptcha 流程、登录固定传空 id/code——2026-10-25 实测服务端已强制校验验证码，空值登录恒 CODEFALSE（真实时间戳探针复现，排除时钟偏差；GET /lyuapServer/kaptcha 返回算术验证码图）。根因不是风控，是协议实现缺验证码环节。

**改了什么**：从 Wxxy-CampusHub 原样移植其验证码自动识别器——`tauri-app/src-tauri/src/vpn/captcha.rs`（算术题「d1 op d2 =」自动解题：颜色线性归一化强度图 → 垂直投影切分 → bbox 锚定 16×14 画布 → NCC 模板匹配，阈值 0.88/间隙 0.03 双判，不达阈值绝不硬猜）+ 模板 `tauri-app/src-tauri/templates/kaptcha-templates.json`（编译期内嵌，约 330KB）。`vpn/cas.rs` 补回 `kaptcha()`（GET /kaptcha）、`parse_kaptcha`、`login` 增加 captcha_uid/captcha_code 两参、`solved_captcha(templates, max)`（最多 3 张，识别失败换一张）与模板单例 `kaptcha_templates()`（OnceLock）。三个调用点（桌面 commands/vpn.rs run_full_chain、vpn/cache.rs login_new_session、安卓 vpn_cmds.rs run_full_chain）登录前先自动解题；识别穷尽报「验证码自动识别失败（连续 3 张未识别）」；CODEFALSE 文案改为「验证码校验未通过（识别答案被服务端拒绝，请重试）」。Cargo.toml 新增 `image = "0.25"`（仅 png feature）。协议代码单点在桌面 crate，安卓经 path 依赖自动继承（例外 1，不复制）。

**验证**：真实凭据探针实测完整链路：CAPTCHA_SOLVED code=8 → CAS 登录成功拿到 TGT-2058***（修复前同凭据恒 CODEFALSE）；桌面 `cargo check --all-targets` Finished 无警告，`cargo test` 547 通过 0 失败（5 ignored 既有基线）；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27 环境，CC 用 `aarch64-linux-android24-clang.cmd`——带 `-cl` 的旧名会被 cc crate 误判 MSVC）Finished 无错误。

### 桌面 crate dead_code 警告清零 + 安卓错误映射导入路径修复（2026-10-25，分支 fix/rust-dead-code-warnings）


**为什么**：桌面 crate `cargo check --all-targets` 存在 dead_code/unused_imports 警告（合并后累积：vpn/mod.rs 的 pub use 转发、self_service 7 个"默认直连"薄包装、cas/session 的零使用字段与方法等）。另在验证安卓端时发现 main 上安卓 aarch64 check 本身就是坏的——bc995910 重构把三个错误文案映射函数迁到 `vpn/error.rs`，安卓 `vpn_cmds.rs` 仍从旧位置 `vpn::cache` 导入（E0432/E0603×2），属迁移遗留破损（已 stash 实测确认与本次改动无关），不修则安卓端无法作为验证手段。

**改了什么**：删除零使用代码——`self_service/mod.rs` 7 个直连薄包装（bind_operator/query_bind_status/reveal_credential/change_password/query_dashboard/query_online_log/offline_session，两端命令层只用 `_via` 变体）；`vpn/mod.rs` 两行 unused `pub use` 转发；`vpn/cas.rs` CasClient 的 jar 字段与 jar()/http_client() 方法；`vpn/session.rs` WebVpnSession 的 jar/http 字段、wrapped_client()（业务全用 wrapped_client_manual）、jar() 方法；`monitor/dynamic_interval.rs` offline_streak() 访问器（#[cfg(test)] 但测试也没用）。保留+`#[allow(dead_code)]`：crypto.rs 的 decrypt_host/Aes128CfbDec（golden 向量测试锚定）、cas.rs CasLoginOk.ticket（协议响应完整形态）、session.rs COOKIE_TICKET（仅测试断言+文档引用）、jar.rs RESTORE_URL/snapshot/restore/has（M1 会话恢复冻结接口，上层批 3 取舍不扩展）、route.rs Unreachable（commands/vpn.rs 已就绪匹配）。修复 `android/src-tauri/src/vpn_cmds.rs:15` 导入：三个 map_* 错误映射改从 `campus_login_lib::vpn::error` 导入（bc995910 迁移遗留）。_via 函数 doc 改为「通道化变体：执行通道由调用方决定」。

**验证**：桌面 `cargo check --all-targets` 警告 0（Finished 0.58s）；桌面 `cargo test` 551 通过 0 失败（547 lib + 4 集成，5 ignored 为既有基线；基线 397→551 因 VPN 接线合并新增测试）；安卓 `cargo check --target aarch64-linux-android --all-targets`（注入 NDK 27 环境）Finished 5.40s 无错误无警告。

### 玻璃 Dock 渲染降负载：hyalite 新增 lit 选项裁剪边缘光层（2026-10-25，worktree 收编进 main，双端）

**为什么**：2026-10-24 真机实测发现玻璃 Dock 常驻渲染 filter 链过长（21 图元）；中间档/紧凑档在收起态并无边缘光必要，裁掉可降 GPU 合成负担。原改动留存于 worktree（perf-dock-glass-primitives-20261024）未提交，2026-10-25 分支/worktree 清理时收编进 main（提交 c2befbc6）。

**改了什么**：双端（tauri-app/frontend + android/frontend）vendor/hyalite.js 新增 `lit` 选项（默认 true 保持原效果）：`lit:false` 时跳过 4 个边缘光图元（feColorMatrix/feComponentTransfer/feComposite 一族），缓存 key 增加 lit/nolit 维度防串缓存；双端 glass-dock.tsx 中间档与紧凑档两处 Dock 改为 `sat:1, lit:false`，filter 链 21→8 图元。

**验证**：`npx vitest run src/lib/dock-lens.actionButton.test.tsx` 3/3 通过（main 实现，含独立控件豁免；测试文件 main 已有、d0797d68 引入，与 worktree 版内容一致）；双端 `npx tsc --noEmit --incremental` exit 0。

### WebVPN 校外访问：安卓端 Rust 后端接线（2026-10-25，分支 feat/android-vpn-wiring，仅安卓端）

**为什么**：WebVPN 校外自动访问此前只接了双端前端与桌面后端；安卓端命令面零接线（前端 5 个 invoke 封装存在但后端无对应命令），审查确认这是该功能最重缺陷。

**改了什么**

- **新模块 `android/src-tauri/src/vpn_cmds.rs`**（桌面 commands/vpn.rs 同构）：5 命令 `vpn_save_credential` / `vpn_credential_status` / `vpn_clear_credential` / `set_self_service_via_vpn` / `vpn_test_connection`，命令名与返回形状（CommandResult {success,message,data}、status 的 {saved,accountMasked,viaVpn}）与桌面及安卓前端 invoke 契约对齐。凭据加密用 AndroidKeyStore（tauri-plugin-campus-keystore 的 CryptoBridge），不用 DPAPI——`vpn_password_enc` 字段语义为「平台加密器密文」（桌面 DPAPI / 安卓 Keystore，两侧密文互不可解、字段语义一致），内存态与落盘态均为密文。改状态命令（save/clear/set）设 `ensure_identity_gate` 生物识别门，查询类（status/test）不设——对齐 self_service_cmds 纪律（桌面 clear/set 无门是桌面差异，安卓侧按自身纪律设门）。save/clear/set 全程持 config_io_lock 覆盖 load→merge→save；先落盘再改内存缓存；日志只出 mask_account 打码账号。
- **路由决策与链路**：`resolve_self_service_channel`（OffCampus+开关开+有凭据→WebVPN 包装，否则直连+校外配置指引提示）与 `run_full_chain`（①CAS 登录→②WebVPN 网关登录→③探活→会话入缓存）逻辑复用桌面 crate 的 `campus_login_lib::vpn`（cache/cas/rsa/session）；网络 zone 判定复用安卓 `campus_detect::probe_campus_with_ssid`（SSID 名单/网关/Portal TCP 探测，失败→Unknown→直连）。
- **config_state.rs**：Settings 增 3 字段 `vpn_account` / `vpn_password_enc` / `self_service_via_vpn`（camelCase 序列化自动得 vpnAccount/vpnPasswordEnc/selfServiceViaVpn；默认空/空/true 对齐桌面；容器级 serde(default) 兜底旧配置文件，无需 schema 迁移）；`masked_for_display` 出站打码 vpn_password_enc（密文本机可解，出站即泄露攻击面）；`save_config` 对 vpn_password_enc 空/MASK 回退保留已存密文（防前端回传掩码覆盖真实密文，桌面 config_cmd 同构）。
- **self_service_cmds.rs**：7 个网络命令（bind/query_bind_status/query_dashboard/query_online_log/offline/change_password/reveal）接自动路由——执行前经 resolve_self_service_channel 决策通道，直连函数换 `_via` 变体，校外直连失败文案自动附加配置指引；验证门语义与参数校验顺序不变。`ensure_identity_gate` / `load_settings` 提为 pub(crate) 供 vpn_cmds 复用。
- **lib.rs**：mod vpn_cmds + 5 命令注册进 invoke_handler。

**验证**（实测输出）：`android/src-tauri` 下注入 NDK 环境后 `cargo check --target aarch64-linux-android --all-targets` → **Finished dev profile in 4.66s，零 error 零 warning**（lib + lib test 全 targets 编译通过；aarch64 目标单测运行不可行，测试编译随 --all-targets 验证）。新增单测：vpn_cmds 3 用例（append_hint 去重/offcampus_hint 语义）、config_state 4 用例（VPN 默认值、camelCase 键名契约、掩码打码 vpn 密文、vpn 密文落盘回环原样保留）+ vpn 密码字段回退语义，随树内既有测试一并编译验证。桌面端（tauri-app/）与安卓前端（android/frontend/）未动。密码明文不出现在任何日志。

**未做的实测**：真机端到端（保存凭据→校外连接测试→自助服务查询）需真实凭据与校外网络环境，不具备；上线前建议真机走一遍。

### WebVPN 校外访问：双端前端 UI（VpnCredentialDialog/VpnManageDialog/SelfServicePanel 入口）（2026-10-25，随 13d1fb9a 同提交，补记）

**为什么**：补记 13d1fb9a 中双端前端改动——此前条目自述「前端由另一任务完成」未覆盖，审查指出 changelog 覆盖缺口。

**改了什么**：双端（tauri-app/frontend + android/frontend）新增 VpnCredentialDialog（学号+统一身份认证密码录入，保存调 vpn_save_credential）与 VpnManageDialog（脱敏账号展示/清除/「校外自动经 WebVPN」总开关 Switch/连接测试 vpn_test_connection）；SelfServicePanel 头部加 ShieldCheck VPN 按钮（挂载即查 vpn_credential_status，已存则进管理弹窗，未存则进凭据弹窗）；tauriApi 5 方法封装；i18n zh/en 各 22 键；桌面新增 SelfServicePanel.test.tsx（vitest 10 用例）。

**验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 vitest 10/10 passed；安卓树无单测为记录在案约定（learnings/android-frontend-no-unit-tests）。

### VPN 弹窗增加统一身份认证「账号激活」入口（2026-10-25，分支 feat/webvpn-self-service，双端同提交）

**为什么**：用户要求在 VPN 弹窗提供 https://wxcas.cwxu.edu.cn/sf/#/account/accountActivation 账号激活入口，方便统一身份认证未激活/凭据无法登录时自助激活。

**改了什么**：双端 VpnCredentialDialog（按钮行左侧 link 按钮）与 VpnManageDialog（vpnAutoDesc 说明行下）各加「账号激活」链接按钮（ExternalLink 图标），点击经 `tauriApiWithRetry.openExternal` 打开（桌面 invoke `open_external`、安卓 plugin-opener，复用两端既有实现）；URL 以 `ACCOUNT_ACTIVATION_URL` 常量单点定义于各组件文件。i18n 双端新增 `vpnActivateAccount`（中「账号激活」/ 英 "Activate account"）。

**验证**：双端 `npx tsc --noEmit --incremental` EXIT=0；桌面 `npx vitest run src/account/SelfServicePanel.test.tsx` 10/10 passed。

### 自助服务经 WebVPN 校外访问：协议移植 + 自动共存路由（2026-10-25，分支 feat/webvpn-self-service，桌面 crate 单点实现、安卓自动继承）

**为什么**：自助服务系统（http://10.1.80.200:8080/Self）只在校内可达；校外需要经深澜 WebVPN 网关（https://webvpn.cwxu.edu.cn）包装访问。用户确认采用**自动共存路由**：校内直连（现状行为），校外且已保存 VPN 凭据时自动经 WebVPN——不做手动模式开关。协议代码移植自 Wxxy-CampusHub（campus-webvpn / campus-auth 两 crate），按 AGENTS.md 第 3 条「协议实现单点存在于桌面 crate、安卓经 Cargo path 依赖自动继承」，未复制到 android/。

**改了什么**

- **新模块 `tauri-app/src-tauri/src/vpn/`**（lib.rs + main.rs 双侧注册 `mod vpn`；注明移植来源与日期 2026-10-25，保留原中文注释与 golden 测试）：
  - `crypto.rs`：AES-128-CFB128 host 加密（KEY=`wrdvpnisthebest!`，hex(KEY)+hex(密文) 形态，坏 IV/hex 校验）。
  - `wrap.rs`：`wrap_url(raw, gateway)` URL 包装（非默认端口 `{scheme}-{port}` 段、path/query/fragment 保留、网关域幂等）。
  - `route.rs`：自动路由决策表 `route(raw_url, NetZone, has_session)`（Campus/Unknown→Direct；OffCampus 非内网→Direct；OffCampus 内网+无会话→NeedLogin；内网+有会话→包装，Err 保守 Direct）；`INTERNAL_HOSTS` 钉住 10.1.80.200；红线：Unknown 不做二次尝试（写路径安全）、公网域名永远直连。
  - `cas.rs`：CAS 统一认证最小集（账密登录拿 TGT、`sso_ticket` 换 ST；本校无验证码，captcha 相关全部裁掉）。
  - `rsa.rs`：textbook RSA 无填充密码加密（CAS 密钥 258 位 hex，126 字节分块）+ `cas_token_header`。
  - `jar.rs`：RecordingJar（reqwest CookieStore 实现，记录 cookie 名值对）。
  - `session.rs`：`WebVpnSession`（CAS TGT 静默登录 302 链、`is_alive` 探活、自动/手动跟随双 client）。
  - `error.rs` + `mod.rs`：`CasAuthError`/`WebVpnError`（thiserror）。
  - `cache.rs`：进程内会话缓存（`lazy_static + parking_lot::RwLock<Option<Arc<WebVpnSession>>>`），`ensure_alive` 缓存活→复用、失效→静默重登一次；错误映射为不含凭据的中文文案。
- **协议通道化 `self_service/mod.rs`**：新增 `SelfServiceChannel{base, client, local_addr}`（`direct` 直连 / `via_webvpn` 包装基址 + WebVPN 会话 client）；全部内部函数改收 channel，`format!("{SELF_BASE_URL}/...")` 统一走 `channel.url(path)`；7 个公开函数**签名不变**（安卓 path 依赖契约），内部转 `_via` 变体（`bind_operator_via` / `query_bind_status_via` / `query_dashboard_via` / `query_online_log_via` / `offline_session_via` / `change_password_via` / `reveal_credential_via`）。WebVPN 下 Location 判定不受包装影响（/Self/dashboard、/Self/login 字样判定两态通用）。
- **config 3 新字段**（`config/model.rs`，serde camelCase + default）：`vpnAccount`（明文学号）、`vpnPasswordEnc`（DPAPI 密文，内存态亦为密文、使用时解密）、`selfServiceViaVpn`（语义=「校外自动经 WebVPN 回退」总开关，默认 true）；`mask_in_place` 掩码 vpnPasswordEnc。`network/adapter.rs` make_test_config 同步补字段。
- **命令层 `commands/vpn.rs`**（5 命令注册进 startup.rs invoke_handler）：
  - `vpn_save_credential(account, password)`：identity gate → DPAPI 加密 → 先落盘再更新内存（save_config 同则）→ 总开关自动置 true → 重置缓存会话；日志只出打码账号。
  - `vpn_credential_status()`：`{saved, accountMasked, viaVpn}`（复用 self_service::mask_account）。
  - `vpn_clear_credential()`：清凭据 + 总开关置 false + 重置会话。
  - `set_self_service_via_vpn(enabled)`：总开关落盘（同值幂等短路）。
  - `vpn_test_connection()`：完整链路分步（①CAS 登录→②WebVPN 网关登录→③探活），message 分步中文结果，成功后会话入缓存复用。
  - `resolve_self_service_channel(state, local_addr)`：命令层共用路由决策——`detect_net_zone` 复用 `monitor::campus_check::check_campus_network`（spawn_blocking 包裹同步 ICMP/SSID 判定；探测失败→Unknown→直连）；OffCampus+开关开+有凭据→WebVPN 通道，否则直连并在失败文案附加「可配置 WebVPN」提示（不重复追加）。
- **7 个自助服务命令改走通道决策**（`commands/self_service.rs`，签名不变）：`ensure_identity_gate` 提为 pub(crate) 供 vpn_save_credential 复用。
- **配置保存防误抹**（`commands/config_cmd.rs`）：save_config 对 `vpnPasswordEnc` 空值/mask 占位回填保留当前值；import_config 整体丢弃导入文件里的 vpnAccount/vpnPasswordEnc（学号与加密密码是本机凭据对，防止错配）。
- **Cargo.toml 6 新依赖**（全部纯 Rust、双端可编译）：aes 0.8、cfb-mode 0.8、hex 0.4、thiserror 2、num-bigint-dig 0.8、num-traits 0.2。
- 前端改动（VpnCredentialDialog 等）由另一任务完成，本任务未动前端文件。

**验证**（实测输出）：`tauri-app/src-tauri` 下 `cargo check --lib` 无 error 无新 warning；`cargo test` → **lib 547 passed / 0 failed（基线 396 不回归，新增 vpn 模块 34 用例、route 决策 9 用例、通道化 self_service 与命令层 11 用例等全部计入）+ 集成 1 passed**，5 ignored 与基线一致。密码明文不出现在任何日志/测试/示例（测试凭据均为形态合法的假值）。

**未做的实测**：真实 WebVPN 网关的端到端登录（需校外网络+真实凭据，测试环境不具备）；`vpn_test_connection` 的分步文案在真机上的实际观感。下次校外环境可用「保存凭据→连接测试→查询在线信息」三步验证全链路。

### 安卓端前台内存优化：立绘/二维码纹理降采样 + 平板外壳按需加载（2026-10-25，分支 perf/android-foreground-memory，仅安卓端）

**为什么**：用户要求「优化安卓端前台内存占用，但是不要导致前台使用卡顿，功能不全」。真机（25060RK16C，Android 16，1280×2772，density 520）实测定位出两处纯浪费——装饰图按远大于显示尺寸的原始分辨率解码、手机端把平板外壳整棵模块树在启动时求值。

**改了什么**

- **装饰立绘降采样**（`android/frontend/public/girl/` 27 张 webp，PIL LANCZOS + `quality=82 method=6`）：长边统一收到 768——`1024×1536 → 512×768`、`800×1200 → 512×768`、`800×800 → 768×768`、`600×800 → 576×768`。目录体积 4.7MB → 2.2MB，**解码纹理总量 108.8MB → 46.9MB**。
  依据：设备 DPR = 520/160 = 3.25，而安卓端立绘的最大显示宽度是 `w-64`（256 CSS px = 832 设备像素），且全部以 `opacity-[0.10]` / `opacity-[0.08]` / `opacity-[0.06]` 作背景装饰；降采样后 512 宽仍高于 832 设备像素需求的 1/1.6，实际渲染无差异。桌面端 `tauri-app/frontend/public/girl/` 与源稿 `assets/ui-girl/` 未动（平台专属优化，按 AGENTS.md 第 3 条例外条款单端实施）。
- **赞助二维码降采样**（`android/frontend/public/`）：`sponsor-weixin.png` 1325×1806 → 514×700（纹理 9.13MB → 1.37MB）、`sponsor-alipay.jpg` 1170×1755 → 467×700（7.83MB → 1.25MB）。
  功能验证：`pyzbar` 解码两张图均返回 `DECODE-OK ['QRCODE']`，可扫性未损。显示位置 `shared/SponsorCard.tsx:34-46`（`w-1/2`）与 `auth/AboutDialog.tsx:356-367`（`max-h-full`）远小于原图。
- **平板外壳按需加载**（`android/frontend/src/App.tsx`）：`TabletShell` 由静态 import 改为 `lazy(() => import(...))`，`App()` 内 `isTablet` 分支包 `<Suspense>`（fallback 用外壳同款底色占位，避免平板首帧白闪），并在判定为平板时 `useEffect` 预取 chunk。手机形态下 `TitleBar` / `DockNav` / `RightPanel` / `LogPanel` / `StatusBar` / `useSettings` 整棵树**连模块求值都不发生**。
  `FaceCaptureDialog` 保持静态 import——它由验证门（`tauriApi`）命令式驱动，懒加载会在 chunk 加载窗口内丢指令，属功能风险。

**真机实测**（同一台机、同一套流程：`android_launch_app` relaunch → 静置 → `dumpsys meminfo`）

| 场景 | 改动前 | 改动后 |
|---|---|---|
| 冷启动静置 20s（主进程 PSS） | 233MB / graphics 110MB | **227MB / graphics 102MB** |
| 五页签循环后静置（主进程 PSS） | 288MB / graphics 157MB | **273MB / graphics 137MB** |

- **不卡顿**：页签循环全程 `dumpsys gfxinfo` → **914 帧 / 掉帧 7 帧（0.77%）**。
- **功能不缺**：总览、账号、自助服务、监测、更多五个页签逐一截图核对，全部正常渲染；立绘降采样后视觉无差异；主题设置里玻璃档位三档可切换。
- `npx tsc --noEmit` → exit 0；`npx vite build` → 主包 `507.38KB → 475.40KB`（-32KB），新增 `TabletShell-*.js 34.00KB`，>500KB 体积警告消失；`human.esm` 仍是独立按需 chunk（1577KB）。
- 真机验证 APK 由 `pwsh android/build-apk.ps1 -SkipFrontend` 产出（21.63MB，签名校验通过）并 `adb install -r` 安装。

**未做的/已知边界**：主进程 Native Heap（43MB，Tauri/Rust 侧）与 WebView 渲染进程的 `.apk mmap`（约 51MB Chromium 共享代码 PSS）不可减，属框架基线；实测确认玻璃档位不是 graphics 大头（切到 off 档只降约 10MB）。

### 自助服务查询登录会话整体重试：修复偶发「登录页响应体读取失败: error decoding response body」整次失败（2026-10-24，分支 fix/login-reliability，仅桌面 crate）

**现象**：用户报「查询自助服务在线信息失败: 登录页响应体读取失败: error decoding response body」。自助服务服务器 10.1.80.200:8080 偶发连接中途断开/chunked 截断，`read_bounded_body_async`（`auth/protocol.rs`）内部 `resp.bytes().await` 解码失败即整链路失败，且 `login_session` 及全部公开入口一次失败即返回，无任何重试。

**改了什么**（`tauri-app/src-tauri/src/self_service/mod.rs`，安卓经 Cargo path 依赖自动继承，无前端改动）
- 新增 `is_retryable_transport_error`：按错误文案标记串（读取失败 / 无法连接 / error decoding response body / error sending request / 请求失败）判定是否传输层错误；业务失败（如"账号或密码错误！"、"登录会话失效"）不重试。
- 新增 `login_session_with_retry`：包装 `login_session` 整体重试，最多 3 次、间隔 1s；每次尝试重建全新 reqwest::Client（cookie 会话可能被半开连接污染），重试日志带原始错误全文（self_service 错误不含凭据）。
- 公开入口 `query_dashboard` / `query_online_log` / `bind_operator`（经 `login_and_fetch_bind_page`）/ `query_bind_status` / `reveal_credential` / `offline_session` / `change_password` 全部改走重试入口。
- 新增单测 `transport_error_retry_classification`：12 组正反样本锁定分类判据。

**验证**：`tauri-app/src-tauri` 下 `cargo test self_service` → 12 passed / 0 failed（含新用例）；`cargo check` → Finished dev profile in 8.28s，无警告输出。

### 系统级未登录自动登录：SYSTEM 计划任务 + 无头核心 campus-login-core（2026-10-24，分支 fix/login-reliability，仅桌面端）

**为什么**：原「开机自启」走 HKCU Run 键，用户登录 Windows 后才生效；用户要求锁屏/重启停在登录界面（未登录）时也要完成校园网登录。

**改了什么**：
- 新增无头核心 bin `campus-login-core`（`src/bin/campus-login-core.rs` + `Cargo.toml [[bin]]`）：读 `%ProgramData%\CampusLoginCore\core-config.json` → 机器作用域 DPAPI 解密密码 → 循环（上限 10 分钟、每轮 7s）等适配器有 IP → `check_portal_full` 探测 → 已在线退出 / 可登录且非校园检查静默窗时 `do_login_with_retry` 登录；独立追加日志 `%ProgramData%\CampusLoginCore\core.log`（超 512KB 自动截断）。
- `account/crypto.rs`：DPAPI 新增机器作用域（`CRYPTPROTECT_LOCAL_MACHINE`）`encrypt_machine`/`decrypt_machine`（沿用同 entropy），含往返单测——SYSTEM 进程解不开用户作用域密文，凭据必须机器作用域。
- 新模块 `platform/core_autostart.rs`：核心配置组装/序列化（`build_core_config`、camelCase `CoreConfig`）、base64 请求编解码、`select_core_adapters`（名单命中优先否则任意带 IP，3 个单测）、`core_autostart_enabled`（schtasks /query）、`enable/disable_core_autostart`（复用 `helper_spawn` 三级提权通道）。
- `platform/task_proxy.rs`：新增 `register_boot_task_via_com`（BOOT 触发、延迟 30s、ExecutionTimeLimit 15m、SYSTEM 主体、RunLevel Highest、SDDL 同 TASK_SDDL）与 `remove_boot_task_via_com`（幂等删除）。
- `helper/mod.rs`：新 op `register_boot_task`（校验配置 → 原子写配置文件 → `icacls /inheritance:r` 收紧 ACL 至 SYSTEM+Administrators 专有，ACL 失败整体失败并清理 → 校验 core exe 存在 → 注册任务，注册失败删配置不留凭据孤儿）与 `remove_boot_task`（删任务 + 删配置，幂等）。
- IPC：`commands/system.rs` 新增 `set_core_autostart` / `get_core_autostart`，`get_init_data` 增 `coreAutostart` 字段；`app/startup.rs` 注册命令。
- 前端（仅桌面）：设置页「启动设置」卡开机自启下方新增「未登录时自动登录（系统级）」开关（`SettingsPanel.tsx` 本地状态挂载查询，busy 防抖），`tauriApi.ts` 加 `getCoreAutostart`/`setCoreAutostart`，i18n zh/en 文案如实提示密码机器级加密存 ProgramData、仅 SYSTEM 可读、本机管理员可解密。
- 平台专属能力，按 AGENTS.md 例外条款单端实现，安卓端不涉及。

**验证**：`cargo check --bins` 无警告；`cargo test` → lib 497 passed / 0 failed（含新增 dpapi 机器作用域往返与 select_core_adapters 单测）+ 集成全绿；前端 `npx tsc --noEmit --incremental` 通过。
**未验证（真机项）**：计划任务实际注册/开机触发、NSIS 安装包是否随主程序携带 `campus-login-core.exe`、icacls 实际收口效果。手动验证命令：`schtasks /query /tn CampusLoginCoreBoot /v`（注册后应见 SYSTEM/BOOT）；`icacls "%ProgramData%\CampusLoginCore"`（应仅 SYSTEM/Administrators）；重启不登录查看 `core.log`。

### CodeWiki 增量重编：28 篇文章刷新（2026-10-23，分支 chore/sess-session--codewiki-refresh）


**为什么**：安卓账号页两处修复（竖排卡宽 / 密码已保存判据，见下一条）与桌面巡检相关改动落地后，`cw status` 判定 28 篇过期；其中 4 篇超出批量通道预算，改用会话通道逐篇重写。

**改了什么**（仅 `.codewiki/`，无代码改动）
- 模块/概念文章按当前代码重写并重新标注行号锚点：`rust-backend-commands`、`desktop-rust-core`、`frontend-account-module`、`background-check-and-auto-login` 等。
- 「密码已保存」判据写进 `rust-backend-commands` 与 `desktop-rust-core` 的 Known Issues（旧文记的「`password_saved` 恒 false」已标注为已修复），并附 `config_state.rs` 的 `password_saved_at` 与回归测试锚点。
- `_index.md` 索引重建（225 篇）、`_meta.yaml` 编译基线推进到 5bed703。

**验证**：`codewiki_progress` → stage=done，写 11 篇 + 会话通道 6 篇全数落盘；`cw index` → `Index updated: 225 articles`；`cw meta update` → `commit 5bed703`。

### 安卓端账号页两处缺陷：竖排卡宽被压到 64% + 账号卡恒显「密码未保存」（2026-10-23，分支 fix/sess-session--android-card-fixes，仅安卓端）

**现象**（用户真机反馈 + 截图）：
1. 「上下切换卡片宽度不够，导致排版不对」——账号管理页竖排堆叠卡片的当前卡只有半个屏宽，卡内 IP 地址、MAC 信息与底部「使用流量 / 使用时长 / 注销」挤成一团，注销按钮被挤出卡外。
2. 「密码已保存但是卡片显示密码未保存」——账号档案里密码实测已存（弹窗「密码已保存，留空则保持原密码」），但账号卡上的胶囊徽标恒显示「密码未设置」，且卡片被判定为「待补全登录信息」。

**根因**

1. **竖排卡宽误用了水平版参数 + 舞台被 shrink-to-fit 收窄**
   - `android/frontend/src/account/SelfOnlineStack.tsx` 的竖排 `OnlineDeviceStack` 卡槽沿用 `inset-x-[18%]`——这是水平封面流为左右两侧露边让位取的参数，卡宽只有父容器 64%。竖排只靠**上下**露出边条区分当前卡与邻卡，左右不需要让位，18% 纯属误用。
   - 同处舞台 div（原 `className="relative select-none"`）没有 `w-full`，在父级 flex 行里被 shrink-to-fit 收窄，百分比 inset 相对的是收窄后的舞台，卡片进一步偏移。
   - CDP 实测（390×844 视口，`getBoundingClientRect`）：修复前卡宽 202.3px / 舞台 316px = **64.0%**，邻卡 48.0%；舞台无 `w-full` 时卡宽与卡心不变（该父容器本身就是块级满宽），但一旦进入 flex 行上下文即失效，故一并补上。

2. **账号卡密码判据取错了字段**
   - `android/src-tauri/src/config_state.rs:245-248` 的 `EncodedSettings { settings: Settings, password_cipher: String }` 是 disk 形态；`save_file` 落盘时刻意把外露 `settings.password` 清空、只留 `passwordCipher`（明文密码绝不落盘）。
   - `android/src-tauri/src/account_cmds.rs:113` 的 `read_account_card_info` 却取 `s["password"].as_str().map(|p| !p.is_empty())`——读的是那个**恒为空**的外露字段，于是每张卡都判成「未保存」。
   - 前端还有第二条并行的坏路径：`android/frontend/src/hooks/useConfigStore.ts` 的 `mergeConfigFromBackend` 只在字段不处于 dirty 时才合入，另存/删除/切换/改名回传全量掩码 config 时用 `updateConfig` 落地，本地未广播的 `password` dirty 会挡住真实值，`passwordSaved` 随之与卡片、自助服务凭据判断脱节；清除密码后 dirty 标记不复位，后续空密码回传被跳过，`passwordSaved` 永久残留 true。

**改了什么**（仅安卓端；桌面端 `tauri-app/src-tauri/src/config/persist.rs:118-160` 的 `list_account_items` 走的是另一套磁盘格式——`password` 字段本身就是密文、读出来判非空即正确，不存在本缺陷，故不涉及 AGENTS.md 第 3 条的双端同步）

- `android/frontend/src/account/SelfOnlineStack.tsx`：新增 `V_CARD_INSET = 'inset-x-[6%]'`（竖排卡宽 = 舞台 88%，左右各留 6% 露边），卡槽 className 改用该常量；舞台补 `w-full`（撑满父容器，百分比 inset 才有稳定基准）与 `overflow-x-clip`（拖拽/过渡期间不撑出横向滚动条）；卡内底部信息行改 `min-w-0` + `whitespace-nowrap`，让流量值（如 `81449.169M`）可向两端让位而不顶宽，注销按钮加 `shrink-0` 不被挤出卡外。
- `android/src-tauri/src/config_state.rs`：新增 `pub fn password_saved_at(path: &Path) -> bool`，解析 disk 形态 `EncodedSettings` 并判 `passwordCipher` 是否非空（读失败兜底 false）；补单测 `密码已保存判据_落盘后仍为已保存`，同时断言外露 `settings.password` 恒空——把这个判据的前提锁进测试。
- `android/src-tauri/src/account_cmds.rs`：`read_account_card_info` 的 `password_saved` 改调 `config_state::password_saved_at(path)`，并补注释说明为何不能取 `settings.password`。
- `android/frontend/src/hooks/useConfigStore.ts`：`mergeConfigFromBackend` 在合入 `password` 时同步 `passwordSaved`（MASK = 有密码、空 = 无密码），与 `getInitData` 口径同源。
- `android/frontend/src/account/useAccount.ts`：另存/删除/切换/改名四条回传路径统一改为 `updateConfigLocal` + `clearDirtyFields()` + `syncPasswordSaved(config.password === PASSWORD_MASK)`，避免本地 dirty 挡住真实值、切号时密码状态串号。
- `android/frontend/src/account/AccountPanel.tsx`：新增 `applyActiveCardPasswordSaved`，在「填了新密码保存」与「清除密码」两处就地更新激活卡徽标（账号档案要等 `saveCurrentAsAccount` 才回写，中间窗口读的是旧密文）；清除密码后补 `clearDirtyFields()` 让后端回传能裁决状态。

**验证**（实测输出）
- `android/frontend`：`npx tsc --noEmit --incremental` → exit 0；`npx vite build` → ✓ built in 8.22s（仅既存 chunk>500kB 提示）。
- `android/src-tauri`：`cargo check --target aarch64-linux-android --all-targets`（NDK 27.0.12077973 注入工具链）→ `Finished dev profile`，exit 0。
- CDP 浏览器实测（vite dev + 临时注入仓库既有的 `public/mock-tauri.js`，测后 index.html 已 `git checkout` 还原，`git diff` 干净）：
  - 卡宽：**88.0%**（278.1px / 舞台 316px），邻卡 66.0%；卡心与舞台中心同为 195.0px（不再偏侧）；`cardOverflowPx = 0`、`documentElement` 无横向滚动。
  - 对照 A（去掉舞台 `w-full`）：卡宽/卡心不变 → 确认收窄只发生在 flex 行上下文。
  - 对照 B（把卡宽临时还原成 `inset-x-[18%]`）：卡宽回落到 **64.0%**（202.3px）→ 复现用户截图形态，确认这就是根因。
  - 密码徽标：账号卡由「密码未设置」变为「密码已保存」；自助服务 `hasCred` 同步为真。
- `cw status` → `Wiki is up to date`（本次无架构/约定变化，`.codewiki/` 无文章改动）。
- **未做**：真机（物理安卓机）端到端复测——浏览器 mock 环境无法覆盖 Keystore 解密路径，`password_saved_at` 的正确性由 `cargo check` + 单测保证；建议下一次真机安装后核对账号卡徽标与自助服务凭据。

### 整夜休眠后到点不自动登录：登录标记粘性 + 还原后无即时评估（2026-10-22，分支 feat/sess-session--sleep-wake-autologin，仅桌面端）

**现象**：用户反馈「整夜开机电脑休眠后，关闭屏幕后，到了登录时间不会自动登录，检测功能好像失效了，这个登录还是我手动点击的」。日志时间线：20:45:00 已在线 → 23:00:38 以太网 2 已离线 → 次日 08:12:17 以太网 正在登录…（手动点）→ 08:12:17 登录成功 → 08:12:18 已在线。

**根因（代码链实证，两处缺口叠加）**

1. **`has_logged_online` 没有失效重置点（主因，即「检测失效」的真相）**
   - 全部写点：置 true 在 `monitor/auto_auth.rs:91`（准备自动登录成功）、`:185`（断线重连成功）、`:398`/`:453`（开机自启登录）、`monitor/background_emit.rs:193-195`（巡检发现在线时补置）；置 false 只有 `monitor/background_check.rs:101`（校园网检测失败）、`commands/login.rs:174`、`monitor/scheduled.rs:380`（定时注销）。
   - 门控消费点唯一：`monitor/auto_auth.rs:42` 的 `if snap.has_logged_online { return; }`。
   - 夜间出站切换（`monitor/outbound_night.rs` 全文）**没有任何 `has_logged_online` 写点**。20:45 登录成功把标记置 true → 23:00:37 那拍巡检落在静默期（`is_campus_check_silent` 覆盖 23:00–07:40），campus 判定恒为通过 → 不触发 `:101` 的重置 → 23:00:44 切换把校园网卡设备级禁用（会话实际已中断），标记仍是 true → 整夜巡检每拍都在「出站切换态」直接 return，连 campus 判定都不做 → 早上还原后即使补跑巡检，`auto_auth.rs:42` 也立刻 return。**自动登录被永久拦死，只能手动点。**
   - `background_check.rs:97-99` 的既有注释记录过同类缺陷（「仅在登录成功时置位、永不重置…必须手动登录」），但当时只补了「campus 检测失败」这一条路，覆盖不到「网络名没变、会话超时或网卡被禁用后重启」的睡眠场景。
2. **出站还原收尾没有「还原后立即评估」钩子（次因，决定要等多久）**
   - 还原发生在 `monitor/scheduled.rs` 的 30s 调度拍（睡眠唤醒后 `MissedTickBehavior::Burst` 立即补拍），而巡检是 `monitor/background_task.rs` 里独立重建的 `tokio::time::interval`（轻量化 300s）。还原完成后最坏要等一整个巡检周期才重新评估——即使标记没被粘住，也慢一拍。

**改了什么**（仅桌面端；安卓 `android/src-tauri/src/monitor_loop.rs:887-924` 的 `NightOutboundAction::Restore` 本身就是直接 `night_switch_login`，不存在「还原后等巡检」的缺口，按 AGENTS.md 第 3 条平台专属能力例外，不做双端同步）

- `tauri-app/src-tauri/src/monitor/background_check.rs`（+16，后续重构 +47）：在 `try_auto_login_on_preparation` 之前新增会话失效判定——`reachable && !any_online && has_logged_online` 时重置标记并记 info 日志。放在评估之前是关键：**同一拍内就清掉标记，本次巡检就能真正登录**，不必再等一拍。带 `reachable` 前置条件是为了避免探测失败／无 IP 时的瞬时抖动误重置（无 IP 时 `login_available` 本来也是 false，重置没有意义）。判据随后抽成纯函数 `should_reset_logged_online(reachable, any_online, has_logged_online) -> bool` 并补 `mod tests` 四条边界用例（在线不重置／不可达不重置／从未登录不重置／可达且均未在线则重置）。
- `tauri-app/src-tauri/src/monitor/background_task.rs`（+103）：新增 `spawn_background_check_after_outbound_restore(app_handle)`——异步里 `spawn_blocking` 轮询等校园网卡拿到 IP（`POST_RESTORE_IP_POLL_MS = 3_000`，上限 `POST_RESTORE_IP_WAIT_MAX_MS = 45_000`），就绪即调 `spawn_background_check_once` 跑一次巡检。用 `get_adapters_force` 而非 cached：cached 有 TTL，会返回「还原前无 IP」的旧快照，导致白等到上限。抽出纯函数 `post_restore_wait_should_continue(waited_ms, max_ms)` 便于单测。
- `tauri-app/src-tauri/src/monitor/outbound_night.rs`（+6）：`finish_outbound_restore` 尾部挂上述钩子。这里是自动还原／手动还原／启动对账三条路径的唯一汇合点，钩子只挂一次。

**验证**
- `cargo check --lib --message-format short` → exit 0，`Finished dev profile`，无 error。
- `cargo test --lib` → **484 passed / 0 failed / 5 ignored**（较上一条基线 478 净增 6 个用例：`post_restore_wait_stops_at_cap`、`post_restore_wait_zero_budget_never_waits`、`no_reset_while_online`、`no_reset_when_unreachable`、`no_reset_when_never_logged_in`、`reset_when_reachable_offline_and_logged_before`）。单独跑 `cargo test --lib background_check` → `6 passed`（4 条新用例 + 2 条既有 validate 用例）。
- **未做**：真实「整夜睡眠 + 夜间出站切换」端到端复现（需挂机一整夜）。下次同类场景的日志应先出现「夜间出站还原: 已重新启用校园网卡 …」，紧跟「巡检判定未在线（服务可达），已重置登录标记」与「触发自动登录」，可据此确认。

### USB 网卡幽灵态恢复加固：消除假成功 + 复核窗口对称化（2026-10-21，分支 fix/sess-session-usb-recovery-hardening）

**背景**：上一条修复（幽灵态分流 + 总线重枚举）交付后做了两轴 code-review，发现 9 项问题；本条修其中 8 项（第 8 项「`find_disabled_device` 改名」属判断题，改为补文档注释说明函数真实语义，避免波及 5 处调用点）。

**改了什么**

1. **消除假成功（最严重）**——`tauri-app/src-tauri/src/network/adapter_cache.rs`：重枚举后原先只处理 `Disabled` 分支，若设备仍 `Unlocatable` 或实例 ID 彻底消失（`Ok(None)`）则静默 `return Ok(())` 并记「设备实例已恢复」，调用方看到成功就不再重试——与 2026-10-21 事故同类。现抽出 `devnode.rs` 的纯函数 `classify_post_reenumerate`（`Ok(None)` 完成 / `Ok(Some(id))` 补 pnputil / `Err` 上报失败），使该判定可单测。
2. **提权通道超时预算**——`devnode.rs` 新增 `ELEVATED_HELPER_TIMEOUT_SECS = 60`：USB 复核窗口已从 3s 抬到 15s，原 `enable_device` 的 30s 只剩 15s 覆盖 UAC + 计划任务代理 + CMSTPLUA；且 `helper_spawn.rs` 的通道降级链（task_proxy → raw）失败后以同一 timeout 重走一次，总耗时上限实为 2×timeout。60s 保证降级重走后仍盖住 15s 复核窗口。
3. **复核窗口对称化**——`devnode.rs` `disable_device` 原硬编码 `ENABLE_VERIFY_TOTAL_MS`(3s)，USB 禁用同样等价物理拔插，现改用 `verify_window_ms`（USB 15s / 其他 3s）。
4. **父节点回退假成功**——`reenumerate_device_raw` 原在 `CM_Get_Parent` 失败时退回对本实例重枚举；官方语义是「枚举指定设备节点及其子节点」，对叶子自身调用只会静默 `CR_SUCCESS` 什么也不做。现改为直接报错。
5. **去重**——`adapter_cache.rs` 三处逐字重复的「清 ADAPTER_CACHE + 两个 registry 缓存失效」抽成 `invalidate_adapter_caches()`；`helper/mod.rs` 两个 worker 重复的白名单失败四字段构造抽成 `invalid_instance_id_result()`。
6. **消除裸魔法数**——`helper/mod.rs` 的 `problem != 22` 改用 `devnode::PROBLEM_DISABLED`（新增 `pub(crate) const`，与 `CM_PROB_DISABLED.0` 同源）。
7. **弱测试修正**——原 `verify_window_usb_is_longer_than_generic` 的断言属常量自比，新增 `elevated_helper_timeout_covers_verify_window` 锁住「外层预算 ≥ 复核窗口，且降级重走一次后仍 ≥」这一真实约束。
8. **wiki 越界删除回滚**——`.codewiki/decisions/dns-reset-requires-elevation.md` 的 `source_files` 恢复了仍生效的 `tauri-app/src-tauri/src/helper/mod.rs`。

**验证**（实测输出）
- `cargo build --lib` → exit 0，无 warning 无 error。
- `cargo test --lib` → **478 passed / 0 failed / 5 ignored**（较基线 473 净增 5 个用例）。
- 新增用例：`elevated_helper_timeout_covers_verify_window`、`problem_disabled_constant_matches_win32`、`post_reenumerate_still_unlocatable_is_error`（假成功回归锁）、`post_reenumerate_disabled_returns_instance_id`、`post_reenumerate_healthy_completes`。
- **未做**：真实 USB 幽灵态端到端复现（无故障机），重枚举与假成功防护的正确性由上述单测 + Win32 API 官方语义保证。

### USB 网卡幽灵态恢复：启用前分流 + 总线重枚举兜底（2026-10-21，分支 fix/sess-session-usb-phantom-recovery）

**现象**：2026-10-21 凌晨，USB 网卡（Realtek RTL8156，「以太网 2」）被夜间出站切换禁用后整夜无法恢复；用户次日开设备管理器看到设备「无法连接」，且电脑全程未关机。

**根因（日志实证，非推测）**

`app-2026-10-21.log` 时间线：
- 23:00:15 夜间切换设备级禁用该卡 → 正常。
- 08:46:31 起自动启用连续 7 次失败，错误文案全部是 `启用适配器失败：netsh 返回非零退出码但未输出错误信息`。
- 用户 09:12 在设备管理器点「更新驱动程序」、09:14 再点一次，设备才回来。

三处代码缺陷共同把「可恢复的瞬时态」变成「整夜不可恢复」：

1. **`devnode_problem` 查询失败被当成「未禁用」**（`tauri-app/src-tauri/src/network/discovery/devnode.rs:151` 旧实现 `_ => Ok(None)`）。USB 网卡被设备级禁用后端口复位，`CM_Locate_DevNodeW(PHANTOM)` 对它返回 `0x0D CR_NO_SUCH_DEVNODE`（本机实测确认），于是分流判定「设备正常」→ 降级 netsh → 而 netsh 在接口行缺失时把错误写到 stdout 并返回非零，代码只读 stderr（`adapter_cache.rs:149`），拼出「未输出错误信息」这句无信息量的错误。
2. **3 秒复核窗口对 USB 必然误判**。设备级启用对 USB 网卡等价一次物理拔插，RTL8156 还要在 CD-ROM(8151) 与 NIC(8156) 两种形态间切换，3 秒内 problem 22 不可能解除；`enable_device` 因此报错，上层退避重试又回到第 1 条的死路。
3. **没有任何「设备实例消失」的恢复动作**。代码只会对可定位的设备发 `pnputil /enable-device`，设备一旦进幽灵态就彻底无解——用户手动点「扫描检测硬件改动」恰好是唯一缺失的那一步。

**改了什么**（仅桌面端；安卓无 PnP 概念，不适用）

- `devnode.rs`：
  - 新增 `DeviceState { Disabled, Unlocatable }`，`find_disabled_device` 改为三分支：problem 22 → `Disabled`；**查询失败 → `Unlocatable`（不再折成 None）**；正常启动 → `None`。
  - 新增 `reenumerate_device` / `reenumerate_device_raw`：对设备**父节点**发 `CM_Reenumerate_DevNode(CM_REENUMERATE_SYNCHRONOUS)`，等效「扫描检测硬件改动」。只传 SYNCHRONOUS，不传 `RETRY_INSTALLATION`（官方警告后者可能弹驱动安装提示）。复核窗口 15 秒，等实例重新可定位。
  - 新增 `verify_window_ms`：按实例 ID 前缀 `USB\` 选复核窗口（USB 15s / 其他 3s），`enable_device` 与 helper worker 共用同一判据，不再各写一份 3000。
- `adapter_cache.rs`：`enable_adapter` 分流改为 `match`，新增 `Unlocatable` 分支——重枚举 → 若设备回来仍是 problem 22 则补 pnputil 启用 → 清三层缓存后返回。**不再落 netsh**：此刻接口行可能仍在重建，worker 的「必须存在于本机适配器列表」校验会误报「未找到本机适配器」。
- `helper/mod.rs`：新增 `HelperOp::ReenumerateDevice` + `run_reenumerate_device`，让非管理员也能经「计划任务代理 → CMSTPLUA → runas」提权链做重枚举；把实例 ID 字符集白名单抽成 `valid_instance_id`，两个 op 共用（拒绝 `/`、空格、控制字符、`-` 开头，防 pnputil 开关注入）；`run_enable_device` 的 3000ms 硬编码改为调用 `verify_window_ms`。

**验证**
- `cargo check` → `Finished dev profile`，无 error、无新 warning。
- `cargo test`（全量）→ **473 passed / 0 failed / 5 ignored**（基线 397 之上为既有增量，本次净增 4 个用例）。
- 新增用例：`verify_window_usb_is_longer_than_generic`（USB 前缀大小写两态 + PCI 对照）、`reenumerate_nonexistent_instance_errors`（虚构实例必须失败而非静默成功）、`build_op_reenumerate_device`、`instance_id_whitelist_rejects_injection`（`USB\X/remove-device` 等四种注入形态全部拒绝）。
- **未做的实测**：真实 USB 幽灵态的端到端恢复无法在无故障机上复现，重枚举路径的正确性目前只由「虚构实例必失败」与「窗口选择」两个单测 + Win32 API 语义保证。下次同类故障的日志应出现 `设备实例不可定位(USB 幽灵态)，触发总线重枚举`，可据此确认。

### 安卓「更多」页页签：修掉顶部被裁 + 每次进出先黑一下（2026-11-13，安卓）

**为什么**：

1. 页签行容器用 `overflow-x-auto` 却只有 `pb-1`，顶部没有内边距。`.glass-dock` 的外缘暗线（`0 0 0 0.67px`）与近影画在元素边界**外**，滚动容器按 padding box 裁切——顶部这条边界被整体裁掉，药丸顶边读成「被切平了一截」。实测容器 top 与页签 top 同为 78，没有任何余量。
2. 页签照搬了悬浮栏「等祖先链干净再放开半透明」的就绪语义，并且挂了 Hyalite 折射。Hyalite 的位移图是 feImage 引用的 dataURL，贴图解码与首帧栅格化不在同一帧完成；悬浮栏在启动期从 attach 到首次可见隔着整段初始化，贴图早已就绪，而页签是在面板转场结束（祖先链刚变干净）后两帧就 live，可见后的 170–190ms 内 backdrop 采样为空，整颗药丸读成黑底。浅色主题逐帧实测：页签区域平均灰度 43/255（黑），正常态 89/255。

**改了什么**：

- `components/mobile/GlassTabChip.tsx`：不再调用 `useGlassRefraction`，改走 `.glass-dock` 的 CSS blur 降级路径——不挂折射时 `--hyalite` 不存在，`backdrop-filter: var(--hyalite, blur(...))` 自然落到 blur。页签只有 36px 高、圆角 18px（bevel 12 已占 2/3），折射在这个尺寸上本就分辨不出；两档实测平均灰度差 2/255（Hyalite 89.3 / blur 91.4）。
- `index.css`：`.glass-chip` 从「就绪前整条不可见」的等待链里排除。它没有滤镜贴图要等，沿用该语义只会在放开可见的头 170ms 露出黑膜。
- `components/mobile/MobileMore.tsx`：页签行容器 `pb-1` → `py-1`（顶部留 4px 给边界外的暗线与阴影）。

**验证**：

- 安卓 `npx tsc --noEmit --incremental` → EXIT=0。
- vite mock + headless Chrome（412×892 @2x，浅色主题）逐帧 screencast，取页签区域平均灰度：
  - 进入更多页：修复前 `98 → 43.2（持续约 190ms）→ 89.3`；修复后 `247 → 91.6` 单调下降，无黑帧（最小值 91.3）。
  - 切走更多页：修复前 `89.3 → 43.2（约 97ms）→ 59.4 → 243`；修复后 `91.3 → 93.4 → 243` 平滑。
  - 几何：容器 top 78 → 页签 top 82（顶部留出 4px），截图里顶部圆弧、暗线与内缘光完整。
- 对照实验（排除 backdrop root 假设）：把 `.glass-dock` 换成纯 `blur(8px)` 后黑窗消失；把 attach 推迟到祖先干净之后、去掉 opacity 过渡，黑窗都不变——黑窗属于滤镜贴图就绪延迟，不是祖先合成层污染。
- 悬浮栏（`.glass-nav`）：面板切换时它不重挂载、不重新 attach，实测切页全程底栏区域灰度 93.5 → 92.7，无黑帧；启动期从 attach 到首次可见隔着约 700ms 初始化，贴图已就绪，也不复现。故本次只改页签，不动悬浮栏。
- 真机（RMX3300 / Android 15 / 1440×3216）装 release 包复测：更多页页签顶部圆弧与暗线完整（修复前顶部被裁成直边）；底栏切「总览」再切「更多」连拍，页签区域无黑帧。
- 桌面端无此页签（桌面设置页没有「更多」聚合页），属安卓专属组件，未做双端同步。
