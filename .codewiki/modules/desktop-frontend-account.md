---
title: desktop-frontend-account
type: module
source_files:
  - tauri-app/frontend/src/account/SelfOnlineRecords.tsx
  - tauri-app/frontend/src/account/SelfOnlineRecords.test.tsx
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/account/AccountPanel.helloGate.test.tsx
  - tauri-app/frontend/src/account/AccountPanel.rename.test.tsx
  - tauri-app/frontend/src/account/BindOperatorDialog.tsx
  - tauri-app/frontend/src/account/operatorChip.ts
  - tauri-app/frontend/src/account/index.ts
tags: [account, desktop, frontend, tauri, self-service]
---

## Overview

`desktop-frontend-account` 模块负责桌面端（Tauri）应用中的账号管理与自助服务功能。核心职责包括：
1.  **账号生命周期管理**：添加、编辑、删除、重命名账号，以及处理 Windows Hello 身份验证门控。
2.  **运营商绑定**：通过弹窗界面支持登录信息或手动填写凭据进行 ISP 绑定，并处理状态查询与明文揭示。
3.  **密码管理**：修改校园网自助服务密码及清除本地存储的登录密码。
4.  **在线设备监控**：以 Coverflow 堆叠卡片形式展示当前在线设备，支持别名编辑与离线操作。
5.  **上网记录融合**：统一渲染“近期流水”与“账单明细查询”两种模式的数据，提供聚合统计与详细列表。

该模块高度依赖 `useConfigStore` 获取全局配置，并通过 Tauri API (`tauriApiWithRetry`) 与后端交互。

## Key Components

### AccountPanel
**文件**: `tauri-app/frontend/src/account/AccountPanel.tsx`
**职责**: 账号管理主面板容器，整合所有子组件与状态逻辑。

#### Props & Interfaces
*   `AccountPanelProps` (行40-50):
    *   `accounts`: 账号列表数据。
    *   `activeAccount`: 当前激活的账号 ID。
    *   `onRenameAccount`: 回调函数 `(accountId, displayName) => Promise<boolean>`，用于执行改名操作。
    *   其他隐式依赖 props 来自 `useConfigStore` 和上下文。

#### State Management
*   `config`: 从 `useConfigStore` 订阅，浅比较。
*   `passwordSaved`: 布尔值，标记密码是否已保存。
*   `mountedRef`: Ref 对象，用于 React StrictMode 兼容，防止异步 setState 在组件卸载后触发。
*   `switchingAccount`: 布尔值，防止重复提交切换账号请求。
*   `renamingId`, `renameDraft`: 内联改名状态。
*   `loginInfoMode`: `'create' | 'edit'`，控制 `LoginInfoDialog` 模式。
*   `bindOpen`: 布尔值，控制 `BindOperatorDialog` 显示。
*   `chpwdOpen`: 布尔值，控制 `ChangePasswordDialog` 显示。

#### Methods & Logic
*   `handleSwitchAccount` (行93-97):
    *   检查 `switchingAccount` 状态防抖。
    *   忽略当前激活账号的点击。
*   `startRename` (行104-107):
    *   初始化 `renamingId` 和 `renameDraft` 为当前账号名。
*   `submitRename` (行114-130):
    *   校验 `isValidDisplayName` (行118)。
    *   调用 `onRenameAccount(id, name)`。
    *   成功则调用 `cancelRename`；失败保留草稿供重试。
*   `handleSaveLoginInfo` (行168-207):
    *   **Create 模式**: 调用 `tauriApiWithRetry.createAccount`，更新 store 配置，刷新列表 (`refreshAccounts`)。
    *   **Edit 模式**: 先 `saveConfigDirect` (若密码为空则语义为保留旧密码)，再调用 `onAddAccount(activeAccount)` (即 `save_current_as_account`)。
*   `handleReveal` (行515-530):
    *   触发 Windows Hello 验证后，调用 `revealOperatorCredential` 查看绑定状态的明文凭据。
*   `cancelRename` (行109-112):
    *   重置 `renamingId` 和 `renameDraft`。

### BindOperatorDialog
**文件**: `tauri-app/frontend/src/account/BindOperatorDialog.tsx`
**职责**: 运营商绑定弹窗 UI，负责凭据源选择与表单渲染。

#### Types
*   `BindCredSource = 'login' | 'manual'` (行28).

#### Methods & Logic
*   Lifecycle Reset (行84-91):
    *   当弹窗关闭时，重置 `credSource` 为 `'login'` 并清空手动输入草稿。
*   UI Components:
    *   **凭据分段切换器** (行102-147): Grid 布局切换 'login'/'manual'。Manual 模式显示手动账号/密码输入框。
    *   **ISP 选择器** (行150-160): 过滤掉 `__default__` 选项。
    *   **手机号/SMS密码输入** (行162-184).
    *   **按钮组**: `onBind` 触发绑定，`onQueryStatus` 触发状态查询。

### ChangePasswordDialog
**文件**: `tauri-app/frontend/src/account/ChangePasswordDialog.tsx`
**职责**: 修改校园网自助服务密码弹窗。

*(Note: Source code not provided for line verification in this request)*

### DeviceAliasDialog
**文件**: `tauri-app/frontend/src/account/DeviceAliasDialog.tsx`
**职责**: 在线设备别名编辑弹窗。

*(Note: Source code not provided for line verification in this request)*

### LoginInfoDialog
**文件**: `tauri-app/frontend/src/account/LoginInfoDialog.tsx`
**职责**: 添加或编辑登录信息的弹窗。

*(Note: Source code not provided for line verification in this request)*

### SelfOnlineRecords
**文件**: `tauri-app/frontend/src/account/SelfOnlineRecords.tsx`
**职责**: 核心组件，实现“上网记录融合卡”。支持**近期流水态**（Dashboard 自动带回数据）和**日期查询态**（用户手动查询账单明细）。通过归一化的 `SessionRow` 结构统一渲染双源数据，采用日分组列表替代传统宽表格以适配桌面端布局。

#### Types & Exports
*   `SelfHistoryRow` (行24): Dashboard 历史数组元素类型。
*   `SelfLogRow` (行24): 账单明细对象数组元素类型。
*   `SelfLogSummary` (行24): 账单汇总对象类型。
*   `SessionRow` (行35-55): 归一化会话行结构。
    *   `key`: 唯一标识 (`recent-${idx}` 或 `bill-${idx}`)。
    *   `mode`: `'recent'` | `'bill'`，决定渲染列。
    *   通用字段: `loginTime`, `logoutTime` (0表示在线), `timeMin`, `flowMb`, `costMoney`, `ip`.
    *   近期特有: `mac`, `terminalType`, `payStyle` (1时长/2流量/3包月).
    *   账单特有: `intlUp/Down`, `cnUp/Down`, `nasIp`, `nasPort`.

#### Utility Functions
*   `fmt2` (行26): 保留两位小数，默认0。
*   `fmtMoney` (行28-32): 金额格式化，null/空/非数值返回 '-'，否则原样字符串。
*   `recentToSessionRows(history: SelfHistoryRow[])` (行57): 将 Dashboard 历史数组转为会话行。使用 `Number(row[...]) || 0` 处理时间戳防止 NaN (行61-62)。
*   `billToSessionRows(rows: SelfLogRow[])` (行73): 将账单明细对象数组转为会话行。使用 `Number.isFinite` 校验 flow 和 costMoney，非法值转为 null (行80-81)。
*   `aggregateStats(rows: SessionRow[])` (行100): 计算聚合统计。
*   `groupByDay(rows: SessionRow[])` (行114): 按登录日期分组，跳过 loginTime 为 0 的记录 (行118)。

#### Sub-components
*   `SessionTimeRange` (行131): 渲染时间区间。若未注销 (`logoutTime` falsy)，显示绿色 "在线" 徽标；跨日注销补全日期前缀 (行142)。
*   `RecentSessionRow` (行155): 近期态行。首行显示时间、时长、流量、金额、计费方式；次行显示 IP、MAC (格式化)、终端类型。
*   `BillSessionRow` (行182): 账单态行。首行显示时间、时长、流量、金额；次行显示 IP；底部分隔线后显示上下行流量拆分及 NAS IP/端口。缺失值显示 '-'。
*   `SessionDayGroups` (行205): 基于 `groupByDay` 进行日分组渲染，每日标题带分割线。

#### Main Component Logic (`SelfOnlineRecords`, 行239-377)
*   **Props**: `configLoaded`, `hasCred`, `history`, `logStart/End`, `logQuerying`, `logRows`, `logSummary`, 回调函数。
*   **模式判断** (行248): `queried = logRows !== null`。
*   **数据归一化** (行249-252): 根据 `queried` 选择调用 `billToSessionRows` 或 `recentToSessionRows`。
*   **统计聚合** (行253): `recentStats` 始终基于当前 `sessions` 计算。
*   **UI 分支**:
    1.  **配置加载中** (行311): 显示加载 Spinner。
    2.  **无凭据** (行315): 提示需要凭据。
    3.  **查询态** (行317):
        *   显示 8 枚汇总芯片 (行321-339): INTERNETUP/DOWN, CHINANETUP/DOWN, FLOW, TIME, COSTMONEY, COU。计数显示整数，其他 `fmt2`。
        *   渲染 `BillSessionRow` 列表 (行342)。
        *   溢出提示: `logRows.length >= 500` 时显示警告 (行346-348)。
    4.  **近期态** (行350):
        *   显示 4 枚聚合芯片 (行353-365): TIME, FLOW, MONEY, COUNT。
        *   渲染 `RecentSessionRow` 列表 (行366)。
    5.  **空闲/空态** (行368): `history === null` 显示 Idle 提示，`history === []` 显示 Empty 提示。
*   **底部说明** (行373): 显示单位说明。

### SelfOnlineCards
**文件**: `tauri-app/frontend/src/account/SelfOnlineCards.tsx`
**职责**: 在线设备卡片展示组件及 Coverflow 堆叠交互容器。

*(Note: Source code not provided for line verification in this request)*

### operatorChip
**文件**: `tauri-app/frontend/src/account/operatorChip.ts`
**职责**: 运营商品牌色样式生成器。
*   `operatorChipClass(operator)`: 匹配 `@cmcc` (天蓝), `@telecom` (蓝), `@unicom` (橙)，其他中性色。

### index
**文件**: `tauri-app/frontend/src/account/index.ts`
**职责**: 模块公共入口。
*   **导出**: `AccountPanel`, `ChangePasswordDialog`, `useAccount`, 以及 `types` 下所有导出。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                        App (External)                        │
└───────────────────────┬──────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────────┐
│                   AccountPanel.tsx                           │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  useConfigStore (accounts, config, activeAccount)      │  │
│  ├────────────────────────────────────────────────────────┤  │
│  │  Account List Renderer                                 │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │ OnlineDeviceCardStack (Coverflow/Narrow)         │  │  │
│  │  │  ┌─────────────┐ ┌─────────────┐ ┌────────────┐ │  │  │
│  │  │  │ Card 1      │ │ Card 2      │ │ Card N     │ │  │  │
│  │  │  │ (onRename)  │ │ (onRename)  │ │ (onRename) │ │  │  │
│  │  │  └─────────────┘ └─────────────┘ └────────────┘ │  │  │
│  │  └──────────────────────────────────────────────────┘  │  │
│  ├────────────────────────────────────────────────────────┤  │
│  │  Dialogs / Modals                                      │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────┐  │  │
│  │  │LoginInfoDlg │ │BindOperator │ │ChangePasswordDlg│  │  │
│  │  │(create/edit)│ │(Hello Gate) │ │                 │  │  │
│  │  └─────────────┘ └─────────────┘ └─────────────────┘  │  │
│  │  ┌─────────────┐ ┌─────────────┐                      │  │
│  │  │DeviceAlias  │ │SelfOnlineRec│                      │  │
│  │  │(Rename Alias)│ │(Records/Cards)│                    │  │
│  │  └─────────────┘ └─────────────┘                      │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

*   **AccountPanel** 是顶层协调者，持有所有对话框的状态。
*   **OnlineDeviceCardStack** 负责可视化展示在线设备，支持拖拽交互。
*   **SelfOnlineRecords** 独立于卡片堆栈，专注于历史记录的数据融合与展示。
*   **Windows Hello Gate** 逻辑封装在 `BindOperatorDialog` 及其关联 Hook 中，确保敏感操作前的身份验证。

## Data Flow

1.  **账号列表数据流**:
    *   `useConfigStore` -> `AccountPanel` -> `accounts` array.
    *   `AccountPanel` 遍历 `accounts` -> 渲染 `OnlineDeviceCard` (在 `OnlineDeviceCardStack` 内).

2.  **改名流程**:
    *   User Click Rename -> `AccountPanel.startRename(item)` -> UI 进入内联编辑态.
    *   User Input -> `submitRename()` -> 校验 `isValidDisplayName` (行118) -> 调用 prop `onRenameAccount(id, name)`.
    *   Success -> `refreshAccounts()` (调用 `tauriApiWithRetry.listAccounts`) -> 更新 store `setAccounts`.

3.  **登录信息保存流程**:
    *   **Create**: `AccountPanel.handleSaveLoginInfo` -> `tauriApiWithRetry.createAccount` -> 更新 store config/activeAccount -> `refreshAccounts`.
    *   **Edit**: `AccountPanel.handleSaveLoginInfo` -> `store.saveConfigDirect(partial)` (若密码为空则保留旧密码) -> `onAddAccount(activeAccount)` (即 `save_current_as_account`).

4.  **运营商绑定流程**:
    *   `AccountPanel` 调用 `useOperatorBind` hook.
    *   Hook 内部处理 `verifyWindowsIdentity` (Hello Gate).
    *   UI 层 (`BindOperatorDialog`) 通过 `onBind` 触发 `handleBindOperator`.
    *   查询状态通过 `fetchBindStatus` 触发，结果在独立 `Dialog` 中渲染.
    *   揭示明文通过 `handleReveal` 触发 `revealOperatorCredential`.

5.  **上网记录数据流**:
    *   **输入**:
        *   `history: SelfHistoryRow[] | null` (Tuple Array: `[loginMs, logoutMs, ip, mac, time, flow, payStyle, cost, ..., terminal]`).
        *   `logRows: SelfLogRow[] | null` (Object Array: `{ loginTime, logoutTime, time, flow, costMoney, internetUpFlow, ... }`).
        *   `logSummary: SelfLogSummary | null` (Object: `{ INTERNETUPFLOW, ..., COU }`).
    *   **中间态**: `SessionRow[]` (标准化对象数组，包含 `mode` 标记).
        *   `recentToSessionRows` / `billToSessionRows` 转换.
    *   **输出**: React Node (DOM 结构，含 Chip Grid 和 List Items).
        *   `aggregateStats(sessions)` -> 生成 `RecordStats`.
        *   `groupByDay(sessions)` -> 生成分组数据.
        *   `SessionDayGroups` -> 遍历 `rows` -> 根据 `row.mode` 渲染 `RecentSessionRow` 或 `BillSessionRow`.

## Connections

*   [[desktop-frontend-config]]: `AccountPanel` 使用 `useConfigStore` 读取和写入账号配置。
*   [[desktop-frontend-ui]]: 基础 UI 组件如 `Label`, `Input`, `Button`, `Card`, `AnimatedCard`, `CardIcon` 被广泛使用。
*   [[desktop-backend-api]]: 通过 `tauriApiWithRetry` 调用后端接口，如 `listAccounts`, `createAccount`, `save_current_as_account`, `revealOperatorCredential` 等。
*   [[desktop-auth-hello]]: `BindOperatorDialog` 及相关 Hook 集成 Windows Hello 验证逻辑。

## Known Issues

1.  **数据量限制**: 当 `logRows.length >= 500` 时，显式显示溢出警告文本 (行346-348)，暗示后端或前端可能有分页/限制策略，但未实现自动分页。
2.  **StrictMode 兼容**: `AccountPanel` 使用 `mountedRef` 处理 React StrictMode 下的双重挂载/卸载周期，需确保所有异步回调都检查此 Ref。
3.  **密码保存语义**: 后端“空密码”语义为“保留旧密码”，前端 `saveConfigDirect` 在编辑模式下若未输入新密码，不发送 `password` 字段。需确保 UI 提示清晰，避免用户误解。
4.  **在线设备堆叠性能**: `OnlineDeviceCardStack` 使用 CSS transforms 进行定位和缩放，在设备数量较多时可能影响性能，目前限制最多展示5张可见卡。
5.  **内联改名边界**: 空字符串或纯空格视为无效，阻止提交并 Toast 提示。Esc 键取消编辑，恢复原状。需确保键盘事件处理正确。