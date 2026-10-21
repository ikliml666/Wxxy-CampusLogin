---
title: tauri-gen-schemas
type: module
source_files:
  - android/src-tauri/gen/schemas/acl-manifests.json
  - android/src-tauri/gen/schemas/android-schema.json
  - android/src-tauri/gen/schemas/capabilities.json
  - android/src-tauri/capabilities/default.json
  - android/src-tauri/gen/schemas/desktop-schema.json
  - android/src-tauri/gen/schemas/mobile-schema.json
tags: [tauri, acl, 权限, 生成物, android, schema]
---

## Overview

`tauri-gen-schemas` 是安卓端 Tauri 2 的 **ACL/能力（capability）生成物快照**，位于 `android/src-tauri/gen/schemas/`。这些文件不是人手写的源码，而是 `tauri build` / `tauri dev` 期间由 tauri-cli 依据三样输入自动产出的：

1. 应用自身在 `android/src-tauri/capabilities/*.json` 里声明的 capability 文件；
2. 各插件（官方 `core:*`、`biometric`、`notification`、`opener`，以及本项目自研的 `campus-keystore` / `campus-monitor-service` / `campus-network-bind`）在 `build.rs` / `tauri_plugin::Builder` 中登记的权限清单；
3. Tauri 2 内置的 capability JSON Schema 模板。

作用有三个：给 VSCode 的 JSON 编辑器提供 `$schema` 自动补全与错误提示；给运行时 ACL 引擎提供「哪个权限标识符允许/禁止哪些 IPC 命令」的权威映射；给前端与构建脚本提供可机器读取的插件命令面清单。因此它是**校验侧的单一事实来源**，任何对命令面的解读都应与它对齐，而不是与手写文档对齐。

注意 `gen/` 前缀的含义：整棵目录都是可再生的构建中间产物，改它会在下次构建时被覆盖。

## Key Components

### 生成文件清单（`android/src-tauri/gen/schemas/`）

| 文件 | 行/大小 | 用途 |
| --- | --- | --- |
| `acl-manifests.json` | 84 507 字节，单行 | 全部插件的能力清单：每个插件的 `default_permission`、全部 `permissions`（allow-\*/deny-\* 及各自放行/拦截的命令名）、`permission_sets`、`global_scope_schema` |
| `android-schema.json` | 2 951 行 | 安卓目标的 `CapabilityFile` JSON Schema，含 `Capability` 全字段定义与合法的权限标识符枚举 |
| `desktop-schema.json` | 2 490 行 | 桌面目标的同构 schema，是 `capabilities/default.json` 的 `$schema` 指向对象 |
| `mobile-schema.json` | 2 951 行 | 移动目标合并版 schema |
| `windows-schema.json` | 未内联 | Windows 目标 schema，本文不引用其行号 |
| `capabilities.json` | 单行 | 所有 capability 文件编译合并后的最终结果，运行时与 schema 校验都以它为准 |

### `android-schema.json` 结构（行号为真实行号）

- `$schema` 声明 `draft-07`，`title` 为 `CapabilityFile`：`android-schema.json:2-6`
- 顶层 `anyOf` 接受三种 capability 文件形态 —（a）单个能力对象、（b）能力数组、（c）含 `capabilities` 键的对象：`android-schema.json:6-37`
- `definitions.Capability`：一个能力分组的全部字段，`required` 仅 `identifier` 与 `permissions`：`android-schema.json:39-105`
  - `identifier`（string，必填）：能力标识符，如 `main-user-files-write`：`android-schema.json:47-50`
  - `description`（string，默认 `""`）：该能力放行内容的说明：`android-schema.json:51-55`
  - `remote`（`CapabilityRemote` 或 null）：允许使用该能力的远端 URL：`android-schema.json:56-66`
  - `local`（boolean，默认 `true`）：是否对本地应用 URL 生效：`android-schema.json:67-71`
  - `windows`（string[]）：受影响的窗口 label，支持 `*`、`admin-*` 之类的 glob：`android-schema.json:72-78`
  - `webviews`（string[]）：受影响的 webview label，多 webview 窗口建议用它做细粒度授权：`android-schema.json:79-85`
  - `permissions`（`PermissionEntry[]`，`uniqueItems: true`）：权限条目列表，插件权限须带 `插件名:` 前缀，应用自身实现的命令可只写权限名：`android-schema.json:86-93`
  - `platforms`（`Target[]` 或 null）：限定生效平台，默认全平台：`android-schema.json:94-104`
- `definitions.CapabilityRemote`：远端域配置，`required: ["urls"]`，`urls` 采用 URLPattern 语义（如 `https://*.mydomain.dev`）：`android-schema.json:106-121`
- `definitions.PermissionEntry`：能力内单条权限项，允许是纯字符串标识符，也允许是带 `identifier` + `allow`/`deny` 作用域数据的对象，用法见 `Capability.permissions` 的示例：`android-schema.json:87`、`android-schema.json:122-343`
- `definitions.Identifier`：**权限标识符枚举**（约 2 495 行），以 `oneOf` + `const` 把每个合法标识符固定下来，例如 `biometric:default` 与 `opener:deny-reveal-item-in-dir`，覆盖全部插件权限与拒绝项：`android-schema.json:344-2838`
- `definitions.Value`：ACL 作用域值，null / string / array / object 四种形态：`android-schema.json:2839-2877`
- `definitions.Number`：`int64` 或 `double`：`android-schema.json:2878-2892`
- `definitions.Target`：平台枚举 `macOS` / `windows` / `linux` / `android` / `iOS`：`android-schema.json:2893-2932`
- `definitions.Application`：opener 作用域的应用参数，null（默认应用）/ boolean（任意应用）/ string（指定应用）：`android-schema.json:2933-2950`

### `acl-manifests.json` 中的插件面（整文件位于第 1 行，单行 JSON 无内部行号）

共 **16 个** 插件键，按来源分两类。

官方 / Tauri 内置：

| 插件 | 放行的命令（allow-\* 对应的 command） |
| --- | --- |
| `core` | 核心默认集（`permissions` 为空，走默认能力集） |
| `core:app` | app-hide/show、bundle-type、name、version、identifier、fetch-data-store-identifiers、register/remove-listener、set-app-theme 等 |
| `core:event` | emit、emit-to、listen、unlisten |
| `core:image` | from-bytes、from-path、new、rgba、size |
| `core:menu` | new、append、prepend、popup、get、items、set-text、set-checked、set-accelerator 等 |
| `core:path` | basename、dirname、extname、is-absolute、join、normalize、resolve、resolve-directory |
| `core:resources` | close |
| `core:tray` | new、get-by-id、remove-by-id、set-icon、set-menu、set-title、set-tooltip、set-visible 等 |
| `core:webview` | create-webview、create-webview-window、get-all-webviews、print、reparent、webview-close、set-webview-size 等 |
| `core:window` | close、create、destroy、hide、show、maximize、minimize、set-title、set-size、set-focus、inner-position 等（本清单最长，约 80 余项） |
| `biometric` | `authenticate`、`status`，默认能力集含 `allow-authenticate`、`allow-status` |
| `notification` | notify、show、batch、cancel、check-permissions、request-permission、create/delete/list-channel、register-action-types 等 |
| `opener` | open-url、open-path、default-urls、reveal-item-in-dir；唯一带 `global_scope_schema` 的插件 |

本项目自研（`campus-` 前缀）：

| 插件 | 放行的命令 |
| --- | --- |
| `campus-keystore` | `encrypt`、`decrypt` — 安卓 KeyStore AES-GCM 加解密原语，密钥不出安全硬件，替代桌面端 DPAPI |
| `campus-monitor-service` | `installApk`、`isBootAutostartEnabled`、`setBootAutostart`、`startMonitor`、`stopMonitor`、`updateNotification` |
| `campus-network-bind` | `acceptWifiNetwork`、`bindToWifi`、`unbind`、`ensureAvoidBadWifi`、`restoreAvoidBadWifi`、`getSecureSettingsStatus`、`restoreWrittenSettings` |

每个插件都成对提供 `allow-<command>` 与 `deny-<command>`：`allow` 分支的 `commands.allow` 列出命令名、`deny` 为空；`deny` 分支反之。这一对被运行时 ACL 直接读取，因此「某命令是否可被前端调用」的答案只在这里。

### capability 文件与合并结果

- `android/src-tauri/capabilities/default.json`：唯一的手写能力文件，`identifier` 为 `default`，作用窗口 `["main"]`，`permissions` 只有四项 —— `core:default`、`biometric:default`、`opener:default`、`notification:default`：`capabilities/default.json:1-14`（`$schema` 在 `:2`，`permissions` 数组在 `:8-13`）
- `android/src-tauri/gen/schemas/capabilities.json`：上述文件的编译产物，单行 JSON，键为 `default`，字段与源文件一致（`identifier`、`description`、`local`、`windows`、`permissions`）：`capabilities.json:1`

## Architecture

```text
┌──────────────────────────────┐
│ capabilities/default.json    │  手写：identifier/description/
│ (:1-14)                      │  windows["main"] + 4 项 permissions
└──────────────┬───────────────┘
               │ tauri build 编译期
               ▼
┌──────────────────────────────┐        ┌─────────────────────────────┐
│ gen/schemas/capabilities.json│◄───────┤ 插件 Builder 注册的权限清单  │
│ (单行, 编译后能力集)          │        │ acl-manifests.json (第1行)   │
└──────────────┬───────────────┘        │ 16 插件: core:*/biometric/  │
               │                        │ notification/opener/campus-*│
               │                        └─────────────┬───────────────┘
               ▼                                      │
┌──────────────────────────────┐                      ▼
│ 运行时 ACL 校验              │        ┌─────────────────────────────┐
│ 前端 invoke() ──► 命令是否在 │        │ android-schema.json (:1-2950)│
│ 已授权 permission 的         │        │ CapabilityFile 定义          │
│ commands.allow 内？          │        │ + Identifier 枚举 (:344-2838)│
└──────────────────────────────┘        └─────────────────────────────┘
                                                 │ 供编辑器 $schema 校验
                                                 ▼
                                        capabilities/default.json :2
                                        （指向 desktop-schema.json）
```

## Data Flow

1. **编写**：开发者在 `android/src-tauri/capabilities/default.json` 中声明窗口与权限集（`capabilities/default.json:3-13`）。
2. **生成**：构建期 tauri-cli 汇总该能力文件与各插件的权限清单，输出 `gen/schemas/capabilities.json`（合并结果）、`acl-manifests.json`（插件命令面）以及各平台 schema（`android-schema.json`、`desktop-schema.json`、`mobile-schema.json`、`windows-schema.json`）。
3. **校验**：编辑器的 JSON 语言服务通过 `$schema`（`capabilities/default.json:2`）加载 schema，对 `identifier`/`permissions` 等字段即时报错；`Identifier` 的 `const` 枚举（`android-schema.json:344-2838`）保证写错的权限名（如拼错 `biometric:default`）当场被标记。
4. **运行**：前端调用 `invoke("encrypt", ...)` 之类的插件命令时，Tauri 运行时用能力集匹配「当前窗口 label 命中的 capability → 其 permissions 展开后的 `commands.allow`」，命中则放行，未命中直接返回 ACL 拒绝错误，Rust 侧处理函数根本不会执行。
5. **守卫**：反向的 `deny-<command>`（如 `campus-keystore:deny-decrypt`）优先于 allow，用于在宽泛的 default 权限集之上做定点关闭。

## Connections

- [[android-gen-schemas]] — 同一 `gen/` 目录下安卓构建生成物的姊妹篇，本文聚焦 ACL/schema 部分。
- [[android-tauri-shell-and-plugins]] — 这些权限清单的源头：插件在 Rust 侧如何注册、`campus-*` 插件如何挂载。
- [[android-plugins]] — 插件宿主与命令实现细节。
- [[android-rust-backend]] — 被 ACL 保护的命令真正落地在哪个模块。
- [[android-gen-build-system]] — 生成这些 schema 的构建链路（Gradle + `tauri android` 流程）。
- [[desktop-tauri-shell-config]] — 桌面端 `tauri.conf.json` 与 capability 的对应配置，用于对照双端授权差异。
- [[android-frontend-mock-tauri]] — 前端 mock 层需要与真实 ACL 放行面保持一致，否则测试通过而真机被拒。
- [[desktop-platform]] — 桌面端 DPAPI 与安卓 KeyStore 的能力对位关系（`campus-keystore` 的存在理由）。

## Known Issues

1. **`$schema` 指向桌面 schema 而非安卓 schema**：`android/src-tauri/capabilities/default.json:2` 写的是 `"../gen/schemas/desktop-schema.json"`，而同目录下同时存在更贴合安卓目标的 `android-schema.json`。两者定义基本同构，编辑器校验不会报错，但对安卓专属字段（如移动端平台枚举）的提示会退化。若需切换，应改用 `../gen/schemas/android-schema.json`。
2. **自研插件权限未出现在 default capability 中**：`capabilities/default.json:8-13` 只授权了 `core:default`、`biometric:default`、`opener:default`、`notification:default` 四项，而 `acl-manifests.json`（第 1 行）中另有 `campus-keystore`、`campus-monitor-service`、`campus-network-bind` 三个插件的权限标识符。二者不匹配意味着：要么这些命令实际由应用自身（非插件）实现、不受 capability 约束，要么需要补上 `campus-keystore:default` 之类的授权。核实前不要假定前端对这三个插件的调用一定被放行。
3. **`acl-manifests.json` 与 `capabilities.json` 是压缩的单行 JSON**：整份文件只有第 1 行，任何「行号级」引用都只能定位到 `:1`；审查命令面时应按 JSON key 路径检索，不要依赖行号。
4. **`gen/` 下全部文件为出生即弃的生成物**：手工修改会在下次 `tauri build`/`tauri dev` 时被覆盖，且可能造成「本地能跑、CI 重建后行为变化」的幽灵问题。要改权限只能改 `android/src-tauri/capabilities/*.json` 或插件注册代码。
5. **`windows-schema.json` 未纳入本文的可引用范围**：该文件在写作输入中被显式排除，因此 Windows 目标的能力定义与安卓/桌面的差异未在此文记录，需要另查该文件本体。