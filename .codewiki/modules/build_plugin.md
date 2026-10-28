---
title: build_plugin
type: module
source_files:
  - android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt
  - android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt
  - android/src-tauri/gen/android/buildSrc/build.gradle.kts
  - android/src-tauri/gen/android/settings.gradle
  - android/src-tauri/gen/android/tauri.settings.gradle
  - android/src-tauri/gen/android/app/build.gradle.kts
  - android/src-tauri/gen/android/app/tauri.build.gradle.kts
  - android/src-tauri/gen/android/build.gradle.kts
  - android/src-tauri/gen/android/gradle.properties
tags: [gradle, rust, android, tauri, build-plugin, buildSrc]
---

## Overview

`build_plugin` 是把 Rust 代码编译成安卓各 ABI 动态库（`.so`）的 Gradle 构建插件。它不是 Rust crate 的 `build.rs`，而是位于 Tauri 生成工程 `android/src-tauri/gen/android/buildSrc/` 里的一套 Kotlin/DSL 代码，随 `android/src-tauri/gen/android` 一起被版本化提交（见 `android/README.md` 第 10 行：Tauri 生成的安卓壳工程版本化提交）。

核心由两文件组成：
- `RustPlugin.kt` —— 注册 Gradle 插件 `rust`，在安卓工程里注入 `abi` 维度 productFlavor，并在 `afterEvaluate` 阶段按「debug/release × 目标架构」矩阵动态创建编译任务，把它们挂到 Android Gradle Plugin（AGP）的 `merge*JniLibFolders` 任务之前。
- `BuildTask.kt` —— 上一步创建的任务的具体实现（`DefaultTask`），`@TaskAction` 里通过 `project.exec` 调用本机 `node.exe` 跑 `@tauri-apps/cli` 的 `android android-studio-script` 子命令，按 `--target` 逐个 ABI 编译 Rust 动态库。

本模块没有独立的单元测试文件（`buildSrc` 目录下仅含 `src/main/java/...`，无 `src/test`）。

## Key Components

### `RustPlugin.kt`（`android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/RustPlugin.kt`）

- `TASK_GROUP`：`const val TASK_GROUP = "rust"`（第 8 行）—— 所有任务归入 `rust` 分组，方便 `./gradlew tasks --group=rust` 查看。
- `open class Config`（第 10–12 行）—— 插件扩展 DSL 配置对象，只有一个字段 `lateinit var rootDirRel: String`（第 11 行），由 `app/build.gradle.kts` 的 `rust { rootDirRel = "../../../" }` 赋值（第 75–77 行）。它代表 `app` 模块相对 Rust 工程根（`tauri-app/src-tauri`）的相对路径，传给 `BuildTask` 作为 `project.exec` 的 `workingDir` 基准。
- `open class RustPlugin : Plugin<Project>`（第 14 行）—— 插件主体，唯一公开方法：

  - `override fun apply(project: Project)`（第 17 行）—— 入口：
    - 第 18 行：`extensions.create("rust", Config::class.java)` 注册扩展，供 `app/build.gradle.kts` 配置。
    - 第 20–26 行：读取 Gradle property `abiList`/`archList`/`targetList`（无则用默认四元组）。默认 `abiList=["arm64-v8a","armeabi-v7a","x86","x86_64"]`、`archList=["arm64","arm","x86","x86_64"]`、`targetList=["aarch64","armv7","i686","x86_64"]`。注意 `archList` 与 `targetList` 仅靠索引对齐：`archList[index]` 决定 flavor 名与 `--target` 的映射载体，`targetList[index]` 才是真正传给 `cargo`/`tauri` 的 Rust target 前缀。
    - 第 28–47 行：`extensions.configure<ApplicationExtension>`（导入 `com.android.build.api.dsl.ApplicationExtension`，第 1 行）注入 `abi` flavor 维度（第 30 行 `@Suppress("UnstableApiUsage")`），并创建 productFlavor：`universal`（包含全部 `abiList`，第 32–37 行）以及 `defaultArchList` 中每个 arch 对应一个 flavor，其 `ndk.abiFilters` 取 `defaultAbiList[index]`（第 38–45 行）。flavor 名与 ndk abi 的对应关系是 `arm64→arm64-v8a`、`arm→armeabi-v7a`、`x86→x86`、`x86_64→x86_64`。
    - 第 49–83 行：`afterEvaluate` 块，在 AGP 已生成 `merge*JniLibFolders` 任务后接线：
      - 第 50 行：遍历 `listOf("debug", "release")`，`profileCapitalized` 大写首字母。
      - 第 52–58 行：`tasks.maybeCreate("rustBuildUniversal$profileCapitalized", DefaultTask::class.java)`，设 `group = TASK_GROUP`、`description`。这是聚合任务，本身无动作（基类 `DefaultTask`），只用来 `dependsOn` 各架构子任务。
      - 第 60 行：`tasks["mergeUniversal${profileCapitalized}JniLibFolders"].dependsOn(buildTask)` —— 把聚合任务挂到 AGP 的 universal 变体 JNI 合并任务之前。
      - 第 62–81 行：遍历 `targetsList.withIndex()`，`targetName=targetList[index]`（Rust target 前缀如 `aarch64`），`targetArch=archList[index]`（如 `arm64`），按 `rustBuild$targetArchCapitalized$profileCapitalized` 名 `maybeCreate` 一个 `BuildTask`（第 66–68 行），并设 `rootDirRel = config.rootDirRel`、`target = targetName`、`release = (profile=="release")`（第 72–74 行）。
      - 第 77 行：`buildTask.dependsOn(targetBuildTask)` —— 聚合任务依赖每个子任务。
      - 第 78–80 行：`tasks["merge$targetArchCapitalized${profileCapitalized}JniLibFolders"].dependsOn(targetBuildTask)` —— 把每个 `BuildTask` 挂到对应 AGP 架构变体的 JNI 合并任务之前，确保 `.so` 在打包前就位。

### `BuildTask.kt`（`android/src-tauri/gen/android/buildSrc/src/main/java/com/campuslogin/app/kotlin/BuildTask.kt`）

- `open class BuildTask : DefaultTask()`（第 9 行）—— 每个「架构 × profile」编译任务的实现类。三个 `@Input` 字段（Gradle 据此判定任务输入，决定增量/UP-TO-DATE）：
  - `@Input var rootDirRel: String? = null`（第 10–11 行）—— 相对 `project.projectDir` 的 Rust 工程根路径。
  - `@Input var target: String? = null`（第 12–13 行）—— Rust target 前缀（`aarch64`/`armv7`/`i686`/`x86_64`）。
  - `@Input var release: Boolean? = null`（第 14–15 行）—— 是否 `--release` 构建。

- `fun assemble()`（第 17 行，`@TaskAction`）—— 任务入口：
  - 第 19 行：硬编码首选可执行文件 `C:\Program Files\nodejs\node.exe`。
  - 第 20–21 行：先尝试 `runTauriCli(executable)`。
  - 第 22–44 行：若抛异常，且 `Os.isFamily(Os.FAMILY_WINDOWS)`（导入 `org.apache.tools.ant.taskdefs.condition.Os`，第 2 行），依次回退尝试 `$executable.exe`/`.cmd`/`.bat`（第 25–29 行）；全失败则抛出最后一个异常（第 40 行）；非 Windows 直接原样抛出（第 42 行）。

- `fun runTauriCli(executable: String)`（第 47 行）—— 真正执行编译：
  - 第 48–50 行：`rootDirRel`/`target`/`release` 任一为 `null` 即 `throw GradleException(...)`。
  - 第 52 行：硬编码 `tauriJs = E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\node_modules\@tauri-apps\cli\tauri.js` —— 直接点本机 `node_modules` 里 `@tauri-apps/cli` 的入口脚本（注释说明本机无全局 tauri CLI，第 51 行）。
  - 第 53 行：基础参数 `listOf(tauriJs, "android", "android-studio-script")`。
  - 第 55–68 行：`project.exec { ... }.assertNormalExitValue()`：
    - `workingDir(File(project.projectDir, rootDirRel))`（第 56 行）—— 切到 Rust 工程根再执行。
    - `executable(executable)`（第 57 行）。
    - `args(args)`（第 58 行）加 `-vv`（DEBUG 级，第 59–60 行）或 `-v`（INFO 级，第 61–62 行）冗长输出。
    - 第 64–66 行：`release==true` 时追加 `--release`。
    - 第 67 行：追加 `--target` 与 `target`（即 Rust target 前缀）。

### `buildSrc/build.gradle.kts`

- 第 2 行：`plugins { kotlin-dsl }` —— 让 `buildSrc` 下的 Kotlin 脚本可依赖 Gradle API 编译为插件。
- 第 5–12 行：`gradlePlugin { plugins { create("pluginsForCoolKids") { id = "rust"; implementationClass = "RustPlugin" } } }` —— 把 `RustPlugin` 注册为插件 id `rust`，即 `app/build.gradle.kts` 第 6 行 `id("rust")` 解析到的实现类。
- 第 21 行：`implementation("com.android.tools.build:gradle:8.11.0")` —— 提供 `ApplicationExtension` 等 AGP DSL 类型。

## Architecture

```
┌──────────────────────────────────────────┐
│ app/build.gradle.kts  (id("rust"))       │
│ rust { rootDirRel = "../../../" }         │
└───────────────────┬──────────────────────┘
                    │ apply(RustPlugin)
                    ▼
┌──────────────────────────────────────────┐
│ RustPlugin.kt : Plugin<Project>           │
│ - 注册扩展 "rust" (Config.rootDirRel)     │
│ - flavorDimension "abi" + flavors         │
│ - afterEvaluate → 动态建任务              │
└───────────────────┬──────────────────────┘
        per profile×per target (maybeCreate)
                    │
        ┌───────────┴────────────┐
        ▼                        ▼
┌────────────────────┐  ┌──────────────────────────┐
│ rustBuildUniversal│  │ rustBuild<Arch><Profile>  │
│ <Profile>         │  │ = BuildTask (DefaultTask) │
│ (聚合, 无动作)    │  │ @TaskAction assemble()    │
└─────────┬─────────┘  └───────────┬──────────────┘
          │ dependsOn              │ dependsOn
          └───────────┬────────────┘
                      ▼
┌──────────────────────────────────────────┐
│ BuildTask.runTauriCli → project.exec(node)│
│ node tauri.js android android-studio-     │
│ script [--release] --target <aarch64..>   │
└───────────────────┬──────────────────────┘
                    │ 产出 *.so
                    ▼
┌──────────────────────────────────────────┐
│ merge<Arch><Profile>JniLibFolders (AGP)   │
│ dependsOn BuildTask → 打包进 APK          │
└──────────────────────────────────────────┘
```

## Data Flow

1. 开发者在 Android Studio / 命令行触发某个变体构建（如 `assembleArm64Release` 或 `assembleUniversalDebug`）。
2. Gradle 配置期：`RustPlugin.apply` 注册 `rust` 扩展、注入 `abi` flavor 维度与 `universal`/`arm64`/`arm`/`x86`/`x86_64` 五个 productFlavor（其 `ndk.abiFilters` 决定最终 APK 打哪些 ABI）。
3. 配置期结束后 `afterEvaluate`：`RustPlugin` 为 `debug`/`release` 各建一个聚合任务 `rustBuildUniversal<Profile>`，并为每个「架构 × profile」建 `rustBuild<Arch><Profile>`（`BuildTask`），把 `config.rootDirRel`、`target`（Rust target 前缀）、`release` 写入任务字段；同时把聚合任务与每个 `BuildTask` 注册为对应 AGP `merge*JniLibFolders` 任务的前置依赖。
4. 执行期：AGP 在执行 `merge*JniLibFolders` 前先运行 `BuildTask.@TaskAction assemble()`。`assemble()` 调 `runTauriCli`，以 `rootDirRel` 为工作目录、用 `node.exe` 执行本机 `@tauri-apps/cli/tauri.js` 的 `android android-studio-script` 子命令，带 `--target <aarch64|armv7|i686|x86_64>`（release 时加 `--release`，debug 时按日志级别加 `-v`/`-vv`）。
5. `tauri` CLI 调用 cargo 把 Rust 后端编译成对应 ABI 的 `lib*.so`，放到 AGP 预期的 JNI 目录。`assertNormalExitValue()`（第 68 行）确保非零退出即失败。
6. AGP 的 `merge*JniLibFolders` 把 `.so` 收拢进对应变体的 APK。

注意 `rootDirRel` 在 `app/build.gradle.kts` 第 76 行被设为 `"../../../"`：`app` 目录是 `android/src-tauri/gen/android/app`，上三层正好回到 `android/src-tauri`，即 `tauri-app/src-tauri` 之外、可经 `../../../` 抵达 `tauri-app`（Rust 工程根，也是 cli 期望的工作目录）。

## Connections

- 本模块由 Tauri 生成工程 `android/src-tauri/gen/android` 承载，与 [[android]] 安卓壳工程、`app/build.gradle.kts` 的 `rust` 扩展配置直接耦合。
- `BuildTask` 实际调用 `@tauri-apps/cli` 的 `android-studio-script` 子命令，是 [[tauri-app]] 桌面/移动双端共享协议核心的安卓构建入口；它与 `tauri-app/src-tauri/build.rs`、各插件 `build.rs`（如 `android/plugins/foreground-service/build.rs`）共同构成协议核心的编译链。
- AGP 任务（`merge*JniLibFolders`）来自 `com.android.tools.build:gradle:8.11.0`（见 `buildSrc/build.gradle.kts` 第 21 行、`build.gradle.kts` 第 7 行），是安卓侧产物整合点。
- `settings.gradle` 第 9–16 行持久 `include` 的手写插件（`tauri-plugin-campus-network-bind`/`keystore`/`monitor-service` 等）是 `BuildTask` 编译产物的下游依赖方；生成工程 `tauri.settings.gradle` 会漏掉它们（见 `settings.gradle` 第 3–8 行注释），须在主 `settings.gradle` 兜底声明。

## Known Issues

- **机器相关硬编码路径（功能性但不可移植）**：`BuildTask.kt` 第 19 行硬编码首选 node 路径 `C:\Program Files\nodejs\node.exe`；第 52 行硬编码 tauri CLI 入口 `E:\ik\Documents\trae_projects\1\Wxxy-CampusLogin\tauri-app\node_modules\@tauri-apps\cli\tauri.js`。这两处都是作者本机绝对路径，在另一台机器 / CI 上若路径不同则会失败（非 Windows 还会跳过 `.exe/.cmd/.bat` 回退直接抛原始异常）。这是该模块最大的可移植性风险，迁移环境必须改这两行。
- **`archList` 与 `targetList` 仅按索引对齐**：`RustPlugin.kt` 第 62–64 行用 `targetsList.withIndex()` 取 `targetName=targetList[index]`、`targetArch=archList[index]`。默认两者顺序一致；若通过 Gradle property 只覆盖 `archList` 而没同步 `targetList`（或反之），索引错位会导致「flavor 名/AGP merge 任务名」与「真正 `--target`」不匹配，编译或打包错 ABI。
- **`buildSrc` 是生成物但被手写修改**：整个 `buildSrc/` 属于 Tauri `gen/android` 自动生成工程，本应不手改；但 `BuildTask.kt` 的硬编码路径与 `settings.gradle` 第 3–8 行注释表明此处已被定制。下次 Tauri CLI 重生成 `gen/android` 可能覆盖 `buildSrc` 与 `app/build.gradle.kts` 中的 `rust` 配置，需重新校对（见 `settings.gradle` 第 8 行「升级插件或 tauri 版本时需同步校对下方路径」）。
- **`universal` 聚合任务无实际 action**：`rustBuildUniversal<Profile>` 是空 `DefaultTask`（第 52–54 行），仅作 `dependsOn` 容器，`.so` 的真正产出来自各 `rustBuild<Arch><Profile>`；若误以为 universal 任务本身会编译全部目标而单独 `run`，它什么都不做。
