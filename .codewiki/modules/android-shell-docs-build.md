---
title: android-shell-docs-build
type: module
source_files:
  - android/README.md
  - android/CODE_WIKI.md
  - android/build-apk.ps1
  - android/.gitignore
tags: [android, build, tauri, automation]
---

## Overview
本模块负责 Android 平台应用的构建流程、项目结构说明及版本控制配置。核心包含一份项目自述文档 (`README.md`)，记录技术栈与平台决策；一份代码 Wiki 更新日志 (`CODE_WIKI.md`)，记录自动登录通知逻辑的演进；以及一个 PowerShell 构建脚本 (`build-apk.ps1`)，实现从前端依赖安装到 APK 签名校验的一键打包流程。此外，`.gitignore` 定义了针对 Tauri Android 工程的本地忽略规则。

## Key Components

### `android/build-apk.ps1`
PowerShell 脚本，用于自动化构建 Android APK。
- **参数**: `[switch]$SkipFrontend` (第 6 行)，跳过前端构建步骤。
- **环境变量设置**:
  - `JAVA_HOME`: 优先使用环境变量，若未设置则回退至默认路径 `C:\Users\ik\AppData\Local\Android\jdk\jdk-17.0.20.1+1` (第 19-21 行)。
  - `PATH`: 将 Java bin 目录加入 PATH (第 25 行)。
  - 编码: 强制 UTF-8 输出以解决 GBK 终端下的字符乱码问题 (第 11-12 行)。
- **前端构建逻辑** (第 27-41 行):
  - 检查 `node_modules` 是否存在，存在则跳过 `npm install` (第 29-34 行)。
  - 执行 `npx vite build` 构建前端资源 (第 37-41 行)。
- **APK 构建逻辑** (第 43-55 行):
  - 删除旧 APK 以防止 Gradle up-to-date 缓存导致产物陈旧 (第 45-47 行)。
  - 调用 Tauri CLI 进行交叉编译和打包：`tauri android build --target aarch64 --apk` (第 50 行)。
- **签名校验与修复** (第 57-87 行):
  - 查找最新的 APK 文件 (第 57-59 行)。
  - 尝试定位 `apksigner.bat` (第 63-67 行)。
  - 验证签名状态：若 `apksigner verify` 失败，则手动执行 `zipalign` 和使用 `debug.keystore` 进行签名 (第 75-87 行)。
  - 签名密钥库路径硬编码为 `%USERPROFILE%\.android\debug.keystore` (第 78 行)。

### `android/README.md`
项目文档，描述 Android 端架构与构建前提。
- **依赖关系**: 通过 Cargo path 依赖 `campus-login` (main 仓库 `tauri-app/src-tauri`) (第 4 行)。
- **目录结构**:
  - `frontend/`: React 前端，使用 Vite，别名 `@shared` 指向主仓库共享层 (第 8 行)。
  - `src-tauri/`: Rust 后端，包含协议核心与安卓专属探针 (第 9 行)。
  - `src-tauri/gen/android/`: Tauri 生成的安卓壳工程 (第 10 行)。
- **构建命令**: 提供调试 (`dev`) 和生产 (`build`) 命令 (第 17-18 行)。
- **平台决策**:
  - 强制 WLAN: 使用 Kotlin `ConnectivityManager.bindProcessToNetwork()` (第 23 行)。
  - 校园网检测: IPv4 枚举 + /18 子网判定 + Portal TCP 可达性 (第 25-26 行)。明确注明 ICMP 原始 socket 被 SELinux 禁止，`surge-ping` 不可用 (第 26 行)。
  - 密码存储: 阶段 1 不落盘，阶段 2 接入 Android Keystore (第 27 行)。

### `android/CODE_WIKI.md`
记录特定功能变更的历史注释。
- **自动登录通知优化** (第 1 行):
  - 时间: 2026-09-09, v27/v28。
  - 变更: 删除系统通知 `notify_system`，改为 emit `auto-login-result{success,message}` 供前端监听 (第 1 行)。
  - 效果: 消除连续系统通知轰炸，保留失败/掉线/达上限的系统通知以维持锁屏感知 (第 1 行)。
  - 状态管理: 成功时预置 `MONITOR.was_online=true` 并清除注销保护期 (第 1 行)。

### `android/.gitignore`
Git 忽略规则。
- **依赖与构建**: `node_modules/`, `frontend/dist/`, `src-tauri/target/`, `build/`, `.tauri/` (第 2, 5, 6, 12, 13 行)。
- **本地配置**: `*.local`, `local.properties` (第 9, 14 行)。
- **其他**: `*.tsbuildinfo` (第 17 行)。

## Architecture

```text
┌─────────────────────┐       ┌──────────────────────────┐
│   Frontend (React)  │       │     src-tauri (Rust)     │
│  frontend/          │◄────►│  - campus-login (path dep)│
│  (Vite Build)       │       │  - Android Probes        │
└─────────┬───────────┘       └──────────┬───────────────┘
          │                              │
          │ npx vite build               │ tauri android build
          ▼                              ▼
┌─────────────────────────────────────────────────────────┐
│              build-apk.ps1 (Automation)                 │
│                                                         │
│  1. Check JAVA_HOME & Set Env                           │
│  2. npm install (if missing)                            │
│  3. Vite Build                                          │
│  4. Clean Old APKs                                      │
│  5. Run Tauri CLI (Cross-compile AArch64)               │
│  6. Verify/Repair Signature (zipalign/apksigner)        │
└─────────────────────────┬───────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              Output: Wxxy-CampusLogin_<ver>.apk         │
│              src-tauri/gen/android/app/build/outputs... │
└─────────────────────────────────────────────────────────┘
```

## Data Flow
1. **环境准备**: `build-apk.ps1` 启动后，首先校验 `JAVA_HOME`。若未设置，则硬编码回退至 `C:\Users\ik\AppData\Local\Android\jdk\jdk-17.0.20.1+1` (第 20 行)。
2. **前端构建**: 若未跳过 (`$SkipFrontend` 为 false)，脚本检查 `frontend/node_modules`。若缺失，执行 `npm install`；随后执行 `npx vite build` 生成静态资源 (第 29-41 行)。
3. **清理旧包**: 为防止 Gradle 增量构建导致的旧 APK 残留，脚本强制删除 `src-tauri/gen/android/app/build/outputs/apk/universal/release/` 下的所有 `.apk` 文件 (第 45-47 行)。
4. **Tauri 构建**: 调用 `tauri.js android build --target aarch64 --apk` 触发 Rust 交叉编译和 Android Gradle 构建 (第 50 行)。
5. **产物定位**: 脚本按最后修改时间排序，选取最新的 APK 文件 (第 57-59 行)。
6. **签名验证**:
   - 查找 `apksigner.bat` (第 63-67 行)。
   - 运行 `apksigner verify`。若退出码非 0，视为未签名 (第 70-72 行)。
   - 若未签名，使用 `zipalign.exe` 对齐，并使用 `%USERPROFILE%\.android\debug.keystore` (密码 `android`) 通过 `apksigner sign` 重新签名 (第 76-86 行)。
7. **完成**: 输出 APK 路径和大小 (第 89-90 行)。

## Connections
- [[tauri-app]]: Android 端通过 Cargo path 依赖此模块中的 `campus-login` 协议核心。
- [[frontend-shared]]: Android 前端通过 `@shared` alias 引用此模块的共享层。
- [[campus-login-core]]: Android 端复用的协议核心，禁止复制，单点维护。