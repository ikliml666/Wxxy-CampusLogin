# 更新日志

## v2.3.8（2026-09-20 发布）

### 行为调整

- **2026-09-19 新增科教中心无线网络 SSID 匹配（合并外部 PR #2）** — 桌面端校园网名称检测（`tauri-app/src-tauri/src/monitor/campus_check.rs:88-91`）在用户配置为默认值 `i-wxxy` 时额外匹配 `iwxxy-2` / `iwxxy-3`（忽略大小写）；用户自定义了网络名称时仍严格按自定义值匹配，行为不变。双端四份 i18n 文案同步更新（桌面 + 安卓各 en/zh 的 `campusNetworkNamePlaceholder` / `campusNetworkNameTip` / `detectionLogicStep1`）。贡献者：srcenchen。
  **为什么**：科教中心区域 SSID 与默认 `i-wxxy` 不同（为 `iwxxy-2` / `iwxxy-3`），默认配置的用户在科教中心会因名称不匹配而判定不在校园网；额外匹配以"用户使用默认值"为前提，不侵入自定义配置的语义。安卓端仅同步文案——安卓 Rust 无 SSID 检测通道（`android/src-tauri/src/campus_detect.rs` 注释明确，检测走 /18 子网 + Portal/网关 TCP），检测行为不变。
  **验证**：PR 分支实检出跑验证——`cargo test` 329 独立用例全绿（lib 328 + 集成 1，0 失败）；双端 `npx tsc --noEmit --incremental` 通过；4 个 i18n JSON 语法校验通过。Squash 合并为 `a9f56e5`（`feat: 新增科教中心无线网络 SSID (#2)`）。

- **2026-09-19 新增「晚间断网自动切换运营商」（双端）** — 账号管理页自动化开关卡新增第 5 个开关"晚间断网自动切换"（默认关，双端同卡同位、同 i18n 键）。开启后按固定时间表自动切换：周日/周一 23:00、周五/周六 23:30 把登录运营商切至无锡学院（原运营商暂存新配置字段 `nightOperatorRestore`），每日 6:30 起自动切回；当前运营商仅在三家白名单（电信/联通/移动）内才触发切换。判定逻辑为共享纯函数 `tauri-app/src-tauri/src/config/night_switch.rs::evaluate_night_switch`（安卓经 Cargo path 依赖复用，禁止复制），桌面接入 `monitor/scheduled.rs` 30s 循环、安卓接入 `monitor_loop.rs::run_scheduled_actions`（均带过点补触发与落盘失败下拍重试）；切账号时自动清空暂存（防止跨账号污染）。新增双端共有配置字段 `enableNightOperatorSwitch` / `nightOperatorRestore`（serde default 兼容旧配置，无 schema 迁移）。
  **为什么**：运营商线路有固定断网规律（周日晚/周一晚 23:00、周五晚/周六晚 23:30 后电信/移动/联通服务下线，切至无锡学院可继续上网但网速较慢）；用户要求开关开启后到点自动切换、关闭不生效，次日自动切回。
  **已知边界（有意不修，文案已提示"需保持应用在后台运行"）**：午夜 0 点至次日 6:30 之间启动 App 不补触发切换（6:30 恢复窗口自愈）；桌面 `autoExitAfterLogin` 开启时夜切登录成功即退出，恢复推迟到下次启动 App；安卓侧依赖后台检测循环（桌面循环无条件启动）。评审（deepseek-flash 交叉复核）发现的 2 个 P1 问题已随本次修复：安卓后端保存配置后此前从不广播 `config-changed`（夜切状态会被前端旧快照覆盖回写且 UI 不反映），现 `save_config` 命令与夜切落盘统一广播（掩码复用 `masked_for_display`）；安卓夜切登录改走直接登录路径，绕过 `auto_login_on_start` 的"不在校园网则跳过"探测闸（避免静默不登录）。
  **验证**：桌面 `cargo test` 342 用例全绿（lib 341 含夜切纯函数 13 个新增单测 + 集成 1）；双端 `npx tsc --noEmit --incremental` 通过；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27 显式工具链注入）通过；浏览器实测（vite dev + `__TAURI_INTERNALS__` mock，用后已还原）两端账号页开关显示/翻转/`save_config` 落盘（`enableNightOperatorSwitch: true`）与安卓更多页 chips 新顺序、默认选中均符合预期。

- **2026-09-19 夜切动作后自动验证切换是否生效（双端）** — 晚间断网自动切换在执行切换/切回并重新登录后，自动查询 eportal 在线状态接口（`GET {portal_url origin}/drcom/chkstatus`，按请求源 IP 返回本机在线账号 `uid`，2026-09-19 实测逆向发现）核对在线账号后缀：切换后期望为无后缀学号（无锡学院）、切回后期望为原运营商后缀，忽略大小写与首尾空白比对。登录完成后等 15s 首查，查询失败重试最多 3 次（间隔 5s、单次超时 15s）；未生效先复查配置未被用户手动改走，随后执行一轮「注销→重新登录」再复验一轮；仍不符则写告警日志并发系统通知，生效写成功日志。共享解析/核对纯函数（`parse_chkstatus`/`uid_matches`）落在桌面 crate night_switch 模块（含 BOM/JSONP 壳剥离与容错，7 个单测），安卓经 path 依赖复用。
  **为什么**：夜切的自动性决定了无人值守，切换是否真正生效（认证成功且在线账号后缀正确）此前无从得知；chkstatus 的 uid 字段可精确核对在线账号，验证闭环补齐后未生效可自动自救一轮（注销重登）并确保用户最终会被告知。
  **验证**：桌面 `cargo test` 349 用例全绿（lib 348 含新增 7 个解析/核对单测 + 集成 1）；安卓 `cargo check --target aarch64-linux-android --all-targets`（NDK 27）通过；chkstatus 实测响应样例（`uid":"24385214@cmcc"`，含前导空格/BOM 形态）已固化为解析单测；未发起任何真实切换/注销动作。

- **2026-09-19 安卓「更多」页顶部标签重排：设置、测速、日志、后台检测（默认选中设置）** — `android/frontend/src/components/mobile/MobileMore.tsx` 的 `TABS` 顺序由 后台检测→测速→日志→设置 调整为 **设置→测速→日志→后台检测**（后台检测 chip 仍仅在质量检测开启时出现、殿后），默认选中子页由 `monitor` 改为 `settings`，质量检测关闭时 monitor 的派生降级目标由 `speedtest` 改为 `settings`（第一个 chip），chips 与内容区保持一致。
  **为什么**：用户要求常用入口前置——设置排第一、测速次之、日志其后。
  **验证**：浏览器实测 chips 顺序与默认选中、内容区渲染正确；`npx tsc --noEmit --incremental` 通过。

### 缺陷修复

- **2026-09-19 修复桌面端 `config-changed` 事件 payload 缺 `{config}` 包裹（历史回归）** — `tauri-app/src-tauri/src/commands/config_cmd.rs` 的 `save_config_to_disk_encrypted` 此前直发裸掩码 Config 对象，而双端前端监听契约均为 `data.config`（`useEventListeners` 的 `if (data?.config)` 门卫），导致桌面前端对该事件整体静默失效（后端改配置后前端不同步）。git 考古确认历史正确形态为 `json!({"config": ...})` 包裹（`80c4ecf` 引入、M1 重构 `dd89460` 收敛 EventBus 时丢失）。本次随夜切功能首次重新依赖该事件，一并修复；安卓侧新建广播时已按正确契约实现。
  **为什么**：夜切自动切换发生后前端需经该事件同步新运营商值；不修则桌面端 UI 显示与实际配置脱节。
  **验证**：`cargo test` 342 用例全绿；桌面浏览器实测开关翻转后 `save_config` 落盘正常。

### 文档

- **2026-09-15 更新日志改为按版本文件维护（changelogs/），废弃统一 CHANGELOG.md** — 归档重组：原单文件 CHANGELOG.md（73 条全部堆积在 [Unreleased]）按日期切分——2026-09-12 起的 68 条归 `changelogs/CHANGELOG_v2.3.7.md`（v2.3.7 开发期内容），2026-09-11 的 4 条归 `changelogs/CHANGELOG_v2.3.6.md`（v2.3.6 时代内容）；`CHANGELOG_v2.3.6.md` 原为 9/15 版本号升级前的整份快照，混入的 9/12+ 条目（即 v2.3.7 内容）已全部移出；1 条相邻重复条目（"顶栏恢复独立 status 圆点"）去重为一份。项目根 CHANGELOG.md 删除。AGENTS.md 必守约定 1 改写：新内容默认写入 `changelogs/CHANGELOG_v<当前版本+1>.md`（当前 v2.3.7 → `CHANGELOG_v2.3.8.md`），发布后封版、新内容顺延下一版文件，不再有快照步骤；`make-release.ps1`（本地脚本）同步删除 CHANGELOG 快照段、RELEASE_NOTES 缺失提示改为指向 changelogs/。决策记录见 CodeWiki `decisions/changelog-per-version-files`。
  **为什么**：单文件 [Unreleased] 无限堆积，且快照时机滞后造成混版——v2.3.6 的快照实际混入 68 条 v2.3.7 条目，本次靠日期切分手工拆分；写入时即确定版本归属可消除这类返工。
  **验证**：切分条目数守恒（原 73 = v2.3.7 68 + v2.3.6 4 + 去重 1）；两归档文件条目首行日期实测 v2.3.7 = 2026-09-12~09-15、v2.3.6 = 2026-09-11；make-release.ps1 pwsh Parser 语法检查零错误。

### 后台留存优化（2026-09-20 补记：本批先于第二/三批实施，条目此前漏记）

- **2026-09-20 桌面后台留存优化——轻量化模式 + Windows 效率模式 + 默认值调整（提交 69b5b96..72109ca）** — 新增「轻量化模式」（`lightweight_mode` 默认开）：点关闭按钮窗口真销毁（WebView2 进程组退出）但后端核心功能保留，托盘/单实例/show_window 五处统一经 `WebviewWindowBuilder::from_config` 重建入口唤回（几何恢复 + ready 门防白闪、5s 超时兜底）；`RunEvent::ExitRequested` 两段式拦截隐式退出配单次退出守卫（`EXPECT_LIGHTWEIGHT_EXIT` swap 语义），托盘退出菜单仍可完整退出。轻量化期间自动启用 Windows 效率模式（`SetProcessInformation` ProcessPowerThrottling 三态写法，随轻量化启停）并把网络状态检测间隔抬至 ≥300s、质量检测 ≥1800s（原值恢复随去轻量化）。默认值调整：`auto_exit_after_login` / `auto_exit_on_online` / `minimize_to_tray` 默认关，`background_check_interval` 默认 60s、`latency_test_interval` 默认 600s；`config_version` v3→v4 迁移。质量循环改每轮重读配置，顺带修复运行中改间隔不重启任务不生效的历史缺陷。
  **为什么**：配合晚间断网自动切换需要进程常驻，而用户游戏时对后台检测的延迟抖动与资源占用敏感——点 X 后零窗口渲染 + 效率模式降调度 + 检测降频，以接受托盘唤回的二次延迟为代价换取后台低占用。
  **验证**：桌面 `cargo test` 全绿（本批时点 697 用例，后续批次增至 719）；浏览器实测点 X 后窗口销毁、托盘重建、设置页轻量化开关（mock + vite dev，用后 git 还原）；EcoQoS 三态纯函数单测覆盖。待真机确认：安装包环境点 X 后 WebView2 进程消失与任务管理器效率模式叶标。

- **2026-09-20 安卓后台内存与稳定——退出护栏 + 稳态退避 + renderer 放行（提交 9590d81..6feb2b2）** — ① 修复 `RunEvent::ExitRequested` 未拦截导致划掉任务卡片连带杀前台服务（tao `process::exit(0)` 连带问题，tauri #15671），加 `api.prevent_exit()` 护栏；② 质量循环稳态退避：连续 5 拍质量良好（excellent/great/good）间隔翻倍、封顶 1800s，波动即恢复原间隔（`next_backoff_interval_ms` 纯函数）；③ 质量检测默认间隔 600s（`config_schema_version` v5→v6 迁移），前端 auto_exit 幽灵常量与桌面对齐清理；④ renderer 优先级放行：`onWebViewCreate` 覆写 `setRendererPriorityPolicy(BOUND, waivedWhenNotVisible=true)`（API 26+），退后台允许系统回收渲染进程，回前台重建（决策已析：不新增 crash 面，极端 OOM 由 START_STICKY 自愈）。
  **为什么**：后台检测常驻是夜切与保活的前提，安卓侧需要在不牺牲保活的前提下把后台内存与功耗压到最低。
  **验证**：`cargo check --target aarch64-linux-android --all-targets`（NDK 27 显式工具链注入）通过；退避纯函数单测覆盖。待真机确认：划任务后前台服务存活、`dumpsys meminfo` 前后台内存对比。

---

---

# 追加（2026-09-20 第二批：定时动作哨兵化 + 固定匹配 + 截断修复）

> 分支 `feat/sess-52f23648-scheduled-disable-and-fixed-fields`（3 个提交 + 文档）。

## 行为调整

- **夜切默认开启**：`enableNightOperatorSwitch` 旧默认 false → **true**（双端）。过点补触发语义下：晚间切换时刻后开机立即切到无锡学院、06:30 后开机且处于切换态则立即恢复原运营商（00:00~06:30 开机不补触发，06:30 恢复窗口自愈）。存量旧默认 false 由桌面 v4→v5 / 安卓 v6→v7 迁移刷为 true。
- **每日定时登录/注销提供关闭选项**：禁用值从 0（= 00:00）改为哨兵 **1440**，00:00 变为真实可设时刻；前端卡片新增独立「启用」开关（关闭=哨兵 1440，开启时从哨兵态给默认 08:00），时间选择框与开关联动禁用。判定纯函数 `should_fire_scheduled_action` 改 `target_minutes >= 1440 → false`，桌面 validate clamp 放行 `.min(1440)`，安卓双禁用短路同步。存量 0 由迁移刷为 1440。

## 缺陷修复

- **安卓时间选择框数字截断**：`校园网检测时间段`起止、`每日定时登录`、`每日定时注销` 四处 `<input type="time">` 由 `w-24`（96px）加宽至 `w-32`——Android WebView 原生时间控件渲染「HH : MM + 调节箭头」超出 96px 导致数字被裁。

## 前端调整

- **固定匹配展示**：`校园网络名称`（固定 i-wxxy，额外匹配 iwxxy-2/iwxxy-3）、`校园网关地址`（固定 10.2.127.254）、网络质量检测`固定网关地址`（固定 10.2.127.254）双端共 6 处手填入口（草稿 + blur 提交模式）改为**固定只读展示具体值**，说明文案同步；后端字段与兜底校验保留（导入配置仍可携带，仅 UI 不提供编辑）。净删 112 行草稿代码。

## 验证数据

- 桌面 `cargo test` 全绿 **719 用例 0 失败**（新增：v4→v5 迁移、禁用哨兵 1440 判定、00:00 时刻触发、新默认断言等）。
- 双端 `npx tsc --noEmit --incremental` **0 errors**；安卓 `cargo check --target aarch64-linux-android --all-targets` **0 errors**。
- 浏览器实测（mock + vite dev）：桌面端监控面板「校园网络名称/校园网关」固定只读展示块、定时登录/注销启用开关联动（关闭=时间框禁用显示 --:--，开启=启用并给默认 08:00）、设置面板轻量化开关默认开、质量检测固定网关展示块——全部通过（含截图）。修复过程中发现并纠正：检测时段结束时间框漏加宽导致与开始框不等宽（3f9c267）。
- 待真机验证：安卓时间框（w-32×4 处已 grep 核实一致）在 Android WebView 原生控件下不再截断（桌面浏览器无法复现该原生渲染）；定时登录/注销开关 + 00:00 时刻实际触发。

---

# 追加（2026-09-20 第三批：账号面板自动化卡移除两开关 + 夜切文案对齐）

> 分支 feat/sess-52f23648-remove-auto-exit-ui（2 个提交）。

## 前端调整

- 账号面板「自动化设置」卡移除「登录成功后退出」「在线后自动退出」两个开关（3ec615a）：默认 false 配合夜切与后台常驻；配置字段与后端生命周期逻辑保留，存量显式 true 仍生效。设置面板与安卓端同款入口不动。
- 夜切 i18n 文案时间表对齐（d00b280）：双端中英文案由「周日、周一 23:00」改为「周日至周四 23:00」（补第二批时间表更正的文案遗漏）。

## 验证数据

- npx tsc --noEmit 双端 0 errors。
- 浏览器实测（mock+vite）：账号面板自动化卡仅剩自动登录校园网/可登录时自动登录/晚间断网自动切换三开关（截图留档）；设置面板同名开关保留未动。

---

# 追加（2026-10-03 第四批：夜间出站目标默认 WLAN/无线网卡优先）

> 分支 `feat/sess-session--night-outbound-wlan`（1 个提交，仅桌面端——适配器跃点切换为 Windows 专属能力，安卓夜间出站语义=注销+WiFi 重检无此概念）。

## 行为调整

- **夜间出站切换目标默认 WLAN（未排序时的代码默认语义，2026-10-03）**：`outboundPriority` 为空（新装/清配置/未拖动排序）时，出站生效顺序由「适配器发现顺序」改为「**无线网卡优先、有线网卡随后**（组内保持发现顺序）」——夜间出站目标默认落在 WLAN/无线出口，校园有线出口不会再成为首选目标。生效顺序同时是校园卡禁用白名单：未排序时在网校园卡同样纳入夜间临时禁用，与已排序语义一致。用户已排序（列表非空）时行为完全不变。
  实现：后端新增纯函数 `tauri-app/src-tauri/src/monitor/outbound_switch.rs::effective_outbound_priority`（`tauri-app/src-tauri/src/monitor/scheduled.rs::apply_outbound_switch` 调用点计算一次，同时供 `select_outbound_candidate` / `select_campus_to_disable` / 空禁用名单告警判定消费）；原「未配置优先级」静默跳过分支（debug 日志 + 「尚未配置出站优先级」错误）随默认序生效而移除，统一并入「无可用非校园网候选」每日一次告警。前端 `tauri-app/frontend/src/network/outboundOrder.ts::buildOutboundOrder` 增加第三参 `wirelessNames`（空优先级 + 无线名单非空 → 无线优先序），`tauri-app/frontend/src/network/NetworkPanel.tsx` 拖拽列表与「夜间出站目标」徽标同语义展示。
  **为什么**：用户要求夜间出站目标默认调整为 WLAN；名字硬编码默认（写死 "WLAN"）在英文系统（"Wi-Fi"）与多网卡环境下不成立，泛化为「无线优先」在本机恰产生与既有落盘 `["WLAN", "以太网", "以太网 2"]` 一致的顺序。
  **文案同步**：适配器列表拖拽提示补「未排序时默认无线网卡（WLAN）优先」；删除已失真的「列表外网卡不参与切换」（未排序时全部检测到的网卡默认参与）。
  **验证**：桌面 `cargo test` 全绿 **447 用例 0 失败**（lib 442 + 集成 4 + 其他 1，新增 `effective_outbound_priority` 单测 4 个：非空透传/无线优先保发现序/多无线卡保序/空列表）；前端 `npx tsc --noEmit --incremental` 0 errors；`outboundOrder.test.ts` vitest 10 用例全绿（新增无线优先/多无线卡/名单外忽略/纯函数不可变等断言）。既有 `select_outbound_candidate` 全部单测未动（select 语义未变，默认序在调用点展开）。
