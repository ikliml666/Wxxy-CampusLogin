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
