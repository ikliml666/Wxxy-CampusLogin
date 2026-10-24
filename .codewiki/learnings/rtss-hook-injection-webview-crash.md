---
title: "RTSS hook 注入导致 WebView2 白屏崩溃：根因、诊断与排除尝试的完整失败记录"
type: learning
source_files:
  - tauri-app/src-tauri/src/platform/rtss_compat.rs
  - tauri-app/src-tauri/src/platform/mod.rs
  - tauri-app/src-tauri/src/main.rs
  - tauri-app/src-tauri/src/app/startup.rs
tags:
  - webview2
  - rtss
  - crash-diagnosis
  - third-party-conflict
---

## Overview

本文档记录了 RivaTuner Statistics Server (RTSS，MSI Afterburner 组件) 向 WebView2 子进程注入 `RTSSHooks64.dll` 导致应用界面白屏崩溃的根因分析、诊断过程，以及自动写入排除配置失败的完整实验记录与当前代码侧的应对策略。

**现象与根因**：RTSS 运行时向 WebView2 子进程 (`msedgewebview2.exe`) 注入 `RTSSHooks64.dll`，部分机器上触发 `msedgewebview2.exe` 内部 `0xc0000005` 访问违规崩溃，导致 WebView 白屏后自动重载（重启守卫恢复，但“重装后首次启动”每次必现）（见 `rtss_compat.rs:3-5`）。

**排除尝试失败**：曾向 RTSS 的 Profiles 目录写入按被注入进程名命名的排除 profile（两行精简版与全字段完整版各测一轮，且覆盖“RTSS 重启前后”两种时序），本应用 6 个 WebView2 进程中始终有 3 个仍被注入；对照组（已存在的其他 WebView2 进程）恒为 0 注入，只能证明 RTSS 不补注入旧进程，不能证明 profile 生效。结论：RTSS 7.3.7 不会应用手放入 Profiles 目录的 cfg 文件（见 `rtss_compat.rs:7-13`）。

**当前策略**：可靠的排除只能在 RTSS 界面手动完成（官方 Help：Add 按钮按住 Shift 可为当前活动 3D 应用创建“禁用检测的排除 profile”，或创建 profile 后把 Application detection level 设为 none）。因此应用侧只做**检测与告知**：装了 RTSS 就在启动日志里留下风险说明与手动排除步骤，不写任何文件（见 `rtss_compat.rs:15-16`）。

## Key Components

- **`enum RtssOutcome`** (`rtss_compat.rs:23-28`)
  - 派生属性：`Debug, Clone, PartialEq, Eq` (`:22`)
  - 变体 `NotInstalled` (`:25`)：所有候选 Profiles 目录都不存在。
  - 变体 `Installed` (`:27`)：任一候选 Profiles 目录存在——存在 hook 注入风险，需提示用户。

- **`profiles_dir_candidates() -> Vec<PathBuf>`** (`rtss_compat.rs:31-45`)
  - 构建候选目录列表。遍历环境变量 `ProgramFiles(x86)` / `ProgramFiles` (`:33`)，若非空则拼接 `RivaTuner Statistics Server\Profiles` (`:36-40`)。不硬编码盘符。

- **`detect_installed_in(&[PathBuf]) -> RtssOutcome`** (`rtss_compat.rs:48-55`)
  - 纯逻辑检测。遍历候选目录，若任一 `dir.is_dir()` 返回 true 则返回 `Installed` (`:50-51`)，否则返回 `NotInstalled` (`:54`)。

- **`detect_rtss() -> RtssOutcome`** (`rtss_compat.rs:58-60`)
  - 运行期入口，组合 `profiles_dir_candidates()` 与 `detect_installed_in()`。

- **`static PREINIT_OUTCOME: OnceLock<RtssOutcome>`** (`rtss_compat.rs:64`)
  - 早期检测结果暂存量。main 入口执行时日志系统尚未初始化，先存下来，待 startup 里 logger 就绪后由 `log_preinit_outcome` 留痕 (`:62-64`)。

- **`preinit_detect()`** (`rtss_compat.rs:68-70`)
  - 在 main 入口（WebView2 环境创建之前、helper 参数拦截之后）调用。只做检测不写文件；不阻塞、不 panic (`:66-69`)。

- **`log_preinit_outcome()`** (`rtss_compat.rs:73-79`)
  - logger 就绪后调用：把早期结果留痕；若早期路径未执行（异常情况）则此时补检一次 (`:74-77`)。

- **`log_outcome(outcome: &RtssOutcome)`** (`rtss_compat.rs:81-97`)
  - `NotInstalled` → `log_info!("rtss", "RTSS 未安装，无 hook 注入风险")` (`:83-85`)。
  - `Installed` → `log_warn!("rtss", ...)` (`:86-95`)，文案包含注入机制、白屏后果、“自动写入排除配置已被 RTSS 7.3.7 实测忽略”以及 RTSS 界面手动排除步骤（Shift+Add 或 detection level 设为 none）。

- **单元测试 `mod tests`** (`rtss_compat.rs:99-130`)
  - `not_installed_when_no_candidate_exists` (`:103-108`)：验证候选目录不存在时返回 `NotInstalled`。
  - `installed_when_first_candidate_exists` (`:110-119`)：验证候选目录列表中任一存在时返回 `Installed`。
  - `candidates_use_program_files_env` (`:121-129`)：断言候选目录不为空且包含 "RivaTuner Statistics Server"。

- **模块声明** (`mod.rs:38-40`)
  - `#[cfg(all(desktop, target_os = "windows"))] pub mod rtss_compat;`，其上方注释写“RTSS(MSI Afterburner) hook 注入致 WebView 白屏崩溃的预防(写排除 profile)”(`:38`)，但实际逻辑早已不写任何文件（见 `rtss_compat.rs:15-16` 模块注释）。

- **启动极早期调用** (`main.rs:48-52`)
  - 在 WebView2 环境创建之前执行，受 `#[cfg(all(desktop, target_os = "windows"))]` 保护，调用 `crate::platform::rtss_compat::preinit_detect()`。

- **启动留痕期调用** (`startup.rs:194-199`)
  - 在 `setup_app` 中、logger 初始化之后调用 `crate::platform::rtss_compat::log_preinit_outcome()`，受相同 cfg 保护。注释提及 NSIS POSTINSTALL hook 兜底 (`:197`)，但实为过时残留。

## Architecture

```text
┌───────────────────────────────────────┐
│ main.rs (进程入口)                      │
│ preinit_detect() (main.rs:48-52)       │
│   └─► rtss_compat::detect_rtss()       │
│         (rtss_compat.rs:58-60)         │
│   └─► PREINIT_OUTCOME.set()            │
│         (rtss_compat.rs:64,68-70)      │
└───────────────────────────────────────┘
                  │
                  │ (Tauri Builder / setup_app 执行)
                  ▼
┌───────────────────────────────────────┐
│ startup.rs (setup_app)                 │
│ init_logger() (startup.rs:184)         │
│ log_preinit_outcome() (startup.rs:198) │
│   └─► rtss_compat::log_outcome()       │
│         (rtss_compat.rs:73-79,81-97)   │
│   └─► 日志输出风险告知 / 无风险         │
└───────────────────────────────────────┘
```

## Data Flow

1. **进程极早期检测**：`main.rs:48-52` 在 `--helper-task` 及 helper 提权参数拦截后、WebView2 环境创建前，调用 `rtss_compat::preinit_detect()`。该函数内部执行 `detect_rtss()`（遍历 `profiles_dir_candidates()` 检查目录是否存在），将 `RtssOutcome` 存入 `PREINIT_OUTCOME`（类型为 `OnceLock`，见 `rtss_compat.rs:64`）。
2. **浏览器参数设置**：`main.rs:68-80` 调用 `gpu::build_browser_args()`（行56）并设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`（行68，含 Crashpad 转储目录 `data_dir/com.campus.login/crashdumps`），此时不产生 RTSS 相关日志。
3. **Logger 就绪后留痕**：Tauri `setup_app` 在 `startup.rs:184` 初始化 logger 后，于 `startup.rs:198-199` 调用 `rtss_compat::log_preinit_outcome()`。该函数读取 `PREINIT_OUTCOME`，若为空则补检 (`rtss_compat.rs:76`)，最终调用 `log_outcome` 打印 `log_info!` 或 `log_warn!` 日志（`rtss_compat.rs:81-97`）。
4. **诊断补充**：`startup.rs:204-208` 补记 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 实际值（含 crash 转储参数），配合 `webview_recovery::record_webview2_runtime_version()` (`startup.rs:204`) 与 `subscribe_process_failed()` (`startup.rs:204-205`) 构成崩溃诊断证据链。

## Connections

- [[webview-recovery]]：WebView2 崩溃恢复与诊断模块，`startup.rs:246-248` 记录运行时版本并订阅 ProcessFailed 事件，与 RTSS 导致的白屏崩溃自动恢复直接相关。
- [[gpu-config]]：GPU 检测与浏览器参数构建，`main.rs:56` 调用 `gpu::build_browser_args()` 设置 WebView2 初始化参数，与 WebView2 环境创建紧密相关。
- [[app-startup]]：应用启动流程，`startup.rs` 包含 logger 初始化、状态设置及后台任务启动，RTSS 检测留痕严格依赖其 logger 就绪时机。

## Known Issues

1. **过时残留注释（行为不受影响，读代码时勿被误导）**：
    - `startup.rs:194`（及 197 行）注释仍写“安装期由 NSIS POSTINSTALL hook 以管理员身份静默兜底”——该 NSIS 钩子已随方案证伪移除，当前安装器无此逻辑，RTSS 处理只剩“检测 + 日志告知”（见 `rtss_compat.rs:7-16`）。
    - `platform/mod.rs:38` 模块声明处注释仍写“预防(写排除 profile)”，实际代码早已不写任何文件，仅做检测与告知（见 `rtss_compat.rs:15-16`）。

2. **RTSS 配置读取行为限制**：RTSS 7.3.7 不会读取手动放入 Profiles 目录的 `.cfg` 文件，可靠排除必须在 RTSS 界面手动操作（按住 Shift 点 Add 或设 Application detection level 为 none），详见 `rtss_compat.rs:7-13`。

3. **WebView2 Crashpad 转储路径不确定性**：尽管 `main.rs:60-68` 追加了 `--enable-crash-reporter --crash-dumps-dir=...`，实测某些情况下转储可能不落于指定目录；该功能仅作诊断辅助，崩溃恢复依赖 `webview_recovery::subscribe_process_failed` (`startup.rs:247-248`) 事件驱动，而非依赖 dump 文件分析。
