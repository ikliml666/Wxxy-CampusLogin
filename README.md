# Wxxy-CampusLogin

无锡学院校园网登录助手 — 基于 Tauri 2 + React 19，Windows 桌面 + Android 双端应用

![version](https://img.shields.io/badge/version-2.3.5-blue)
![platform](https://img.shields.io/badge/platform-Windows%20x64%20%7C%20Android-lightgrey)
![license](https://img.shields.io/badge/license-MIT-green)

<img alt="screenshot" src="assets/screenshot-main.png" />

## 下载安装

前往 [Releases](https://github.com/ikliml666/Wxxy-CampusLogin/releases) 下载最新版本：

- **Windows**：`Wxxy-CampusLogin_{版本}_x64-setup.exe`，双击安装即可
- **Android**：`Wxxy-CampusLogin_{版本}.apk`，安装需允许"未知来源应用"（系统版本要求 Android 10+）

两端应用均内置更新检查，新版本发布后会自动提示（安卓端支持应用内下载 APK 并唤起系统安装器）。

## 功能特性

- **一键登录** — 自动检测网络适配器、DHCP 续租、可重试失败智能重试
- **一键注销** — 两步注销：Radius 注销 + MAC 解绑，支持指定适配器或全部注销
- **自动重连** — 后台巡检断线检测，最多 3 次自动重连
- **自动退出** — 登录成功后倒计时自动退出，`Ctrl+Shift+C` 取消
- **校园网检测** — WiFi/有线独立检测：网络名称匹配 → /18 子网匹配 → 网关 Ping 可达，非校园网环境自动退出
- **DNS 智能解析** — 动态评分选择最优 DNS 服务器，应用级 DoH 解析（RFC 8484），三级智能解析策略
- **DNS 优化** — 检测当前 DNS/DoH 配置，一键设置推荐 DNS（IPv4/IPv6/双栈）+ 启用 DoH 加密
- **网络质量监测** — 网关/DNS/DoH/HTTPS 延迟并发测试，DNS 解析专项测试，结果增量推送；"网络拥堵"通知经 15s 间隔两次复核确认，避免瞬时抖动误报
- **双适配器支持** — 有线 + 无线同时管理，Dock 栏适配器选择菜单
- **多账号管理** — DPAPI 加密存储、快速切换
- **运营商账号绑定** — 对接校园网自助服务系统，绑定/查询运营商账号，绑定状态一目了然
- **Windows Hello 验证** — 查看运营商密码明文、踢设备下线等敏感操作需本地生物识别/PIN 验证，后端时效复核
- **自助服务查询** — 在线设备信息、近期上网记录（日期范围筛选 + 汇总统计），支持踢设备下线
- **总览自定义卡片** — 首页卡片可增删与拖拽排序，内置"在线信息"与"近期上网记录"卡（关键信息验证后查看）
- **主题定制** — 7 种预设主题 + 自定义主题色 + 深浅模式
- **系统托盘** — 最小化到托盘后台运行，支持托盘快速登录
- **开机自启** — 支持静默启动
- **日志管理** — 运行日志面板（级别/模块/关键词过滤），可按 3/7/14/30 天或永久自动清理
- **用户自助服务** — 一键打开校园网自助服务系统
- **中英语言切换** — 标题栏一键切换，默认中文

**安卓端**（`android/`，与桌面端共享同一 Rust 协议核心，功能同名对齐）：

- 一键登录 / 两步注销、后台断线自动重登（前台服务 + 常驻通知 + WifiLock 保活）
- 校园网检测（/18 子网匹配 + Portal 可达性探测）、网络质量监测与定时测试
- 多账号管理（AndroidKeyStore AES-GCM 加密存储）、运营商账号绑定与自助服务查询（敏感操作经系统 BiometricPrompt 生物识别验证）
- 开机自启、运行日志面板、应用内更新（下载 APK 唤起系统安装器）
- 平台差异：系统托盘、Windows Hello、DNS 优化、双适配器/有线管理为桌面专属；安卓端登录流量强制绑定 WLAN（`bindProcessToNetwork`，不走蜂窝数据）

## 技术栈

| 层级 | 技术 |
|------|------|
| 框架 | Tauri 2 |
| 前端 | React 19 + TypeScript + Vite 6 |
| 样式 | TailwindCSS 3.4 + Framer Motion 12 + GSAP 3 |
| 后端 | Rust + Tokio |
| 网络 | reqwest 0.12 + tokio-rustls 0.26 + hickory-resolver 0.24 |
| 加密 | Windows DPAPI（桌面）/ AndroidKeyStore AES-256-GCM（安卓） |
| 平台 | Windows (Win32/WinRT API, Windows Hello)；Android (前台服务, BiometricPrompt, minSdk 29) |
| 国际化 | react-i18next + i18next-browser-languagedetector |

## 项目结构

```
Wxxy-CampusLogin/
├── assets/                  # 截图等资源
├── tauri-app/
│   ├── build.ps1            # 发布构建脚本（生成安装包 + .sha256）
│   ├── frontend/            # React 前端
│   │   ├── src/
│   │   │   ├── components/  # UI 组件
│   │   │   │   └── layout/  # 布局组件（标题栏/状态栏/Dock导航/右侧面板）
│   │   │   ├── auth/        # 认证面板（总览/关于对话框）
│   │   │   ├── account/     # 账号面板
│   │   │   ├── network/     # 网络面板（适配器/DNS 优化）
│   │   │   ├── monitor/     # 监控面板（状态/质量/延迟/测速）
│   │   │   ├── settings/    # 设置面板（主题/新手引导）
│   │   │   ├── shared/      # 共享组件（日志/错误边界/Toast/赞助浮层等）
│   │   │   ├── hooks/       # 状态管理（按领域拆分的 zustand store）& IPC & 初始化子 hook
│   │   │   ├── i18n/        # 国际化（zh.json / en.json）
│   │   │   ├── lib/         # 工具函数
│   │   │   └── App.tsx      # 根组件
│   │   └── package.json
│   └── src-tauri/           # Rust 后端
│       ├── src/
│       │   ├── commands/    # Tauri 命令（模块化拆分）
│       │   ├── network/     # 网络模块（适配器/DNS/质量检测/缓存/HTTP计时）
│       │   ├── config/      # 配置管理（model/persist/validate）
│       │   ├── auth/        # 认证模块（Portal检测/登录注销协议/会话管理）
│       │   ├── account/     # 账号模块（crypto.rs DPAPI 加密）
│       │   ├── monitor/     # 监控模块（后台巡检/自动登录/延迟测试/适配器监控）
│       │   ├── infra/       # 基础设施（状态管理/日志/事件总线/退出生命周期/通知）
│       │   ├── platform/    # 平台交互（DNS配置/UAC提权/GPU检测/开机自启/Windows Hello验证）
│       │   ├── helper/      # 提权辅助子进程（改 MAC/设 DNS，无 PowerShell 依赖）
│       │   ├── app/         # 应用生命周期（启动/托盘/窗口/快捷键/心跳）
│       │   └── update/      # 更新模块（检查/下载/安装/SHA256校验）
│       ├── icons/           # 应用图标
│       ├── Cargo.toml
│       └── tauri.conf.json
├── android/                 # 安卓端（与桌面共享协议核心，Cargo path 依赖）
│   ├── frontend/            # React 前端（桌面复刻 + 移动裁剪，底部导航布局）
│   ├── src-tauri/           # 安卓 Rust 后端（监控循环/加密配置/校园网探针/更新）
│   │   ├── src/
│   │   │   ├── lib.rs           # 入口（44 个 Tauri 命令，与桌面同名对齐）
│   │   │   ├── protocol_cmds.rs # 登录/注销/Portal 探测（复用桌面协议核心）
│   │   │   ├── campus_detect.rs # 校园网探针（子网匹配 + Portal TCP 可达）
│   │   │   ├── config_state.rs  # 配置管理（AndroidKeyStore 加密落盘）
│   │   │   ├── monitor_loop.rs  # 后台检测 + 断线自动重登状态机
│   │   │   └── ...              # 自助服务/账号/日志/更新/SoC 分档等
│   │   └── gen/android/     # Tauri 生成的 Gradle 工程（产物不入库）
│   └── plugins/             # 手写 Tauri 插件（keystore / foreground-service / network-bind）
└── CODE_WIKI.md             # 详细代码文档
```

## 开发环境搭建

### 前置要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) (stable)
- [Tauri 2 CLI](https://tauri.app/start/prerequisites/)

### 安装依赖

```bash
cd tauri-app/frontend
npm install

cd ..
npm install
```

### 开发模式

```bash
cd tauri-app
npx tauri dev
```

### 构建发布

```bash
# 完整发布构建：前端打包 + Tauri 打包 + 生成 .sha256 校验文件
pwsh tauri-app/build.ps1
```

### 安卓构建

前置要求：[Android Studio](https://developer.android.com/studio)（SDK + NDK + JDK 17）、`ANDROID_HOME` / `NDK_HOME` / `JAVA_HOME` 已配置、`rustup target add aarch64-linux-android`。Windows 需开启开发者模式（允许符号链接）。

```bash
# 真机 / 模拟器调试
cd android/frontend && npm install && npm run dev
cd ../src-tauri && npx @tauri-apps/cli android dev

# 构建发布 APK（tauri CLI 不执行 beforeBuildCommand，前端必须先单独构建）
cd ../frontend && npm install && npm run build
cd ../src-tauri && npx @tauri-apps/cli android build --target aarch64 --apk
```

> 安卓端协议核心经 Cargo path 依赖桌面 crate（`tauri-app/src-tauri`），构建前请先完成桌面端依赖安装（`tauri-app/src-tauri` 可编译）。安卓 Rust 侧依赖 mobile-only 插件，host `cargo check` 不可用，以 `tauri android build` 交叉编译结果为准。

### 测试

项目包含后端 Rust 测试和前端 TypeScript 测试，CI 前请确保全部通过。

```bash
# 后端测试（246 个单元测试 + 1 个回归集成测试）
cd tauri-app/src-tauri
cargo test

# 后端 lint
cargo clippy --all-targets -- -D warnings

# 前端测试（74 个测试）
cd ../frontend
npm test

# 前端类型检查
npx tsc --noEmit --incremental
```

## 安全说明

- 密码使用 Windows DPAPI 加密存储，绑定当前 Windows 用户（登录密码与自助服务密码同措施）
- 安卓端密码使用 AndroidKeyStore AES-256-GCM 加密存储（密钥硬件隔离，随应用卸载销毁），磁盘文件不含明文；敏感操作经系统 BiometricPrompt 验证，后端校验验证时效（600 秒 TTL）
- 登录/注销流量强制绑定 WLAN 接口（`ConnectivityManager.bindProcessToNetwork`），物理上不经蜂窝数据发送
- 应用内更新下载域名白名单校验 + 500MB 上限 + SHA256 完整性校验（GitHub API digest 优先），APK 安装路径限定应用更新目录
- 前端显示密码为 `***`，不暴露明文；保存时空密码不覆盖旧密码；所有配置出站路径统一经后端掩码出口，明文不出后端
- 查看运营商账户密码明文需通过 Windows Hello 验证，后端校验验证时效（600 秒 TTL），绕过前端也无法获取明文
- HTTP 客户端默认 TLS 1.3，回退 TLS 1.2
- DoH 解析使用 RFC 8484 wire format
- 更新安装包 SHA256 完整性校验：校验源全部 4xx 时默认拒绝安装，5xx/传输错误/哈希不匹配一律拒绝
- 提权操作（改 MAC/设 DNS）由 Rust 直调 Win32/winreg，无 shell 拼接命令注入面；适配器名称额外做元字符校验
- 账号名校验防止路径遍历攻击
- 外部链接仅允许 http/https 白名单并限制长度

## 致谢

本项目参考了 [Wxxy\_network\_auto\_login](https://github.com/Senquan007/Wxxy_network_auto_login) 的 Portal 认证逻辑与网络检测方案。

## 许可证

本项目采用**双轨许可**：

- **源代码**：[MIT License](LICENSE) —— 允许自由使用、修改与再分发（含商用），须保留版权声明。
- **美术素材**（看板娘/背景娘全套插画、应用图标等）：**版权所有，不在 MIT 授权范围内**。禁止单独提取、二次分发、改编与商用，详见 [LICENSE-ASSETS.md](LICENSE-ASSETS.md)。
- **第三方组件**：本项目依赖的开源组件（Rust crates、npm 包等）的版权与许可声明汇总见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。
