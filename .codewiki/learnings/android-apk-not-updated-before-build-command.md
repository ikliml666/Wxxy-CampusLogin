---
title: 安卓构建后 APK 没更新：tauri CLI 不跑 beforeBuildCommand
type: learning
source_files:
  - android/build-apk.ps1
  - android/src-tauri/tauri.conf.json
tags: [教训, 安卓, 构建, 前端]
---

## 现象

在 Windows 环境下执行安卓打包流程并安装生成的 APK 后，发现前端界面或交互逻辑仍是旧版本，本地源码的改动未生效。

## 根因

1. **Tauri CLI 不会自动构建前端**：Tauri 配置文件 `android/src-tauri/tauri.conf.json` 中，`build` 节点下的 `frontendDist` 设置为 `"../frontend/dist"`（行 7），`devUrl` 为 `"http://localhost:5174"`（行 8）；`bundle` 节点下 `android.minSdkVersion` 为 `29`（行 31）。CLI 在执行 `tauri android build` 时仅消费 `frontendDist` 指向的静态文件目录，不会主动执行前端构建。在 `android/build-apk.ps1:36` 的注释中明确写明：“tauri CLI 不会执行 frontend 的构建，每次打包都必须先构建，否则打进 APK 的是旧页面”。若未手动更新 `dist`，打进 APK 的就是过时的页面。

2. **Gradle up-to-date 误判导致旧包残留**：即使前端已重新构建，如果 Rust 代码没有变动，Gradle 可能判定构建任务为 up-to-date 从而跳过实际打包，保留旧的 APK 文件。如 `android/build-apk.ps1:44` 注释所述：“改过 Rust 代码时 gradle 可能判 up-to-date，保留旧 APK 会让人以为打包成功”，这会让人产生“打包成功但改动没生效”的错觉。

## 解决

仓库内提供了一键构建脚本 `android/build-apk.ps1`（与 `make-release.ps1` 完全分开，互不调用，见行 1 注释），已将完整流程自动化。常规出包直接运行 `pwsh android/build-apk.ps1` 即可。其核心组件与执行流程（依据 `android/build-apk.ps1` 源码）如下：

### 关键组件与步骤

- **参数与编码初始化**：脚本接受 `-SkipFrontend` 开关（`android/build-apk.ps1:6`），用于跳过前端构建。设置 `$ErrorActionPreference = "Stop"`（行 8），并将控制台与管道输出编码强制设为 UTF-8（行 11-12），避免子进程输出中的 `✓` 在 GBK 终端下乱码。初始化路径变量 `$root`、`$frontendDir`（行 14-15）及 `$outDir`（定义为 `src-tauri\gen\android\app\build\outputs\apk\universal\release`，行 16）。
- **JAVA_HOME 校验**：优先读取环境变量（行 19），未设置时退回默认 JDK 17 路径 `C:\Users\ik\AppData\Local\Android\jdk\jdk-17.0.20.1+1`（行 20）。校验 `bin\java.exe` 存在，否则抛出异常（行 22-24），并将 JDK 的 `bin` 目录注入 `PATH`（行 25）。
- **前端依赖安装（按需）**：若未传入 `-SkipFrontend`，且 `frontend/node_modules` 不存在时才执行 `npm install`（行 27-34），避免每次重复安装。
- **前端构建（强制）**：在非 `-SkipFrontend` 模式下，于 `frontend` 目录执行 `npx vite build`（行 36-41）。这是解决旧页面问题的核心步骤，注释见行 36。
- **清理旧产物**：调用 `Remove-Item "$outDir\*.apk"` 删除 `universal/release` 下的旧 APK（行 44-47），防止 Gradle up-to-date 遗留旧包。
- **Rust 交叉编译与 APK 打包**：在仓库根目录调用 `node "..\tauri-app\node_modules\@tauri-apps\cli\tauri.js" android build --target aarch64 --apk`（行 48-55，具体命令见行 50）。
- **查找与校验产物**：通过 `Get-ChildItem` 查找输出目录下的 APK 并按修改时间倒序取最新（行 57-59）。
- **签名校验与补签**：通过 `Get-ChildItem` 查找 `apksigner.bat`（行 63-67，遍历 `$env:LOCALAPPDATA\Android\Sdk\build-tools` 目录）。验证 APK 是否已签名（行 68-72，执行 `& $apksigner verify --print-certs`，行 70）。若已签名则输出提示（行 73-74）；若未签名，则使用 `debug.keystore` 依次执行 `zipalign.exe`（行 82）与 `apksigner sign`（行 84）进行对齐和补签，最后通过 `Move-Item` 覆盖原包（行 86）。脚本结束前会打印 APK 的完整路径与体积（行 89-90）。

另外，配置 `android/src-tauri/tauri.conf.json` 在 `security` 节点中通过 `dangerousDisableAssetCspModification`（行 24）禁用了 asset 协议对 `style-src` 的 CSP 强制修改限制，该配置属于安全限制的禁用，但与 APK 内容更新无直接关联。

### 构建流程架构

```text
┌────────────────────┐   ┌────────────────────┐   ┌────────────────────┐
│ 校验 JAVA_HOME     │──►│ 安装/构建前端      │──►│ 删除旧 APK         │
│ (build-apk.ps1:    │   │ (npm/vite :27-41)  │   │ (:44-47)           │
│  18-25)            │   └────────────────────┘   └────────────────────┘
└────────────────────┘                                │
                                                      ▼
┌────────────────────┐   ┌────────────────────┐   ┌────────────────────┐
│ 签名校验与补签     │◄──│ 查找 APK 产物      │◄──│ tauri android build│
│ (:61-87)           │   │ (:57-59)           │   │ (:48-55)           │
└────────────────────┘   └────────────────────┘   └────────────────────┘
```

如果仅修改了 Rust 代码并希望复用现有的 `dist` 目录，可在执行脚本时附加 `-SkipFrontend` 参数（定义见 `android/build-apk.ps1:6`，跳过逻辑见行 27）。

## 教训

安卓出包流程中，前端构建是与 Tauri/Rust 编译完全解耦的独立步骤。绝不能指望 `tauri CLI` 自动帮你触发前端构建（参考 `android/src-tauri/tauri.conf.json:7` 的静态路径配置与 `android/build-apk.ps1:36` 的注释）。当遇到“改动没生效”时，第一步应检查 `frontend/dist` 的时间戳与输出 APK 的生成时间。

`build-apk.ps1` 已将手动构建、清旧包、签名确认封装进自动化脚本，常规出包无需额外操作。但使用者必须理解脚本逻辑：不要在不该跳过的时候误用 `-SkipFrontend`（行 6、27），也不要在机器迁移时忽略 JAVA_HOME 的配置（行 18-25）。

## Connections

[[tauri-android-build-report-path-missing]]、[[android-host-cargo-check-fails]]
