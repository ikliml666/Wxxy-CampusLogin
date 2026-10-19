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

本模块确立了应用版本号的单一事实来源（Single Source of Truth）机制。`tauri-app/src-tauri/tauri.conf.json` 中的 `version` 字段是桌面端唯一的权威源；桌面端通过 `build.rs` 在编译期将其提取并注入为 Rust 环境变量 `APP_VERSION`，代码中统一使用 `env!("APP_VERSION")` 引用。安卓端通过 `android/src-tauri/Cargo.toml` 独立声明版本号，并通过 path 依赖共享桌面端的协议核心库。此设计确保了桌面端版本号的动态获取能力，同时保持了 Cargo 体系的兼容性。

## Key Components

### tauri-app/src-tauri/tauri.conf.json
- **Line 3**: `"version": "2.4.1"` — 版本号定义处，所有桌面端构建的版本号均源自此字符串。这是版本管理的唯一入口。

### tauri-app/src-tauri/build.rs
- **Line 1–38**: 整个文件作为 Tauri 构建脚本。
    - **Line 1–2**: `use std::fs; use std::path::Path;` — 引入标准库用于文件读取和路径处理。
    - **Line 4–38**: `fn main()` — 构建脚本的主入口函数。
        - **Line 6**: `tauri_build::build();` — 调用 Tauri 原生构建逻辑，生成 IPC 代码、读取配置等基础工作。
        - **Line 8–15**: 注释说明设计目的：`tauri.conf.json` 是版本号唯一权威源，后端 Rust 代码通过 `env!("APP_VERSION")` 引用，避免硬编码。Cargo.toml 保留 version 字段以符合 cargo 强制要求，需与 tauri.conf.json 保持一致（cargo build 不一致会警告）。
        - **Line 17–19**: `let conf_path = Path::new("tauri.conf.json");` 及存在性检查。如果文件不存在则 panic。
        - **Line 20**: `let content = fs::read_to_string(conf_path).expect("Failed to read tauri.conf.json");` — 读取配置文件内容。
        - **Line 21–32**: JSON 解析逻辑。手动遍历文件行，查找以 `"version"` 开头的行，提取冒号后的值，去除引号和逗号。这是获取版本号的核心算法，不依赖外部 JSON 库以保持轻量。
            - **Line 23**: `trimmed.starts_with("\"version\"")` — 匹配键名。
            - **Line 25–28**: 分割冒号，清理空白字符和引号。
        - **Line 35**: `println!("cargo:rerun-if-changed=tauri.conf.json");` — 依赖监听指令。仅当 `tauri.conf.json` 变化时触发重编译，避免无关修改导致全量重建。
        - **Line 37**: `println!("cargo:rustc-env=APP_VERSION={version}");` — 将提取的版本号注入 Rust 编译环境变量。Rust 源码可通过 `env!("APP_VERSION")` 宏在编译期访问该值。

### android/src-tauri/Cargo.toml
- **Line 3**: `version = "2.4.1"` — 安卓端 Cargo crate 的独立版本号声明。虽然当前与桌面端保持一致，但在 Cargo 体系中它是独立的元数据源。
- **Line 31**: `campus-login = { path = "../../tauri-app/src-tauri" }` — 安卓端依赖桌面端的协议核心库（作为 path 依赖），确保网络协议层代码复用。这意味着安卓端可以直接引用桌面端定义的常量（包括可能通过 build.rs 注入的环境变量，如果桌面端 crate 暴露了这些常量的话，但通常是通过各自构建流程）。

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

1. **开发者修改**：更新 `tauri-app/src-tauri/tauri.conf.json` 中的 `version` 字段。
2. **构建触发**：执行 `cargo build` 或 `cargo tauri build`。
3. **脚本执行**：`tauri-app/src-tauri/build.rs` 运行，首先调用 `tauri_build::build()`。
4. **解析提取**：`build.rs` 读取 `tauri.conf.json`，通过文本扫描定位 `"version"` 键并提取其值（如 "2.4.1"）。
5. **环境注入**：`build.rs` 通过 `println!("cargo:rustc-env=APP_VERSION={version}")` 指令将值传递给 Rust 编译器。
6. **代码引用**：桌面端 Rust 源码中的 `env!("APP_VERSION")` 宏在编译期展开为字面量字符串 "2.4.1"。
7. **安卓端同步**：安卓端开发者需手动或在 CI/CD 中确保 `android/src-tauri/Cargo.toml` 的 `version` 字段与桌面端保持一致。

## Connections

[[version-json-push-before-release-404]]、[[release-asset-integrity]]

## Known Issues

- **手动同步风险**：安卓端的 `version` 字段（在 `Cargo.toml` 中）与桌面端的 `version` 字段（在 `tauri.conf.json` 中）目前是两个独立的实体。如果只更新了其中一个而忘记更新另一个，会导致两个平台发布的版本号不一致。建议在未来引入自动化脚本或 CI 检查来确保两者同步。
- **JSON 解析鲁棒性**：`build.rs` 中的 JSON 解析逻辑非常简化，仅基于文本行匹配。如果 `tauri.conf.json` 格式发生非标准变化（例如 `"version"` 键不在单独一行，或者被注释掉），解析可能会失败或提取错误值。对于大多数标准格式的 Tauri 配置，这通常是足够的，但需注意边界情况。