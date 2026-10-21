---
title: "DNS 重置必须走提权路径，确保设置/撤销能力对等"
type: decision
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
tags: [决策, DNS, 提权, helper, 可逆性]
---

## Overview

本模块定义了 `reset_dns`（重置 DNS 为自动获取）功能的完整实现架构。核心决策在于：**DNS 重置操作必须具备与 DNS 设置（`setup_dns_doh`）完全一致的提权路径**，严禁在非管理员权限下直接内联调用底层 API。

该架构通过 `helper_spawn` 模块调度高权限 Helper 进程，利用“结果文件协议”异步获取执行状态。这种设计确保了无论用户是以普通身份还是管理员身份启动应用，`reset_dns` 都能通过 UAC 或 COM 静默提权机制成功执行，从而满足“设置/撤销能力对等”的产品原则。

## Key Components

### `tauri-app/src-tauri/src/helper/mod.rs`

提权辅助子进程的核心逻辑。统一处理两种执行形态：
1. **命令行模式 (`--helper`)**：主进程经 UAC 提权后启动管理员副本，执行单次操作并写入结果。
2. **计划任务 Worker 模式 (`--helper-task`)**：以 SYSTEM 身份运行的后台代理，轮询请求目录执行任务，无需 UAC。

**关键实现细节**：

*   **路径安全与固定根目录 (审计 A11/P0-2)**：
    *   `proxy_base_dir()` (31-50|)：优先通过 `SHGetKnownFolderPath(FOLDERID_ProgramData)` 获取 ProgramData（避免环境变量劫持），失败回退至 `PROGRAMDATA` 环境变量，再失败硬编码 `C:\ProgramData`。所有路径均经 `validate_programdata_path` 校验 (19-28|)。
    *   `helper_results_dir()` (75-77|) & `helper_requests_dir()` (80-82|)：分别返回 `{ProgramData}/CampusLogin/results` 和 `requests` 子目录。
    *   `resolve_result_path()` (88-100|)：**核心安全函数**。仅接受纯文件名（拒绝 `/`, `\`, `..`, `.`, 长度>128, 非ASCII字母数字/`-_`字符），确保 SYSTEM worker 只能写入固定结果目录，防止任意路径写原语。

*   **数据模型**：
    *   `HelperResult` (102-112|)：序列化结构体，包含 `success`, `message`, `op`, `logs` (空时跳过), `details` (Option<serde_json::Value>)。
    *   `HelperOp` 枚举 (116-156|)：定义所有支持的操作，包括 `Dns`, `ClearDns`, `Mac`, `EnableAdapter`, `DisableAdapter`, `RouteDelete`, `EnableDevice`, `ReenumerateDevice`, `SetMetric`, `RegisterTaskProxy`, `SelfCheck`。注意部分操作受 `#[cfg(target_os = "windows")]` 保护。
    *   `TaskRequest` (314-321|)：计划任务 Worker 使用的 JSON 结构，包含 `op`, `args`, `result`。

*   **参数解析与分发**：
    *   `parse_helper_args()` (242-271|)：解析 `--helper <op> [args...] --result <file>`。提取位置参数和 `--family` 开关，委托给 `build_op_from_args`。
    *   `build_op_from_args()` (161-238|)：通用构造器，将字符串 op 名和参数数组转换为 `HelperOp` 枚举实例。严格校验必填参数（如 mac 需要 guid 和 mac_no_dash）。
    *   `run_helper()` (274-309|)：命令行模式入口。匹配 `HelperOp` 调用具体执行函数，生成 `HelperResult`，并通过 `write_result_file` 原子写入结果。

*   **计划任务 Worker 逻辑**：
    *   `run_helper_task()` (328-351|)：Worker 主循环。扫描 `requests` 目录，按修改时间取最旧请求 (`oldest_request`)。处理完一个请求后空扫两轮（兜住竞态），无请求则退出。
    *   `process_request()` (374-435|)：单请求处理流程。**读入内存后立即删除请求文件** (384|)，防止重复执行或篡改。解析 JSON，构建 Op，执行并写入结果。

*   **具体操作执行函数 (Windows)**：
    *   `run_dns()` (437-456|)：调用 `crate::network::dns_setup::setup_dns_doh_admin`，透传 `details`。
    *   `run_clear_dns()` (461-516|)：逐适配器调用 `clear_adapter_dns_via_api`。汇总成功/失败明细至 `details.restored/failed`。非 Windows 平台直接返回失败。
    *   `run_mac()` (518-578|)：严格校验 MAC 格式 (12位十六进制)。查找适配器，调用 `apply_mac_change_via_registry` 设置伪装 MAC，并立即调用 `remove_mac_from_registry` 清除持久化值（防止重启后残留）。
    *   `run_set_metric()` (620-701|)：夜间出站切换。解码 `"guid:family:automatic:metric"` 条目。使用 Win32 `SetIpInterfaceEntry`，强制 `SitePrefixLength=0` 防错。逐条执行，任一条失败即整体失败。
    *   `run_enable_adapter()` (706-756|) & `run_disable_adapter()` (767-847|)：
        *   **双校验**：适配器名字符集合法 + 必须存在于本机适配器列表 (`get_adapters_force`)。
        *   **禁用策略优化**：`run_disable_adapter` 优先尝试非持久化设备级禁用 (`disable_adapter_nonpersistent`)，避免 netsh disable 导致的跨重启 Problem 22 和 USB 卡驱动安装弹窗。若无可 PnP ID 的虚拟卡则回退 netsh。
    *   `run_route_delete()` (882-944|)：精确删除路由。解码 `"dest|mask|gateway|metric|if_index"`。调用 `delete_route_exact`，确保四元组完全匹配才删除，避免误删 DHCP 默认路由。
    *   `run_enable_device()` (980-1036|)：PnP 设备启用。校验 Instance ID 白名单 (防注入)。调用 `pnputil /enable-device` 后，**轮询复核** `devnode_problem` 直到脱离 Problem 22 状态，不信任 pnputil 返回值。
    *   `run_reenumerate_device()` (1041-1062|)：总线重枚举，恢复幽灵态 USB 设备。
    *   `run_register_task()` (1066-1084|)：注册计划任务代理。

*   **辅助与安全函数**：
    *   `decode_set_metric_row()` (584-614|) & `decode_route_row()` (853-872|)：严格的字段解析与类型校验。
    *   `valid_instance_id()` (950-957|)：设备实例 ID 白名单，拒绝 `/`, 空格, 控制字符, 以 `-` 开头。
    *   `write_result_file()` (1088-1098|)：原子写入（先写 `.tmp` 再 rename）。

*   **测试用例** (1100-1431|)：覆盖了路径校验、参数解析、Op 构造、MAC 校验、set_metric/route 解码、Instance ID 白名单、JSON roundtrip 等场景。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Main Process (Tauri)                         │
│                                                                     │
│  ┌──────────────────────┐      ┌──────────────────────────────┐     │
│  │ network_cmd.rs       │      │ platform/helper_spawn.rs     │     │
│  │                      │      │                              │     │
│  │ reset_dns() [383|]   │─────►│ spawn_elevated_helper() [51|]│     │
│  │                      │      │                              │     │
│  │ 1. Filter Adapters   │      │ Channel 1: Task Proxy        │     │
│  │ 2. Check Admin?      │      │    ├─ proxy_usable()         │     │
│  │    └─ Yes: Direct    │      │    ├─ run_via_task()         │     │
│  │    └─ No: Elevate    │      │    └─ Fail → Invalidate Cache│     │
│  └──────────────────────┘      │                              │     │
│                                │ Channel 2: Raw Spawn         │     │
│                                │    ├─ COM Silent (CMSTPLUA)  │     │
│                                │    ├─ UAC Prompt (ShellExec) │     │
│                                │    └─ Poll Result File       │     │
│                                └──────────────┬───────────────┘     │
│                                       ▲             │               │
│                                       │             ▼               │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              Shared Directory (ProgramData/results/)         │  │
│  │                                                              │  │
│  │  r-<pid>-<ts>.json                                           │  │
│  │  (JSON: { success, message, details... })                    │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                       ▲                             │
│                                       │ Read & Parse                │
│  ┌────────────────────────────────────┴──────────────────────────┐  │
│  │                        Helper Subprocess                       │  │
│  │                                                                │  │
│  │  parse_helper_args() [242|]                                    │  │
│  │       │                                                        │  │
│  │       ▼                                                        │  │
│  │  run_helper(ClearDns) [274|]                                   │  │
│  │       │                                                        │  │
│  │       ▼                                                        │  │
│  │  run_clear_dns() [461|]                                        │  │
│  │       │                                                        │  │
│  │       ▼                                                        │  │
│  │  clear_adapter_dns_via_api()                                   │  │
│  │       │                                                        │  │
│  │       ▼                                                        │  │
│  │  write_result_file() [1088|]                                   │  │
│  └────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **请求发起**：用户在 UI 触发 DNS 重置，前端调用 `network_cmd.rs::reset_dns` (383|)。
2.  **权限判断与预处理**：
    *   命令层校验目标适配器列表，排除黑名单 (398-404|)。
    *   若当前进程已是管理员，直接调用 `dns_config::clear_adapter_dns_via_api` (419|) 并汇总结果。
    *   若非管理员，进入提权流程。
3.  **提权调度 (`helper_spawn`)**：
    *   调用 `spawn_elevated_helper` (51|)，传入操作 `clear_dns` 和目标适配器参数。
    *   **优先级 1：计划任务代理**。尝试通过已注册的 Windows 计划任务以 SYSTEM 身份执行。若失败或不可用，清除缓存并降级。
    *   **优先级 2：直接提权**。
        *   首先尝试 COM 静默提权 (`shell_exec_elevated`) (120-124|)。若用户取消 UAC 或 COM 失败且不允许弹窗，直接报错。
        *   若允许弹窗，调用 `run_elevated` 弹出标准 UAC 窗口 (125|)。
    *   **参数构建**：构建命令行 `--helper clear_dns ... --result "r-<pid>-<ts>.json"` (105-117|)。
4.  **Helper 执行**：
    *   新的 Helper 进程启动，解析参数 `parse_helper_args` (242|)。
    *   识别 `ClearDns` 操作，调用 `run_clear_dns` (461|)。
    *   在 Helper 的管理员上下文中，遍历 GUID 调用 `clear_adapter_dns_via_api` (461|)。
    *   将执行结果（成功/失败明细）序列化为 JSON。
5.  **结果回传**：
    *   Helper 通过 `write_result_file` (1088|) 原子写入结果文件到固定目录。
    *   主进程在 `spawn_elevated_raw` (128-142|) 中轮询读取该文件。
    *   读取成功后，通过 `read_helper_result` (31|) 解析 JSON，清理临时文件，并将结果返回给 `network_cmd`。
6.  **反馈用户**：`network_cmd` 根据解析出的 `success` 和 `message` 向用户展示重置结果 (452-464|)。

## Connections

[[helper-self-restart-elevation]]、[[adapter-operation-scope]]、[[desktop-network-dns]]、[[verification-gate-tiers]]