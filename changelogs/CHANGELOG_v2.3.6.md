# 更新日志

## v2.3.6


### 缺陷修复

- **2026-09-11 前端黑屏（WebView 浏览器进程退出后无法自愈）** — 模块 `app/webview_recovery.rs`。
  - **根因（日志 + 复现双证据）**：用户日志 `2026-09-11 21:18:28.911` 记录
    `尝试重载 WebView: 浏览器进程退出（WebView 已整体失效）(kind=0)`，同一时刻
    WebView2 用户数据目录的 `Crashpad/temp/edge_shutdown_crash.txt` 被写入（21:18:28.590），
    证实浏览器进程真的异常终止。浏览器进程退出后 WebView 进入 Closed 状态，
    原恢复动作 `window.eval("window.location.reload()")` **返回成功但页面不恢复**——
    与微软文档一致（BrowserProcessExited 后必须重建 WebView），日志中也确实没有
    "reload 指令发送失败"，即恢复动作静默失败。实测复现：强杀 `msedgewebview2.exe`
    浏览器进程后窗口整体变黑（无障碍元素 175 → 9），应用主进程存活、日志继续写，
    用户只能手动关闭并重启应用。
  - **修复**：`handle_process_failed` 按崩溃种类分流——`BROWSER_PROCESS_EXITED` 走新的
    `attempt_app_restart`（自动重启应用，等效用户手动重启），`RENDER/FRAME_RENDER_PROCESS_EXITED`
    仍走 reload（此状态 WebView 仍有效）。重启带跨进程限流（`webview_restart_guard` 落盘记录，
    10 分钟窗口最多 2 次），防止"必现故障 → 无限重启"闪屏循环；超限只留 ERROR 日志。
  - **诊断链补齐**：`ProcessFailed` 事件补读 `ICoreWebView2ProcessFailedEventArgs2` 的
    `Reason` / `ExitCode`（原实现只记 kind）——Reason 直接区分 进程崩溃 / 内存不足 /
    进程被外部终止 / 未预期退出，是"为什么退出"的第一手证据；此前多次崩溃只知
    "浏览器进程退出"，无法定性。
  - **验证**：`cargo check` 通过；`cargo test` 255 + 1 用例全绿（新增
    `重启防护放行至上限后拒绝且状态不变`、`重启防护窗口过期后旧记录被丢弃` 两个单测）。


- **2026-09-11 安卓端新手引导整体缺失（手机重做 UI / 平板接入适配）** — 模块
  `android/frontend/src/settings/`、`App.tsx`、`TabletShell.tsx`、`MobileMore.tsx`。
  - **现状（"不可用"的具体表现）**：手机外壳不渲染向导（`App.tsx` 顶部注释把它列在"桌面件"里），
    首次无账号直接跳账号页并**立刻**写 `campus-onboarding-done`——等于没有引导，用户不知道
    填学号还是手机号；平板外壳同样未接入；设置面板"打开新手指引"按钮在 `MobileMore.tsx:71`
    被传成空函数 `() => {}`，点击无任何反应；向导本体是固定 640×640 的桌面 Dialog，
    手机窄屏（360-430dp）下必然挤压不可用。
  - **做法**：抽 `useOnboardingFlow`（5 步状态机、账号/适配器校验、绑定运营商、登录收尾、
    配置落盘——单点维护，两端不会各自漂移）；手机新建全屏向导 `OnboardingWizardMobile`
    （段式进度轨 + 48px 触控目标 + `env(safe-area-inset-*)` + 底部固定操作区 + 去 autoFocus）；
    平板复用桌面 Dialog 向导，尺寸由固定 640×640 收敛为 `min(640px,92vw)` × `min(640px,86vh)`；
    两外壳的引导标记改由向导在「跳过」或「登录成功」时写入（未走完则下次启动继续引导）；
    `onShowOnboarding` 从外壳贯通到 `MobileMore`/`TabletShell` 的设置面板。
  - **验证**：`npx tsc --noEmit` 通过；`npm run build`（tsc + vite）无警告；浏览器实测
    （360×800 手机 + 600×960 平板视口，临时注入 `__TAURI_INTERNALS__` mock，**用后已完整还原、
    `git diff` 干净**）——手机 5 步逐页截图确认布局与触控目标，平板 Dialog 实测收敛到
    552×640 居中不溢出。实测中发现并修复完成页「上一步」误用 `w-full` 把「开始登录」
    挤出容器的缺陷。


- **2026-09-11 安卓 WiFi+流量同开无法登录（全链路 WiFi 绑定修复）** — 模块
  `android/plugins/network-bind`、`android/src-tauri/src/protocol_cmds.rs`、
  `tauri-app/src-tauri/src/network/client.rs`。
  - **上一版为何没解决**（逐条有代码/文档证据，都不是"参数没调对"，而是实现层缺陷）：
    1. **手动登录链路根本没绑**：`do_login` 与 `check_portal_status` 里没有
       `ensure_wifi_bound`（只有启动自动登录、monitor 每拍、`do_logout` 和两个探测命令有）。
       用户手点登录/手动探测时进程仍绑在系统默认路由上，双网同开即落蜂窝。
    2. **绑定对已有连接无效**：`bindProcessToNetwork` 的 fwmark 由 netd 在 **socket
       创建时**打标（官方文档："All Sockets created in the future…"），而 HTTP
       `CLIENT_POOL` 里 keep-alive 的旧连接是绑定前建的、TTL 600s 内继续复用——
       绑定对它们完全无效。
    3. **绑定可能直接失败**：Kotlin 用 `allNetworks.firstOrNull{TRANSPORT_WIFI}` +
       `bindProcessToNetwork`，该组合在 Android 12+ 与部分 OEM 设备上会返回 false
       （WiFiFlutter#296、Android issuetracker#249023377），官方推荐在 `requestNetwork`
       的 `onAvailable` 回调里绑定；且原实现只校验 TRANSPORT_WIFI、不校验
       `NET_CAPABILITY_INTERNET`，可能绑到系统判定"无互联网"的 WiFi 上（比不绑更糟）。
    4. **失败无从查证**：`ensure_wifi_bound` 用 `eprintln!` 只进 logcat，用户能导出的
       日志文件（应用内日志面板）里看不到绑定成败与原因。
  - **修复**：① `do_login` / `check_portal_status` 补 `ensure_wifi_bound`，五个环节
    （登录/周期探测/断线重连/注销/手动探测）全部先绑定；② 绑定成功后
    `clear_client_pool()` 清空 HTTP 连接池，强制后续请求新建 socket 以吃到新 fwmark；
    ③ Kotlin 侧加 `NET_CAPABILITY_INTERNET` 校验，并新增 `requestNetwork`（3s 超时）
    回退路径，在 `onAvailable` 内绑定；返回 `{bound, path, reason}` 供诊断；
    ④ 绑定成败改 `log_info!/log_warn!` 落盘（能进日志文件），带 path/reason。
  - **验证**：`cargo check --target aarch64-linux-android` 通过（mobile cfg 全编译）；
    Windows 侧 `cargo check --lib` 通过；Kotlin 经
    `gradlew :tauri-plugin-campus-network-bind:compileReleaseKotlin` 编译通过
    （首次编译即抓出 `requestNetwork` 超时参数类型错误 Long→Int 并修正）。
  - **遗留**：需真机在"WiFi（校园网未认证）+ 流量同开"场景复验；本机无设备/无 adb，
    仅做到交叉编译与 Kotlin 编译级验证。下次复现时日志文件里会有 `[wifi-bind]` 行为证。


- **2026-09-11 安卓校园网绑定失败真机复现与修复（captive portal + VPN 场景）** — 模块
  `android/plugins/network-bind`、`android/src-tauri/src/protocol_cmds.rs`。
  - **真机证据**（Redmi / Android 16，应用内日志面板）：
    `WARN [wifi-bind] 未绑定 WiFi（reason=bind_rejected_request_failed），本次回落系统默认路由`
    （22:40:08.946 起连续多条），紧接着
    `WARN [login] 登录请求失败 [2/3]: error request for url (http://10.1.99.100:801/epertal/…)`
    ——绑定失败 → 回落蜂窝 → 校园内网 Portal 不可达，链路完全对上。
  - **根因（四个环节，均有依据）**：
    1. **筛选条件把校园网 WiFi 排除了**：`hasCapability(NET_CAPABILITY_INTERNET)`
       会把认证前（captive portal）的校园 WiFi 筛掉——系统此时不给它 VALIDATED，
       部分 ROM 连 INTERNET 也不给。上一版"绑定被拒"的直接原因之一。
    2. **`requestNetwork` 带了 NET_CAPABILITY_INTERNET**：AOSP WifiNetworkFactory 对含该
       能力的 WiFi 请求直接拒绝（"Request with wifi network specifier cannot contain
       NET_CAPABILITY_INTERNET. Rejecting"），表现为 onUnavailable 或异常。
    3. **VPN 活动时 `bindProcessToNetwork` 必然返回 false**（社区与 Google issue 的首要
       已知原因）；真机 `dumpsys connectivity` 中确有 `Transports: VPN`。另有 Android 12+
       上该 API 失效的平台 bug（issuetracker#249023377、WiFiFlutter#296）。
    4. **系统级前置条件**（用户真机反馈 + AOSP 源码确认）：无互联网的 WiFi 只有在用户
       确认后（`NetworkAgent.explicitlySelected(acceptUnvalidated)`，即系统弹窗里的
       "仍然连接"）才会被系统当作默认可选网络，否则设备保持走蜂窝——应用无法绕过。
  - **修复**：① 选网改为"优先有 INTERNET 能力的 WiFi，没有再退到任意 WiFi"；
    ② `requestNetwork` 显式 `removeCapability(NET_CAPABILITY_INTERNET)`；
    ③ catch 中带出异常类名与 message（上一版丢弃了异常类型，只留 `request_failed`，
    无从定位）；④ 新增 VPN 检测，reason 明确标注 `bind_rejected_vpn_active`；
    ⑤ 绑定成功时 reason 变为所选网络的能力摘要（net/nonet+val/unval+cp）并落盘。
  - **验证**：Kotlin `gradlew :tauri-plugin-campus-network-bind:compileReleaseKotlin` 通过；
    `cargo check --target aarch64-linux-android` 通过。
  - **使用前提（需告知用户）**：校园网登录需 (a) 关闭 VPN 或让 VPN 排除本应用，
    (b) 在系统"此网络无法访问互联网"提示中选择"仍然连接"。
