---
title: build_config
type: module
source_files:
  - tauri-app/build.ps1
  - make-release.ps1
  - clean-build-cache.ps1
  - android/build-apk.ps1
  - tauri-app/frontend/vite.config.ts
  - tauri-app/frontend/vite.mock.config.ts
  - tauri-app/frontend/vitest.config.ts
  - tauri-app/frontend/tailwind.config.js
  - tauri-app/frontend/postcss.config.js
  - tauri-app/frontend/tsconfig.json
  - tauri-app/frontend/tsconfig.node.json
  - tauri-app/frontend/package.json
  - tauri-app/frontend/index.html
  - tauri-app/frontend/tauri-mock.js
  - tauri-app/frontend/about-preview.html
  - tauri-app/pnpm-workspace.yaml
  - tauri-app/src-tauri/build.rs
  - tauri-app/src-tauri/Cargo.toml
  - tauri-app/src-tauri/tauri.conf.json
  - tauri-app/src-tauri/tauri.windows.conf.json
  - tauri-app/src-tauri/.cargo/config.toml
  - tauri-app/src-tauri/windows/hooks.nsh
tags: [build, bundler, vite, tauri, rust, nsis, release, powershell, frontend, android]
---

## Overview

`build_config` 模块覆盖「校园网登录助手」（campus-login，当前版本 `2.4.3`，见 `tauri-app/src-tauri/tauri.conf.json:3` 与 `tauri-app/src-tauri/Cargo.toml:3`）全平台构建与打包的所有脚本与配置：前端 Vite/TS 打包、Rust 后端编译、Tauri 桌面（NSIS）安装包、以及安卓 APK 出包。

工程分两条独立打包链路，互不调用（见 `make-release.ps1:11`、注释与 `android/build-apk.ps1:1` 的明确说明）：

- **桌面端（Windows）**：由 `tauri-app/build.ps1` 串起「环境校验 → 前端 `npm install`/`npm run build` → Tauri 编译 → 拷贝产物到 `tauri-release/`」，再由 `make-release.ps1` 把 NSIS 安装包重命名/加 `.sha256` 汇总到 `release/`；`clean-build-cache.ps1` 负责清理编译缓存。
- **安卓端**：由 `android/build-apk.ps1` 单独走 `tauri android build`，产出 APK，不进 `release/`。

版本号以 `tauri.conf.json` 为唯一权威源：Rust 侧 `build.rs`（第 18–43 行）启动时解析该文件注入 `APP_VERSION` 编译期环境变量，`Cargo.toml` 的 `version` 字段仅因 cargo 强制要求保留、需与之保持一致（`build.rs:23`–`17`）。`make-release.ps1:43`–`47` 与 `clean-build-cache.ps1:132`–`134` 也直接读 `tauri.conf.json` 取版本号/`identifier`。

构建系统的关键约束与奇技（均来自源码注释，写明具体位置）：
- cargo 输出目录由 `tauri-app/src-tauri/.cargo/config.toml` 的 `target-dir` 重定向到项目外（`config.toml:7`），原因见该文件第 3–6 行（沙箱 Low 完整性标签导致 build-script 以 Low IL 运行而写 `%TEMP%` 失败）。`make-release.ps1:64`–`72` 与 `clean-build-cache.ps1:48`–`55` 都解析它来定位真实产物。
- 无头核心 `campus-login-core` 作为 `externalBin` 嵌进安装包：`build.rs:4`–`9` 先落空占位让 `cargo check`/`test` 校验通过，正式打包时由 `tauri.conf.json` 的 `beforeBundleCommand`（`tauri.conf.json:9`）编译真实产物覆盖。

## Key Components

下列组件均来自 `source_files` 列表中的真实文件。行号以 `read_code` 返回的「行号| 」前缀为准。

### 桌面构建编排 `tauri-app/build.ps1`

PowerShell 脚本，5 步流程（标题见第 32/57/82/99/154 行）：
- 参数 `param(...)`（`build.ps1:1`–`5`）：`-SkipFrontend`、`-SkipCopy`、`-TargetDir`（覆盖 `CARGO_TARGET_DIR`）。
- 路径推导（`build.ps1:15`–`20`）：`$scriptDir`/`$tauriAppDir` 为 `tauri-app/`，`$releaseOutputDir` 为仓库根的 `tauri-release/`。
- `[1/5]` 环境校验（`build.ps1:32`–`54`）：`Get-Command` 检查 `rustc`/`node`/`cargo` 缺失即 `exit 1`。
- `[2/5]` 依赖安装（`build.ps1:57`–`79`）：未带 `-SkipFrontend` 时，缺 `frontend/node_modules` 才 `npm install`（`build.ps1:60`–`64`），缺 `tauri-app/node_modules` 才 `npm install`（`build.ps1:69`–`73`）。
- `[3/5]` 前端构建（`build.ps1:82`–`96`）：`npm run build`（`build.ps1:86`）；带 `-SkipFrontend` 时用既有 `dist/index.html`（`build.ps1:90`–`94`），否则报错退出。
- `[4/5]` Tauri 编译（`build.ps1:99`–`145`）：`$env:CARGO_BUILD_JOBS = $cpuCount`（`build.ps1:101`–`102`）；`$env:CARGO_TARGET_DIR` 默认仓内 `target/`（`build.ps1:111`–`113`，注释说明 2026-09-15 起由 `C:\cl-build` 短路径内化），`-TargetDir` 可逃生（`build.ps1:105`–`107`）；直接调用 `node node_modules\@tauri-apps\cli\tauri.js build`（`build.ps1:119`–`130`，`$tauriCliJs` 路径，`build.ps1:119`），不进管道以免丢 tty 进度（注释 `build.ps1:116`–`118`）；用 `Stopwatch` 计时（`build.ps1:122`–`137`）。
- `[5/5]` 拷贝产物（`build.ps1:153`–`213`）：从 `<target>/release/bundle` 递归拷到 `tauri-release/`（`build.ps1:169`–`178`），并单独拷 `campus-login.exe`（`build.ps1:180`–`184`）；为每个 `*-setup.exe` 生成 shasum 兼容的 `.sha256`（`build.ps1:189`–`193`）。若 `-SkipCopy` 则早退（`build.ps1:147`–`151`）。

### 桌面发布汇总 `make-release.ps1`

- 参数 `param([switch]$SkipBuild)`（`make-release.ps1:23`–`25`）。
- `Fail([string]$msg)`（`make-release.ps1:38`–`41`）：报错并 `exit 1`。
- 版本权威源读取（`make-release.ps1:43`–`48`）：从 `tauri-app\src-tauri\tauri.conf.json` 的 `version` 字段解析（UTF-8 输出设置 `make-release.ps1:30`–`31`）。
- 构建（`make-release.ps1:50`–`62`）：未带 `-SkipBuild` 时 `npx.cmd tauri build`，不接管道保 tty。
- 定位 cargo 真实输出目录（`make-release.ps1:64`–`72`）：解析 `tauri-app\src-tauri\.cargo\config.toml` 的 `target-dir`。
- 安装包选取（`make-release.ps1:78`–`98`）：遍历 `cargoTarget`/`$env:CARGO_TARGET_DIR`/仓内 `target`/`src-tauri/target` 的 `release\bundle\nsis`，按正则 `_${version}_[A-Za-z0-9]+-setup$` 匹配**当前版本**（`make-release.ps1:85`），绝「取最新」（`make-release.ps1:75`–`77` 注释），旧包列 `staleExes` 并告警（`make-release.ps1:90`–`98`）。
- 资产重命名与校验（`make-release.ps1:100`–`113`）：`Wxxy-CampusLogin_${version}_${arch}-setup.exe`，生成同名 `.sha256`。
- 收集 `RELEASE_NOTES_v<version>.md`（`make-release.ps1:118`–`122`），提示历史残留文件（`make-release.ps1:133`–`139`）。

### 构建缓存清理 `clean-build-cache.ps1`

- 参数 `[CmdletBinding(SupportsShouldProcess, ConfirmImpact="High")] param()`（`clean-build-cache.ps1:40`–`41`）。
- cargo 真实目录解析（`clean-build-cache.ps1:49`–`55`）：同 `make-release.ps1` 解析 `.cargo/config.toml`。
- `$ownCratePattern = "^(campus-login|campus_login|tauri-plugin-campus)"`（`clean-build-cache.ps1:60`），`Test-OwnedTargetDir`（`clean-build-cache.ps1:61`–`71`）：通过 `.fingerprint` 目录名判归属，避免误删别的项目遗留目录（`clean-build-cache.ps1:57`–`82`）。
- `$targets` 列表（`clean-build-cache.ps1:84`–`95`）：仓内 `target`、旧 `C:\cl-build\target`、桌面 `src-tauri\target`、cargo `target-dir`、`android\...` 交叉编译与 gradle 产物、两个前端 `dist`。
- 进程互斥检查（`clean-build-cache.ps1:102`–`106`）：检测到 cargo/rustc/tauri/`campus-login`/应用运行中即退出。
- WebView2 缓存清理（`clean-build-cache.ps1:129`–`157`）：从 `tauri.conf.json` 读 `identifier`（`clean-build-cache.ps1:132`–`134`），删 `EBWebView` 下 `Cache`/`Code Cache`/`GPUCache`/`Dawn*`/`ShaderCache`/`Service Worker` 等可再生子目录（保留 Local/Session Storage）。
- 唯一副本告警（`clean-build-cache.ps1:164`–`195`）：APK（`clean-build-cache.ps1:169`–`175`）与 NSIS 包（`clean-build-cache.ps1:180`–`194`）若在 `release\` 无对应资产，红色警告需重跑构建。
- 实际删除（`clean-build-cache.ps1:199`–`208`）：经 `ShouldProcess` 确认后 `Remove-Item -Recurse -Force`。

### 安卓打包 `android/build-apk.ps1`

- 参数 `param([switch]$SkipFrontend)`（`android/build-apk.ps1:6`）。
- UTF-8 输出设置（`android/build-apk.ps1:10`–`12`）。
- `JAVA_HOME` 处理（`android/build-apk.ps1:18`–`25`）：未设则回退本机 JDK 17，校验 `bin\java.exe` 存在。
- 前端构建（`android/build-apk.ps1:27`–`41`）：缺 `node_modules` 才 `npm install`，每次都 `npx vite build`（Tauri CLI 不替前端构建）。
- 交叉编译打 APK（`android/build-apk.ps1:43`–`55`）：先删旧 APK（`android/build-apk.ps1:45`–`47`，gradle up-to-date 可能保留旧包），再 `node ..\tauri-app\node_modules\@tauri-apps\cli\tauri.js android build --target aarch64 --apk`（`android/build-apk.ps1:50`）。
- 签名校验与补签（`android/build-apk.ps1:61`–`87`）：`apksigner verify`；未签名则 `zipalign` + `apksigner sign`（`--ks ~/.android/debug.keystore`）。

### 前端打包器 `frontend/vite.config.ts`

- `defineConfig`（`vite.config.ts:7`）：插件 `react()`（`vite.config.ts:8`）、`@` → `./src` 别名（`vite.config.ts:9`–`13`）、`base: './'`（`vite.config.ts:14`）、`clearScreen: false`（`vite.config.ts:15`）。
- dev server（`vite.config.ts:5`–`33`）：`port: 5173`、`strictPort: true`、`host: $host`（读 `process.env.TAURI_DEV_HOST`，`vite.config.ts:26`/`19`）、HMR `ws:5174`（`vite.config.ts:20`–`24`）；`watch.ignored` 排除 `src-tauri/**` 与 `.*.tmpdir/**`（`vite.config.ts:25`–`32`，注释说明 Windows EBUSY 闪退）。
- `build`（`vite.config.ts:34`–`57`）：`outDir: 'dist'`、`emptyOutDir: true`、`sourcemap: false`、`minify: 'esbuild'`、`cssMinify: true`、`cssCodeSplit: true`、`reportCompressedSize: false`、`target: 'es2021'`、`modulePreload.polyfill: false`；`rollupOptions.maxParallelFileOps: 8`、`output.manualChunks`（`vite.config.ts:49`–`54`）把 `react`/`react-dom`、`framer-motion`、`gsap`、`@radix-ui/*` 各自切为 `vendor-*` 块。

### 浏览器 mock 配置 `frontend/vite.mock.config.ts`

- 在基础配置上叠加 `tauri-mock-inject` 插件（`vite.mock.config.ts:4`–`18`），`transformIndexHtml` 把 `/tauri-mock.js` 以 `head-prepend` 注入（先于 module 脚本），mock 住 `window.__TAURI_INTERNALS__`。
- 继承基础配置（`vite.mock.config.ts:27`–`34`）：`cacheDir` 外移到系统 TEMP（`vite.mock.config.ts:31`，绕过 esbuild 子进程 Access is denied）、`server.port: 5175`。
- 注意（`vite.mock.config.ts:22`–`25`）：本机防护软件拦原版 `@esbuild/win32-x64/esbuild.exe` 写盘，需以 `ESBUILD_BINARY_PATH` 环境变量带改名副本启动。

### 单测配置 `frontend/vitest.config.ts`

- `defineConfig`（`vitest.config.ts:5`）：`react()` 插件、`test.environment: 'jsdom'`、`globals: true`、`setupFiles: ['./src/test-setup.ts']`、`@` → `./src` 别名（`vitest.config.ts:6`–`14`）。

### 样式与类型 `frontend/tailwind.config.js` / `postcss.config.js` / `tsconfig*.json`

- `tailwind.config.js`（`tailwind.config.js:4`）：`darkMode: 'class'`、`content` 扫描 `index.html` 与 `src/**`，`theme.extend.colors` 全量映射到 `hsl(var(--*))` CSS 变量（`tailwind.config.js:7`–`53`）、`borderRadius` 用 `--radius`（`tailwind.config.js:54`–`58`）、自定义 `animation`/`keyframes`（`tailwind.config.js:72`–`103`）、`backdropBlur.xs: '2px'`（`tailwind.config.js:63`–`106`）。
- `postcss.config.js`（`postcss.config.js:3`–`7`）：插件 `tailwindcss` + `autoprefixer`。
- `tsconfig.json`（`tsconfig.json:15`–`27`）：`target: ES2020`、`module: ESNext`、`moduleResolution: bundler`、`strict`、`noUnusedLocals`/`noUnusedParameters`、`@` → `./src` 路径（`tsconfig.json:17`–`22`）、`references` 到 `tsconfig.node.json`（`tsconfig.json:25`）。
- `tsconfig.node.json`（`tsconfig.node.json:1`–`13`）：`composite: true`、`types: ["node"]`、`include: ["vite.config.ts"]`。

### 包管理 `frontend/package.json` / `pnpm-workspace.yaml`

- `frontend/package.json`（`package.json:6`–`52`）：`version: 2.4.3`、`type: module`；`scripts`（`package.json:8`–`13`）：`dev`/`build`=`vite build`/`preview`/`test`=`vitest run`/`clean`；`dependencies`（`package.json:14`–`36`，react 19、zustand、i18next、framer-motion、gsap、radix 系列等）、`devDependencies`（`package.json:37`–`50`，vite 6、vitest 4、tailwindcss 3、typescript 5 等）。
- `tauri-app/pnpm-workspace.yaml`（`pnpm-workspace.yaml:2`–`6`）：`allowBuilds.esbuild` 占位、`minimumReleaseAgeExclude` 排除 `plugin-notification@2.6.0`/`plugin-shell@2.5.0`。

### 前端入口与 mock `frontend/index.html` / `tauri-mock.js` / `about-preview.html`

- `index.html`（`index.html:1`–`120`）：`lang="zh-CN"`、首屏内联骨架屏 CSS 与 `data-light`/`dark`/`theme-*`/`data-glass` 初始化脚本（`index.html:16`–`27`，玻璃档位默认 `ultra`，`index.html:22`–`24`）、加载 `/src/main.tsx`（`index.html:117`）。
- `tauri-mock.js`（`tauri-mock.js:333`–`427`）：浏览器 mock，定义 `window.__TAURI_INTERNALS__`（`tauri-mock.js:366`–`374`），实现 `invoke`/`transformCallback` 的事件路由（`tauri-mock.js:333`–`364`），含 `get_init_data`/`get_config`/`save_config`/`query_*`/`vpn_*`/`create_account`/`switch_account` 等命令处理器（`tauri-mock.js:214`–`314`），30s 网络质量事件推送（`tauri-mock.js:100`–`331`），`?nomotion`/`?panel`/`?theme`/`?mockempty`/`?probe`/`?dialog` 等 URL 探针（`tauri-mock.js:89`–`14`、`376`–`423`）。**仅供浏览器本地调试，由 `vite.mock.config.ts` 注入，不进构建产物。**
- `about-preview.html`（`about-preview.html:1`–`278`）：关于页静态预览模板（v2.4.3，`about-preview.html:227`/`251`），用于截图/预览，非运行时入口。

### Rust 构建脚本 `src-tauri/build.rs` 与 `Cargo.toml`

- `build.rs::main()`（`build.rs:23`–`44`）：① `ensure_external_bin_placeholder()`（`build.rs:9`，见下）；② `tauri_build::build()` 生成 IPC 代码（`build.rs:12`）；③ 解析 `tauri.conf.json` 的 `version` 注入 `APP_VERSION` 环境变量（`build.rs:18`–`43`：`cargo:rerun-if-changed=tauri.conf.json` 第 41 行、`cargo:rustc-env=APP_VERSION=...` 第 43 行）。
- `ensure_external_bin_placeholder()`（`build.rs:48`–`61`）：仅 Windows 目标（`build.rs:53`），`binaries/campus-login-core-<triple>.exe` 不存在则写空占位（`build.rs:59`），打包时由 `beforeBundleCommand` 覆盖。
- `Cargo.toml`（`Cargo.toml:1`–`118`）：`package.name = campus-login`、`version = 2.4.3`、`default-run = "campus-login"`（`Cargo.toml:5`）；`[lib]` crate-type `["cdylib","rlib"]`（`Cargo.toml:10`–`12`）；`[[bin]] campus-login-core`（`Cargo.toml:16`–`18`）；依赖分 `android`（`Cargo.toml:68`–`72`）、桌面插件（`Cargo.toml:75`–`79`）、`windows`（`Cargo.toml:81`–`109`，含完整 `windows` crate features 列表）；`[profile.release]`（`Cargo.toml:111`–`118`）：`lto = "thin"`、`codegen-units = 16`、`opt-level = 3`、`strip = true`（注释说明不用 `panic="abort"`，审计 A1，`Cargo.toml:116`–`117`）。

### Tauri 打包配置 `tauri.conf.json` / `tauri.windows.conf.json` / `.cargo/config.toml` / `windows/hooks.nsh`

- `tauri.conf.json`（`tauri.conf.json:2`–`69`）：`productName`/`version`/`identifier`（`tauri.conf.json:4`–`4`）；`build`（`tauri.conf.json:5`–`11`）：`beforeDevCommand`/`devUrl:5173`/`beforeBuildCommand`/`beforeBundleCommand`（`tauri.conf.json:9`，编译并拷贝 `campus-login-core-x86_64-pc-windows-msvc.exe` 到 `binaries/`）、`frontendDist: ../frontend/dist`；`app.windows`（`tauri.conf.json:13`–`30`，无边框、`visible:false`、`center:true`）；`security.csp`（`tauri.conf.json:32`）；`bundle`（`tauri.conf.json:35`–`66`）：`targets: ["nsis"]`、`resources`、`icon`、`windows.webviewInstallMode: downloadBootstrapper`、`nsis.installMode: both`、双语言 `SimpChinese`/`English`、`installerHooks: ./windows/hooks.nsh`。
- `tauri.windows.conf.json`（`tauri.windows.conf.json:1`–`8`）：仅 Windows 合并 `bundle.externalBin: ["binaries/campus-login-core"]`（`tauri.windows.conf.json:3`–`5`）。
- `.cargo/config.toml`（`config.toml:1`–`16`）：`build.jobs = -1`、沙箱 Low IL 重定向说明（`config.toml:3`–`7`）、`target-dir` 指向项目外（`config.toml:7`）；`[profile.dev]`（`config.toml:9`–`15`）。
- `windows/hooks.nsh`（`hooks.nsh:6`–`10`）：`NSIS_HOOK_POSTUNINSTALL`（`hooks.nsh:6`）卸载时 `schtasks /delete /tn "CampusLoginPowerOps" /f`（`hooks.nsh:7`）并 `RMDir /r $COMMONPROGRAMDATA\CampusLogin`（`hooks.nsh:8`）清理 SYSTEM 计划任务与数据目录。

## Architecture

构建系统以「版本号权威源 `tauri.conf.json`」为中心，串起两条互不耦合的出包链路；Rust 侧 `build.rs` 在编译期从同一源注入 `APP_VERSION`，`externalBin` 占位由 `beforeBundleCommand` 填实。

```text
                          tauri.conf.json  (version 权威源, tauri-app/src-tauri/tauri.conf.json:3)
                          │
            ┌─────────────┴──────────────────────────────────────────────┐
            │ parse version │                                            │ read bundle/externalBin
            ▼                                                             ▼
   build.rs:18-43 (APP_VERSION)                              tauri.windows.conf.json:3 (externalBin)
            │                                                             │
            ▼                                                             ▼
   Cargo.toml:16 campus-login-core bin                  beforeBundleCommand (tauri.conf.json:9)
            │                                             cargo build --release --bin campus-login-core
            ▼                                                             │
   ensure_external_bin_placeholder (build.rs:48)  ──empty .exe ──►  binaries/campus-login-core-*.exe
            │
   ┌────────┴─────────┐
   │ 桌面链路          │  │ 安卓链路
   ▼                  ▼  ▼
 build.ps1          make-release.ps1   android/build-apk.ps1
 (npm+vite+tauri)   (rename+sha256)    (tauri android build)
   │                  │                  │
   ▼                  ▼                  ▼
 tauri-release/      release/           gen/.../universal/release/*.apk
 (NSIS bundle)       (Wxxy-*-setup.exe)(独立渠道, 见 make-release.ps1:11)
   ▲
   │ .cargo/config.toml:7 target-dir (项目外, Low IL 规避)
   │ make-release.ps1:64 / clean-build-cache.ps1:49 解析它定位产物
```

组件规模：4 个 PowerShell 构建脚本 + 前端 6 个打包配置（vite/vite.mock/vitest/tailwind/postcss/tsconfig）+ Rust 侧 build.rs/Cargo.toml/tauri.conf.json 等 6 个打包配置。前端运行时代码见子模块 [[tauri-app/frontend]]，桌面运行时后端见 [[tauri-app]]。

## Data Flow

1. **桌面端完整出包**：`tauri-app/build.ps1:32` 起 5 步——环境校验（rustc/node/cargo，`build.ps1:34`–`54`）→ 前端依赖/构建（`build.ps1:57`–`96`，`npm run build` 调 `vite build`，`build.ps1:86`）→ 设置 `CARGO_TARGET_DIR`（默认仓内 `target/`，`build.ps1:111`–`113`）→ `node tauri.js build`（`build.ps1:127`/`129`）→ 拷 `release/bundle` 与 `campus-login.exe` 到 `tauri-release/`（`build.ps1:169`–`184`）并生成 `*-setup.exe.sha256`（`build.ps1:189`–`193`）。Tauri CLI 自身会按 `tauri.conf.json` 的 `beforeDevCommand`/`beforeBuildCommand`/`beforeBundleCommand`（`tauri.conf.json:9`/`8`/`9`）驱动前后端编译与 `externalBin` 填实。
2. **编译期版本注入**：`build.rs:23`–`43` 读 `tauri.conf.json` 的 `version`，`cargo:rustc-env=APP_VERSION={version}`（`build.rs:43`），后端经 `env!("APP_VERSION")` 引用；`cargo:rerun-if-changed=tauri.conf.json`（`build.rs:41`）保证版本变更触发重编。
3. **发布汇总**：`make-release.ps1` 解析 `.cargo/config.toml` 的 `target-dir`（`make-release.ps1:64`–`72`）定位 NSIS 包，按版本号正则 `_${version}_[A-Za-z0-9]+-setup$`（`make-release.ps1:85`）选中当前版本包（旧包 `staleExes` 告警，`make-release.ps1:90`–`98`），重命名为 `Wxxy-CampusLogin_${version}_${arch}-setup.exe` 并写同名 `.sha256`（`make-release.ps1:100`–`113`）。`-SkipBuild` 仅汇总（`make-release.ps1:20`/`50`）。
4. **缓存清理**：`clean-build-cache.ps1` 解析同一 `target-dir`（`clean-build-cache.ps1:49`–`55`），收集仓内/项目外/gradle/WebView2 缓存（`clean-build-cache.ps1:84`–`157`），`Test-OwnedTargetDir` 用 `.fingerprint` 归属判定避免误删（`clean-build-cache.ps1:61`–`71`），唯一副本 APK/NSIS 红色告警（`clean-build-cache.ps1:169`–`194`），`ShouldProcess` 确认后删除（`clean-build-cache.ps1:199`–`208`）。
5. **安卓出包**：`android/build-apk.ps1` 设 `JAVA_HOME`（`android/build-apk.ps1:18`–`25`）→ 前端 `npm install`/`npx vite build`（`android/build-apk.ps1:27`–`41`）→ 删旧 APK（`android/build-apk.ps1:45`–`47`）→ `tauri android build --target aarch64 --apk`（`android/build-apk.ps1:50`）→ `apksigner verify` 未签名则 `zipalign`+`apksigner sign`（`android/build-apk.ps1:61`–`87`）。与桌面发布完全分开（`android/build-apk.ps1:1`/`make-release.ps1:11`）。
6. **浏览器 mock 链路**：`npx vite --config vite.mock.config.ts`（`vite.mock.config.ts:4`）继承基础配置并把 `/tauri-mock.js` 注入 `index.html` 头（`vite.mock.config.ts:10`–`18`），`tauri-mock.js` 以内存实现顶替 `window.__TAURI_INTERNALS__`（`tauri-mock.js:366`），供无 Tauri 环境的浏览器截图/联调，不进任何产物。



## Connections

- [[tauri-app]] — 桌面运行时后端（Rust），本模块 `Cargo.toml`/`build.rs`/`tauri.conf.json` 即为其编译与打包配置；`APP_VERSION` 注入、无头核心 `campus-login-core` 嵌入均服务于该模块。
- [[tauri-app/frontend]] — React/TS 前端运行时代码；本模块 `vite.config.ts`/`vitest.config.ts`/`tailwind.config.js`/`tsconfig*.json`/`package.json` 为其打包器与依赖配置，`index.html`/`tauri-mock.js` 为其入口与浏览器 mock。
- [[tauri-app/vpn]] — WebVPN 协议核心（`vpn/`），其纯 Rust 实现被无头核心 `campus-login-core` 复用（`Cargo.toml:16`–`18` 注释、`build.rs:48` 占位）。
- 安卓壳工程 `android/`（含 `android/src-tauri/`、`android/plugins/` 下 `foreground-service`/`keystore`/`network-bind` 三个自定义 Rust 插件）由 `android/build-apk.ps1` 单独出包，与本模块桌面脚本互不调用。
- 发布资产最终落在仓内 `release/`（`Wxxy-CampusLogin_*` 与 `.sha256`、`RELEASE_NOTES_v*.md`），桌面中间包落在 `tauri-release/`，应用内更新器按 `<资产URL>.sha256` 校验（见 `build.ps1:186`–`188` 注释）。

## Known Issues

- **cargo `target-dir` 重定向到项目外**（`config.toml:3`–`7`）：本机沙箱给工作区树打 Low 完整性标签，导致 `target\` 下编译出的 build-script exe 以 Low IL 运行无法写 `%TEMP%`（cl 报 D8050、cc-rs 报 os error 5）。所有定位真实产物的脚本（`make-release.ps1:64`–`72`、`clean-build-cache.ps1:48`–`55`）都必须解析 `.cargo/config.toml`，否则会打包/清理到旧的仓内 `target/`（`clean-build-cache.ps1:45`–`47` 注释明确这是 2026-10-26 之前的问题）。
- **`externalBin` 占位与真实产物切换**（`build.rs:4`–`9`、`48`–`61`）：fresh clone 直接 `cargo test`/`check` 会因 `externalBin` 路径不存在失败，故 `build.rs` 落空占位；正式打包由 `tauri.conf.json:9` 的 `beforeBundleCommand` 以真实 `campus-login-core.exe` 覆盖，占位绝不含安装包。
- **Windows 路径过长逃生口**（`build.ps1:108`–`113`）：默认用仓内 `target/`（2026-09-15 起由 `C:\cl-build` 短路径内化），遇路径过长报错用 `-TargetDir 'C:\short\target'`（注释 `build.ps1:109`–`110`）或 `make-release.ps1` 的 `CARGO_TARGET_DIR`。
- **子进程不可接管道**（`build.ps1:116`–`118`、`make-release.ps1:53`）：重定向/管道会让子进程失去 tty，cargo 的 `Building` 进度条与 tauri 的 `Compiling` 行消失，看似卡死；统一直通调用、失败则提示用 `-TargetDir` 逃生。
- **发布版本号交叉校验**（`make-release.ps1:75`–`98`）：bundle 目录会残留上一版 NSIS 包，「取最新一个」会把旧包改名成新版本号发布；脚本按正则匹配当前版本，旧包列 `staleExes` 红色告警，绝不给旧包改名。
- **前端构建不进缓存清理白名单之外的依赖**（`clean-build-cache.ps1:6`–`7`）：脚本只删可重建的编译缓存与产物，**不碰 `node_modules/`**（含其内 `.vite` 缓存）、`release\`、`changelogs\` 与源码，避免误删用户数据或发布资产。
- **WebView2 缓存需单独清**（`clean-build-cache.ps1:129`–`157`）：前端更新后 WebView2 缓存旧资产导致界面仍旧，但只删 `Cache`/`Code Cache`/`GPUCache`/`Dawn*`/`ShaderCache`/`Service Worker` 等可再生子目录，保留 Local/Session Storage 等用户数据。
- **esbuild 防护软件拦截**（`vite.mock.config.ts:22`–`25`）：本机防护软件按文件路径拦截原版 `@esbuild/win32-x64/esbuild.exe` 写盘（改名副本无碍），配置内设 `ESBUILD_BINARY_PATH` 已晚，必须从启动命令带环境变量。
