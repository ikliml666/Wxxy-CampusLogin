---
title: android-tauri-shell-config
type: module
source_files:
  - android/src-tauri/Cargo.toml
  - android/src-tauri/build.rs
  - android/src-tauri/tauri.conf.json
  - android/src-tauri/capabilities/default.json
  - android/src-tauri/icons/android/mipmap-anydpi-v26/ic_launcher.xml
  - android/src-tauri/icons/android/values/ic_launcher_background.xml
  - android/src-tauri/gen/schemas/capabilities.json
  - android/src-tauri/gen/android/app/src/main/assets/tauri.conf.json
  - android/plugins/keystore/permissions/default.toml
  - android/plugins/network-bind/permissions/default.toml
  - android/plugins/foreground-service/permissions/default.toml
  - android/src-tauri/src/lib.rs
  - android/src-tauri/src/protocol_cmds.rs
  - android/frontend/src/hooks/tauriApi.ts
  - android/.gitignore
  - android/src-tauri/gen/android/.gitignore
  - .gitignore
tags: [tauri2, cargo, build-config, capabilities, android, icons, shell]
---

## Overview

本模块是安卓端 Tauri 2 应用的**构建壳与配置面**：`android/src-tauri` 这个 crate 里没有业务逻辑，只有一份 Cargo 清单、一个三行的构建脚本、一份 Tauri 应用清单、一份窗口能力文件，以及图标资源。它的作用是把前端产物（`android/frontend`）挂进 Tauri 运行时、把 ACL 权限面收敛到四个官方插件、把 release 档编译参数钉死，最终由 `gen/android` 下的 Gradle 工程打成 APK。

**范围说明（重要）**：本文名为 `android-tauri-shell-config`，源码范围与全部行号锚点均落在 `android/src-tauri/` 下。仓库另有一份独立的桌面端配置（`tauri-app/src-tauri/tauri.conf.json`、`tauri-app/src-tauri/capabilities/default.json`、`tauri-app/src-tauri/capabilities/desktop.json`），本模块**未**覆盖其内容；桌面壳层请读 [[desktop-tauri-shell]]。

本模块的硬不变量（改动时必查）：

| 不变量 | 落点 |
| --- | --- |
| 版本号两处一致 | `android/src-tauri/Cargo.toml:3`、`android/src-tauri/tauri.conf.json:4` |
| 窗口 label 必须能对上 | `tauri.conf.json:11-17` 未写 `label`，由 tauri-build 补 `"main"`；`capabilities/default.json:6` 依赖该字面量 `"main"` |
| 能力文件不被忽略 | `capabilities/` 目录下全部 `*.json` 由 tauri-build 编译进 `gen/schemas/capabilities.json` |

## Key Components

本模块没有导出函数、类型或常量——只有 `build.rs` 里的 `main()` 是可执行入口，其余全部是声明式配置，因此下面按「文件 → 键/域」列出每一处可定位的构件。

### `build.rs` — 构建钩子

| 行号 | 符号 | 作用 |
| --- | --- | --- |
| `android/src-tauri/build.rs:1-3` | `fn main()` | 整个 crate 唯一的函数；函数体只有 `tauri_build::build();`（`:2`） |

`tauri_build::build()` 是后面所有 `gen/` 产物的来源：它读取 `Cargo.toml` 与 `tauri.conf.json`、编译 `capabilities/*.json`、把规范化后的应用清单落到安卓 assets，并把 ACL schema 写进 `gen/schemas/`。

### `Cargo.toml` — 包清单、依赖面与 release 档

段结构与逐项：

| 行号 | 段 / 键 | 值 | 作用 |
| --- | --- | --- | --- |
| `Cargo.toml:1-5` | `[package]` | `name = "campus-login-android"`（`:2`）、`version = "2.4.1"`（`:3`）、`description = "校园网登录助手安卓端"`（`:4`）、`edition = "2021"`（`:5`） | 包名带 `-android` 后缀，与桌面 crate `campus-login` 区分；`:3` 是版本号的第一处源 |
| `Cargo.toml:9-10` | `[lib]` | `name = "campus_login_android_lib"`（`:8`）、`crate-type = ["cdylib", "rlib"]`（`:9`） | `cdylib` 是 Android 侧 JNI 动态库的必需产物，`rlib` 让本 crate 可被其它 Rust 目标复用 |
| `Cargo.toml:12` | `[build-dependencies]` | `tauri-build = { version = "2", features = [] }`（`:12`） | 唯一构建依赖；`features = []` 表示不启用额外代码生成开关 |
| `Cargo.toml:15` | `tauri` | `version = "2", features = ["protocol-asset"]` | `protocol-asset` 是 `tauri.conf.json:20-23` 的 `assetProtocol` 生效的前提 |
| `Cargo.toml:16` | `tauri-plugin-notification` | `"2"` | 前台通知（对应 `capabilities/default.json:12`） |
| `Cargo.toml:17-18` | `serde` / `serde_json` | `derive` feature | 命令出入参与配置序列化 |
| `Cargo.toml:19` | `tokio` | features: `rt-multi-thread, time, net, macros, io-util, sync` | 运行时的六个能力面；无 `full`，避免带入未用的驱动 |
| `Cargo.toml:20` | `chrono` | `default-features = false, features = ["clock"]` | 只取本地时钟，关掉多余的 locale/serde 默认集 |
| `Cargo.toml:21-23` | `lazy_static` / `regex` / `network-interface` | `"1.5"` / `"1"` / `"2"` | 全局静态、Dr.COM 页面解析、网卡枚举 |
| `Cargo.toml:25` | `sha2` | `"0.10"`（注释在 `:24`：APK 下载完整性校验，与桌面同算法） | 校验 `version.json` 的 `sha256_checksum` |
| `Cargo.toml:28` | `libc` | `"0.2"`（注释在 `:27`：`sched_setaffinity` 绑小核做稳态功耗） | 仅安卓需要的原语；注释声明「依赖树内已有，零成本引入」 |
| `Cargo.toml:31` | `campus-login` | `{ path = "../../tauri-app/src-tauri" }` | **协议核心单点共享**：安卓不复制协议实现，直接 path 依赖桌面 crate；注释 `:30` 说明阶段 3 将收敛为 git 依赖 + tag |
| `Cargo.toml:34-40` | 三个自研插件 | `tauri-plugin-campus-network-bind`（`:34`）、`tauri-plugin-campus-keystore`（`:37`）、`tauri-plugin-campus-monitor-service`（`:40`），均为 `path` 依赖 | 注释逐条给出职责：`:33` WLAN 绑定、`:36` AndroidKeyStore AES-GCM 替代桌面 DPAPI、`:39` 前台服务保活 + 开机自启 |
| `Cargo.toml:43-44` | 官方插件 | `tauri-plugin-biometric = "2"`（`:43`）、`tauri-plugin-opener = "2"`（`:44`） | 生物识别验证门与外链/APK 打开；注释在 `:42` |
| `Cargo.toml:47-48` | 更新流 | `reqwest 0.12`（`:47`，`default-features = false`，features `rustls-tls, json, stream`）、`futures-util = "0.3"`（`:48`） | 注释 `:46` 说明选 rustls 是为避开安卓的 openssl 交叉编译问题；`stream` 支持流式下载进度 |
| `Cargo.toml:55-58` | `[profile.release]` | 注释块 `:50-54` + `lto = "fat"`（`:56`）、`codegen-units = 1`（`:57`）、`strip = true`（`:58`） | 见下方「release 档的三条约束」 |

`[profile.release]` 是本文件最容易被误改的地方，三条约束都有注释支撑：

1. **必须写在这里**：`Cargo.toml:50-52` 明确「本 crate 是安卓构建的 workspace 根，桌面端的 `[profile.release]` 不随 path 依赖生效」；缺省档下 `.so` 不 strip、无 LTO，体积「可达配置后的近两倍」。
2. **不设 `panic = "abort"`**：`Cargo.toml:53-54` 给出的理由是 Tauri 移动入口的 async 任务 panic 时 unwind 会被捕获为任务失败，而 `abort` 会直接杀死整个进程，前台服务与保活随之陪葬。
3. **`lto = "fat"` 与 `codegen-units = 1` 同时出现**：体积优先的取舍，会显著拉长 release 构建时间。

### `tauri.conf.json` — 应用清单（34 行）

| 行号 | 键 | 值 | 作用与耦合点 |
| --- | --- | --- | --- |
| `tauri.conf.json:2` | `$schema` | `https://schema.tauri.app/config/2` | 公开 JSON Schema，仅编辑器校验 |
| `tauri.conf.json:3` | `productName` | `CampusLogin` | 应用显示名 |
| `tauri.conf.json:4` | `version` | `2.4.1` | 版本号第二处源，须与 `Cargo.toml:3` 一致 |
| `tauri.conf.json:5` | `identifier` | `com.campuslogin.client` | 应用标识 |
| `tauri.conf.json:7` | `build.frontendDist` | `../frontend/dist` | Rust 侧指向安卓前端构建产物；路径相对 `android/src-tauri` |
| `tauri.conf.json:8` | `build.devUrl` | `http://localhost:5174` | 开发态前端地址，**端口 5174**（非桌面模板默认的 1420）；安卓前端 vite 配置必须对齐 |
| `tauri.conf.json:11-17` | `app.windows[0]` | `title`（`:13`）、`width: 400`（`:14`）、`height: 800`（`:15`） | **没有 `label` 字段**——label 由 tauri-build 在生成产物中补为 `"main"` |
| `tauri.conf.json:19` | `app.security.csp` | `default-src 'self'; connect-src 'self' ipc: http://ipc.localhost http://asset.localhost; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:` | 五条关键放行：`ipc:` 与 `http://ipc.localhost`（IPC）、`http://asset.localhost`（资产协议）、`data:`/`blob:`（图片与 worker）、`'unsafe-inline'` 样式、`'wasm-unsafe-eval'` 脚本 |
| `tauri.conf.json:20-23` | `app.security.assetProtocol` | `enable: true`（`:21`）、`scope: ["$APPDATA/files/face-models/**"]`（`:22`） | 资产协议根，仅开放人脸模型目录；与 `Cargo.toml:31` 的 `protocol-asset` feature 成对出现 |
| `tauri.conf.json:24` | `dangerousDisableAssetCspModification` | `["style-src"]` | 禁止 Tauri 对 `style-src` 做资产协议 CSP 改写，配合 `:19` 的 `'unsafe-inline'` |
| `tauri.conf.json:28` | `bundle.active` | `true` | 启用打包 |
| `tauri.conf.json:29` | `bundle.targets` | `"all"` | 桌面打包语义在此保留；安卓实际打包由 `gen/android` 的 Gradle 承担 |
| `tauri.conf.json:30-32` | `bundle.android.minSdkVersion` | `29`（`:31`） | APK 最低 Android 10（API 29） |

`bundle` 块（`tauri.conf.json:27-33`）里**没有 `icon` 数组**，这直接决定图标来源。

### `capabilities/default.json` — 窗口 ACL 面（14 行）

| 行号 | 键 | 值 | 说明 |
| --- | --- | --- | --- |
| `capabilities/default.json:2` | `$schema` | `../gen/schemas/desktop-schema.json` | **安卓端却指向 desktop 变体**；`gen/schemas/` 下另有 `android-schema.json` 与 `mobile-schema.json`，见 Known Issues 第 2 条 |
| `capabilities/default.json:3` | `identifier` | `default` | 能力集名 |
| `capabilities/default.json:4` | `description` | 安卓端主窗口能力：官方 biometric（验证门）/opener（外链、APK 安装）/notification（监控前台通知权限请求）前端 API | 描述即职责声明 |
| `capabilities/default.json:5-7` | `windows` | `["main"]`（`:6`） | 只作用于 label 为 `main` 的窗口 |
| `capabilities/default.json:8-13` | `permissions` | `core:default`（`:9`）、`biometric:default`（`:10`）、`opener:default`（`:11`）、`notification:default`（`:12`） | **只有四项**；三个自研插件未获授权，见 Known Issues 第 3 条 |

### 图标资源

| 文件 / 目录 | 行号 | 内容 |
| --- | --- | --- |
| `icons/android/mipmap-anydpi-v26/ic_launcher.xml` | `:1-5` | `<adaptive-icon>`；`foreground` 引用 `@mipmap/ic_launcher_foreground`（`:3`）、`background` 引用 `@color/ic_launcher_background`（`:4`） |
| `icons/android/values/ic_launcher_background.xml` | `:1-4` | `<color name="ic_launcher_background">#fff</color>`（`:3`）——纯白背景 |
| `icons/android/mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/` | — | 每档三张：`ic_launcher.png`、`ic_launcher_foreground.png`、`ic_launcher_round.png` |
| `icons/ios/` | — | `AppIcon-*.png` 全套（20/29/40/60/76/83.5/512 各倍率） |
| `icons/` 根 | — | `32x32.png`、`64x64.png`、`128x128.png`、`128x128@2x.png`、`icon.png`、`icon.ico`、`icon.icns`、`tray-icon.png`、`Square*Logo.png`、`StoreLogo.png` |

因为 `bundle` 块（`tauri.conf.json:27-33`）没有 `icon` 字段，生成产物里该字段被填成空数组，**安卓桌面图标不由 `bundle.icon` 驱动**，而由 `icons/android/` 这套手工 res 承担。改图标改 `icons/android/mipmap-*` 与两个 XML，改 `bundle.icon` 无效。

### 生成产物（`gen/`，已入库）

| 文件 | 行号 | 内容与含义 |
| --- | --- | --- |
| `gen/schemas/capabilities.json` | `:1`（单行 JSON） | `capabilities/default.json` 的编译结果：`identifier` 为 `default`、`local` 为 `true`、`windows` 为 `["main"]`、`permissions` 为四项。`local: true` 表示应用本地能力集，不是远程窗口 |
| `gen/schemas/{android,mobile,desktop,windows}-schema.json` + `acl-manifests.json` | — | ACL 六件产物；四个 `*-schema.json` 分别对应 Android / iOS（mobile）/ 桌面 / Windows 的平台权限 schema |
| `gen/android/app/src/main/assets/tauri.conf.json` | `:1`（单行 JSON） | `tauri.conf.json` 的规范化副本：补全默认字段、给窗口补 `"label":"main"` 与 `"url":"index.html"`、`security.capabilities` 落为空数组 `[]`（空表示采纳 `capabilities/` 目录下全部能力文件）、`app.pattern.use` 为 `"brownfield"`、`devUrl` 带尾斜杠 `http://localhost:5174/`、`bundle.icon` 为空 |

## Architecture

```text
┌────────── android/src-tauri/Cargo.toml:1-58 ──────────┐
│ campus-login-android 2.4.1 (:3)                        │
│ lib cdylib+rlib (:8-9) · workspace 根 (:50-54)         │
│ tauri 2 + protocol-asset (:15)                         │
│ campus-login path (:31) · 3 自研插件 (:34,37,40)       │
│ [profile.release] (:55-58)                             │
└──────────────────────────┬─────────────────────────────┘
                           │ cargo
                           ▼
      ┌──── build.rs:1-3  main() ► tauri_build::build() ───┐
      └────────┬─────────────────────────────┬─────────────┘
               │ 读                           │ 读
               ▼                              ▼
 ┌───────────────────────────┐  ┌─────────────────────────┐
 │ tauri.conf.json:1-34      │  │ capabilities/default.   │
 │ frontendDist:7 devUrl:8   │  │ json:1-14               │
 │ csp:19 assetProtocol:20-23│  │ windows ["main"]:5-7    │
 │ bundle.android.minSdk:31  │  │ permissions:8-13        │
 └────────────┬──────────────┘  └───────────┬─────────────┘
              │ 规范化+补 label "main"         │ 编译 ACL
              ▼                              ▼
 ┌──────────────────────────────┐  ┌─────────────────────┐
 │ gen/android/.../assets/      │  │ gen/schemas/        │
 │ tauri.conf.json:1            │  │ capabilities.json:1 │
 └──────────────┬───────────────┘  └──────────┬──────────┘
                └──────► Gradle assemble ◄────┘
```

图外两条旁路：`Cargo.toml:31` 的 path 依赖把桌面 crate `campus-login` 编进同一个 `.so`；`icons/android/` 的手工 res 直接进 Gradle 资源目录，不经 `bundle.icon`。

装配关系分三层：

1. **编译层**：`Cargo.toml` 决定二进制内容。因为 `android/src-tauri` 是 workspace 根（`Cargo.toml:51-55`），`[profile.release]` 才对整棵依赖图（含三个 path 插件与桌面协议 crate）生效。
2. **清单层**：`tauri.conf.json` 决定窗口、CSP、资产协议与最小 SDK；`capabilities/default.json` 决定哪个窗口能用哪些官方插件 API。两者由 `build.rs:2` 一次性消费。
3. **产物层**：`gen/` 是这两份输入的规范化投影。它入库（`android/.gitignore` 只忽略 `src-tauri/target/` 与 `.tauri/`；根 `.gitignore:19` 只忽略 `tauri-app/src-tauri/gen/`），因此可 diff，也容易产生噪音 diff。

## Data Flow

1. **确定 workspace 根**：`cargo` 在 `android/src-tauri` 看到 `Cargo.toml`（无 `[workspace]` 表），该包自成根。`[profile.release]`（`Cargo.toml:51-54`）因此作用于全量依赖，包括 path 进来的 `campus-login`（`:31`）与三个插件（`:34`、`:37`、`:40`）。
2. **执行构建钩子**：`cargo` 先编 `build.rs` 并调 `main()`（`build.rs:1-3`）→ `tauri_build::build()`。此调用是所有生成产物的唯一入口。
3. **读取并校验清单**：`tauri_build` 读 `Cargo.toml` 的 `package.version`（`:3`）与 `tauri.conf.json` 的 `version`（`:4`）。两处必须同为 `2.4.1`，否则清单与二进制版本错位。
4. **编译能力面**：`capabilities/default.json:1-14` 被编译成 `gen/schemas/capabilities.json:1`，同时落出 `android-schema.json`、`mobile-schema.json`、`desktop-schema.json`、`windows-schema.json` 与 `acl-manifests.json`。编译结果里 `windows: ["main"]` 与四项权限（`capabilities/default.json:9-12`）原样保留，并多出 `local: true`。
5. **规范化应用清单**：`tauri.conf.json:1-34` 被补全默认值后落到 `gen/android/app/src/main/assets/tauri.conf.json:1`。窗口因为原文没有 `label`（`tauri.conf.json:11-17`），在这里被补成 `"label":"main"`、`"url":"index.html"`；`security.capabilities` 落为 `[]`。
6. **窗口 label 与 ACL 对上**：`capabilities/default.json:6` 的 `"main"` 与第 5 步补出的 label 相等，能力集才挂到主窗口。若日后有人在 `tauri.conf.json:11-17` 显式写了别的 `label`，这条匹配会静默断掉——能力集仍在，只是窗口不匹配。
7. **前端产物与资源**：运行/打包时前端从 `../frontend/dist`（`tauri.conf.json:7`）加载；开发态走 `http://localhost:5174`（`:8`），安卓前端 vite 端口必须为 5174。人脸模型目录经资产协议放行（`:22` 的 `$APPDATA/files/face-models/**`），对应 CSP 的 `http://asset.localhost`（`:19`）；`assetProtocol.enable`（`:21`）依赖 `Cargo.toml:15` 的 `protocol-asset`。安卓桌面图标由 `icons/android/`（`ic_launcher.xml:1-5`、`ic_launcher_background.xml:1-4`）直接供给 Gradle res。
8. **前端调用落在应用命令通道**：`capabilities/default.json:8-13` 只授了四个官方插件，但前端真正调的自研能力不走插件命令——`android/frontend/src/hooks/tauriApi.ts:199` 是 `invoke('bind_to_wifi')`，对应应用自身命令 `protocol_cmds.rs:183`（`pub fn bind_to_wifi(app: tauri::AppHandle)`），由 `lib.rs:66-68` 的 `generate_handler!` 注册。应用命令只需 `core:default`，所以三个自研插件的权限集虽存在，不出现在 capability 里也能跑通。

## Connections

- [[desktop-tauri-shell]] —— 桌面端 Tauri 外壳与同名配置文件（`tauri-app/src-tauri/tauri.conf.json`、`capabilities/default.json`、`capabilities/desktop.json`），与本文是双端对照关系。
- [[android-tauri-shell-and-plugins]] —— 安卓壳层与插件的整体装配视角；本文只切配置与构建这一段，两者有重叠但侧重不同。
- [[android-gen-schemas]] —— `gen/schemas/capabilities.json` 与四个 `*-schema.json` 的详细解读。
- [[android-gen-build-system]] —— `gen/android` 的 Gradle 工程，接收本文第 5 步产出的 assets 清单与图标 res。
- [[android-icons-resources]]、[[android-native-resources]] —— 图标与原生资源目录的完整清单。
- [[android-rust-backend]] —— `Cargo.toml:31` 的 path 依赖所承载的协议核心单点共享（另见 decisions `protocol-core-single-source` 与 concepts [[dual-platform-sharing]]）。
- [[plugin-keystore]]、[[plugin-network-bind]]、[[plugin-foreground-service]] —— `Cargo.toml:34`、`:37`、`:40` 三个 path 插件各自的权限集与实现。
- [[ipc-command-surface]] —— `bind_to_wifi` 这类应用命令与插件命令的边界，解释 Data Flow 第 8 步为何 capability 只列四项。
- [[gen-schemas-crlf-diff-noise]]、[[cargo-lock-manual-bump-collateral-sed]] —— 修改 `gen/` 与 `Cargo.lock` 时的坑。

## Known Issues

1. **文件命名与源码范围错配**：旧版本文名曾为 `desktop-tauri-shell-config`，但给定源码与全部行号锚点都在 `android/src-tauri/`（安卓端）。桌面端同名配置实际存在于 `tauri-app/src-tauri/tauri.conf.json`、`tauri-app/src-tauri/capabilities/default.json`、`tauri-app/src-tauri/capabilities/desktop.json`，本文未覆盖。读者若按标题找桌面配置会落空，需按路径区分。

2. **`capabilities/default.json:2` 的 `$schema` 指错平台**：安卓端能力文件的 schema 写成 `../gen/schemas/desktop-schema.json`，而 `gen/schemas/` 下同时存在 `android-schema.json` 与 `mobile-schema.json`。这只影响编辑器校验（`$schema` 不参与构建），但会让 IDE 按桌面权限集报错或漏报，掩盖真实的安卓权限问题。

3. **三个自研插件有权限集，但 capability 未授予**：`Cargo.toml:37`、`:37`、`:40` 引入的 `network-bind`、`keystore`、`monitor-service` 各自在 `permissions/default.toml` 定义了 default 权限集（例如 `android/plugins/keystore/permissions/default.toml:7` 要求 `allow-encrypt`/`allow-decrypt`），而 `capabilities/default.json:8-13` 只列了 `core:default`、`biometric:default`、`opener:default`、`notification:default`。当前能工作是因为前端调应用命令而非插件命令（`tauriApi.ts:199` → `protocol_cmds.rs:183` → `lib.rs:66-68`）。**若将来前端改为直接 `invoke('plugin:…')`，必须在此文件补上对应前缀，否则被 ACL 拒绝。**

4. **窗口 label 隐式依赖**：`tauri.conf.json:11-17` 没有 `label`，`capabilities/default.json:6` 硬编码 `"main"`。这层关系只靠 tauri-build 的默认补齐（见 `gen/android/app/src/main/assets/tauri.conf.json:1` 的 `"label":"main"`）维系，配置里没有任何显式声明提示它。改动窗口配置时不要引入其它 label。