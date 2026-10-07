# iOS 端全量迁移规格（Spec）

状态：v2 · 评审修订版 · 2026-10-07 · 分支 `feat/ios-port-spec-v2`
前置：用户已批准方案 A（三层递进）与 L2 远期定位；initiative 记录于 Hindsight kp-a6b6ea38。
v2 修订：吸收独立评审（deepseek-flash@high）5 项阻断发现（均已对照代码核实属实）——本地网络权限缺失、55 条命令面 14 条零处置、`.cargo/config.toml` 绝对 target-dir、ICMP 改造落点未明、锚点行号错误。

## 1. 目标与非目标

**北极星**：iOS 端（iPhone 全机型 + iPad）与安卓端同构的校园网登录助手。协议核心 crate 单源继承（`decisions/protocol-core-single-source` 铁律：登录/注销/Portal/自助服务/网络质量只存在于桌面 crate，iOS 经 Cargo path 依赖复用，禁止复制协议逻辑；**新增探测分支同样只写进桌面 crate**，见 §3 ICMP 条）。

**范围**：安卓端功能面全量迁移到 iOS——以移动端共同边界与 iOS 平台限制两项明示豁免/降级为前提（见 §3 放弃与降级清单、§5 命令面逐组处置）。

**非目标**：网卡适配器族、DHCP、出站切换、系统 DNS 设置（L2 DNS Proxy 是替代形态而非恢复）、自启动、前台服务保活、Android 系统网络设置写入类命令——这些在 iOS 无底层机制或属桌面专属。

## 2. 调研依据（2026-10-07 实测检索 + 评审核实）

1. **快捷指令自动登录是成熟路径**：GitHub 先例 OUC-autoLogin（iOS 快捷指令+自动化）、GUET 校园网登录脚本等一批项目，形态均为「加入 WiFi 自动化 → HTTP 登录」。
2. **NetworkExtension 路线存在但有门槛**：`com.apple.developer.networking.networkextension` entitlement 需向 Apple 申请（付费开发者账号）；Packet Tunnel Provider 是独立于 app 的系统级 extension 进程，须 Swift/ObjC 编写（Apple 文档、newly.app、kean.blog）；App Store Connect 打包时强校验该 entitlement。
3. **extension 内跑 Rust 核心可行，形态是共用框架而非跨进程调用**：sing-box iOS（开源）的 Packet Tunnel Provider 与主 App **共用同一 xcframework、各自链接**（非 provider 内跨进程 FFI）；对应本项目 = Rust 编 `staticlib`/xcframework，主 App 与 provider 分别链接，协议单源不破。
4. **构建环境**：本机 win32 无 Xcode，dsh-ios 插件不可用（ios_sim_devices 实测报错）；iOS SDK 只随 Xcode 分发 → target 级构建必须 macOS。本仓库为公开仓库，GitHub Actions macOS runner 免费。TLS 栈为 reqwest `rustls-tls`（ring provider，`android/src-tauri/Cargo.toml:50`）；ring 0.17+ 官方支持 aarch64-apple-ios，交叉编译在 CI 首验，备选 fallback aws-lc-rs。
5. **App Intents 后台执行上限 30 秒**（官方口径），登录 HTTP 为秒级，够用；「加入 WiFi 触发」走快捷指令自动化（iOS 16+）。
6. **NEHotspotNetwork.fetchCurrent 返回 SSID 的官方条件**：app 已用 `NEHotspotConfiguration` 配置过该网络，**或**用户已授予精确定位权限（Apple 文档）——hotspot entitlement 与定位授权双门槛，均不影响 L0/L1 主线（降级网关探测判定）。

## 3. 放弃与降级清单（iOS 平台限制，明示）

| 功能 | 处置 | 依据 |
| --- | --- | --- |
| 网卡枚举/禁用/DHCP/出站切换/DNS-DoH 系统设置 | 放弃（移动端共同边界，安卓亦无） | iOS 沙盒无网络接口控制权 |
| Android 系统网络设置写入族：`accept_wifi_network` / `get_avoid_bad_wifi_status` / `restore_written_settings`（`android/src-tauri/src/protocol_cmds.rs:196/215/235`） | **裁剪**（v2 修订：原 spec 与「全量注册」矛盾） | 依赖 `WRITE_SECURE_SETTINGS`（signature|privileged 权限）写 `Settings.Global`，iOS 无对应机制 |
| 自启动（含 `get/set_boot_autostart` 命令） | 放弃并显式裁剪命令（v2 修订：不再笼统归入后台监测） | iOS 无此概念 |
| FGS 前台服务保活/电池优化族 | 放弃；`battery_cmds` 4 条中 3 条裁剪（信息/申请/厂商页），`open_notification_settings` 换 iOS `UIApplication.openSettingsURLString` 跳转实现 | iOS 无前台服务与厂商电池页 |
| 人脸模型族：`face_models_state` / `face_models_download`（`android/src-tauri/src/face_model_cmds.rs:121/126`） | **裁剪**（v2 修订：整组补处置）；身份验证门直接用 tauri-plugin-biometric（Face ID）替代 2D 人脸校验，`allow2dFaceVerify` 配置字段置灰 | iOS 无对应模型栈；200MB 素材下载与相机权限成本不值，Face ID 是平台原生等价物 |
| 后台 60s 常驻监测 | 降级：前台全功能 + 快捷指令触发（L1）+ L2 远期隧道；`monitor_loop` 其余命令保留 | iOS 后台执行模型 |
| ICMP 探测原语 | **协议核心 crate 内加 cfg(ios) 分支换 TCP connect**（v2 修订：明确落点）：`network/quality.rs:48-52`（surge_ping）与 `network/subnet.rs` → `platform/icmp_probe.rs`（仅 Windows 注释）为真实 ICMP 所在；iOS 分支写进桌面 crate（合规于单源铁律），壳 crate 零复制；`ping_test` 命令实为空壳（`protocol_cmds.rs:171` 返回 "pong"），不动 | iOS 无 ICMP socket |
| WiFi SSID 读取 | 可选增强：NEHotspotNetwork + hotspot entitlement（需申请）+ 定位授权（见 §2.6）；无 entitlement 时降级网关探测判定 | Apple 双门槛 |
| 应用内更新 | 降级：版本检查 + 跳转 Release/TestFlight（`update_cmds` 4 条逐一换实现，下载/安装条裁剪） | iOS 禁止应用内装包 |
| bind_to_wifi / avoid_bad_wifi 命令族 | 裁剪（iOS 无进程级网络绑定） | 平台机制缺失 |

## 4. 目录与架构（对称 `android/`）

```
ios/
├── src-tauri/          # 壳 crate campus-login-ios（对称 android/src-tauri）
│   ├── Cargo.toml      # path 依赖 campus-login（对齐 android/src-tauri/Cargo.toml:34 模式）
│   ├── .cargo/config.toml  # 见下「构建配置」——必须自带，不继承桌面绝对路径
│   └── src/            # 平台探针/监控编排/命令包装/落盘记录 四类平台件
├── frontend/           # 以 android/frontend 为起点派生的复刻树
├── plugins/            # campus-keychain（Swift）等 iOS 专属插件
└── gen/                # tauri ios init 生成的 Xcode 工程（AppIcon/LaunchScreen 资产从 android/src-tauri/icons 对应重制；NE extension target L2 时加入）
```

- 桌面 crate 门控已预留：`tauri-app/src-tauri/Cargo.toml:53-57` 为 `cfg(not(any(target_os = "android", target_os = "ios")))`，iOS 编译桌面专属插件天然不可见，门控零改动。
- TLS 栈沿用 reqwest `rustls-tls`（对齐 `android/src-tauri/Cargo.toml:49-50` 注释与配置的交叉编译考量；v2 修正锚点，原 :56 有误）。

**构建配置（v2 新增，M0 前置）**：`tauri-app/src-tauri/.cargo/config.toml:7` 的 `target-dir = 'E:\ik\...'` 是 Windows 沙箱 Low IL workaround（背景见 `decisions/cargo-target-outside-labeled-tree`），Cargo 向上搜索 `.cargo` 时该绝对路径会被下游 crate 继承。iOS 壳 crate 与 CI 必须：`ios/src-tauri/.cargo/config.toml` 自设 target-dir，**且** `ci-ios.yml` 显式 `CARGO_TARGET_DIR` 环境变量双保险，否则 macOS runner 上构建直接失败。

## 5. 命令面与事件（v2 重写：逐组处置）

安卓注册面实为 `android/src-tauri/src/lib.rs:58-114`（generate_handler 起 :58，止 :114；55 条命令，v2 修正行号）。iOS 处置规则：

| 处置 | 命令组 | 条数 |
| --- | --- | --- |
| 保留（同名注册，语义不变） | do_login/do_logout、check_portal_status、self_service 7 条、account 6 条、config_state 2 条、quality_cmds 3 条、campus_detect 的 detect_campus/check_campus_status、system_cmds 大部（日志/初始化/心跳等）、monitor_loop 的监测控制与通知开关 | ~38 |
| 换实现（保留契约，iOS 原语重写） | campus_detect 探测内核（TCP connect）、open_notification_settings（openSettingsURLString）、update_cmds 4 条（检查保留/下载安装裁剪换跳转）、事件推送面 | ~8 |
| 裁剪（iOS 无机制，不注册） | WiFi 系统设置族 3 条（§3）、bind_to_wifi、boot_autostart 2 条、battery 3 条、face_model 2 条、ping_test 视 M1 核对结论（空壳，倾向裁剪） | ~9 |

- **跨端映射例外**（v2 补注）：身份验证命令 iOS 沿用安卓名 `verify_biometric_identity`（桌面为 `verify_windows_identity`，是既有 3 对跨端映射之一），对齐基准是安卓而非桌面。
- **55 条逐条对照表**在 M1 实施计划中产出并作为验收锚（spec 自身只锁分组规则与零处置清单，避免与代码漂移）。
- **事件总线**：安卓 7 个真实事件（config-changed / face-models-download-progress / login-log / background-check-result / update-available / update-download-progress / auto-login-result）中，face-models-download-progress 随命令族裁剪，iOS 首批 6 个。
- **配置字段**：iOS 前端 `settings/types.ts` 从安卓 47 字段树派生；`allow2dFaceVerify` 置灰保留。

## 6. 适配件（iOS 专属，v2 补 Info.plist 族）

| 件 | 方案 | 对标安卓件 |
| --- | --- | --- |
| 密钥存储 | campus-keychain（Swift，Keychain；加密落盘语义对齐） | plugins/keystore |
| 生物识别 | tauri-plugin-biometric 官方插件直接继承（Face ID） | 同插件（安卓已用） |
| 通知 | tauri-plugin-notification 继承（UNUserNotificationCenter） | 同插件 |
| 校园判定 | campus_detect iOS 分支：TCP connect 网关/Portal 探测（落桌面 crate cfg(ios)） | campus_detect.rs |
| **本地网络权限**（v2 新增，阻断级） | Info.plist 声明 `NSLocalNetworkUsageDescription` + 首启引导授权；**用户拒绝后核心登录/自助全线静默失败且无降级**——引导文案与失败提示必须显式覆盖 | 安卓无对应（局域网直连免权限） |
| **明文网络策略**（v2 新增） | 核心请求由 Rust reqwest 发起，ATS 不约束；但 ATS `NSExceptionDomains` 对 IP 直连架构性无效（Apple 文档原文），WebView 内加载内网 http 页不可行——iOS 端不做 WebView 内网页面，如未来需要只能 `NSAllowsArbitraryLoads`（审核风险） | 安卓 usesCleartextTraffic |
| 自动化（L1） | App Intents：登录/查询两个 Intent（后台执行上限 30s，登录 HTTP 秒级够用）+ 快捷指令引导页 | 无（iOS 新增） |

## 7. 前端与设备适配

- 复刻树从 `android/frontend` 派生：i18n 文案（`android/frontend/src/i18n/locales` 全量继承，App 名本地化补 `InfoPlist.strings`）、面板结构、hooks 全量继承；前端栈 React 19 + Tailwind + GSAP + framer-motion（双端同版本）经评审核实为 WKWebView/Safari 15+ 良好支持面，无兼容风险项。iOS 差异集中在 safe-area（底部悬浮导航 home-indicator 安全区）、平板双列（复用 `decisions/android-tablet-two-col-align` 经验）。
- 机型矩阵（CI 模拟器）：iPhone SE（最小尺寸）+ iPhone Pro Max（最大尺寸）+ iPad Pro（平板双列）。

## 8. CI 与验证基线（对齐 decisions/verification-baseline 分层）

| 层 | 环境 | 命令/产物 |
| --- | --- | --- |
| 协议核心回归 | 本机 win32 | `cargo test`（tauri-app/src-tauri，基线 397+ 用例）——**仅验证共享逻辑，不作为 iOS 命令面证据**（v2 注：安卓 host check 即失败是先例，mobile-only 门控） |
| iOS target 编译 | GitHub Actions macOS | `cargo check --target aarch64-apple-ios --all-targets`（M1 新增锚，v2） |
| iOS 应用构建 | GitHub Actions macOS | tauri iOS 模拟器构建 + 三机型模拟器冒烟 + 截图工件 |
| 前端 | 本机 | `npx tsc --noEmit --incremental`（ios/frontend；禁 `tsc -b`） |
| 真机/交互 | 用户侧（M4） | 真机调试（免费 Apple ID，7 天签名）或 TestFlight（$99/年） |

CI 工作流：`ci-ios.yml` —— job1（windows）host 回归；job2（macos）`CARGO_TARGET_DIR` 覆盖 + iOS 构建 + 冒烟截图。

## 9. 里程碑与验收锚

- **M0 骨架**：`ios/` 三目录就位（含 `ios/src-tauri/.cargo/config.toml`）；`cargo tauri ios init` 产物入仓；CI macOS job 出模拟器包成功（验收：Actions 绿 + 截图工件 + 构建日志无 target-dir 路径错误）。
- **M1 后端**：55 条逐条对照表产出；命令面按 §5 注册完成；**CI `cargo check --target aarch64-apple-ios --all-targets` 绿**（验收：对照表 + CI 输出；host `cargo test` 仅作协议核心回归佐证）。
- **M2 前端**：复刻树迁移完成，三机型模拟器截图通过（验收：截图 + tsc 干净）。
- **M3 自动化（L1）**：App Intents 登录 Intent 可被快捷指令触发（验收：触发→登录成功→通知结果 链路实录；30s 上限实测确认）。
- **M4 隧道（L2，远期可选）**：付费账号 + entitlement 申请获批后立项；Packet Tunnel Provider + Rust xcframework（主 App 与 provider 各自链接，协议单源不破）；单独立项另写 spec。

## 10. 风险与开放问题（v2 扩充）

1. **本地网络权限拒绝**（v2 新增，最高优先）：用户拒绝 `NSLocalNetworkUsageDescription` 授权后核心功能静默失效——引导文案 + 失败提示是 M2 必做项。
2. AppIntent 后台执行 30s 上限（官方口径）需 M3 实测确认；登录 HTTP 秒级、风险低。
3. hotspot entitlement / NetworkExtension entitlement 申请结果不可控（均不影响 L0/L1，L2 前置条件）。
4. **Tauri iOS 成熟度**（v2 新增）：现用 tauri 2.11.5（`android/src-tauri/Cargo.lock`），上游 iOS 侧有活跃缺陷报告（如 #16130 Release 链接失败、#11764 ios dev 报错，评审检索所得，M0 以 CI 实测为准）；CI 需锁 Xcode 版本。
5. **PrivacyInfo.xcprivacy**（v2 新增）：iOS 17+ 上架硬要求（required-reason API 申报：UserDefaults/文件时间戳/磁盘空间等）；自用/TestFlight 不强制，上架前必补。
6. ring（rustls provider）iOS 交叉编译首验在 M0；失败则 fallback aws-lc-rs（评审建议，代码未动）。
7. Tauri iOS 工程加入 NE extension target 属 Xcode 工程级改动（L2 范围）。
8. 快捷指令自动化需用户一次性手动配置（app 内引导页缓解）。
