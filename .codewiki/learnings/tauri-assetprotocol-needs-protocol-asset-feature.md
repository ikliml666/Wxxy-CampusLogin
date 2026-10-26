---
title: 启用 assetProtocol 必须同步给 tauri 依赖加 protocol-asset feature
type: learning
source_files:
  - android/src-tauri/tauri.conf.json
  - android/src-tauri/Cargo.toml
tags: [教训, 安卓, tauri, 构建, cargo]
---

## Overview

本文记录在 Android 端 Tauri 应用中启用 `assetProtocol` 安全配置时，因 `Cargo.toml` 缺少对应 feature 导致编译期 build script 校验失败的问题。核心教训是：Tauri 的某些安全能力（如 `assetProtocol`）需要在 `tauri.conf.json` 和 `Cargo.toml` 中双重声明，且 build script 会严格校验二者的一致性。以当前代码（`android/src-tauri`）为准，`tauri.conf.json` 行20-23 启用了 `assetProtocol`，而 `Cargo.toml` 行15 已正确声明 `features = ["protocol-asset"]`。

## Key Components

- **`android/src-tauri/tauri.conf.json`**
    - **`security.assetProtocol`** (行20-23：`"assetProtocol": { "enable": true, "scope": ["$APPDATA/files/face-models/**"] }`)：启用资产协议，`enable` 为 `true` 允许应用通过自定义协议访问本地文件；`scope` 限定可访问路径仅为 `$APPDATA/files/face-models/**`。
    - **`security.dangerousDisableAssetCspModification`** (行24：`"dangerousDisableAssetCspModification": ["style-src"]`)：禁用 CSP 中对 `style-src` 指令的自动修改，以支持内联样式等需求。
    - **`security.csp`** (行19：`"default-src 'self'; connect-src 'self' ipc: http://ipc.localhost http://asset.localhost; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:"`)：自定义 Content Security Policy，其中 `connect-src` 包含 `http://asset.localhost`，`img-src` 包含 `data:` 和 `blob:`。
    - **`app.windows`** (行11-17)：定义主窗口标题与尺寸，与资产协议无直接关联但属同文件配置。

- **`android/src-tauri/Cargo.toml`**
    - **`tauri` 依赖** (行15：`tauri = { version = "2", features = ["protocol-asset"] }`)：必须显式声明 `protocol-asset` feature 以支持 `tauri.conf.json` 中的 `assetProtocol` 配置。
    - **`campus-login` 路径依赖** (行31：`campus-login = { path = "../../tauri-app/src-tauri" }`)：与桌面端单点共享协议核心逻辑。
    - **`libc` 依赖** (行28：`libc = "0.2"`)：用于 `sched_setaffinity` 绑小核（行27 注释说明），稳定功耗。
    - **其他插件/依赖** (行16 `tauri-plugin-notification`、行34 `tauri-plugin-campus-network-bind`、行37 `tauri-plugin-campus-keystore`、行40 `tauri-plugin-campus-monitor-service`、行43 `tauri-plugin-biometric`、行44 `tauri-plugin-opener`、行47 `reqwest` 等)：提供通知、网络绑定、密钥库、前台服务、生物识别、外链打开及更新流功能。
    - **`[profile.release]`** (行55-58：`lto = "fat"`、`codegen-units = 1`、`strip = true`)：优化 APK 体积；行53-54 注释明确不设 `panic="abort"` 以避免进程被杀。

> 旧版文章中提及的 `android_system_properties` 依赖在当前源码中已不存在（行27-28 仅 `libc`），故移除。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   android/src-tauri                         │
│                                                             │
│  ┌──────────────────────┐       ┌────────────────────────┐  │
│  │  Cargo.toml          │       │  tauri.conf.json       │  │
│  │                      │       │                        │  │
│  │  [dependencies]      │       │  [app.security]        │  │
│  │  tauri v2 (行15)     │──────►│  assetProtocol         │  │
│  │   + protocol-asset   │  sync │   enable: true (行21)  │  │
│  └──────────────────────┘       │   scope: [...] (行22)  │  │
│                                 └────────────────────────┘  │
│                                         ▲                    │
│                                         │                    │
│                              ┌──────────────────────┐       │
│                              │  Tauri Build Script  │       │
│                              │  (Build-time Check)  │       │
│                              │                      │       │
│                              │  Validates:          │       │
│                              │  conf.json ↔ Cargo   │       │
│                              └──────────────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. 开发者在 `tauri.conf.json` 中设置 `"security.assetProtocol.enable": true`（行21）并定义 `scope`（行22）。
2. 运行 `cargo check --target aarch64-linux-android` 或 `cargo build` 执行安卓构建。
3. Tauri 的 build script 启动，读取 `tauri.conf.json` 中的安全配置（行20-24）。
4. Build script 校验 `Cargo.toml` 中 `tauri` 依赖的 features 是否包含 `protocol-asset`（行15）。
5. 如果缺失，build script 报错并终止编译，提示用户添加该 feature。
6. 开发者在 `Cargo.toml` 中添加 `features = ["protocol-asset"]`（如当前行15 所示）。
7. 重新编译，校验通过，构建成功。

## Connections

[[android-host-cargo-check-fails]]、[[verification-baseline]]

## Known Issues

暂无其他已知问题。需警惕若遗漏 `Cargo.toml` 行15 的 `protocol-asset` feature，将复现编译期 build script 校验失败，错误通常指向 `assetProtocol` 配置与依赖 feature 不一致。
