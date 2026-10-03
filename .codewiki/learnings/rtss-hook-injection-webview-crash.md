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

## 现象

用户实测：WebView 白屏/黑屏数秒后自动恢复（重启守卫工作正常），且**重新安装应用后首次启动必触发**。

## 根因（minidump 锁定）

RivaTuner Statistics Server（RTSS，MSI Afterburner 组件）运行时向 WebView2 子进程注入
`RTSSHooks64.dll`，部分机器上在 `msedgewebview2.exe` 内 0xc0000005 访问违规崩溃
（faulting module=RTSSHooks64.dll，ProcessType=browser，`WV=campus-login.exe`）。
"重装后首次必现"与 WebView2 首次 GPU 初始化和注入竞争吻合。

诊断手段：main.rs 在 WebView2 环境创建之前向 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`
追加 `--enable-crash-reporter --crash-dumps-dir="…\com.campus.login\crashdumps"`
（main.rs:60-67，写 env 在 main.rs:68），让 Crashpad 首次抓到 minidump；
**`EBWebView/Crashpad/watson_metadata`（ASCII 可读）直接给出
ApplicationName/ModuleName/SubCode/WV 宿主名**，比解析 .dmp 便宜得多，优先读它。
注意：即使追加了 `--crash-dumps-dir`，实测转储仍落在 `EBWebView/Crashpad/reports/`，
该参数不生效。

## 排除尝试的完整失败记录（勿重蹈）

按 RTSS 官方 Help（`Help/BUTTON_ADD`：按住 Shift 点 Add = 创建"禁用检测的排除 profile"），
向 `Profiles\msedgewebview2.exe.cfg` 写排除 profile（`[Hooking]` + `EnableHooking\t\t= 0`）：

- 两行精简版 + RTSS 未重启 → 应用 6 个 WebView2 进程中 3 个仍被注入（进程模块枚举实测）
- 全字段完整版（1395 字节，复制自同目录 FlClash.exe.cfg）+ 重启 RTSS → 仍 3/6 注入
- 时序矩阵（profile 先于/后于 RTSS 启动、RTSS 重启后等 25 秒再启动应用）全部无效
- 对照组（其他应用的 32 个已存在 WebView2 进程）恒 0 注入——**只能证明 RTSS 不补注入旧进程，
  不能证明排除生效**；早期一轮"0/6 注入"是检查太早（D3D 设备尚未创建）造成的假阴性

结论：**RTSS 7.3.7 忽略手放入 Profiles 目录的 cfg 文件**（无论格式与时机），可靠的排除只能在
RTSS 界面手动完成；向 Program Files 写文件还需管理员权限，即便生效也不值得。曾实现的
NSIS 安装期写入钩子随方案证伪一并移除（当前全仓 grep `POSTINSTALL` 仅剩
startup.rs:194 一处注释残留，安装器脚本内已无任何 RTSS 写入逻辑）。

## 正确解法（用户操作，一次永久）

启动本应用 → 打开 RTSS 主界面 → 按住 **Shift** 点左下角 Add（为本应用创建排除 profile），
或点 Add 选择 `msedgewebview2.exe` 创建后把 **Application detection level 设为 none**。

## 应用侧现状：只检测不写文件（代码映射，2026-10-03 实读校准）

应用侧保留的是**检测与告知**：装了 RTSS 就在启动日志里留风险说明与手动排除步骤，不写任何文件。

模块 `platform/rtss_compat.rs`（经 platform/mod.rs:34 声明，130 行）：

- `enum RtssOutcome`（rtss_compat.rs:23-28，derive Debug/Clone/PartialEq/Eq）：变体
  `NotInstalled`（:25，所有候选 Profiles 目录都不存在）与 `Installed`（:27，任一存在）。
- `profiles_dir_candidates() -> Vec<PathBuf>`（rtss_compat.rs:31-45）：候选目录经环境变量
  `ProgramFiles(x86)` / `ProgramFiles`（:33）拼 `RivaTuner Statistics Server\Profiles`，不硬编码盘符。
- `detect_installed_in(&[PathBuf]) -> RtssOutcome`（rtss_compat.rs:48-55）：纯逻辑，任一候选
  `is_dir()` 即 `Installed`。
- `detect_rtss() -> RtssOutcome`（rtss_compat.rs:58-60）：运行期入口，组合前两者。
- `static PREINIT_OUTCOME: OnceLock<RtssOutcome>`（rtss_compat.rs:64）：main 入口执行时
  logger 尚未初始化，检测结果先暂存。
- `preinit_detect()`（rtss_compat.rs:68-70）：main 入口调用，只检测不写文件、不阻塞、不 panic。
- `log_preinit_outcome()`（rtss_compat.rs:73-79）：logger 就绪后读 OnceLock 留痕；早期路径
  未执行（异常情况）则此时补检一次（:76）。
- `log_outcome(&RtssOutcome)`（rtss_compat.rs:81-97）：`NotInstalled` → `log_info!("rtss", …)`
  （:84）；`Installed` → `log_warn!("rtss", …)`（:87-94），文案含注入机制、白屏后果、
  "自动写入排除配置已被 RTSS 7.3.7 实测忽略"与 RTSS 界面手动排除步骤。
- 单测 3 个（内嵌 `mod tests`，rtss_compat.rs:99-130，无独立测试文件）：
  `not_installed_when_no_candidate_exists`（:103-108）、
  `installed_when_first_candidate_exists`（:110-119）、
  `candidates_use_program_files_env`（:121-129，断言候选含 "RivaTuner Statistics Server"）。

数据流（检测先于 WebView2 环境创建，留痕晚于 logger 就绪）：

1. `main()`（main.rs:16）→ panic hook（main.rs:20-24）→ `--helper-task` 拦截（main.rs:30-32）
   → helper 提权参数拦截（main.rs:37-46，非正常流程在此退出）。
2. main.rs:51-52（`#[cfg(all(desktop, target_os = "windows"))]`）调用
   `rtss_compat::preinit_detect()`，结果落入 `PREINIT_OUTCOME`——此时 WebView2 环境、
   Tokio runtime 均未创建（注释 main.rs:48-50）。实测放在 setup 里为时已晚：浏览器进程
   已创建，6 个 WebView2 进程中 3 个已被注入（startup.rs:191-192 注释留档）。
3. main.rs:56-68：`gpu::build_browser_args()` 组基础浏览器参数，追加 crash 转储参数
   （目标目录 `data_dir\com.campus.login\crashdumps`，与 app_data_dir 同源，main.rs:61-66），
   整体写入 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`（main.rs:68）。
4. Tauri setup → `setup_app`（startup.rs:152-249）：先初始化 logger（startup.rs:181），
   再于 startup.rs:195-196（同样 cfg 限定 desktop+windows）调用
   `rtss_compat::log_preinit_outcome()` 完成留痕；随后 startup.rs:198-202 补记
   `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 实际值（含 crash 转储参数），构成白屏诊断
   证据链（配合 `webview_recovery` 的运行时版本记录 startup.rs:230 与
   ProcessFailed 订阅 startup.rs:231-232）。

已知过时残留（行为不受影响，读代码时勿被误导）：

- startup.rs:194 注释仍写"安装期由 NSIS POSTINSTALL hook 以管理员身份静默兜底"——该
  NSIS 钩子已随方案证伪移除，当前安装器无此逻辑，RTSS 处理只剩"检测 + 日志告知"。
- platform/mod.rs:32 模块声明处注释仍写"预防(写排除 profile)"，实际早已不写任何文件。

## 教训

1. 第三方注入类崩溃先读 `watson_metadata`，不要直接啃 minidump。
2. "对照组恒 0"只能证明反向命题（不补注入旧进程）；判断"排除是否生效"必须让**实验组的新进程**
   在注入决策之后创建，并给足 D3D 初始化时间，否则得到假阴性。
3. 对第三方软件的"文件协议"做逆向（写它的配置文件）前，先用最小实验验证该文件真的被读取——
   本例写了 5 轮才确认 RTSS 根本不读，早期应先做"重启 RTSS + profile 已存在"这一组决定性实验。
