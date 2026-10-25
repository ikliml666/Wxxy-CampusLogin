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

本决策确立了应用版本号的单一事实来源（Single Source of Truth）机制。`tauri-app/src-tauri/tauri.conf.json` 中的 `version` 字段（行3）是桌面端唯一的权威源；桌面端通过 `tauri-app/src-tauri/build.rs` 在编译期将其提取并注入为 Rust 环境变量 `APP_VERSION`（行43），代码中统一使用 `env!("APP_VERSION")` 引用。安卓端通过 `android/src-tauri/Cargo.toml` 独立声明版本号（行3），并通过 path 依赖（行31）共享桌面端的协议核心库。此外，`build.rs` 还包含 `ensure_external_bin_placeholder` 函数（行48-60）为无头核心二进制生成空占位以通过 Tauri 构建校验。此设计确保了桌面端版本号的动态获取能力，同时保持了 Cargo 体系的兼容性。

## Key Components

### tauri-app/src-tauri/tauri.conf.json
- **行3**: `"version": "2.4.1"` — 版本号定义处，所有桌面端构建的版本号均源自此字符串。这是版本管理的唯一入口。

### tauri-app/src-tauri/build.rs
- **行4-44**: `fn main()` — 构建脚本的主入口函数。
  - **行1-2**: `use std::fs; use std::path::Path;` — 引入标准库用于文件读取和路径处理。
  - **行5-9**: 注释及调用说明 externalBin 占位背景：Tauri 构建脚本在 check/test 阶段校验 externalBin 路径存在，无头核心 bin 往往尚未构建，先落空占位让校验通过，正式打包时由 `beforeBundleCommand` 覆盖（行9 为调用 `ensure_external_bin_placeholder();`）。
  - **行11-12**: 注释及 `tauri_build::build();` — 调用 Tauri 原生构建逻辑，生成 IPC 代码、读取配置等基础工作。
  - **行14-17**: 注释设计目的：`tauri.conf.json` 是版本号唯一权威源，后端 Rust 代码通过 `env!("APP_VERSION")` 引用，避免硬编码；Cargo.toml 保留 version 字段以符合 cargo 强制要求，需与 tauri.conf.json 保持一致（cargo build 不一致会警告）。
  - **行18-21**: `let conf_path = Path::new("tauri.conf.json");` 及存在性检查，若文件不存在则 panic。
  - **行22**: `let content = fs::read_to_string(conf_path).expect("Failed to read tauri.conf.json");` — 读取配置文件内容。
  - **行23-38**: JSON 解析逻辑。手动遍历文件行，查找以 `"version"` 开头的行，提取冒号后的值，去除引号和逗号。核心算法不依赖外部 JSON 库以保持轻量。
  - **行41**: `println!("cargo:rerun-if-changed=tauri.conf.json");` — 依赖监听指令，仅当配置变化时触发重编译。
  - **行43**: `println!("cargo:rustc-env=APP_VERSION={version}");` — 将提取的版本号注入 Rust 编译环境变量，源码可通过 `env!("APP_VERSION")` 访问。
- **行48-60**: `fn ensure_external_bin_placeholder()` — 为 externalBin 的 campus-login-core 生成空占位（仅 Windows 目标；已存在则不动，打包时由 beforeBundleCommand 以真实产物覆盖）。行49-55 匹配 `TARGET` 环境变量含 windows 才继续，行56-60 在 binaries 目录写空文件。

### android/src-tauri/Cargo.toml
- **行3**: `version = "2.4.1"` — 安卓端 Cargo crate 的独立版本号声明。虽然当前与桌面端保持一致，但在 Cargo 体系中它是独立的元数据源。
- **行31**: `campus-login = { path = "../../tauri-app/src-tauri" }` — 安卓端依赖桌面端的协议核心库（作为 path 依赖），确保网络协议层代码复用，可直接引用桌面端常量。

## Architecture

```text
┌──────────────────────────┐       ┌──────────────────────────┐
│ tauri.conf.json          │       │ Cargo.toml (Android)     │
│ (Source of Truth)        │       │ (Android Version)        │
│ 行3: v2.4.1              │       │ 行3: v2.4.1              │
└───────────┬──────────────┘       └───────────┬──────────────┘
            │                                   │
            ▼                                   ▼
┌──────────────────────────┐       ┌──────────────────────────┐
│ build.rs (Desktop)       │       │ Android Build            │
│ 行4-44 main()            │       │                          │
│ - 行23-38 解析 version   │       │ - 使用 CARGO_PKG_VERSION  │
│ - 行43 注入 APP_VERSION  │       │                          │
└───────────┬──────────────┘       └───────────┬──────────────┘
            │                                   │
            ▼                                   ▼
┌──────────────────────────┐       ┌──────────────────────────┐
│ Desktop Rust Code        │       │ Android Rust Code        │
│ env!("APP_VERSION")      │       │ env!("CARGO_PKG_...")    │
└──────────────────────────┘       └──────────────────────────┘
```

## Data Flow

1. **开发者修改**：更新 `tauri-app/src-tauri/tauri.conf.json` 中的 `version` 字段（行3）。
2. **构建触发**：执行 `cargo build` 或 `cargo tauri build`。
3. **脚本执行**：`tauri-app/src-tauri/build.rs` 的 `main` 运行，首先调用 `ensure_external_bin_placeholder()`（行9）生成 Windows 占位（若需要）。
4. **原生构建**：调用 `tauri_build::build()`（行12）处理 Tauri 配置与 IPC 生成。
5. **解析提取**：`build.rs` 读取 `tauri.conf.json`（行18-22），通过文本扫描定位 `"version"` 键并提取其值（行23-38）。
6. **环境注入**：`build.rs` 通过 `println!("cargo:rustc-env=APP_VERSION={version}")` 指令（行43）将值传递给 Rust 编译器。
7. **代码引用**：桌面端 Rust 源码中的 `env!("APP_VERSION")` 宏在编译期展开为字面量字符串。
8. **安卓端同步**：安卓端开发者需手动或在 CI/CD 中确保 `android/src-tauri/Cargo.toml` 的 `version` 字段（行3）与桌面端保持一致。

## Connections

[[version-json-push-before-release-404]]、[[release-asset-integrity]]

## Known Issues

- **手动同步风险**：安卓端的 `version` 字段（在 `android/src-tauri/Cargo.toml` 行3）与桌面端的 `version` 字段（在 `tauri-app/src-tauri/tauri.conf.json` 行3）目前是两个独立的实体。如果只更新了其中一个而忘记更新另一个，会导致两个平台发布的版本号不一致。建议在未来引入自动化脚本或 CI 检查来确保两者同步。
- **JSON 解析鲁棒性**：`build.rs` 中的 JSON 解析逻辑（行23-38）非常简化，仅基于文本行匹配。如果 `tauri.conf.json` 格式发生非标准变化（例如 `"version"` 键不在单独一行，或者被注释掉），解析可能会失败或提取错误值。对于大多数标准格式的 Tauri 配置，这通常是足够的，但需注意边界情况。