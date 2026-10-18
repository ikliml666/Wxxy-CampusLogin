---
title: frontend-account
type: module
source_files:
  - android/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/account/BindOperatorDialog.tsx
  - android/frontend/src/account/ChangePasswordDialog.tsx
  - android/frontend/src/account/DeviceAliasDialog.tsx
  - android/frontend/src/account/index.ts
  - android/frontend/src/account/LoginInfoDialog.tsx
  - android/frontend/src/account/operatorChip.ts
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/selfServiceTypes.ts
  - android/frontend/src/account/types.ts
  - android/frontend/src/account/useAccount.ts
  - android/frontend/src/account/useOperatorBind.ts
  - android/frontend/src/account/validators.ts
tags: [account, self-service, biometric, tauri, state-management]
---

## Overview

`frontend-account` 模块是 Android/Tauri 前端应用中的核心账户管理与自助服务层。它负责处理多账号生命周期（创建、切换、删除、重命名）、校园网运营商绑定、密码管理以及在线设备监控。该模块深度集成了 Windows Hello 生物识别验证门控，确保敏感操作（如查看明文密码、注销设备）的安全性。UI 层面采用了复杂的堆叠卡片（Coverflow）布局以适配手机与平板形态，并提供了详细的上网记录查询功能。

## Key Components

### `AccountPanel.tsx`
**职责**：账号管理主面板，整合账号列表、登录信息编辑、运营商绑定及自动化设置。

*   **Props**:
    *   `accounts: AccountItem[]`: 账号列表数据。
    *   `activeAccount: string`: 当前激活账号 ID。
    *   `onUpdateConfig(partial: Partial<Config>)`: 更新配置回调。
    *   `onAddAccount(name: string)`: 添加账号回调。
    *   `onDeleteAccount(id, displayName)`: 删除账号回调。
    *   `onSwitchAccount(name)`: 切换账号回调。
    *   `onRenameAccount(accountId, displayName)`: 改名回调。
*   **状态管理 (Hooks)**:
    *   `useConfigStore(useShallow(...))`: 浅比较订阅 config，避免非必要重渲染。
    *   `loginInfoOpen/mode`: 控制弹窗A（创建/编辑登录信息）。
    *   `bindOpen/bindCredOverride`: 控制弹窗B（绑定运营商），`credOverride` 用于手动输入凭据覆盖默认配置。
    *   `switchingAccount`: 防重复提交锁，防止在切换进行中再次点击。
    *   `renamingId/renameDraft/renaming`: 内联改名状态机。
*   **关键函数**:
    *   `handleClearPassword`: 调用 `store.saveConfigDirect({ password: '' }, true)` 清除密码。注意后端空密码语义为“保留旧”，需显式标志位。失败时 Toast 且不显示成功。
    *   `handleCardEdit(item)`: 若为非激活卡，先执行 `handleSwitchAccount` 切换至该卡，再打开弹窗A编辑态。
    *   `submitRename()`: 校验 `isValidDisplayName`，调用 `onRenameAccount`。
    *   `refreshAccounts()`: 调用 `tauriApiWithRetry.listAccounts()` 刷新列表并更新 Store。
    *   `handleSaveLoginInfo(values)`:
        *   **Create 模式**: 调用 `tauriApiWithRetry.createAccount`。成功后刷新列表。
        *   **Edit 模式**: 先 `store.saveConfigDirect` 保存主配置（user/operator/password），再调用 `tauriApiWithRetry.saveCurrentAsAccount` 回写激活档案。
    *   `bindFlow = useOperatorBind(...)`: 封装运营商绑定状态机。

### `BindOperatorDialog.tsx`
**职责**：运营商绑定弹窗（弹窗B）。包含凭据分段选择（登录信息 vs 手动填写）、ISP 选择、手机号/SMS 密码输入，以及内置的绑定状态查询子弹窗。

*   **Props**:
    *   `bind: BindFlow`: 传入 `useOperatorBind` 返回的状态机实例。
    *   `onCredChange(cred | null)`: 上报凭据草稿变化，供父级设置 `credOverride`。
*   **关键实现**:
    *   **凭据分段**: `credMode` ('login' | 'manual')。`manual` 模式下用户输入账号密码，通过 `onCredChange` 实时传给父级 hook。
    *   **状态复位**: `useEffect` 监听 `open`，打开时重置凭据草稿。
    *   **子弹窗**: 内部嵌套 `Dialog` 用于展示 `fetchBindStatus` 的结果（掩码账号、揭示密码按钮）。
    *   **操作**: "查询绑定状态" 和 "绑定" 按钮，依赖 `canQueryStatus` / `canBind` 等状态机属性禁用。

### `ChangePasswordDialog.tsx`
**职责**：修改校园网密码弹窗。

*   **Props**:
    *   `onSave(newPassword, oldPassword)`: 保存回调。
*   **关键实现**:
    *   **校验**: 正则 `/^[0-9A-Za-z]{6,11}$/` 校验新密码格式；二次确认密码一致性。
    *   **旧密码处理**: 默认隐藏，点击链接展开输入框。留空则后端回退使用已保存密码。
    *   **防双击**: `saving` 状态锁定提交按钮。

### `DeviceAliasDialog.tsx`
**职责**：在线设备改名弹窗。

*   **Props**:
    *   `mac: string`: 原始 12 位 hex MAC 地址。
    *   `initialName: string`: 初始别名。
    *   `onSave(name)`: 保存回调。
*   **关键实现**:
    *   轻弹窗结构，仅包含一个输入框。
    *   错误处理由调用方负责，本组件吞掉异常以防 unhandled rejection，保持弹窗打开以便重试。

### `LoginInfoDialog.tsx`
**职责**：添加/编辑登录信息弹窗（弹窗A）。

*   **Props**:
    *   `mode: 'create' | 'edit'`: 决定 UI 字段（创建时多 `accountName`）和校验规则。
    *   `initial: { user, operator }`: 编辑态预填值。
    *   `passwordSaved`: 是否已保存密码，控制是否显示“修改/清除密码”链接。
    *   `onChangePassword` / `onClearPassword`: 快捷入口回调。
*   **关键实现**:
    *   **校验**: 创建时校验 `accountName` (`isValidDisplayName`)、`user` 非空、`password` 非空。
    *   **运营商选择**: 默认值处理，`__default__` 转为空字符串提交。
    *   **密码行为**: 编辑态且密码已保存时，密码输入框 placeholder 提示“已保存”，并提供旁侧操作链接。

### `operatorChip.ts`
**职责**：运营商品牌色样式生成器。

*   **导出**: `operatorChipClass(operator: string | null | undefined): string`
*   **映射**:
    *   `@cmcc`: 天蓝 (`sky`)
    *   `@telecom`: 蓝 (`blue`)
    *   `@unicom`: 橙 (`orange`)
    *   其他: 中性灰 (`muted`)

### `selfServiceState.ts`
**职责**：管理 Windows Hello 验证状态、TTL 过期处理及错误翻译。

*   **全局变量**:
    *   `helloGateVerifiedAt`: 通用 Hello 验证时间戳。
    *   `selfSessionVerifiedAt`: 自助服务面板会话验证时间戳。
*   **核心逻辑**:
    *   `gateFresh(verifiedAt)`: 判断 TTL 是否有效 (`VERIFY_TTL_MS = 570_000`)。考虑时钟回拨 (`elapsed >= 0`)。
    *   `useHelloGate(options?)`:
        *   返回 `Promise<boolean>`。
        *   `ignoreToggle: true` 时强制验证，无视总开关。
        *   调用 `tauriApiWithRetry.verifyWindowsIdentity`。
    *   `useSelfServiceVerify()`:
        *   检查 `selfReverifyEachAction` 配置。若为 false 且 TTL 未过，直接放行。
        *   面板卸载时由 `resetSelfSessionGate()` 重置时间戳。
    *   `biometricFailMessage(err, t)`: 根据错误码 (`code`) 映射 i18n key (如 `userCancel`, `biometryNotEnrolled`)。
*   **导出名**: `biometricFailMessage`, `useHelloGate`, `resetSelfSessionGate`, `useSelfServiceVerify`。

### `selfServiceTypes.ts`
**职责**：定义自助服务相关数据结构及通用格式化函数。

*   **类型**:
    *   `SelfHistoryRow`: Tuple `[epoch_login, epoch_logout, ip, mac, duration_min, flow_mb, billing_type, cost, hostname, terminal_type, ...unknown]`。
    *   `SelfLogRow`: 详细日志对象 (`loginTime`, `logoutTime`, `time`, `flow`, `costMoney`, `internetUpFlow`, etc.)。
    *   `SelfLogSummary`: 汇总数据 (`INTERNETUPFLOW`, `FLOW`, `TIME`, `COU`, etc.)。
    *   `SelfOnlineItem`: 在线信息 (`loginTime`, `ip`, `mac`, `useTime`, `downFlow`, `upFlow`, `hostName`, `terminalType`, `sessionId`)。
*   **工具函数**:
    *   `formatMac(mac)`: 剥离非 hex 字符，每两位加 `-`。
    *   `formatEpoch(ms)`: 转为本地 YYYY-MM-DD HH:mm:ss。
    *   `formatTerminalType(val)`: 去除 `#` 前缀。
    *   `toInt(value)`: 安全解析整数。
    *   `formatUseTimeMinutes(seconds)`: 秒转分钟，`Math.floor`。
    *   `formatFlowMb(down, up)`: KB 转 MB，保留 3 位小数。
    *   `fmtMoney(value)`: 保留原始精度，非数值返回 `-`。
    *   `localDateStr(d)`: 生成 YYYY-MM-DD 字符串。
*   **导出名**: `SelfHistoryRow`, `SelfLogRow`, `SelfLogSummary`, `formatMac`, `formatEpoch`, `formatTerminalType`, `toInt`, `formatUseTimeMinutes`, `formatFlowMb`, `fmtMoney`, `localDateStr`。

### `types.ts`
**职责**：定义账号 CRUD 操作的统一返回结构。

*   **接口**:
    *   `AccountResult`: `success`, `message?`, `activeAccount?`, `config?`, `displayName?`。
    *   `SwitchAccountResult`, `DeleteAccountResult`, `SaveAccountResult`: 类似结构，侧重不同场景。
*   **导出名**: `AccountItem` (复用), `AccountResult`, `SwitchAccountResult`, `DeleteAccountResult`, `SaveAccountResult`。

### `useAccount.ts`
**职责**：封装账号增删改查及切换逻辑，处理并发去重与状态同步。

*   **并发控制**: `switchInFlight` Promise 防止切换操作交错。
*   **API 调用**:
    *   `refreshAccounts`: 调用 `listAccounts`。
    *   `handleAddAccount`: `saveCurrentAsAccount`。成功则更新 config/activeAccount 并 Toast。
    *   `handleDeleteAccount`: `deleteAccount`。注意历史缺陷修复：必须检查 `activeAccount !== undefined` 而非真值判断。
    *   `handleSwitchAccount`: `switchAccount`。同上，严格检查 `activeAccount`。
    *   `handleRenameAccount`: `renameAccount`。仅更新 displayName。
*   **状态同步**: 所有写操作后均调用 `refreshAccounts` 保持列表最新。
*   **导出名**: `useAccount` (返回包含 handlers 的对象)。

### `useOperatorBind.ts`
**职责**：管理运营商绑定、状态查询、密码揭示及修改流程。

*   **状态**: `bindOp`, `bindPhone`, `bindSms`, `bindStatuses`, `revealedPassword` 等。
*   **凭据逻辑**:
    *   `credOverride` 优先于 `user/passwordSaved`。
    *   `credReady` 判定：账号非空 + 密码已保存/非空。
*   **API 交互**:
    *   `fetchBindStatus`: `getBindStatus`。成功后打开状态弹窗。
    *   `handleReveal`: `revealOperatorCredential`。需 `ignoreToggle: true` 的 Hello 验证。明文 30s 自动清除。
    *   `handleBindOperator`: `bindOperator`。校验手机号格式 `^1\d{10}$` 及短信验证码。
    *   `handleChangePassword`: `changeSelfPassword`。成功后同步更新本地 `password` 配置。
*   **导出名**: `BIND_OPERATOR_NONE`, `useOperatorBind` (返回大量状态 setters 及 handlers)。

### `validators.ts`
**职责**：提供显示名校验逻辑，对齐后端规范。

*   **常量**: `DISPLAY_NAME_MAX_LEN = 32` (码点数)。
*   **正则**: `CONTROL_CHARS_RE` 匹配 C0/DEL/C1 控制字符。
*   **函数**: `isValidDisplayName(trimmed)`: 非空、长度<=32、无控制字符。
*   **导出名**: `DISPLAY_NAME_MAX_LEN`, `CONTROL_CHARS_RE`, `isValidDisplayName`。

## Architecture

模块采用分层架构：UI 层 (`AccountPanel`, `SelfServicePanel`) 协调状态，业务逻辑层 (Hooks `useAccount`, `useOperatorBind`) 处理 API 交互与并发，验证层 (`selfServiceState`) 管理生物识别门控，类型层 (`types`, `selfServiceTypes`) 定义契约。

```text
┌─────────────────────────────────────────────────────────────┐
│                      SelfServicePanel                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ OnlineDevice │  │ SelfOnline   │  │ Operator Bind    │  │
│  │ Stack/H      │  │ Records      │  │ Dialog           │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
│         │                 │                   │            │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌───────▼─────────┐  │
│  │ useAccount   │  │ useOperator  │  │ useSelfService  │  │
│  │ Hook         │  │ Bind Hook    │  │ Verify Hook     │  │
│  └──────┬───────┘  └──────┬───────┘  └───────┬─────────┘  │
│         │                 │                   │            │
│  ┌──────▼───────┐  ┌──────▼───────┐  ┌───────▼─────────┐  │
│  │ Tauri API    │  │ Tauri API    │  │ Tauri API       │  │
│  │ Retry Wrapper│  │ Retry Wrapper│  │ verifyWinIdent  │  │
│  └──────────────┘  └──────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **顶层容器**: `AccountPanel` 是核心协调者。
    *   它持有 `useOperatorBind` 实例 (`bindFlow`)，并将其传递给 `BindOperatorDialog`。
    *   它管理 `LoginInfoDialog` 的打开/关闭及表单值提交 (`handleSaveLoginInfo`)。
    *   它管理 `ChangePasswordDialog` 的打开/关闭及保存 (`handleChangePassword`)。
    *   它通过 `onRenameAccount` prop 处理内联改名。

2.  **数据流向**:
    *   **账号列表**: `AccountPanel` -> `refreshAccounts` -> `tauriApiWithRetry.listAccounts` -> `useConfigStore.setAccounts`。
    *   **登录信息保存**:
        *   `LoginInfoDialog` -> `onSave` (`AccountPanel.handleSaveLoginInfo`)。
        *   Create: `tauriApiWithRetry.createAccount`。
        *   Edit: `useConfigStore.saveConfigDirect` + `tauriApiWithRetry.saveCurrentAsAccount`。
    *   **运营商绑定**:
        *   `BindOperatorDialog` -> `bindOp`/`bindPhone`/`bindSms` 状态更新。
        *   凭据来源: 默认取 `config.user` + `passwordSaved` 状态；手动模式通过 `onCredChange` -> `AccountPanel.bindCredOverride` -> `useOperatorBind` hook 内部使用。
        *   操作: `bindFlow.handleBindOperator` / `fetchBindStatus`。
    *   **密码修改**:
        *   `ChangePasswordDialog` -> `onSave(newPwd, oldPwd)`。
        *   成功后由父级（未在源码中完全展示父级具体逻辑，但注释提及）同步本地登录密码。

3.  **UI 组件复用**:
    *   `operatorChipClass` 被 `AccountPanel` 中的账号卡片运营商标签使用。
    *   `SelfOnlineRecords` 是一个独立的卡片组件，可能在更外层容器中被引入，但在本批源码中仅作为独立文件存在，未直接嵌入 `AccountPanel` 的 JSX（注释提到“手机形态...”，可能位于其他页面或作为独立 Tab）。

## Connections

*   [[frontend-config]]: 通过 `useConfigStore` 读取和写入用户配置。
*   [[frontend-auth]]: 通过 `selfServiceState.ts` 集成 Windows Hello 生物识别验证。
*   [[backend-api]]: 通过 `tauriApiWithRetry` 系列函数调用后端 REST/RPC 接口。

## Known Issues

1.  **StrictMode 双重渲染**: `AccountPanel` 使用 `mountedRef` 并在 `useEffect` 中恢复它，防止 React StrictMode 下异步回调在组件卸载后尝试 setState。
2.  **密码清除语义**: 后端对空密码的处理是“保留旧密码”，因此前端必须通过 `saveConfigDirect({ password: '' }, true)` 传递显式清除标志，否则无法真正清除。
3.  **切换账号防抖**: `switchingAccount` 状态锁住所有切换相关操作，直到 `onSwitchAccount` 完成。
4.  **改名校验**: 前端 `isValidDisplayName` 与后端 `validate_display_name` 口径一致，失败时即时 Toast 错误。
5.  **绑定凭据隔离**: `BindOperatorDialog` 的手动输入凭据仅在本次绑定中使用（`credOverride`），不保存到全局配置或本地存储，关闭弹窗即回收。
6.  **设备改名错误吞没**: `DeviceAliasDialog` 捕获 `onSave` 的异常，不向上抛出，而是保留弹窗状态供用户修改重试，错误提示由调用方发出。
7.  **登录信息编辑回填**: 编辑模式下，密码字段不回填明文（出于安全考虑），留空表示保留现有密码。
8.  **上网记录分页/限制**: `SelfOnlineRecords` 在日志行数超过 500 条时给出提示，暗示后端可能有分页或截断机制，前端仅展示前 500 条或全部（视后端而定，此处仅为 UI 提示）。
9.  **并发与竞态**:
    *   `useAccount` 中 `switchInFlight` 锁防止重复切换。
    *   `mountedRef` 在所有异步回调中检查，防止组件卸载后 setState。
    *   `SelfOnlineStack` 中 `suppressClick` 标志防止拖拽结束后误触点击事件。
10. **边界条件**:
    *   `SelfOnlineStack`: `count < 2` 时禁用拖拽/点击切换；`count === 2` 时隐藏底部露出区。
    *   `SelfServicePanel`: `configLoaded` 未完成时不执行自动刷新/凭据检查。
    *   `selfServiceState`: `gateFresh` 处理时钟回拨 (`elapsed < 0`)，视为过期。
    *   `useOperatorBind`: `revealedPassword` 30秒自动清除。
11. **非显然行为**:
    *   **验证门 TTL**: 前端缓存 TTL (570s) 略小于后端 (600s)，预留 30s 余量，避免前端永真后端拒绝的死锁。
    *   **明文保护**: `ignoreToggle: true` 模式下，即使配置关闭 Hello 验证，仍强制执行生物识别，防止绕过。
    *   **账号 ID 判断**: `useAccount` 中修复了历史 Bug，使用 `!== undefined` 而非真值判断 `activeAccount`，因为后端可能返回空串。
    *   **时间格式化**: `formatUseTimeMinutes` 使用 `Math.floor` 模拟原站行为，避免 `round` 导致的显示误差 (90s -> 1min vs 2min)。
    *   **本地日期**: `localDateStr` 避免使用 `toISOString` (UTC)，防止跨时区日期偏移。