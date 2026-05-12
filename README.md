# Wxxy-CampusLogin

无锡学院校园网登录助手 — 基于 Tauri 2 + React 19 的桌面应用
<img width="1438" height="1014" alt="986d77eb0b98b272" src="https://github.com/user-attachments/assets/341181ef-6cf1-47dc-acc2-b6e02fbe671b" />


![version](https://img.shields.io/badge/version-2.1.5-blue)
![platform](https://img.shields.io/badge/platform-Windows%20x64-lightgrey)
![license](https://img.shields.io/badge/license-MIT-green)

<br />

## 功能特性

- **一键登录** — 自动检测网络适配器、DHCP 续租、智能重试
- **自动重连** — 后台巡检断线检测，最多 3 次自动重连
- **网络质量监测** — 网关/DNS/HTTP/游戏服务器延迟并发测试
- **多账号管理** — DPAPI 加密存储、快速切换
- **双适配器支持** — 有线 + 无线同时管理
- **开机自启** — 静默启动、自动登录、登录后自动退出
- **主题定制** — 6 种预设主题 + 自定义主题色 + 深浅模式
- **系统托盘** — 最小化到托盘后台运行，支持托盘快速登录
- **窗口最大化** — 支持最大化/还原切换，最大化时内容区自适应扩展
- **登录历史** — 记录每次登录的时间、结果和适配器信息
- **系统通知** — 登录成功/失败时发送桌面通知

## 技术栈

| 层级 | 技术                                 |
| -- | ---------------------------------- |
| 框架 | Tauri 2                            |
| 前端 | React 19 + TypeScript + Vite 6     |
| 样式 | TailwindCSS 3.4 + Framer Motion 12 |
| 后端 | Rust + Tokio                       |
| 网络 | reqwest 0.12 + tokio-rustls 0.26   |
| 加密 | Windows DPAPI                      |
| 平台 | Windows (Win32 API)                |

## 项目结构

```
Wxxy-CampusLogin/
├── assets/                  # 截图等资源
├── tauri-app/
│   ├── frontend/            # React 前端
│   │   ├── src/
│   │   │   ├── components/  # UI 组件
│   │   │   │   ├── dialogs/ # 对话框
│   │   │   │   ├── layout/  # 布局组件（标题栏/状态栏/导航栏）
│   │   │   │   ├── panels/  # 面板组件
│   │   │   │   ├── shared/  # 共享组件
│   │   │   │   └── ui/      # 基础 UI 组件
│   │   │   ├── hooks/       # 状态管理 & IPC
│   │   │   ├── lib/         # 工具函数
│   │   │   ├── types/       # TypeScript 类型
│   │   │   └── constants/   # 常量
│   │   └── package.json
│   ├── src-tauri/           # Rust 后端
│   │   ├── src/
│   │   │   ├── commands/    # Tauri 命令（模块化拆分）
│   │   │   ├── network/     # 网络模块（适配器/Portal/登录/质量/缓存）
│   │   │   ├── config.rs    # 配置管理
│   │   │   ├── crypto_utils.rs  # 加密工具
│   │   │   ├── http_timing.rs   # HTTP 计时
│   │   │   └── logger.rs    # 日志系统
│   │   ├── icons/           # 应用图标
│   │   ├── Cargo.toml
│   │   └── tauri.conf.json
│   └── build.ps1            # 构建脚本
└── CODE_WIKI.md             # 详细代码文档
```

## 更新日志

### v2.1.5

#### ✨ 新功能

- **窗口最大化按钮** — 标题栏最小化和关闭按钮之间新增最大化/还原按钮，支持点击切换和双击标题栏切换；图标随状态动态变化（方框 ↔ 重叠方框），Tooltip 同步更新（"最大化"/"还原"）

- **窗口尺寸限制解除** — 移除 `maxWidth: 1400` / `maxHeight: 900` 约束，允许窗口真正全屏最大化

#### 🎨 界面优化

- **最大化布局智能适配** — 最大化时卡片内容区从 `560px` 扩展至 `960px`，充分利用屏幕空间展示更多内容

- **仅内容区放大策略** — 标题栏、状态栏、右侧面板（固定宽度）、底部导航栏在最大化时保持原有尺寸不变，避免 UI 元素过度拉伸

- **全屏圆角与阴影移除** — 最大化时自动去除外层容器圆角（`border-radius: 16px → 0`）和投影阴影，无缝贴合屏幕边缘；还原时恢复原样式

- **状态实时同步** — 通过 `getCurrentWindow().onResized()` 监听器实时追踪窗口最大化状态，确保按钮图标与实际窗口状态始终一致

---

### v2.1.4

#### 🐛 Bug 修复

- **修复登录成功后适配器缓存竞态问题**：移除登录成功后5秒延迟清除缓存的逻辑，避免与后台检测循环冲突导致无效重复查询

- **修复 Portal 缓存锁释放时机**：将缓存写入操作移到锁释放之前，防止并发检测写入缓存时数据不一致

- **修复登录频率限制过严**：将限流从1秒1次调整为3秒3次，避免用户快速双击登录被误拒

- **修复 AC 认证失败时返回码不一致**：`code` 从 `"0"` 改为 `"ac_auth_failed"`，消除与 `success: false` 的歧义

- **修复自动退出倒计时延长后前端不知情**：快捷键注册失败时延长3倍倒计时后重新通知前端，避免用户看到的倒计时与实际不符

- **修复前端初始化重复执行**：使用 ref 守卫确保初始化逻辑只执行一次，避免 Context 值变化导致反复调用 `getInitData`

- **修复内网 DNS 测试被阻止**：放宽 IP 限制，仅阻止 loopback 和 link-local，允许校园网内网 `10.x.x.x` DNS 服务器测试

#### 🏗️ 架构重构

- **拆分 network.rs 上帝模块**：将1400行的 `network.rs` 拆分为 `cache.rs`、`adapter.rs`、`portal.rs`、`login_request.rs`、`quality.rs` 五个职责清晰的子模块

- **合并全局可变状态**：将 AppState 的16个散列字段分组为 `TaskFlags`（6个任务标志）和 `NetworkStatus`（7个网络状态），状态关系更清晰；将 `PORTAL_URL` 全局变量合并进 `NET_CACHE`

- **合并前端三层 Context**：将 `ConfigProvider → NetworkProvider → UIProvider` 三层嵌套合并为单一 `AppStoreProvider`，`useIpc()` 实例从3个减少到1个

#### ⚡ 性能优化

- **优化编译参数**：`codegen-units` 从 256 改为 16，让 LTO 优化真正生效，预期减少二进制体积5-10%

- **缩短适配器缓存 TTL**：从60秒缩短为15秒，插拔网线后UI更新延迟从最长60秒降至15秒

#### 🔒 安全改进

- **配置验证返回错误而非静默修正**：`portal_url`、`theme_mode`、`custom_theme_color`、`fixed_gateway` 等字段验证失败时返回明确错误信息，不再静默替换为默认值

- **错误处理改进**：20处 `let _ =` 忽略错误改为适当处理——文件操作失败传播错误或记录日志，配置保存失败返回前端，登录历史写入失败记录警告

#### 🧹 代码清理

- 移除前端未使用的 `pauseQualityListener`/`resumeQualityListener` 死代码
- 移除后端未使用的 `NetworkError` 枚举及其3个 impl 块
- 移除前端未使用的 `NetworkQualityLevel` 类型
- 修复 Toast 通知起始位置，不再遮挡状态栏胶囊行

---

### v2.1.3

初始发布版本。

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

```powershell
cd tauri-app
.\build.ps1
```

或手动构建：

```bash
cd tauri-app/frontend
npm run build

cd ../src-tauri
cargo build --release
```

## 安全说明

- 密码使用 Windows DPAPI 加密存储，绑定当前 Windows 用户
- 配置文件中的密码字段始终以加密形式保存
- 前端显示密码为 `***`，不暴露明文
- CSP 策略限制脚本、插件和表单提交来源
- 外部链接打开有 URL 验证和本地地址黑名单
- 登录频率限制防止滥用

## 致谢

本项目参考了 [Wxxy\_network\_auto\_login](https://github.com/Senquan007/Wxxy_network_auto_login) 的 Portal 认证逻辑与网络检测方案。

## 许可证

MIT License
