---
title: "DNS 重置必须走提权路径，确保设置/撤销能力对等"
type: decision
source_files:
  - tauri-app/src-tauri/src/commands/network_cmd.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
tags: [决策, DNS, 提权, helper, 可逆性]
---

## Overview

本模块定义了 `reset_dns`（重置 DNS 为自动获取）功能的完整实现架构。核心决策在于：**DNS 重置操作必须具备与 DNS 设置（`setup_dns_doh`）完全一致的提权路径**，严禁在非管理员权限下直接内联调用底层 API。

该架构通过 `helper_spawn` 模块调度高权限 Helper 进程，利用“结果文件协议”异步获取执行状态。这种设计确保了无论用户是以普通身份还是管理员身份启动应用，`reset_dns` 都能通过 UAC 或 COM 静默提权机制成功执行，从而满足“设置/撤销能力对等”的产品原则。

## Key Components

### `tauri-app/src-tauri/src/commands/network_cmd.rs`

负责暴露 Tauri 前端命令，协调网络配置逻辑。

*   **`reset_dns(app_handle)`** [382|]：DNS 重置入口命令。
    *   **平台限制**：非 Windows 直接返回错误 [386-390|]。
    *   **目标筛选**：基于配置 `dns_optimize_adapters` 过滤存在且未黑名单的适配器 [398-404|]。
    *   **提权分支**：
        *   **管理员路径**：遍历 GUID 直接调用 `clear_adapter_dns_via_api` [419|]，汇总 restored/failed [416-437|]。
        *   **非管理员路径**：通过 `spawn_elevated_helper` 拉起 `--helper clear_dns` [444|]。解析 helper 返回的 success/message [452-464|]。

### `tauri-app/src-tauri/src/platform/helper_spawn.rs`

主进程侧的提权操作调度器，负责启动高权限 Helper（SYSTEM 或管理员身份），并通过固定目录下的结果文件协议异步获取执行结果。实现了三级降级通道以平衡用户体验（无 UAC）与兼容性。

*   **`new_result_name() -> String`** [21|]：生成唯一结果文件名，格式为 `r-<pid>-<ts>.json`，基于当前进程 ID 和纳秒级时间戳。确保 Worker/提权副本写入的文件名唯一且路径受控。
*   **`read_helper_result(content: &str, result_path: &Path) -> Result<serde_json::Value, String>`** [31|]：解析 Helper 返回的 JSON。若包含 `logs` 数组，则逐行通过 `crate::log_info!` 输出诊断日志；删除结果文件以保持目录整洁；返回解析后的 JSON Value。
*   **`spawn_elevated_helper(...)`** [51|]：**入口函数**。接收操作名 `op`、参数 `args`、结果文件名 `result_name`、超时 `timeout` 及是否允许弹 UAC 标志 `allow_uac_prompt`。
    *   **通道 1：计划任务代理** [60-89|]。优先调用 `task_proxy::proxy_usable()` 检查可用性。若可用，尝试 `run_via_task`。失败则记录警告并调用 `invalidate_proxy_cache()` 清除缓存以便下次重新检测。
    *   **注册重试逻辑** [70-89|]。若代理未注册但状态非 `Disabled`，尝试调用 `ensure_registered()` 进行注册（内部可能触发一次提权）。注册成功后再次尝试 `run_via_task`。无论注册成功与否导致代理失败，均记录警告并进入降级链。
    *   **降级链** [91|]：若代理通道不可用或失败，调用 `spawn_elevated_raw`。
*   **`spawn_elevated_raw(...)`** [98|]：**底层提权执行器**。绕过计划任务代理，直接执行 COM 静默提权或 ShellExecute runas。
    *   **参数构建** [105-117|]：获取当前 exe 路径。构建命令行参数 `--helper {op}`，对每个 `args` 元素添加双引号以防止空格拆分（如 "以太网 2"）。追加 `--result "{result_name}"`。注释强调路径由 helper 侧 `resolve_result_path` 收口至固定目录，防止任意路径写入。
    *   **COM 静默提权** [120-124|]：调用 `crate::platform::elevation::shell_exec_elevated`。若失败且 `allow_uac_prompt=false`（首试自动模式），直接返回 Err，禁止弹窗，交由调用方退避重试。
    *   **UAC 兜底提权** [125|]：若 COM 失败且允许弹窗，调用 `crate::platform::elevation::run_elevated` 弹出标准 UAC 窗口。
    *   **结果轮询** [128-142|]：计算结果文件绝对路径 `resolve_result_path`。在 `deadline` 前每 100ms 轮询读取文件。若读到内容，解析并返回。
    *   **超时后检查** [137-141|]：即使超时，仍尝试最后一次读取。这是为了处理 UAC 等待或 DHCP 慢路径导致的延迟，避免误报超时失败。若最终未找到文件，返回超时错误。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        Main Process (Tauri)                         │
│                                                                     │
│  ┌──────────────────────┐      ┌──────────────────────────────┐     │
│  │ network_cmd.rs       │      │ platform/helper_spawn.rs     │     │
│  │                      │      │                              │     │
│  │ reset_dns() [382|]   │─────►│ spawn_elevated_helper() [51|]│     │
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
│  │  write_result_file() [1083|]                                   │  │
│  └────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **请求发起**：用户在 UI 触发 DNS 重置，前端调用 `network_cmd.rs::reset_dns` [382|]。
2.  **权限判断与预处理**：
    *   命令层校验目标适配器列表，排除黑名单 [398-404|]。
    *   若当前进程已是管理员，直接调用 `dns_config::clear_adapter_dns_via_api` [419|] 并汇总结果。
    *   若非管理员，进入提权流程。
3.  **提权调度 (`helper_spawn`)**：
    *   调用 `spawn_elevated_helper` [51|]，传入操作 `clear_dns` 和目标适配器参数。
    *   **优先级 1：计划任务代理**。尝试通过已注册的 Windows 计划任务以 SYSTEM 身份执行。若失败或不可用，清除缓存并降级。
    *   **优先级 2：直接提权**。
        *   首先尝试 COM 静默提权 (`shell_exec_elevated`) [120-124|]。若用户取消 UAC 或 COM 失败且不允许弹窗，直接报错。
        *   若允许弹窗，调用 `run_elevated` 弹出标准 UAC 窗口 [125|]。
    *   **参数构建**：构建命令行 `--helper clear_dns ... --result "r-<pid>-<ts>.json"` [105-117|]。
4.  **Helper 执行**：
    *   新的 Helper 进程启动，解析参数 `parse_helper_args` [242|]。
    *   识别 `ClearDns` 操作，调用 `run_clear_dns` [461|]。
    *   在 Helper 的管理员上下文中，遍历 GUID 调用 `clear_adapter_dns_via_api` [461|]。
    *   将执行结果（成功/失败明细）序列化为 JSON。
5.  **结果回传**：
    *   Helper 通过 `write_result_file` [1083|] 原子写入结果文件到固定目录。
    *   主进程在 `spawn_elevated_raw` [128-142|] 中轮询读取该文件。
    *   读取成功后，通过 `read_helper_result` [31|] 解析 JSON，清理临时文件，并将结果返回给 `network_cmd`。
6.  **反馈用户**：`network_cmd` 根据解析出的 `success` 和 `message` 向用户展示重置结果 [452-464|]。

## Connections

[[helper-self-restart-elevation]]、[[adapter-operation-scope]]、[[desktop-network-dns]]、[[verification-gate-tiers]]