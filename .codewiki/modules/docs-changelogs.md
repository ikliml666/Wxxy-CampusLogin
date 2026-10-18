---
title: modules/docs-changelogs.md
type: module
source_files:
  - tauri-app/src-tauri/src/monitor/scheduled.rs
  - tauri-app/src-tauri/src/monitor/outbound_switch.rs
  - tauri-app/src-tauri/src/monitor/mod.rs
  - tauri-app/src-tauri/src/app/startup.rs
tags: [changelog, audit, outbound-switch, monitor, account, android, desktop, architecture]
---

## Overview

本模块「modules/docs-changelogs.md」是 Wxxy-CampusLogin 项目的核心文档集合，涵盖了从架构审计、版本变更日志（Changelog）、发布说明（Release Notes）到具体功能实施计划（Plans）及修复报告（Audit Fixes）的全生命周期记录。该模块不仅记录了 v2.3.3 至 v2.4.1 的版本迭代细节，还深入剖析了桌面端与安卓端的调度逻辑、夜间出站切换机制、账号体系增强以及系统稳定性加固方案。

核心产出包括：
1.  **深度审计记录**：识别出巨型函数、死代码、接口不一致及竞态条件，并提出了具体的重构建议（如 `pony_receipt.md`）。
2.  **版本演进轨迹**：详细记录了从安卓端正式可用、平板适配、注销协议重构到夜间自动切换落地的完整过程。
3.  **技术调研与设计**：基于 Windows PnP 状态机和路由 API 的调研，确立了“禁用网卡”优于“修改 Metric”的技术路线；定义了夜间切换的配置契约与判定逻辑。
4.  **修复执行闭环**：通过 A/B/C/D 四组修复计划，解决了 Panic 策略、竞态条件、前端状态同步及生物识别链路等关键问题。

## Key Components

### 桌面端调度器 (Desktop Scheduler)
*   **文件**: `tauri-app/src-tauri/src/monitor/scheduled.rs`
    *   **职责**: 桌面端定时任务编排中心，负责登录/注销循环、夜间出站切换、夜切验证。
    *   **关键函数**:
        *   `run_scheduled_action_loop`: 30s 主循环入口（第 67-158 行）。由 `watcher::run_startup_tasks` 经 `task_manager.spawn` 拉起。包含启动对账、定时登录/注销判定、夜间出站切换、运营商夜切逻辑。
        *   `is_quitting`: App 是否正在退出。抽成独立函数只为让循环体不跨 await 持有 State 守卫（第 160-162 行）。
        *   `reconcile_scheduled_logout_on_startup`: 启动对账（定时注销防冷启动误触发）（第 164-190 行）。应用启动时若当日注销时刻已过，消耗当日额度不执行动作。
        *   `evaluate_scheduled`: 单拍判定：命中返回 `(应登录, 应注销)`，不置任何标记。门控用拍前切换态（第 192-206 行）。
        *   `mark_scheduled_day_fired`: 审计 A7：当日额度标记，在动作抢锁成功后置位（第 208-216 行）。
        *   `format_minutes`: 格式化分钟数为 HH:MM 字符串（第 218-220 行）。
        *   `run_scheduled_login`: 定时登录：复用 `full_login`，成功后走 `post_login_handler`（第 222-229 行）。
        *   `spawn_login_action`: 登录动作共用执行体（定时登录与晚间断网切换登录同款）（第 231-257 行）。抢 `is_logging_in` 互斥锁，抢不到本拍跳过。
        *   `apply_night_switch_action`: 晚间断网切换动作：改内存配置 → 落盘 → 触发登录（第 259-285 行）。
        *   `run_scheduled_logout`: 定时注销：复用 `full_logout`，成功后对齐全量注销后处理（第 287-295 行）。
        *   `perform_logout`: 注销执行体（定时注销与夜切验证复登前的注销同款）（第 297-332 行）。抢 `is_logging_out` 互斥锁，成功后取消登录后自动退出、重置登录/重连状态并启动 60s 注销保护期。
        *   `verify_night_switch`: 夜切后的登录生效自动验证（与安卓端同构，独立 tokio 任务不阻塞监控循环）（第 334-375 行）。等 15s → 查 eportal chkstatus(JSONP) 核对在线账号 uid 与期望运营商后缀 → 仍不符则注销重登一轮再复验。
        *   `night_verify_config`: 读取验证所需的配置快照（第 377-383 行）。
        *   `night_switch_verify_round`: 一轮验证（NIGHT_VERIFY_ATTEMPTS 次重试、间隔 5s）（第 385-403 行）。
        *   `night_switch_check_once`: 单次 chkstatus 查询 + uid 核对（第 405-423 行）。
        *   `portal_origin`: portal_url 的 origin（第 425-437 行）。
        *   `expected_operator_display`: 期望运营商的展示名（第 439-445 行）。
    *   **常量**:
        *   `SCHEDULED_TICK_MS`: 判定拍间隔，30s（第 51 行）。
        *   `NIGHT_VERIFY_WAIT_SECS`: 夜切验证时序参数，登录编排完成后等 15s 再查（第 55 行）。
        *   `NIGHT_VERIFY_RETRY_SECS`: 查询失败重试间隔 5s（第 56 行）。
        *   `NIGHT_VERIFY_ATTEMPTS`: 最多 3 次（第 57 行）。
        *   `NIGHT_VERIFY_HTTP_TIMEOUT_SECS`: 单次请求 15s 超时（第 58 行）。

### 纯判定层 (Pure Decision Layer)
*   **文件**: `tauri-app/src-tauri/src/monitor/outbound_switch.rs`
    *   **职责**: 出站切换的纯判定与序列化薄模块。
    *   **关键函数**:
        *   `is_campus_adapter`: 逐卡判定该适配器是否处于校园网内（第 19-41 行）。IP 与校园网关同 /18 网段，或绑该卡源 IP 可达校园网关/portal 主机。存在重复调用 `gateway_probe` 的逻辑（同参数双倍探测），意图是去抖动容忍瞬时抖动。
        *   `portal_probe_host`: 从 portal_url 提取可作绑源探测目标的私网 IPv4 主机（第 43-56 行）。
        *   `effective_outbound_priority`: 出站优先级的生效列表（第 58-75 行）。用户已排序按原序透传；未排序默认无线卡优先、有线卡随后。
        *   `select_outbound_candidate`: 按优先级名序选出出站目标卡（第 77-104 行）。跳过不在优先级列表、无 IP、处于校园网内的卡。
        *   `snapshot_json`: 序列化选中卡的接口跃点快照为 JSON（第 128-140 行）。
        *   `disabled_adapters_json`: 序列化待禁用校园网卡名单为 JSON（第 153-155 行）。
        *   `parse_disabled_adapters`: 解析禁用名单快照（第 157-166 行）。空串 → 空名单；非法 JSON → Err。
        *   `parse_standby_route`: 解析兜底路由快照（第 185-187 行）。
        *   `standby_cleanup_decision`: 兜底路由删除判定（纯函数，跨端可测）（第 207-222 行）。
        *   `standby_route_cleanup`: 读实时路由表判定历史兜底路由状态（Windows）（第 224-263 行）。
        *   `unsafe_to_disable`: 该卡是否不能参与夜间禁用（实例 ID 都读不到的卡保守跳过）（第 265-273 行）。
        *   `select_campus_to_disable`: 选出本次切换要临时禁用的校园网卡（第 275-311 行）。
    *   **Struct/Model**:
        *   `SnapshotRow`: 快照行（第 106-111 行）。字段：`guid` (String), `family` (u16), `automatic` (bool), `metric` (u32)。
        *   `DisabledRow`: 待禁用校园网卡行（第 113-117 行）。字段：`guid` (String), `name` (String)。
        *   `StandbyRoute`: 历史兜底默认路由快照（第 168-177 行）。字段：`dest` (String), `mask` (String), `gateway` (String), `metric` (u32), `if_index` (u32)。
        *   `StandbyCleanup`: 枚举类型（第 179-183 行）。变体：`AlreadyGone`, `ForeignRowKept`, `LastDefaultPath`, `Safe`。

### 安卓端调度器 (Android Scheduler)
*   **文件**: `android/src-tauri/src/monitor_loop.rs`
    *   **职责**: 安卓端同构调度，包含 `run_scheduled_actions` 与 `run_check_once`。
    *   **关键函数**:
        *   `run_scheduled_actions`: 包含同构的出站切换与运营商夜切逻辑。
        *   `run_check_once`: 建议拆分为 probe/consume/login 三步。

### 前端面板 (Frontend Panels)
*   **文件**: `tauri-app/frontend/src/network/NetworkPanel.tsx`
    *   **职责**: 网络设置面板。
    *   **关键区域**:
        *   DNS 设置: 结构重复，建议合并。
        *   DHCP: 配置项。
        *   出站拖拽: 多流耦合。
*   **文件**: `tauri-app/frontend/src/account/AccountPanel.tsx`
    *   **职责**: 账号管理面板。
    *   **关键区域**:
        *   运营商绑定流: 与增删改逻辑耦合。
        *   账号增删改: 耦合于同一组件。
*   **文件**: `tauri-app/frontend/src/hooks/tauriApi.ts`
    *   **职责**: IPC 命令封装。
    *   **关键发现**:
        *   存在多处 `desktopOnly` 假接口（恒 reject），如 `getAdapters`、`enableAdapter` 等，前端调用方被迫使用 `?.` 兜底。

### 死代码锚点 (Dead Code Anchors)
*   **文件**: `tauri-app/src-tauri/src/auth/portal.rs`
    *   **现象**: `data_length` 字段恒为 0，构造点填 0，消费点传 0，测试断言检查 ==0。
*   **文件**: `android/src-tauri/src/campus_detect.rs`
    *   **现象**: 安卓 `detect_campus` 死命令，注册在 `lib.rs`，前端实际调用的是 `checkCampusStatus`。

### 账号体系增强 (Account System Enhancement)
*   **文件**: `docs/superpowers/plans/2026-09-14-account-adapter-binding.md`
    *   **职责**: 定义账号体系增强的实施蓝图。
    *   **关键字段**:
        *   `display_name` (String, default ""): 显示名称。
        *   `adapter1_account` (String, desktop only): 桌面适配器1指定账号。
        *   `adapter2_account` (String, desktop only): 桌面适配器2指定账号。
    *   **行为规则**:
        *   R2 自动建号: `id = sanitize(user)`，幂等短路。
        *   R3 显示名管理: 长度 1-32，无控制字符，唯一性检查。
        *   R4 切换刷新修复: 后端返回 `activeAccount`，前端守卫 `result?.activeAccount !== undefined`。

### 夜间出站切换设计 (Night Outbound Switch Design)
*   **文件**: `docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md`
    *   **职责**: 定义“夜间自动切换”功能的完整技术方案。
    *   **配置字段**:
        *   `enableNightOutboundSwitch`: `false`（总开关）。
        *   `outboundPriority`: `[]`（网卡友好名有序列表）。
        *   `outboundMetricRestore`: `''`（桌面快照 JSON）。
        *   `nightOutboundRestore`: `''`（安卓标记）。
    *   **判定逻辑**:
        *   `switch_minute_for(weekday)`: 周日至四返回 `1380` (23:00)，周五六返回 `1410` (23:30)。
        *   `evaluate_night_outbound(...)`: 返回 `Switch | Restore | None`。

### 审计修复计划 (Audit Fix Plan)
*   **文件**: `changelogs/_audit-fix-plan.md`
    *   **职责**: 定义通用约定、接口契约及各组具体任务（A-D 组）。
    *   **关键约定**:
        *   栈溢出预防: `async fn` 内禁止 ≥4KB 栈数组。
        *   新命令: `cancel_download_update`。
        *   新事件: `scheduled-action-skipped`, `mac-reset-cycle`, `update-unverified`。
*   **文件**: `changelogs/_audit-fix-report-{A,B,C,D}.md`
    *   **A 组 (桌面 Rust)**: 18 项修复，包括 Panic 策略调整、账号操作落盘顺序、静默吞错改上报、单飞互斥锁、同步命令移出主线程、下载取消机制等。
    *   **B 组 (安卓 Rust)**: 9 项修复，包括 Panic 策略、延迟巡检间隔失效修复、配置 IO 锁统一、后台监控 FGS 存活互校等。
    *   **C 组 (桌面前端)**: 11 项修复，聚焦 UI 状态一致性、错误感知及新事件接线。
    *   **D 组 (安卓 Kotlin + 前端)**: 5 项修复，确保安卓端行为与桌面端一致，完善生物识别链路。

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Frontend (React/Tauri)                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐ │
│  │ NetworkPanel │  │ AccountPanel │  │ Settings     │  │ useEventList.. │ │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └───────┬────────┘ │
│         │                 │                  │                  │          │
│         ▼                 ▼                  ▼                  ▼          │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                        IPC Bridge (Tauri Commands)                   │  │
│  │  startup.rs / lib.rs / tauriApi.ts                                 │  │
│  └──────┬──────────────────────────────────────────────────────────────┘  │
└─────────┼─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Backend (Rust/Kotlin)                               │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────────┐ │
│  │ Desktop Monitor  │  │ Android Monitor  │  │ Shared Logic             │ │
│  │ scheduled.rs     │  │ monitor_loop.rs  │  │ campus_login_lib         │ │
│  └──────┬───────────┘  └──────┬───────────┘  └──────────┬───────────────┘ │
│         │                     │                         │                  │
│         ▼                     ▼                         ▼                  │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────────┐ │
│  │ Network Bind     │  │ ConnectivityMgr  │  │ Auth Service             │ │
│  │ adapter_watch    │  │ report_network.. │  │ auth/service.rs          │ │
│  │ outbound_switch  │  │ Multinetwork..   │  │ full_login               │ │
│  └──────────────────┘  └──────────────────┘  └──────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **调度分层**:
    *   `network_cmd.rs` -> `scheduled.rs` (入口) -> `outbound_switch.rs` (判定) + `scheduled.rs` (执行 via `outbound_night`)。
    *   现状：执行逻辑厚植于 `scheduled.rs`，建议搬至 `monitor/outbound_night.rs`。
2.  **IPC 收敛点**:
    *   桌面 `useInitialDataLoad.ts` 发起 `get_gpu_info`、`get_log_retention_days` 等独立请求。
    *   建议：并入 `get_init_data` 一次返回，减少 IPC 往返。
3.  **双端同构**:
    *   `scheduled.rs` (桌面) 与 `monitor_loop.rs` (安卓) 在 `run_scheduled_actions` 逻辑上高度同构。
    *   判定层已共享 (`campus_login_lib`)，但编排层未共享，形成真缝。
4.  **账号体系数据流**:
    *   **自动建号**: `save_config` 成功后触发 -> `sanitize(user)` 生成 ID -> 幂等检查 -> 写入磁盘。
    *   **切换刷新**: `switch_account` IPC 调用 -> 后端更新 `active_account` 并广播 `config-changed` -> 前端监听事件 -> 强制刷新列表和 Store。
    *   **适配器指定**: `full_login` 根据传入适配器名称 -> 查找 `adapter1_account` 或 `adapter2_account` -> 加载对应 Config 副本 -> 注入凭据 -> 下游登录流程。

## Connections

*   本模块与 `modules/core-network` 紧密相关，涉及网络绑定、出站切换及校园判定逻辑。
*   与 `modules/user-auth` 交互频繁，涉及账号管理、认证协议及生物识别验证。
*   `docs/research/2026-09-27-windows-outbound-switch-research.md` 为 `docs/superpowers/specs/2026-09-21-night-outbound-switch-design.md` 提供了底层原理验证。
*   `changelogs/_audit-fix-plan.md` 驱动了 `changelogs/_audit-fix-report-{A,B,C,D}.md` 的执行与验证。

## Known Issues

1.  **非显然 Bug**: `outbound_switch.rs` `is_campus_adapter` 中 `gateway_probe` 被调用两次且参数完全相同，意图是去抖动但实际无效，导致双倍 ICMP 探测开销。
2.  **死接口陷阱**: 安卓前端 `NetworkPanel.tsx` 渲染网卡列表时调用 `getAdapters`，但该命令在安卓侧被 `desktopOnly` 包装为 reject，导致列表恒空。这是典型的“死 UI + 死命令”。
3.  **配置兼容风险**: `config/model.rs` 使用 `#[serde(default)]` 容器级默认值，若强行对 Config struct 进行字段分组，会破坏旧配置兼容性（缺字段整体失败）。因此配置扁平化应谨慎，优先删除死字段而非重组结构。
4.  **异步命令栈溢出**: 安卓 `download_update` async fn 中局部大数组 `[0u8; 65536]` 在 release 构建下压穿 JavaBridge 线程栈，必须改用 `vec!` 堆分配。
5.  **WebView2 最小化节流**: 系统节流定时器导致面板数据陈旧，需通过 `visibilitychange` 和 `focus` 事件主动触发刷新。
6.  **启动竞态**: `adapter_watch` 15s 轮询与前端初始加载存在竞态，导致禁用列表永久冻结。修复：轮询加首轮无条件推送，且禁用项从活跃 adapters 派生。
7.  **Captive Portal 与 VPN 干扰**: 安卓 WiFi 绑定受 captive portal (无 INTERNET 能力) 和 VPN 活动影响，需多策略回退 (`requestNetwork` 去 capability, VPN 检测)。
8.  **URL 解析一致性**: 多处手写 URL 解析 (`portal_origin`)，易出错且不通用，需收敛为纯函数。
9.  **路径约定差异**: 安卓 `app_data_dir` 与 `filesDir` 差异导致更新安装失败，需严格对齐 FileProvider 路径。
10. **YAGNI 代码**: `events.rs` 预留了 trait 注入的注释但未实现，属于 YAGNI 代码，应清理。