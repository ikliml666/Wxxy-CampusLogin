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

本文档记录了 RivaTuner Statistics Server (RTSS，MSI Afterburner 组件) 向 WebView2 子进程注入 `RTSSHooks64.dll` 导致应用界面白屏崩溃的根因分析、诊断过程，以及自动写入排除配置失败的完整实验记录。

核心结论：**RTSS 7.3.7 忽略手放入 Profiles 目录的 `.cfg` 配置文件**。可靠的排除只能在 RTSS 界面手动完成。因此，应用侧策略调整为**仅检测与告知**：在启动日志中留下风险说明与手动排除步骤，不写任何文件。

## 现象

用户实测现象如下：
1. **白屏/黑屏后自动恢复**：WebView 出现白屏或黑屏数秒后自动恢复（重启守卫工作正常）。
2. **重装后首次必现**：重新安装应用后，首次启动必定触发该问题。
3. **部分进程受影响**：应用运行时产生多个 WebView2 进程，其中约半数被注入 DLL。

## 根因（minidump 锁定）

### 注入机制
RivaTuner Statistics Server (RTSS) 运行时向 WebView2 子进程 (`msedgewebview2.exe`) 注入 `RTSSHooks64.dll`。在部分机器上，该注入会导致 `msedgewebview2.exe` 内部发生 `0xc0000005` 访问违规崩溃（faulting module=RTSSHooks64.dll，ProcessType=browser，`WV=campus-login.exe`）。

"重装后首次必现"与 WebView2 首次 GPU 初始化和注入竞争吻合。

### 诊断手段
应用通过向 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 追加参数启用 Crashpad 转储：
- 参数：`--enable-crash-reporter --crash-dumps-dir="…\com.campus.login\crashdumps"`
- 目标目录：`data_dir\com.campus.login\crashdumps`（与 app_data_dir 同源，由 `main.rs` 设置）
- 诊断优势：`EBWebView/Crashpad/watson_metadata`（ASCII 可读文件）直接给出 ApplicationName/ModuleName/SubCode/WV 宿主名，比解析二进制 .dmp 文件便宜得多，作为优先读取源。
- 注意：即使追加了 `--crash-dumps-dir`，实测转储仍可能落在 `EBWebView/Crashpad/reports/`，该参数在某些情况下不生效。

## 排除尝试的完整失败记录（勿重蹈）

早期曾尝试通过代码向 RTSS 的 Profiles 目录写入排除 profile（`[Hooking]` + `EnableHooking\t\t= 0`），但所有实验均失败。

### 实验矩阵与结果

| 实验条件 | 结果 | 备注 |
| :--- | :--- | :--- |
| 两行精简版 profile + RTSS 未重启 | 6 个 WebView2 进程中 3 个仍被注入 | 进程模块枚举实测 |
| 全字段完整版 (1395 字节) + 重启 RTSS | 6 个 WebView2 进程中 3 个仍被注入 | 复制自同目录 FlClash.exe.cfg |
| 时序变化 (profile 先于/后于 RTSS 启动) | 全部无效 | 覆盖 RTSS 重启后等 25 秒再启动应用 |
| 对照组 (其他应用的 32 个已存在进程) | 恒为 0 注入 | **只能证明 RTSS 不补注入旧进程，不能证明排除生效** |
| 早期一轮 "0/6 注入" | 假阴性 | 检查太早（D3D 设备尚未创建）造成 |

### 结论
**RTSS 7.3.7 忽略手放入 Profiles 目录的 cfg 文件**（无论格式与时机）。可靠的排除只能在 RTSS 界面手动完成。向 Program Files 写文件还需管理员权限，即便生效也不值得。

曾实现的 NSIS 安装期写入钩子随方案证伪一并移除（当前全仓 grep `POSTINSTALL` 仅剩 `startup.rs:194` 一处注释残留，安装器脚本内已无任何 RTSS 写入逻辑）。

## 正确解法（用户操作，一次永久）

用户需执行以下手动操作以永久解决：
1. 启动本应用。
2. 打开 RTSS 主界面。
3. **方法 A**：按住 **Shift** 键点左下角 Add（为本应用创建排除 profile）。
4. **方法 B**：点 Add 选择 `msedgewebview2.exe` 创建 profile 后，把 **Application detection level 设为 none**。

## 应用侧现状：只检测不写文件（代码映射，2026-10-03 实读校准）

应用侧保留的是**检测与告知**：装了 RTSS 就在启动日志里留风险说明与手动排除步骤，不写任何文件。

### 模块结构 (`platform/rtss_compat.rs`)

模块 `platform/rtss_compat.rs`（经 `platform/mod.rs:32` 声明，受 `#[cfg(all(desktop, target_os = "windows"))]` 保护）：

#### 公开函数与类型

- **`enum RtssOutcome`** (`rtss_compat.rs:23-28`, derive Debug/Clone/PartialEq/Eq)
    - `NotInstalled` (`:25`)：所有候选 Profiles 目录都不存在。
    - `Installed` (`:27`)：任一候选目录存在——存在 hook 注入风险，需提示用户。
- **`profiles_dir_candidates() -> Vec<PathBuf>`** (`rtss_compat.rs:31-45`)
    - 构建候选目录列表。遍历环境变量 `ProgramFiles(x86)` / `ProgramFiles` (`:33`)，拼接 `RivaTuner Statistics Server\Profiles`。不硬编码盘符。
- **`detect_installed_in(&[PathBuf]) -> RtssOutcome`** (`rtss_compat.rs:48-55`)
    - 纯逻辑检测。遍历候选目录，若任一 `is_dir()` 返回 true，则返回 `Installed`。否则返回 `NotInstalled`。
- **`detect_rtss() -> RtssOutcome`** (`rtss_compat.rs:58-60`)
    - 运行期入口，组合前两者。
- **`static PREINIT_OUTCOME: OnceLock<RtssOutcome>`** (`rtss_compat.rs:64`)
    - 早期检测结果暂存量。`main` 入口执行时日志系统尚未初始化，先存下来，待 `startup` 里 logger 就绪后由 `log_preinit_outcome` 留痕。
- **`preinit_detect()`** (`rtss_compat.rs:68-70`)
    - 在 `main` 入口（WebView2 环境创建之前、helper 参数拦截之后）调用。只做检测不写文件；不阻塞、不 panic。
- **`log_preinit_outcome()`** (`rtss_compat.rs:73-79`)
    - logger 就绪后调用：把早期结果留痕；若早期路径未执行（异常情况）则此时补检一次 (`:76`)。
- **`log_outcome(&RtssOutcome)`** (`rtss_compat.rs:81-97`)
    - `NotInstalled` → `log_info!("rtss", "RTSS 未安装，无 hook 注入风险")` (`:84`)。
    - `Installed` → `log_warn!("rtss", ...)` (`:87-94`)，文案含注入机制、白屏后果、"自动写入排除配置已被 RTSS 7.3.7 实测忽略"与 RTSS 界面手动排除步骤。

#### 单元测试 (`mod tests`, `rtss_compat.rs:99-130`)

- **`not_installed_when_no_candidate_exists`** (`:103-108`)：验证当候选目录不存在时返回 `NotInstalled`。
- **`installed_when_first_candidate_exists`** (`:110-119`)：验证当候选目录列表中任一存在时返回 `Installed`。
- **`candidates_use_program_files_env`** (`:121-129`)：断言候选目录不为空且包含 "RivaTuner Statistics Server"。

### 数据流（检测先于 WebView2 环境创建，留痕晚于 logger 就绪）

```text
┌─────────────────────────────────────────────────────────────────────────────────────┐
│ 1. main() [main.rs:16]                                                             │
│    ├─ panic hook (main.rs:20-24)                                                   │
│    ├─ --helper-task 拦截 (main.rs:30-32)                                           │
│    ├─ helper 提权参数拦截 (main.rs:37-46)                                          │
│    └─ [Windows Desktop] preinit_detect() (main.rs:51-52)                           │
│       └─► rtss_compat::PREINIT_OUTCOME.set(detect_rtss())                          │
│           └─► profiles_dir_candidates() + detect_installed_in()                    │
│                                                                                     │
│ 2. WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS 设置 (main.rs:56-68)                       │
│    └─ gpu::build_browser_args() + crash-dumps-dir                                  │
│                                                                                     │
│ 3. Tauri setup -> setup_app() [startup.rs:152-249]                                 │
│    ├─ logger 初始化 (startup.rs:181)                                               │
│    ├─ [Windows Desktop] log_preinit_outcome() (startup.rs:195-196)                 │
│    │   └─► 从 PREINIT_OUTCOME 读取或补检，调用 log_outcome()                        │
│    ├─ 记录 WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS 实际值 (startup.rs:198-202)        │
│    ├─ webview_recovery::record_webview2_runtime_version() (startup.rs:230)         │
│    └─ webview_recovery::subscribe_process_failed() (startup.rs:231-232)            │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

### 关键代码位置引用

- **检测调用**：`main.rs:51-52` (`#[cfg(all(desktop, target_os = "windows"))]`) 调用 `rtss_compat::preinit_detect()`，结果落入 `PREINIT_OUTCOME`。此时 WebView2 环境、Tokio runtime 均未创建。
- **留痕调用**：`startup.rs:195-196` (`#[cfg(all(desktop, target_os = "windows"))]`) 调用 `rtss_compat::log_preinit_outcome()` 完成留痕。
- **Crashpad 参数**：`main.rs:56-68` 组基础浏览器参数，追加 crash 转储参数（目标目录 `data_dir\com.campus.login\crashdumps`，与 app_data_dir 同源，`main.rs:61-66`），整体写入 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (`main.rs:68`)。
- **诊断证据链**：`startup.rs:198-202` 补记 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 实际值（含 crash 转储参数），配合 `webview_recovery` 的运行时版本记录 (`startup.rs:230`) 与 ProcessFailed 订阅 (`startup.rs:231-232`)。

## Connections

- [[webview-recovery]]：WebView2 崩溃恢复与诊断模块，记录运行时版本并订阅 ProcessFailed 事件。
- [[gpu-config]]：GPU 检测与浏览器参数构建，与 WebView2 初始化紧密相关。
- [[app-startup]]：应用启动流程，包含 logger 初始化、状态设置及后台任务启动。

## Known Issues

1. **过时残留（行为不受影响，读代码时勿被误导）**：
    - `startup.rs:194` 注释仍写“安装期由 NSIS POSTINSTALL hook 以管理员身份静默兜底”——该 NSIS 钩子已随方案证伪移除，当前安装器无此逻辑，RTSS 处理只剩“检测 + 日志告知”。
    - `platform/mod.rs:32` 模块声明处注释仍写“预防(写排除 profile)”，实际早已不写任何文件。

2. **RTSS 配置读取行为**：RTSS 7.3.7 不会读取手动放入 Profiles 目录的 `.cfg` 文件，必须通过界面操作。