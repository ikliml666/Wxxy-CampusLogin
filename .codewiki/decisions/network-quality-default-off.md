---
title: 网络质量检测默认关闭（schema v3）及多端 UI 适配
type: decision
source_files:
  - android/src-tauri/src/config_state.rs
  - android/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/App.tsx
tags: [决策, 质量检测, 默认值, 安卓, 双端同语义]

## Overview

本决策记录 `enable_network_quality` 字段在 Android 端（及桌面端同语义）的默认值变更及其对前端 UI 布局、状态展示的影响。

核心决策：2026-09-12 起，`enable_network_quality` **默认关闭 (false)**。
理由：质量检测链路包含 12+ 外网目标探测，启动即跑一轮耗时数秒，功耗较高。关闭后，质量页由后台检测状态代替展示，提升省电体验。

该决策通过后端 `config_schema_version` 迁移机制（v2→v3）落地，并同步调整了桌面端与移动端的前端交互逻辑。当前代码（截至提供源码）中，后端 `android/src-tauri/src/config_state.rs` 的 `Settings::default()` 将 `enable_network_quality` 设为 `false` (Line 174)，`config_schema_version` 为 7 (Line 197)；迁移函数 `migrate_legacy_defaults` (Line 355-417) 在 `config_schema_version < 3` 时强制刷 `false` (Line 367-369)。前端常量 `android/frontend/src/settings/constants.ts` 的 `DEFAULT_CONFIG` 仍声明 `enableNetworkQuality: true` (Line 38)，但仅作初始化参考，实际以后端 IPC 加载覆写。桌面端 `tauri-app/frontend/src/App.tsx` 在 `AppInner` 中订阅该字段 (Line 143) 并条件渲染 `QualityPanel` (Line 383-391)。本次提供源码未包含安卓前端 `App.tsx`，故移动端 UI 适配细节从略。

## Key Components

### 后端配置结构 (`android/src-tauri/src/config_state.rs`)

#### `Settings` Struct (Line 26-128)
全量配置结构（camelCase IPC 契约），字段清单（含行号）：
- `user: String` (Line 30)
- `password: String` (Line 31)
- `self_hello_enabled: bool` (Line 32)
- `self_reverify_each_action: bool` (Line 33)
- `allow_2d_face_verify: bool` (Line 35)
- `operator: String` (Line 36)
- `enable_night_operator_switch: bool` (Line 38)
- `night_operator_restore: String` (Line 40)
- `enable_night_outbound_switch: bool` (Line 42)
- `outbound_priority: Vec<String>` (Line 44)
- `outbound_metric_restore: String` (Line 46)
- `outbound_disabled_adapters: String` (Line 48)
- `outbound_standby_route: String` (Line 50)
- `night_outbound_restore: String` (Line 52)
- `dns_optimize_adapters: Vec<String>` (Line 54)
- `device_aliases: std::collections::HashMap<String, String>` (Line 57, serde default)
- `auto_login_on_start: bool` (Line 59)
- `enable_background_check: bool` (Line 60)
- `background_check_interval: u64` (Line 61)
- `background_check_idle_interval: u64` (Line 63)
- `auto_login_on_preparation: bool` (Line 64)
- `max_disconnect_reconnect: u32` (Line 65)
- `auto_login_cooldown_secs: u64` (Line 66)
- `theme_mode: String` (Line 68)
- `enable_notification: bool` (Line 69)
- `custom_theme_color: String` (Line 70)
- `default_panel: String` (Line 71)
- `display_name: String` (Line 73)
- `active_account: String` (Line 74)
- `enable_boot_autostart: bool` (Line 75)
- `enable_latency_test: bool` (Line 77)
- `latency_test_interval: u64` (Line 78)
- `enable_network_quality: bool` (Line 79) — 本决策核心字段
- `skip_ttfb_in_latency: bool` (Line 80)
- `skip_content_in_latency: bool` (Line 81)
- `portal_url: String` (Line 83)
- `fixed_gateway: String` (Line 84)
- `required_network_name: String` (Line 85)
- `enable_network_name_check: bool` (Line 86)
- `campus_gateway: String` (Line 87)
- `campus_check_start_minutes: u16` (Line 89)
- `campus_check_end_minutes: u16` (Line 91)
- `scheduled_login_minutes: u16` (Line 93)
- `scheduled_logout_minutes: u16` (Line 95)
- `update_source: String` (Line 98)
- `log_retention_days: u32` (Line 100)
- `config_schema_version: u32` (Line 127)

#### `Default` Impl (Line 130-200)
- `enable_network_quality`: 默认值 `false` (Line 174)，注释明确 2026-09-12 起默认关闭（省电）。
- `enable_latency_test`: 默认 `false` (Line 168)。
- `latency_test_interval`: 默认 `600_000` (Line 170)。
- `skip_ttfb_in_latency`: 默认 `true` (Line 175)。
- `skip_content_in_latency`: 默认 `true` (Line 176)。
- `config_schema_version`: 默认 `7` (Line 197)。

#### `migrate_legacy_defaults` Function (Line 355-417)
执行旧配置一次性迁移：
- `legacy` 判定：`s.config_schema_version < 3` (Line 356)。
- 在 `if legacy` 块 (Line 360-374) 内：`if s.config_schema_version < 3 { s.enable_network_quality = false; }` (Line 367-369)。落盘后版本置 3。
- 后续 v3→v4 (Line 375-383) 补 `background_check_idle_interval`；v4→v5 (Line 384-392) 刷 `campus_check_end_minutes`；v5→v6 (Line 393-401) 刷 `latency_test_interval`；v6→v7 (Line 402-416) 刷夜切与定时哨兵。

#### `load_from` Function (Line 326-336)
调用 `migrate_legacy_defaults` (Line 328)，并扩展旧 SSID 单值 (Line 332-334)。

#### `masked_for_display` Function (Line 424-430)
返回掩码后配置 JSON，密码非空变 `***`，敏感信息出站唯一出口。

#### `emit_config_changed` Function (Line 436-439)
广播 `config-changed` 事件，payload 为掩码配置，前端 store 依赖此同步。

#### 测试模块 (Line 522-827)
- `默认值_业务语义` 测试 (Line 632-664)：断言 `enable_network_quality` 为 `false` (Line 654)，`config_schema_version` 为 7 (Line 646) 等。
- `迁移_旧默认15s升60s且落盘后用户值不被覆盖` (Line 666-711)：验证 v2→v3 关闭质量检测 (Line 690)，v7 迁移后用户显式开启不被覆盖 (Line 709)。

### 前端常量定义 (`android/frontend/src/settings/constants.ts`)

#### `DEFAULT_CONFIG` Object (Line 5-58)
- `enableNetworkQuality: true` (Line 38) — 前端初始态，不与后端默认一致，实际被 IPC 覆盖。
- `enableLatencyTest: true` (Line 33)
- `latencyTestInterval: 600000` (Line 35)
- `skipTtfbInLatency: true` (Line 39)
- `skipContentInLatency: true` (Line 40)
- `configSchemaVersion: 7` (Line 57)

### 桌面端应用入口 (`tauri-app/frontend/src/App.tsx`)

#### `AppInner` Component (Line 114-576)
- **Config 订阅** (Line 143): `const configEnableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality)`
- **Quality Panel 渲染门控** (Line 383-391):
  ```tsx
  case 'quality':
    panelContent = configEnableNetworkQuality !== false ? (
      <QualityPanel
        onUpdateConfig={updateConfig}
        onRefreshQuality={refreshQuality}
        onToggleLatencyTest={handleToggleLatencyTest}
      />
    ) : null
  break
  ```
  当后端下发 `false` 时，不渲染 `QualityPanel`，返回 `null`。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Backend (Rust)                         │
│  android/src-tauri/src/config_state.rs                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Settings::default()                                   │  │
│  │ enable_network_quality = false (Line 174)             │  │
│  │ config_schema_version = 7 (Line 197)                  │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ migrate_legacy_defaults() (Line 355-417)              │  │
│  │ v2→v3: enable_network_quality = false (Line 367-369)  │  │
│  └───────────────────────────────────────────────────────┘  │
└───────────────────────┬─────────────────────────────────────┘
                        │ IPC: get_config / config-changed
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                      Frontend (React)                       │
│  tauri-app/frontend/src/App.tsx (Desktop)                   │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ useConfigStore.config.enableNetworkQuality (Line 143)  │  │
│  │ Quality Panel: 条件渲染 (Line 383-391)                 │  │
│  └───────────────────────────────────────────────────────┘  │
│  android/frontend/src/settings/constants.ts                │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ DEFAULT_CONFIG.enableNetworkQuality=true (Line 38)    │  │
│  │ (仅初始化参考, 被 IPC 覆写)                             │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **首次启动/升级**:
   - 后端 `load_from` (Line 326) 读取磁盘配置。
   - 若 `config_schema_version < 3`，`migrate_legacy_defaults` (Line 355) 将 `enable_network_quality` 设为 `false` (Line 367-369)，版本逐步升至 7。
   - 保存至磁盘（`save_file` 经加密），并通过 `emit_config_changed` (Line 436) 广播。
2. **前端接收**:
   - 桌面端 `App.tsx` 中 `useConfigStore` 更新 `config.enableNetworkQuality` 为 `false`（来自 IPC 掩码配置）。
   - 安卓前端 `constants.ts` 的 `DEFAULT_CONFIG` 初始 `true` 被覆盖。
3. **UI 响应 (桌面端)**:
   - `configEnableNetworkQuality` 为 `false`（Line 143 订阅值）。
   - `quality` 面板 case 返回 `null` (Line 384-390)，质量页不渲染。
   - 导航栏/菜单中质量入口依赖 `DockNav` 等过滤，本文源码未展示但其渲染门控已生效。

## Connections

[[android-interval-default-schema-migration]]: 关联背景，说明 schema 迁移机制的历史沿革（v0-v7）。
[[quality-detail-key-contract]]: 关联质量检测数据的具体协议和展示细节。
[[tablet-layout-alignment-audit]]: 关联平板布局与桌面布局的对齐审计，涉及 `enable_network_quality` 对布局的影响。

## Known Issues

- `android/frontend/src/settings/constants.ts` 中 `DEFAULT_CONFIG.enableNetworkQuality` 仍为 `true` (Line 38)，与后端默认 `false` 不一致；若前端 Store 初始化未正确触发 IPC 加载，可能出现瞬时错误态，但 `get_config` 命令 (config_state.rs Line 453-466) 在启动时即加载并广播，风险低。
- 本次提供源码未包含安卓移动端 `App.tsx`，旧文章描述的移动端 Tab 动态切换与胶囊逻辑无法从当前代码核实，需后续补全。
- 桌面端 `App.tsx` 仅在 `quality` case 返回 `null`，若用户手动从导航强制激活 `quality` 面板（如深链接），可能白屏；但正常导航过滤应已屏蔽。