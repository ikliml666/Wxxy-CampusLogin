---
title: "DNS 重置必须走提权路径，确保设置/撤销能力对等"
type: decision
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
tags: [决策, DNS, 提权, helper, 可逆性]

## Overview

本模块（`tauri-app/src-tauri/src/helper/mod.rs`）定义了提权辅助子进程的核心逻辑，其中包含 DNS 重置（`HelperOp::ClearDns`）功能的完整实现架构。核心决策在于：**DNS 重置操作必须具备与 DNS 设置（`HelperOp::Dns`）完全一致的提权路径**，严禁在非管理员权限下直接内联调用底层 API。

该架构通过 `helper` 模块调度高权限 Helper 进程，支持两种执行形态（见文件头注释 1-13）：
1. **命令行模式 (`--helper`)**：主进程经 UAC/CMSTPLUA 提权后启动管理员副本，执行单次操作并写入结果文件。
2. **计划任务 Worker 模式 (`--helper-task`)**：以 SYSTEM 身份运行的后台代理（`run_helper_task` 350-373），轮询请求目录执行任务，无需 UAC。

两种形态共用 `build_op_from_args` 构造 `HelperOp`，并通过“固定目录结果文件协议”异步获取执行状态（`resolve_result_path` 88-100、`write_result_file` 1235-1245）。无论用户以普通还是管理员身份启动应用，`clear_dns` 都能通过提权机制成功执行，满足“设置/撤销能力对等”的产品原则。

## Key Components

### `tauri-app/src-tauri/src/helper/mod.rs`

提权辅助子进程的核心逻辑。统一处理两种执行形态，并严格约束 SYSTEM 写路径（审计 A11/P0-2）。

**路径安全与固定根目录**：
- `validate_programdata_path` (19-28)：校验 ProgramData 候选路径合法性（绝对、非相对、含盘符/根），防止路径劫持。
- `proxy_base_dir` (31-50)：优先 `SHGetKnownFolderPath(FOLDERID_ProgramData)`（55-72 `known_folder_programdata`），失败回退 `PROGRAMDATA` 环境变量或硬编码 `C:\ProgramData`，拼接 `CampusLogin`。
- `helper_results_dir` (75-77)：返回 `{ProgramData}/CampusLogin/results` 固定结果目录。
- `helper_requests_dir` (80-82)：返回 `{ProgramData}/CampusLogin/requests` 请求目录。
- `resolve_result_path` (88-100)：**核心安全函数**。仅接受纯文件名（拒绝 `/`、`\`、`..`、`.` 开头、长度>128、非 ASCII 字母数字及 `-_.` 字符），确保 SYSTEM worker 只能写入固定结果目录，杜绝任意路径写原语。

**数据模型**：
- `HelperResult` (102-112)：序列化结构体，全部字段：
  - `success: bool` — 操作是否成功
  - `message: String` — 汇总文案
  - `op: String` — 操作名
  - `logs: Vec<String>` — 诊断日志（`#[serde(skip_serializing_if = "Vec::is_empty")]`，空时跳过序列化）
  - `details: Option<serde_json::Value>` — 操作明细（如 DNS 重置的 `restored`/`failed`），`None` 时跳过序列化
- `HelperOp` 枚举 (115-164)：定义所有支持的操作（部分受 `#[cfg(target_os = "windows")]` 保护）：
  - `Dns { targets: Vec<String>, family: String }` (119) — 设置 DNS+DoH
  - `ClearDns { targets: Vec<String> }` (122) — **恢复 DNS 自动获取（DNS 重置）**
  - `Mac { guid: String, mac_no_dash: String }` (124)
  - `EnableAdapter { name: String }` (127)
  - `DisableAdapter { name: String }` (130)
  - `RouteDelete { rows: Vec<String> }` (136) — 精确删除兜底路由（已废弃 `route_add`）
  - `EnableDevice { instance_id: String }` (139)
  - `ReenumerateDevice { instance_id: String }` (144)
  - `SetMetric { rows: Vec<String> }` (150)
  - `RegisterTaskProxy` (153)
  - `RegisterBootTask { config_b64: String }` (158)
  - `RemoveBootTask` (161)
  - `SelfCheck` (163)
- `TaskRequest` (336-343)：计划任务 Worker 使用的 JSON 结构，字段：`op: String`、`args: Vec<String>`（默认空）、`result: String`（纯文件名）。

**参数解析与分发**：
- `build_op_from_args` (169-256)：通用构造器，扫描位置参数与 `--family` 开关，将字符串 op 转为 `HelperOp`；严格校验必填参数（如 `mac` 需要 guid+mac_no_dash，`register_boot_task` 需要 config_b64）。
- `parse_helper_args` (260-289)：解析 `--helper <op> [args...] --result <文件名>`，命中返回 `Some((HelperOp, Option<String>))`，未命中返回 `None`。
- `run_helper` (292-331)：命令行模式入口。匹配 `HelperOp` 调用具体执行函数，生成 `HelperResult`，经 `resolve_result_path` 校验后 `write_result_file`，返回进程退出码。
- `run_helper_task` (350-373)：Worker 主循环。扫描 `requests` 目录取最旧请求（`oldest_request` 376-392），处理完空扫两轮兜住竞态，无请求则退出。
- `oldest_request` (376-392)：按修改时间排序取 `req-*.json` 中最旧者。
- `process_request` (396-461)：单请求处理。**读入内存后立即删请求文件** (406) 防重复执行/篡改；解析 JSON → `build_op_from_args` → 执行 → 写结果。

**具体操作执行函数（含 DNS 重置）**：
- `run_dns` (463-482)：调用 `crate::network::dns_setup::setup_dns_doh_admin`，透传 `details`。
- `run_clear_dns` (487-542)：**DNS 重置核心**。逐适配器 GUID 调用 `crate::platform::dns_config::clear_adapter_dns_via_api` (515)；成功/失败明细写入 `details.restored`/`failed` (539)；三态汇总文案（全成功/部分失败/全失败）；非 Windows 直接返回失败 (488-498)。
- `run_mac` (544-604)：校验 MAC 12 位十六进制，调 `apply_mac_change_via_registry` 后立刻 `remove_mac_from_registry` 清除持久伪装值。
- `decode_set_metric_row` (610-640)：严格解码 `"guid:family:automatic:metric"` 条目。
- `run_set_metric` (646-727)：夜间出站切换，调 Win32 `SetIpInterfaceEntry`，强制 `SitePrefixLength=0`，逐条执行。
- `run_enable_adapter` (732-782) & `run_disable_adapter` (793-873)：双校验适配器名+本机存在；禁用优先非持久化设备级禁用 (`disable_adapter_nonpersistent`) 避免跨重启 Problem 22。
- `decode_route_row` (879-898)：严格解码 `"dest|mask|gateway|metric|if_index"`。
- `run_route_delete` (908-970)：调 `delete_route_exact` 四元组精确删除，避免误删 DHCP 路由。
- `valid_instance_id` (976-983) & `invalid_instance_id_result` (988-1000)：设备实例 ID 白名单防注入。
- `run_enable_device` (1006-1062)：pnputil 启用后轮询复核脱离 Problem 22。
- `run_reenumerate_device` (1067-1088)：总线重枚举恢复幽灵态 USB。
- `run_register_task` (1092-1110)：注册计划任务代理。
- `run_register_boot_task` (1118-1199)：写核心配置+收紧 ACL+注册 BOOT 任务。
- `run_remove_boot_task` (1203-1231)：幂等移除开机任务及配置。
- `write_result_file` (1235-1245)：原子写（先 `.tmp` 再 rename），确保结果目录存在。

**测试模块** (1247-1579)：覆盖路径校验、参数解析、Op 构造、MAC/set_metric/route 解码、实例 ID 白名单、JSON roundtrip 等。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│ Main Process (Tauri)                                            │
│ 构造 `--helper clear_dns <targets> --result <file>`             │
│ 或写入 TaskRequest JSON 到 helper_requests_dir (80-82)         │
└───────────────────────────────────┬─────────────────────────────┘
                                    │ 提权启动 / SYSTEM worker 拉起
                                    ▼
┌─────────────────────────────────────────────────────────────────┐
│ Helper Subprocess (--helper / --helper-task)                    │
│                                                                 │
│ parse_helper_args (260-289) / process_request (396-461)        │
│        │                                                        │
│        ▼                                                        │
│ build_op_from_args (169-256) → HelperOp::ClearDns {targets}    │
│        │                                                        │
│        ▼                                                        │
│ run_helper (292-331) / run_helper_task (350-373)               │
│        │                                                        │
│        ▼                                                        │
│ run_clear_dns (487-542)                                        │
│        │ 逐 GUID 调 clear_adapter_dns_via_api (515)             │
│        ▼                                                        │
│ write_result_file (1235-1245)                                  │
└───────────────────────────────────┬─────────────────────────────┘
                                    │ 原子写入
                                    ▼
┌─────────────────────────────────────────────────────────────────┐
│ Fixed Directory: helper_results_dir (75-77)                     │
│ 经 resolve_result_path (88-100) 收口到纯文件名                  │
│ 主进程轮询读取 JSON 结果                                        │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **请求发起**：主进程构造 DNS 重置请求。命令行形态：`--helper clear_dns {GUID1} {GUID2} --result r-<id>.json`（`parse_helper_args` 260-289 解析）；计划任务形态：写入 `{"op":"clear_dns","args":["{GUID}"],"result":"r.json"}` 到 `helper_requests_dir`（`TaskRequest` 336-343）。
2. **Helper 启动与解析**：
   - `--helper` 模式：提权副本启动，`parse_helper_args` 提取 op 与参数，调 `build_op_from_args` 生成 `HelperOp::ClearDns { targets }` (122)。
   - `--helper-task` 模式：`run_helper_task` 取最旧请求，`process_request` 读入并删除请求文件后同样构造 Op。
3. **权限上下文执行**：`run_helper` (292-331) 或 `process_request` 内部分发至 `run_clear_dns` (487-542)。该函数在管理员/SYSTEM 上下文中运行，枚举适配器 (`get_adapters_force` 505)，逐 GUID 调用 `crate::platform::dns_config::clear_adapter_dns_via_api` (515)。
4. **结果汇总**：`run_clear_dns` 将成功名填入 `restored`、失败明细填入 `failed`，生成 `HelperResult` (534-540)，`details` 携带上述数组。
5. **结果回传**：`write_result_file` (1235-1245) 经 `resolve_result_path` (88-100) 校验纯文件名后，原子写入 `helper_results_dir` (75-77) 下。主进程读取该 JSON，解析 `success`/`message`/`details` 反馈用户。

## Connections

[[helper-self-restart-elevation]]、[[adapter-operation-scope]]、[[desktop-network-dns]]、[[verification-gate-tiers]]