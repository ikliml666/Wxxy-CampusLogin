---
title: frontend-account-module
type: module
source_files:
  - android/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/account/ChangePasswordDialog.tsx
  - android/frontend/src/account/DeviceAliasDialog.tsx
  - android/frontend/src/account/SelfOnlineRecords.tsx
  - android/frontend/src/account/SelfOnlineStack.tsx
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/selfServiceTypes.ts
  - android/frontend/src/account/types.ts
  - android/frontend/src/account/validators.ts
tags: [account, ui, self-service, validation, biometric]
---

## Overview
`frontend-account-module` 负责 Android/Tauri 应用中的用户账户管理、自助服务面板及在线状态展示。核心功能包括：多账号切换与管理（增删改查）、登录凭据（用户名/密码）的安全存储与编辑、运营商绑定（ISP Binding）、校园网密码修改、上网记录查询与可视化，以及在线设备的堆叠式交互展示。

该模块采用 Zustand 进行状态管理，利用自定义 Hooks (`useOperatorBind`, `useHelloGate`) 处理复杂的业务逻辑与安全验证。UI 层通过 `AnimatedCard` 和自定义堆叠组件 (`OnlineDeviceStack`) 提供流畅的移动端体验。

## Key Components

### AccountPanel (android/frontend/src/account/AccountPanel.tsx)
主面板组件，聚合了登录信息、自动化设置、账号管理和运营商绑定 UI。
- **Props**:
  - `accounts`: `AccountItem[]`，当前保存的账号列表。
  - `activeAccount`: `string`，当前激活账号 ID。
  - `onUpdateConfig`: `(partial: Partial<Config>) => void`，配置更新回调。
  - `onAddAccount`: `(name: string) => Promise<boolean>`，添加账号。
  - `onDeleteAccount`: `(id: string, displayName: string) => void`，删除账号。
  - `onSwitchAccount`: `(name: string) => Promise<void>`，切换账号。
  - `onRenameAccount`: `(accountId: string, displayName: string) => Promise<boolean>`，重命名账号。
- **State & Logic**:
  - **密码输入防抖与草稿**: 使用 `passwordDraft` 和 `usernameDraft` 本地状态，仅在 `blur` 或 `Enter` 时提交到 Store，避免频繁触发级联渲染 (Lines 65-90)。
  - **密码清除**: `handleClearPassword` 调用 `saveConfigDirect` 并同步 `syncPasswordSaved` (Lines 97-108)。
  - **账号切换防重复**: `switchingAccount` 状态防止并发切换请求 (Lines 123-129)。
  - **账号改名**: `startRename`, `cancelRename`, `submitRename` 处理内联编辑，集成 `isValidDisplayName` 校验 (Lines 135-163)。
  - **运营商绑定集成**: 通过 `useOperatorBind` Hook 获取绑定状态和操作句柄，渲染绑定表单和弹窗 (Lines 169-177)。

### ChangePasswordDialog (android/frontend/src/account/ChangePasswordDialog.tsx)
修改校园网密码的模态框。
- **Props**:
  - `open`: `boolean`。
  - `onSave`: `(newPassword: string, oldPassword: string) => Promise<boolean>`。
  - `onCancel`: `() => void`。
- **Logic**:
  - **密码格式校验**: 正则 `/^[0-9A-Za-z]{6,11}$/` 限制新密码为 6-11 位字母数字 (Line 34)。
  - **确认匹配**: 检查 `confirmPassword === newPassword` (Line 50)。
  - **旧密码输入**: 默认隐藏旧密码输入框，点击 toggle 显示 (Lines 75-83)。

### DeviceAliasDialog (android/frontend/src/account/DeviceAliasDialog.tsx)
在线设备别名编辑模态框。
- **Props**:
  - `mac`: `string`，原始 12 位 hex MAC 地址。
  - `initialName`: `string`，初始别名。
  - `onSave`: `(name: string) => Promise<void>`。
  - `onCancel`: `() => void`。
- **Logic**:
  - **状态复位**: 依赖 `open` prop 变化重置内部状态，确保无副作用 (Lines 29-33)。

### SelfOnlineRecords (android/frontend/src/account/SelfOnlineRecords.tsx)
上网记录融合卡片，支持近期流水和账单明细两种模式。
- **Data Converters**:
  - `recentToSessionRows`: 将 `SelfHistoryRow[]` 转换为 `SessionRow[]` (Lines 36-48)。
  - `billToSessionRows`: 将 `SelfLogRow[]` 转换为 `SessionRow[]` (Lines 50-63)。
  - `groupByDay`: 按登录时间本地日期分组 (Lines 77-89)。
  - `aggregateStats`: 聚合统计时长、流量、费用 (Lines 71-76)。
- **Render Logic**:
  - 根据 `logRows` 是否为 null 决定显示“查询态”还是“近期态” (Lines 135-137)。
  - 查询态显示 8 项汇总芯片和详细上下行/NAS 信息 (Lines 168-198)。
  - 近期态显示 4 项聚合芯片和终端类型/MAC 信息 (Lines 202-236)。

### OnlineDeviceStack / OnlineDeviceStackH (android/frontend/src/account/SelfOnlineStack.tsx)
实现封面流 (Coverflow) 效果的堆叠卡片容器，支持垂直 (手机) 和水平 (平板) 变体。
- **Constants**:
  - `PEEK_TOP/BOTTOM`: 露出边高度 (44px) (Lines 18-19)。
  - `DRAG_THRESHOLD`: 拖拽翻页阈值 (60px) (Line 20)。
  - `V_SIDE_SCALE`: 竖排侧卡缩放比例 (0.75) (Line 28)。
  - `SIDE_SCALE/SIDE_SCALE_2`: 横排侧卡缩放比例 (0.6 / 0.45) (Lines 33-34)。
- **Props**:
  - `cards`: `React.ReactNode[]`。
  - `variant`: `'coverflow' | 'narrow'`。
- **Interaction**:
  - **Pointer Events**: 处理 `onPointerDown`, `onPointerMove`, `onPointerUp` 实现拖拽 (Lines 92-118 for Vertical, Lines 186-212 for Horizontal)。
  - **Click Split**: 点击区域中线划分，左/上为上一张，右/下为下一张 (Lines 120-130 for Vertical, Lines 214-224 for Horizontal)。
  - **Transform Logic**: 基于 `rel` 索引计算 `top`/`x` 偏移和 `scale`，实现整副牌连续滚动效果 (Lines 132-160 for Vertical, Lines 226-280 for Horizontal)。
- **OnlineDeviceStackCard**: 单张卡片组件，展示 IP, MAC, 流量, 时长及设备别名编辑入口 (Lines 296-356)。

### selfServiceState (android/frontend/src/account/selfServiceState.ts)
生物识别验证门控逻辑。
- **Hooks**:
  - `useHelloGate`: 绑定操作前的 Windows Hello 验证。TTL 为 570s (Lines 72-93)。
  - `useSelfServiceVerify`: 自助服务面板会话门控。支持每次操作验证 (Lines 109-128)。
- **Helpers**:
  - `biometricFailMessage`: 解析生物识别错误码并翻译 (Lines 42-49)。
  - `gateFresh`: 检查验证时间戳是否在 TTL 内且时钟未回拨 (Lines 65-67)。

### validators (android/frontend/src/account/validators.ts)
账号显示名校验工具。
- **Constants**:
  - `DISPLAY_NAME_MAX_LEN`: 32 (Lines 14)。
  - `CONTROL_CHARS_RE`: 匹配 C0/C1 控制字符 (Lines 18)。
- **Functions**:
  - `isValidDisplayName`: 校验 trim 后的字符串长度和非法字符 (Lines 22-24)。

### Types (android/frontend/src/account/types.ts & selfServiceTypes.ts)
- **AccountResult/SwitchAccountResult/DeleteAccountResult/SaveAccountResult**: 账号操作的标准返回结构 (types.ts Lines 9-27)。
- **SelfHistoryRow**: 元组类型，包含上线时间、IP、MAC、时长、流量等 (selfServiceTypes.ts Line 9)。
- **SelfLogRow**: 接口类型，包含详细的上下行流量和 NAS 信息 (selfServiceTypes.ts Lines 13-23)。
- **SelfLogSummary**: 汇总数据接口 (selfServiceTypes.ts Lines 26-34)。
- **SessionRow**: 统一会话行接口，用于合并近期和账单数据 (SelfOnlineRecords.tsx Lines 23-34)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     AccountPanel                             │
│  (android/frontend/src/account/AccountPanel.tsx)             │
│                                                              │
│  ┌──────────────────┐   ┌──────────────────┐                │
│  │ Login Info Card  │   │ Auto Switch Card │                │
│  │ (User/Pwd/ISP)   │   │ (Toggles)        │                │
│  └────────┬─────────┘   └────────┬─────────┘                │
│           │                      │                           │
│  ┌────────▼──────────────────────▼─────────┐                │
│  │          Account Management             │                │
│  │  (List, Add, Rename, Switch, Delete)    │                │
│  │  Uses: OnlineDeviceStackH (Narrow)      │                │
│  └────────────────────────┬────────────────┘                │
│                           │                                  │
│  ┌────────────────────────▼────────────────┐                │
│  │         Operator Binding                │                │
│  │  (Form + Dialog for Status Query)       │                │
│  └────────────────────────┬────────────────┘                │
│                           │                                  │
│  ┌────────────────────────▼────────────────┐                │
│  │       ChangePasswordDialog              │                │
│  └─────────────────────────────────────────┘                │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **Configuration Loading**:
    *   `AccountPanel` subscribes to `useConfigStore` using `useShallow` to minimize re-renders (AccountPanel.tsx Line 54).
    *   User inputs (Username, Password Draft) are held in local state until blur/enter, then dispatched via `onUpdateConfig` (AccountPanel.tsx Lines 85-90).

2.  **Account Management**:
    *   **Add/Rename**: Input is validated against `isValidDisplayName` (validators.ts). If valid, `onRenameAccount` or `onAddAccount` is called. Success updates the `accounts` list passed from parent.
    *   **Switch/Delete**: `handleSwitchAccount` sets a busy state to prevent race conditions. Calls parent handler. On success, `activeAccount` prop updates, triggering UI refresh.

3.  **Operator Binding**:
    *   `useOperatorBind` hook manages the binding form state (`bindOp`, `bindPhone`, `bindSms`).
    *   `handleBindOperator` triggers the binding process.
    *   `fetchBindStatus` queries the backend. Results are displayed in a `Dialog`.
    *   Revealing passwords requires `useHelloGate` verification (selfServiceState.ts).

4.  **Online Records**:
    *   `SelfOnlineRecords` receives `history` (recent) and `logRows` (queried).
    *   `useMemo` converts raw rows into normalized `SessionRow[]` using `recentToSessionRows` or `billToSessionRows`.
    *   Rows are grouped by day and aggregated for stats display.

## Connections

*   [[settings]]: `AccountPanel` imports `Config` and `AccountItem` types from settings. It uses `ISP_OPTIONS` from settings constants.
*   [[hooks-useConfigStore]]: Central store for configuration. `AccountPanel` uses shallow subscription to avoid unnecessary renders.
*   [[hooks-useLogToastStore]]: Used for displaying error/success messages across account operations.
*   [[shared-MascotFigure]]: Used for empty states in account management.
*   [[components-ui-card]]: Base UI components used for structuring panels.
*   [[components-ui-dialog]]: Used for `ChangePasswordDialog`, `DeviceAliasDialog`, and Bind Status query.

## Known Issues

1.  **Password Input Wipe Bug (P1-F5)**: Historically, if input paused >500ms, the store's `config-changed` event would overwrite the masked password field, truncating user input. Mitigated in `AccountPanel` by using `passwordDraft` and only committing on `blur` (AccountPanel.tsx Lines 65-68).
2.  **StrictMode Double Mount**: `mountedRef` is used to handle React StrictMode's double invocation of effects, ensuring async handlers don't discard state updates (AccountPanel.tsx Lines 59-62).
3.  **Biometric Gate TTL Mismatch**: The frontend gate TTL (570s) is aligned with the backend's 600s to prevent deadlocks where the frontend thinks it's verified but the backend rejects due to expiration (selfServiceState.ts Lines 61-62).
4.  **Touch Action Conflict**: The `OnlineDeviceStack` container uses `touchAction: 'none'` to capture gestures for card switching, which conflicts with WebView page scrolling. Users must scroll outside the stack area (SelfOnlineStack.tsx Line 15).