# iOS 端全量迁移规格（Spec）

状态：待用户评审 · 2026-10-07 · 分支 `feat/ios-port`
前置：用户已批准方案 A（三层递进）与 L2 远期定位；initiative 记录于 Hindsight kp-a6b6ea38。

## 1. 目标与非目标

**北极星**：iOS 端（iPhone 全机型 + iPad）与安卓端同构的校园网登录助手。协议核心 crate 单源继承（`decisions/protocol-core-single-source` 铁律：登录/注销/Portal/自助服务/网络质量只存在于桌面 crate，iOS 经 Cargo path 依赖复用，禁止复制协议逻辑）。

**范围**：安卓端功能面全量迁移到 iOS——以移动端共同边界与 iOS 平台限制两项明示豁免/降级为前提（见 §3 放弃与降级清单）。

**非目标**：网卡适配器族、DHCP、出站切换、系统 DNS 设置（L2 DNS Proxy 是替代形态而非恢复）、自启动、前台服务保活——这些在 iOS 无底层机制或属桌面专属，且安卓端本就不含网卡控制族。

## 2. 调研依据（2026-10-07 实测检索）

1. **快捷指令自动登录是成熟路径**：GitHub 先例 OUC-autoLogin（iOS 快捷指令+自动化）、GUET 校园网登录脚本等一批项目，形态均为「加入 WiFi 自动化 → HTTP 登录」。
2. **NetworkExtension 路线存在但有门槛**：`com.apple.developer.networking.networkextension` entitlement 需向 Apple 申请（付费开发者账号）；Packet Tunnel Provider 是独立于 app 的系统级 extension 进程，须 Swift/ObjC 编写（Apple 文档、newly.app、kean.blog）；App Store Connect 打包时强校验该 entitlement。
3. **extension 内跑 Rust 核心可行**：sing-box iOS（开源）证明「隧道 provider 进程内经 FFI 调非原生核心」架构成立；对应本项目 = Rust `staticlib`（aarch64-apple-ios）供 Swift provider 调用，协议单源不破。
4. **构建环境**：本机 win32 无 Xcode，dsh-ios 插件不可用（ios_sim_devices 实测报错）；rustls+ring 依赖含 C 代码，iOS SDK 只随 Xcode 分发 → target 级构建必须 macOS。本仓库为公开仓库，GitHub Actions macOS runner 免费。
5. **App Intents 后台执行时长**为官方支持路径（iOS 16+），具体时长上限实现期实测验证（登录 HTTP 为秒级，风险低）。

## 3. 放弃与降级清单（iOS 平台限制，明示）

| 功能 | 处置 | 依据 |
| --- | --- | --- |
| 网卡枚举/禁用/DHCP/出站切换/DNS-DoH 系统设置 | 放弃（移动端共同边界，安卓亦无） | iOS 沙盒无网络接口控制权 |
| 自启动 | 放弃 | iOS 无此概念 |
| FGS 前台服务保活/电池白名单 | 放弃，L2 隧道为远期替代形态 | iOS 无前台服务 |
| 后台 60s 常驻监测 | 降级：前台全功能 + 快捷指令触发（L1）+ L2 远期隧道 | iOS 后台执行模型 |
| ICMP 探测原语 | 换 TCP connect，对外命令契约不变 | iOS 无 ICMP socket |
| WiFi SSID 读取 | 可选增强：NEHotspotNetwork + hotspot entitlement（需申请）；无 entitlement 时降级网关探测判定 | Apple hotspot entitlement 门槛 |
| 应用内更新 | 降级：版本检查 + 跳转 Release/TestFlight | iOS 禁止应用内装包 |
| bind_to_wifi / avoid_bad_wifi 命令族 | 裁剪（iOS 无进程级网络绑定） | 平台机制缺失 |

## 4. 目录与架构（对称 `android/`）

```
ios/
├── src-tauri/          # 壳 crate campus-login-ios（对称 android/src-tauri）
│   ├── Cargo.toml      # path 依赖 campus-login（对齐 android/src-tauri/Cargo.toml:33-34 模式）
│   └── src/            # 平台探针/监控编排/命令包装/落盘记录 四类平台件
├── frontend/           # 以 android/frontend 为起点派生的复刻树
├── plugins/            # campus-keychain（Swift）等 iOS 专属插件
└── gen/                # tauri ios init 生成的 Xcode 工程（含 NE extension target，L2 时加入）
```

- 桌面 crate 门控已预留：`tauri-app/src-tauri/Cargo.toml:53-57` 为 `cfg(not(any(target_os = "android", target_os = "ios")))`，iOS 编译桌面专属插件天然不可见，门控零改动。
- TLS 栈沿用 rustls（对齐 `android/src-tauri/Cargo.toml:56` 注释的交叉编译考量）。

## 5. 命令面与事件

- **命令面**：安卓 `android/src-tauri/src/lib.rs:57-111` generate_handler 注册面逐条核对，iOS 可用面全量注册，命令名与安卓端同名对齐（延续 `decisions/ipc-command-name-alignment` 模式）；探测类命令（campus_detect/ping_test）内部实现换 TCP connect，对外契约不变。
- **事件总线**：沿用安卓「真实事件是桌面 16 事件子集」的模式（安卓 7 个），iOS 首批与安卓子集对齐，随命令面核对逐条确认。
- **配置字段**：iOS 前端 `settings/types.ts` 从安卓 47 字段树派生，增删随命令面裁剪同步。

## 6. 适配件（iOS 专属）

| 件 | 方案 | 对标安卓件 |
| --- | --- | --- |
| 密钥存储 | campus-keychain（Swift，Keychain；加密落盘语义对齐） | plugins/keystore |
| 生物识别 | tauri-plugin-biometric 官方插件直接继承（Face ID） | 同插件（安卓已用） |
| 通知 | tauri-plugin-notification 继承（UNUserNotificationCenter） | 同插件 |
| 校园判定 | campus_detect iOS 分支：TCP connect 网关/Portal 探测 | campus_detect.rs |
| 自动化（L1） | App Intents：登录/查询两个 Intent + 快捷指令引导页 | 无（iOS 新增） |

## 7. 前端与设备适配

- 复刻树从 `android/frontend` 派生：i18n 文案、面板结构、hooks 全量继承；iOS 差异集中在 safe-area（底部悬浮导航 home-indicator 安全区）、平板双列（复用 `decisions/android-tablet-two-col-align` 经验）、iOS 系统形态（无返回键语义差异等）。
- 机型矩阵（CI 模拟器）：iPhone SE（最小尺寸）+ iPhone Pro Max（最大尺寸）+ iPad Pro（平板双列）。

## 8. CI 与验证基线（对齐 decisions/verification-baseline 分层）

| 层 | 环境 | 命令/产物 |
| --- | --- | --- |
| 协议核心回归 | 本机 win32 | `cargo test`（tauri-app/src-tauri，基线 397 用例） |
| 前端 | 本机 | `npx tsc --noEmit --incremental`（ios/frontend；禁 `tsc -b`） |
| iOS target 构建 | GitHub Actions macOS | tauri iOS 模拟器构建 + 模拟器冒烟 + 截图工件 |
| 真机/交互 | 用户侧（M4） | 真机调试（免费 Apple ID，7 天签名）或 TestFlight（$99/年） |

CI 工作流：`ci-ios.yml` —— job1（windows）host 回归；job2（macos）iOS 构建 + 冒烟截图。

## 9. 里程碑与验收锚

- **M0 骨架**：`ios/` 三目录就位；`cargo tauri ios init` 产物入仓；CI macOS job 出模拟器包成功（验收：Actions 绿 + 截图工件）。
- **M1 后端**：§5 命令面注册完成且 host `cargo test` 全绿；keychain/detect 适配件落地（验收：命令面清单核对表 + 测试输出）。
- **M2 前端**：复刻树迁移完成，三机型模拟器截图通过（验收：截图 + tsc 干净）。
- **M3 自动化（L1）**：App Intents 登录 Intent 在模拟器/真机可被快捷指令触发（验收：触发→登录成功→通知结果 链路实录）。
- **M4 隧道（L2，远期可选）**：付费账号 + entitlement 申请获批后立项；Packet Tunnel Provider + Rust staticlib FFI（协议单源不破）；单独立项另写 spec。

## 10. 风险与开放问题

1. AppIntent 后台执行时长上限未实测（M3 首项验证，登录 HTTP 秒级、风险低）。
2. hotspot entitlement / NetworkExtension entitlement 申请结果不可控（均不影响 L0/L1，L2 前置条件）。
3. Tauri iOS 工程加入 extension target 属 Xcode 工程级改动，CI 维护成本中等（L2 范围）。
4. ring 依赖的 iOS 交叉编译若在 CI 首次构建报工具链问题，按 sing-box 先例排查（M0 首项验证）。
5. 快捷指令自动化需用户一次性手动配置（app 内做引导页缓解）。
