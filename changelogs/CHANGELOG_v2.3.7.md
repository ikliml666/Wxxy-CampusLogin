# 更新日志

## v2.3.7


### 行为调整

- **2026-09-15 版本号全量升级 v2.3.7（发布准备）+ Release 文稿 + 双端更新日志维护** — 附录 H 清单 15 处：
  双端 `tauri.conf.json`/`Cargo.toml`、双端 `package.json`+`package-lock.json`、双端 `ui-constants.ts`、
  `about-preview.html`（v 前缀×2）、README 徽章、`version.json`；桌面 `Cargo.lock` 由 cargo check 重写、
  安卓 `Cargo.lock` 手动同步 `campus-login`/`campus-login-android` 两条目。`version.json` 随版本号补 `notes`
  （双端关于弹窗/更新弹窗共用，`### 标题`/`- 列表` 渲染契约双端同构已核实），用户确认随本次合并推送
  （Release 发布窗口期由用户把握，404 教训已知）。新写 `release/RELEASE_NOTES_v2.3.7.md`（本地不入库），
  内容取自 9/12 晚 v2.3.6 发布后的全部 [Unreleased] 条目并按用户可见性取舍（构建脚本/文档类不进 release）。
  **验证**：双端 `npx tsc --noEmit --incremental` rc=0；桌面 `cargo check` rc=0（`Cargo.lock` 2.3.7）；
  全仓 grep 无 2.3.6 残留（`THIRD-PARTY-NOTICES.md` 第三方依赖名与 `update_cmds.rs` 测试字面量属正常语义引用）。


- **2026-09-15 桌面构建缓存项目内化 + 发布脚本产物路径修正（本地脚本，不入库）** — `tauri-app/build.ps1`
  废弃 `C:\cl-build\target` 短路径（原为避 Windows 路径过长），`CARGO_TARGET_DIR` 默认改到项目根 `target\`——
  编译缓存与产物统一放项目内，路径过长时仍可用 `-TargetDir` 逃生；`make-release.ps1` bundle 候选列表同步改为
  仅项目内（去掉 C 盘与 `src-tauri\target` 旧候选，防 -SkipBuild 误收旧产物），并修复 APK 收集路径 bug
  （`android\gen\...` → `android\src-tauri\gen\...`，release/ 此前从未收到过 APK）；`clean-build-cache.ps1`
  清理列表按用户要求扩为「所有编译缓存」：九个目录（四处 cargo target + gradle build/.gradle + 前端 dist 两个）+ 两个前端的 *.tsbuildinfo（tsc 增量缓存）；gen 下未收集进 release 的 APK 唯一副本（2.3.6）删除前红色告警。-WhatIf 实测合计 10.50 GB。
  同日 make-release.ps1 曾误判用户意图短暂入库（616c430）后撤销（f054255），维持「仅本地保留」约定。
  **验证**：三脚本 pwsh Parser 语法零错误；clean-build-cache.ps1 -WhatIf 在主目录实测清理列表正确（6.35 GB）。


- **2026-09-14 账号体系增强：适配器指定账号登录 + 输入即自动建号 + 账号显示名可编辑（双端）** — 模块
  `tauri-app/src-tauri/src/{config/model.rs,config/persist.rs,commands/account.rs,auth/service.rs}`、
  `tauri-app/frontend/src/{account,network,hooks,settings}`、`android/src-tauri/src/{config_state.rs,account_cmds.rs}`、
  `android/frontend/src/...`（双端复刻同构、各改一份；**适配器指定账号为桌面专属**，安卓无网卡概念不复制）。
  ① **主/副适配器各自指定账号（桌面）**：`Config` 新增 `adapter1Account`/`adapter2Account`（存账号 id，空 = 跟随当前激活账号），
  网络面板适配器设置区各加「指定账号」下拉；`full_login` 为每个适配器构造一份 `Arc<Config>` 凭据副本（仅覆盖
  user/password/operator）穿过双适配器闭包（不改 `session.rs` 签名，保 `execute_dual` 的 `'static` 约束），手动指定网卡时
  按网卡名匹配主/副角色取账号；未指定或账号档案不可读时回退全局账号并 log_warn（行为与改动前完全一致）；这两个字段属
  设备级配置，切账号不覆盖。
  ② **输入即自动建号（双端）**：`save_config` 落盘后自动以用户名建号（id = 用户名 sanitize：非法字符替换为 `_`、截断 32 位），
  已存在且 user/password/operator 一致时不写盘（幂等短路），更新时保留用户自定义显示名，`user` 原文不同的撞库场景跳过并告警
  （防覆盖他人凭据）；桌面建号后补刷托盘菜单。
  ③ **账号显示名与内部 id 分离（双端）**：账号 id = 账号文件名 stem 且稳定不变，新增 `displayName`（可含空格/emoji，
  trim 后按 Unicode 码点 1..=32，为空时读取阶段兜底为 id）；新增 `rename_account(accountId, displayName)` 命令（重名拒绝、
  只改显示名、不动文件名与 activeAccount、被改名账号为当前激活账号时同步主配置并落盘）；`list_accounts` 返回由
  `string[]` 改为 `{id, displayName}[]`，双端前端类型贯通（含 `get_init_data`）。
  **验证**：桌面 `cargo test` 325 passed / 0 failed；桌面前端 `npx tsc --noEmit --incremental` 通过、`npx vitest run`
  14 files / 89 tests 全绿；安卓前端 `npx tsc --noEmit` 通过；安卓 `cargo check --target aarch64-linux-android --all-targets`
  零 error。两端结构均无 `deny_unknown_fields` + 容器级 `serde(default)`，故新字段向后兼容、**未升 schema 版本、无需迁移**。
  绑定运营商功能按需求**未改动**。


- **2026-09-14 夜间断网机制分析报告（授权测试，docs）** — 新增 `docs/security-analysis-nightly-cutoff.md`。
  基于协议逆向事实（登录无签名/token、注销按 IP 无凭据校验、mac/unbind 无鉴权、/Self 独立体系）与现场
  观察（非许可时段有线无 DHCP 响应、无线 AP 不发信号）判定断网主体为**接入层定时开关**（无线 AP 下电 +
  有线 D1 端口隔离 / D2 仅 DHCP 停服两候选），给出 6 条漏洞假设（静态 IP 旁路 / Portal 入口漏配 /
  IPv6-RA / 租约保持 / MAC 欺骗 / 无鉴权踢线 DoS）与按序授权实验清单 E0-E5；结论：绕断网方案存在与否
  取决于有线端口夜间转发状态（E1 静态 IP 实验为核心判据）。附带客户端工程建议（夜间禁触发 DHCP 释放自愈、
  时段拒绝 msg 独立 code）。


- **2026-09-14 检测页"检测逻辑"说明折叠为问号 + 修复定时登录/注销行窄屏截断（双端）** — 模块
  `tauri-app/frontend/src/monitor/MonitorPanel.tsx`、`android/frontend/src/monitor/MonitorPanel.tsx`（复刻同构，一次提交双端各一份）。
  ① 校园网验证 Label 旁新增 HelpCircle 问号按钮，检测逻辑四步说明收进 Tooltip（沿用 QualityPanel 指标问号
  同款模式），删除展开区常驻 5 行说明块，双端各省约一行 110px 纵向空间；② 每日定时登录/注销两行改窄屏
  （<640px）纵向堆叠（同"校园网检测时间段"行既有模式），修复安卓 380px 下描述被时间选择器挤成竖排截断；
  ③ 根因修复：Input 组件对 time 类型自带 relative w-full 包装层，直接作 flex 子项会吃满剩余空间使选择器悬中、
  并挤压左侧文字，外包 flex shrink-0 容器约束为内容宽（详见 .codewiki/learnings/input-time-wrapper-w-full）。
  **验证**：双端 `tsc --noEmit` 通过；vite + __TAURI_INTERNALS__ mock 浏览器实测三档——安卓 380×780（深色，
  描述完整两行、选择器独立成行、问号 Tooltip 四步说明完整弹出）、桌面 1280（选择器贴右、与退出时段行对齐）、
  桌面 620（窄窗堆叠正常），实测后 mock 已清理、git 工作区还原干净。


- **2026-09-14 旁路生效时跳过 bindProcessToNetwork,消除误导性"未绑定 WiFi"警告** — 模块
  `android/src-tauri/src/protocol_cmds.rs`。secure VPN 全量接管本应用时 bindProcessToNetwork 被
  netd EPERM 拒绝,每拍打"未绑定 WiFi"警告而实际传输由 SO_BINDTODEVICE 旁路接管完全可用
  (用户反馈:日志一直警告未绑定 WiFi 但实际可用)。旁路能力 Supported 时 ensure_wifi_bound
  直接跳过——传输出口已由内核设备级绑定决定,进程级 mark 绑定冗余,且省去每拍最多 3s 的
  requestNetwork 尝试。旁路不可用(能力封堵)时保持原绑定逻辑不变。


- **2026-09-14 新增根目录 clean-build-cache.ps1（B 档构建缓存清理，仅本地保留）** — 清理两处 cargo 构建缓存：
  `C:\cl-build\target`（桌面端，`tauri-app/build.ps1` 默认短路径 target，实测 2.45 GB）与
  `android\src-tauri\target`（安卓交叉编译缓存，实测 6.39 GB），合计约 8.8 GB。不碰源码、`release/` 发布资产、
  `changelogs/` 归档与 `node_modules/`；代价是下次构建全量重编译（桌面十几分钟起，安卓更久）。复用 PowerShell
  原生 `SupportsShouldProcess`（-WhatIf 预览、-Confirm 确认，ConfirmImpact=High）而非自造预览与确认逻辑；删除前
  检测 cargo/rustc/tauri 进程，避免删到一半。按既有要求**仅本地保留**（同 make-release.ps1 口径）：本地文件持续
  保留、加入 .gitignore，不入库。**验证**：-WhatIf 对真实缓存实测列出 2.45 GB、跳过不存在的安卓目录且未删除任何
  内容；沙箱副本（临时 4 MB 目录）实测三条路径——-WhatIf 不删、交互确认答 n 不删、关闭确认后两处目录（含相对
  路径拼接）均删除成功。

- **2026-09-14 旁路传输层改本地转发代理:修复手写 HTTP 被网关 nginx 400 的回归** — 模块
  `tauri-app/src-tauri/src/network/{bound_socket.rs,client.rs}`、`tauri-app/src-tauri/src/auth/{protocol.rs,portal.rs}`。
  真机验证发现旁路生效后(无 VPN 场景也)登录/探测被学校网关 nginx 400 Bad Request 拒绝。三轮对照实验定位:
  同一 URL 下 reqwest 200(返回 JSONP)而 hyper 手写请求 400,补齐 accept/cache-control/pragma 头、去掉
  SO_BINDTODEVICE 均无法消除——手写 wire 级差异无法穷举,放弃手写。终态:本地起单请求 HTTP 转发器(127.0.0.1
  随机端口,代理 absolute-form 请求行解析 → origin-form 重放,响应注入 connection: close),reqwest 以
  `Proxy::all` 指向它(`create_bypass_http_client`),登录/Radius 注销/MAC 解绑/Portal 页面探测的客户端整体
  切换——HTTP 层(头/重定向/charset)100% 复用 reqwest,仅传输层走旁路 socket;删除手写 hyper 通道及
  hyper/hyper-util/http-body-util 依赖。同时修复非阻塞 connect EINPROGRESS(115)不被 std 映射为 WouldBlock
  导致旁路连接全部误判回退的 bug(按 raw_os_error 判定,新增 android libc 依赖)。**验证**:真机代理启动
  (127.0.0.1:36507)、登录经旁路 200 + JSONP 与原路径一致、常驻通知恢复「监控运行中 · 在线」;桌面 cargo test
  全绿 289+1(新增代理目标解析纯函数测试)、安卓交叉编译 Finished。


- **2026-09-14 修复旁路 EINPROGRESS 误判回退 + 断开通知翻转提速(目标 1-2s)** — 模块
  `tauri-app/src-tauri/src/network/bound_socket.rs`、`android/src-tauri/src/{monitor_loop.rs,campus_detect.rs}`、
  `android/plugins/network-bind/.../NetworkBindPlugin.kt`。真机验证抓到关键缺陷:非阻塞 connect 返回
  EINPROGRESS(115),std 不把它映射为 WouldBlock,旁路每次连接被误判失败整体回退——修复为按 raw errno 判定
  (新增 android libc 直接依赖,已在 socket2 依赖树内),修复后真机确认旁路真实生效(登录请求物理网卡直达
  Portal)。断开→通知翻转三刀提速:WiFi lost 事件零延迟(连上方向保持 2500ms 等 DHCP)、网关/Portal TCP 探测
  并行 join!(串行最坏 6s→3s)、allNetworks 无 WiFi 时绑定立即返回(不等 requestNetwork 3s)。**验证**:桌面
  cargo test 全绿 290+1、安卓交叉编译 Finished、真机探针 Supported 且旁路连接无回退日志;教训入 wiki
  `learnings/nonblocking-connect-einprogress-not-wouldblock`。


- **2026-09-13 安卓 SO_BINDTODEVICE 物理网卡旁路:全量 VPN 下探测/登录不再被 tun 劫持** — 模块
  `tauri-app/src-tauri/src/network/bound_socket.rs`(新增)、`tauri-app/src-tauri/src/auth/{protocol.rs,portal.rs}`、
  `tauri-app/src-tauri/Cargo.toml`、`android/src-tauri/src/{campus_detect.rs,monitor_loop.rs}`。背景:secure 全量 VPN
  (Clash/UU 等,未排除本应用)下 socket 探测与登录全部被 tun 劫持(用户实测:开 Clash 不排除则无法登录)。Network
  API 层(bindProcessToNetwork/Network.bindSocket)在 secure VPN 下被 netd 以 EPERM 拒绝(ip rule 优先级
  13000<15000,AOSP 源码级核实),不可绕;但 Linux kernel ≥5.7(Android 12+)SO_BINDTODEVICE 新建 socket 首绑免
  CAP_NET_RAW 且不经 netd ip rule,可物理网卡直连(GrapheneOS#2381 实测、po4yka/RIPDPI 参考实现)。实现:能力探针
  (对 lo 试绑定,OnceLock 缓存;非 Supported 自动回退原通道)+ 物理网卡选择(排除 tun*/utun*/ppp*/rmnet*/ccmni*/lo/
  dummy*,wlan0 优先)+ `tcp_connect_bounded`(绑定层错自动回退、connect 层失败不回退防翻倍超时)+
  `http_get_bounded`(hyper http1 over 绑定 socket,GET-only、1MB 上限对齐 MAX_HTTP_BODY、同 authority 重定向≤5)
  接入 Portal TCP/HTTP 探测与登录/Radius 注销/MAC 解绑(Android 分支优先旁路,桌面 cfg 隔离零变化,GBK 解码与
  特征匹配解析层完全复用);monitor_loop 失败类通知在检测到 tun 接口时追加「请在 VPN 中排除本应用」提示。
  **验证**:桌面 `cargo test` 全绿 290+1(新增 8 个 bound_socket 纯函数测试);安卓 `cargo check --target
  aarch64-linux-android --all-targets` Finished;真机旁路生效性(探针 Supported 与否)待装包实测(logcat
  `[bind-dev]` 前缀)。调研与决策详见 CodeWiki `decisions/android-vpn-bypass-infeasible`。


- **2026-09-13 校园网检测时段默认改为 07:40–23:00（双端 + 存量迁移）** — 模块
  `tauri-app/frontend/src/settings/{constants.ts,types.ts}`、`android/frontend/src/settings/{constants.ts,types.ts}`、
  `tauri-app/src-tauri/src/config/{model.rs,validate.rs}`、`android/src-tauri/src/config_state.rs`、两端
  `frontend/src/monitor/MonitorPanel.tsx`。原默认 `campusCheckEndMinutes: 0`（end ≤ start 退化为“仅开始时间限制”，
  即 07:40 之后全天探测）改为 `1380`（23:00），夜间不再探测。存量配置走版本化迁移一次性刷新：安卓
  `config_schema_version` 4→5（`migrate_legacy_defaults` 新增段）、桌面 `config_version` 2→3（`validate_config` 内
  新增 v2→v3 段），只刷新“等于旧默认 0”的值，迁移落盘后用户改回不再二次覆盖；前端 `DEFAULT_CONFIG.configVersion`
  同步为 5（安卓）/ 3（桌面）。UI 空值兜底 `?? 0` 一并改为 `?? 1380`，避免字段缺失时显示 00:00 而后端按 23:00 判定。
  **验证**：桌面 `cargo test` 全绿、两端 `npx tsc --noEmit --incremental` rc=0、安卓
  `cargo check --target aarch64-linux-android --all-targets` Finished、浏览器实测（380×780 手机壳 mock）时间窗显示
  `07:40 – 23:00`。

- **2026-09-13 清理 slideDirection 死信号（双端）** — 模块 `tauri-app/frontend/src/{App.tsx,lib/animations.ts}`、
  `android/frontend/src/lib/animations.ts`。面板切换的 `slideDirection` 是"未写完的实现"：App.tsx 维护了
  state + 同步 effect + 两处 `custom` 传参，但 `createPanelAppleVariants` 返回静态 variants 对象、从不消费
  custom——方向值传了等于没传，无任何视觉效果。对照证据：`QualityPanel` 的 `tabItemVariants` 与
  `OnboardingWizard` 的 `slideVariants` 都是函数形式（initial/exit 接收 direction）并正确消费 custom，
  说明该机制在项目中是有效的既定模式，只是面板这处没写完。删除 slideDirection state/同步 effect/
  prevPanelRef（仅为此服务）及 AnimatePresence 与 m.div 两处 custom 传参，并删除因此失去调用方的
  `getPanelDirection` 与 `PANEL_ORDER`（安卓侧两函数原就无调用方）。纯死代码清理、行为不变；若后续要做
  方向性切换动画，需把 `createPanelAppleVariants` 改为函数形式变体。**验证**：双端
  `npx tsc --noEmit --incremental` rc=0、桌面 npm test 12 文件 77 用例全过、全仓 grep 无
  slideDirection/getPanelDirection/PANEL_ORDER 残留。

- **2026-09-13 Windows 桌面端 P2 审计落地（33 项，双端同步）** — 来源：双分身逐文件审计
  （后端 Rust / 前端 React）+ 主智能体逐条代码核实 + 一轮安卓同步适用性专项取证；计划文档
  `docs/superpowers/plans/2026-09-13-desktop-p2-improvements.md`（本地，工作区外不可见）。
  按主题落地：

  **① 共享 crate 后端优化（改桌面即双端受益，安卓经 path 依赖自动继承）** — `network/dns.rs`
  `resolver_get_or_create` 命中路径持锁仅轮转、未命中锁外构造后 double-check 写回（消除并发
  解析被 Resolver 构造串行化）；`network/client.rs` 客户端池命中由 `get_mut` 写锁改读锁 `get`
  （淘汰退化为按插入时间，TTL 600s/容量 32 不变）；`infra/logger.rs` `clear_logs` 由"删目录
  内所有文件"改为只删 `app-*.log`（新增 `is_app_log_file` 纯函数 + 正反例单测）；`auth/protocol.rs`
  新增 `read_bounded_body_async`，self_service 七处 `.text().await` 改走 1MB 限长读取；
  `infra/async_util.rs` `fallback_runtime` 去 `expect` panic 改 `log_error` + `flush_quick` +
  `exit(1)`（panic=abort 下保现场可诊断）。

  **② 桌面后端健壮性与降载** — DPAPI 失败路径补 `LocalFree`；`background_check` 的
  `Local::now()` 两次取时分改取一次（消除跨分钟 `now_min` 不一致）；`campus_check` 12 条日志
  分级（过程性降 debug、异常升 warn，原每轮十余条 info 刷屏）；`NetworkSnapshot` 的
  `current_ssid`/`last_network_quality` 改 `Option<Arc<str>>`（CAS clone 变引用计数）；
  `get_init_data` 复用适配器快照（去掉第二次全量 Vec clone）；更新检查 24h 等待由 5s 步进
  循环（17280 次唤醒/天）改单次 `tokio::select!`；质量恶化复核新增 lightweight 模式（只跑
  网关 + 1 个外网站点，原单次恶化触发 3 倍外网流量，"15s×2 复核才通知"语义保留）；
  `adapter_watch` 注册表遍历 15s → 60s（未选"仅变化时刷新"——禁用分类检测本身依赖该缓存）。

  **③ 前端性能与修复（双端各一份）** — `onAdapterDetailsChanged` 内容未变则不 setState；
  安卓移除 4 个 `noopListener` 死订阅（适配器是桌面专属概念）；`withTimeout` 在 finally
  清理输家定时器（原每次登录/注销悬挂 60s 定时器）；新增 `setStatusStable` 与 checkOnline
  各分支的比较后 set（status 引用稳定，StatusBar 不再无效重渲染）；`RefreshButton` ref
  无条件归位；`MascotVariant` 桌面补 `alert` 与安卓 7 变体对齐；`onResized` 加 100ms 尾节流
  （拖拽期高频 IPC 合并为一次）；`ConfirmDialog` 复位改由 `open` 翻转驱动（慢网络下按钮不再
  提前解禁，防重复提交；代价是长请求期间不可取消，属有意取舍）；`LogPanel` 行数选择器由
  自绘改 Radix Select（键盘可达）；`DockNav` tooltip 补 `aria-hidden`（按钮已有 aria-label
  且文案相同，补 role+describedby 会让读屏重复播报）；`ToastContainer` celebrate 娘图首帧
  一次性 spring 弹入（economy 档跳过）；面板标题/描述复用内容同款 variants 实现同向滑入。

  **④ 桌面专属前端** — `SponsorCard` 浮层锚点由硬编码 `right-[104px]`/`top-[52px]` 改测量
  定位，并以新增的 `data-sponsor-anchor` 数据属性建立稳定契约（替代起初的 aria-label 文本
  匹配——后者依赖 i18n 文案与 locale）；`FluidBackground` 实为纯色静态 div，补 2 枚 CSS-only
  慢速光斑（挂 `willChangeOrbs` 档位，economy 档不渲染；prefers-reduced-motion 已由
  resolveTier 归入 economy，无需另写 matchMedia）。

  **⑤ 实用功能** — 见下方各独立条目（DNS 一键还原、配置/诊断导出、质量落盘、定时登录注销、
  更新检查重试、托盘增强）。

  **验证**：桌面 `cargo test` 全绿（末轮 281 lib + 281 bin + 回归 1，自 262 基线上累计新增
  19 用例）；`cargo clippy --all-targets -- -D warnings` 全程无新增告警（存量 6 lint 不变）；
  双端 `npx tsc --noEmit --incremental` rc=0；桌面 `npm test` 12 文件 77 用例全过；安卓
  `cargo check --target aarch64-linux-android --all-targets` Finished（每轮均验证）；
  双端 i18n JSON 键集完全对齐。**审计修正三处（原描述与代码事实不符，已按代码落地）**：
  ① P2-5 原称"深拷贝整个 Config"——`ConfigStore::load()` 实为返回 `Arc<Config>`，原代码是
  引用计数，真实冗余是 `Arc::new(Arc<Config>)` 双重包装；② 安卓更新循环原称"与桌面同构的
  5s 步进轮询"——实为「5s 首查 + 直睡 24h」且无取消源，故该项仅改桌面；③ 面板动画原称
  "方向性平移 + slideDirection"——实为固定 y 位移且 `slideDirection` 未被 variants 消费
  （死信号，已记录备查），故标题改为复用内容同款 variants 实现真正同向。


- **2026-09-13 DNS 一键还原（reset_dns 命令 + 提权路径）** — 模块
  `tauri-app/src-tauri/src/commands/network_cmd.rs`、`app/startup.rs`、`helper/mod.rs`；
  前端 `tauri-app/frontend/src/{network/NetworkPanel.tsx,hooks/tauriApi.ts,i18n/locales/{zh,en}.json}`、
  `android/frontend/src/hooks/tauriApi.ts`。背景：`platform/dns_config.rs` 的
  `clear_adapter_dns_via_api`（恢复 DHCP 自动获取）早已实现，但全仓仅 `network/dns_setup.rs`
  在 WiFi 设置流程内部调用——命令面无暴露，用户一键设置 DNS/DoH 后只能手改系统网络设置。
  ①新增 `reset_dns` 命令（沿用 `resolve_adapter_names` + `filter_operation_adapters` + 黑名单
  过滤的主/副适配器操作范围约定），逐适配器清除后按全成功/部分失败/全失败三态汇总；
  ②helper 新增 `clear_dns` op（`HelperOp::ClearDns` + `run_clear_dns`，逐适配器明细写入
  `details.restored`/`details.failed`，与 `dns` op 结果文件格式同构）；③**提权路径与
  `setup_dns_doh` 对等**——管理员直接执行，非管理员经 `spawn_elevated_helper` 提权（30s
  超时同款），UAC 取消给出「需要管理员权限」明确提示而非静默失败。**此缺口是排查中发现的**：
  `SetInterfaceDnsSettings` 需管理员权限，若不走提权则非管理员下一键还原必失败（功能只做
  一半）；④前端双端 `tauriApi` 加 `resetDns`（安卓 `desktopOnly`，平台专属例外），
  NetworkPanel DNS 卡新增「恢复自动获取」按钮（成功 toast + 复用 `checkDnsDohStatus` 刷新），
  i18n zh/en 各 4 键。**验证**：`cargo test` 全绿（265 lib + 265 bin + 回归 1，含新增 helper
  `parse_clear_dns_with_targets` 用例）、clippy 无新增告警、双端 tsc rc=0、zh/en JSON 键集
  无单侧独有键。


- **2026-09-13 每日定时登录 / 定时注销（双端）** — 新增跨平台纯函数
  `tauri-app/src-tauri/src/config/schedule.rs::should_fire_scheduled_action`（`config/mod.rs`
  接入，无 cfg 门控，双端共用同一实现与同一组测试）：`target=0` 禁用；`now < target` 不触发；
  `now >= target` 且 `last_fired_day != today_day` 触发。**"过点补触发"是有意设计**——循环拍
  间隔与系统休眠会错过精确分钟，等值比较会让功能整天静默失效。配置字段
  `scheduledLoginMinutes`/`scheduledLogoutMinutes`（u16 分钟制，默认 0=禁用，validate clamp
  0..=1439，双端同提交；旧配置缺字段经容器级 serde default 自动落 0，无需 schema 迁移）。
  桌面新增 `monitor/scheduled.rs`（30s 一拍，watcher 经 task_manager 拉起跟踪），命中调
  `auth::service::full_login`/`full_logout`（复用入口，零协议副本），登录成功走
  `post_login_handler` 同款编排、注销成功对齐全量注销后处理（取消自动退出、重置重连计数、
  60s 注销保护期——防止定时注销后被自动准备登录立即登回）；当日标记在
  `AppState.scheduled`（`ScheduledFired` 两个 AtomicI32）。安卓 `monitor_loop.rs::run_scheduled_actions`
  每拍判定，**置于巡检分档跳拍与校园网检测静默期之前**——定时动作常配置在静默期/灭屏闲时段
  （如 23:30 注销），放进 `run_check_once` 会被静默期整拍吞掉。**互不干扰保证**：判定只依赖
  配置目标值 + 自身当日标记（不读写冷却/重连计数）；标记独立（跨天由 `num_days_from_ce`
  变化自然重置）；动作经 `is_logging_in`/`is_logging_out` 与手动登录、托盘、掉线重连同锁
  互斥（抢不到则本拍跳过、下拍过点补触发兜底）。UI 双端在监控页「校园网检测时间段」行后加
  「每日定时登录 / 每日定时注销」两行（单 time 输入，风格一致），i18n 四文件各 4 键。
  **验证**：cargo test 全绿（279 lib + 回归 1，基线 270 + 新增 9，含 schedule.rs 5 个边界
  用例）、clippy 无新增、双端 tsc rc=0、npm test 77 通过、安卓交叉编译 Finished、i18n
  桌面 749/749 与安卓 779/779 键对齐。


- **2026-09-13 质量检测结果落盘（双端）** — 背景：`NetworkSnapshot.last_network_quality`
  仅存内存，质量结果只 emit 不持久化——无历史趋势、重启即失、恶化事件无法回溯。桌面
  `config/persist.rs` 新增 `append_quality_history`（内核拆出收 `&Path` 版本以便单测免
  AppHandle），完整复刻 `login_history` 范式：独立 `QUALITY_HISTORY_LOCK` 全程互斥、头插、
  上限 100 条滚动截断、损坏文件 `.bak` 备份、`atomic_write`（tmp+rename+sync_all）；落盘点
  两处（`monitor/quality_scheduler.rs` 定时循环含复核轮一处覆盖、`commands/network_cmd.rs`
  手动检测），disabled/busy/unknown 占位结果不落盘。安卓新增 `src/quality_history.rs`
  （同构实现，附 4 测试），落盘点在 `quality_cmds.rs::run_quality_once`——安卓质量检测
  （命令 / latency_loop / monitor_loop 启动首检）已全部汇聚于此单点，故 monitor_loop 无需
  改动。记录形状双端一致：`{"timestamp": unix_ms, "gatewayLatency": number|null,
  "externalLatency": number|null, "quality": string}`，头插最新在前；延迟源值 -1（失败标记）
  统一落为 null 以区分"未测"与 0ms。本任务不含前端图表 UI。**验证**：cargo test 全绿
  （270 lib + bin + 回归 1，含新增 3 用例）、clippy 无新增、安卓交叉编译 Finished。


- **2026-09-13 托盘菜单新增快速注销与切换账号（桌面专属）** — 模块
  `tauri-app/src-tauri/src/app/tray.rs`、`commands/{account,login,config_cmd}.rs`。原托盘
  仅「显示主窗口 / 快速登录 / 退出」，双账号用户与常用「立即注销」场景都须开窗口操作。
  ①新增「快速注销」与「切换账号」子菜单（账号列表取自 `config::persist::list_account_names`，
  项 id 前缀 `switch-account:` 可按名解析，账号名经 validate 校验不含冒号）；菜单构建抽为
  `build_tray_menu` 供构建与刷新共用；`refresh_tray_menu_state` 内部经 `spawn_blocking` 重建
  并 `set_menu`（tauri 2.11 的 `set_menu` 走主线程同步等待，主线程直调会死锁，故统一异步
  派发）；②状态启用/禁用：「快速注销」以 `config.user` 非空为判据（后端无可靠的"会话已登录"
  标志——`any_adapter_online`/`has_logged_online` 是网络检测推论且存在时序盲区；`user` 非空
  恰是 `full_logout` 的硬前置，保证启用的项必然可执行），「切换账号」账号列表非空才启用、
  当前账号标「（当前）」并禁用；③刷新时机挂 `save_config_to_disk_encrypted` 尾部（覆盖
  保存配置/切换账号/保存账号等所有落盘路径）+ `delete_account` 尾部；④**抽取共享函数避免
  复制**：`commands/login.rs` 抽出 `pub(crate) perform_full_logout_sync`（原 `do_logout` 的
  拿锁 → `full_logout` → 1s 在线复检 → 全量/单适配器状态重置 + 60s 注销保护期，逐行等价，
  `do_logout` 瘦身为 spawn_blocking 包装）、`commands/account.rs` 抽出
  `pub(crate) perform_switch_account_sync`；⑤反馈走既有 `emit_notification`；文案与既有托盘
  一致为硬编码中文，不为此引入 i18n。**验证**：cargo test 全绿（279 lib + 回归 1）、clippy
  无新增（存量 6 lint 均在未触碰文件）、改动均为 `#[cfg(desktop)]` 限定安卓不编译。



- **2026-09-13 校园网检测开始时间升级为检测时间段（双端）** — 原"校园网检测开始时间"单边
  门控升级为可配置时间段：新增 `campusCheckEndMinutes` 配置字段（u16，默认 0=不限制，
  旧配置缺字段自动兼容），UI 在监控页"校园网检测时间段"行内加结束时间输入框（与"退出
  生效时段"双输入框同布局），文案由"开始时间"改为"时间段"。判定语义与
  `campus_exit` 生效时段一致：`start=0` 禁用门控；`end=0` 或 `end<=start` 退化为仅受开始
  时间限制（防误设整天静默失效），生效窗口 `[start, end)` 内才执行校园网检测。桌面
  判定收敛为共享函数 `monitor::campus_check::is_campus_check_silent`（`background_check`
  与 `auto_auth` 两处单边判断原来重复，统一走一处），静默期日志/UI message 同步显示
  检测时段；安卓端 `config_state.rs` Settings 加同名字段（camelCase 契约 + Default 0）、
  `monitor_loop.rs` 静默期门控同步加结束时间判断。**验证**：桌面 `cargo test` 全绿
  （262 lib 用例含新增 4 个：静默期判定单边/双边/退化 3 个 + end clamp 1 个）、双端
  `npx tsc --noEmit` 通过、安卓 `cargo check --target aarch64-linux-android --all-targets`
  Finished；浏览器 mock 实测监控页新增行渲染正确（截图确认双输入框 07:40 – 00:00
  与退出生效时段布局一致）。

- **2026-09-13 安卓前台服务锁收敛到探针窗口按需持有（后台锁省电）** — 模块
  `android/plugins/foreground-service/.../ForegroundService.kt`、`MonitorServicePlugin.kt`、
  `plugins/foreground-service/src/lib.rs`。①`onCreate` 常驻的 `WIFI_MODE_FULL_HIGH_PERF`
  WifiLock（campus:wifi，创建到销毁全程持有；CDD [C-3-1] 要求持有期系统不得进入 WiFi 省电）
  改为 Rust 巡检每拍经插件命令 `beginProbeWindow`/`endProbeWindow` 在探针窗口内按需持有
  （窗口内 WifiLock + `campus:probe` PARTIAL_WAKE_LOCK(30s timeout 兜底)，窗口外全释放；
  `run_check_once` 以 drop guard 保证任何 early return/panic 必释放）；②nudge 回调按
  "关注字段翻转"（TRANSPORT_WIFI/CELLULAR + VALIDATED 三 bit 位掩码）去重，5s 节流降为
  第二道闸（与 network-bind 插件 watcher 同款去重）。为什么——常驻 WifiLock 让系统永不进入
  WiFi 省电，而一轮探针只数百毫秒。**验证**：真机 25060RK16C 实测 `dumpsys power`
  `Wake Locks: size=0`（应用运行中零持锁）；`campus:probe` 单次持锁 110ms~1.1s，
  `campus:wifi` 常驻条目消失；nudge 仅在网络事件时短持 3s；出包全链路（Kotlin +
  aarch64 Rust）编译通过。commit fac6407 已合并 main。


- **2026-09-13 安卓后台巡检分档省电：蜂窝/灭屏时段拉长巡检间隔 + schema v3→v4** — 模块
  `android/src-tauri/src/{monitor_loop.rs,config_state.rs}`、`plugins/foreground-service`、
  `android/frontend/src/settings/{types.ts,constants.ts}`。①新增配置
  `backgroundCheckIdleInterval`（默认 300000=5min，后端 Settings::default 真源 + 前端双源
  同步，顺带把前端 `configVersion: 2` 对齐到后端 schema 版本 4）；②巡检分档：唤醒周期恒为
  基础间隔（60s），每拍经新插件命令 `getPowerState`（`PowerManager.isInteractive` +
  ConnectivityManager 活动网络 TRANSPORT_WIFI）判定，WiFi 且亮屏走基础间隔，蜂窝或灭屏走
  闲时间隔跳拍（纯函数 `effective_interval_ms` 单测锁定，3 个测试 7 组断言）；③
  `migrate_legacy_defaults` v3→v4 存量配置补默认值。为什么——非 WiFi 环境探针必失败、
  灭屏时用户不在看状态，60s 一拍的整轮 Portal 探测是无效功耗；亮屏/回 WiFi 后最迟一拍
  （60s）恢复，无需事件通道。**验证**：真机实测——亮屏期 probe 每 60s 一拍；熄屏后跳闲时档
  （实测相邻探测 336~360s，含 doze 定时器聚合延迟）；亮屏后最迟一拍恢复（12:10:38 立即
  探测）；`cargo check --target aarch64-linux-android --all-targets` Finished、tsc rc=0。
  commit 07d7bf5 已合并 main。


- **2026-09-13 安卓通知大图补 drawable 资源，消除 large_icon 必然失败的降级重建** — 模块
  `android/src-tauri/gen/android/app/src/main/res/drawable-xxhdpi/`。新增 `mascot_alert.png`
  （341×512）与 `mascot_offline.png`（512×512），由 `android/frontend/public/girl/` 同名
  webp 经 PIL 缩放转出（长边 ≤512、保留透明通道）；`monitor_loop.rs` 代码不变。为什么——
  gen/android 的 res 原本无任何 mascot 图，`notify_system` 的 `large_icon(mascot)` 每次抛
  `Resources.NotFoundException` 后走降级分支重建纯文本通知（每次关键通知多一次跨层调用 +
  一次无效 Builder 构造）；补资源后掉线（mascot_alert）/重连失败·达上限（mascot_offline）
  通知真正带看板娘大图。**验证**：PIL 转换尺寸/字节数核对、PNG 目检正常；资源名与代码传参
  （`mascot_alert`/`mascot_offline`）一致；带图通知待下次真实掉线时目验。commit 7dec464
  已合并 main。


- **2026-09-13 安卓电池优化白名单一键引导 + 厂商自启页跳转（保活设置区块）** — 模块
  `android/plugins/foreground-service`（3 新命令）、`android/src-tauri/src/{battery_cmds.rs,lib.rs}`、
  `AndroidManifest.xml`、`android/frontend/src/settings/{KeepAliveSettingsCard.tsx,SettingsPanel.tsx}`、
  `hooks/tauriApi.ts`、i18n zh/en。①插件新增 `getBatteryOptimizationInfo`（ignoring/brand/
  manufacturer/hasVendorTarget，Build.BRAND 小写同源判定）、`requestIgnoreBatteryOptimizations`
  （系统确认框，非静默加入）、`openVendorBatterySettings`（厂商自启/省电页候选表逐个
  try/catch 降级：xiaomi/redmi/huawei/honor/oppo/realme/oneplus/vivo/iqoo/samsung 组件候选 +
  魅族 SHOW_APPSEC action → 应用详情页 → 系统设置；不用 resolveActivity 预探测——Android 11+
  包可见性会让未声明 <queries> 的厂商包误判不存在）；②manifest 增
  `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` 权限；③新建 `battery_cmds.rs` Rust 命令面（3 个
  #[tauri::command]，桌面端返回明确错误）；④前端设置页新增「保活设置」区块（白名单状态徽章 +
  一键申请先弹自家说明框 + 厂商跳转，首次进入仅自动提示一次，isAndroid 门控）+ zh/en 各 16 个
  双语键。为什么——部分厂商 ROM 息屏后限制后台检测导致掉线无法自动重登，需引导用户加白名单；
  权限敏感故 UI 先确认再跳系统页。**验证**：cargo check（aarch64）Finished、tsc rc=0、vite
  build 无新告警、i18n 键 16/16 一致；真机 25060RK16C（MIUI/HyperOS）端到端走查通过——首次
  说明弹窗正常、系统授权后 `dumpsys deviceidle whitelist` 确认 `user,com.campuslogin.client`
  生效、设置页徽章同步显示"已允许"、厂商跳转精准命中
  `com.miui.securitycenter/...AutoStartManagementActivity`。commits 6cbee74/f4e643d/7efba97
  已合并 main。


- **2026-09-12 新增根目录 make-release.ps1（编译后一键汇总发布资产）** — 包装
  `tauri-app/build.ps1`（SkipFrontend/TargetDir 透传，`-SkipBuild` 复用已有产物），补齐
  三件事：① 安装包统一改名为 updater 硬编码的 `Wxxy-CampusLogin_{ver}_x64-setup.exe`
  （productName 中文名产物不再需要手动改名）；② 生成 shasum 兼容 `.sha256`；③ APK 与
  `RELEASE_NOTES_v{ver}.md` 一并收进 `release/`（.gitignore 第 7 行已有），结尾打印
  `gh release upload` 命令。CODE_WIKI 附录 H 检查清单补第 0 条。**验证**：
  `-SkipBuild` 对既有 2.3.6 产物一次跑通，改名/校验/收集/提示全部正确，生成的
  .sha256 与已上传 Release 资产哈希逐字节一致。（更正：同日按用户要求改为仅本地保留——3894c12 一度入库，e3734bd 已从仓库移除并加入 .gitignore，CODE_WIKI 第 0 条改为本机本地脚本口径；本地文件持续保留。）


- **2026-09-12 版本号全量升级 v2.3.6（发布准备）** — 附录 H 清单 13 处：双端
  `tauri.conf.json`/`Cargo.toml`、双端 `package.json`+`package-lock.json`（仅项目条目
  第 3/9 行）、`ui-constants.ts`、`about-preview.html`（v 前缀×2）、README 徽章、
  `CODE_WIKI.md` 顶部+元信息、`version.json`（v 前缀）；桌面 `Cargo.lock` 由 cargo 自动
  重写。**验证**：`cargo test` 全绿（258 bin + 234 lib + 1 repro）、双端
  `npx tsc --noEmit --incremental` rc=0。

- **2026-09-12 修复 `clear_client_pool` 桌面 bin dead_code 告警** — 模块
  `tauri-app/src-tauri/src/network/client.rs`。该函数仅安卓端经 `campus_login_lib`
  路径依赖调用（`android/src-tauri/src/protocol_cmds.rs:235`，网络路由变化后必须
  清池避免复用旧路由连接）；桌面无多网络路由切换场景故不调用，加
  `#[allow(dead_code)]` 并注释调用方归属。**验证**：`cargo build` 零 warning，
  `cargo test` 全绿。

- **2026-09-12 娘图格式盘点结论（无改动）**：双端 `public/girl/` 立绘全部 WebP，
  单张 68–384KB（最大 mascot-busy），已压缩到位；非娘图（赞助码 png/jpg、favicon
  png）不在盘点范围，维持原状。

- **2026-09-12 非校园网自动退出默认值核查结论（无改动）**：`campusExitOnFail` 自引入
  起各层默认即开启——后端 serde `default_true` + `Default::default()` true、前端
  constants true + 展示回退 `?? true`；v2.3.5 发布版与本机实际配置
  （`%APPDATA%/com.campus.login/config.json`）均为 `true`，无需代码改动。


- **2026-09-12 安卓端看板娘扩面展示（平板侧边娘/底部娘每日轮换/错误态娘图）** — 模块
  `android/frontend/src/shared/{dailyMascot.ts(新增),MascotFigure.tsx,ErrorBoundary.tsx}`、
  `android/frontend/src/components/tablet/TabletShell.tsx`、
  `android/frontend/src/{auth/DashboardPanel,monitor/QualityPanel,settings/SettingsPanel}.tsx`。
  - **平板侧边娘**：TabletShell 补两个 fixed 侧边娘（此前平板一张娘都没有），复用桌面同款
    视觉（低透明度、避让右栏 304px）；断点按平板实际宽度适配——左侧 ≥900px（720 主区+
    128 立绘+边距）、右侧 ≥1330px（需再让过 288px 日志栏），w-32。
  - **手机底部娘每日轮换**：总览/网络质量/设置三面板滚动末尾的固定水印娘（bg-laptop/
    bg-lounge/bg-nap）换为 14 张 side-* 竖版立绘按日轮换，offset 0/1/2 错开保证同一天
    三个面板三张不同的图。
  - **错误态娘图**：ErrorBoundary fallback 的 AlertTriangle 图标换为 `mascot-alert`
    惊慌娘（`MascotVariant` 扩 `alert`）。
  - **展示性能治理（GitHub 调研结论落地）**：装饰娘一律懒加载 + 静态低透明度展示，
    不引入动画引擎——Live2D 在 Android WebView 实测内存数百 MB 且有 Cubism 许可约束
    （可扩展性应用需事先取得批准、live2d-widget 为 GPL-3.0），Lottie 无成体系二次元
    素材且主线程开销高于 CSS；解码内存 = 宽×高×4（800×1200 单张 3.84MB），同屏
    合成层控制在 3-4 个以内。来源：live2d_api（轮换范式）、ark-waifu（省电默认参数）、
    AIRI DevLog 2026.03.23（WebView Live2D 内存实测）、Live2D 许可条款。
  - **验证**：`npx tsc --noEmit` 通过；轮换逻辑性质经 Node 实测（182 天循环、
    offset 错开无重复）。平板/手机实际观感待真机目检。

- **2026-09-12 Windows 侧边看板娘每日随机（图池扩至 14 张）** — 模块
  `tauri-app/frontend/src/App.tsx`、`assets/ui-girl/`、双端 `public/girl/`。
  - **图池**：新增 12 张竖版全身立绘（side-phone/mug/music/heart/stretch/book/game/plant/
    victory/snack/umbrella/think，1024×1536 参考既有角色生成），rembg 抠图后 800×1200 入
    `assets/ui-girl/`（原图归档 `original/`），WebP 入双端 `public/girl/`，连同既有
    side-wave/side-laptop 共 14 张。
  - **每日切换**：`pickDailySideMascots` 按本地日期（epoch 天数）从 14×13=182 种有序组合中
    轮转取第 k 组——连续 182 天不重样后循环；同日固定（刷新不换）、左右不重复；常驻托盘
    场景每分钟比对日期，跨天即时切换。365 天实测恰覆盖 182 种组合、零左右重复。
  - **验证**：`npx tsc --noEmit` 通过；组合性质经 Node 脚本实测（见上）。

- **2026-09-12 安卓 WiFi 变化即时触发网络状态检测（事件驱动，不再等下一拍轮询）** — 模块
  `android/plugins/network-bind/{src/lib.rs,android/.../NetworkBindPlugin.kt}`、
  `android/src-tauri/src/monitor_loop.rs`。
  - **背景**：WiFi 连上校园网后（含息屏后断网重连场景）要等后台检测下一拍（60s 级）
    才执行校园网判定与自动重登；改由系统网络事件即时触发，检测时延从分钟级降到秒级。
  - **机制**：Kotlin `NetworkBindPlugin` 新增 `startWifiWatcher`/`stopWifiWatcher`
    （`ConnectivityManager.registerNetworkCallback`，`TRANSPORT_WIFI`，不取 SSID 免定位
    权限）：`onAvailable`/`onLost` 上报连上/断开，`onCapabilitiesChanged` 仅在
    `NET_CAPABILITY_VALIDATED` 翻转（captive portal 认证前后）时上报（其余高频回调吞掉）；
    事件经 Tauri 插件基类 `registerListener`/`trigger` 的官方 Channel 通道直达 Rust
    （不经 JS，WebView 冻结场景可靠）。Rust 侧 1s去抖合并事件风暴 + 2.5s 延迟
    （等 DHCP/路由就绪，避免探测必失败）后执行完整 `run_check_once`（含自动重登）。
  - **生命周期**：监听随后台检测开关起停（start/stop_background_check 同步注册/注销），
    尊重用户关闭后台检测的意图；监听启动失败只记日志，退化为纯周期检测。
  - **验证**：`cargo check --target aarch64-linux-android --all-targets` 通过（注入 NDK
    27.0 工具链）；`build-apk.ps1 -SkipFrontend` 出包通过（APK 已产出）。去抖判定为
    纯函数并带单测（安卓 crate 无 host 测试环境——desktop 模式 ACL 缺 biometric 权限
    编译不过，既有约定，测试代码随 `--all-targets` 完成编译检查）。**真机实测通过**
    （Redmi 25060RK16C / HyperOS 3.0，2026-09-12）：手动切换 WiFi（16:06:24-36）期间
    在 60s 拍点外出现 4 轮即时检测（16:06:24.8 切换中、16:06:26.6×2 重连后、16:06:36.6
    认证后），时序与 lost/available/validated 三类事件逐一对应，事件驱动链路端到端
    生效；其余时段仅正常 60s 拍点检测。注：log_info 仅写日志文件，logcat 只见
    eprintln（wifi-bind 等），监听启动/事件日志在应用内日志页查看。


- **2026-09-12 非校园网自动退出生效时段设置（Windows 桌面端）** — 模块
  `tauri-app/src-tauri/src/{config/model.rs,config/validate.rs,infra/lifecycle.rs,network/adapter.rs}`、
  `tauri-app/frontend/src/{settings/{types,constants}.ts,monitor/MonitorPanel.tsx,i18n/locales/{zh,en}.json}`、`CODE_WIKI.md`。
  - **需求**：非校园网自动退出（campusExitOnFail）增加生效时间段设置，默认 8:00–23:00——深夜/清晨不因离开校园网被强制退出应用。
  - **实现**：Config 新增 `campusExitStartMinutes`(默认480=8:00)/`campusExitEndMinutes`(默认1380=23:00)（serde default + validate clamp 0..=1439，旧配置自动补默认）；窗口判定收敛在 `start_campus_exit` 入口（新增 `is_within_campus_exit_window` 纯函数 + 2 个单测，后台巡检与开机自启两条触发链一处生效）；终点≤起点时退化为仅按起点限制（防误设导致功能静默失效）。UI 在监控面板"非校园网自动退出"开关下新增"退出生效时段"行（两个 time 输入框），i18n 中英同步；检测行为与既有"校园网检测开始时间"静默期设置不受影响。
  - **验证**：cargo test 全绿（lib 258 + bin 同套 258 + 回归 1，含新增 2 用例）；`npx tsc --noEmit --incremental` 通过；vitest 77 全过；__TAURI_INTERNALS__ mock + vite dev 浏览器实测：新行布局正常无溢出、默认 08:00–23:00 回显、修改为 07:30 即时生效且终点不受影响；clippy 无新增告警（本机存量 6 lint 均在未触碰文件）；验证用 index.html mock 注入已完整还原（git diff 干净）。


- **2026-09-12 全端通知对齐看板娘（新增 alert/busy/update 三个娘变体）** — 模块
  `assets/ui-girl/`、双端 `frontend/src/shared/{ToastContainer,ui-types}`、双端 `useAuthStore`、
  `tauri-app/src/App.tsx`、`tauri-app/src-tauri/src/{platform/toast,infra/notification,infra/lifecycle,
  monitor/{auto_auth,background_emit,latency},update/updater}.rs`、`tauri.conf.json`、
  `android/src-tauri/src/{monitor_loop,update_cmds}.rs`、`android/plugins/foreground-service/.../ForegroundService.kt`。
  - **背景**：安卓系统通知全无娘图（FGS 用系统同步图标、业务通知兜底 ic_dialog_info）；
    Windows 侧"带娘的通知"其实全是应用内 toast 且仅登录场景 3 处带图，系统通知（自组
    toast 与插件通知）零图片——插件（notify-rust）在 Windows 不暴露图片参数，通知上带不出娘。
  - **应用内 toast 统一娘头像**：ToastContainer 不再按 mascot 参数分叉渲染 lucide 图标，
    改为全部 toast 渲染娘头像，按 type 取默认变体（info→portrait/success→celebrate/
    error→offline/warning→alert），显式第 5 参可覆盖；新版本提示 toast 传 update。
    useAuthStore 三处显式 celebrate/offline 参数改为默认映射后删除。
  - **桌面系统通知带娘**：`emit_notification` 增加 mascot 参数并改走自组 WinRT toast
    （`platform/toast::show_system_toast`，appLogoOverride 圆形头像，AUMID 沿用 PowerShell；
    失败降级插件纯文本通知）；13 个调用点按场景选变体（登录成功/网络恢复→celebrate、
    断线/非校园网→alert、重连失败/仍断线→offline、拥堵→busy、其余→portrait）。
    娘头像 PNG（128²，共 5 张 + 更新专用 256²）经 `bundle.resources` 打包，
    运行时 `resource_dir()` 解析为 `file:///` URL。
  - **安卓通知带娘**：`monitor_loop::notify_system` 增加 mascot 参数，走插件
    `large_icon`（drawable 资源名，`getIdentifier` 解析；解析失败自动降级无图纯文本），
    掉线→mascot_alert、登录/重连失败→mascot_offline、新版本→mascot_update；
    FGS 常驻通知 `setLargeIcon(mascot_portrait)`（256² PNG 放自研插件模块
    `res/drawable/`，避免 gen/ 重生成丢失）。
  - **新图入库**：`mascot-alert/busy/update` 三张 AI 生图（1024×1536，参考
    `mascot-celebrate.png` 保角色一致），update 经 rembg(bria-rmbg-2.0) 抠图，统一缩放
    800×1200 入 `assets/ui-girl/`（原图归档 `original/`），WebP(q90) 入双端 `public/girl/`。
  - **验证**：双端 `npx tsc --noEmit` 通过；`cargo test` 256+1 全绿；`cargo check
    --target aarch64-linux-android`（双 target）通过；`build-apk.ps1` release 全量出包通过。
    系统通知实际效果（Windows 通知中心 appLogoOverride、安卓 largeIcon）待真机目检。

- **2026-09-12 人脸验证体验迭代（基于真机首轮实测反馈）** — 模块
  \`android/frontend/src/face/{faceService,FaceCaptureDialog}.tsx\`、\`i18n/locales/{zh,en}.json\`。
  - **转头挑战接受任一侧**：原实现按提示方向匹配（facing left/right），但前置摄像头
    镜像语义让用户第一次容易转反（首轮实测 5.8s 的主因）——转头方向本身不是安全边界
    （动作完成即活体信号），改为任一侧均可；挑战集合由眨眼/左转/右转收敛为眨眼/转头，
    文案合并为「请向左或向右转头」。
  - **未检出人脸实时提示**：faceService 的 onFacePresence 回调（验证链原有、录入链新增）
    接入 UI——弹窗下方在未检出人脸时显示「未检测到人脸，请正对屏幕」，消除"傻等超时"。
  - **验证/录入成功反馈**：通过后显示绿色 ✓「验证通过」0.6s 再关闭弹窗，闭环可感知。
  - **验证**：\`npx tsc --noEmit\`、\`build-apk.ps1\` release 全量构建通过；真机（Redmi
    25060RK16C / HyperOS 3.0）体验确认三项改进全部生效。照片攻击实测未完成（用户手边
    无照片；动作挑战机理上挡静态照片），列为后续验证项。
  - **附带核查**：「网络状态检测」（enableBackgroundCheck）双端默认值与真机落盘配置
    均已是 true（含迁移强制拉回），无改动。


- **2026-09-12 安卓新增 2D 人脸验证（可选低安全回退，@vladmandic/human）** — 模块
  `android/frontend/src/face/*`（新增）、`hooks/tauriApi.ts`、`account/selfServiceState.ts`、
  `settings/SettingsPanel.tsx`、`settings/{types,constants}.ts`、`App.tsx`、
  `android/src-tauri/src/config_state.rs`、`tauri.conf.json`、`gen/android .../AndroidManifest.xml`。
  - **背景（GitHub 调研）**：应用内人脸验证仅有两条现实路径——系统人脸不可达（Class 1
    对三方封闭：ColorOS 上报强度 4096/4095、HyperOS Android 14+ 封反射、荣耀官方明确
    不开放、华为仅 3D 白名单机型走 HMS、HarmonyOS NEXT 不兼容 APK）；支付宝式刷脸为
    自采相机 + 本地/云端比对。用户选择应用内 2D 人脸（接受低于指纹的安全等级），要求
    告知风险并提供开关。
  - **选型（许可全核）**：`@vladmandic/human` 3.3.6（代码 MIT；blazeface/facemesh/faceres
    模型均 Apache-2.0，内置眨眼/转头手势与 match.similarity）。**排除** insightface（权重
    non-commercial）、facefusion（OpenRAIL-AS + 依赖 NC 权重）、kby-ai（无 license）；
    **不启用** human 包内 antispoof/liveness 模型（来源许可不明）。许可与用途声明记入
    THIRD-PARTY-NOTICES.md（BlazeFace 模型卡的 identity recognition out-of-scope 与
    本应用"本人自愿 1:1、非监控"场景的适配说明）。
  - **实现**：① `face/faceService.ts`：human 单例（webgl，懒加载 + warmup）、getUserMedia
    前置相机、录入（质量门控采 8 帧描述子均值）、验证（随机动作挑战眨眼/左转/右转
    10s 窗口 → 通过帧比对 similarity≥0.55，3 次机会）；眨眼用 human 同款 mesh 比率
    判据 + 帧间状态机（瞬态动作不丢帧）。② 模板（1024 维均值）存本机 safeStorage，
    后端/网络零传输；开关 `allow2dFaceVerify` 落后端 Settings（serde default false，
    旧配置自动回落）。③ 门集成在 `verifyWindowsIdentity` 单点：开关开 + 已录入 + 系统
    生物不可用（plugin-biometric checkStatus）→ 人脸验证，其余路径不变；失败码
    （faceTimeout/faceChallenge/faceMismatch/faceCamera/userCancel）并入
    `biometricFailMessage` 中文文案。④ UI：FaceCaptureDialog 单例（App 根部，双外壳共用，
    faceVerifyStore 命令式桥接）；设置页安全卡新增开关 + 风险确认 ConfirmDialog +
    录入/重录按钮 + 已录入状态；关闭开关即清模板（数据最小化）。⑤ 配置：CSP 增加
    `script-src 'self' 'wasm-unsafe-eval'`、`img-src/worker-src blob:`；Manifest 增加
    CAMERA 权限——wry 0.55.1 Kotlin 层已内置 onPermissionRequest 的运行时权限弹窗链路
    （VIDEO_CAPTURE → CAMERA → grant/deny），无需自写插件。APK 增量约 11MB（模型 8.9MB）。
  - **验证**：`npx tsc --noEmit --incremental` 通过；`build-apk.ps1` 全量构建通过。
    **真机功能（相机授权弹窗、录入、验证、阈值标定）待装机实测**——相似度阈值 0.55 为
    human 默认注释值从严起步，需按实测 FAR/FRR 调整。


- **2026-09-12 安卓生物验证失败提示本地化，设置文案说明锁屏密码兜底** — 模块
  `android/frontend/src/account/selfServiceState.ts`、`settings/SettingsPanel.tsx`、
  `i18n/locales/{zh,en}.json`。
  - **背景（GitHub 调研）**：androidx.biometric 的 Class 1（便捷级）传感器对三方应用
    完全不可达（CDD 7.3.10 规定 Class 1 无 BiometricPrompt 集成；androidx
    `BIOMETRIC_CONVENIENCE` 在 Android 10+ 恒失败），且无 API 能"检测到设备有人脸
    只是没开放给三方"。2023-2026 国产平板（小米 Pad/荣耀 MagicPad/OPPO Pad/vivo Pad/
    华为 MatePad）的 2D 人脸全部按 Class 1 处理、仅供系统锁屏与应用锁（小米官方 FAQ
    自述可被照片破解；华为走私有 UserIAM 体系）；量产反例仅三星 Tab（2D 人脸 Class 2）
    与 Pixel 8（2D 升 Class 3）。故"降低要求/提供 Class 1 开关"在标准 API 上不可实现；
    应用内人脸验证的现实路径为：生物可用优先生物（BIOMETRIC_WEAK），否则回退锁屏
    凭据——后者已由 `verifyWindowsIdentity` 的 `allowDeviceCredential: true` 天然支持
    （Windows Hello"生物或 PIN"同款语义），本次不改验证链路本身。
  - **改动**：① 新增 `biometricFailMessage` 公共翻译：按插件 reject 的 code
    （userCancel/systemCancel/biometryNotEnrolled/noDeviceCredential/
    biometryNotAvailable/biometryLockout/authenticationFailed）映射中文 toast，此前
    插件英文 message（如 "Cancel button was pressed"）直接透出给用户；三处调用方
    （绑定门/自助会话门/设置页安全降级验证）统一接入。② `settings.selfHelloDesc`
    补充"未录入指纹/人脸的设备自动改用锁屏密码验证"（zh/en）。③ 新增 4 个
    `account.biometric*` i18n 键。
  - **验证**：`npx tsc --noEmit --incremental` 通过；`build-apk.ps1` 全量构建通过；
    zh/en JSON 解析校验通过。
  - **真机实测**（Redmi 25060RK16C / HyperOS 3.0 / Android 16，2026-09-12）：录入（8 帧
    采样）✓；验证全链路 ✓——动作挑战 turnLeft 5.8s 完成、本人 similarity=0.69（阈值
    0.55，通过且余量健康）、验证启动链（相机→human load→warmup）约 0.8s；撤销权限后
    首次使用弹窗链路 ✓（系统权限弹窗正常出现，允许后流程正常）。**dumpsys biometric
    实证**：该机 2D 人脸传感器 `oemStrength: 4095`（=Class 1，与调研结论吻合）、指纹
    `strength: 15`（Class 3）——有指纹设备 checkStatus 返回 biometryType=1(指纹)
    isAvailable=true，按设计走系统 BiometricPrompt，人脸回退不触发。
  - **实测发现并修复的两个缺陷**（同批提交）：
    ① i18n 的 `face` 顶级组被插入脚本误嵌进 `mobile` 组，弹窗显示原始键名
    （face.riskTitle）——json 结构化移回顶层；
    ② Radix Dialog 的 Portal 内容挂载晚于外层组件 effect：FaceCaptureDialog 外层
    effect 执行时 videoRef 尚为 null，run() 静默 return，getUserMedia 从未发起，
    表象为「正在启动相机」永久挂起——流程拆进 Portal 内部子组件 CaptureFlow
    （自身 effect 时 ref 必然就绪）。



### 前端性能优化

- **2026-09-13 安卓前台静置满帧修复（常驻 rAF 清零，双端 renderLiveness）** — 模块
  `android/frontend/src/{index.css,hooks/useAdaptiveFramePace.ts,lib/renderLiveness.ts}`、
  `tauri-app/frontend/src/lib/renderLiveness.ts`（双端同提交）。①`index.css` 修回
  `.anim-idle .animate-pulse` 的 `animation-play-state: paused` 冻结（此前该选择器与
  `.scrollbar-none` 被误并进同一条规则而整条失效，安卓独有回归；桌面树本就正确）；②
  `lib/renderLiveness.ts` 模块级常驻 rAF 无限循环（导入即启动、永不停止）改按需 2 帧探测
  （距上次探测 >4s 才开 ≈33ms 窗口，10s 停滞判定语义不变，`document.hidden` 直接返回 true
  防节流期误判），双端各改一份（原文件两端 md5 一致）；③`useAdaptiveFramePace.ts` 的 rAF
  递归轮询改 `setInterval`(250ms) 回落 + 交互事件回调即时提帧（零感知延迟），gsap
  `autoSleep: 5` 空闲停摆语义保留。为什么——真机 25060RK16C 实测前台静置 10s 渲染 2438 帧
  （≈满刷新率）、RenderThread 40~44% + VizWebView 16~22% + 主线程 15%（≈0.8 核），三处
  常驻/失效的合成源叠加导致 Chromium 合成器永不休眠——这是前台 2.5W 功耗/20% CPU 的主因。
  **验证数据（A/B）**：修复后 `dumpsys gfxinfo` 10s 静置 `Total frames rendered` = **0**
  （基线 2438）、Janky 0.00%；`top -H` RenderThread **0.0%**（基线 40~44%）、VizWebView
  0.0%（基线 16~22%），进程全部线程瞬时 CPU <1%。双端 tsc rc=0、vite build 无新告警。
  commits 29a0550/266624b/02b0a74 已合并 main。



### 文档

- **2026-09-13 修正 dual-platform-sharing 概念篇的过期计数并补齐 source_files** — 模块 `.codewiki/concepts/dual-platform-sharing.md`。
  - **为什么**：该篇正文引用的两端计数在 P2 后已过期（命令 56→60、字段 44→46 / 35→37），但 `cw status` **没有**把它标为 stale——它的 `source_files` 列的是 `config/mod.rs`、`network/mod.rs` 这类模块声明文件，而实际失效的是正文里由 `app/startup.rs`、`config/model.rs`、`config_state.rs` 决定的具体数字。这是 cw 机制的一个盲区（相比之下同批 10 篇因 source_files 命中而被正确检出）：**文章结论取决于哪些文件，就该把哪些文件列进 `source_files`**，否则正文数字会静默过期。
  - **改动**：① 计数修正——桌面命令 56→60（`app/startup.rs:63-124`）、桌面配置字段 44→46（`config/model.rs:10-110`）、安卓字段 35→37（`config_state.rs:15-76`），桌面独有命令清单补 `reset_dns`/`export_config`/`import_config`/`export_diagnostics`。② 全文 13 处行号漂移修正（`network/mod.rs:19-20/22-23/45-46`、`platform/mod.rs:5-18`、`update/updater.rs:443`、安卓 `update_cmds.rs:278/398`、`cpu_affinity.rs:8-30`、`identity_gate.rs:8/23-35`、`config_state.rs:140-169/248-273/374-383` 等）。③ Known Issues 的字段交集按当前结构体重算——46 / 37 中 **33 个共有**（原 31），桌面独有 13、安卓独有 4 的集合不变；本轮双端各 +2 个同名 scheduled 字段只推高了共有数。④ 失效引用 `CODE_WIKI.md:289-290`（该文件已删除）按「历史事实 + 现状补注」改写为约定现落在 AGENTS.md。⑤ `source_files` 补 12 个决定关键数字的文件（两端 `settings/types.ts`、`config/model.rs`、`config/validate.rs`、`app/startup.rs` 已在、`auth/mod.rs`、`commands/system.rs`、`update/updater.rs`、`android/{identity_gate,quality_cmds,system_cmds,update_cmds}.rs`），堵住上述盲区。
  - **验证**：补的 12 个 `source_files` 逐一实测存在；抽查 `update/updater.rs:443` = `env!("APP_VERSION")`、`build.rs:37` = `rustc-env=APP_VERSION` 注入行、`config/model.rs:10-110` 与 `config_state.rs:15-76` 的结构体边界均吻合；字段交集算术自洽（33+13=46、33+4=37）；`cw index` 109 篇无跳过、`cw status` = `Wiki is up to date.`


- **2026-09-13 wiki 同步 10 篇 stale 文章至 P2 与死代码清理后状态** — 模块 `.codewiki/`（10 篇文章 + `_meta.yaml`）。
  - **为什么**：P2 的 14 个提交（`8410605..1dda320`，94 文件 +2916/-511）与 `slideDirection` 死代码清理后，`cw status` 报 10 篇文章 stale——其 `source_files` 命中了改动文件，文中大量 `文件:行号` 引用与部分结论已与实际代码脱节（典型如已知问题条目描述的 `PANEL_ORDER`/`getPanelDirection` 已被删除）。
  - **改动**：按前端 / 后端两组并行重核更新。① 前端组：`desktop-frontend-panels`（App.tsx 全面重标行号、转场管线与标题同向滑入段、Known Issues 删方向动画条目并重排编号为 1-7）、`desktop-frontend-shared`（删 `PANEL_ORDER`/`getPanelDirection` 表格与常量条目、Known Issues 重排为 1-16、i18n 叶子键 719→750）、`android-frontend-core`（TauriApi 表 +4 方法、`useAuthStore` +12 行偏移、Known Issues 第 4 条改写为现状）、`deferred-panel-transition`、`panel-import-strategy`。② 后端组：`ipc-command-surface`（桌面命令 56→60，新增 `export_config`/`import_config`/`reset_dns`/`export_diagnostics`；安卓命令 48 不变但模块 14→15 个；事件总线矩阵 16 行行号）、`network-quality-default-off`、`quality-detail-key-contract`（补 `lightweight` 复核模式不破坏 key 契约的说明）、`audit-findings-must-be-verified`（补 `1dda320` 后续）、`_architecture`（命令数、`Config`/`Settings` 字段数、数据流 30 余处行号）。历史叙述按「历史事实 + 现状补注」保留，删除条目同步重排后续编号。
  - **验证**：`cw index` 109 篇文章全部进索引、零静默跳过；`cw status` = `Wiki is up to date.`。抽查命中：`App.tsx` 转场 `424-438` / `variants:427` / `ErrorBoundary:434` / `Suspense:435`；桌面命令注册块（`app/startup.rs:63-124`）实测 60 条、安卓（`lib.rs:57-106`）48 条、模块声明 15 个；i18n 双端各 750 叶子键 / 22 命名空间 / 803 行（node 递归精确统计，与文章一致）。
  - **影响**：wiki 编译基准推进到 `1dda320`；此后文档改动与代码同一次提交的约定继续生效（本次为 `1ffcb0f`）。


- **2026-09-13 code wiki 迁入仓库内 `.codewiki/`（给 cw 打本地补丁）** — 模块 `.codewiki/`（新增 108 文件）、`AGENTS.md`；工具 `cw`。
  - **为什么**：wiki 原先在仓库外 `~/.codewiki/<项目名>/`（工具硬编码），不随仓库走——换机器丢失、团队看不到、无法评审。
  - **改动**：① 给 `cw` 打补丁：`wiki_path()` 由 `~/.codewiki/<项目名>` 改为 `<repo>/.codewiki`，`cmd_projects` 的无 wiki 提示语同步更新；补丁源码存 `E:\ik\software\codewiki`（分支 `feat/wiki-inside-repo`），`cargo install --path .` 重装。② 103 篇文章 + `_architecture`/`_patterns`/`_index`/`_meta` 共 108 个文件迁入仓库内 `.codewiki/`（`queries/` 加 `.gitkeep` 保留空目录），文章内容零改动。③ `~/.zcode/AGENTS.md` 的 CodeWiki 章节改为「wiki 在仓库内、随仓库进 git、改动与代码同一次提交」，并写明这是打过补丁的 cw、重装必须用本地源码。④ 项目 `AGENTS.md` 引用与 `.codewiki/_meta.yaml` 编译点同步。
  - **验证**：`cw path` = `E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin\.codewiki`；`cw status` = `Wiki is up to date.`；`cw projects` 提示语已更新；旧位置的 C 盘 junction 与 `E:\ik\Documents\trae_projects\.codewiki` 均已清理；合并推送 `44a1da9`，主目录已 ff 快进。
  - **影响**：此后 wiki 变更与代码改动同一次提交；`_meta.yaml` 的 `repo_path` 为绝对路径（换机器需重新 `cw init`）。


- **2026-09-13 代码知识库迁移到 CodeWiki 体系（删除仓库内 CODE_WIKI.md）** — 模块 `CODE_WIKI.md`（删除）、`AGENTS.md`、`README.md`；工具链 `cw`（mraza007/codewiki，Rust CLI，`cargo install` 全局装到 `~/.cargo/bin`）。
  - **为什么**：原 2134 行人工文档靠手工维护、易与代码脱节——本次核对即发现安卓命令数「44」、`config_state` Settings 字段数「31」、`protocol_cmds.rs` 行数「180」等均已过期。
  - **改动**：① 卸载先前试装的 Python 版 CodeWiki（pip 包 + `~/.codewiki/config.json` + Windows Credential Manager 凭证），改装 `cw`——它不调用 LLM、由 agent 读代码写文章，无需 API key；② 编译出 103 篇文章到仓库外 `~/.codewiki/Wxxy-CampusLogin/`（modules 19 / concepts 5 / decisions 32 / learnings 47）加 `_architecture.md` 系统总览与 `_patterns.md` 模式集；原文档「关键约定 / 踩坑记录 / 决策记录」三节共 56 条全部迁移为 decisions/learnings，无遗漏；③ 删除仓库根 `CODE_WIKI.md`，`AGENTS.md` 6 处引用改指向新 wiki，README 目录树同步（顺带修正安卓命令数 44→48）；④ ZCode 侧在全局 `~/.zcode/AGENTS.md` 追加 CodeWiki 会话规则：会话开始 `cw status`、任务收尾写 learnings/decisions + `cw index` + `cw meta update`，对所有项目生效。
  - **验证**：`cw status` 返回 `Wiki is up to date.`（基准 commit b187408，含已合并的安卓省电改造等 11 个提交）；`cw index` 103 篇全进索引、零悬空零遗漏；独立分身在 8 篇文章抽查 40 处 `文件:行号` 引用命中 37 处、模块覆盖 335/344 源文件；核出并订正的错误均已复核（安卓命令 48 条 / 注册块 55-104、前端 `Config` 43 字段四处漂移收敛、`android-plugins` 锁语义前后矛盾、15 条失效 source_files 路径）。
  - **注意**（当日后续已变更）：wiki 最初位于仓库外，同日已迁入仓库内 `.codewiki/`（见本分类下一条）；`cw` 在 Windows 上生成的 `_index.md` 用反斜杠分隔符（Obsidian 不解析），已记入全局约定。


- **2026-09-12 新增 RELEASE_NOTES_v2.3.6.md（历代同款风格）** — 按 v2.3.5..HEAD 的 46 个提交
  与 CHANGELOG [Unreleased] 汇总：引言三大主题（安卓联网修复完整落地/WiFi 事件驱动秒级响应/
  2D 人脸验证）+ 🌐联网检测 6 项、🔐安全验证 3 项、✨新功能 5 项、🎨界面素材 6 项、🛡️稳定性
  2 项 + 升级说明 8 条。注意：v2.3.5 公告提到的安卓 WiFi 全链路绑定修复实际提交在 tag 之后，
  本版 notes 按代码事实完整纳入。本地存档不入 git（RELEASE_*.md 在 .gitignore，与历代一致）；发布 GitHub Release 时正文可直接取用。

- **2026-09-12 删除过时的版本号口径与本地旧计划文档** — 模块 `CODE_WIKI.md`、`AGENTS.md`、`docs/`（本地，不入 git）。
  - **版本号口径修正**：「五处同步」与附录 H 实际步骤清单数字不符（历史漂移），§1.4/§三-9/AGENTS.md 三处统一改为「版本号同步清单」并指向附录 H；附录 H 第 4 步明确双端 package.json 路径，新增第 9 步补安卓后端两处（`android/src-tauri/tauri.conf.json` + `Cargo.toml`，实测均为 2.3.5，APK versionName 随 tauri.conf.json 生成）。
  - **docs/ 本地清理（删除不可恢复）**：删 `TASK_PLAN_2026-06-23.md`（v2.2.7 时代、状态仍「待执行」）、`docs/plans/2026-06-09-per-profile-dns.md`、`docs/releases/v2.2.8.md`、`docs/superpowers/` 5~6 月旧计划 7 份与 spec 6 份；保留 2026-08-06 审计计划与 2026-09-07 安卓系列计划/spec。
  - **验证**：git diff 复核（5+/4−）；删除后 ls 复核保留项无误；commit b16a929 已合入本地 main。

- **2026-09-12 AGENTS.md 项目约定更新为双端（Windows+安卓）** — 模块 `AGENTS.md`。
  - **为什么**：安卓端已并入本仓库（2026-09-10），安卓前端是独立复刻代码库，桌面改动不会
    自动同步——近期平板布局差异审计（CODE_WIKI 决策记录 2026-09-12）实际查出 5 处桌面修复
    未同步；原约定仍是 Windows 单端视角，缺双端同步的流程约束。
  - **改动**：① 必守约定新增第 3 条「通用改进双端同步」：前端 UI/交互、文案 i18n、图标与
    看板娘素材、IPC 命令面、配置字段与默认值、事件与日志类型等通用改进，必须在同一次提交内
    双端各改一份（`tauri-app/` + `android/`），提交前 `git diff --stat` 自检；例外为协议
    核心（桌面 crate 单点，安卓 path 依赖自动继承）与平台专属能力（DPAPI/Keystore、托盘/
    前台服务）。② 验证小节补安卓端：host `cargo check`/`cargo test` 在 `android/src-tauri`
    基线即失败（mobile-only 插件门控），只认 `cargo check --target aarch64-linux-android
    --all-targets`（注入 NDK 工具链）或 `pwsh android/build-apk.ps1` 出包；安卓前端类型
    检查同桌面同法（`android/frontend` 下）。③ 项目速览改为双端描述（两端根目录）。④ 修正
    过时基线：cargo test 全绿 229→493（对齐 CODE_WIKI §1.4）。
  - **验证**：纯文档改动；git diff 复核（10+/7−），分支 feat/sess-85719bbc-agents-dual-sync
    已合并推送 main（8b230c5），主目录已 ff 快进。


- **2026-09-12 精简 CODE_WIKI：删除已解决问题的叙述与历史流水** — 模块 `CODE_WIKI.md`。
  - **为什么**：文档积累了大量"曾如何 / 旧实现如何 / 重构后如何"的修复过程复盘、内部任务
    编号（B9-x/9c/AM-x/FE-A-x/3b 阶段）与无信息量的日期夹注——均属已解决问题的叙述，对后人
    无指导价值，且稀释正文可读性。
  - **改动**：删除修复过程叙述与旧实现/新实现对比句（Portal 端口误报、注销顺序重构的旧实现
    描述、网络绑定权限误判复盘、WebView2 白屏恢复实测过程、安卓向导缺失背景等）、内部任务
    编号与默认值变更日期。顺带修正 §4.1 命令数 55→56、§6.1 标题格式、附录 H 与升级流程重复的
    "5 处必须一致"清单块。**保留**：架构、模块结构、关键约定、踩坑记录（仍会再撞的坑）、决策
    记录（含"为什么"）、已知限制与待加固方向（`secrecy::SecretString`、安卓 host 测试限制），
    **认证与自助服务协议模块（§4.5.3 / §4.5.4 / §4.5.4.1~§4.5.4.3 / §4.7）技术内容整体保留**。
  - **验证**：`CODE_WIKI.md` 2124→2122 行（79+/81−，删的是段内夹注与历史句）；代码围栏 56 个
    成对闭合；认证协议关键词计数逐一比对无变化（`Radius 注销` 7、`mac/unbind` 3、`dr1003` 2、
    `FLDEXTRA` 5、`getOnlineList` 2、`getLoginHistory` 2、`checkcode` 4、`randomCode` 2、
    `wlan_user_ip` 7、`block_on_http` 4、`ensure_portal_port` 3，均与原版一致）；分身交叉扫描
    复核残留与破损，据此修复 1 处删改破损（helper 提权清除 MAC 的设计理由被删过头）+ 7 处漏删。



### 缺陷修复

- **2026-09-14 定位 WebView 白屏崩溃根因（RTSS hook 注入）并增加启动检测告警（桌面）** — 模块
  `tauri-app/src-tauri/src/{platform/rtss_compat.rs(新增),main.rs,app/startup.rs}`。用户实测 WebView 白屏后
  自动恢复、且"重新安装后首次启动必触发"：崩溃转储（EBWebView/Crashpad，本次新开启的
  `--enable-crash-reporter` 首次抓到 minidump）证明根因是 **RivaTuner Statistics Server（MSI Afterburner
  组件）向 WebView2 子进程注入 RTSSHooks64.dll**，在 msedgewebview2.exe 内 0xc0000005 访问违规
  （faulting module=RTSSHooks64.dll，ProcessType=browser，崩溃时刻与重装后首次启动的 WebView2 首次
  GPU 初始化吻合）。① 启动极早期（main 入口、WebView2 环境创建之前；实测放在 setup 回调里为时已晚，
  浏览器进程已创建）检测 RTSS 安装并暂存结果，logger 就绪后在 startup 留痕；② **尝试过"自动写排除
  profile"并证伪**：按被注入进程名向 RTSS 的 Profiles 目录写 msedgewebview2.exe.cfg（两行精简版与
  全字段完整版各一轮、覆盖"RTSS 重启前后"两种时序、含重启 RTSS 清内存缓存的对照），本应用 6 个
  WebView2 进程始终 3 个被注入；对照组（已存在的其他 WebView2 进程）恒 0 注入只能证明 RTSS 不补
  注入旧进程，不能证明 profile 生效——结论：**RTSS 7.3.7 忽略手放入 Profiles 目录的 cfg 文件，排除
  只能在 RTSS 界面手动完成**（按住 Shift 点 Add 创建排除 profile，或创建 profile 后把 Application
  detection level 设为 none），检测到已安装即 log_warn 告知该结论与手动步骤；③ 随方案证伪移除了
  曾实现的 NSIS 安装期写入钩子。**验证**：`cargo test` 328 passed / 0 failed（含 rtss_compat 3 个新
  用例）；注入状态用进程模块枚举实测（RTSS 运行时 3/6 注入、崩溃与注入的因果由 minidump 锁定）。


- **2026-09-14 修复「切换账号提示成功但页面未切换、需手动刷新」** — 模块 `tauri-app/src-tauri/src/commands/account.rs`、
  `tauri-app/frontend/src/{account/useAccount.ts,hooks/useEventListeners.ts}`、`android/frontend/src/...`（前端双端同构）。
  根因是**三条路径同时断了**：① 桌面 `switch_account` 成功返回的 `activeAccount` 为 `None`，且该字段带
  `skip_serializing_if = "Option::is_none"` 被整体从返回 JSON 中省略（安卓本就返回，故只有 Windows 端有症状）；
  ② 前端 `if (result?.activeAccount)` 真值守卫恒假，从未写入 zustand 中与 `config` 并列的独立 `activeAccount` 字段
  （界面高亮读的正是该字段）；③ `config-changed` 事件只把数据 merge 进 `config` 对象、不更新独立字段——**托盘菜单切换账号
  走的正是这条事件路径**，只修返回值会漏掉一半场景。三处同修：后端改用 `ok_with_account` 返回、前端守卫改为
  `!== undefined`（与 delete 一致）、事件回调里同步 `activeAccount` 并补拉账号列表。
  **验证**：桌面 `cargo test` 325 passed / 0 failed（含新增的切换返回契约用例）；前端 `npx vitest run` 新增
  `useAccount.test.tsx` 8 例覆盖切换/删除/改名的返回值处理，14 files / 89 tests 全绿。


- **2026-09-14 修复「自动启用被禁用网卡」永不触发（禁用分类盲区）** — 模块
  `tauri-app/src-tauri/src/network/discovery/windows.rs`。用户反馈：昨晚禁用手选网卡，今早
  开机应用未自动启用（v2.3.6 日志全程无一条「检测到手选适配器被禁用」）。根因：四分类只在
  `OperStatus==NotPresent` 且注册表 `ConfigFlags` 含 `CONFIGFLAG_DISABLED` 位时判禁用，其余归
  「未连接」——但微软文档明文禁用后 OperStatus「Down 或 NotPresent 皆可能」（见
  `MIB_IF_ROW2`/`IP_ADAPTER_ADDRESSES_LH`），且 `ConfigFlags` 为未文档化行为、开机重新枚举/
  驱动重装会洗掉该位（本机 setupapi.dev.log 实证 USB 网卡开机重装 rtump64 驱动）。业界
  （psutil 等）判禁用的文档化依据是 `AdminStatus`（禁用→Down；拔线→保持 Up）。修复：非 Up
  接口查 `GetIfEntry2` 的 `AdminStatus==Down` 判禁用，查询失败回退 `ConfigFlags` 位；决策
  提取为纯函数 `classify_adapter_status`（TDD，8 用例决策表）。见 CodeWiki
  `learnings/adapter-disabled-classification-blindspot`。**验证**：cargo test 全绿
  （297 lib + 297 集成 + 1 doc，新增 8 例决策表；原行为用例无回归）。


- **2026-09-13 修复安卓常驻通知“WiFi 断开后仍显示在线”** — 模块 `android/src-tauri/src/monitor_loop.rs`。
  真机反馈：断开校园网 WiFi 后，常驻监控通知长时间仍显示“在线”。根因两处叠加：① 在线判定三态护栏
  `Ok(s) if s.error_kind.is_none() => s.online, _ => prev_online` 把“探测失败”一律折叠成“沿用上一拍在线”，而 WiFi
  断开后进程走蜂窝、内网 Portal 探测必失败（`error_kind=request_failed`），在线状态被永久钉死；② 常驻通知只在
  状态翻转时重建（省电设计），状态不翻转 ⇒ 通知永不更新。“是否在校园网”的 `on_campus` 此前只用于自动重登与
  掉线通知，不参与在线判定。修复：新增 `_ if on_campus => prev_online` 分支，`on_campus=false` 时判离线（WiFi 断开
  走蜂窝/家用网络时 /18 子网、校园网关 TCP、Portal TCP 三判据全否）—— 与桌面 `background_check.rs` campus fail 置
  `any_adapter_online=false` 语义对齐；校园网内探针失配仍保持上一拍，2026-09-09 的反误报语义不变。同时补齐检测
  静默期（时段外整拍跳过探测，设计不变）的通知保鲜：return 前把常驻通知重建为“监控运行中 · 已暂停检测(非检测时段)”，
  `notified_online` 状态码新增 3，避免夜间停在白天那一拍的“在线”。**验证**：安卓交叉编译 `cargo check` Finished、
  桌面 `cargo test` 全绿（共享 crate 语义未动）。**真机实测（25060RK16C，2026-09-13）**：无 VPN 下关 WiFi
  走蜂窝约 5 秒通知翻转为「未连接」（WiFi 事件去抖 1s + 延迟 2.5s + 探测耗时，与设计吻合）；连回家用 WiFi
  （路由器级联校园网，Portal 经路由器 NAT 真实可达且返回路由器 WAN 会话的「已在线」页）显示「在线」属真实
  可达判定。secure 全量 VPN 接管本应用时探测/登录不可用为平台边界，绕过不可行（netd ip rule 优先级
  13000<15000，bind 被 EPERM 拒绝），详见 CodeWiki `decisions/android-vpn-bypass-infeasible`。


- **2026-09-13 修复「校园网检测时间段」行窄屏中文竖排（双端）** — 模块
  `android/frontend/src/monitor/MonitorPanel.tsx`、`tauri-app/frontend/src/monitor/MonitorPanel.tsx`。手机端该行标题与描述
  被压成竖排单字、行高撑满整屏：外层 `flex items-center justify-between` 下，右侧两个 `w-24` 的 `<input type="time">`
  合计约 200px 不可压缩（`shrink-0`），左侧 `min-w-0` 容器被压到接近 0 宽。改为
  `flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between`：窄屏纵向堆叠、≥640px 恢复原一行布局，语义
  （时间值读写 / id / aria-label / i18n key）零改动。**验证**：浏览器 mock 实测 380×780 —— 临时还原旧 class 量得标题
  14×164px（竖排单字）、描述 645px 高，修复后标题 112×17px 单行、描述 45px 三行；700px 宽下 `flexDirection: row`
  且标题与时间控件同行；两端 tsc rc=0。


- **2026-09-12 补齐 WebView2 诊断链四个缺口（崩溃转储/版本落盘/参数日志/wifi-accept）** — 模块
  `tauri-app/src-tauri/src/main.rs`、`platform/gpu.rs`、`app/startup.rs`、`app/webview_recovery.rs`、
  `android/plugins/network-bind/.../NetworkBindPlugin.kt`。
  - **为什么**：前轮白屏/崩溃调研遗留四项待办——① WebView2 Crashpad 转储默认禁用，
    `EBWebView/Crashpad/reports/` 一直为空，浏览器进程退出拿不到 dump；②
    `record_webview2_runtime_version` 依赖 EdgeUpdate 注册表，实测机器三路径全 MISSING，
    版本号未落盘；③ `build_browser_args()` 的参数日志在 logger 初始化之前打印，从未落盘；
    ④ `[wifi-accept]` 的 `already_validated` 分支 Kotlin 侧不设 reason，日志显示 `reason=?`。
  - **改动**：① `main.rs` 在设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 时追加
    `--enable-crash-reporter` + `--crash-dumps-dir=<Roaming>\com.campus.login\crashdumps`
    （目录启动即建，与 app_data_dir 同源）；② 注册表三路径失败后回退枚举
    `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\*` 目录名取最高版本
    （纯函数 `pick_runtime_version` + 单测），来源标注"安装目录回退"；③ 参数日志从
    `build_browser_args()` 移除，改在 `startup.rs` logger 初始化后打印 env 实际值（含
    crash 参数）；④ Kotlin `already_validated` 分支补 `reason: "already_validated"`。
  - **验证**：桌面 `cargo test` 256 全绿（含新增目录回退单测）；Android 侧一行 JSON
    字段补齐未单独出包，随下次构建验证；crash 转储落盘需待真实崩溃复现确认。


- **2026-09-12 修复更新下载两个边界：并发下载互斥 + 大小上限写入前判定（双端）** — 模块
  `tauri-app/src-tauri/src/commands/updater.rs`、`tauri-app/src-tauri/src/infra/state/mod.rs`、
  `android/src-tauri/src/update_cmds.rs`。
  - **为什么**：核查发现的两个已知边界——① 并发 IPC 调用 `download_update` 会互写同一
    临时文件互相覆盖（前端 downloading 状态机只挡 UI 层重复点击，单实例锁下触发面极小
    但非零）；② 大小上限检查在写入分块之后，超限时多写一个 chunk 才中止。
  - **改动**：① 桌面 `TaskFlags` 新增 `is_downloading: TaskLock`，`download_update` 开头
    `try_acquire`，TaskGuard RAII 全路径自动释放，占用中返回"已有下载任务进行中"；安卓
    新增 `DOWNLOAD_RUNNING: AtomicBool`，外壳函数 swap 抢占 + 内部函数 `download_update_inner`
    承载原逻辑，保证所有提前 return 路径复位标志（语义与桌面一致）。② 两端把
    `downloaded + chunk.len() > 上限` 判定移到 `write_all` 之前，超限不落盘即删临时文件中止。
  - **验证**：桌面 `cargo test` 511 全绿；安卓
    `cargo check --target aarch64-linux-android --all-targets` 通过。


- **2026-09-12 更新检查时机统一：启动延迟 5s 首查 + 每 24h 一轮（双端）** — 模块
  `tauri-app/src-tauri/src/update/updater.rs`、`android/src-tauri/src/update_cmds.rs`。
  - **为什么**：桌面原逻辑"启动即查（last_update_check_epoch_ms 仅内存，重启清零）"会与
    启动期任务挤在同一窗口；安卓原首启延迟 30s，配合新增的更新弹窗显得提醒偏慢。
  - **改动**：桌面 `start_update_check_loop` 重写——启动后延迟 5s 首查（新增常量
    `STARTUP_CHECK_DELAY_SECS`），删除"距上次检查超 24h 才立即查"的内存时间戳判断，之后
    每 24h 一轮（保留 5s 步进 + cancel token/退出标志可中断语义，循环改为"先睡后查"避免
    首查后双查）；安卓启动延迟 30s → 5s，之后每 24h 不变。`last_update_check_epoch_ms`
    字段保留写入（手动检查命令仍写），仅移除循环内的读取判断。
  - **验证**：桌面 `cargo test` 511 全绿；安卓
    `cargo check --target aarch64-linux-android --all-targets` 通过（注入 NDK 27.0
    工具链环境变量）。


- **2026-09-12 更新提醒补强：桌面通知可点击跳转关于界面 + 安卓新增更新弹窗（手机/平板）** — 模块
  `tauri-app/src-tauri/src/platform/toast.rs`（新增）、`infra/events.rs`、`update/updater.rs`、
  `Cargo.toml`、`tauri-app/frontend/src/App.tsx`、`hooks/tauriApi.ts`、
  `android/frontend/src/shared/UpdateAvailableDialog.tsx`（新增）、`hooks/useQualityStore.ts`、
  `hooks/useEventListeners.ts`、`App.tsx`、`components/tablet/TabletShell.tsx`、两端 `i18n/locales/*.json`。
  - **为什么**：两端均有自动检查更新机制（桌面启动+每 24h；安卓启动 30s 后+每 24h），
    但提醒链路弱：桌面系统通知点击无任何行为；安卓只有日志+版本角标、无主动弹窗。
  - **改动**：① 桌面新增 `platform/toast.rs` 自组 WinRT toast（tauri-plugin-notification
    桌面端不暴露 Activated 回调，无法自定义点击行为）——文案"新版本 vX 可用，前往关于界面
    进行更新"，点击通知唤起主窗口并 emit 新事件 `update-notification-click`，前端监听后打开
    关于界面（AboutDialog）；发送失败降级走原 `emit_notification`；AUMID 借用 notify-rust
    同款 PowerShell APP_ID，与既有系统通知同源。② 安卓新增 `UpdateAvailableDialog` 弹窗
    （手机 App.tsx 与平板 TabletShell 双壳挂载），收到 `update-available(hasUpdate)` 时弹出，
    含「立即前往更新」（跳转关于界面）与「关闭」按钮，状态 `qualityStore.updatePromptOpen`，
    关闭后本次运行不再打扰（每 24h 检查循环重弹一次）。③ 下载模块核查结论：检查→下载→
    SHA256→安装链路两端完整；核查发现的两个边界（并发下载互写临时文件、大小上限
    写入后才判定）当时标记"不修"，随后已于本日修复，见下方「缺陷修复」条目。
  - **验证**：桌面 `cargo check --all-targets` + `cargo test` 511 全绿；两端
    `npx tsc --noEmit --incremental` 通过；i18n JSON 校验通过；安卓 Rust 未改动、
    无需交叉编译。clippy（本机首次安装）对存量代码报 6 个 lint（protocol/logger/
    auto_auth/client/quality），与本次改动无关。


- **2026-09-12 安卓新手指引移除"选择网络适配器"步骤（5 步 → 4 步）** — 模块
  `android/frontend/src/settings/useOnboardingFlow.ts`、`OnboardingWizard.tsx`、
  `OnboardingWizardMobile.tsx`、`App.tsx`、`components/tablet/TabletShell.tsx`。
  - **为什么**：安卓网络出口由系统决定（`protocol_cmds.rs` 的 `do_login` 对 adapter
    参数显式丢弃 `let _ = adapter`），指引里让用户选主/副适配器在安卓上完全无效，
    纯属桌面复刻残留，误导用户。
  - **改动**：流程 hook 删除适配器状态/落盘/校验（`handleLoginAndFinish` 改为无参
    `onLogin()`），平板 Dialog 向导与手机全屏向导删除适配器步骤 UI 与完成页适配器摘要
    （步骤圆点/进度轨随 `ONBOARDING_STEP_COUNT=4` 自动收敛）；两处调用方同步去掉
    `adapters` prop。向导组件不再消费 `adapters`，`onLogin` prop 类型保持
    `(adapterName?: string)` 不变以兼容 `doLogin` 签名。
  - **验证**：`npx tsc --noEmit --incremental`（android/frontend）通过；净删约 230 行。
    GitHub 调研确认（见会话记录）：Android 开机自启（BOOT_COMPLETED，normal 权限）与
    生物认证（BiometricPrompt BIOMETRIC_WEAK）均无需 root，故设置页"开机自启动"与
    "生物认证"两项**保留不删**。


- **2026-09-12 校园网检测开始时间默认值 8:00 → 7:40（双端）** — 模块
  `tauri-app/src-tauri/src/config/model.rs`、`tauri-app/frontend/src/settings/constants.ts`、
  `tauri-app/frontend/src/monitor/MonitorPanel.tsx`、`android/frontend/src/settings/constants.ts`。
  - **为什么**：用户在校场景希望 7:40（第一节课前）即开始自动检测/登录，原默认 8:00 偏晚。
  - **改动**：默认分钟数 480 → 460（Rust `default_campus_check_start_minutes` + Default
    impl、前端 constants、MonitorPanel `?? 460` 兜底，双端共 5 处）。仅影响新装/未自定义
    该项的用户；已存量配置中落盘的 480 不迁移（尊重用户既有值）。旧版小时值迁移逻辑
    （`validate.rs` ×60）不受影响。
  - **验证**：桌面 `cargo test` 256+1 全绿（含 `campus_check_start_minutes` 迁移/钳制用例）。


- **2026-09-12 安卓端补齐"校园网检测开始时间"设置（对齐桌面）** — 模块
  `android/src-tauri/src/config_state.rs`、`android/src-tauri/src/monitor_loop.rs`、
  `android/frontend/src/monitor/MonitorPanel.tsx`。
  - **为什么**：此前该设置仅桌面端有（安卓 UI 被 `!isAndroid` 隐藏、后端 `Settings`
    无字段、巡检无门控）——非在校时段（如假期）仍每 60s 探测耗电、掉线通知可能误报。
  - **改动**：① 后端 `Settings` 新增 `campus_check_start_minutes: u16`（容器级 serde
    default，旧配置文件缺字段自动回落 460）；② `run_check_once` 在绑 WiFi/探测前加
    时间门控：当前时间早于开始时间（0=禁用）整拍跳过，在线状态保持上一拍记忆（与桌面
    `background_check` 静默期语义一致）；③ 前端把"检测开始时间"块移出 `!isAndroid`
    死代码条件（"自动退出非校园网"因后端不消费 `campusExitOnFail` 保持隐藏）。
  - **验证**：`npx tsc --noEmit --incremental`（android/frontend）通过；
    `pwsh android/build-apk.ps1` 全量构建（vite + aarch64 交叉编译 + Kotlin 打包）通过。
    注：安卓 crate 的 host `cargo test` 因 build.rs 解析 mobile 插件权限失败为存量环境
    限制（CodeWiki §踩坑已录），非本次改动引入。



### 缺陷修复

- **2026-09-12 安卓启动瞬间重复绑定反复清空 HTTP 连接池** — 模块
  `android/plugins/network-bind`、`android/src-tauri/src/protocol_cmds.rs`。
  - **现象**：`ensure_wifi_bound` 由启动自动登录 / 后台检测首拍 / 质量首测 / 手动链路并发
    调用（真机 `08:58:33` 一秒 6 次），每次绑定成功都 `clear_client_pool()`——池中已有连接被
    反复丢弃，首批 HTTP 请求因此多次重建（多付 TLS 握手）。
  - **修复**：Kotlin 侧记住上次绑定的 `Network`，同一网络重复绑定回 `path=already_bound`
    （`unbind` 清空该记忆）；Rust 侧见 `already_bound` 跳过清池。换 WiFi 或重连得到新
    netId 时 `Network` 不相等，照常清池——语义不变，只去掉重复动作。
  - **验证**：`build-apk.ps1` 全量构建通过（Rust 交叉编译 + Kotlin 编译 + 打包）；**真机复验**
    （Redmi 25060RK16C / HyperOS 3.0，重开无线调试后装机）：启动瞬间首次 `path=requestNetwork`，
    随后 3 次并发调用均为 `path=already_bound cleared=0`；稳态每拍（`09:39:14`、`09:40:14`）
    同样 `already_bound` —— 绑定只在首次真做，之后全部短路（还省掉 `requestNetwork` 的
    callback 注册与 3s 超时窗口），全程无 panic。**注**：本次日志 `cleared` 恒为 0，因为该
    时刻池中本无 keep-alive 连接；"跳清池"的收益是**不与首批请求竞争**，不体现在日志条数上。


- **2026-09-12 安卓周期检测 Portal 探测线程每拍 panic（裸线程无 Tokio reactor context）** —
  模块 `android/src-tauri/src/monitor_loop.rs`。
  - **真机证据**（Redmi 25060RK16C / HyperOS 3.0 / Android 16，`adb logcat` tag
    `RustStdoutStderr`）：`thread 'portal-probe' (19261) panicked at
    core/src/ops/function.rs:250:5: there is no reactor running, must be called from the
    context of a Tokio 1.x runtime` —— 23:15:57 / 23:16:57 / 23:18:27 / 23:19:27 连续复现，
    **每 60 秒一拍必现**。
  - **根因**：`portal_probe_on_little_cores` 用 `std::thread::Builder` 起裸线程绑小核
    （稳态功耗优化），裸线程没有 Tokio thread-local context；`check_portal_full` 内部经
    `block_on_sync` 驱动的 reqwest 构造超时计时器（`tokio::time::sleep` →
    `Handle::current()`）时 panic。**仅靠 `infra/async_util::block_on_sync` 的 Err 兜底
    分支（自持 Runtime）实测并未兜住**（原因未查明）；改为与桌面
    `commands/login.rs` 裸线程同款处理——闭包入口 `Handle::enter()`，panic 消失。
  - **影响**：探测 panic → `rx.await` 得 Err → `portal_reachable=false`；三态消费护栏
    （非 `error_kind=None` 保持上一拍记忆）虽避免了误判掉线，但**在线状态永远冻结在初值**，
    Portal 探测功能实质失效，并每拍产生一条 panic 噪音。
  - **修复**：① 闭包入口 `Handle::enter()`（在 async 上下文先取 handle）；② 闭包体包进
    `catch_unwind`，panic 转为 `Err("探测线程 panic")` 回传并落日志——原先直接 unwind，
    调用侧只看到"探测线程提前退出"，真实原因丢失。
  - **Windows 端对照（本次一并排查）**：**不存在同类问题**。桌面侧 Portal 探测调用点全部
    在带 runtime context 的线程上——`monitor/background_check.rs`（每拍）、
    `monitor/auto_auth.rs:356`、`commands/network_cmd.rs:90` 均在 `spawn_blocking` 内；
    唯一的裸线程点 `commands/login.rs:24-46`（`std::thread::scope` 并行双适配器检测）
    已显式 `tauri::async_runtime::handle().inner().enter()`（2026-09-05 注销流程崩溃事故的
    修复方式）。"绑小核裸线程"是安卓端独有实现（`cpu_affinity.rs` 读 cpufreq 分簇），
    桌面无对应代码。另：桌面 `panic="abort"`，若真踩到会直接崩进程，日志中无此现象。
  - **验证**：真机重装后连续两拍无 panic（`[wifi-bind] ok` 正常出现）；
    `cargo check --target aarch64-linux-android` 通过。


- **2026-09-12 安卓 WiFi 绑定回退路径缺权限 + 失败归因误导** — 模块
  `android/plugins/network-bind`。
  - **真机证据**：`[wifi-bind] not_bound reason=bind_rejected_vpn_active_SecurityException:
    com.campuslogin.client was not granted either of these permissions:
    android.permission.CHANGE_NETWORK_STATE,android.permission.WRITE_SETTINGS.
    path=requestNetwork` —— 启动后 1 秒内连续 4 条，绑定全程失败（回落蜂窝）。
  - **根因 1（权限）**：插件清单只有 `ACCESS_NETWORK_STATE`/`ACCESS_WIFI_STATE`，
    `ConnectivityManager.requestNetwork` 必需的 `CHANGE_NETWORK_STATE` 未声明 → 回退路径
    直接抛 SecurityException。（同一时刻 `allNetworks` 直绑也返回 false，与 Android 12+
    该组合失效的既有已知问题一致。）
  - **根因 2（归因）**：失败原因拼接把 requestNetwork 的异常挂在 `vpn_active` 之后，日志
    读起来像 VPN 故障，实际是权限缺失——上一版据此让用户反复排查 VPN/白名单，方向错误。
  - **修复**：① 插件清单声明 `CHANGE_NETWORK_STATE`；② 归因改为
    `allNetworks[<直绑失败原因>] requestNetwork[<异常类名:消息>]`，两条原因分开呈现；
    ③ `ensure_wifi_bound` 的绑定结果补 `eprintln!`（release 包无 root 读不到私有目录日志，
    logcat 是唯一免 root 通道，与 `do_login` 既有诊断同款）。
  - **验证**：真机重装后 `[wifi-bind] ok path=requestNetwork wifi=net+val`（回退路径绑定
    成功），后台脚本另跑两拍同样成功。


- **2026-09-12 安卓 `applyNetworkSettingsCompat` 权限检查用错表（System.canWrite 检查
  Global 写）** — 模块 `android/plugins/network-bind`。
  - 原实现 `if (!Settings.System.canWrite(context)) return "no_write_settings_perm"`：检查的是
    System 表的 `WRITE_SETTINGS`，而实际写的是 `Settings.Global`（由 `WRITE_SECURE_SETTINGS`
    保护，signature|privileged，普通应用**无法通过用户授权获得**）。该检查恒为 false，既让这条
    路径永远不执行，也把真实原因（SecurityException）挡在门外。
  - 改为直接试写并捕获真实异常：root / Shizuku / 系统预装场景可生效，普通签名如实回报。
  - **"直接调底层 API"这条路的真机判定（本次回答用户调研问题）**：
    `[wifi-accept] accepted path=already_validated hiddenApi=false` —— 反射
    `ConnectivityManager.setAcceptUnvalidated` 在 HyperOS 3.0 / Android 16 上**不可达**
    （Android 9+ hidden API 名单拦截，且该 API 需 CONNECTIVITY_INTERNAL 权限）；
    `Settings.Global` 写需系统级权限。**结论：普通签名应用两条底层路径都走不通——但实测也不需要**
    （见下条真机确认）：真正解决"WiFi+流量同开无法登录"的是 `bindProcessToNetwork`，
    它让认证请求能到达校园内网，系统随后自行把网络转 VALIDATED。
  - **用户真机确认（2026-09-12，本条目落地后）**：表现变为**系统默认直连该 WiFi**，不再需要
    手动在系统弹窗选"仍然连接"。机制链条：应用绑定该 WiFi（未验证状态也能经 `requestNetwork`
    绑到）→ 应用完成 Portal 认证 → 系统探测到连通 → 网络转 `NET_CAPABILITY_VALIDATED` →
    系统"登录到 WLAN"提示消失、默认路由落到 WiFi。即"让系统判定直接连接"是**认证成功的自然
    结果**，不是需要修改的系统设置——此前"必须手动确认"的表象，根因是绑定失败导致认证永远
    做不成。两条底层 API（hidden API / Settings.Global）因此并非必需手段（仅在 root/Shizuku
    身份下才可能用上，本项目不引入）。
  - **新增能力（本次一并落地）**：插件新增 `acceptWifiNetwork` 命令（三路径
    `already_validated` / `hidden_api` / `settings_global`），`ensure_wifi_bound` 绑定成功后
    顺带调用，结果落盘日志；Rust 命令 `accept_wifi_network` + 权限文件（`build.rs` COMMANDS
    与 `default.toml`）同步补齐。



### 待办 / 未覆盖

- **2026-09-13 计划 C1（内存口径核查）结论（无代码改动）**：真机 `dumpsys meminfo` 实测
  主进程 TOTAL PSS 255MB（其中 Graphics 136MB = EGL 59MB + GL 77MB）、渲染进程
  sandboxed_process0 RSS 76MB；**Tauri #13554（APK assets openFd 读图载入整 APK）经核实
  安卓端不适用**——安卓 Rust 侧无 `bundle.resources`/`resource_dir()` 读图路径，看板娘
  立绘走 Vite 前端打包（`public/girl/*.webp` → dist），通知大图已补 gen drawable（见
  行为调整）。Graphics 136MB 属 WebView 解码位图缓存本性（`public/girl/` 4.7MB 常驻解码），
  无低成本可摘项，后续若观察到异常增长再深挖。

- ~~崩溃转储缺失~~ **已开启**（2026-09-12）：`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`
  追加 `--enable-crash-reporter` + `--crash-dumps-dir=<Roaming>\com.campus.login\crashdumps`；
  下次崩溃复现时确认 minidump 确实落盘该目录。

- ~~`record_webview2_runtime_version` 注册表 MISSING~~ **已修复**（2026-09-12）：注册表
  三路径失败后回退枚举 `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\*`
  目录名取最高版本（该机预期记录 151.0.4129.107，来源标注"安装目录回退"）。

- ~~"WebView2 浏览器参数"日志从未落盘~~ **已修复**（2026-09-12）：改在 startup logger
  初始化后打印 env 实际值。

- ~~`[wifi-accept]` already_validated 分支 `reason=?`~~ **已修复**（2026-09-12）：Kotlin
  侧补 `reason: "already_validated"`，随下次 APK 构建生效。

- **2026-09-12 安卓端省电优化第一轮（GitHub 调研落地：nudge 节流 + 常驻通知按需更新）** — 模块
  `android/plugins/foreground-service/.../ForegroundService.kt`、`android/src-tauri/src/monitor_loop.rs`。
  - **nudge 唤醒锁节流（5s 最小间隔）**：服务侧 NetworkCallback 的 onCapabilitiesChanged
    在 WiFi 信号/带宽波动时高频连发，原实现每次事件都 acquire 3s PARTIAL_WAKE_LOCK，
    弱信号环境会反复抑制系统 suspend；加 5s 节流窗口。检测及时性不受影响——事件驱动
    检测在 Rust 侧 watcher（自带 2.5s 延迟 + 1s 去抖）与周期 tick。
  - **常驻通知按需更新**：通知文案原含"已检测 N 次"导致每拍（60s）必经 IPC +
    NotificationManager.notify 重建一次，属稳态功耗点；改为仅在线状态翻转时更新
    （文案"监控运行中 · 在线/未连接"），检测次数前端状态页仍可见。
  - **调研备查（未实施项）**：前台服务已是 specialUse 类型（规避 dataSync 6h/24h 上限）、
    事件驱动检测已具备，Tauri 官方口径下"前台服务驱动巡检"架构现状正确。后续可做：
    质量测试循环稳态退避（12+ 外网目标每 60s 一轮为最大功耗主力）、WifiLock
    FULL_HIGH_PERF 降级评估、电池优化白名单入口、dumpsys batterystats 功耗 A/B 基线。
  - **验证**：cargo check --target aarch64-linux-android --all-targets 通过；完整出包
    build-apk.ps1 通过。行为变化：常驻通知不再显示逐拍递增的检测次数。

- **2026-09-12 网络质量检测默认关闭（schema v3），质量页由后台检测状态代替** — 模块
  `android/src-tauri/src/config_state.rs`、`android/frontend/src/{App.tsx,monitor/QualityPanel.tsx,
  hooks/useQualityStore.ts,components/layout/DockNav.tsx,components/tablet/TabletShell.tsx,
  i18n/locales/{zh,en}.json}`。
  - **默认值与迁移**：`enable_network_quality` 默认 true→false（省电：质量检测含 12+ 外网
    DNS/DoH/HTTPS 目标，启动即跑一轮数秒级）；`config_schema_version` 2→3 一次性迁移，存量
    配置刷为 false，用户显式开回后落盘不再覆盖（沿用 v1→v2 迁移先例）。`run_startup_tasks`
    的启动质量检测/定时循环本就受该开关门控，后端逻辑零改动。
  - **前端语义反转**：原先"开关关闭 = 隐藏质量 tab"（DockNav/TabletShell/App 三处条件渲染），
    改为质量页常驻导航；QualityPanel 关闭态第一卡展示后台检测状态（在线/离线 badge + 状态条：
    状态点/最近检测消息/检测次数 + 省电提示与手动测试引导），定时测试卡与详情卡保留。
  - **手机顶栏胶囊**：关闭态由 NetworkQualityCapsule（恒显"未知"，误导）改为后台在线状态
    （状态点 + 在线/离线文案），点击仍进质量页。
  - **手动刷新放开**：refreshQuality 去掉 enableNetworkQuality 门控——关闭默认只省稳态功耗，
    用户点刷新做单次质量测试是显式行为，页面可即时获得完整质量数据。
  - **验证**：`npx tsc --noEmit --incremental` 通过；`cargo check --target aarch64-linux-android
    --all-targets` 通过；浏览器实测（__TAURI_INTERNALS__ mock + vite dev，用后已还原）：平板
    Dock 与手机 BottomNav 质量页关闭态/开启态、顶栏胶囊在线状态均符合预期（截图确认布局）。
    迁移语义有单测锁定（v0→v3 全链路 + 用户值不被覆盖）。

- **2026-09-12 平板布局对齐 Windows 版（差异审计驱动）+ 日志类型枚举漂移修复** — 模块
  `android/frontend/src/{shared/ui-types.ts,hooks/useLogToastStore.ts,components/layout/{RightPanel,TitleBar}.tsx,
  components/tablet/TabletShell.tsx,auth/AboutDialog.tsx}`、`android/src-tauri/src/monitor_loop.rs`。
  - **布局对齐**（deepseek 双树差异审计逐条核实后补齐桌面已修而平板未同步的 5 处）：
    ①日志折叠 ×N 计数徽标（LogEntry.count + store 累加 + 两处渲染）——重复日志此前被静默
    折叠让用户低估失败风暴；②运行日志卡茶娘水印（relative overflow-hidden + 列表 z-10）；
    ③TitleBar logo 换看板娘头像（平板 TOPBAR_ZOOM=1.3,用 w-8 对齐桌面 w-10 视觉尺寸）；
    ④AboutDialog 高度 h-[min(720px,85dvh)] + portrait/sponsor 娘图（原固定 520px 低高度
    横屏易溢出）；⑤侧边娘认断点修正——左娘 900→1008px（720 主区+2×(128+16),原 900-1008
    区间会压进内容列），右娘竖屏不再偏移 304px（竖屏右栏隐藏时 right-[304px] 会错位）。
  - **缺陷修复（真机 mock 实测发现）**：安卓后端 emit_login_log 的 "warn" 与前端 LogType
    枚举 'warning' 不一致——LOG_ICONS[type] 取 undefined 致日志面板渲染白屏（启动自动登录
    校园网判定失败即可触发）；后端改 "warning" 对齐 + 前端映射加 ?? Info 等兜底。
  - **验证**：tsc 通过；cargo check aarch64 交叉编译通过；浏览器 mock 平板视口（1280×800/
    960×720）验证 ×3 徽标、茶娘水印、娘头像、断点边界（960px 左娘正确隐藏）、AboutDialog；
    真机（Redmi 25060RK16C,手机壳）验证：v3 迁移生效、质量页关闭态、顶栏胶囊"在线"、
    手动单次质量测试放行、常驻通知"监控运行中 · 在线"无检测次数、关于对话框正常。

- **2026-09-12 质量检测关闭时质量 tab 删除,底栏由后台检测补位（修正上一轮方向）** — 模块
  `android/frontend/src/{App.tsx,components/layout/{BottomNav,DockNav}.tsx,
  components/tablet/TabletShell.tsx,monitor/QualityPanel.tsx,hooks/useQualityStore.ts,
  i18n/locales/{zh,en}.json}`。
  - **与桌面同语义**：关闭（默认,省电）即删除质量标签页,其余面板补位。平板 DockNav 恢复按
    开关过滤 quality;手机 BottomNav 第 4 位动态互换——质量开启→"网络质量",关闭→"后台检测"
    （网络状态检测面板,MobileTab 类型加 monitor,App.tsx 补 MonitorPanel case 与 handler）。
  - **回滚上一轮的关闭态改造**：QualityPanel 关闭态后台状态展示移除（页面关闭时不可达）、
    refreshQuality 恢复 enableNetworkQuality 门控、i18n 删除 bgSubstitute/bgCheckCount/
    bgManualHint 三个死 key（bgOnline/bgOffline/bgUnknown 保留——顶栏胶囊仍用）。
  - **手机顶栏胶囊**：关闭态显示后台在线状态（绿点+在线/离线）,点击跳后台检测面板;开启态
    仍为网络质量胶囊,点击进质量页。
  - **验证**：tsc 通过;浏览器 mock 实测双态——关闭态底栏 [总览/账号/自助服务/网络状态检测/
    更多] 且后台检测面板完整渲染（运行中/立即刷新/验证设置等）,开启态底栏恢复 […网络质量…]
    且质量页原样 UI;index.html mock 用后已还原,git diff 干净。

- **2026-09-12 更多页删重复后台检测子页 + 顶栏删冗余状态点 + 补抠两张漏抠娘图** — 模块
  `android/frontend/src/{App.tsx,components/mobile/MobileMore.tsx}`、双端
  `public/girl/mascot-side-{music,phone}.webp`、`CODE_WIKI.md`。
  - **更多页子页去重**：质量检测关闭(默认)时"后台检测"已提为底栏 tab,"更多"里的同名子页
    同步移除（chips 动态过滤仅剩 测速/日志/设置）;sub 派生降级防 config 晚到时 chips 与
    内容区不一致。质量开启时"更多"保留后台检测子页（底栏无该 tab）。
  - **顶栏状态圆点删除**：手机壳 header 的 status 圆点与胶囊自带状态点重复,删（连带清理
    未再使用的 status 订阅）。
  - **缺陷修复（资产）**：mascot-side-music/side-phone 两张为 VP8 无 alpha（生成 14 张时
    漏跑 rembg）,深色模式下显示整块浅色背景——按 Windows 版 rembg 管线补抠（u2net,
    裁透明边后 800×1200 画布居中,webp q85）并双端同步替换,解析 RIFF chunk 校验
    VP8X+ALPH 存在;CODE_WIKI 踩坑记入"娘图资产必须过 alpha 校验"约定。
  - **验证**：tsc 通过;浏览器 mock 实测——更多页 chips 仅 [测速/日志/设置] 且默认落测速、
    内容区一致,顶栏仅剩胶囊状态点;抠图质量目检（人物/音符/耳机细节完整,背景透明）。

- **2026-09-12 顶栏恢复独立 status 圆点，质量胶囊关闭时同步隐藏（撤回后台状态胶囊方向）** — 模块
  `android/frontend/src/App.tsx`、`i18n/locales/{zh,en}.json`。
  - 撤回上一轮"关闭态显示后台在线状态胶囊"的方向：恢复原独立 status 圆点（登录状态色点）；
    "● 在线"胶囊删除，NetworkQualityCapsule 仅在 enableNetworkQuality 开启时渲染、关闭同步
    隐藏（与桌面 StatusBar 同语义，也消除关闭态恒显"未知"的误导）。
  - 清理 bgOnline/bgOffline/bgUnknown 三个 i18n key 与 bgStatus 相关订阅。
  - **验证**：tsc 通过；浏览器 mock 实测——关闭态顶栏仅剩 1 个 status 圆点（胶囊隐藏），
    开启态圆点 + 质量胶囊并存；mock 用后已还原。

- **2026-09-12 顶栏图标组固定右侧（胶囊隐藏后不再左移）** — 模块 `android/frontend/src/App.tsx`。
  质量胶囊隐藏后顶栏失去 flex-1 撑开元素,右侧图标按钮（赞助/设置/主题/关于）被同步左移;
  第一个图标加 ml-auto 接管推开职责——开启态胶囊 flex-1 占满时无影响,关闭态图标固定右缘。
  **验证**：tsc 通过;浏览器 mock 实测关闭/开启两态关于按钮右缘一致（374/390 视口）。
