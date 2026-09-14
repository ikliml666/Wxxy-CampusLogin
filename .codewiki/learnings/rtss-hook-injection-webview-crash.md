---
title: "RTSS hook 注入导致 WebView2 白屏崩溃：根因、诊断与排除尝试的完整失败记录"
type: learning
source_files:
  - tauri-app/src-tauri/src/platform/rtss_compat.rs
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

诊断手段：main.rs 开启的 `--enable-crash-reporter --crash-dumps-dir` 让 Crashpad 首次抓到
minidump；**`EBWebView/Crashpad/watson_metadata`（ASCII 可读）直接给出
ApplicationName/ModuleName/SubCode/WV 宿主名**，比解析 .dmp 便宜得多，优先读它。
注意 tauri 传的 `--crash-dumps-dir` 实际不生效，转储仍落在 `EBWebView/Crashpad/reports/`。

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
NSIS 安装期写入钩子随方案证伪一并移除。

应用侧保留的是：main 入口（WebView2 环境创建之前）检测 RTSS 安装（`platform/rtss_compat.rs`，
结果经 `OnceLock` 暂存、logger 就绪后在 startup 留痕 warn，附手动排除步骤）。检测本身
（Profiles 目录存在性）经 3 个单测覆盖。

## 正确解法（用户操作，一次永久）

启动本应用 → 打开 RTSS 主界面 → 按住 **Shift** 点左下角 Add（为本应用创建排除 profile），
或点 Add 选择 `msedgewebview2.exe` 创建后把 **Application detection level 设为 none**。

## 教训

1. 第三方注入类崩溃先读 `watson_metadata`，不要直接啃 minidump。
2. "对照组恒 0"只能证明反向命题（不补注入旧进程）；判断"排除是否生效"必须让**实验组的新进程**
   在注入决策之后创建，并给足 D3D 初始化时间，否则得到假阴性。
3. 对第三方软件的"文件协议"做逆向（写它的配置文件）前，先用最小实验验证该文件真的被读取——
   本例写了 5 轮才确认 RTSS 根本不读，早期应先做"重启 RTSS + profile 已存在"这一组决定性实验。
