---
title: 版本号以 tauri.conf.json 为唯一权威源，build.rs 编译期注入
type: decision
source_files:
  - tauri-app/src-tauri/tauri.conf.json
  - tauri-app/src-tauri/build.rs
  - android/src-tauri/Cargo.toml
tags: [决策, 版本号, 构建, 发布]
---

## Overview

本模块确立了应用版本号的单一事实来源（Single Source of Truth）机制。`tauri.conf.json` 中的 `version` 字段是唯一的权威源；桌面端通过 `build.rs` 在编译期将其提取并注入为 Rust 环境变量 `APP_VERSION`，代码中统一使用 `env!("APP_VERSION")` 引用。安卓端通过 `Cargo.toml` 独立声明版本号，并通过 path 依赖共享桌面端的协议核心库。

## Key Components

### tauri-app/src-tauri/tauri.conf.json
- **Line 2**: `"version": "2.4.1"` — 版本号定义处，所有桌面端构建的版本号均源自此字符串。

### tauri-app/src-tauri/build.rs
- **Line 5–39**: `main()` 函数 — 构建脚本入口。
    - **Line 6**: `tauri_build::build();` — 调用 Tauri 原生构建逻辑。
    - **Line 17–30**: JSON 解析逻辑 — 手动遍历文件行，查找以 `"version"` 开头的行，提取冒号后的值并去除引号和逗号。这是获取版本号的核心算法。
    - **Line 33**: `println!("cargo:rerun-if-changed=tauri.conf.json");` — 依赖监听，仅当配置文件变化时触发重编译。
    - **Line 35**: `println!("cargo:rustc-env=APP_VERSION={version}");` — 将提取的版本号注入 Rust 编译环境变量。

### android/src-tauri/Cargo.toml
- **Line 3**: `version = "2.4.1"` — 安卓端 Cargo crate 的独立版本号声明。虽然当前与桌面端保持一致，但在 Cargo 体系中它是独立的元数据源。
- **Line 38**: `campus-login = { path = "../../tauri-app/src-tauri" }` — 安卓端依赖桌面端的协议核心库（作为 path 依赖），确保网络协议层代码复用。

## Architecture

```text
┌──────────────────────────┐       ┌──────────────────────────┐
│   tauri.conf.json        │       │   Cargo.toml (Android)   │
│   (Source of Truth)      │       │   (Android Version)      │
│   v2.4.1                 │       │   v2.4.1                 │
└──────────┬───────────────┘       └──────────┬───────────────┘
           │                                   │
           ▼                                   ▼
┌──────────────────────────┐       ┌──────────────────────────┐
│   build.rs               │       │   Android Build          │
│   (Parser & Injector)    │       │                          │
│   - Reads conf JSON      │       │   - Uses CARGO_PKG_      │
│   - Extracts "version"   │       │     VERSION              │
│   - Sets APP_VERSION env │       │                          │
└──────────┬───────────────┘       └──────────┬───────────────┘
           │                                   │
           ▼                                   ▼
┌──────────────────────────┐       ┌──────────────────────────┐
│   Desktop Rust Code      │       │   Android Rust Code      │
│   env!("APP_VERSION")    │       │   env!("CARGO_PKG_...")  │
└──────────────────────────┘       └──────────────────────────┘
```

## Data Flow

1. **开发者修改**：更新 `tauri.conf.json` 中的 `version` 字段。
2. **构建触发**：执行 `cargo build` 或 `cargo tauri build`。
3. **脚本执行**：`build.rs` 运行，读取 `tauri.conf.json`。
4. **解析提取**：通过简单的文本扫描定位 `"version"` 键并提取其值。
5. **环境注入**：通过 `cargo:rustc-env` 指令将值传递给 Rust 编译器。
6. **代码引用**：Rust 源码中的 `env!("APP_VERSION")` 宏在编译期展开为字面量字符串。

## Connections

[[version-json-push-before-release-404]]、[[release-asset-integrity]]