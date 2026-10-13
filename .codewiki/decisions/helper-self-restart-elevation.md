---
title: 提权辅助子进程 helper
type: module
source_files:
  - tauri-app/src-tauri/src/helper/mod.rs
  - tauri-app/src-tauri/src/platform/helper_spawn.rs
  - tauri-app/src-tauri/src/platform/elevation.rs
tags: [提权, helper, windows, dns, mac]
---

## Overview

本模块实现了跨进程提权执行体系。主进程（非管理员）通过三种通道之一拉起自身的管理员副本或 SYSTEM 计划任务，执行 DNS、MAC、路由等敏感操作。

核心设计：
1. **结果收口**：所有提权操作的结果必须写入固定目录 `ProgramData\CampusLogin\results\` 下的纯文件名（如 `r-1234-1695.json`），杜绝任意路径写原语。
2. **三级降级链**：计划任务代理（无 UAC）→ COM 静默提权 (CMSTPLUA) → ShellExecuteW runas (弹 UAC)。
3. **原子性**：结果文件采用“先写 `.tmp` 再 rename”的原子写入方式，防止主进程读到半截内容。

## Key Components

### `tauri-app/src-tauri/src/helper/mod.rs`

**公开函数：**
- `proxy_base_dir()` (`mod.rs:27-40`)：代理根目录。Windows 优先 `SHGetKnownFolderPath(FOLDERID_ProgramData)`，失败回退环境变量 `PROGRAMDATA`，再不行硬编码 `C:\ProgramData`。非 Windows 使用系统临时目录。
- `helper_results_dir()` (`mod.rs:43-45`)：固定结果目录 `proxy_base_dir().join("results")`。
- `helper_requests_dir()` (`mod.rs:48-50`)：请求目录 `proxy_base_dir().join("requests")`。
- `resolve_result_path(name: &str)` (`mod.rs:53-65`)：校验结果文件名并拼接完整路径。拒绝包含 `/`, `\`, `..`, `.` 开头或非 ASCII 字母数字/`-_` 字符的名称。
- `parse_helper_args(args: &[String])` (`mod.rs:136-160`)：解析 `--helper <op> [参数...] --result <文件名>` 命令行参数。返回 `HelperOp` 和可选的结果文件名。
- `run_helper(op: HelperOp, result_path: Option<String>)` (`mod.rs:163-194`)：执行 helper 操作并退出。根据 `op` 分发到具体执行函数，最后调用 `write_result_file`。
- `run_helper_task()` (`mod.rs:222-242`)：计划任务 worker 入口。扫描请求目录，按最旧优先处理，处理完空扫三轮后退出。

**公开枚举：**
- `HelperOp` (`mod.rs:79-116`)：提权操作类型。
    - `Dns { targets: Vec<String>, family: String }`：设置 DNS + DoH。
    - `ClearDns { targets: Vec<String> }`：恢复 DNS 自动获取。
    - `Mac { guid: String, mac_no_dash: String }`：修改适配器 MAC。
    - `EnableAdapter { name: String }`：启用被禁用的网络适配器。
    - `DisableAdapter { name: String }`：禁用网络适配器。
    - `RouteAdd { rows: Vec<String> }`：新增兜底默认路由。
    - `RouteDelete { rows: Vec<String> }`：删除兜底默认路由。
    - `EnableDevice { instance_id: String }`：PnP 设备级启用。
    - `SetMetric { rows: Vec<String> }`：设置接口跃点。
    - `RegisterTaskProxy`：注册计划任务提权代理。
    - `SelfCheck`：通道自检。

**数据结构：**
- `HelperResult` (`mod.rs:68-76`)：操作结果。
    - `success: bool`：是否成功。
    - `message: String`：摘要消息。
    - `op: String`：操作类型。
    - `logs: Vec<String>`：诊断日志（空时跳过序列化）。
    - `details: Option<serde_json::Value>`：操作明细（如 DNS 设置的详细状态，空时跳过序列化）。

**私有函数：**
- `validate_programdata_path(p: &Path)` (`mod.rs:14-24`)：校验 ProgramData 候选路径合法性（绝对、含盘符/根）。
- `known_folder_programdata()` (`mod.rs:29-47`)：经 `SHGetKnownFolderPath` 读取 ProgramData 实际位置。
- `build_op_from_args(op: &str, args: &[String])` (`mod.rs:119-133`)：从 op 名和参数构造 `HelperOp`。
- `process_request(path: &Path)` (`mod.rs:245-287`)：处理单个计划任务请求。读入内存后立即删除请求文件，防止重复执行。
- `oldest_request(dir: &Path)` (`mod.rs:215-234`)：取请求目录中最旧的 `req-*.json`。
- `run_dns(targets: &[String], family: &str, logs: &mut Vec<String>)` (`mod.rs:290-309`)：执行 DNS 设置。
- `run_clear_dns(targets: &[String], logs: &mut Vec<String>)` (`mod.rs:312-355`)：恢复 DNS 自动获取。
- `run_mac(guid: &str, mac_no_dash: &str, logs: &mut Vec<String>)` (`mod.rs:358-398`)：修改 MAC。包含注册表值校验、适配器查找、应用变更及清除伪装值。
- `decode_set_metric_row(row: &str)` (`mod.rs:401-432`)：解码 set_metric 条目。严格校验 GUID、协议栈、跃点值。
- `run_set_metric(rows: &[String], logs: &mut Vec<String>)` (`mod.rs:435-505`)：设置接口跃点。复用读路径枚举接口，修改副本后调用 `SetIpInterfaceEntry`。
- `run_enable_adapter(name: &str, logs: &mut Vec<String>)` (`mod.rs:508-546`)：启用适配器。双校验（名字合法 + 存在于本机列表）。
- `run_disable_adapter(name: &str, logs: &mut Vec<String>)` (`mod.rs:549-587`)：禁用适配器。
- `decode_route_row(row: &str)` (`mod.rs:590-615`)：解码路由条目。严格校验 IPv4 地址、跃点数、接口索引。
- `route_delete_miss(output_text: &str)` (`mod.rs:618-622`)：判断 route delete 输出是否为“找不到”，用于幂等处理。
- `run_route_add(rows: &[String], logs: &mut Vec<String>)` (`mod.rs:625-628`)：新增路由。
- `run_route_delete(rows: &[String], logs: &mut Vec<String>)` (`mod.rs:631-634`)：删除路由。
- `run_route_rows(op: &str, action: &str, rows: &[String], logs: &mut Vec<String>)` (`mod.rs:637-697`)：路由操作的公共执行体。全量校验后逐条执行 `route.exe`。
- `run_enable_device(instance_id: &str, logs: &mut Vec<String>)` (`mod.rs:700-752`)：PnP 设备启用。字符集白名单校验 + 存在性复核 + 轮询 problem 22 解除。
- `run_register_task(logs: &mut Vec<String>)` (`mod.rs:755-773`)：注册计划任务代理。
- `write_result_file(path: &Path, result: &HelperResult)` (`mod.rs:776-784`)：原子写结果文件。先写 `.tmp` 再 rename。

### `tauri-app/src-tauri/src/platform/helper_spawn.rs`

**公开函数：**
- `new_result_name()` (`helper_spawn.rs:24-30`)：生成唯一结果文件名 `r-<pid>-<ts>.json`。
- `spawn_elevated_helper(op: &str, args: &[&str], result_name: &str, timeout: Duration, allow_uac_prompt: bool)` (`helper_spawn.rs:46-93`)：启动提权 helper。三级通道自动降级。
- `spawn_elevated_raw(op: &str, args: &[&str], result_name: &str, timeout: Duration, allow_uac_prompt: bool)` (`helper_spawn.rs:96-143`)：直走 COM 静默 → runas 提权链。供代理注册动作自身与兜底使用。

**私有函数：**
- `read_helper_result(content: &str, result_path: &Path)` (`helper_spawn.rs:33-43`)：解析结果文件，并入日志，删除结果文件。

### `tauri-app/src-tauri/src/platform/elevation.rs`

**公开函数：**
- `is_admin()` (`elevation.rs:2-22`)：检查当前进程是否具有管理员权限。
- `run_elevated(cmd: &str, args: &str)` (`elevation.rs:25-47`)：通过 `ShellExecuteW` with `runas` verb 启动提权进程。
- `shell_exec_elevated(file: &str, params: &str, hide_window: bool)` (`elevation.rs:50-117`)：通过 COM ICMLuaUtil moniker 静默提权。使用 `BIND_OPTS3` 修复历史缺陷。
- `parse_guid(s: &str)` (`elevation.rs:120-143`)：解析 GUID 字符串。

**内部结构：**
- `ICMLuaUtilVtbl` (`elevation.rs:146-159`)：ICMLuaUtil 虚函数表布局。`shell_exec` 位于 slot 9。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                               Main Process                                  │
│  (Non-Admin)                                                                │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ spawn_elevated_helper() [helper_spawn.rs:46]                          │  │
│  │   1. Check proxy_usable()                                             │  │
│  │      ├─ Yes → run_via_task() (Task Proxy, No UAC)                     │  │
│  │      └─ No → ensure_registered()?                                     │  │
│  │         ├─ Success → run_via_task()                                   │  │
│  │         └─ Fail → fallthrough                                         │  │
│  │   2. spawn_elevated_raw() [helper_spawn.rs:96]                        │  │
│  │      ├─ shell_exec_elevated() [elevation.rs:50] (COM CMSTPLUA)        │  │
│  │      └─ run_elevated() [elevation.rs:25] (ShellExecuteW runas)        │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Poll Result File                                                      │  │
│  │   resolve_result_path(result_name)                                    │  │
│  │   read_helper_result()                                                │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Elevated / SYSTEM Process                           │
│  (Launched via --helper or --helper-task)                                   │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ parse_helper_args() [mod.rs:136]                                      │  │
│  │   ├─ --helper <op> [args] --result <name>                             │  │
│  │   └─ --helper-task (Worker Mode)                                      │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ run_helper() / process_request() [mod.rs:163, 245]                    │  │
│  │   Dispatch to specific ops:                                           │  │
│  │   ├─ Dns / ClearDns                                                   │  │
│  │   ├─ Mac                                                            │  │
│  │   ├─ EnableAdapter / DisableAdapter                                   │  │
│  │   ├─ RouteAdd / RouteDelete                                           │  │
│  │   ├─ EnableDevice                                                   │  │
│  │   ├─ SetMetric                                                      │  │
│  │   └─ RegisterTaskProxy                                              │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ write_result_file() [mod.rs:776]                                      │  │
│  │   Write to: ProgramData\CampusLogin\results\<result_name>.json.tmp    │  │
│  │   Rename to: <result_name>.json                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **发起提权**：主进程调用 `spawn_elevated_helper`。
2. **通道选择**：
   - 若计划任务代理可用，直接触发。
   - 否则尝试注册代理（如需）。
   - 最终降级到 COM 静默提权或 `runas` 弹 UAC。
3. **子进程启动**：自身 exe 以管理员/SYSTEM 身份启动，传入 `--helper <op> ... --result <name>`。
4. **执行操作**：`run_helper` 分发到具体函数（如 `run_dns`），执行 Win32/API 调用。
5. **结果回写**：`write_result_file` 将 `HelperResult` 序列化为 JSON，原子写入固定结果目录。
6. **主进程读取**：轮询结果文件，解析后清理临时文件，返回结果给调用方。

## Connections

- [[platform-com-elevation-undocumented]]：COM 提权机制细节。
- [[single-notification-channel]]：可能涉及的通知通道。
- [[task-proxy-registration]]：计划任务代理注册逻辑。

## Known Issues

1. **结果写入错误忽略**：`write_result_file` (`mod.rs:776-784`) 忽略全部 IO 错误。写失败时主进程只能等到超时（25-30s），无法立即感知失败。
2. **无自我超时**：helper 进程本身没有超时机制，依赖主进程轮询超时。
3. **无重入保护**：helper 进程没有防止并发执行的机制（尽管计划任务代理有 `MultipleInstances=IgnoreNew` 配置，但 COM/runas 路径无此保护）。
4. **MAC 伪装值清除失败**：`run_mac` (`mod.rs:358-398`) 中，如果清除注册表伪装值失败，仍会返回成功，导致重启后伪装 MAC 持续生效。