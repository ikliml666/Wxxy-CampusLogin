---
title: 网络质量检测默认关闭（schema v3）及多端 UI 适配
type: decision
source_files:
  - android/src-tauri/src/config_state.rs
  - android/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/App.tsx
tags: [决策, 质量检测, 默认值, 安卓, 双端同语义]
---

## Overview

本决策记录 `enable_network_quality` 字段在 Android 端的默认值变更及其对前端 UI 布局、状态展示的影响。

核心决策：2026-09-12 起，`enable_network_quality` **默认关闭 (false)**。
理由：质量检测链路包含 12+ 外网目标探测，启动即跑一轮耗时数秒，功耗较高。关闭后，质量页由后台检测状态代替展示，提升省电体验。

该决策通过后端 `config_schema_version` 迁移机制（v2→v3）落地，并同步调整了桌面端与移动端的前端交互逻辑。

## Key Components

### 后端配置结构 (`android/src-tauri/src/config_state.rs`)

**`Settings` Struct (Line 48-156)**
定义全量配置结构，关键字段如下：
- `enable_network_quality: bool` (Line 107): 网络质量检测总开关。
- `enable_latency_test: bool` (Line 105): 定时质量测试开关（默认 false）。
- `skip_ttfb_in_latency: bool` (Line 108): 延迟测试中跳过 TTFB（默认 true）。
- `skip_content_in_latency: bool` (Line 109): 延迟测试中跳过内容加载（默认 true）。
- `config_schema_version: u32` (Line 155): 配置结构版本，当前值为 7。用于触发一次性迁移。

**`Default` Impl (Line 158-220)**
- `enable_network_quality`: 默认值设为 `false` (Line 199)。注释说明 2026-09-12 起默认关闭。
- `enable_latency_test`: 默认值设为 `false` (Line 197)。
- `skip_ttfb_in_latency`: 默认值设为 `true` (Line 200)。
- `skip_content_in_latency`: 默认值设为 `true` (Line 201)。

**`migrate_legacy_defaults` Function (Line 284-343)**
执行旧配置向新默认值的迁移。
- **v2→v3 迁移 (Line 298-299)**: 如果 `config_schema_version < 3`，将 `enable_network_quality` 强制刷为 `false`。这是本决策的核心迁移逻辑。

**`masked_for_display` Function (Line 374-381)**
返回掩码后的配置 JSON，确保敏感信息不出站。

### 前端常量定义 (`android/frontend/src/settings/constants.ts`)

**`DEFAULT_CONFIG` Object (Line 4-56)**
- `enableNetworkQuality: true` (Line 33): **注意**：此处前端默认值仍为 `true`。
  *分析*：前端 `constants.ts` 中的 `DEFAULT_CONFIG` 主要用于初始化 Store 或提供 UI 初始态参考。实际生效的配置来自后端 IPC 加载。后端加载时会经过 `load_from` -> `migrate_legacy_defaults`，最终 `enableNetworkQuality` 会根据后端迁移结果更新到前端 Store。前端组件逻辑需兼容后端下发的真实值。

### 桌面端应用入口 (`tauri-app/frontend/src/App.tsx`)

**`AppInner` Component (Line 152-430)**
- **Quality Panel 渲染门控 (Line 318-324)**:
  ```tsx
  case 'quality':
    panelContent = configEnableNetworkQuality !== false ? (
      <QualityPanel ... />
    ) : null
  break
  ```
  当 `configEnableNetworkQuality` 为 `false` 时，不渲染 `QualityPanel`，返回 `null`。这导致桌面端侧边栏/菜单中“网络质量”选项可能不可见或点击无内容，具体取决于导航组件的实现（通常配合过滤逻辑）。

- **Config 订阅 (Line 186)**:
  ```tsx
  const configEnableNetworkQuality = useConfigStore((s) => s.config.enableNetworkQuality)
  ```
  App 外壳层仅订阅必要的配置字段，避免不必要的重渲染。

### 移动端应用入口 (`android/frontend/src/App.tsx`)

**`AppInner` Component (Line 30-250)**
- **Tab 动态切换逻辑 (Line 53-56)**:
  ```tsx
  const tabOrder: MobileTab[] = [
    'dashboard',
    'account',
    'selfservice',
    configEnableNetworkQuality !== false ? 'quality' : 'monitor',
    'more',
  ]
  ```
  底栏第 4 位根据 `configEnableNetworkQuality` 动态决定显示 `'quality'` 还是 `'monitor'`。
  - 开启：显示 "网络质量"。
  - 关闭（默认）：显示 "后台检测/网络状态检测" (`MonitorPanel`)。

- **Panel 渲染逻辑 (Line 110-136)**:
  ```tsx
  case 'quality':
    panelContent = configEnableNetworkQuality !== false ? (
      <QualityPanel ... />
    ) : null
  break
  case 'monitor':
    panelContent = (
      <MonitorPanel ... />
    )
  break
  ```
  - 当 Tab 为 `quality` 但后端配置关闭时，渲染 `null`（空页面）。
  - 当 Tab 为 `monitor` 时，始终渲染 `MonitorPanel`，用于展示后台检测状态。

- **配置就绪后的自动归位 (Line 97-103)**:
  ```tsx
  useEffect(() => {
    if (!configLoaded) return
    if (configEnableNetworkQuality !== false && tab === 'monitor') {
      handleTabChange('quality')
    }
  }, [configLoaded, configEnableNetworkQuality, tab])
  ```
  如果配置加载完成且质量开关开启，而当前 Tab 是 `monitor`（例如用户之前手动切过），则自动跳回 `quality`。反之，如果质量关闭，保留在 `monitor` 或允许用户停留在 `monitor`。

- **顶栏胶囊展示 (Line 78-82)**:
  ```tsx
  {configEnableNetworkQuality !== false && (
    <button ... onClick={() => handleTabChange('quality')} ...>
      <NetworkQualityCapsule networkQuality={networkQuality} />
    </button>
  )}
  ```
  仅在质量开启时显示网络质量胶囊。关闭时，顶栏左侧仅显示状态点（在线/离线/加载中）和运营商徽标，不再误导显示“未知”。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                      Backend (Rust)                         │
│  config_state.rs                                            │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Settings::default()                                   │  │
│  │ enable_network_quality = false (Line 199)             │  │
│  │ config_schema_version = 7                             │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ migrate_legacy_defaults()                             │  │
│  │ v2→v3: enable_network_quality = false                 │  │
│  └───────────────────────────────────────────────────────┘  │
└───────────────────────┬─────────────────────────────────────┘
                        │ IPC: get_config / config-changed
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                      Frontend (React)                       │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Zustand Store (useConfigStore)                        │  │
│  │ config.enableNetworkQuality                           │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Desktop App.tsx                                       │  │
│  │ Quality Panel: 条件渲染 (Line 318)                    │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Mobile App.tsx                                        │  │
│  │ BottomNav Tab Order: Dynamic Switch (Line 53)         │  │
│  │ Header Capsule: Conditional Render (Line 78)          │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **首次启动/升级**:
   - 后端 `load_from` 读取旧配置文件。
   - 若 `config_schema_version < 3`，调用 `migrate_legacy_defaults`。
   - 迁移逻辑将 `enable_network_quality` 设置为 `false`，并将版本号提升至 3（后续逐步升至 7）。
   - 保存修改后的配置到磁盘。
   - 后端通过 `emit_config_changed` 广播事件。

2. **前端接收**:
   - 前端 Store 接收到新的配置对象。
   - `configEnableNetworkQuality` 变为 `false`。

3. **UI 响应**:
   - **移动端**:
     - `tabOrder` 数组第 4 项变为 `'monitor'`。
     - 底栏显示 "后台检测" 图标。
     - 顶栏隐藏 `NetworkQualityCapsule`。
     - 若用户之前处于 `quality` 页，由于 `tabOrder` 变化，需确保 Tab 状态合法；代码中有逻辑处理 `tab === 'monitor'` 时的状态保持。
   - **桌面端**:
     - `QualityPanel` 渲染条件不满足，返回 `null`。
     - 导航菜单中相关入口应被过滤或禁用（依赖 `DockNav` 或 `RightPanel` 的过滤实现，源码中 `App.tsx` 仅控制内容渲染）。

## Connections

[[android-interval-default-schema-migration]]: 关联背景，说明 schema 迁移机制的历史沿革（v0-v7）。
[[quality-detail-key-contract]]: 关联质量检测数据的具体协议和展示细节。
[[tablet-layout-alignment-audit]]: 关联平板布局与桌面布局的对齐审计，涉及 `enable_network_quality` 对布局的影响。