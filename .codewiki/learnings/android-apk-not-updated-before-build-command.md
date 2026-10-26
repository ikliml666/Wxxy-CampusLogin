---
title: 安卓构建后 APK 没更新：tauri CLI 不跑 beforeBuildCommand
type: learning
source_files:
  - android/build-apk.ps1
  - android/src-tauri/tauri.conf.json
tags: [教训, 安卓, 构建, 前端]
---

## 现象

安卓打包完成后安装 APK，发现界面或逻辑仍是旧版本，代码改动未生效。

## 根因

tauri CLI 在执行 `tauri android build` 时不会自动构建前端资源，`frontend/dist` 目录（由 `android/src-tauri/tauri.conf.json:7` 的 `frontendDist: "../frontend/dist"` 指定）若未手动更新，会保留上一次构建的产物，导致打进 APK 的是旧页面。

此外，即使前端已重新构建，若 Rust 代码未改动，Gradle 可能判定构建 up-to-date 而保留旧的 APK 文件（见 `android/build-apk.ps1:44` 注释：“改过 Rust 代码时 gradle 可能判 up-to-date，保留旧 APK 会让人以为打包成功”），造成“打包成功”的错觉。

## 解决

一键脚本 `android/build-apk.ps1` 已将完整流程自动化，常规出包直接运行 `pwsh android/build-apk.ps1` 即可。核心步骤（依据当前源码）：

1. **校验 JAVA_HOME**：优先使用环境变量，否则退回默认 JDK 17 路径（`android/build-apk.ps1:19-25`）。
2. **按需安装前端依赖**：仅当 `frontend/node_modules` 不存在时才执行 `npm install`（`android/build-apk.ps1:29-34`），避免每次打包重复安装。
3. **强制构建前端**：除非传入 `-SkipFrontend`，否则在 `frontend` 目录执行 `npx vite build`（`android/build-apk.ps1:27-41`，关键注释见行 36 “tauri CLI 不会执行 frontend 的构建，每次打包都必须先构建，否则打进 APK 的是旧页面”）。
4. **删除旧 APK**：构建前清空 `universal/release` 输出目录下的 `*.apk`（`android/build-apk.ps1:44-47`），防止 Gradle up-to-date 遗留旧包。
5. **交叉编译与打包**：调用 `node "..\tauri-app\node_modules\@tauri-apps\cli\tauri.js" android build --target aarch64 --apk`（`android/build-apk.ps1:48-55`）。
6. **签名校验与补签**：查找 apksigner 并验证产物是否已签名（`android/build-apk.ps1:63-72`），未签名则用 debug.keystore 执行 zipalign + apksigner（`android/build-apk.ps1:75-87`）。

如需使用现有 `dist`（例如仅改 Rust 代码），可加 `-SkipFrontend` 开关跳过前端构建（参数定义见 `android/build-apk.ps1:6`）。

## 教训

安卓出包流程中前端构建是独立一步，绝不能指望 tauri CLI 帮你跑；看到“改动没生效”先查 `dist` 时间戳与输出 APK 的生成时间。`build-apk.ps1` 已将手动构建、清旧包、签名确认封装进自动化脚本，常规出包无需额外操作，但需理解脚本逻辑以避免误用 `-SkipFrontend` 或忽略 JAVA_HOME 配置。

## Connections

[[tauri-android-build-report-path-missing]]、[[android-host-cargo-check-fails]]
