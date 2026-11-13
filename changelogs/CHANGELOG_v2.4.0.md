# 更新日志

## v2.4.0（开发中）

### 行为调整

- **安卓：人脸模型移出 APK，按需从 GitHub 下载（支持镜像），APK 减重约 8.9MB**（2026-10-03）。模型不再打进安装包，首次使用人脸录入时下载到应用数据目录。
  - **新模块 `android/src-tauri/src/face_model_cmds.rs`**：命令 `face_models_state`（六文件就位状态）/ `face_models_download`（拉取）。源 = `https://raw.githubusercontent.com/vladmandic/human/master/models/`（实测 master 六文件与原打包内容逐字节一致，上游无 3.3.6 tag）+ 三个前缀代理镜像（ghfast.top / gh-proxy.com / ghproxy.net，与更新链 VERSION_MIRRORS 同构），源顺序跟随 `update_source` 设置（镜像优先时镜像在前）；逐文件：已存在且 sha256 匹配则跳过（断点续传语义），`.part` 临时文件写完原子改名（半截不留正式名），每 200ms emit `face-models-download-progress` 事件；**六文件硬编码 sha256**——前缀代理失败时常返回 HTML 错误页，哈希是唯一真伪判据，失配即删文件并报错提示换网络/镜像重试；`FACE_DL_RUNNING` AtomicBool 防并发重入；落盘 `app_data_dir/files/face-models`。
  - **复用而非复制更新链设施**：`update_cmds.rs` 的 `allowed_url`（下载白名单唯一漏斗）与 `verify_file_sha256`（流式哈希校验）放行 `pub(crate)` 共用，未复制第二份白名单。
  - **前端** `faceService.ts`：新增 `ensureFaceModels()`（模块级共享 in-flight Promise，弹窗先行 + getHuman 兜底并发不双下载；失败清占位可重试），`getHuman()` 的 modelBasePath 改 `convertFileSrc(modelDir)`（human 的 URL 拼接对无尾斜杠 base 自动补 `/`），初始化失败可重试；`FaceCaptureDialog.tsx` 下载前置（失败不开相机直接报错）、下载进度覆盖层（六文件折算 0-100%）、失败「重试」按钮；i18n zh/en 各 +3 键。
  - **WebView 可达**：`tauri.conf.json` 启用 assetProtocol（scope 精确收窄 `$APPDATA/files/face-models/**`）+ CSP connect-src 加 `http://asset.localhost`；`Cargo.toml` tauri features 相应加 `protocol-asset`；删除 `android/frontend/public/models/` 六文件（8.9MB）。
  - 桌面端不涉及（无人脸栈）。验证：前端 `tsc --noEmit --incremental` 0 错误；`cargo check --target aarch64-linux-android --all-targets` 通过（含新模块单测 2 项：镜像前缀形式不变式 / 源排序语义）；真机观察项=首次启用人脸录入的下载进度/断流重试/哈希失配分支。

- **UI：标签栏图标按功能语义重制（桌面 + 安卓同步）**（2026-10-03）。只换 NAV_ITEMS 字形，布局/动画/快捷键/i18n 全不动：网络适配器 Wifi→Network（适配器含有线+无线，Wifi 只表达无线）、网络状态检测 Radar→Activity（脉搏线=状态检测）、网络质量 Gauge→Signal（信号条=质量分级，与质量面板 activeBars 隐喻一致）、测速 Zap→Gauge（速度表=测速经典符号，与「质量」语义解绑）、日志 FileText→ScrollText；安卓手机底栏「更多」LayoutGrid→Ellipsis（M3 惯例，避免与总览网格撞形）。总览/账号/自助服务/设置本就贴名，保持不变。涉及双端 `shared/ui-constants.ts` NAV_ITEMS、两端 `components/layout/DockNav.tsx` ICON_MAP、`android/frontend/src/components/layout/BottomNav.tsx`（手机底栏直接引组件）；各面板内部区块图标（Panel 头部 Radar/Zap/FileText 等）不在本次范围。验证：双端 `tsc --noEmit` 0 错误。

- **行为调整：夜间切换恢复时间 6:30 → 7:30（双端双功能同步）**（2026-10-02）。运营商夜切与夜间出站切换共用常量 `config/night_switch.rs::RESTORE_START_MINUTES` 390→450，恢复窗口变为 `[07:30, 23:00)`——凌晨补切窗口相应变宽（切换态在 00:00–07:29 保持/补切），23:00/23:30 切换时刻与 23:00 恢复窗终点不变；安卓经共享 crate path 依赖自动继承。同步更新：桌面/安卓 i18n 描述（zh/en）、双端代码注释、README、测试边界分钟。验证：`cargo test --lib` 429 passed；`cargo check --all-targets` 0 error；i18n JSON 解析合法。

- **修复：夜间切换漏禁副适配器 + USB 网卡纳入禁用名单 + 当前出站徽标与手动切换按钮**（2026-10-02）。
  - **根因修复（绑源探测）**：10-01/10-02 日志复盘实证——副适配器「以太网 2」（192.168.6.109，校园网段有网关 10.2.127.254）夜间未被禁用，根因是 surge_ping 0.8.4 的 socket2 绑源探测在 FlClash TUN 常驻环境**系统性失败**（非提权进程 RAW 被拒回退 DGRAM，绑源发送失败；同时段 OS `ping -S` 全部正常，应用后台也持续误报「网关不可达」）→ `is_campus_adapter` 把校园网卡误判为非校园 → 不进禁用名单。修复：新建 `platform/icmp_probe.rs`（IcmpCreateFile/IcmpSendEcho2Ex/IcmpCloseHandle 直调，IP Helper 官方探测通道=OS ping 同源，支持绑源、无管理员要求），`network/subnet.rs::check_gateway_reachable_from` 的 v4 路径换用（v6/非 Windows 保留 surge_ping）；**关键坑：RequestOptions 必须传 None**——全零 `IP_OPTION_INFORMATION` 的 Ttl=0 会被原样发出、包在第一跳 TTL 过期（真机诊断 replies=1 status=11013 IP_TTL_EXPIRED_TRANSIT），传 None 用系统默认 TTL 后真机三断言全过（绑源 192.168.6.109→校园网关 status=0x0 可达 / 伪造源不可达 / 不绑源可达，`#[ignore]` 真机测试保留）。同时放弃 /18 网段判定的回退误判担忧——探测修好后 `is_same_subnet_18` 短路保持不变。
  - **USB 网卡纳入禁用名单**：用户确认副适配器即 USB 2.5G 网卡（RTL8156）且要求禁用它（USB 禁用事故为用户本人报告=知情；「USB 禁用后下次开机无法启用」无官方文档佐证，仅用户早年实证）。`select_campus_to_disable` 移除 USB 前缀排除，仅保留「PnP 实例 ID 读不到→跳过」保守分支（参数 usb_guard→bus_guard）；安全网不变：运行期 disable/enable 对称、6:30 还原、启动对账重放、看门狗、手动启用、adapter_cache pnputil 兜底。
  - **新功能：当前出站徽标 + 立即切换/还原按钮**：`platform/best_route.rs` 新增 `current_outbound_v4()`（GetIpForwardTable2 枚举默认路由、有效跃点=路由 Metric+接口 Metric 求和取最小——与 WireGuard/Tailscale/Mullvad 判定口径一致，GetBestRoute 会被 TUN metric 0 干扰故不用；GetIfEntry2 取接口别名）；命令 `get_current_outbound_name` 软降级 Option。前端 NetworkPanel 头部常显「当前出站: X」徽标（TUN/列表外网卡也能展示）+ 两枚按钮「立即切换出站」「立即还原出站」（60s 兜底轮询 + 行内 emerald「当前出站」徽标按名字匹配）。真机验证：当前出站=ifIndex 45 FlClash，与 TUN 常驻现状一致。
  - **手动切换语义（`outboundManualHoldDay`）**：手动切换成功后写入当日日期，冻结当天自动 Switch/Restore（凌晨补切、恢复窗还原均不覆盖手动决策；清理分支——关开关时的快照清理——不冻结，优先执行）；次日自动恢复。`outbound_switch_now` 绕过退避闸但拒绝在切换态重复触发（防把已改跃点当原值重新快照）；`outbound_restore_now` 先清 hold 再还原（失败后自动重试不被冻结）。导入配置保留本机 hold 值不迁移。
  - 验证：`cargo test --lib` 429 passed（含新增 hold 冻结用例与 2 个 `#[ignore]` 真机测试）；clippy 新代码 0 警告；`tsc --noEmit` 0 错误；`vite build` 通过。待夜间实测：23:00 实际切换观察副适配器是否进禁用名单。

- **新功能：夜间出站自动切换（双端）**（2026-09-22，12 个实现任务全部评审通过）。校园网夜里整体断网（周日至周四 23:00、周五六 23:30 后），原「晚间断网自动切换」只在登录凭据层切运营商，覆盖不了整体断网——本次在出站层新增自动切换，与运营商夜切共用时间表、动作互序（切换侧先出站、切入则跳过运营商夜切；恢复侧先还原出站成功才运营商恢复）。
  - **桌面**：到点把排序中的非校园网网卡（热点/物联网卡/随身 WiFi）接口跃点调至 1 使其成为默认出站，06:30~23:00 恢复窗口按快照还原。实现：Iphlpapi 运行时读写（`GetIpInterfaceTable` 读 + helper 新 op `set_metric` 提权写 `SetIpInterfaceEntry`，`SitePrefixLength=0` 防 `ERROR_INVALID_PARAMETER`），不碰持久层注册表——重启天然还原，永不永久性破坏；切换态快照 `outboundMetricRestore`（`[{guid,family,automatic,metric}]` JSON）落盘先行、helper 部分失败不清快照退避补写；三重还原保障（重启兜底 / 恢复窗口主动还原 / 启动对账重放）+ 终态出口（目标卡消失视为已还原）；失败退避 60/120/240/300s、还原失败 3 次告警保留快照；提权首试静默（CMSTPLUA 通道）确因通道失败才降级弹 UAC。目标卡选择按 `outboundPriority` 白名单 + 逐卡非校园网判定（/18 同段或绑源网关可达，SSID 不参与逐卡判定）。切换态期间后台巡检整轮跳过、定时登录跳过不消耗当日标记。
  - **安卓**：到点注销校园网认证（前置在线检查；"离线已生效"判据取 `radiusOk || unbindOk` 任一踢下线信号）→ 写 `nightOutboundRestore` 标记 → `reportNetworkConnectivity(wifi, false)` 触发系统 WiFi 重检，系统 `network_avoid_bad_wifi` 自动把流量切到蜂窝；06:30 恢复窗口重登当前活跃账号后清标记。注销失败上限 3（达上限按断网规律建态交系统验证）、还原失败上限 3（达上限放弃自动还原恢复巡检并要求手动登录）；巡检闸覆盖周期拍/WiFi 事件/手动检测三入口；落盘前重读配置防同拍 stale 复活标记；切账号/删当前账号清标记。
  - **配置字段（双端同构四字段，纯新增无迁移）**：`enableNightOutboundSwitch`（默认关）、`outboundPriority`、`outboundMetricRestore`（桌面切换态快照）、`nightOutboundRestore`（安卓切换态标记）；导入配置清空两个切换态字段（本机系统状态不可迁移）。共享判定纯函数 `config/outbound_switch.rs` 安卓经 path 依赖继承。
  - **UI**：桌面网络适配器页新增「夜间出站切换」卡（开关 + 网卡 ↑/↓ 排序 + 排第一的卡常显「夜间出站目标」徽标，落 `outboundPriority`）；安卓同名卡内含 `network_avoid_bad_wifi` 引导块（adb 命令一键复制 + ROM 私有开关/蜂窝数据/常开副作用边界提示）。i18n zh/en 双语言包双端同步。
  - **待真机实测项**：① 桌面 helper 写 metric 成功之外的实测判据——默认路由确实落在目标卡（`route print` 观察 0.0.0.0 路由的接口与跃点），及夜间 DNS 解析走热点的观察项（DNS 一期不随 metric 走）；② 安卓 `network_avoid_bad_wifi` 引导实测（adb 开启 → 注销 → 系统切蜂窝 → 06:30 重登 → 切回 WiFi 全链路）；③ captive portal 白名单误判场景（个别校园网部署放行系统探测 URL 时注销后验证仍"通过"、不切流量；命中则二期改"注销+提示"降级）。
  - 机制与已知限制（`auto_login_on_start` 未过出站闸、`acceptWifiNetwork` 写 `network_avoid_bad_wifi=0` 不回滚的独立待办、桌面 `nightOutboundRestore` 死字段等七项）详见 CodeWiki `decisions/night-outbound-switch`。
- **改名：「晚间断网自动切换」→「自动切换运营商」**（2026-09-22，双端 zh/en 文案同步）。与夜间出站切换功能在名字上区分——前者切登录运营商线路、后者切系统出站；配置字段名（`nightOperatorSwitch` 等）一律不变保证兼容。决策文档标题同步更新（`.codewiki/decisions/night-operator-switch.md`）。
- **安卓 WRITE_SECURE_SETTINGS 自动写增强：`network_avoid_bad_wifi` 免 adb、自动确保与全量还原**（2026-09-22）。此前夜间出站切换的 `network_avoid_bad_wifi=1` 只能靠用户手动 adb 写入，且 `acceptWifiNetwork` 兜底路径写 `captive_portal_mode=0` + `network_avoid_bad_wifi=0` 后不回滚（决策文档已知限制②）——本次打通应用内自动管理：
  - **Kotlin 侧**（`network-bind` 插件 `NetworkBindPlugin.kt`）：manifest 新增 `WRITE_SECURE_SETTINGS` 声明（`tools:ignore="ProtectedPermissions"`，仍需 adb `pm grant` 一次性授权，授权后应用即可静默写 Settings.Global）；新增快照还原机制——`applyNetworkSettingsCompat` 写两键前先记原值到插件 SharedPreferences（`network_settings_guard`，原值=1 视为系统默认不记录），存在即待还原、不覆盖；新命令 4 个：`ensureAvoidBadWifi`（权限检查 + 幂等写 1 + 记快照）、`restoreAvoidBadWifi`（快照写回，写失败保留快照重试）、`restoreWrittenSettings`（两键全量手动还原）、`getSecureSettingsStatus`（授权/当前值/待还原状态查询）。
  - **Rust 编排**（`monitor_loop.rs`）：Switch 分支落标记成功后自动 `ensureAvoidBadWifi`（替代原「无法程序化确保」warn 日志）；Restore 分支两处清标记后自动还原；启动兜底（标记空但快照在 → 还原，覆盖崩溃/强杀错过恢复窗口）；切账号/删当前账号清标记后还原（`account_cmds.rs`）。原值语义同桌面 metric 快照——还原的是用户系统原值而非默认值。
  - **前端**：NetworkPanel 引导块升级为状态感知——已授权显示绿框「已授权自动管理」；未授权显示 `pm grant com.campuslogin.client android.permission.WRITE_SECURE_SETTINGS` 一键复制（原 `settings put` 手动命令保留为备选）；新增「还原系统设置」按钮（调 `restoreWrittenSettings`，成功/无操作/失败 toast 反馈）。
  - 桌面端不涉及（Android 平台专属能力，Settings.Global 写通道为安卓特有）。

- **桌面 UI 重构：网络适配器与夜间出站切换两卡合并 + 长按拖拽排序**（2026-09-23）。用户需求：两张卡合为一体、排序从 ↑/↓ 按钮点击改为长按拖拽。仅桌面（安卓端排序 UI 不存在，不涉及）。
  - **合并卡**（`tauri-app/frontend/src/network/NetworkPanel.tsx`）：头部左侧「网络适配器 + 检测数」、右侧 MoonStar 图标 + 夜间出站切换开关；头部下方整行功能描述小字。卡片列表 = 全部检测网卡统一列表，每行：拖拽把手 + 有线/无线图标 + 名称/IP/链路速度 + 徽标（主适配器/副适配器/无线/状态/夜间出站目标——排第一的卡常显）+ 行按钮（启用/获取新IP/刷新DHCP）。原「主适配器排最前」的强制排序取消，主/副改由徽标表达；列表底部保留管理员提示、新增拖拽操作提示行。i18n zh/en 同步（删 outboundMoveUp/outboundMoveDown，增 outboundDragHint）。
  - **拖拽排序**（framer-motion `Reorder.Group/Item + useDragControls`，桌面 frontend 已有依赖、零新增）：行体任意处长按 250ms 起拖（移动超 8px 取消，保护滚动/点击），或按住左侧把手立即拖动；行内按钮区 pointer 事件隔离不误触。拖拽中本地顺序渲染，松手一次性提交 `outboundPriority`（拖拽期间屏蔽外部顺序异步回显）；`whileDrag` 抬起态视觉反馈。顺序构建抽纯函数 `outboundOrder.ts::buildOutboundOrder`（出站优先序过滤失效网卡 + 新检测网卡追加尾部，语义与一期 ↑/↓ 版本一致），7 个 vitest 用例覆盖。
  - **验证**：`npx tsc --noEmit` 0 错误；vitest 96/96 全绿；`npm run build` 通过；Playwright 真浏览器实测 4 场景全过（初始序渲染、长按拖拽换序后 `save_config` 恰一次且 `outboundPriority` 正确、快速滑动不重排不提交、把手拖拽正确）。
  - 决策与实现细节见 CodeWiki `decisions/night-outbound-switch`「桌面卡片合并与拖拽排序（三期）」节（一期「不引入拖拽」决策撤销）。

- **桌面 UI 迭代：适配器排序撤销把手通道，仅保留长按拖拽；文案改「切换为非校园网」并提示拖动决定切换目标**（2026-09-23）。用户需求：去掉点击特定卡片（把手）进行排列，改为长按条目任意位置即可排列；描述改为切换为非校园网；增加提示用户通过拖动切换。仅桌面（安卓端无排序 UI，不涉及）。
  - `tauri-app/frontend/src/network/NetworkPanel.tsx`：删除行内把手按钮（GripVertical + `pointerdown` 立即起拖通道），仅保留行体长按 250ms 起拖（`NetworkPanel.tsx:52` 常量 + `:54` 8px 死区，保护滚动/点击）+ 行内按钮区 pointer 隔离；本地顺序渲染、`onDragEnd` 一次性提交 `outboundPriority` 语义全部不变。GripVertical 图标保留——列表底部提示行装饰图标仍用它。
  - i18n zh/en 同步：`nightOutboundSwitchDesc` 改为「…自动把出站切换为非校园网（热点/物联网卡）…」（从机制描述改为面向效果）；`outboundDragHint` 改为「长按条目任意位置拖动调整顺序，排最前的非校园网网卡即夜间切换目标」（把「拖动 → 排最前 → 成为切换目标」因果写进提示）；`outboundAdminHint` 删去与徽标重复的「排第一的网卡即夜间出站目标」。
  - 验证：`npx tsc --noEmit` 0 错误；vitest 96/96 全绿；Playwright 真浏览器实测 6 场景全过（三处新文案在 DOM、行内无把手按钮、首行 WLAN+徽标、快速拖不重排不落盘、长按「以太网 2」到顶 `save_config` 恰一次且 `outboundPriority` 正确、原地长按不落盘，0 page error）。
  - 决策文档见 CodeWiki `decisions/night-outbound-switch`「桌面排序仅保留长按拖拽（2026-09-23 四期）」节。

- **桌面 UI 文案精简：夜间出站切换描述去括号补充、去「需保持应用在后台运行」**（2026-09-27）。用户需求：①「非校园网」后括号（热点/物联网卡）补充去掉；②「需保持应用在后台运行」去掉。`nightOutboundSwitchDesc` zh/en 同步改为「周日至周四 23:00、周五六 23:30 自动把出站切换为非校园网，次日 6:30 起自动还原」。同日对 2026-09-26 夜间断网做日志复盘（切换成功但流量未跟随：FlClash TUN auto-route metric 0 压制 + 6:30 恢复窗口应用未运行，还原由启动对账补做），机制与二期候选方案见 CodeWiki 决策文档「文案精简与夜间断网实测复盘（五期）」节与已知限制⑧。

- **桌面夜间出站切换重构：「禁用校园网网卡 + 兜底路由 + 保留 metric」三件套**（2026-09-27）。五期复盘确认纯跃点切换被 FlClash TUN auto-route（0.0.0.0/0 RouteMetric=0）压制——跃点修改不产生路由事件，mihomo auto-detect-interface 不跟随，流量焊死在已断的校园网卡整夜断网。用户拍板用「禁用+兜底路由+保留 metric」替换五期候选方案：
  - **切换**（`monitor/scheduled.rs::apply_outbound_switch`）：目标卡照旧两族 metric=1；新增禁用校园网网卡——除目标卡外、连接中的校园网卡逐张 helper `disable_adapter`（USB/未知总线保守跳过：PnPInstanceId 前缀 `USB\` 或读取失败一律不禁，防 USB 网卡下次开机提示安装）；名单 `outboundDisabledAdapters`（`[{guid,name}]`）只记亲手禁用的卡，不靠状态推断；新增兜底路由 `0.0.0.0/0 metric=2` 指向目标卡（helper `route_add`，排 TUN 之后，仅在 FlClash 退出后接管 failover），`outboundStandbyRoute` 落盘。禁用产生接口/路由事件，mihomo 重选出站 → TUN 出站落在热点卡。成功通知「已切换出站到 X，已临时禁用 N 张校园网网卡」。
  - **还原**（`apply_outbound_restore`）：route delete（失败仅告警不阻塞，运行时路由重启即清）→ metric 按快照写回 → 逐卡 enable（名单外不碰、卡已不存在跳过；enable 失败=还原失败整体重试保留名单）；成功通知「已还原网络设置（启用 N 张校园网网卡）」，无卡启用时沿用「已还原网卡跃点设置」。
  - **三快照切换态**：`outboundMetricRestore`（原有）+ `outboundDisabledAdapters` + `outboundStandbyRoute`，任一非空即切换态；启动对账 replay 改三分支幂等补做（metric 补写 + 禁用补做 + 路由补加）；导入配置清空三字段；提权 helper 新增 `disable_adapter`/`route_add`/`route_delete` 三个 op（路由行编码 `dest|mask|gateway|metric|if_index` 严格 Ipv4 解析防注入）。
  - **配套修复**：① USB 网卡拔出后幽灵设备误报「已禁用」（AdminStatus/ConfigFlags 残留 DISABLED 位）——`classify_adapter_status` 加 PnP Present 判据，幽灵设备直接判未连接（用户截图实证：以太网 2 实为未连接却显示已禁用）；② adapter_watch 自动启用跳过本功能夜间禁用的卡（名单过滤，09-26 实证 PnP problem 22 自动启用与夜切冲突）。
  - 验证与已知限制见 CodeWiki 决策文档「禁用+兜底路由方案落地（2026-09-27 六期）」节。

- **桌面通知 AUMID 自有化：通知来源显示应用名而非 PowerShell**（2026-09-27）。未打包应用借用 PowerShell AUMID 的旧做法让通知中心来源显示 "Windows PowerShell"——首次发通知前向 HKCU `Software\Classes\AppUserModelId\com.campus.login` 写 `DisplayName`（校园网登录助手）与 `IconUri`（`icons/128x128.png` 加入 bundle.resources；HKCU 免提权），`show_system_toast`/`show_update_toast` 全部改用自有 AUMID；注册失败（策略封锁等）回退 PowerShell AUMID 保证通知仍弹出，dev 模式无安装图标只缺来源角标。决策修订见 CodeWiki `decisions/update-notification-per-platform`。

- **桌面 DNS 优化卡内置目标适配器多选，不再跟随适配器设置卡**（2026-09-27）。用户需求：去掉「跟随适配器卡片」的隐式范围，改为在 DNS 卡内部直接选择要优化的网卡：
  - **后端**（`commands/network_cmd.rs`）：`setup_dns_doh` 与 `reset_dns` 的目标从 `resolve_adapter_names` 主/副解析改为配置字段 `dnsOptimizeAdapters`（网卡名多选名单）；空名单提示「请先在 DNS 优化卡片选择」，名单内卡已断开/不存在则过滤（全不可用提示重新选择）。提权 helper 通道（dns/clear_dns op）与 `filter_operation_adapters` 等既有链路不变；DHCP 续租仍走主/副范围。
  - **前端**（`NetworkPanel.tsx` DNS 卡）：「优化目标」行下新增「目标适配器」chip 多选区（候选 = 已选名单 ∪ 当前有 IP 的活跃卡，已选但暂不活跃的卡也显示保证可取消）；选中态主题色胶囊，点击 toggle 即经 `onUpdateConfig` 落盘；一键优化/恢复按钮在前端同样拦截空名单（warning 日志）。i18n zh/en 同步（dnsAdapterLabel/dnsAdapterTip/dnsNoAdapterSelected/dnsAdapterEmpty）。
  - **配置字段**：`dnsOptimizeAdapters: string[]`（默认空数组，纯新增无迁移）；安卓端三文件镜像（config_state.rs + types/constants.ts，不消费）。
  - 决策修订见 CodeWiki `decisions/adapter-operation-scope`（DNS 设置/恢复摘出主/副范围）。

- **桌面 UI 精修：SettingRow/CardIcon 基元、参数重分组与动画修复**（2026-09-27，仅 Windows 端）。用户需求：排版优化、卡片重设计、各卡参数重新分类放置、动画优化，做到方便快捷美观；经 GitHub 调研（Clash Verge Rev / Spacedrive / shadcn blocks / Tremor / Motion 官方文档）定案「体系内精修」——先抽基元锁死层级，不换技术栈不重写。
  - **新基元**（`tauri-app/frontend/src/components/ui/`）：`setting-row.tsx` 新增 **SettingRow**——参数行唯一形态（label 左 + description + 右侧控件，`min-w-0`/`shrink-0`，可选彩色语义 icon 与 htmlFor），收编全库 57 处手写 `justify-between` 参数行；`card.tsx` 新增 **CardIcon**（w-10 h-10 rounded-xl bg-primary/10 图标片），统一此前 rounded-full/rounded-xl 混用的手写卡头，卡头标题区补 `min-w-0`。涉及 SettingsPanel/AccountPanel/MonitorPanel/NetworkPanel/QualityPanel/DashboardPanel/SelfServicePanel/SpeedTestPanel/LogPanel 全部卡头与行式参数。
  - **参数重分组**：SettingsPanel 启动卡 8 参数按「启动行为（autoLaunch/hiddenStart）/登录自动化（autoLoginOnStart/autoExitAfterLogin/autoExitOnOnline）/窗口与界面（lightweightMode/minimizeToTray/defaultPanel）」三组重排（组标题 + 组间 Separator）；通知+安全两卡合并为「通知与安全」单卡（7→6 卡，组标题复用 settings.security）。MonitorPanel 校验卡按「自动化/校园网校验」两组标题分区，检测逻辑五步说明移到组标题旁问号 Tooltip，SSID/网关固定值行转「行式 + 右侧 mono 框」。`enableQuality` 关闭联动与 Windows Hello 验证（handleSecurityDisable）逻辑原样迁移。表单输入与数据展示行保持原布局，四个时间窗行保留窄屏堆叠结构（learnings/input-time-wrapper-w-full）。
  - **动画修复**：三个 GSAP hook（useBreatheAnimation/useGlowAnimation/usePulseAnimation）存在条件渲染元素重建后动画静默失效的 bug（tween 绑定在已卸载的旧 DOM 节点上）——统一改回调 ref 模式（node 进 effect 依赖，重挂载即重建动画）。RightPanel 空态呼吸（空→非空→空后失效）、质量劣化红色光晕（劣化→恢复→再劣化后失效）、DockNav 加载脉冲三处受益。其余动效按调研结论保持现状：入场动画一次性不绑数据刷新、hover/开关走纯 CSS、reduced-motion 与 economy 动画分档不动。
  - i18n zh/en 各同步新增 7 键（notificationSecurity/notificationSecurityDesc/groupStartup/groupLoginAutomation/groupWindowBehavior/groupAutomation/groupCampusCheck）。安卓端为独立复刻树，本次不动（双端同步欠账，如需同款基元另次任务移植）。决策与基元规范见 CodeWiki `decisions/desktop-ui-card-primitives`。

- **桌面 UI 精修二轮：设置页两列空洞填补 + 五项 UX 改进**（2026-09-27，仅 Windows 端）。用户指出设置页中部 345px 空洞并批准五项改进（调研依据：shadcn/Tremor 仪表盘判据、Atlassian 四段式空态、Polaris 徽标语义、NN/g 渐进披露）。
  - **设置页两列均衡**（`settings/SettingsPanel.tsx`）：原「启动设置高卡 + 下方两张全宽卡」改为手动两列 items-start——左列=启动设置+数据管理导出卡，右列=通知与安全+质量检测卡（enableQuality 关闭联动逻辑原样迁移）+新手引导；弃 justify-between 拉伸，列尾参差 ~170px 属正常。
  - **Dashboard 首屏摘要带**（`auth/DashboardPanel.tsx::DashboardSummaryBand`）：卡片列表上方新增三段 KPI 行——校园网认证（useAuthStore.status 五态色板与 StatusBar 同步维护）/ 适配器在线数 / 网络质量（resolveQualityDisplay 延迟值按质量着色），点击各段直达对应面板，质量段可就地刷新；enableNetworkQuality=false 整段隐藏。数据与 StatusBar 同源零新轮询。StatusBar 质量胶囊新增点击锚点直达总览摘要带（`monitor/StatusBar.tsx`，hover 明细弹层不变，i18n +1 键 statusbar.gotoDashboard）。
  - **Quality 四段式空态**（`monitor/QualityPanel.tsx`）：质量卡与测试明细卡在「无数据且非检测中」时显示 图标+标题+一句原因+「立即检测」主按钮 的虚线空态（替代 5 行 "--" 占位与 loading 空转）；检测中保留行内 pulse skeleton。
  - **Account 绑定卡渐进披露**（`account/AccountPanel.tsx`）：绑定状态区（掩码账号+查询按钮）常驻，绑定表单默认收起；披露入口 aria-expanded + 标签写明点开所见 + ChevronDown 旋转；展开动画 opacity/y ±6（不动 height，避开布局动画与 reduced-motion 分档）。
  - **badge 语义色**：定时测试「运行中」badge 绿→蓝+Loader2 spinner（Polaris 规则：绿=终态成功、进行中不占色相）；「已在线」绿保留。
  - **可发现性**：Dashboard「获取新IP」双适配器时按钮尾端常驻 ChevronDown（菜单开合旋转）；NetworkPanel 可排序行左侧常驻低对比 GripVertical 把手（hover 加深，长按起拖逻辑与 250ms/8px 参数不变）；Dashboard 卡片编辑按钮 ghost→outline。
  - i18n zh/en 同步新增 12 键（dashboard.bandAuth/bandAdapters/bandQuality/bandOnlineCount；quality.emptyTitle/emptyDesc/runTestNow/emptyDetailsTitle/emptyDetailsDesc；account.bindFormToggle/bindFormToggleDesc）。安卓端不动（双端同步欠账）。决策见 CodeWiki `decisions/desktop-ui-card-primitives`「第二轮：六项 UX 精修」。

- **桌面 UI：删除 Dashboard 首屏摘要带**（2026-09-27）。用户裁定「去除首页的这个样式设计没有必要」——`DashboardPanel.tsx` 撤 `DashboardSummaryBand` 三段 KPI 行（校园网认证/适配器在线/网络质量，昨日精修二轮引入），组件与用法、仅 band 使用的 `ChevronRight` import 一并移除；质量卡就地刷新（onRefreshQuality→QualityPanel）与 StatusBar 质量胶囊锚点不受影响；i18n zh/en 各删 4 键（bandAuth/bandAdapters/bandQuality/bandOnlineCount）。验证：tsc 0 错、vitest 16 文件 98/98 全绿、vite build 通过。

- **安卓端通知权限主动索取：首次进入即引导，三处入口统一收敛**（2026-09-21）。
  - 调研结论（GitHub 一手资料，2026-09-21）：Android 13+ 通知受 `POST_NOTIFICATIONS` 运行时权限管控；13 以下与国产 ROM（MIUI/EMUI/ColorOS/OriginOS 等）不存在任何能弹框的标准 API，用户在系统设置里关掉通知后应用侧唯一手段是 `areNotificationsEnabled()` 检测 + 引导跳应用通知设置页（蓝本 getActivity/XXPermissions 的 `NotificationServicePermission` 跳转链；`tauri-plugin-notification` 只覆盖 13+ 弹框，13- 的 `requestPermission()` 是无害空转）。13+ 拒绝两次即永久拒绝，之后 `requestPermission()` 不再弹框，跳设置页是唯一路径。
  - 新增安卓专属命令 `open_notification_settings`：Kotlin 插件 `MonitorServicePlugin.openNotificationSettings` 实现降级链——① API 26+ `ACTION_APP_NOTIFICATION_SETTINGS`（API 21-25 裸 action + `app_package`/`app_uid`）→ ② 应用详情页 → ③ 通用设置，逐个 try/catch（沿用 `openVendorBatterySettings` 的「不用 resolveActivity 预探测」经验，避免 Android 11+ 包可见性误判）；Rust 侧 `battery_cmds.rs` 转发（`#[cfg(mobile)]` 模式，桌面调用返回错误）。
  - 首次进入索取：App 顶层新增 `NotificationPermissionGate` 引导弹窗（`ConfirmDialog` 正色按钮复用，新增可选 `confirmLabel`/`confirmVariant` props），无 `campus-notification-prompt`（safeStorage）标记即弹，点「去开启」→ 13+ 弹系统框、仍无权限则跳设置页；点任意按钮即标记、不重复弹。新手向导为后挂载全屏覆盖层，天然先于本弹窗展示，无需时序协调。
  - 三处触发统一收敛到新 helper `android/frontend/src/lib/notificationPermission.ts` 的 `requestNotificationPermission({ openSettingsIfDenied? })`：① 首启引导弹窗（跳设置）；② 设置页「启用通知」开关 + TitleBar 铃铛开关打开时（跳设置——用户明确要通知）；③ 启动后台检查（只弹框不跳设置——间接路径不打断操作流）。原 useMonitor 内联的 13+ 请求逻辑已替换为 helper。
  - 桌面端不涉及（安卓平台专属能力例外，无双端同步义务）。决策与厂商适配边界记录于 CodeWiki `decisions/android-notification-permission-prompt`。

- **新功能：状态栏当前在线运营商徽标（双端）**（2026-09-30）。Portal 页面探测解析 uid（完整在线账号=账号+运营商后缀）推导真实在线线路，挂在既有探测响应上零新增网络请求；uid 解析结果只进 emit/展示链，绝不进 auto_auth 判定输入（注销保护期强制不显示，断线重连闸门链不感知此字段）。
  - **后端**（双端共用 Rust 核心）：`auth/portal.rs` 新增 `operator_suffix_from_uid`（rsplit_once('@')）与 extract_uid_from_html 单测；`monitor/background_check.rs` 主/副适配器各自解析（仅对应适配器在线时取值）；`monitor/background_emit.rs` `background-check-result` 事件新增 `onlineOperator`/`secondaryOnlineOperator` 字段（保护期→null）。
  - **桌面 UI**（StatusBar 状态胶囊旁）：chip 徽标=运营商 logo+名称，logo 为本地 WebP 资产（public/isp/{telecom,unicom,mobile,school}.webp 共约 7KB，零新依赖）；新增 `shared/IspMark.tsx` 映射后缀→资产，无锡学院（空后缀）用白圆底校徽；夜切临时态 Moon 图标从 chip 撤下，恢复目标仅在 tooltip 标注；tooltip 展示主/副完整在线账号；仪表盘账号管理卡「当前」行同步显示（一期）。
  - **双适配器**：副适配器在线且运营商与主不同时并排第二枚 chip（相同则单 chip 不重复），副账号进 tooltip（i18n accountSecondary，zh/en 双语言包双端同步）。
  - **安卓**：平板壳 StatusBar 与桌面同源直接获得徽标；手机壳顶部条不渲染桌面件（StatusBar 不挂载），等价落位在总览「网络状态」卡新增「当前在线运营商」行（同款 chip），`useFormFactor` 门控仅手机形态渲染（防平板壳 StatusBar+卡片双处重复）。
  - **验证**：cargo test 428 通过 0 失败（基线保持）；桌面 vitest 徽标 5 用例（logo chip/夜切无 Moon/旧后端降级/双适配器第二枚/同运营商不重复）+锚点 2 用例全绿；双端 tsc --noEmit 0 错误；Edge+mock 注入实测桌面 4 场景（电信/双适配器/夜切/学校线路）与安卓手机 3 场景+平板 1 场景渲染正确。

- **安卓常驻通知防折叠：服务侧 12h 定时刷新 + 通知可点击回应用**（2026-10-01）。用户报告：隔天后澎湃OS 把常驻通知折叠进「更多消息」。根因与厂商机制：2026-09-12 起 Rust 侧为省功耗只在在线状态翻转时 notify（`monitor_loop.rs`），连接稳定时通知可数天不重建；小米官方文档明确 MIUI/HyperOS「通知过滤」按评分模型折叠低分通知——维度为通知文本 + 该应用在本设备过去 30 天历史点击率（影响最大）+ 用户属性，且过滤阈值在手机激活后 14 天内从 0 逐小时增长（解释「隔天后消失」）。修复仅动 `ForegroundService.kt`，Rust 侧零改动：
  - **通知可点击**：`buildNotification` 补 `setContentIntent`（`PendingIntent.getActivity` → `setClassName(packageName, "${packageName}.MainActivity")` 沿用 BootReceiver 先例 + `FLAG_IMMUTABLE`，minSdk 24 无需版本守卫）。点击率是评分模型最大因子，不可点击的常驻通知点击率恒为 0，迟早被折叠；插件 `updateNotification` 与服务侧共用 buildNotification 单一漏斗，一并生效。
  - **12h 定时刷新**：companion 新增 `REFRESH_INTERVAL_MS = 12h` 与 `@Volatile lastText`（buildNotification 记录当前文案）；服务侧 `refreshHandler`（主线程）在 `onCreate` 挂 `postDelayed`，到点重建通知（`startAtMs` 拨到 now + 复用 `lastText`），`onDestroy` `removeCallbacks`。原「固定为服务启动时刻保 Chronometer 连续」的设计随之作废——运行时长每 12h 重计一次（重启观感等同重开服务，已接受）。
  - **不做通道 importance 迁移**（留作后备）：评分模型未列 importance，投机迁移且有用户感知风险。
  - **2026-09-08 决策维持**：不做厂商私有 extras（小米焦点通知等），统一 Android 标准协议（ongoing + Chronometer + CATEGORY_SERVICE）。
  - **其他国产 ROM 调研结论**（官方文档，引导用户自查，未做代码适配）：荣耀/华为按通知重要性分级展示（低重要性不显示/折叠；引导用户关闭应用通知的「静默通知」或用华为「优先通知」智能识别）；vivo OriginOS 支持 Android 12+ FGS 通知延迟 10s 与按类型静默；ColorOS 流体云识别标准 promoted ongoing（本通知特征已满足）。澎湃OS 用户侧纠正：左滑通知「设为重要」，或设置→通知和状态栏→关闭「通知过滤」。
  - **验证**：一键出包 `pwsh android/build-apk.ps1` 全流程通过（vite + `tauri android build --target aarch64 --apk`），APK 产出 `gen/android/app/build/outputs/apk/universal/release/`；期间先以 `:tauri-plugin-campus-monitor-service:compileReleaseKotlin` 单模块快速回归（补漏的 `android.os.Handler` 导入）。真机长周期观察项（12h 刷新后澎湃OS 不再折叠）列入设备侧清单。

- **双端悬浮栏激活页签改纵排：图标在上、文字标签在下**（2026-10-01）。用户需求：悬浮栏激活页签的文字从图标右边移到图标正下方（原话「图标文件显示位置」实为「图标文字」笔误）；范围仅页签，注销/登录快捷按钮保持横排不变。
  - **桌面 DockNav**（`tauri-app/frontend/src/components/layout/DockNav.tsx` DockItem）：按钮容器 `flex items-center gap-1` → `flex flex-col items-center gap-0.5`，图标与标签纵向堆叠（rounded-full/px-2.5/py-1.5、tooltip -top-9、layoutId="dock-active-pill" 药丸 inset-0、磁吸 GSAP 全部不动）；激活项内容增高按钮约 30→43px，药丸自动跟随新几何。
  - **安卓平板 DockNav**（`android/frontend/src/components/layout/DockNav.tsx`，TabletShell 使用、与桌面同源）：与桌面同构同改。
  - **安卓手机 BottomNav**（`android/frontend/src/components/layout/BottomNav.tsx`）：页签按钮 `h-10` → `h-12` + flex-col + gap-0.5 统一纵排高度（纵排内容 20+2+12.5≈34.5px 需要更高统一高度），标签 span 删 ml-1（横排间距改纵排 gap）；CompactAuthButtons 注销/登录横排不动；激活药丸随按钮变高。非激活项仍纯图标。
  - **验证**：双端 `npx tsc --noEmit --incremental` 0 错误；puppeteer（Chromium + 双端 vite dev + mock-tauri 注入，用后完整还原、worktree 仅剩 3 个组件文件改动）真浏览器实测 5 截图全过——手机壳亮/暗 + 点击切页签（药丸滑到「账号」纵排跟随）、平板壳 DockNav、桌面 DockNav，激活项均为图标上/文字下药丸包住，注销/登录横排不变。

- **安卓手机悬浮栏页签图标等比例放大：20→24px 方便点按**（2026-10-01）。用户需求：等比例放大安卓手机悬浮图标大小，方便用户点击。`BottomNav.tsx` 页签图标 `h-5 w-5`（20px）→ `h-6 w-6`（24px，Material 推荐导航图标尺寸），非激活页签按钮 `w-8`（32px）→ `w-9`（36px）同步保持 6px 图标边距与整体比例；激活药丸（inset-0）随内容自适应，注销/登录快捷按钮与标签字号不动。
  - **验证**：`npx tsc --noEmit` 0 错误；puppeteer 真浏览器实测三场景全过——412px 亮色（「总览」激活、图标明显放大）、412px 点击切页签（药丸滑到「账号」纵排跟随）、360px 窄屏回归（五页签+注销/登录无溢出、「账号」标签完整显示）。

- **行为调整：「自动切换运营商」默认改回关闭（双端）**（2026-10-02）。2026-09-20 起该开关曾默认开启，用户裁定回归默认关闭——新装/重置不再默认夜间切线路。双端后端 Default（`tauri-app/src-tauri/src/config/model.rs` / `android/src-tauri/src/config_state.rs`）与双端前端 `DEFAULT_CONFIG`（双端 `settings/constants.ts`）四处翻转；**存量落盘值不迁移**（沿用 2026-09-20 auto_exit 默认值变化先例：已显式落盘的 true 保持不动，由用户在设置页自行关闭）；历史迁移块 v4→v5 / v6→v7 保留原样（迁移记录语义，更旧的存量配置升级仍按当时默认刷开）。UI 与判定逻辑零改动。

### 缺陷修复

- **修复安卓常驻通知被折叠进「更多消息」：通道 IMPORTANCE_LOW 是底层判据，版本化换通道升 IMPORTANCE_DEFAULT**（2026-10-03）。用户报告 2026-10-01 的 12h 重建 + 可点击优化后依旧被收进「更多消息」，要求从底层解决。根因：`NotificationChannel` 的 importance 创建后不可变，现役通道 `campus_monitor` 建于 `IMPORTANCE_LOW`——importance 低于 DEFAULT 在 AOSP 即静默通知桶（无横幅无提示音），也正是国产 ROM（澎湃OS 通知过滤）折叠「更多消息」的底层判据；通道档位是评分模型（文本/点击率因子）之上的先决条件，前两轮优化动不了它。修复仅动 `ForegroundService.kt`：
  - 通道 ID 版本化 `campus_monitor` → `campus_monitor_v2`（importance 不可变，只能换 ID 重建），`startForegroundWithText` 建新通道 `IMPORTANCE_DEFAULT` + `setSound(null, null)` + `setShowBadge(false)`——无铃声、无角标、DEFAULT 不触发 heads-up 横幅（那需要 HIGH），安静常驻体验不变；随后 `deleteNotificationChannel("campus_monitor")` 清掉旧低档通道。
  - **撤销 2026-10-01「不做通道 importance 迁移（留作后备）」决策**：折叠投诉坐实 LOW 档即主因，后备方案启用；其余不变（12h 刷新、可点击、不做厂商私有 extras）。
  - foregroundServiceType 不动（Android 15 BOOT_COMPLETED FGS 限制红线）。验证：`:tauri-plugin-campus-monitor-service:compileReleaseKotlin` 通过；真机观察项=升级后常驻通知应进常规列表不再折叠（旧通道随 v2 建立自动清除）。

- **检测链路提速：非校园网拍删除 Portal HTTP 探测空转，离网反应 11s→3s**（2026-10-02）。用户需求：降低各检测环节耗时、加快反应速度。逐环节盘点：WiFi 断开事件延迟已为 0（`WIFI_LOST_EVENT_DELAY_MS=0`，2026-09-14）、连接方向 2500ms 等 DHCP 有注释依据不动、探测内 TCP 双探已并行（最坏 3s）——唯一剩余空转是**非校园网拍的 Portal HTTP 探测**：`run_check_once` 在 `on_campus=false`（子网未命中且网关/Portal 两个 3s TCP 探测已并行执行且全败）后照常起小核线程发 HTTP 请求，同一目标同一端口必败，空转至多 8s（`portal_config::CLIENT_TIMEOUT`），最坏 3+8=11s。
  - 修复：`android/src-tauri/src/monitor_loop.rs run_check_once` Portal 探测 match 改 `(source_ip, on_campus)` 分流——`(Some(src), true)` 才发 HTTP 探测；`(Some(_), false)` 直接 Err「不在校园网,跳过 Portal 探测」；`(None, _)` 维持「WiFi 未连接」短路。语义论证：`on_campus=false` ⇔ TCP 探测必已执行且全败（子网命中强制 `gateway_ok=true`、SSID 命中早退 `on_campus=true`，均不可能落此分支），HTTP 同目标必败，online 判定 `match &portal` 对 Err 本就落 `_ => false`，与今天 HTTP 失败路径同语义；在线判定、掉线通知（`was_online && !online && on_campus` 闸）、自动登录闸均不受影响。
  - 维持现状项：校内 Portal 宕机（`on_campus=true`）仍走 HTTP 探测由页面请求裁决（属真实判定场景）；`CLIENT_TIMEOUT=8s` 不动（与登录流程共用 client，桌面同源）；Portal URL 配置非 80 端口且 80 被滤的理论错位边界不处理（Dr.COM 默认 80，TCP 探测恒打 PORTAL_PORT=80）。
  - 验证：`build-apk.ps1 -SkipFrontend` release 构建通过（纯 match 重排，无新依赖）；真机核验点=关闭 WiFi/离开校园网后通知与状态页在单拍内（约 3s）翻转，不再出现 8s 空转。

- **安卓网络状态检测无法识别 WiFi 已关闭：探测链全程无 WiFi 概念——无非蜂窝接口时短路探测并给出明确状态**（2026-10-02）。用户实测：关闭 WiFi 后检测一直在继续，状态停在「未连接校园网」。根因：整条探测链（`campus_detect.rs` → `monitor_loop.rs` → 常驻通知）只有「连着 WiFi 但不在校园网」的语义；WiFi 关闭时仅剩蜂窝接口（`pick_campus_source_ip` 刻意排除 rmnet*/ccmni*，返回 None），每拍照样空跑网关+Portal 并行 2×3s TCP 探测（走蜂窝必失败）加 Portal 小核线程探测，并笼统报「未连接校园网」无法区分 WiFi 已关闭（`should_attempt_login` 的 `!on_campus` 闸已挡住自动登录，但检测本身无短路概念）。
  - 修复（零 Kotlin/插件/前端改动，前端文案经既有 payload 通道自动透传）：`campus_detect.rs` `CampusProbe` 新增 `wifi_off` 字段（=source 为 None），`probe_campus` 枚举网卡后立即短路返回（省 2×3s 空转）；`monitor_loop.rs run_check_once` Portal 探测改按 `source_ip` 分流——None 直接给 Err「WiFi 未连接,跳过 Portal 探测」不再起小核线程；payload `message`/`campusMessage` WiFi 关闭时显示「WiFi 未连接」（新增共用纯函数 `campus_status_message`，check_campus_status 同步共用）；常驻通知新增状态码 4「监控运行中 · WiFi 未连接」（1=在线/2=未连接/3=非检测时段不动，纯函数 `notify_state` 单测锁定）；检测卡 `detect_campus` detail 新增「WiFi 未连接,跳过探测」分支。
  - 边界：eth0 等非 WiFi 有线接入不受影响（source=Some 不误判 WiFi 关闭）；`auto_login_on_start`、夜切 `run_scheduled_actions`、`ensure_wifi_bound` 快速失败路径均不动。
  - 验证：`pwsh android/build-apk.ps1 -SkipFrontend` 交叉编译通过（安卓 host cargo check 基线即失败，同 2026-10-01 日志清理修复条目口径）；真机「关 WiFi → 检测立即报 WiFi 未连接、不再空转，开 WiFi → 恢复正常探测」待用户复验。

- **状态栏双适配器徽标：同运营商被折叠只显一枚——去除同运营商去重，主/副各出一枚、靠「· 适配器名」区分**（2026-10-01）。用户实测：主/副适配器都在线且同为中国移动时，状态栏只有「中国移动 · 以太网」一枚（2026-09-30 迭代二「相同则单 chip 不重复」的既定设计，用户现要求双适配器都显示）。
  - 根因：非上报链路缺陷——后端（`tauri-app/src-tauri/src/monitor/background_check.rs:297-301`）对副适配器独立解析会话运营商并照常上报 `secondaryOnlineOperator`；折叠发生在前端 `tauri-app/frontend/src/monitor/StatusBar.tsx` 的 `showSecondaryBadge` 条件 `secondaryOnlineOperator !== onlineOperator`。徽标本就带「· 适配器名」后缀（迭代三引入），同运营商双徽标仍可区分，去重条件整体移除（注释同步改写）。
  - 附带行为变化：同运营商场景下副适配器账号信息也进 tooltip（原设计「副适配器会话进 tooltip」此前被去重一并隐藏，现恢复）。
  - 不涉及：NetworkPanel 适配器卡为逐行徽标本无去重；安卓端无此双徽标特性。
  - 验证：`npx tsc --noEmit` 0 错；vitest 103/103 全绿——原「双适配器但副适配器运营商与主相同时不重复渲染」用例翻转为「仍渲染两枚 chip（靠适配器名区分）」断言 2 枚徽标 + 2 处运营商文案 + 副适配器名；puppeteer + mock 三场景 DOM 实证——同运营商「中国电信 · WLAN 中国电信 · 以太网」、异运营商回归「中国电信 · WLAN 中国联通 · 以太网」、单适配器「中国电信 · WLAN」不变。

- **双端日志清理机制：启动路径无任何清理，重装后旧日志残留——set_log_retention_days 改「应用即清理」，安卓启动补接持久化保留天数**（2026-10-01）。用户实测：Windows 重装后之前的日志没有清理。
  - 根因：清理此前只有两条触发路径——①按数量保最新 5 个（`cleanup_old_logs`，仅当日志文件超 5MB 尺寸轮转成功后触发）；②按保留天数删过期文件（`cleanup_old_logs_by_time`，仅 logger worker 空闲且距上次清理满 1 小时的周期检查触发）。**启动路径零清理**：重装后的首个会话旧日志必然残留（短会话甚至长期无人清）。叠加放大因素：桌面日志目录=exe 所在目录\logs，NSIS 覆盖安装保留 logs\（卸载钩子只清 ProgramData 不清安装目录）；安卓侧启动从不应用持久化 `logRetentionDays`，保留天数重启即回退默认 7（与桌面 v2.3.9 已修的「仅保存不生效」历史缺陷同型）。
  - 修复：`infra/logger.rs`（双端共用，安卓经 campus_login_lib path 依赖）`set_log_retention_days` 从纯 atomic store 改为「应用即清理」——新增 `LOG_DIR: OnceLock<PathBuf>` 由 `init_logger` 记录实际日志目录，set 时对其立即执行一次 `cleanup_old_logs_by_time(dir, days)`。该函数全部 4 个调用方（桌面启动接线 startup.rs、配置导入 config_cmd.rs:200/:258、双端日志面板改保留天数）都是「保留天数被设定/变更」时刻，语义处处正确：改天数即时生效，重装/升级后旧日志首次启动即被清。`cleanup_old_logs_by_time` 对 retention=0（永久保留）天然跳过、只删 `mtime < now-N天` 的文件（当天日志恒不误删）、所有 fs 错误静默吞掉（与 worker 周期清理并发重复删除同文件为无害 NotFound）；不做启动按数量清理（count-cap 是轮转尺寸守卫，按天保留契约优先）。
  - 安卓同类缺口同批修复：`monitor_loop.rs run_startup_tasks` 在加载持久化设置后调用 `campus_login_lib::infra::logger::set_log_retention_days(settings.log_retention_days)`——此前安卓只有日志面板命令（system_cmds.rs:53-55）会临时 set，重启即丢；启动接线后与桌面行为对齐（配置生效、启动即清）。
  - 说明（维持现状项）：跨目录重装时旧安装目录的日志成孤儿（日志目录随 exe 走），属安装器语义不在本次范围；NSIS 卸载钩子继续不删用户日志。
  - 验证：桌面 `cargo check` 0 错；`cargo test` 全绿——lib 428 passed / 0 failed / 3 ignored（lib 与 main 二进制各跑一遍同一套）+ 集成 4 passed / 1 ignored。安卓 host `cargo check` 在 build script 阶段即失败（权限文件生成的既有环境局限，基线对照同样失败，Rust 代码从未编译），改走正式交叉编译 `build-apk.ps1 -SkipFrontend` → `tauri android build --target aarch64 --apk` 成功（campus-login + campus-login-android release 编译链接通过，APK 已产出）。

- **桌面端偶发整进程崩溃：hickory Resolver 在 runtime 上下文线程被 drop——退役路径统一移交 retire_resolvers**（2026-09-28）。用户实测崩溃，panic 签名 `tokio-1.52.2/runtime/blocking/shutdown.rs:51:21: Cannot drop a runtime in a context where blocking is not allowed`（崩溃日志 2026-09-28 18:43:48，panic=abort 整进程退出）。
  - 根因：`network/dns.rs` 同步 Resolver 池的退役路径（键数超限整体重建 drain、并发写回溢出丢弃多余实例）把废弃 Resolver 留在当前线程自然 drop——hickory 同步 Resolver 内部持有 current-thread Tokio Runtime，当调用方处于 `tauri::async_runtime` 的 runtime 上下文（worker 线程等 Entered 线程）时，tokio 拒绝在 blocking 不允许的上下文执行 Runtime::drop → panic。触发需低频的超限重建/并发溢出路径，解释偶发性。
  - 实证（新增 `tests/repro_dns_runtime_drop.rs` 四场景）：Resolver 最后一份引用在 runtime worker 线程 drop 必炸（catch_unwind 捕获 "Cannot drop a runtime..."）；spawn_blocking 线程与无 runtime 上下文的普通线程 drop 安全；spawn_blocking 闭包内经 retire 移交后安全。安卓端不涉及（无 hickory 依赖）。
  - 修复：`network/dns.rs` 新增 `retire_resolvers` 作为唯一安全回收点——`Handle::try_current()` 为 Ok（任意 runtime 上下文，含 worker 与 spawn_blocking 线程）时以 `spawn_blocking(move || drop(resolvers))` 移交 blocking 池线程 drop（blocking 上下文允许 blocking drop）；无 runtime 上下文的普通线程直接 drop；runtime 关闭致 spawn 失败时 Resolver 泄漏至进程退出（可接受最坏情况）。超限重建与并发溢出两条退役路径全部改经该入口，约束以「崩溃红线」注释固化（`dns.rs:38-44`）。池参数与轮转语义不变（RESOLVER_POOL_SIZE=4 / RESOLVER_CACHE_MAX_KEYS=16），纯防御性改造。
  - 验证：`cargo test` 全绿——lib 415 passed / 0 failed / 3 ignored（含新增单测 3 个：worker 线程 retire 不炸、超限重建 drain 路径不炸、同键并发压力不炸，依赖真实 runtime 上下文故以 `#[cfg(not(panic = "abort"))]` 守卫）+ 集成测试全过（dns 回归 3 用例 + 既有注销崩溃回归 repro_logout_panic 1 用例）。构建环境附带结论：本机 MSVC 对 /Z7 调试记录写入与 C 盘 Temp 下 lib.exe 临时文件存在偶发干扰（D8050/LNK1104），以 `CARGO_PROFILE_DEV_DEBUG=0` + TMP 重定向规避，详见 CodeWiki learnings。

- **安卓设计审计 P0 修复一：改设置后杀进程丢配置——待存配置挂 webview 生命周期冲刷兜底**（2026-09-28）。安卓 webview 无「关闭窗口」语义，原唯一冲刷路径 `onCloseRequested` 永不触发（审计 KI#7），500ms debounce 内的设置改动随进程死亡丢失。修复：useEventListeners.ts 补挂 `visibilitychange→hidden` 与 `pagehide` 两条生命周期路径冲刷 `flushPendingConfig`（`hasPendingConfig` 短路、in-flight `Promise.race` 2s 兜底），cleanup 对应移除监听；visibilitychange 在桌面 WebView2 最小化同样触发，但冲刷幂等仅在有待存时执行，无副作用。验证：`npx tsc --noEmit` 0 错；真机「改设置→立即杀进程→重开」复验列入设备侧清单。

- **安卓设计审计 P0 修复二：停在后台检测页杀进程重开回落总览——页签恢复白名单补 monitor**（2026-09-28）。`campus-mobile-tab` 恢复白名单漏 `'monitor'` 而 handleTabChange 可写入（质量关闭时第 4 页签即 monitor，审计 KI#14），且质量开启时恢复出的 monitor 会让底栏无高亮页签。修复：白名单补齐；另加归位 effect（`configLoaded` 门控防 DEFAULT_CONFIG 竞态）：质量开启且 tab=monitor 时归位 quality 并同步 storage。验证：tsc 0 错；CDP 设备模拟预置 monitor 重载 → 存储归位 quality、底栏高亮「网络质量」。

- **安卓设计审计 P1 密度批次：总览两空卡合并、自助服务指引去重、配色 4 列、日志工具栏触控 44px**（2026-09-28）。①总览页未配置自助凭据时「在线信息/近期上网记录」两张整卡重复渲染同一空态文案占约两屏——DashboardPanel 新增 mobile-only `mergeSelfServiceEmpty` prop 渲染期合并为一卡（不动 cards state，规避布局迁移缺口：迁移只补 `defaultPosition==='start'` 的 extra 卡，exclude+新增方案老用户看不到新卡）；编辑模式显示真实两卡，桌面不受影响；新空态基元 `components/ui/empty-state.tsx`（tonal 圆底图标+标题+描述，compact/默认两档）。②自助服务页「填写上方学号与自助服务密码后可查询」三卡重复——卡2/卡3 整体包 `{hasCred && …}` 门控，删卡内 `!hasCred` 分支与卡2 单位注（指引集中凭据卡讲一遍；卡3 查询按钮 tooltip 保留），凭据配齐后恢复两卡。③配色方案网格 3→4 列、swatch 32→40px：7 主题从 3+3+1 孤儿行变 4+3，外观卡矮一整行。④LogPanel 工具栏 7 控件 h-7(28px)→h-11(44px)、text-[11px]→text-xs。验证：tsc 0 错；CDP 设备模拟（412×915@2x）四断言全过（总览「近期上网记录」0 次、凭据提示恰 1 次；自助页两查询卡 ABSENT；工具栏控件实测 44px），截图 worktrae\shots\b2。

- **安卓设计审计 P1 体验批次：方向性面板转场 + 总览下拉刷新**（2026-09-28）。①页签切换从单向淡入升级为方向性转场：按底栏索引差定向——前进新面板从右滑入(+18px)、后退从左滑入(−18px)，旧面板反向滑出(−12px)，复用 createPanelAppleVariants 的 Apple 弹簧(320/24/0.7)与 0.04s 快退出；关键实现=variants 逐键 resolver 函数 + custom=navDir（AnimatePresence 将 custom 转发给退出中的旧面板，退出方向才随新导航生效）；方向基准=底栏页签序（第 4 位随质量开关在 quality/monitor 间补位，同位互切回落纵向浮起）。②总览页下拉刷新：手写 usePullToRefresh（零新依赖）——仅 scrollTop=0 接管下拉、阻尼 0.4 跟手、64px 触发；指示器为 header 下独立磨砂胶囊浮层（不位移内容，规避与面板转场的 transform 冲突），过阈值箭头翻转 180°+10ms 触觉反馈，松手转圈调 refreshAdapters，完成自动归位；touchcancel 只归位不触发；motion-reduce 回弹直落；main 加 overscroll-contain。验证：tsc 0 错；CDP 页内 rAF 采样面板 transform——前进 max tx=+18.00、后退 min tx=−18.00 PASS；指示器可见(opacity 1)/转圈/归位(GONE)三态 PASS，截图 worktrae\shots\b3。

- **安卓设计审计 P2 设置批次：设置页 IA 重组——启动与通知合并、自动化归位、网络高级折叠**（2026-09-28）。①「启用通知」并入启动卡为「启动与通知」（自启动/自动登录/系统通知同属"应用何时自己动"心智）；②新建「自动化」卡（Zap 图标）：「可登录时自动登录」+「自动切换运营商」自账号页迁入（全局行为参数不该藏在账号凭据页），账号页「自动化设置」整卡移除（autoLoginCampus 本就与设置页重复），连带清 Switch/Separator/Zap import 与 isAndroid 声明；③质量检测卡改名「网络」，新增「高级设置」默认收起折叠（ChevronDown rotate-180 + aria-expanded），包住延迟计算选项/跳过TTFB检测/跳过内容检测/固定网关四项专家参数，降低新手误触面；④「关于」「诊断」不建卡——关于已有头部 ⓘ 入口（AboutDialogMobile 含更新源选择）、debugMode 已在日志工具栏，再建即重复入口。i18n zh/en 各 +7 键行号镜像。验证：tsc 0 错；CDP 设备模拟 18/18 断言（设置页七项 PRESENT、折叠收起时专家参数 ABSENT→展开后含 10.2.127.254 PRESENT、账号页「自动化设置/自动切换运营商」ABSENT），截图 worktrae\shots\b4。

- **双端 UI 批次：平板设置两列对齐、去「线条包裹」盒、日志头描述独占一行**（2026-09-28）。用户平板实测反馈：设置页左列卡下大片空洞；账号页「查询绑定状态」描边盒与总览监控卡全宽描边刷新按钮「与我们的风格不符」；手机日志头描述被挤成多行。①设置页容器改 `flex flex-col gap-4 md:grid md:grid-cols-2 md:items-start` + 显式 A/B 紧堆列（A=启动与通知+自动化+保活，B=安全+新手指引，「网络」全宽收底），弃右列 `justify-between` 拉伸 hack（空洞根因），对齐桌面端 tauri SettingsPanel 既有做法，手机单列顺序不变；②去线条包裹（双端口径一致）：提示盒→`rounded-xl bg-muted/40` tonal 软底、盒内按钮 outline→secondary、监控卡刷新→`bg-muted/60 active:bg-muted`，android 与 tauri AccountPanel 同改；③日志控件容器 +`max-sm:basis-full`，手机上描述独占一行、控件整行下移。验证：tsc 双端 0 错；CDP 设备模拟 7/7 断言（平板两列顶对齐 A(159,575)|B(490,576)、网络卡 cardW=610 全宽收底、三处 computed border=0px、日志描述 h=20 单行、手机六卡纵序不变），截图 worktrae\shots\b5。

- **桌面设置页不对称修复：删固定网关行、两列重排（左三卡右三卡）、延迟计算选项折叠改 Modal**（2026-09-28）。用户桌面实测反馈：1317/1462px 两档下右列（质量检测卡）远高于左列且参差形态随分辨率变化；「固定网关地址」只读行无交互价值（后端 2026-09-20 起固定匹配）。①质量检测卡「延迟计算选项」默认折叠（折叠按钮 aria-expanded + ChevronDown rotate-180，包住跳过TTFB/跳过内容开关组与 TTFB 解释框，头文本复用 `settings.latencyCalcOptions` 键、zh/en 零改动），质量卡折叠态 181px；后应用户「查看延迟选项改为弹出 modal 进行设置」再演进为 Modal——折叠按钮与内联展开块删除，改为 Modal 触发行（样式沿用原行、ChevronRight 静态、aria-haspopup="dialog"），点击打开 sm:max-w-md Dialog：标题 settings.latencyCalcOptions、描述 settings.qualityDetectionDesc（该键在 settings 段 :531；首版误用 dashboard 段的 networkQualityDesc 渲染出裸键名，截图目检兜住），内含跳过TTFB/跳过内容两 SettingRow+Switch（原绑定不动）与 TTFB/内容传输/网络延迟三行解释框；质量卡高度与对齐机制不变；②删「固定网关地址」行与前后 Separator（后端 `fixedGateway` 字段与 10.2.127.254 固定匹配保留，zh/en 键留置）；③两列重排两步迭代（初版「数据管理↔质量检测互换列」收到 313px，用户复看截图仍判「空了一大片」后改左列单卡右列三卡收到 81px，用户仍判「没水平对齐」后定稿）：左列=启动设置单卡，右列=通知与安全+数据管理+质量检测（折叠）三卡，网格去 `items-start` 改默认 stretch，两列末卡（启动设置/质量检测）`grow` + `AnimatedCard h-full` 吃掉各自列残差——两列底缘**像素级相等**（1414/1414），残差藏进启动设置卡内底部而非页面背景；④用户再提「启动设置拆成三个卡片垂直排列用卡片之间的空隙补上距离」落地终版：拆为 启动设置(187px,Rocket)/登录自动化(234px,LogIn 图标)/窗口与界面(311px,LayoutList) 三卡垂直排列，段内 Separator 与组小标题随拆卡消失（卡片标题即分组，zh/en 零改动），左列 justify-between 把环境字体渲染差异产生的残差摊进卡间空隙（本机 11px 残差 → 空隙 21/22px、右列恒 16px；模拟右列 +30px 残差时空隙自适应 36/36 且「窗口与界面」底与「质量检测」底焊死），右列末卡 grow 退化为反向兜底。验证：tsc 0 错；CDP 实测 24/24+12/12+18/18+13/16+18/18（1280/1462/1920 三档 × 打开设置/每列三卡/底缘等高 delta≤1/触发器行存在且折叠块移除/网关 ABSENT/Modal 开合+开关与解释框齐全+描述非裸键，前项 3 个 FAIL 为脚本断言算术误差），列底缘差 413→313→81→0px 且三档恒定（设置区定宽容器不随窗宽回流，「随分辨率变化」的表象消除），截图 worktrae\shots\d6。决策文档见 CodeWiki `decisions/desktop-settings-asymmetry-fold`。

- **夜间出站切换与巡检自动启用打架：守护窗口闸门修复（七期）**（2026-09-28）。用户实测（09-27 夜日志）：切换前手动禁用校园网卡（以太网/以太网 2，无 IP）后，23:04:57 与 23:12:33（重启 replay 后）巡检仍将其自动启用，与切换意图打架。根因：六期闸门只按 `outboundDisabledAdapters` 名单豁免，而手动禁用的卡无 IP，`select_campus_to_disable` 无 IP 跳过不入名单（名单为空）→ 闸门失效。修复：新增纯函数 `is_night_outbound_guard_window`（`config/outbound_switch.rs`，窗口=[当日切换时刻, 次日 06:30 恢复窗开)，凌晨分支（now<06:30）覆盖前一晚 23:00/23:30 之后全部时段，跨日天然成立），`adapter_watch.rs` 自动启用在窗口内一律不出目标（log_debug 记跳过张数），还原仍由恢复窗 scheduled.rs 负责；白天照旧保留「意外禁用→恢复登录」通道。设计取舍：不把用户手动禁用的卡记进名单——名单驱动 06:30 还原逐卡 enable，记进去会让还原误启用用户手动禁用的卡，纯时间窗闸门即可全覆盖（重启 replay、启动中途、运行中禁用三场景都落窗口内）。验证：cargo test lib 409/409 全绿（基线 408+守护窗口 1 新用例）、clippy 新增代码零告警。
- **k2.8 深度审计落地：四处状态机/守护修复（八期）**（2026-09-28）。kimi-k2.8-preview 对 src-tauri 后端只读深度审计返回 7 条（每条带 文件:行号），逐条核实后实施 4 条：①**导入配置保留本机切换态**——原「导入即清四快照」在切换中导入会把活跃快照顶掉，系统残留 metric=1/禁用卡/兜底路由却再无还原路径，原始跃点永久丢失；改为导入文件四字段一律丢弃、保留本机当前值（外部状态不迁入、本机状态不丢失，导入任何时刻可用）。②**兜底路由删除失败不再静默收尾**——原设计失败仅 warn 照常清三快照，孤儿 0.0.0.0/0 metric=2 路由持续改道流量而用户看到「已还原」；改为失败计入退避+阻止收尾清快照，下一拍整体幂等重试直至删除（推翻六期「失败不阻塞还原」决策：静默的假成功比延迟重试更危险）。③**还原失败告警周期化**——原仅第 3 次恰好提醒一次、此后无限静默；改为首次达阈值后每 20 次失败（约 100 分钟）重复提醒。④**守护窗叠加切换态**——纯时间窗在整夜休眠错过切换的凌晨会误阻塞自动启用；改为时间窗 ∧ 三快照非空，真实切换态全覆盖、无快照不误伤。其余 3 条处置：helper 请求目录 ACL 加固需真机验证目录属主矩阵，记录方案后延（P2-1）；夜间手动置空运营商被次晨还原覆盖记为已知限制（P2-2）；UI 手动启用不设切换态互斥**否决**——与「不与用户意图打架」原则冲突（P3-3）。验证：cargo test lib 410/410（+告警周期 1 新用例）、clippy 新增代码零告警。

- **CMSTPLUA 静默提权从未真正通过：BIND_OPTS 传小 + vtable slot 错位双 bug 修复**（2026-09-21）。用户实测反馈「不会自动启用已禁用的适配器」，排查牵出应用提权通道的两个叠加 bug。
  - 取证：运行日志证实 15s 巡检**正常触发**（「检测到手选适配器 以太网 2 被禁用，尝试自动启用」），失败发生在提权步骤 `COM静默提权启用失败: HRESULT=0x80080017`，与执行顺序无关。
  - 排查弯路（已修正的误诊）：一度判定「CMSTPLUA 被系统封堵（0x80080017）」——该结论源于验证工具自身参数错误（.NET `Marshal.BindToMoniker` 不传 BIND_OPTS3、对照脚本传 36 而非 x64 正确值 48）。经精确矩阵本机复测：`BIND_OPTS3{cbStruct=48, dwClassContext=CLSCTX_LOCAL_SERVER}` → **S_OK**，`cbStruct=16 → 0x80080017`——CMSTPLUA 未被封堵，CLSID 注册与 COMAutoApprovalList 均完好。
  - 根因一：`platform/elevation.rs` 的 `CoGetObject` 传的是 `BIND_OPTS`（16 字节）——elevation moniker 要求 `BIND_OPTS3`（x64 48 字节）且 `dwClassContext=CLSCTX_LOCAL_SERVER`，cbStruct 过小被 appinfo 按「提升激活配置无效」（CO_E_ELEVATION_DISABLED）拒绝。静默提权**从未成功过**，一直被手动路径的 runas 降级掩盖；自动启用在 9-14 禁用分类修复（b9bd5ab）后首次走到提权，才暴露此问题。
  - 根因二：修好根因一后冒烟测试暴露 `ICMLuaUtil` vtable **slot 错位**——真实布局（UACMe elvint.h 权威定义）ShellExec 在 **slot 9**（slot 3~8 为 SetRasCredentials/SetRasEntryProperties/DeleteRasEntry/LaunchInfSection/LaunchInfSectionEx/CreateLayerDirectory 六个真实方法），原实现错误地把它排在 slot 4（实为 SetRasEntryProperties，签名不匹配报 RPC_X_BAD_STUB_DATA 0x800706F4），并配了错误的「SetCallState(3)」注释。
  - 修复：`CoGetObject` 改传 `BIND_OPTS3{cbStruct=size_of::<BIND_OPTS3>, dwClassContext=CLSCTX_LOCAL_SERVER}`；vtable 按 slot 9 对齐（slot 3~8 六个 usize 占位）并以 `vtbl_shell_exec_slot_is_9` 单测锁定。新增 `#[ignore]` 真实提权冒烟测试 `shell_exec_elevated_smoke`（cmd /c exit 无副作用）。
  - 修复二（自动启用降级链）：自动启用按失败计数决定提权方式——首试静默（零打扰），失败计数 ≥1 后降级 runas 弹 UAC，频率由退避阶梯限制（60s→120s→300s 封顶）；CMSTPLUA 修复后正常环境不会走到弹窗。
  - 验证：冒烟测试通过（静默提权真实走通）+ `cargo test` 全绿（362×2+1）。长期方案（计划任务提权代理/服务 helper，官方推荐方向）另行立项。

- **修复安卓端「启用系统通知」不会向用户请求通知权限**（2026-09-21）。此前通知权限请求只挂在「启动后台检查」路径（Android 13+ 弹框），设置页「启用通知」开关打开时只写配置（`enableNotification`）不请求权限——13+ 用户只开通知开关、未启后台检查时系统通知静默不显示；13 以下/国产 ROM 通知被系统关闭时无任何引导，`getNotificationEnabled` 恒为 false 也无出口。本次随「行为调整」条目的三入口链路一并修复。

- **安卓端底部导航悬浮化重设计：单行磨砂玻璃胶囊栏 + 滑动药丸，快捷按钮视觉等重**（2026-09-27/28）。原流内 64px 底栏改为悬浮玻璃胶囊条（内容从栏下穿过被雾化），登录/注销快捷按钮重做为直接悬浮的双胶囊。中途曾按用户三点需求做过「双层悬浮栏」方案（设置/测速/日志提为上层缝位 chip、聚合页取消），用户验收后裁定**回归单行设计、取消双层**，以下为最终定稿状态。
  - **BottomNav.tsx 全量重写**：单行 5 tab（总览/账号/自助服务/第 4 位动态 quality↔monitor/更多），磨砂玻璃胶囊（surface 72% + blur(24px) saturate(160%)，无可见描边，主色微量晕染柔影，深色自动适配）；激活项为 tonal 滑动药丸（`layoutId="nav-active-pill"` spring，主色 12%/深色 16%，非实心非描边）；图标 22dp 激活加粗描边（2.2/1.8），标签 11px 常驻；触控目标 ≥56px，动画仅 transform/opacity + useReducedMotion 降级；导出 `MobileTab` 类型与 `BottomNav({tab,onChange})` 签名不变，第 4 位动态逻辑（enableNetworkQuality !== false → quality else monitor）原样保留。
  - **MobileQuickActions.tsx**：紧凑双胶囊悬浮于导航栏上方右缘（bottom safe+92px，右缘与栏体右缘对齐）——注销磨砂 tonal 在左、登录实心主色在最右（拇指热区），合计约 200px（原双列铺满约 390px）；除填充外完全同尺寸同圆角同字号同阴影，视觉等重；注销底色用 index.css `.btn-tonal-glass` 主题档（玻璃基底叠主色调：浅 12%/暗 26% 提亮——防暗色下与卡片糊底，并修掉 bg-primary/10 无模糊的内容透印）；登录逻辑（bindToWifi→doLogin、busy 态、i18n auth.*）原样保留。
  - **App.tsx 几何联动**：main paddingBottom dashboard 148px / 其它 88px（原流内底栏时为 96px 常量）；A/B/C 方案切换脚手架（NavLabSwitcher + navLab i18n 键）已清除。
  - 验证：`npx tsc --noEmit`（android/frontend）0 错误；CDP 设备模拟（412×915@2x，浅/深主题）截图 + 真实点击切页验证通过；真机 APK 复验待用户。

- **修复：安卓夜间切换验证 `portal_origin` 空值无回退，双端语义漂移**（2026-10-03）。耦合审计发现同一夜切验证链两端实现漂移：桌面 `monitor/scheduled.rs` 的 `portal_origin()` 在 portal_url 为空时回退 `default_portal_url()`，安卓 `android/src-tauri/src/monitor_loop.rs` 同名函数无回退（portal_url 配置为空时 origin 为空串，chkstatus 校验请求直接落空）。安卓补齐同款回退，双端对齐（`NIGHT_VERIFY_*` 常量双端本就一致：15/5/3/15 秒）。验证：`cargo check --target aarch64-linux-android --all-targets` 通过（NDK 27.0.12077973，llvm-ar 需用 `.exe` 后缀）。

- **修复：桌面网络质量 `busy` 增量分支语义漂移（胶囊卡「检测中」）**（2026-10-03）。双端 `mergeNetworkQuality`（`tauri-app` / `android` 前端 `lib/latency.ts`）漂移：安卓 2026-09-09 已按真机实证修复——慢网络下后端增量推送（busy）终态迟迟不到，若采纳 busy 等级胶囊会永远显示「检测中」；桌面仍是旧语义（`disabled`/`busy` 一律保留旧值），且旧值为空时直接落 busy 等级。桌面对齐安卓三分支：无旧值→`unknown` 兜底；旧值 busy→刷新 details 保持 busy；否则增量合并 details 但不采纳等级；`disabled` 保留旧值逻辑不变。验证：桌面 vitest 17 文件 103 用例全绿；`tsc --noEmit` 0 错误。

### 验证

- 桌面卡片合并+拖拽排序（2026-09-23）：`tauri-app/frontend` `npx tsc --noEmit` 0 错误；vitest 96/96 全绿（含 outboundOrder 纯函数 7 新用例）；`npm run build` 通过；Playwright（Chromium，临时 dev mock 环境）真浏览器实测 4 场景全过。
- 桌面排序交互四期（2026-09-23）：`npx tsc --noEmit` 0 错误；vitest 96/96 全绿；Playwright（Chromium，临时 dev mock 环境，用后完整还原）真浏览器实测 6 场景全过（文案/无把手/落盘语义）。
- 桌面 UI 精修（2026-09-27）：`tauri-app/frontend` `npx tsc --noEmit --incremental` 0 错误；vitest 96/96 全绿；`npx vite build` 通过。真浏览器走查待用户（布局类改动，建议 `npx vite dev` 过一遍设置/监测/首页与浅色主题）。
- 桌面 UI 精修二轮（2026-09-27）：`npx tsc --noEmit --incremental` 0 错误；vitest 98/98 全绿（96 基线 + StatusBar 胶囊锚点 2 新用例；修 DashboardPanel.selfCards 的 useAuthStore mock 补 `status` 字段、AccountPanel.helloGate 先点披露入口）；`npx vite build` 通过（index-DivhEuIB.js 475KB）。六项改动 + 胶囊锚点无头截图逐面板复验（摘要带/两处空态/披露行/两列均衡/把手/蓝运行 badge），实测环境已还原（删 mock + 复原 index.html，工作区仅剩目标文件）。
- 桌面 UI 精修三轮·用户反馈（2026-09-27）：①Account 绑定卡撤渐进披露（用户裁定「展开纯属多此一举」）——删披露入口/AnimatePresence 包裹/bindFormOpen state 与 framer-motion import，表单恢复常开（AccountPanel.tsx -28 行），测试与 zh/en 的 bindFormToggle 键同步移除；②监测页「校园网络名称」由单值框改三枚 chips（i-wxxy/iwxxy-2/iwxxy-3，与后端 campus_check.rs 额外 SSID 语义镜像：extras 仅默认名 i-wxxy 时显示），tip 改「命中以下任一名称即视为已连接校园网」（zh/en 同步）；③设置页两列对齐修正（kimi-k2.8 前端分析设计师诊断）：孤立「新手指引」卡撤出右列、改为页面级紧凑横幅收口（CardContent 横向布局，按钮右置），stagger 乱序修正（数据管理 4→3、质量检测 3→4，恢复行序 Z 字），两列底缘差由约 110px 收敛至约 25px。验证：tsc 0 错、vitest 16 文件 98/98 全绿、无头截图逐面板核验（account 常开表单/monitor 三 chips/settings 横幅）。- StatusBar 胶囊锚点真浏览器点击实测（2026-09-27 补验，Playwright 不可用 → Node 22+ 原生 WebSocket 直连 Edge 无头 CDP，零新增依赖）：临时重建 mock-tauri.js + 注入 index.html，CDP Runtime.evaluate 真实 click + DOM 断言 + before/after 截图——network 面板出发 → 点状态栏质量胶囊 → 总览摘要带（校园网认证/适配器在线/网络质量）出现，全链路通过；验证后再次还原实测环境（删 mock、复原 index.html，git 工作区干净）。
- 设置页两列底缘对齐·微调（2026-09-28）：删网络质量卡 TTFB 信息框内「推荐：日常使用建议关闭两项…」段（SettingsPanel.tsx 信息框第 4 个 <p>，zh/en 删 settings.recommendation(+Detail) 两键，确认仅此一处使用）——信息框仍保留 TTFB/内容传输/网络延迟三段说明，语义完整。删除节省约 37px（两行文字+段间距），与两列底缘差（三轮后残留约 25-33px）基本抵消，数据管理与网络质量两卡底缘取齐；不同窗口宽度下因折行数不同会有数像素浮动。验证：tsc 0 错、vitest 16 文件 98/98；提交 40b11e2 已推送。
- 夜间出站切换（2026-09-22 收尾实测）：桌面 `cargo test`（tauri-app/src-tauri）lib 396 passed / 0 failed / 3 ignored（真机冒烟）+ 集成 1 passed，全绿；较功能前新增约 24 个用例（时间表共用、快照幂等、目标卡选择、互斥顺序、还原终态出口、退避阶梯、helper 编解码 round-trip 等）。`cargo clippy --all-targets` 零新增告警。
- WRITE_SECURE_SETTINGS 增强（2026-09-22）：`android/frontend` `tsc --noEmit` 全绿；一键出包 `pwsh android/build-apk.ps1` 成功（Kotlin 编译含 4 个新 @Command + manifest 权限声明通过，插件 ACL 权限经 build.rs COMMANDS 数组再生成）；真机验证待用户（见下）。
- 前端双端：`npx tsc --noEmit --incremental`（tauri-app/frontend 与 android/frontend）均 0 错误。
- 安卓 Rust：`cargo check --target aarch64-linux-android --all-targets` 全绿（0 error / 0 warning）。
- helper `read_interface_metrics_smoke` 真机冒烟通过（本机实测读回 IPv4/IPv6 两族跃点）。
- 测试基线文档刷新：CodeWiki `decisions/verification-baseline` 更新为 397 独立用例口径（替换 2026-09-14 的 326 与更早来历不明的"493"）。

- `android/frontend`：`npx tsc --noEmit --incremental` 全绿（含新增 helper/弹窗/开关接线）。
- `android/src-tauri`：`cargo check --target aarch64-linux-android --all-targets` 全绿（含插件 lib.rs 与 battery_cmds 改动）。
- 一键出包 `pwsh android/build-apk.ps1` 成功产出 `Wxxy-CampusLogin_2.3.9.apk`（45,033,749 字节；Kotlin 编译含新 `openNotificationSettings` 命令通过，仅既有 deprecation 警告）。
- 真机验证待用户：①首次启动（清数据重装）先见新手向导、完成后见通知权限引导弹窗，点「去开启」→ Android 13+ 弹系统权限框（本机 HyperOS 为 Android 14）；②设置页「启用通知」开关关闭再打开 → 重新触发请求/跳设置；③在系统设置里关掉应用通知后回到 App 打开通知开关 → 跳系统通知设置页；④「暂不」后重启 App 不再自动弹（手动入口仍在设置开关）。
- WRITE_SECURE_SETTINGS 增强真机验证待用户（追加为夜间出站切换第四项实测）：①adb 执行 `pm grant com.campuslogin.client android.permission.WRITE_SECURE_SETTINGS` 后重启应用，网络页引导块变绿框「已授权自动管理」；②到点切换时观察 logcat「network_avoid_bad_wifi 已自动确保」（原值快照记录）；③次日恢复窗口重登后系统键还原为原值（`adb shell settings get global network_avoid_bad_wifi`）；④点「还原系统设置」按钮后两键写回原值、绿框保持（授权不撤销）。

- **外接 USB 网络适配器从禁用无法正确启用：启用前按 PnP 设备级禁用分流**（2026-09-21）。用户实测：USB 网卡被禁用后点「启用」，网卡表现为反复重新连接却始终未启用；内置网卡正常。
  - 根因：Windows 的「网卡被禁用」是两层独立状态——NDIS admin 层（`netsh interface set interface ... enable` 只翻这层）与 PnP 设备层（设备管理器式禁用，`CM_PROB_DISABLED` problem 22，即 `CONFIGFLAG_DISABLED`）。经系统设置/设备管理器禁用的外接 USB 网卡处于 PnP problem 22 态，netsh enable 对它只触发 NDIS miniport 重连（USB 网卡表现为重连）而清不掉 PnP 禁用，故永远启用不成功。依据：微软文档两层状态在 `MSFT_NetAdapter.State`（PnP）与 `InterfaceAdminStatus`（RFC 2863）并列独立；生产实现（Windscribe Desktop-App、devcon）均以 SetupAPI `DICS_ENABLE` 做设备级启用。
  - 修复：新增 `network/discovery/devnode.rs`——启用前先定位设备实例（适配器 guid → 注册表 `Control\Network\{4D36E972-...}\<guid>\Connection\PnPInstanceId`，接口行缺失时回退按 `Connection\Name` 遍历匹配），`CM_Get_DevNode_Status` 判定 problem 22：命中走 `pnputil /enable-device`（管理员直跑 / 非管理员 COM 静默提权，与 netsh 路径权限框架一致），并以 `CM_Get_DevNode_Status` 轮询复核禁用态真正解除（不信命令返回值，实测 PnP 状态落盘有百毫秒级时差）；未命中维持 netsh 原路径（NDIS admin 层操作，对已启动设备零重枚举）。`enable_adapter` 头部插入分流，手动按钮与 15s 巡检自动启用共用同一路径，一处修复两处生效。
  - 验证：`cargo test` 全绿（727 用例，含 devnode 4 个新用例）；注册表定位链路本机实测正确（USB 网卡「以太网 2」的 PnPInstanceId 正确解析为 `USB\VID_0BDA&PID_8156\4013000001`）；pnputil 本机支持 `/enable-device` 语法。端到端（真实禁用 USB 网卡后启用）待用户实测。
  - 说明：设备级启用对 USB 设备物理上伴随一次重枚举（等价拔插，Windows PnP 语义，不可回避）；本修复保证它「只发生一次且真正生效」，且对仅 admin-down 的场景不引入任何设备级操作。

- **新功能：状态栏与仪表盘显示当前在线运营商（双端）**（2026-09-30）。需求：此前 UI 只有「已在线」状态，看不到当前登录的是哪家线路；配置口径（`config.operator`）在夜切/手改配置后与真实在线会话可能不一致，故采用**在线会话口径**——从 Portal 页面解析 uid（完整在线账号=账号+运营商后缀）推导真实线路。
  - 后端（桌面/安卓共用 Rust 核心，一次改动双端生效）：`auth/portal.rs` 页面探测新增 uid 提取（页面内嵌 `uid='…'`，跳过空占位；新函数 `extract_uid_from_html`、`operator_suffix_from_uid`：`2023xxxx@telecom`→`@telecom`、无 `@`→`''`（无锡学院默认线路）、未知后缀→None），`PortalStatus.uid` 仅在线时保留，离线/请求失败/无法判断时为 None；`monitor/portal_check.rs` 透传；`monitor/background_check.rs` 仅主适配器在线时由 uid 推导 `online_operator`；`monitor/background_emit.rs` bgStatus 事件新增 `onlineOperator` 字段（`''`/`@telecom`/`@unicom`/`@cmcc`/null，注销保护期强制 null）。解析挂在既有探测响应上，**零新增网络请求**，结果只进展示链、不进断线重连判定输入。
  - 桌面前端：StatusBar 状态胶囊右侧并列运营商徽标（仅 online 且后缀可映射时渲染；中性灰样式不占用状态色语义；不拼入 200px 截断文本）；tooltip 显示完整在线账号，夜切临时态（前端按 config 检测 `operator==='' && nightOperatorRestore!==''`）时 Moon 图标+恢复目标提示，徽标始终显示真实在线运营商；仪表盘账号管理卡「当前」行同步显示。映射复用 `settings.isp.*` 文案，新增 i18n 键 `statusbar.onlineOperator.{badge,account,nightShift}`（zh/en）。
  - 安卓前端（复刻树等价）：StatusBar 同款徽标+tooltip+i18n；安卓无仪表盘账号管理卡，次落位不适用；Kotlin 层零改动。
  - 迭代二（体验反馈，2026-09-30）：品牌 logo WebP 化（`public/isp/{telecom,unicom,mobile,school}.webp` 共约 7KB，IspMark 组件双端复用）替换 Moon 图标；双适配器不同运营商并排双 chip（bgStatus 新增 `secondaryOnlineOperator`，同运营商去重进 tooltip）；手机壳顶栏不渲染 StatusBar，等价落位在总览「网络状态」卡（useFormFactor 门控防平板双处重复）；迭代三：chip 追加「· 适配器名」标注登录网卡，网络适配器面板主/副行内嵌运营商徽标，安卓顶栏状态点旁加 logo 标。
  - 验证：`cargo test` 428 通过 0 失败（含 uid 提取/后缀映射新单测与离线丢弃 uid 判例）；桌面 vitest 锚点/徽标渲染用例全绿（logo src/适配器名断言）；双端 `tsc --noEmit --incremental` 全绿；Edge+mock 注入浏览器实测桌面 4 场景+适配器面板+安卓手机 3 场景+平板 1 场景，截图目检通过。
  - 迭代四（用户三诉求，2026-09-30）：①安卓顶栏 logo 标升级为桌面同款 chip（logo+运营商名）；②四枚 ISP 图标资产重建——放大审计发现 telecom 碎裂（早期提取只留 1/32 条 path）、mobile 缺左列底行、unicom 底环被裁、校徽量化洗白缝，全部从原始 Wikipedia SVG/官网 PNG 经「Edge 无头透明底渲染 → Pillow 品牌色蒙版列聚类（联通用最大红色连通域 BFS，防把文字红点算入 bbox）→ 96px 高 WebP」重建（合计约 18.6KB，双端 public/isp/ 同步）；③底部导航合并为酷安式单条全圆角胶囊栏——页签群收窄居左（flex-1 min-w-0+truncate+图标 20/文字 10px），缩小版注销/登录（h-9 rounded-full，登录保留绑 WLAN 流程）右侧同排水平对齐，独立悬浮的 MobileQuickActions 删除；顶栏质量胶囊防挤压三连（wrap/pill max-w-full+overflow-hidden、标签 truncate+min-w-[60px]、图标钮 p-2→p-1.5），消除窄屏（360px 实测）胶囊与图标重叠。验证：安卓 tsc 0 错；Edge emuw 模拟 360/390 双宽截图目检（顶栏 chip/单条栏/四图标完整）；桌面截图回归新图标正常。
  - 迭代五（底栏胶囊栏细调，2026-09-30）：①网络质量胶囊去掉「网络质量」描述只显示延迟数值+单位（用户指定；pending 仅加载圈），顺带根除窄屏长标签叠字/竖排换行（真机截图实测：长标签在挤压容器内被逐字断行）；②底栏激活机制反转——非激活页签只保留图标（w-8），激活项图标+文字标签，激活背景块从 rounded-2xl 改为与栏身同形的 rounded-full tonal 药丸（layoutId 滑动不变）；③登录/注销再缩一号（h-8/px-2/text-[10px]）；④顶栏图标钮 p-1.5→p-1 收紧间距。验证：安卓 tsc 0 错；Edge emuw 360/390 双宽目检（360 下「总览」标签完整、顶栏单行无重叠）。
  - 迭代六（桌面/平板 dock 同款机制，2026-09-30）：用户要求「Windows版和安卓平板悬浮栏也使用这样的文字显示设计」——双端 DockNav 同构改造：DockItem 改 flex-row rounded-full，激活项=tonal 药丸（inset-0 rounded-full bg-primary/10）+图标+内联文字标签（text-[11px] font-medium whitespace-nowrap，仅激活渲染），非激活纯图标；删除底部 3px 下划线指示条及全部接线（itemRefs/mounted 定时器/handleItemLayout/useLayoutEffect），磁吸 GSAP quickTo 缩放保留。验证：双端 tsc --noEmit --incremental 0 错；安卓平板 900×700 与桌面 1280×840 无头截图目检通过（桌面 dock 首两张缺席为 headless virtual-time 下 gsap 入场时间线推进过慢的截图工具假象，30s 预算 DOM dump 证实时间线正常推进 opacity 0→0.4975→1，非代码回归）。
  - 迭代七（激活药丸方向滑动，2026-10-01）：用户要求「切换时背景块向右移动就跟向右移动，向左同理」。①桌面 DockNav 与安卓平板 DockNav 的激活药丸从原地淡入缩放（`initial opacity/scale`，无 layoutId）改为 `layoutId="dock-active-pill"` 共享布局元素 + 弹簧（stiffness 420/damping 34，与安卓手机底栏同款）——药丸成为跨项共享的唯一布局元素，切换页签时从旧项位置滑向新项，方向天然跟随切换方向；顺带删除 DockItem 内仅为药丸 easing 服务的 `useAnimationProfile` 引用（DockNav 磁吸与 AdapterMenu 处 profile 不动）。②安卓手机 BottomNav `motion.span`→`m.span`（LazyMotion strict 下统一 `m.*` 约定）并删除 `useReducedMotion` 手动双分支兜底——系统减动态时全局 `MotionConfig reducedMotion="user"` 已自动关闭位移/布局动画，静态 span 兜底冗余。验证：双端 `npx tsc --noEmit --incremental` 0 错；桌面 vitest 17 文件 103/103 全绿；vite 预览 + Edge 无头 1280×840 冒烟：dock 挂载无崩溃、药丸随 activePanel 周期切换（dashboard→network→settings→account→quality）始终落在正确激活项（9000ms 帧落 network、4650ms 帧落 account）；滑动方向性为 framer-motion layoutId 既有机制（与已验收安卓底栏同款），真机目检待用户。

### 架构改进

- **计划任务提权代理（方案 1）：提权通道升级为「SYSTEM 主体哑任务 + 请求文件协议」，全程零 UAC**（2026-09-21）。用户拍板采用 GitHub 调研的方案 1，并经 glm5-3 架构评审（本机 5 组权限实测）修订后实施。
  - 架构：以 SYSTEM 主体 + RunLevel=Highest 注册哑任务 `CampusLoginPowerOps`（action 固定为自身 exe + `--helper-task`），主进程经「写请求文件（`%ProgramData%\CampusLogin\requests\`，唯一名+原子落盘）→ `schtasks /run`（普通权限）→ 轮询结果文件」完成提权操作。worker 复用现有 helper op 分发，新增 `enable_adapter` / `enable_device` / `register_task` / `selfcheck` 四个 op。
  - 评审修订（P0 级，实测证据）：
    - 注册方式弃用 `schtasks /create`（默认 DACL 下普通用户 `/run` 被拒，实测 E2/E3），改为提权副本内 COM `ITaskFolder::RegisterTaskDefinition` + 显式 SDDL `D:P(A;;GRGX;;;BU)(A;;FA;;;BA)(A;;FA;;;SY)`——Users 仅可触发（GRGX），`/change`、`/delete` 均被拒（实测 E4/E5）；零触发器（免过期语义）；电源条件显式关闭；`ExecutionTimeLimit=PT5M`。
    - 结果路径收口（P0-2）：worker/提权副本以高权限运行，结果只写固定目录 `results\` 下纯文件名（拒绝路径分隔符/`..`/点开头），杜绝「SYSTEM 任意路径写」原语——该收口同时回移到既有 `--helper` 通道（旧实现 `--result` 接受任意路径，属既有缺陷一并修复）。
  - 降级链升级为四级：管理员直跑 → 计划任务代理（无 UAC）→ CMSTPLUA 静默 → runas 弹 UAC；代理注册失败带 10 分钟退避（防 UAC 弹窗风暴），每次启动校验任务 action 指向当前 exe（路径漂移自动重注册）。DNS/MAC 现有 helper 场景零改动自动受益。
  - 安全声明：未签名 exe + 用户可写目录 + 持久 SYSTEM 任务——本机恶意软件改写 exe 后可无提示获得 SYSTEM 执行（提权场景从「主进程发起时」扩大为「任意时刻可触发」）；收口后白名单操作滥用增益有限，长期缓解为安装到管理员可写目录（Program Files）。
  - 验证：单测 369×2+1 全绿（含结果路径收口、请求 JSON 协议、op 构造 7 个新用例）；真实链路集成冒烟通过（CMSTPLUA 静默提权注册 → 自检全链路 → SYSTEM worker 回写）；端到端实测「写请求 → 无 UAC 触发 → worker 执行 netsh 启用适配器 → 结果回写」全通；安卓一键出包成功（helper 模块跨平台编译无恙）。
  - 卸载清理：NSIS `installerHooks`（`windows/hooks.nsh`）卸载时删任务并清 `%ProgramData%\CampusLogin`。

- **安卓打包体积修复：补缺失的 `[profile.release]`，APK 43.1 MB → 29.6 MB（-31%）**（2026-10-01）。起因是用户问「安卓安装包为何高达 43MB」，解包发现 87.5% 是单个 `lib/arm64-v8a/libcampus_login_android_lib.so`（37.8 MB，Stored 不压缩方式全额计入 APK）。根因：`android/src-tauri` 是安卓构建的独立 workspace 根，其 Cargo.toml 此前完全没有 `[profile.release]` 段——桌面端 `tauri-app/src-tauri/Cargo.toml` 的 profile（strip / thin LTO / panic=abort）不随 path 依赖传播，安卓构建一直走 Cargo 默认 release 档（无 LTO、符号表不 strip、panic unwind）。修复：`android/src-tauri/Cargo.toml` 末尾补 `[profile.release]`（`lto = "fat"`、`codegen-units = 1`、`strip = true`、`panic = "abort"`，panic 策略与桌面一致），零行为变更、纯构建配置。验证：`pwsh android/build-apk.ps1 -SkipFrontend` 一键出包成功，.so 39,593,968 → 25,404,112 字节（-36%）、APK 45,231,305 → 31,041,449 字节（43.1 → 29.6 MB，-31%）；剩余构成：内嵌前端 dist 17.2 MB（人脸识别模型 9.2 + 看板娘图 4.8 + JS 资产 2.7）+ Rust 代码约 7 MB。后续可选未做：模型按需下载、看板娘图重编码属行为/画质取舍需另行决策；panic=abort 与符号 strip 影响运行时，装机冒烟（登录、检查更新）待真机验证。

- **解耦：监控层配置落盘+广播下沉 `config::persist`，斩断 monitor→commands 反向依赖链**（2026-10-03）。耦合审计发现 monitor 层 5 处（`monitor/background_task.rs`、`monitor/scheduled.rs` 定时任务/巡检/出站恢复共 4 处）反向调用上层 `commands::config_cmd::save_config_to_disk_encrypted`，形成 monitor→commands→app::tray 三层逆向依赖（crate 声明的依赖方向为 commands/app/monitor → config，禁止反向）。下沉修复：
  - 新增 `config/persist.rs::save_config_and_broadcast(app_handle, config)`：取数据目录 + 加密落盘 + 掩码后 `{config: …}` 事件广播，不刷托盘——托盘刷新仅依赖 user/active_account 字段，而监控层调用点只写出站快照/运营商/巡检间隔，跳过刷新行为等价；
  - `commands::config_cmd::save_config_to_disk_encrypted` 包装瘦身为「下沉函数 + 托盘刷新」，用户设置页保存路径行为不变；monitor 层 5 处调用点改指下沉函数，monitor 模块不再 import commands。
  - 验证：cargo test 全绿（397 独立用例）；改动代码 clippy 0 警告（基线整体因 clippy 1.98.0 新 lint 存在 14 处既有告警，全部位于本次未触碰文件，另行记录）。

- **清理：死状态字段删除与 1440 禁用哨兵常量化**（2026-10-03）。`UpdateStats.last_update_check_epoch_ms` 全仓库仅 2 处 store、0 处 load（真实数据源为 `update/updater.rs` 的 `LAST_CHECK_TIME_MS` 静态量），删除字段及两处写入；定时分钟数 `>= 1440` 的禁用哨兵在桌面 `config/schedule.rs` 与安卓 `monitor_loop.rs` 双端硬编码，常量化为 `config/schedule.rs::SCHEDULED_DISABLED_MINUTES` 双端同源引用。

- **解耦：前端 store 交互边界清理——适配器写回收口 actions、删除 `useConfigStore.api` IPC 字段（双端同步）**（2026-10-03）。
  - AdapterStore 补齐 `setAdapterDetails`/`setDisabledAdapters` actions：`refreshAdapterData` 及桌面 `useEventListeners`/`useInitialDataLoad` 共 8 处裸 `useAdapterStore.setState` 全部改走 actions，数据写回路径收口到 store 契约（双端 `useAdapterStore.ts` 同文）。
  - 双端删除 `useConfigStore.api` 字段（store 内缓存 IPC 客户端，数据域与 IPC 混面）：桌面 16 文件 / 安卓 13 文件约 30 处消费点（useAccount/useSettings/useNetwork/useAuth/useMonitor/useEventListeners/useGlobalShortcut/useHeartbeat/useInitialDataLoad/App/SettingsPanel/MobileMore/MobileQuickActions/TabletShell 及测试）改为静态 `import { tauriApiWithRetry as api }`，IPC 出口收敛回 `tauriApi.ts` 单点，`useConfigStore` 回归纯配置数据域。
  - 验证：双端 `npx tsc --noEmit --incremental` 0 错误；桌面 vitest 17 文件 103 用例全部通过；`.codewiki` 6 篇文章行号与语义同步更新。

### 文档

- **README 首图更换为 v2.4.0 三图展示**（2026-10-03）。删除旧首图 assets/screenshot-main.png（v2.3.9 界面），新增三张 v2.4.0 实机截图入库：`assets/screenshot-desktop-overview.png`（桌面总览：快捷操作/网络质量）、`assets/screenshot-desktop-adapters.png`（桌面网络适配器页：当前出站徽标、切换/还原出站、夜间出站目标指定、运营商徽标、长按拖拽排序说明）、`assets/screenshot-android-overview.png`（安卓总览：网络状态/Portal 可达/在线运营商徽标、底栏布局）。README 展示区改为「## 界面预览」节 + 两端小标题分组（### Windows 桌面端：总览全宽、网络适配器 85% 居中；### Android 端：竖图 32% 居中），全部图片 p align=center 居中（初版 49.5%×2 并排在 GitHub 内容区渲染过小、文字不可读且左对齐显零散，当日两次迭代：先改全宽主图+等高并排，再按用户要求改为两端标题分组+全图居中）。

- **全量版本号升级至 v2.4.0**（2026-10-03）。16 个入库文件同步替换：version.json（版本 + 更新日志 notes 换为 v2.4.0 要点导读）、双端 tauri.conf.json / Cargo.toml / Cargo.lock、双端 package.json 与 frontend package.json / package-lock.json、双端前端 `ui-constants.ts` APP_VERSION、桌面 about-preview.html、README 版本徽章；安卓 `gen/android/app/tauri.properties`（gen/ 为忽略目录，不入库）versionName=2.4.0、versionCode 2003009→2004000（major×10⁶+minor×10³+patch 规则）。历史记录类文本（update_cmds.rs 注释、learnings、codewiki、package-lock 中第三方 nwsapi 2.3.9）不动。

- **新增 v2.4.0 Release Notes**（2026-10-03）。按历代 release 风格（v2.3.8 大版本结构 + v2.3.9 下载渠道块）从本 changelog 提炼撰写 `release/RELEASE_NOTES_v2.4.0.md`：导读段三大重点（夜间出站自动切换、状态栏在线运营商徽标、APK 减半）+ 六个主题节（夜出站 7 条 / 在线运营商与检测 4 条 / 安卓体积与人脸 6 条 / 桌面界面精修 4 条 / 提权与稳定性 5 条 / 其他修复 4 条）+ 升级说明 9 条（默认值变化、恢复时间 7:30、已知边界等）。release/ 目录被 .gitignore:7 忽略，与历代 release notes 一致仅本地维护。

- **README 全面同步至 v2.4.0 状态 + 首图更换**（2026-10-03）。功能列表新增「夜间出站切换」（双端；桌面含网络适配器页长按拖拽指定目标网卡、当前出站徽标与一键切换/还原）与「在线运营商显示」（状态栏真实在线运营商徽标，双适配器并排）；「夜间定时切换」条目随 2026-09-22 双端 UI 改名同步更名「自动切换运营商」；安卓段补夜间出站切换（到点注销触发系统切蜂窝、7:30 重登恢复）、应用内 2D 人脸验证回退与模型按需下载（SHA256 校验）；平台差异行补「出站徽标与手动切换为桌面专属，人脸录入为安卓专属」；下载段补夸克网盘渠道（与 version.json 更新文案一致）；测试数字按实测口径更新（后端 397→432 单元 + 1→4 集成、前端 89→103）；安卓命令数 51→55；项目结构注释补 platform/ICMP 探测与出站路由、config/出站切换、monitor_loop 夜间自动切换、人脸模型下载；首图 assets/screenshot-main.png 原位替换为 v2.3.9 界面截图（含在线运营商徽标等新 UI）。版本徽章维持 2.3.9（当前发布版本，与 version.json 一致）。

- **README 同步至当前状态**（2026-09-21）。功能列表补夜间定时切换（v2.3.8 双端交付）、网卡禁用自动启用（免 UAC 提权链）、安卓 WiFi 名称（SSID）匹配（v2.3.9）与通知权限索取引导；移除 UI 入口已删除的「自动退出」条目（`autoExitAfterLogin` 开关 3ec615a 移除，字段保留默认 false）；测试数字按实测口径更新（后端 246→369 单元 + 1 集成、前端 74→89，`cargo test` / `npm test` 实测全绿）；安卓命令数 48→51；项目结构注释补 `config/night_switch`、`network/discovery/devnode`、`platform/task_proxy` 对应模块；安全说明提权条目改写为四级降级链（管理员直跑 → SYSTEM 计划任务代理 → COM 静默 → runas 弹 UAC）+ 计划任务 DACL 与结果目录白名单收口描述。版本徽章维持 2.3.9（当前发布版本）。

- **新增调研报告：Windows 多网卡出站切换 / 定时禁用网卡 / 静默提权**（2026-09-27）。为「夜间出站切换」六期方案（禁用校园网网卡 + 兜底路由 + 保留 metric）补外部取证，落 `docs/research/2026-09-27-windows-outbound-switch-research.md`，回答同类项目做法、切出站的四条姿势对比、静默提权形态对比、可恢复性设计模式、缺陷模式清单五问，每条结论附可复核 URL 并标注证据等级（项目实证 / 官方文档 / 推论 / 推测 / 否定性结论）。
  - **核心取证**：① 有效跃点 = 路由 metric + 接口 metric（[MIB_IPFORWARD_ROW2](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/ns-netioapi-mib_ipforward_row2) 等官方原文），故 TUN 接口 metric=0 时改物理卡 metric 恒无效，与五期实测互证；② WireGuard commit `66ad537f` 把比较式改为 `Metric + iface.Metric`，是"必须相加"的源码级实证；③ **`CM_Disable_DevNode` 默认不跨重启**（"after reboot the device is enabled again"），而 `SetupDiCallClassInstaller` **默认跨重启保持禁用**——两条路径默认行为相反，本项目走前者故有"重启即自愈"的天然兜底；④ UAC **不存在"一次提权长期复用"的官方 API**，`autoElevate` 白名单仅限微软内部签名，服务引入 Session 0 隔离——本项目「计划任务代理 + COM 静默提权」路线得到反向印证；⑤ `CreateIpForwardEntry2` 与 `SetIpInterfaceEntry` 均**无持久化语义**（无 `Store` 成员），印证本项目"不碰持久层、重启天然还原"。
  - **同类项目实证（4 个，均取到正文）**：[skoomasteve/NMAR](https://github.com/skoomasteve/NMAR)（网关可达性轮询 + 全量禁用/启用，自述多网卡误启用陷阱）、[aglasson/PSActiveNetworkSwitcher](https://github.com/aglasson/PSActiveNetworkSwitcher)（事件日志触发 + `Disable-NetAdapter` + SYSTEM 计划任务，**无任何还原/看门狗机制**，且用 `ifIndex` 排序选主卡——而网卡禁用再启用会改变 ifIndex，为设计级隐患）、[arlbibek/nadap-switch](https://github.com/arlbibek/nadap-switch)（GUI + `Enable All` 一键恢复）、[bensheed/windows-sleep-network-fix](https://github.com/bensheed/windows-sleep-network-fix)（启动与唤醒双触发重连）。
  - **缺陷模式实证**：mihomo [Issue #3186](https://github.com/MetaCubeX/mihomo/issues/3186) 记录 TUN 自建明细路由 `RouteMetric=0`（与本项目五期手机热点实测同型），并原文记录 "A separate **watchdog restored the working network after the test**" 与顺序纪律 "# A2: **restore this before restoring any full-public TUN routes**"；[PR #3004](https://github.com/MetaCubeX/mihomo/pull/3004) 在 `Close()` 里注册清理以在退出时移除自加规则——印证本项目"还原时必须清掉自己加的规则"纪律。另实测发现**显式绑定接口（`IP_UNICAST_IF`）在 strong host model + TUN 场景下也可能失效**（`getsockopt` 回读 index 正确但流量仍进 TUN）。
  - **否定性结论（避免后续误引）**：本轮**未找到**做真正路由表级"定时切出站"的开源项目（同类项目清一色停在 `Disable-NetAdapter` 层）→ 本项目"兜底路由"无直接先例，需更强的自证（切换后用 `Get-NetRoute -DestinationPrefix 0.0.0.0/0` 独立验证效果，不只看 helper 返回成功）；官方**无任何文档**支持"改物理卡接口 metric 可压过第三方 TUN 默认路由"；`netsh admin=disable` 落在哪一层官方未定义；`Disable-NetAdapter` 内部实现（PnP/CM vs NDIS admin）官方未公开；CMSTPLUA 的 CLSID `{3E5FC7F9-...}` **未找到可靠来源**（`gist.github.com` 本机 DNS 解析失败），故 `platform/elevation.rs` 的 COM 通道目前属"项目自证"级，缺独立佐证。
  - 仅文档改动（不涉及桌面/安卓代码），故无双端同步；未改 CodeWiki（属外部取证资料而非本项目决策/踩坑记录）。

## 2026-10-02 · 安卓底部悬浮栏布局调优（ui）

**模块**：安卓前端 BottomNav + App.tsx

- **需求**：用户反馈「修改安卓手机底部悬浮栏整体上移，增大图标大小，五个按钮之间的空隙增大」。
- **实现**（android/frontend/src/components/layout/BottomNav.tsx）：① 底缝 12px→20px（`bottom: calc(env(safe-area-inset-bottom) + 20px)`，整体上移）；② 页签图标 h-6 w-6→h-7 w-7（24→28px）；③ 页签群 gap-0.5→gap-2（2→8px）。非激活页签保持 w-9 不加宽（图标边距 4px 可接受），激活药丸 h-12 不变。
- **联动**（App.tsx L284）：main 底部避让随底缝 +8px——dashboard 148→156px、其他页签 88→96px；删除已失效的「快捷登录浮条 +92px」陈旧注释（浮条已并入底栏，见 BottomNav「栏内快捷登录对」）。
- **验证**：安卓 tsc --noEmit 0 错误；playwright-core + 系统 Chrome 真浏览器双宽度实测（mock 注入临时加、验证后已还原）：412×915 底栏宽 388px 页签群 266px、360×780 窄屏底栏 336px 页签群 214px——5 页签+注销/登录完整显示无截断（7 按钮 clipped=0），截图复验上移/图标/间距三处生效。桌面端与 DockNav（平板/桌面镜像组件）零改动。

## 2026-10-02 · 当前出站徽标只认真实物理网卡（fix）

**模块**：后端 platform/best_route.rs

- **需求**：用户反馈「出站网卡只显示真实的物理网卡出口，而不是显示虚拟网卡劫持的出口」——FlClash TUN 劫持默认路由时头部徽标一直显示 FlClash，用户想知道真实物理出口是哪张卡。
- **实现**：`current_outbound_v4` 判定口径变更——默认路由候选改为带接口别名收集（有效跃点=路由 Metric+接口 Metric 不变），新增 `best_physical_outbound` 纯函数：别名命中适配器发现黑名单（`discovery::is_blacklisted`，与适配器列表同一过滤口径，覆盖 clash/mihomo/wintun/tun/vpn/wireguard/tailscale/hyper-v/虚拟/隧道 等）的虚拟卡不参与选优，取跃点最优的物理网卡；别名查询失败（空串）保守视为物理网卡；全部候选均为虚拟卡时返回 Err「没有承载真实出口的物理网卡默认路由（仅虚拟网卡活跃）」，`get_current_outbound_name` 软降级 None → 前端隐藏徽标（永不显示虚拟卡）。
- **验证**：新增单测 3 个（虚拟劫持跳过取次优物理/全虚拟 None/空别名保留），cargo test --lib 432 passed 0 failed 5 ignored；真机 `real_machine_current_outbound`（FlClash TUN ifIndex 45 活跃）返回 **ifIndex 16 以太网**，断言非黑名单通过；clippy 14 错误经 git stash 对照 main 基线确认为 rustc 1.98 新 lint 打在既有代码（auth/protocol.rs:512 等 12 文件），非本次引入。
- 展示口径变化：徽标名与适配器列表行名一致（同一黑名单过滤）；TUN 活跃时显示「以太网/WLAN」等真实物理出口而非 FlClash。

## 2026-10-02 · 安卓底栏：6 字标签完整显示不截断（fix）

**模块**：android/frontend BottomNav

- 用户反馈：关闭网络质量检测后第 4 页签「网络状态检测」6 字被 `max-w-[64px]` 截断成「网络状态检…」（66px>64px）；且测试时质量功能基本关闭，该形态是常态。
- 修复：标签移除 truncate+max-w，改 whitespace-nowrap 自然宽度——justify-around 均布余量充足（412px 右余 80px / 360px 右余 65px），6 字完整显示。
- 复测：双宽度 fullyVisible=true、无横向溢出；中心距 73.6/73.6/80.6/80.6（412）、63.2/63.2/70.2/70.2（360）近均等；tsc 0。changelog 不入库。

## 2026-10-02 · 桌面 Dock 同步：选中药丸只包图标（fix）

**模块**：tauri-app/frontend DockNav

- 同步安卓底栏七轮：DockItem 药丸 `layoutId="dock-active-pill"` 从 inset-0 包整钮（图标+标签蛋形）改为固定 40×28 图标盒（h-7 w-10，桌面图标 18px）内 inset-0 只包图标，标签移到药丸外；按钮宽由图标盒决定自然等宽。
- playwright 实测：btn 40×53 / 图标盒 40×28 / 药丸 40×28 且 pillInsideBox=true、labelOutsidePill=true；切「账号」后药丸同为 40×28 恒定。tsc 0。changelog 不入库。

## 2026-10-02 · 安卓底栏七轮：选中药丸只包图标（fix）

**模块**：android/frontend BottomNav

- 用户问「背景块不包裹文字是否更好防止变形」——是，M3 正统做法：指示器只包图标，标签在药丸外下方。原 inset-0 药丸包整颗按钮成 46×56 蛋形胶囊，且切换时随按钮宽 36↔46 缩放变形。
- 重构：图标盒固定 52×32（h-8 w-[52px]），药丸 inset-0 恒定体育场形，layoutId 滑动纯平移零变形；按钮宽度条件类删除（内容定宽，五钮自然等宽 52px）。
- 复测：tsc 0；中心距 412px 77/76/76/77、360px 66/66/66/66 全均等；动作条仍仅总览页。changelog 不入库。

## 2026-10-02 · 安卓底栏六轮：图标 26px 折中（fix）

**模块**：android/frontend BottomNav

- 用户「适当放大图标」：24px 在均布后偏小，28px 曾挤药丸——取折中 26px（h-[26px]）。药丸占比 57%（28px 时 64%），激活药丸宽 44→46px，justify-around 自动吸收无溢出。
- playwright 复测：中心距 80/74/74/75 均等、360px 69/64/64/64，无重叠；android tsc 0 错误；changelog 不入库。

## 2026-10-02 · 安卓底栏五轮：页签均匀分布消除右侧死空间（fix）

**模块**：android/frontend BottomNav

- 用户截图反馈「右边空隙太多」——登录对移走后页签群仍左聚簇，按钮不 grow 导致死空间全聚右缘。页签群加 justify-around 均匀分布（gap-2 保底为窄屏最小间距），药丸造型不动；M3「目的地等分栏宽」形态。
- playwright 量化复核：页签中心距均等（412px: 79/75/75/75，360px: 69/64/64/65），右缘贴合（lastBtnRight 378 vs barRight 400，残差 22px 为 justify-around 对称半槽）。android tsc 0 错误；changelog 不入库。

## 2026-10-02 · 安卓底栏四轮：登录/注销独立动作条（feat）

**模块**：android/frontend BottomNav + MobileQuickActions(复活) + App

- 用户要求「悬浮栏上方再加一个」+web 调研（M3 导航栏只放目的地、动作配 FAB/bottom app bar；B 站式页内登录）→ 复活 4d5e441 合并前的 MobileQuickActions 独立悬浮对（git show 986aa4a 原样取回），改挂回 dashboard。
- BottomNav 删除 CompactAuthButtons（-53 行）回到纯页签；MobileQuickActions bottom 92→100px（dock 顶缘 88+12 缝）、按钮 h-12/图标 h-5/text-sm 原历史值——按钮即背景块，48px 胶囊对 24px 图标比例 50% 无挤压。
- App.tsx：lazy import+仅 dashboard 挂载+paddingBottom 216/108。playwright 复核：总览动作条可见、切更多页消失（仅总览）、双宽度 5 页签 clipped=0 tabsW 374/322。android tsc 0 错误；changelog 不入库。

## 2026-10-02 · 安卓底栏三轮：图标回 24px、药丸留白（fix）

**模块**：android/frontend BottomNav + App

- 用户反馈「图标太大挤压了背景块」——28px 图标占激活药丸宽度 64%、竖向填充 80%，胶囊被顶满。图标回 24px（Material 3 导航栏标准），激活药丸 px-2→px-2.5、激活描边 2.2→2；放大感由栏体（h-14/p-1.5）、间距（8px）、文案（11px）承担，全部保留。
- 激活药丸宽度不变（24+2×10=44px），360px 最窄档预算零变化；playwright 复测 412/360 均 7 按钮 clipped=0、页签群与注销无重叠（gapPx=2）。
- 顺带修正 App.tsx 注释（栏高实为 68px，上轮误写 80px）。android tsc 0 错误；changelog 不入库。

## 2026-10-02 · 安卓底栏二轮：栏体随图标等比放大（fix）

**模块**：android/frontend BottomNav + App

- 用户反馈「整体悬浮栏大小也增加，图标太大不协调」——28px 图标进 56px 旧栏体导致比例失调。栏体等比放大：bar 内边距 p-1→p-1.5、页签按钮 h-12→h-14（栏总高 56→80px）、激活标签 text-[10px]→text-[11px]、快捷登录对 h-8→h-9 text-[10px]→text-[11px]；max-w-[440px]→[460px]。底缝 20px 与图标 28px、页签间距 8px 保持不变（上轮已定）。
- App.tsx paddingBottom 同步栏高：dashboard 156→168px、其他 96→108px。
- playwright（复用 5174 端口 dev server，dev 按需读磁盘故上轮未死 server 结果有效）双宽度验证：412px tabsW 258 / 360px tabsW 206，7 按钮 clipped=0；激活「更多」页签切页复核，页签群与注销按钮无重叠（gapPx=2 为群盒右缘 vs 注销左缘，按钮内容互不相碰）。360px 最窄档预算重算：内容 324 ≥ 页签 216+登录对 106，容纳。
- 截图自审（frontend-design 纪律）：曾考虑激活药丸内加选中圆点，自行否决——用户嫌大时加装饰违背克制原则，色块即选中态。
- 验证：android tsc --noEmit 0 错误；changelog 不入库。

## 2026-10-02 · 二轮三反馈修正：toast 回左上 + 徽标移位 + 头部文字按钮（fix）

**模块**：前端 NetworkPanel + ToastContainer（双端）

- **① toast 位置回退左上**（用户反馈「通知被改到了右边去了」）：上轮把右上理解成了 toast 期望位置，实际用户只是描述没看到原因——`ToastContainer.tsx` 桌面+安卓同回 `right-4`→`left-4`，动画 x 镜像回 -100/-80。
- **② 夜间出站目标徽标移位**（用户反馈「放在获取新IP左边」）：徽标块从 renew Button 之后移到之前，渲染顺序变为 [夜间出站目标][获取新IP]。
- **③ 头部图标按钮改文字按钮**（用户反馈「按钮不够直观，用户不知道用处是什么」+「按钮名称要简洁」）：去胶囊壳（`rounded-full bg-muted/40 p-1` 删除），⚡/↺ 两个 icon-only ghost 按钮改为 outline 文字按钮「切换出站」「还原出站」（h-7 text-xs，切换钮 amber 配色，Loader2 转圈逻辑保留；title 同步复用同名 i18n 键）。i18n 值缩短：zh「立即切换出站」→「切换出站」、「立即还原出站」→「还原出站」，en "Switch Outbound Now"→"Switch Outbound"/"Restore Outbound Now"→"Restore Outbound"（键不变，无新增）。
- **mock 浏览器验证**（用户要求「使用mock注入现在浏览器中验证」）：扩展不入库 `public/mock-tauri.js`（mockAdapters 3 张卡/WLAN+以太网+以太网 2、outboundPriority、get_current_outbound_name='FlClash'、outbound_switch_now/outbound_restore_now 失败样例、`__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener` 补齐 @tauri-apps/api v2 _unlisten 依赖、`__TAURI_PATTERN__` 标志位补 getCurrentWindow 事件链）；playwright-core（PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 + channel:'chrome' 系统浏览器）真实点击「切换出站」→ mock 返回失败 → toast 左上角带完整原因渲染，DOM 断言 `toast visible = true`。headless 注意：`document.visibilityState` 需强制 'visible'，否则 useLogToastStore.addToast 直接丢弃。
- **验证**：桌面 tsc --noEmit 0 错误；vitest 17 文件 103 tests 全过；vite build 4.86s；安卓 tsc 0 错误。
- 环境备注：验证用 index.html 注入行 + useLogToastStore/useAdapterStore 模块级窗口句柄（`window.__logToastStore`/`window.__adapterStore`）均临时添加、已还原不入库。

## 2026-10-02 · 三反馈修复：失败原因透传 + toast 右上角 + 出站目标徽标回调（fix）

**模块**：src-tauri monitor/scheduled（桌面）+ 前端 NetworkPanel/ToastContainer（双端）

- **① 切换失败原因直透 toast**（用户反馈「切换失败没有显示原因」）：`apply_outbound_switch`/`apply_outbound_restore`/`write_outbound_metric`/`watchdog_re_disable_campus`/`run_outbound_blocking` 由 bool 改 `Result<(), String>`，10+ 处失败分支各自携带具体原因（适配器查询失败/无可用候选/无 GUID/跃点读取失败/快照落盘失败/禁用失败/路由添加失败/路由验证失败等）；聚合步失败以 `fail_reasons` 收集后 join("；") 作为 Err 文案；`manual_outbound_switch`/`manual_outbound_restore` 弃用通用兜底文案（「切换未完成，请查看日志或稍后重试」「还原未完成，系统将自动重试，请查看日志」已删），原因直透前端 toast。调度循环两拍以 `is_ok()` 判定语义等价旧 bool；gate 退避窗跳过也返回 Err（自动拍不展示，手动拍如实告知「正处于自动重试退避窗口」）。
- **② toast 容器移至右上角**（用户反馈「在右上角显示」）：`ToastContainer.tsx` 桌面+安卓同改 `left-4`→`right-4`，入/出场动画 x 方向镜像（-100/-80 → 100/80）。
- **③ 夜间出站目标徽标回调文字版**（用户反馈「夜间出战目标徽标没了」）：上一轮改的 MoonStar 小图标太隐晦，恢复为名称旁文字徽标（`network.outboundBadge`，条件 `displayedOrder[0] === a.name`）；头部胶囊簇去 `border`（降噪，用户反馈样式需精修）。
- **验证**：cargo test --lib 429 passed 0 failed；cargo check --lib 无警告；桌面+安卓 tsc --noEmit 0 错误；vitest 17 文件 103 tests 全过；vite build 5.29s。
- 环境备注：本次验证时 pnpm 在 `tauri-app/frontend` 触发全量重装（+276 包）并自动生成占位 `pnpm-workspace.yaml`（已删）与重写未入库的 `pnpm-lock.yaml`（未提交，保留）。

## 2026-10-02 · 网络适配器卡徽标精简（UI 重构）

**模块**：前端 NetworkPanel（桌面）

- **问题**：单行适配器条目最多同时叠 7 类徽标（主/副、运营商、无线/有线、状态、启用、获取新IP、夜间出站目标、当前出站），信息密度过载、视觉杂乱。
- **重构（信息零丢失）**：
  - 删「无线/有线」徽标——左侧 Wifi/Cable 图标已表达同义信息；孤儿 i18n key `network.wireless` 同步移除（zh/en），`network.wired` 仍被 MonitorPanel 使用保留。
  - 「主适配器/副适配器」徽标 → 名称下 meta 行前缀文字「主适配器 · 10.x.x.x」（复用 network.primary/secondary）。
  - 「夜间出站目标」文字徽标 → 名称旁小 MoonStar 图标（span 承载 title+aria-label，行底色高亮保留）。
  - 「当前出站」文字徽标 → 适配器图标右下角翡翠圆点（h-3.5 w-3.5 bg-emerald-500 ring-2 ring-background）；全局当前出站状态仍在头部徽标。
- **头部**：当前出站 chip + 立即切换(Zap)/立即还原(RotateCcw) 收进一个 rounded-full 胶囊簇（按钮改 ghost h-6 w-6 p-0），MoonStar+夜切开关独立在外，形成两个语义分组。
- **验证**：tsc --noEmit 0 错误；vitest 103 passed；vite build 4.76s；i18n JSON 解析合法。
