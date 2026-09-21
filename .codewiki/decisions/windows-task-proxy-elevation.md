---
title: Windows 提权通道定稿——计划任务代理为首选层，CMSTPLUA 降为兜底
type: decision
source_files:
  - tauri-app/src-tauri/src/platform/task_proxy.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/elevation.rs
  - tauri-app/src-tauri/windows/hooks.nsh
tags: [决策, 提权, 计划任务, 安全, Windows, 架构]
---

## 背景

应用需要提权执行：启用被禁用网卡（netsh / 设备级 pnputil）、DNS+DoH、MAC 修改。
主进程通常非管理员。历史通道 CMSTPLUA 静默提权存在两个叠加 bug（BIND_OPTS 传小、
ICMLuaUtil vtable slot 错位，见 [[cmstplua-elevation-bind-opts3-and-vtable-slot]]），
修复后可用，但它与恶意软件 TTP 同源、微软在成批清理 auto-elevate 通道（UACMe
方法表单调收缩），Administrator Protection（AP）启用后所有 auto-elevation 均需交互。
调研结论：Tailscale/OpenVPN/Firefox 等同类应用做机器级提权操作清一色用
「高权服务/计划任务 + IPC」，无一依赖 UAC bypass。

## 决策

提权通道四级优先链：

1. **管理员直跑**（已是管理员时进程内直接执行）；
2. **计划任务代理**（首选提权通道，零 UAC）：SYSTEM 主体 + RunLevel=Highest 哑任务
   `CampusLoginPowerOps`，action 固定 `自身exe --helper-task`；主进程写请求文件
   （`%ProgramData%\CampusLogin\requests\req-*.json`，唯一名+同目录 tmp rename 原子落盘）
   → `schtasks /run`（普通权限）→ 轮询 `results\r-*.json`。worker 复用 helper op 分发。
3. **CMSTPLUA 静默提权**（兜底，仅当代理不可用）；
4. **ShellExecuteW runas**（弹 UAC 终底；自动启用首试禁用此层，退避重试后再启用）。

### 注册与安全细节（glm5-3 评审实测定稿）

- 注册必须走 **COM `ITaskFolder::RegisterTaskDefinition` + 显式 SDDL**
  `D:P(A;;GRGX;;;BU)(A;;FA;;;BA)(A;;FA;;;SY)`：`schtasks /create` 的默认 DACL 不含
  Users ACE，普通用户 `/run` 被拒（实测）；显式 SDDL 下 `/run` 可行且 `/change`、
  `/delete` 被拒（实测）——「可触发不可篡改」形态。注册动作经现有提权链在
  提权副本内执行（`--helper register_task`）。
- **零触发器**（免 `/sc ONCE /st` 过期语义），电源条件显式关闭，
  `ExecutionTimeLimit=PT5M`，`MultipleInstances=IgnoreNew`。
- **结果路径收口**：高权限 worker/副本只写固定目录 `results\` 下纯文件名
  （拒绝路径分隔符/`..`/点开头），同时回移到 `--helper` 旧通道——
  否则构成「SYSTEM 任意路径写」原语（可覆盖 hosts/系统 DLL）。
- **路径漂移防护**：每次使用前校验任务 action == 当前 exe 且参数为 `--helper-task`，
  不匹配自动重注册覆盖（应用升级/换目录场景）。
- **退避**：注册类失败 10 分钟内不再尝试（防 UAC 弹窗风暴）；触发类失败清缓存重检。
- worker 内新 op 双校验：`enable_adapter` 过 validate_adapter_name（已补 `/`）+
  本机适配器存在性；`enable_device` 过设备实例 ID 字符集白名单（防 pnputil 开关注入，
  如 `/remove-device`）+ `CM_Locate_DevNodeW` 存在性复核。
- 卸载清理：NSIS `installerHooks`（`windows/hooks.nsh`）删任务 + 清数据目录。

### 已声明的风险边界

未签名 exe + 用户可写安装目录 + 持久 SYSTEM 任务：本机恶意软件改写 exe 后可无提示
获得 SYSTEM 执行（暴露面较旧通道扩大为「任意时刻可触发」）。白名单 op 滥用增益有限
（用户手动亦可执行），根本缓解是安装到管理员可写目录——作为长期选项。

## 备选方案

- 常驻 LocalSystem 服务 + 命名管道（Tailscale 模式）：可持续性最高，工程量最大
  （安装/升级/看门狗），留作后续演进。
- 换 CLSID / 进程路径伪装（UACMe 式）：恶意软件指纹，否决。
- 引导用户关 UAC / `requireAdministrator` manifest：否决。

## 影响

DNS/MAC/启用网卡全部经统一 helper 框架；安卓端不编译该通道（cfg 门控，
`cargo check --target aarch64-linux-android` 与一键出包验证通过）。
`pnputil` 需 Win10 2004+；`RegisterTaskDefinition` 需管理员（仅注册时一次）。

## Connections

[[cmstplua-elevation-bind-opts3-and-vtable-slot]]、[[usb-adapter-enable-pnp-vs-ndis-layers]]
