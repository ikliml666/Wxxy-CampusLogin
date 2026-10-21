---
title: android-frontend-account
type: module
source_files:
  - android/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/account/BindOperatorDialog.tsx
  - android/frontend/src/account/ChangePasswordDialog.tsx
  - android/frontend/src/account/DeviceAliasDialog.tsx
  - android/frontend/src/account/index.ts
  - android/frontend/src/account/LoginInfoDialog.tsx
  - android/frontend/src/account/operatorChip.ts
  - android/frontend/src/account/SelfOnlineRecords.tsx
  - android/frontend/src/account/SelfOnlineStack.tsx
  - android/frontend/src/account/SelfServicePanel.tsx
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/selfServiceTypes.ts
  - android/frontend/src/account/types.ts
  - android/frontend/src/account/useAccount.ts
  - android/frontend/src/account/useOperatorBind.ts
  - android/frontend/src/account/validators.ts
tags: [account, self-service, operator-bind, biometric-auth, tauri]
---

## Overview
本模块 `android-frontend-account` 负责 Android 端账号面板与自助服务的核心 UI 与逻辑。它涵盖了账号管理（增删改查、切换）、运营商绑定流程、密码修改、在线设备堆叠展示以及上网记录查询等功能。模块采用 React 组件树结构，通过 Tauri API 与后端通信，并利用自定义 Hooks (`useAccount`, `useOperatorBind`) 封装复杂的状态同步与并发控制逻辑。

## Key Components

### `AccountPanel.tsx` (行 1-486)
**职责**：账号管理页的主容器组件。渲染账号列表、在线设备卡片、自动化设置开关，并承载登录信息编辑、运营商绑定、修改密码三个核心弹窗。

*   **Props 接口** (行 32-41):
    *   `accounts`: 账号列表数据。
    *   `activeAccount`: 当前激活的账号 ID。
    *   `onUpdateConfig`: 配置更新回调。
    *   `onAddAccount`: 添加账号回调。
    *   `onDeleteAccount`: 删除账号回调。
    *   `onSwitchAccount`: 切换账号回调。
    *   `onRenameAccount`: 重命名账号回调。
*   **状态管理**:
    *   `useConfigStore` 订阅 (行 59-63): 获取 `config` (浅比较) 和 `passwordSaved`。
    *   `mountedRef` (行 64-71): 处理 StrictMode 下的异步副作用丢弃，防止无效状态更新。
    *   `switchingAccount` (行 103-108): 防止切换过程中的重复提交。
    *   `renamingId`, `renameDraft`, `renaming` (行 123-125, 134-149): 控制内联改名交互。
*   **关键方法**:
    *   `handleCardEdit` (行 112-119): 非激活卡先执行切换再打开编辑弹窗；激活卡直接打开。
    *   `handleSaveLoginInfo` (行 165-216):
        *   `create` 模式：调用 `tauriApiWithRetry.createAccount`，成功后刷新列表。
        *   `edit` 模式：先 `saveConfigDirect` 保存主配置，再 `saveCurrentAsAccount` 回写激活档案。
    *   `handleClearPassword` (行 90-100): 显式清除密码，调用 `saveConfigDirect({ password: '' }, true)` 并设置 `syncPasswordSaved(false)`。注意：后端空密码语义为“保留旧密码”，必须走此函数才能清除。
    *   `refreshAccounts` (行 153-160): 调用 `tauriApiWithRetry.listAccounts` 刷新本地 Store。
    *   `submitRename` (行 134-149): 执行账号重命名。
*   **UI 结构**:
    *   上半部分：账号管理卡，包含 `OnlineDeviceStack` (行 246-368)。
    *   下半部分：自动化设置卡，包含三个 Switch (自动登录启动/准备时/夜间切换) (行 401-455)。
    *   底部挂载：`LoginInfoDialog`, `BindOperatorDialog`, `ChangePasswordDialog` (行 458-482)。

### `BindOperatorDialog.tsx` (行 1-278)
**职责**：运营商绑定流程的 UI 载体。提供凭据分段选择（默认登录信息 vs 手动填写），并承载绑定状态查询弹窗。

*   **Props** (行 32-39):
    *   `bind`: 来自 `useOperatorBind` 的完整 Hook 返回对象。
    *   `onCredChange`: 用于同步手动凭据草稿给父级 Hook。
*   **关键实现**:
    *   `credMode` (行 51): `'login' | 'manual'`。
    *   手动凭据上报 (行 68-72): 当 `mode === 'manual'` 时，通过 `useEffect` 将草稿通过 `onCredChange` 上报。
    *   弹窗复位 (行 57-64): 打开弹窗时复位草稿。
    *   表单字段 (行 143-179): ISP 选择器 (`Select`)、手机号 (`Input`)、短信验证码密码 (`Input`)。
    *   “查询绑定状态”按钮 (行 182-193): 触发 `fetchBindStatus`，打开状态子弹窗。
    *   “绑定”按钮 (行 194-200): 触发 `handleBindOperator`。
    *   状态子弹窗 (行 206-275): 展示各运营商的绑定结果（掩码账号），支持点击“揭示”查看明文密码（调用 `handleReveal`）。

### `ChangePasswordDialog.tsx` (行 1-166)
**职责**：修改校园网登录密码的轻弹窗。

*   **校验规则** (行 28): 新密码正则 `/^[0-9A-Za-z]{6,11}$/`。
*   **状态** (行 32-38): `oldPassword` (可展开手动输入，默认由后端 resolve 已存密码)、`newPassword`, `confirmPassword`。
*   **提交流程** (行 54-72):
    *   校验新密码格式和确认一致性。
    *   调用 `onSave(newPassword, oldPassword)`，失败保留草稿，成功由父级处理后续同步。
*   **UI 细节** (行 84-108): 旧密码输入框默认隐藏，点击链接展开。

### `DeviceAliasDialog.tsx` (行 1-75)
**职责**：在线设备改名弹窗。编辑 `config.deviceAliases[mac]`。

*   **Props** (行 15-23):
    *   `mac`: 原始12位hex。
    *   `initialName`: 初始别名。
    *   `onSave`: 保存回调。
*   **逻辑** (行 36-47): 简单文本输入，限制最大长度30字符。保存时 `trim()` 后调用 `onSave`。
*   **防抖** (行 50, 68): `saving` 状态期间禁用提交按钮和弹窗关闭。
*   **错误吞没** (行 41-46): catch 块吞掉错误，因为错误提示已由调用方发出，保持弹窗打开以便重试。

### `SelfOnlineStack.tsx` (行 1-509)
**职责**：提供移动端（竖排）与平板/桌面端（水平）的设备在线状态堆叠视图，支持手势切换、外部聚焦及设备详情展示。

*   **常量定义** (行 14-37):
    *   `PEEK_TOP/BOTTOM = 44`: 竖排露出边高度。
    *   `DRAG_THRESHOLD = 60`: 拖拽翻页阈值。
    *   `V_SIDE_SCALE = 0.75`: 竖排侧卡缩放比例。
    *   `NARROW_MIN_W = 213`: 窄版变体最小宽度。
*   **`OnlineDeviceStack`** (行 46-222):
    *   **接口** (行 39-44): `cards: React.ReactNode[]`, `variant?: 'coverflow' | 'narrow'`, `focusIndex?: number`。
    *   **常挂载滚动** (行 91-93): 所有卡片 DOM 常驻，通过 `transform` 和 `opacity` 切换位置。
    *   **手势处理** (行 100-146): `onPointerDown/Move/Up`。单卡不可交互；点击以中线划分上半/下半对应上一张/下一张；拖拽超过阈值触发翻页。
    *   **量高策略** (行 70-85): `ResizeObserver` 监听容器内所有子元素最大高度。
    *   **姿势表** (行 165-178): 根据相对索引 `rel(i)` 计算 `top/scale/z/opacity`。
    *   **外部聚焦** (行 62-65): `focusIndex` 变化时强制 `setIndex`。
*   **`OnlineDeviceStackCard`** (行 242-326):
    *   **接口** (行 225-240): `loginTime`, `ip`, `mac`, `flowMb`, `durationMin`, `terminalType`, `alias`, `onRename`, `onOffline` 等。
*   **`OnlineDeviceStackH`** (行 332-509):
    *   水平方向堆叠，逻辑与垂直版同构。拖拽轴为 X 轴 (`dragX`)，支持 `coverflow` 和 `narrow` 两种变体 (行 459-461)。

### `SelfServicePanel.tsx` (行 1-348)
**职责**：聚合在线设备管理、改名、踢下线、上网记录查询等功能，处理凭据验证与数据流。

*   **数据格式化** (行 39-68):
    *   `formatTerminalType`: 去除 "#" 前缀。
    *   `formatUseTimeMinutes` (行 51-54): 秒转分钟，使用 `Math.floor` 对齐原站行为。
    *   `formatFlowMb` (行 57-61): KB 总和转 MB，保留 3 位小数。
    *   `localDateStr` (行 65-68): 本地日期字符串生成。
*   **状态管理** (行 77-97):
    *   依赖 `useConfigStore` 获取 `configUser`, `passwordSaved`, `deviceAliases`。
    *   依赖 `useSelfServiceVerify` 进行生物识别验证门控 (行 94)。
    *   状态包括 `onlineList`, `history`, `logRows`, `confirmTarget`, `aliasTarget`。
*   **核心业务逻辑**:
    *   **自动刷新** (行 142-148): 配置加载且凭据就绪后，静默调用 `fetchDashboard`。
    *   `fetchDashboard` (行 112-136): 调用 `tauriApiWithRetry.querySelfDashboard`。失败时若为网络原因则静默降级，手动刷新才报错。
    *   `handleOffline` (行 150-175): 踢下线流程。先验证，再调用 `selfOfflineSession`，成功后从列表中过滤掉该 sessionId。
    *   `handleSaveAlias` (行 178-195): 保存设备别名至 `config.deviceAliases`。
    *   `fetchOnlineLog` (行 197-221): 查询上网记录，支持日期范围筛选。
*   **渲染逻辑** (行 224-348):
    *   根据 `formFactor` 决定渲染 `OnlineDeviceStackH` (平板) 或 `OnlineDeviceStack` (手机) (行 268-306)。
    *   集成 `ConfirmDialog` (踢下线确认) 和 `DeviceAliasDialog` (改名弹窗)。

### `selfServiceState.ts` (行 1-127)
**职责**：提供 Windows Hello 生物识别验证的门控逻辑。

*   **`biometricFailMessage`** (行 34-38): 根据错误码 `code` 映射中文 Toast 文案。
*   **`VERIFY_TTL_MS`** (行 54): 570_000 ms (570s)，对齐后端 600s 并留余量。
*   **`useHelloGate`** (行 65-87):
    *   **通用门**：用于绑定、查状态等操作。
    *   **时钟回拨防护** (行 58-62): `gateFresh` 检查 `elapsed >= 0`，防止时钟回拨导致前端永真而后端拒绝的死锁。
    *   **`ignoreToggle`** (行 66, 71): 查看明文密码时，即使总开关关闭也强制验证。
*   **`useSelfServiceVerify`** (行 105-127):
    *   **会话门**：仅限自助服务面板内使用。
    *   **配置联动** (行 110): 读取 `config.selfReverifyEachAction`，若为 true 则每次操作都验证。
    *   **重置** (行 100-102): `resetSelfSessionGate` 在面板卸载时清空验证时间戳。

### `selfServiceTypes.ts` (行 1-79)
**职责**：定义自助服务相关的 TypeScript 类型及共享格式化工具。

*   **类型定义**:
    *   `SelfHistoryRow` (行 5): 元组类型，包含上线/注销时间、IP、MAC、时长、流量、金额等。
    *   `SelfLogRow` (行 9-22): 上网记录账单行，包含上下行流量细分。
    *   `SelfLogSummary` (行 25-34): 汇总数据，字段名大写。
*   **工具函数**:
    *   `formatMac` (行 36-40): 清洗非 Hex 字符并配对连接。
    *   `formatEpoch` (行 42-47): 毫秒时间戳转本地 YYYY-MM-DD HH:mm:ss。
    *   `fmtMoney` (行 75-79): 保留原始精度，非数值返回 '-'。

### `types.ts` (行 1-34)
**职责**：定义账号操作结果类型。

*   **导出**: `AccountItem` (复用 settings), `AccountResult`, `SwitchAccountResult`, `DeleteAccountResult`, `SaveAccountResult`。
*   **关键点**: 所有 Result 均包含 `success`, `message`, `activeAccount`, `config`。

### `useAccount.ts` (行 1-126)
**职责**：封装账号列表的增、删、改、切换逻辑，处理并发去重与状态同步。

*   **并发控制** (行 11, 78-103): `switchInFlight` 变量确保多次切换请求合并为一次执行。
*   **状态更新**:
    *   `handleAddAccount` (行 37-55): 保存后更新 config 和 activeAccount，刷新列表。
    *   `handleDeleteAccount` (行 70-75): 删除后应用返回的 `activeAccount` 修复历史缺陷。
    *   `handleSwitchAccount` (行 87-90): 同样应用返回的 `activeAccount` 和 `config`，注意使用 `!== undefined` 判断而非真值判断，兼容空串返回值。
    *   `handleRenameAccount` (行 108-126): 仅改 displayName，不影响 id 和激活状态。

### `useOperatorBind.ts` (行 1-213)
**职责**：封装运营商绑定、状态查询、明文揭示、改密流程。

*   **凭据处理** (行 75-80): 支持默认凭据（来自登录信息）和 `credOverride`（手动填写），优先使用手动凭据。
*   **生物验证**:
    *   `ensureHelloVerified`: 普通操作验证门。
    *   `ensureRevealVerified` (行 124): 揭示明文密码时使用 `{ ignoreToggle: true }`，强制验证。
*   **核心 Handler**:
    *   `fetchBindStatus` (行 93-119): 查询三网绑定状态，成功后自动打开弹窗。
    *   `handleReveal` (行 125-149): 揭示指定运营商的短信密码，30 秒自动清除。
    *   `handleBindOperator` (行 151-181): 执行绑定，成功后清空短信验证码并刷新状态。
    *   `handleChangePassword` (行 188-213): 修改自助服务密码，成功后同步更新本地存储的 `passwordSaved` 状态。
*   **辅助状态** (行 82-86): `canBind` 校验手机号格式 (`/^1\d{10}$/`) 和短信验证码非空。

### `validators.ts` (行 1-18)
**职责**：前端即时反馈用的显示名校验逻辑，与后端 `validate_display_name` 对齐。

*   **规则**:
    *   `DISPLAY_NAME_MAX_LEN = 32` (码点数)。
    *   禁止控制字符 (`CONTROL_CHARS_RE`)，包括 C0、DEL、C1 区 (行 13)。
    *   `isValidDisplayName` (行 16-18): 检查 trim 后的长度和非控制字符。

### `operatorChip.ts` (行 1-14)
**职责**：根据运营商后缀返回对应的 Tailwind CSS 样式类名。

*   `operatorChipClass(operator)`:
    *   `@cmcc` -> 天蓝 (`sky-500/10 text-sky-600...`)
    *   `@telecom` -> 蓝 (`blue-500/10 text-blue-600...`)
    *   `@unicom` -> 橙 (`orange-500/10 text-orange-600...`)
    *   其他 -> 中性灰 (`bg-muted text-muted-foreground`)

### `SelfOnlineRecords.tsx` (行 1-348)
**职责**：上网记录融合卡。支持“近期流水”和“账单明细”两种视图，提供日期范围查询功能。

*   **数据归一化**:
    *   `recentToSessionRows(history)` (行 52-66): 将 Dashboard 带回的历史数据转换为 `SessionRow`。
    *   `billToSessionRows(rows)` (行 68-85): 将账单明细数据转换为 `SessionRow`，补充上下行流量和 NAS 信息。
*   **统计与分组**:
    *   `aggregateStats(rows)` (行 94-104): 计算总时长、流量、金额和次数。
    *   `groupByDay(rows)` (行 107-120): 按登录时间的本地日期分组。
*   **视图逻辑** (行 155):
    *   `queried` (logRows !== null) 决定是展示账单明细还是近期流水。
    *   **账单明细视图** (行 225-295): 展示8项汇总统计，会话行展示 IP、流量、上下行拆分、NAS 信息。
    *   **近期流水视图** (行 296-348): 展示4项聚合芯片，会话行展示 MAC、终端类型、计费方式。
*   **边界条件** (行 292-294): 日志超过500条时提示溢出。

### `index.ts` (行 1-5)
**职责**：模块入口，统一导出公共组件和 Hook。
*   导出内容：`AccountPanel`, `ChangePasswordDialog`, `useAccount`，以及 `types` 的所有类型。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                    SelfServicePanel.tsx                      │
│  (Core UI Container for Online Devices & Logs)               │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ OnlineDevice │  │ SelfOnline   │  │ ConfirmDialog    │   │
│  │ Stack        │  │ Records      │  │ (Kick Offline)   │   │
│  │ (Stack/H)    │  │              │  │                  │   │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘   │
│         │                 │                    │             │
│         ▼                 ▼                    ▼             │
│  ┌──────────────────────────────────────────────────┐       │
│  │            useSelfServiceVerify()                │       │
│  │  (Biometric Gate for Session Actions)            │       │
│  └──────────────────────┬───────────────────────────┘       │
│                         │                                    │
│  ┌──────────────────────▼───────────────────────────┐       │
│  │            useAccount.ts / useOperatorBind.ts    │       │
│  │  (Logic for Account Mgmt & Operator Binding)     │       │
│  └──────────────────────┬───────────────────────────┘       │
│                         │                                    │
│  ┌──────────────────────▼───────────────────────────┐       │
│  │              selfServiceState.ts                 │       │
│  │  (Global TTL Gates: useHelloGate, etc.)          │       │
│  └──────────────────────┬───────────────────────────┘       │
│                         │                                    │
│  ┌──────────────────────▼───────────────────────────┐       │
│  │              Tauri API Layer                     │       │
│  │  (querySelfDashboard, bindOperator, etc.)        │       │
│  └──────────────────────────────────────────────────┘       │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化与配置加载**:
    *   `SelfServicePanel` 从 `useConfigStore` 获取 `configUser`, `passwordSaved`, `deviceAliases` (行 77-97)。
    *   配置加载且凭据就绪后，自动触发 `fetchDashboard` (行 142-148)。

2.  **数据获取与展示**:
    *   `SelfServicePanel` 调用 `tauriApiWithRetry.querySelfDashboard` 获取在线设备和历史记录 (行 112-136)。
    *   数据经过 `formatTerminalType`, `formatUseTimeMinutes`, `formatFlowMb` 等工具函数格式化 (行 39-68)。
    *   在线设备数据传递给 `OnlineDeviceStack` 或 `OnlineDeviceStackH` 进行堆叠渲染 (行 268-306)。
    *   历史记录数据传递给 `SelfOnlineRecords`，通过 `recentToSessionRows` 或 `billToSessionRows` 归一化为 `SessionRow` 进行展示 (行 52-85)。

3.  **用户交互与操作**:
    *   **踢下线**: 用户点击注销按钮 -> `handleOffline` -> 调用 `useSelfServiceVerify()` 进行生物验证 -> 调用 `selfOfflineSession` API -> 成功后从 `onlineList` 中移除该设备 (行 150-175)。
    *   **改名**: 用户点击改名 -> 弹出 `DeviceAliasDialog` -> 输入新名称 -> 调用 `handleSaveAlias` -> 更新 `config.deviceAliases` (行 178-195)。
    *   **运营商绑定**: 用户点击绑定 -> 弹出 `BindOperatorDialog` -> 选择凭据模式 (`login`/`manual`) -> 输入手机号/验证码 -> 调用 `useOperatorBind` 中的 `handleBindOperator` -> 涉及 `useHelloGate` 生物验证 -> 调用 `bindOperator` API (行 151-181)。
    *   **账号管理**: 用户编辑账号信息 -> `LoginInfoDialog` -> `handleSaveLoginInfo` -> 调用 `createAccount` 或 `saveCurrentAsAccount` -> 更新 Store (行 166-215)。

4.  **生物验证门控**:
    *   所有敏感操作（如踢下线、查询绑定状态、揭示明文密码）均需通过 `useSelfServiceVerify` 或 `useHelloGate`。
    *   `useHelloGate` 维护全局 TTL (`VERIFY_TTL_MS = 570_000`)，并处理时钟回拨防护 (行 58-62)。
    *   揭示明文密码 (`handleReveal`) 强制要求验证，即使总开关关闭 (行 124)。

## Connections

*   **Settings Panel**: `useAccount.ts` 中的 `handleSwitchAccount` 等方法可能被 Settings 模块调用，用于全局账号切换。
*   **Dashboard Module**: `SelfOnlineRecords` 接收来自 Dashboard 的 `history` 数据 (行 155)，表明两者存在数据依赖。
*   **Tauri Backend**: 所有 API 调用 (`tauriApiWithRetry`) 指向后端提供的账号管理、运营商绑定、自助服务等接口。

## Known Issues

1.  **StrictMode 副作用安全**: `AccountPanel` 使用 `mountedRef` 跟踪组件挂载状态，所有异步操作在 setState 前检查 `mountedRef.current`，以防止 React 18 StrictMode 双重调用导致的无效状态更新 (行 66-71, 144, 174, 184, 196, 202, 208, 214)。
2.  **密码清除语义**: 后端约定空字符串密码表示“保留旧密码”。因此，显式清除密码必须走 `handleClearPassword`，它调用 `saveConfigDirect({ password: '' }, true)` 并设置 `syncPasswordSaved(false)`。如果直接留空保存，不会清除密码 (行 88-100)。
3.  **切换账号防重入**: `handleSwitchAccount` 和 `handleCardEdit` 均检查 `switchingAccount !== null`，防止在切换进行中再次触发操作 (行 105, 114, 327, 337, 348)。
4.  **编辑非激活账号的逻辑**: 点击非激活卡的“编辑”按钮时，`handleCardEdit` 会先尝试切换到该账号（`handleSwitchAccount`），切换成功后再打开编辑弹窗。这确保了编辑的是当前上下文可能期望的账号，或者至少触发了切换流程 (行 112-119)。
5.  **运营商凭据覆盖机制**: `BindOperatorDialog` 允许用户在绑定时手动输入账号密码（`credMode === 'manual'`）。这些凭据仅在本次绑定中使用，通过 `bindCredOverride` 传递给 `useOperatorBind` Hook，不会保存到本地存储。关闭弹窗或切换回 `login` 模式时，凭据被清空 (行 68-72, 83-86)。
6.  **密码修改对话框的旧密码处理**: 默认情况下，旧密码由后端 resolve（使用已保存的登录密码）。用户可以选择展开手动输入旧密码框，此时手动输入的旧密码优先于已存密码 (行 16-19, 84-108)。
7.  **上线记录的数据归一化**: `SelfOnlineRecords` 将两种不同来源的数据（Dashboard 历史数据和账单明细）统一转换为 `SessionRow` 接口，使得后续的分组和统计逻辑可以复用 (行 52-85)。
8.  **设备别名保存的错误吞没**: `DeviceAliasDialog` 的 `submit` 函数中，catch 块吞掉了错误，因为错误提示已由调用方（`handleSaveAlias`）发出。这是为了防止 unhandled rejection，同时保持弹窗打开以便重试 (行 41-46)。
9.  **时钟回拨死锁防护**: `selfServiceState.ts` 行 55-62，`gateFresh` 不仅检查 TTL，还检查 `elapsed >= 0`。若系统时钟回拨，前端视为过期，强制重新验证，避免前端缓存永真而后端已过期的死锁。
10. **明文密码安全**: `selfServiceState.ts` 行 50-52, `useOperatorBind.ts` 行 124，查看明文密码 (`reveal`) 必须强制验证，即使 `selfHelloEnabled` 为 false。这是为了对齐后端 TTL 校验，防止总开关成为绕过明文保护的路径。明文密码在 state 中保留 30 秒自动清除 (`useOperatorBind.ts` 行 55-62)。
11. **并发切换去重**: `useAccount.ts` 行 11, 78-103，`switchInFlight` promise 机制确保同一时刻只有一个 switch 请求在执行，后续请求等待其结果，避免状态交错。
12. **假阴性判断**: `useAccount.ts` 行 87-90，切换账号时，后端可能返回空字符串作为 `activeAccount`。代码使用 `result?.activeAccount !== undefined` 而非真值判断，确保能正确处理空字符串情况，避免 UI 不更新。
13. **时间格式化一致性**: `SelfServicePanel.tsx` 行 51-54, `selfServiceTypes.ts` 行 61-64，使用 `Math.floor(seconds / 60)` 而非 `round`，严格对齐原站行为（例如 90 秒显示为 1 分钟而非 2 分钟）。
14. **触控冲突隔离**: `SelfOnlineStack.tsx` 行 13, 155，堆叠容器设置 `touch-action: none`，防止与 WebView 页面滚动冲突。用户需点击堆叠区域外才能滚动页面。
15. **静默降级**: `SelfServicePanel.tsx` 行 127-129，`fetchDashboard` 在非手动触发（如自动刷新）且失败时，不弹出错误 Toast，仅在手动刷新失败时提示，提升用户体验。
16. **JSdom 兼容性**: `SelfOnlineStack.tsx` 行 106-110, 行 81-82，`setPointerCapture` 和 `ResizeObserver` 在测试环境（jsdom）中可能缺失，代码做了 try-catch 或 `typeof undefined` 检查以优雅降级。