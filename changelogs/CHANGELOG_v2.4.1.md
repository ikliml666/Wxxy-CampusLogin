# 更新日志

## v2.4.1（开发中）

### 行为调整

- **UI：双端悬浮导航栏对齐 FlClash 形态（桌面 + 安卓同步）**（2026-10-06）。
  - **页签：图标+短标签竖排常显**（参照 FlClash 截图：单行胶囊、每项图标下方 2-4 字标签、选中项整项背景块）。i18n 缩词定案（双端 zh 同步）：dashboard 总览、account 账号、selfservice 自助服务、network 网络（原"网络适配器"）、monitor 监测（原"网络状态检测"）、quality 质量（原"网络质量"）、speedtest 测速、settings 设置、log 日志、more 更多；en 同步定案 Home/Account/Service/Network/Monitor/Quality/Speed/Settings/Logs/More（原 Dashboard/Self-Service/Network Adapters/Network Monitor/Network Quality/Speed Test）。
  - **药丸零变形**：按钮改等宽（桌面 `w-[52px]`、安卓 `h-14 w-16`），激活药丸从 40×28/52×32 的图标盒改为按钮级 `absolute inset-0 rounded-full` 包整项——等宽按钮间 `layoutId` 共享布局动画退化为纯平移，宽度过渡路径消失，切换不再有任何拉伸变形。
  - **切换动画放慢可感知**：spring stiffness 420/damping 34 → 250/28（双端同参），背景块左右滑动约 0.5s 可感知滑动而非瞬间跳变。
  - **桌面毛玻璃**：`.glass-dock` 补 `backdrop-filter: blur(16px) saturate(150%)`（含 -webkit- 前缀），底色透明度 0.82→0.68（dark 0.85→0.72），低开销真实透出窗口下内容；安卓底栏已有磨砂（blur 24px/72%）不重复改。桌面 hover tooltip 删除（标签常显后冗余），`aria-current="page"` 补激活语义，GSAP 磁吸（MAGNETIC_RANGE=80/MAX_SCALE 1.35/MAX_LIFT -14）与 `motion-reduce` 降级原样保留。
  - 涉及：`tauri-app/frontend/src/components/layout/DockNav.tsx`、`android/frontend/src/components/layout/BottomNav.tsx`、`tauri-app/frontend/src/index.css`（.glass-dock）、双端 `i18n/locales/{zh,en}.json` nav 区。验证：双端 `npx tsc --noEmit --incremental` 0 错误；vite dev + `mockIPC` 浏览器实测（headless Chrome）：9 项标签常显且严格等宽 52px、dock 653×57、点击「网络」后 `aria-current` 正确迁移且药丸全局唯一落在激活按钮内；视觉截图与 FlClash 参照一致。

### 缺陷修复

- **修复：最小化期间夜间自动切换出站后，恢复窗口时适配器面板的当前出站徽标/列表陈旧**（2026-10-06，桌面）。
  - **根因（三因素叠加）**：① WebView2 最小化时定时器被系统节流，60s 兜底轮询失效；② 面板刷新时机之一是 `adapters.length` 变化，而自动切换只改路由 metric/禁卡，数组长度不变；③ 后端 `outbound_switch_now` 完成后不向前端 emit 事件。三者叠加使恢复窗口的展示层与真实出站状态脱钩，且无自愈路径。
  - **修复**：NetworkPanel 新增恢复重探——`syncOnRestore`（`lastFocusSyncRef` 5s 节流去重）挂 `visibilitychange`（visible 时）+ `window focus` 两监听，触发即 `refreshAdapterData({force:true,includeDisabled:true})` + `refreshCurrentOutbound()`；节流防两事件连发双探。手动切换按钮路径已有刷新，不重复加。
  - 验证：mockIPC 浏览器实测（headless Chrome）：恢复可见性派发 → `get_adapters:force`/`get_adapter_details`/`get_disabled_adapters`/`get_current_outbound_name` 全数发出；5s 后再次派发仍可触发（节流窗口过后不卡死）。机制记录见 CodeWiki `learnings/webview-minimize-timer-throttle-stale-panel`。

- **修复：适配器下拉出现未翻译 key「network.status.已禁用」、同名卡双段双勾选与触发器文本拼接**（2026-10-07，桌面）。
  - **根因（三层叠加）**：① 后端 `DisabledAdapter.status` 存中文文案（`as_str()`），前端统一按 `t(` `` `network.status.${a.status}` `` `)` 拼 i18n key，miss 后回退显示原始 key（`Adapter.status` 是 serde 枚举一直正常——两个平行类型不同源）；② `adapter_watch` 15s 轮询差分推送存在启动竞态：启动对账期间前端初始加载拉到禁用快照，随后 netsh 启用两卡，而轮询首轮 baseline 已是启用后状态、`disabled_changed=false` 不推 `onDisabledAdaptersChanged`，前端禁用列表自启动起永久冻结；③ 陈旧快照使同一张卡同时出现在「有 IP」段与「已禁用」段，两个 SelectItem value 同名撞车 → Radix 双勾选 + 触发器拼接「以太网以太网（network.status.已禁用）」。
  - **修复**：① `DisabledAdapter.status: String → AdapterStatus`（与 `Adapter.status` 同源序列化为 `disabled`，文案归前端 i18n；windows.rs 直接赋枚举并删除 `as_str()`；禁用警告 toast 文案改字面量「已禁用」；新增序列化契约测试锁住 `json["status"] == "disabled"`；双端 types.ts 同步收窄）；② 轮询加 `first_round` 首轮无条件推送，消除启动竞态窗口；③ **下拉禁用段改为从 adapters 单通道派生**（`adapters.filter(a => a.status === 'disabled')`：后端把所有卡含禁用都推入 adapters，其刷新通道——60s 轮询/事件推送/操作后 force——远比 disabledAdapters 活跃，单源派生后两条通道短暂不一致影响不到下拉，三段互斥由构造保证、Select value 全局唯一；disabledAdapters store 仅保留给空态判断兜底）；④ NetworkPanel 挂载时对账拉取一次（页签切换不触发 visibilitychange/focus，其他页签停留期间错过的事件靠挂载对账补齐）。
  - 验证：`cargo check --all-targets` 0 错误；`cargo test` 443 passed / 0 failed（含新增 `disabled_adapter_status_serializes_as_enum_key`）；双端 `npx tsc --noEmit --incremental` 0 错误。机制记录见 CodeWiki `learnings/disabled-adapter-status-enum-and-startup-race`。

### 文档

- **文档：新增 iOS 端全量迁移规格 `specs/ios-port-spec.md`**（2026-10-07）。
  - **背景**：用户提出 iOS 版开发需求（iPhone 全机型 + iPad 适配、功能全量迁移、参考 VPN 软件路线）；经方案对比与网络调研定案「方案 A · 三层递进」：L0 基础层（Tauri iOS 壳 + 协议核心 path 依赖继承 + 前台全功能 + Keychain/Face ID/通知适配件 + GitHub Actions 免费构建）→ L1 App Intents 快捷指令自动化（连 WiFi 自动登录，零特殊 entitlement）→ L2 NetworkExtension 隧道层（Packet Tunnel Provider + Rust staticlib FFI 保协议单源，远期可选里程碑，需付费账号 + entitlement 申请）。
  - **内容**：北极星与非目标、八项放弃与降级清单（网卡控制族/自启动/FGS 保活/ICMP→TCP connect/SSID 需 hotspot entitlement/更新降级跳转等，每项附依据）、目录架构（对称 `android/`）、命令面与事件策略（与安卓同名对齐）、验证基线分层、M0-M4 里程碑验收锚、五项风险。
  - **验证**：`git ls-files` 确认 specs/ios-port-spec.md 入库；`feat/ios-port` 分支 `--ff-only` 合并回 main（`bf27cc5..cb38b5c`）。注：`docs/` 为 .gitignore 有意忽略的本地文档区，spec 落 `specs/` 以便版本化；initiative 记录于 Hindsight kp-a6b6ea38。
  - **评审修订（v2，同日）**：delegate 审查判定「修改后批准」，5 项阻断经主 agent 逐一核实属实并全部修订——①`tauri-app/src-tauri/.cargo/config.toml:7` 绝对 target-dir（Windows Low IL workaround）会打断 macOS CI → §4 增构建配置节（ios 壳自带 .cargo/config.toml + CI `CARGO_TARGET_DIR` 双保险，M0 验收锚补齐）；②缺 `NSLocalNetworkUsageDescription`（核心对 http://10.1.99.100 与 http://10.1.80.200:8080/Self 本地网络直连，iOS 14+ 拒绝授权即静默失效）→ §6 补 Info.plist 适配件、§10 置顶风险；③§3/§5 矛盾：accept_wifi_network / get_avoid_bad_wifi_status / restore_written_settings 依赖 WRITE_SECURE_SETTINGS 写 Settings.Global，iOS 无对应 → §5 三档处置表（保留 ~38 / 换实现 ~8 / 裁剪 ~9）显式裁剪；④face_model_cmds 2 条 + battery_cmds 4 条 + boot_autostart 2 条零处置 → 逐一处置（人脸栈裁剪、Face ID 替代 2D 人脸门 allow2dFaceVerify 置灰、battery 3 裁 1 换实现）；⑤ICMP 实际落点在协议核心（quality.rs surge_ping + subnet.rs→icmp_probe.rs），ping_test 为空壳 → 修订为在协议核心加 cfg(ios) TCP 探测分支（合规单源铁律）。另吸收建议级：AppIntents 后台 30s 上限、NEHotspotNetwork 需定位授权、sing-box 形态为主 App 与 provider 共用 xcframework、M1 锚改 CI `cargo check --target aarch64-apple-ios`（host test 仅协议核心回归佐证）、PrivacyInfo.xcprivacy、Tauri 2.11.5 iOS 缺陷 #16130/#11764 待 CI 实测、rustls ring→aws-lc-rs fallback、锚点行号修正。commit `1f9b783`（feat/ios-port-spec-v2 → main ff）。

### iOS 端

- **M0 骨架落地：`ios/` 三目录 + GitHub Actions iOS 构建流水线**（2026-10-07，分支 `feat/sess-session--ios-m0-skeleton`）。
  - **内容**：`ios/src-tauri` 壳 crate（对称 `android/`，path 依赖协议核心，`staticlib` 供 Xcode 链接）、`ios/frontend` 复刻树（自 `android/frontend` 派生，port 5175）、`ios/plugins` 占位（M1 增 keychain 等适配件）、`.github/workflows/ci-ios.yml`（macos-15：前端构建→缺则 `tauri ios init` 并自动提交 gen/apple 回 main→模拟器构建→冒烟截图 artifact）。
  - **本机验证**：前端 `npm ci`+`tsc`+`vite build` 干净（8.19s）；壳 crate host `cargo check` 全绿（1.64s），`Cargo.lock` 生成。
  - **依赖统一化攻坚（重要经验）**：新 lock 解析出的 tauri 生态版本与桌面 lock 基线漂移引发三类编译失败，逐一定位——①wry 0.56+ 拉 webview2-com 0.39 与协议核心直接依赖的 0.38 撞双版本致 E0599（cast）；②tauri-runtime-wry 2.12.x 的 `primary_monitor` 签名变更致 tauri 2.11.0 编译失败 27 错；③**tauri-macros 漂到 2.7.1**（为 2.12 准备，其 `do_menu_item!` proc 宏展开引用 `Error::UnexpectedMenuKind`，2.11.0 的 error.rs 无此 variant）——全部以「统一化锚点」方式钉回桌面 lock 基线（tauri =2.11.0 / tauri-build =2.6.0 / tauri-macros =2.6.0 / tauri-utils =2.9.0 / tauri-runtime(-wry) =2.11.0 / tao =0.35.2 / muda =0.19.1），iOS 升级节奏跟随桌面 lock（同构策略）。教训：`cat >>` 追加依赖须确认落点 section（曾误追加进 [profile.release] 段静默失效）；biometric 插件是 `#![cfg(mobile)]` 空 crate，host 下无 `init`，须放 `#[cfg(mobile)]` 块调用（同安卓 lib.rs 结构）。
  - **待办**：CI 首跑需 push 到 origin 触发（本地领先 origin 若干提交，按纪律待用户授权 push）。

### 移除

- **移除 iOS 端全部文件与本地分支**（2026-10-08）。
  - **背景**：CI 攻坚 8 轮后（gen/apple 自动入仓、xcode-script npm ENOENT 修复、Build Rust Code phase Abort trap 6 未查明），用户决定终止 iOS 端开发。
  - **内容**：git rm ios/ 全目录（壳 crate campus-login-ios、frontend 复刻树、gen/apple Xcode 工程、icons、plugins）、specs/ios-port-spec.md、.github/workflows/ci-ios.yml；.gitignore 移除 ios/src-tauri/target-ios/ 行并删除磁盘编译产物；本地分支 feat/ios-port、feat/ios-port-spec-v2、feat/sess-session--ios-m0-skeleton 全部 `git branch -d` 删除（均已合并，无提交丢失）。
  - **验证**：删除提交 4edfd31 已推送 origin/main（6fd8fbb..4edfd31）；gh api 确认远端 ios/ 与 .github/workflows 均已 404 不存在；删除提交未触发 ci-ios（该 commit 上 workflow 文件已不存在）；本地工作区仅剩与 iOS 无关的遗留未跟踪项（.probe/、pnpm-lock.yaml）。
  - **保留**：changelog 此前 iOS 条目为历史记录不回删；Hindsight initiative kp-a6b6ea38 已更新为终止状态；GitHub Actions 页面保留 8 次 ci-ios run 历史（git 操作无法清除）。
