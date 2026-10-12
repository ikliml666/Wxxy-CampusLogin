---
title: 网络质量检测默认关闭（schema v3→v7），质量页由后台检测状态代替
type: decision
source_files:
  - android/src-tauri/src/config_state.rs
  - android/frontend/src/settings/constants.ts
  - tauri-app/frontend/src/App.tsx
  - android/frontend/src/App.tsx
tags: [决策, 质量检测, 默认值, 安卓, 双端同语义]
---

## 背景

`enable_network_quality` 是网络质量检测链路的总开关。旧版代码中该功能默认开启，但由于包含 12+ 外网目标探测且间隔较短（原 60s），是移动端主要功耗来源之一。因此决定将其默认值改为 `false`，并在前端 UI 层面提供替代状态展示方案。

## 决策

**核心决策**：将 `enable_network_quality` 的默认值从 `true` 改为 `false`。

**实施细节**：

1. **后端默认值变更**：
   - 在 `android/src-tauri/src/config_state.rs:185`，`Settings::default()` 实现中，`enable_network_quality` 初始化为 `false`。
   - 注释明确说明："2026-09-12 起默认关闭(省电:质量检测含 12+ 外网目标,启动即跑一轮数秒;质量页改由后台检测状态代替展示)"。

2. **Schema 迁移机制**：
   - 通过 `config_schema_version` 字段控制配置迁移（`config_state.rs:147-151`）。
   - 当前版本为 `7`（`config_state.rs:217`）。
   - 关键迁移步骤：
     - **v2→v3 (2026-09-12)**：将存量配置的 `enable_network_quality` 强制刷为 `false`（`config_state.rs:280`）。
     - **v3→v4**：新增 `background_check_idle_interval` 默认值。
     - **v4→v5**：调整 `campus_check_end_minutes` 默认值。
     - **v5→v6**：调整 `latency_test_interval` 从 60s 到 600s。
     - **v6→v7**：调整夜间切换和定时登录/注销的默认值。
   - 迁移逻辑位于 `migrate_legacy_defaults` 函数（`config_state.rs:257-305`），确保幂等性：用户主动修改后不再覆盖。

3. **前端常量同步**：
   - `android/frontend/src/settings/constants.ts:41`：`DEFAULT_CONFIG.enableNetworkQuality = true`。**注意**：此处前端默认值仍为 `true`，与后端不一致。这可能是为了前端初始渲染或特定场景下的行为差异，但实际生效取决于后端下发的配置。根据后端代码，新安装用户将获得 `false`。

4. **UI 行为变更**：
   - **桌面端 (tauri-app/frontend/src/App.tsx:193-198)**：当 `configEnableNetworkQuality !== false` 时渲染 `QualityPanel`，否则不渲染任何内容（返回 `null`）。
   - **安卓手机端 (android/frontend/src/App.tsx:110-113, 145-150)**：
     - 底栏 Tab 顺序动态调整：质量开启时第 4 位为 `quality`，关闭时为 `monitor`。
     - 顶栏胶囊：质量开启时显示 `NetworkQualityCapsule`，关闭时隐藏并可能显示后台检测状态（具体由 `MobileDashboard` 或其他组件处理，此处 App.tsx 仅控制胶囊显隐）。
     - `MonitorPanel` 在质量关闭时作为第 4 位 Tab 出现，提供后台检测状态的替代展示。

## 理由

1. **功耗优化**：质量检测链路涉及大量外网请求，频繁执行会显著增加电池消耗。默认关闭可延长设备续航。
2. **用户体验**：用户更关注连接状态而非详细的质量指标。后台检测（Background Check）提供更稳定的在线状态感知，足以满足大多数用户需求。
3. **向后兼容**：通过 Schema 迁移机制，确保旧版本用户的配置被正确更新为新默认值，同时尊重用户手动修改的设置。

## 备选方案

- **保持默认开启**：会导致高功耗，不符合移动端设计原则。
- **降低检测频率**：虽能缓解功耗，但仍会增加不必要的数据传输和电量消耗，且无法完全解决启动时的瞬时高负载问题。

## 影响与约束

1. **配置一致性**：
   - 后端 (`config_state.rs`) 和前端 (`constants.ts`) 的默认值可能存在差异（前端仍为 `true`）。实际运行时，后端下发的配置优先级更高。需确保前端在接收到后端配置后正确更新状态。
   - `config_schema_version` 必须与后端保持一致（当前为 `7`），否则可能导致迁移逻辑失效或重复迁移。

2. **UI 结构依赖**：
   - 安卓端的底栏 Tab 结构依赖于 `enable_network_quality` 开关。改变默认值会影响底栏的默认布局（`quality` vs `monitor`）。
   - 桌面端的质量页签在关闭时将完全消失，用户需重新开启才能访问详细质量数据。

3. **迁移风险**：
   - 迁移逻辑 (`migrate_legacy_defaults`) 必须严格保证幂等性和正确性。任何错误都可能导致用户配置丢失或行为异常。
   - 特别是 v2→v3 的迁移，将存量配置强制刷为 `false`，需确保这是预期的行为。

4. **前端默认值滞后**：
   - `constants.ts:41` 中的 `enableNetworkQuality: true` 可能与后端默认值 `false` 不同步。这可能导致首次启动时前端 UI 显示不一致，直到后端配置加载完成。建议检查前端是否会在后端配置加载前基于 `DEFAULT_CONFIG` 进行渲染。

## Connections

[[android-interval-default-schema-migration]]、[[quality-detail-key-contract]]、[[tablet-layout-alignment-audit]]