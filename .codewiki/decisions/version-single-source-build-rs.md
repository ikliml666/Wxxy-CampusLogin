---
title: 版本号以 tauri.conf.json 为唯一权威源，build.rs 编译期注入
type: decision
source_files:
  - tauri-app/src-tauri/tauri.conf.json
  - tauri-app/src-tauri/build.rs
  - android/src-tauri/Cargo.toml
tags: [决策, 版本号, 构建, 发布]

## Overview

本决策确立了应用版本号的单一事实来源（Single Source of Truth）机制。`tauri-app/src-tauri/tauri.conf.json` 第3行的 `version` 字段（当前 `"2.4.2"`）是桌面端唯一权威源；桌面端通过 `tauri-app/src-tauri/build.rs` 在编译期提取并注入 Rust 环境变量 `APP_VERSION`，代码中统一使用 `env!("APP_VERSION")` 引用。此外，`build.rs` 在 Windows 目标下为 externalBin 生成空占位（第9行调用 `ensure_external_bin_placeholder`，函数体第48-61行），以避免无头核心 `campus-login-core` 未构建时 cargo 校验失败，打包时由 `tauri.conf.json` 第9行 `beforeBundleCommand` 用真实产物覆盖。安卓端 `android/src-tauri/Cargo.toml` 第3行独立声明版本号（当前 `"2.4.2"`），通过第31行 path 依赖共享桌面端协议核心库。此设计确保桌面端版本动态获取，保持 Cargo 兼容，并兼顾外部二进制占位需求。

## Key Components

### tauri-app/src-tauri/tauri.conf.json
- **第3行** `"version": "2.4.2"` — 桌面端版本号定义处，所有桌面构建版本源自此字符串，为唯一入口。
- **第9行** `"beforeBundleCommand": "cd src-tauri && cargo build --release --bin campus-login-core && pwsh -NoProfile -Command \"$t=(cargo metadata --format-version 1 | ConvertFrom-Json).target_directory; New-Item -ItemType Directory -Force binaries | Out-Null; Copy-Item ($t + [char]47 + 'release/campus-login-core.exe') 'binaries/campus-login-core-x86_64-pc-windows-msvc.exe' -Force\""` — 打包前命令：编译无头核心并复制为指定名称二进制，覆盖 build.rs 生成的空占位。
- 其余字段（如第2行 `productName`、第4行 `identifier`、第5-11行 `build`、第12-34行 `app`、第35-66行 `bundle`）提供应用元数据与打包参数，不直接参与版本注入。

### tauri-app/src-tauri/build.rs
- **第4-44行** `fn main()` — 构建脚本主入口。
  - **第9行** `ensure_external_bin_placeholder();` — 调用占位生成函数。
  - **第12行** `tauri_build::build();` — 调用 Tauri 原生构建（生成 IPC 代码、读取配置）。
  - **第18-21行** 构造 `tauri.conf.json` 路径，不存在则 `panic!`（第20行）。
  - **第22行** 读取配置文件全部内容至字符串。
  - **第23-38行** 提取版本号：遍历行，第27行匹配 `"version"` 前缀，第28-32行分割冒号、去除逗号与引号，得到版本字符串；未找到则第38行 `expect` panic。
  - **第41行** `println!("cargo:rerun-if-changed=tauri.conf.json");` — 仅配置变更触发重编译。
  - **第43行** `println!("cargo:rustc-env=APP_VERSION={version}");` — 注入编译期环境变量。
- **第48-61行** `fn ensure_external_bin_placeholder()` — 为 externalBin 的 `campus-login-core` 生成空占位（仅 Windows 目标）。
  - **第49-52行** 读取 `TARGET` 环境变量，获取失败则返回。
  - **第53-55行** 若 `TARGET` 不含 `"windows"` 直接 `return`（即非 Windows 平台禁用占位逻辑）。
  - **第56行** 拼接占位路径 `binaries/campus-login-core-{triple}.exe`。
  - **第57-60行** 占位不存在时创建 `binaries` 目录并写入空文件，已存在则不动。

### android/src-tauri/Cargo.toml
- **第3行** `version = "2.4.2"` — 安卓端 crate 独立版本号，当前与桌面端保持一致。
- **第31行** `campus-login = { path = "../../tauri-app/src-tauri" }` — 以 path 依赖引入桌面端协议核心库，实现代码复用。
- **第55-58行** `[profile.release]` 声明 `lto = "fat"`、`codegen-units = 1`、`strip = true`，因 path 依赖不继承桌面 profile，须在此显式配置安卓 release 优化。

## Architecture

```text
┌─────────────────────────┐    ┌─────────────────────────┐
│ tauri.conf.json          │    │ Cargo.toml (Android)    │
│ version:"2.4.2" (L3)     │    │ version="2.4.2" (L3)    │
└───────────┬─────────────┘    └───────────┬─────────────┘
            │                               │
            ▼                               ▼
┌─────────────────────────┐    ┌─────────────────────────┐
│ build.rs main (L4-44)    │    │ Android cargo build     │
│ - placeholder L9,L48-61  │    │ path dep L31            │
│ - inject APP_VERSION L43 │    │ CARGO_PKG_VERSION       │
└───────────┬─────────────┘    └───────────┬─────────────┘
            │                               │
            ▼                               ▼
┌─────────────────────────┐    ┌─────────────────────────┐
│ Desktop Rust             │    │ Android Rust            │
│ env!("APP_VERSION")     │    │ reuse core lib          │
└─────────────────────────┘    └─────────────────────────┘
```

打包覆盖：conf.json 第9行 `beforeBundleCommand` 编译真实核心并替换占位 exe。

## Data Flow

1. **开发者修改**：更新 `tauri-app/src-tauri/tauri.conf.json` 第3行 `version` 字段。
2. **非打包构建校验**：Windows 下 `cargo check`/`test` 触发 `build.rs` 第9行调用 `ensure_external_bin_placeholder`（第48-61行），生成空 `binaries/campus-login-core-{triple}.exe` 满足 externalBin 存在性校验（非 Windows 第53-55行跳过）。
3. **构建执行**：`cargo build` 或 `tauri build` 运行 `build.rs` `main`：第9行占位，第12行 `tauri_build::build()`。
4. **解析提取**：第18-22行读文件；第23-38行文本扫描定位 `"version"` 提取值（如 `"2.4.2"`）。
5. **环境注入**：第41行 `rerun-if-changed`；第43行 `cargo:rustc-env=APP_VERSION=2.4.2`。
6. **代码引用**：桌面 Rust 源码 `env!("APP_VERSION")` 编译期展开为 `"2.4.2"`。
7. **打包覆盖**：`tauri build` 执行 `tauri.conf.json` 第9行 `beforeBundleCommand`，编译 `campus-login-core` 并复制为 `binaries/campus-login-core-x86_64-pc-windows-msvc.exe` 覆盖空占位。
8. **安卓同步**：维护 `android/src-tauri/Cargo.toml` 第3行与桌面一致；构建时 cargo 提供 `CARGO_PKG_VERSION`，第31行依赖桌面核心库。

## Connections

[[version-json-push-before-release-404]]、[[release-asset-integrity]]

## Known Issues

- **手动同步风险**：安卓端 `android/src-tauri/Cargo.toml` 第3行 `version` 与桌面端 `tauri-app/src-tauri/tauri.conf.json` 第3行 `version` 为独立实体，仅人工保证一致；若只改一处会导致跨平台版本号错位。
- **JSON 解析鲁棒性**：`build.rs` 第23-38行采用简易行匹配，若 `tauri.conf.json` 中 `"version"` 非独立成行或被注释，解析将失败或误提。
- **占位条件限制**：`build.rs` 第53-55行限制占位仅在 Windows 目标生成；若跨平台 externalBin 需求变化需调整。且若打包流程未执行第9行 `beforeBundleCommand` 覆盖，分发包可能含空核心二进制（当前 Windows 打包命令显式覆盖，风险较低）。