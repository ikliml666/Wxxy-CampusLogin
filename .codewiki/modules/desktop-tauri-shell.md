---
title: desktop-tauri-shell
type: module
source_files:
  - tauri-app/build.ps1
  - tauri-app/frontend/about-preview.html
  - tauri-app/frontend/ANIMATION_GUIDE.md
  - tauri-app/frontend/index.html
  - tauri-app/frontend/package.json
  - tauri-app/frontend/pnpm-lock.yaml
  - tauri-app/frontend/postcss.config.js
  - tauri-app/frontend/public/mock-tauri-addon.js
  - tauri-app/frontend/public/mock-tauri.js
tags: [Tauri, Build, Frontend, Mock, Animation]
---

## Overview
该模块涵盖 CampusLogin 桌面端应用（基于 Tauri v2）的构建脚本、前端资源、动画集成指南及开发环境模拟层。核心功能包括：
- **自动化构建**：`build.ps1` 负责清理、依赖安装、前端打包、Rust/Cargo 编译及产物分发。
- **UI 与动效**：基于 React 19 + Framer Motion + Anime.js 的高性能动画系统，包含骨架屏、关于页预览及全局毛玻璃效果。
- **开发仿真**：通过 `mock-tauri.js` 和 `mock-tauri-addon.js` 在浏览器环境中伪造 Tauri API，支持多场景截图验证。
- **工程配置**：基于 Vite 6.0、Tailwind CSS 3.4 及 PostCSS 的前端构建栈。

## Key Components

### 1. Build System (PowerShell)
文件：`tauri-app/build.ps1`

| 组件/变量 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| `$SkipFrontend` | 2| 跳过前端构建开关 |
| `$SkipCopy` | 3| 跳过产物复制开关 |
| `$TargetDir` | 4| 自定义 Cargo 目标目录，解决 Windows 路径过长问题 |
| `Check Environment` | 32-55| 检查 `rustc`, `node`, `cargo` 是否可用 |
| `Install Dependencies` | 57-79| 检查 `frontend/node_modules` 和根目录 `node_modules`，缺失则执行 `npm install` |
| `Build Frontend` | 82-96| 执行 `npm run build`；若跳过且无 `dist/index.html` 则报错 |
| `Build Tauri App` | 99-145| 设置 `CARGO_BUILD_JOBS` 为 CPU 核心数；调用 `@tauri-apps/cli` 进行 Release 编译 |
| `Copy Artifacts` | 153-193| 将 `release/bundle` 下的文件复制到 `tauri-release/`，并生成 `.sha256` 校验文件 |
| `Generate Checksums` | 186-193| 对 `-setup.exe` 生成 SHA256 哈希文件，供应用内更新器验证 |

### 2. Frontend Assets & Configuration

#### A. Skeleton Screen (`index.html`)
文件：`tauri-app/frontend/index.html`

| 组件/逻辑 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| IIFE Theme Init | 10-26| 初始化前读取 `localStorage` 设置 `data-light`, `theme-*`, `data-glass` 属性，避免首屏闪烁 |
| CSS Variables | 28-73| 定义深色/浅色模式下的 CSS 变量（如 `--bg-primary`, `--sk-accent`） |
| Skeleton DOM | 91-102| 渲染加载中的骨架屏结构（`.sk-tb`, `.sk-sb`, `.sk-main`） |
| i18n Fallback | 107-114| 根据 `app-language` 本地存储切换骨架屏文案（中文/英文） |

#### B. About Preview (`about-preview.html`)
文件：`tauri-app/frontend/about-preview.html`

| 组件/样式 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| Dialog Overlay | 18-24| 半透明遮罩层，带模糊背景 |
| Left Panel | 48-57| 显示应用图标、标题、版本、描述及检查更新按钮 |
| Right Panel | 126-137| 显示网络状态圆圈、特性卡片网格（双适配器、多账号、自动重连） |
| Feature Cards | 180-213| 三列网格布局，展示核心功能图标与说明 |

#### C. Package Configuration
文件：`tauri-app/frontend/package.json` / `pnpm-lock.yaml`

| 依赖项 | 版本 | 用途 |
| :--- | :--- | :--- |
| `react` / `react-dom` | ^19.0.0 | UI 框架 |
| `@tauri-apps/api` | ^2 | Tauri 客户端 API |
| `framer-motion` | ^12.38.0 | 声明式动画库 |
| `gsap` | ^3.15.0 | 复杂序列动画 |
| `tailwindcss` | ^3.4.10 | 原子化 CSS |
| `zustand` | ^5.0.13 | 状态管理 |

#### D. PostCSS Config
文件：`tauri-app/frontend/postcss.config.js`

| 插件 | 行号 | 用途 |
| :--- | :--- | :--- |
| `tailwindcss` | 3| 处理 Tailwind 指令 |
| `autoprefixer` | 4| 自动添加浏览器前缀 |

### 3. Animation Guide (`ANIMATION_GUIDE.md`)
文件：`tauri-app/frontend/ANIMATION_GUIDE.md`

| 组件/策略 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| AnimatedNumber | 7-27| 数字平滑滚动（easeOutQuad），颜色高亮闪烁 |
| ElasticBarChart | 30-54| 柱状图弹性生长，波浪式错开启动 |
| StatusPulseBadge | 57-83| 状态切换时触发双次脉冲动画，背景色闪烁 |
| AnimatedCard | 88-104| 卡片交互：hover 上浮 + tap 缩放 + 磁性跟随 |
| DockNav Micro-interactions | 106-115| 图标 hover 放大 + 弹簧回弹 |
| Performance Tips | 222-240| 优先使用 transform/opacity，清理 Anime.js 实例防止内存泄漏 |

### 4. Tauri Mock Layer (Browser Shim)

#### A. Core Mock (`mock-tauri.js`)
文件：`tauri-app/frontend/public/mock-tauri.js`

| 函数/对象 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| `transformCallback` | 24-36| 将回调函数注册到 `window._cb{id}`，返回 ID |
| `mockConfig` | 38-58| 默认配置对象，包含用户、运营商、后台检查间隔等 |
| `campusStatus` | 60-65| 校园网连接状态模拟数据 |
| `backgroundStatus` | 67-73| 后台任务状态模拟数据 |
| `mockAdapters` | 75-79| 模拟网卡列表（WLAN, 以太网, 以太网 2） |
| `handlers` | 98-164| 映射 Tauri 命令名（如 `get_config`, `do_login`）到模拟实现 |
| `__TAURI_INTERNALS__.invoke` | 174-189| 拦截命令调用，路由至 `handlers` 或处理事件监听 |
| `__mockEmitEvent` | 199-205| 手动触发事件，通知注册的回调函数 |

#### B. Scenario Addon (`mock-tauri-addon.js`)
文件：`tauri-app/frontend/public/mock-tauri-addon.js`

| 参数/逻辑 | 行号范围 | 用途 |
| :--- | :--- | :--- |
| URL Params | 5-11| 解析 `?op=`, `dual=`, `night=`, `offline=`, `tab=` 等参数 |
| Override `invoke` | 13-48| 覆盖基础 mock，根据参数动态修改 `check_campus_status`, `get_adapters`, `get_init_data` 返回值 |
| `bgStatus()` | 50-67| 生成符合当前场景的后台状态数据 |
| Event Emission | 70-80| 定时发送 `background-check-result` 事件 |
| Tab Switching | 71-76| 加载后自动点击指定标签页的按钮（用于截图辅助） |
| Emulate Width | 82-87| 通过 CSS 强制 body 宽度，模拟窄屏布局 |

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Browser / Dev Console                    │
│  ┌──────────────┐   ┌──────────────────┐   ┌──────────────┐ │
│  │ index.html   │   │ mock-tauri.js    │   │ mock-addon.js│ │
│  │ (Skeleton)   │──►│ (__TAURI_INTERNALS)│──►│(Scenario Logic)│
│  └──────┬───────┘   └────────┬─────────┘   └──────┬───────┘ │
│         │                    │                     │         │
│         ▼                    ▼                     ▼         │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              React Application (Vite)                   │ │
│  │  ┌────────────┐  ┌──────────────┐  ┌────────────────┐  │ │
│  │  │ Animations │  │ State (Zustand)│  │ Tauri API Call │  │ │
│  │  │(Framer/    │  │              │  │ (invoke())     │  │ │
│  │  │ Anime.js)  │  │              │  │                │  │ │
│  │  └────────────┘  └──────────────┘  └────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │ Build Output
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Build Script (build.ps1)                   │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐  │
│  │ npm install  │──►│ vite build   │──►│ cargo tauri    │  │
│  └──────────────┘   └──────────────┘   │ build          │  │
│                                        └────────┬───────┘  │
│                                                 │          │
│                                                 ▼          │
│                                          ┌──────────────┐  │
│                                          │ tauri-release│  │
│                                          │ (Artifacts)  │  │
│                                          └──────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **开发环境启动**：
    *   `index.html` 加载，执行 IIFE 初始化主题（[[Connections]]）。
    *   `mock-tauri.js` 注入 `window.__TAURI_INTERNALS__`，接管所有 Tauri API 调用。
    *   `mock-tauri-addon.js` 读取 URL 参数，进一步劫持 `invoke` 方法，返回特定场景数据（如双网卡、夜间模式）。
    *   React 应用通过 `@tauri-apps/api` 发起请求，实际由 Mock 层同步返回 Promise 结果。

2.  **生产构建流程**：
    *   运行 `.\build.ps1`。
    *   检查环境（Rust/Node/Cargo）。
    *   安装依赖（前端与根目录）。
    *   执行 `vite build` 生成静态资源至 `frontend/dist`。
    *   执行 `cargo tauri build`，Rust 后端与前端资源捆绑。
    *   将生成的安装包（`.exe`）和可执行文件复制到 `tauri-release/`。
    *   计算 `.sha256` 校验和，确保更新器可验证完整性。

## Connections
*   [[desktop-electron-migration]]: `build.ps1` 中保留了与 Electron 旧版项目（`renderer-app/`, `installer-build/`）的路径对比注释。
*   [[tauri-backend-rust]]: `build.ps1` 调用的 `cargo tauri build` 依赖于 `src-tauri` 目录下的 Rust 代码。
*   [[frontend-react-core]]: 前端动画指南与 Mock 脚本服务于 React 应用的用户界面体验。

## Known Issues
1.  **Cargo 静默期**：在编译单个大型 crate 时，终端可能数分钟无输出（`build.ps1` 第 116-118 行注释说明），这是 Cargo 的正常行为，非死锁。
2.  **Windows 路径长度**：若遇到路径过长错误，需通过 `-TargetDir 'C:\short\target'` 参数缩短路径（`build.ps1` 第 110-111 行）。
3.  **Mock 隔离**：`mock-tauri.js` 和 `mock-tauri-addon.js` 明确标注“绝不入库”，提交时需确保这些文件未被意外纳入生产构建或 Git 追踪（通常通过 `.gitignore` 或 CI 排除）。
4.  **Anime.js 内存泄漏风险**：文档指出必须手动清理动画实例（`ANIMATION_GUIDE.md` 第 237 行），否则可能导致内存泄漏。