---
title: 启用 assetProtocol 必须同步给 tauri 依赖加 protocol-asset feature
type: learning
source_files:
  - android/src-tauri/tauri.conf.json
  - android/src-tauri/Cargo.toml
tags: [教训, 安卓, tauri, 构建, cargo]
---

## Overview

本文记录在 Android 端 Tauri 应用中启用 `assetProtocol` 安全配置时，因 `Cargo.toml` 缺少对应 feature 导致编译期 build script 校验失败的问题。核心教训是：Tauri 的某些安全能力（如 `assetProtocol`）需要在 `tauri.conf.json` 和 `Cargo.toml` 中双重声明，且 build script 会严格校验二者的一致性。

## Key Components

- **`android/src-tauri/tauri.conf.json`**
    - **`security.assetProtocol.enable`** (`"enable": true`)：启用资产协议，允许应用通过自定义协议访问本地文件。
    - **`security.assetProtocol.scope`** (`["$APPDATA/files/face-models/**"]`)：限定可访问的路径范围，仅允许读取 `$APPDATA/files/face-models/` 目录下的文件。
    - **`security.dangerousDisableAssetCspModification`** (`["style-src"]`)：禁用 CSP 中对 `style-src` 指令的自动修改，以支持内联样式等需求。
    - **`security.csp`**：自定义 Content Security Policy，其中 `connect-src` 包含 `http://asset.localhost`，`img-src` 包含 `data:` 和 `blob:`。

- **`android/src-tauri/Cargo.toml`**
    - **`tauri` 依赖** (`line 15`)：`tauri = { version = "2", features = ["protocol-asset"] }`。必须显式声明 `protocol-asset` feature 以支持 `assetProtocol` 配置。
    - **`campus-login` 路径依赖** (`line 37`)：`campus-login = { path = "../../tauri-app/src-tauri" }`。与桌面端共享协议核心逻辑。
    - **Android 特定依赖** (`line 29-34`)：
        - `android_system_properties`：用于读取 Android 系统属性（如 `ro.soc.model`）。
        - `libc`：用于 `sched_setaffinity` 绑定小核，稳定功耗。
    - **Release Profile** (`line 48-51`)：启用 `lto = "fat"`、`codegen-units = 1` 和 `strip = true` 以优化 APK 体积。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                   android/src-tauri                         │
│                                                             │
│  ┌──────────────────────┐       ┌────────────────────────┐  │
│  │  Cargo.toml          │       │  tauri.conf.json       │  │
│  │                      │       │                        │  │
│  │  [dependencies]      │       │  [app.security]        │  │
│  │  tauri v2            │──────►│  assetProtocol         │  │
│  │   + protocol-asset   │  sync │   enable: true         │  │
│  └──────────────────────┘       │   scope: [...]         │  │
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

1. 开发者在 `tauri.conf.json` 中设置 `"security.assetProtocol.enable": true` 并定义 `scope`。
2. 运行 `cargo check --target aarch64-linux-android` 或 `cargo build`。
3. Tauri 的 build script 启动，读取 `tauri.conf.json` 中的安全配置。
4. Build script 校验 `Cargo.toml` 中 `tauri` 依赖的 features 是否包含 `protocol-asset`。
5. 如果缺失，build script 报错并终止编译，提示用户添加该 feature。
6. 开发者在 `Cargo.toml` 中添加 `features = ["protocol-asset"]`。
7. 重新编译，校验通过，构建成功。

## Connections

[[android-host-cargo-check-fails]]、[[verification-baseline]]