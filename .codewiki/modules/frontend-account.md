---
title: frontend-account
type: module
source_files:
  - android/frontend/src/account/validators.ts
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
tags: [account, self-service, biometric, tauri, state-management]
---

## Overview

`frontend-account` 模块是 Android/Tauri 前端应用中的核心账户管理与自助服务层。它负责处理多账号生命周期（创建、切换、删除、重命名）、校园网运营商绑定、密码管理、在线设备改名，并深度集成 Windows Hello 生物识别验证门控以确保敏感操作安全。UI 层面由 `AccountPanel` 作为主面板整合登录信息弹窗（`LoginInfoDialog`）、绑定运营商弹窗（`BindOperatorDialog`）、修改密码弹窗（`ChangePasswordDialog`）与自动化设置；业务逻辑通过 `useAccount`、`useOperatorBind` 等 Hook 封装 Tauri IPC 调用；显示名即时校验 `isValidDisplayName` 与后端 `validate_display_name` 单点对齐，前端仅做即时反馈，后端为权威。

## Key Components

### `android/frontend/src/account/validators.ts`
- 导出常量 `DISPLAY_NAME_MAX_LEN`（`android/frontend/src/account/validators.ts:9`），默认值为 `32`，表示显示名最大长度，按 Unicode 码点数计，与后端一致。
- 导出正则对象 `CONTROL_CHARS_RE`（`android/frontend/src/account/validators.ts:13`），匹配控制字符（C0 全区 + DEL + C1 区 U+0080–U+009F）；`android/frontend/src/account/validators.ts:13-14` 主动使用 `eslint-disable-next-line no-control-regex` 关闭对应 lint 规则，属预期行为。
- 导出函数 `isValidDisplayName(trimmed: string): boolean`（`android/frontend/src/account/validators.ts:16-17`），改名草稿校验；入参 `trimmed` 须为已 trim 的字符串（`android/frontend/src/account/validators.ts:15`）；内部通过 `!!trimmed` 保证非空、`[...trimmed].length <= DISPLAY_NAME_MAX_LEN` 保证码点长度不超上限、`!CONTROL_CHARS_RE.test(trimmed)` 保证不含控制字符。

### `android/frontend/src/account/AccountPanel.tsx`
- 导出组件 `AccountPanel`（memo 包裹，`android/frontend/src/account/AccountPanel.tsx:50`）及接口 `AccountPanelProps`（`android/frontend/src/account/AccountPanel.tsx:32-41`），字段含 `accounts`/`activeAccount`/`onUpdateConfig`/`onAddAccount`/`onDeleteAccount`/`onSwitchAccount`/`onRenameAccount`。
- 状态与订阅：`useConfigStore(useShallow(...))` 浅比较订阅 config（`android/frontend/src/account/AccountPanel.tsx:63`）；`passwordSaved` 取 store（`android/frontend/src/account/AccountPanel.tsx:59`）；`mountedRef` 处理 StrictMode 双调用（`android/frontend/src/account/AccountPanel.tsx:64-71`）。
- 弹窗控制：`loginInfoOpen`/`loginInfoMode`('create'|'edit')（`android/frontend/src/account/AccountPanel.tsx:74-79`）；`bindOpen`/`bindCredOverride`（`android/frontend/src/account/AccountPanel.tsx:82-86`）及 `handleBindCredChange`。
- 关键回调：
  - `applyActiveCardPasswordSaved`（`android/frontend/src/account/AccountPanel.tsx:92-96`）：直接改 store.accounts 内存，修正激活卡密码状态显示（解决档案回写前窗口期读旧密文问题）。
  - `handleClearPassword`（`android/frontend/src/account/AccountPanel.tsx:100-115`）：调 `store.saveConfigDirect({password:''}, true)`（第二参 true 为 clearPassword 标志，因后端空密码语义是保留旧密码），成功则 `syncPasswordSaved(false)`+`applyActiveCardPasswordSaved(false)`+`clearDirtyFields()`。
  - `handleSwitchAccount`（`android/frontend/src/account/AccountPanel.tsx:119-123`）：`switchingAccount` 防重锁。
  - `handleCardEdit`（`android/frontend/src/account/AccountPanel.tsx:127-134`）：非激活卡先 `handleSwitchAccount` 再开编辑弹窗。
  - 改名内联：`renamingId`/`renameDraft`/`renaming` 及 `submitRename`（`android/frontend/src/account/AccountPanel.tsx:138-164`），校验 `isValidDisplayName` 后调 `onRenameAccount`。
  - `refreshAccounts`（`android/frontend/src/account/AccountPanel.tsx:168-175`）：`tauriApiWithRetry.listAccounts()` -> `setAccounts`。
  - `handleSaveLoginInfo`（`android/frontend/src/account/AccountPanel.tsx:180-234`）：create 调 `createAccount`（`android/frontend/src/account/AccountPanel.tsx:183-188`）；edit 调 `saveConfigDirect(partial)` + `saveCurrentAsAccount(activeAccount)`（`android/frontend/src/account/AccountPanel.tsx:211-219`），填新密时先行 `applyActiveCardPasswordSaved(true)`。
  - `bindFlow = useOperatorBind(config.user, config.operator, passwordSaved, bindCredOverride)`（`android/frontend/src/account/AccountPanel.tsx:239`）。
- 渲染逻辑：账号卡用 `OnlineDeviceStack`（`android/frontend/src/account/AccountPanel.tsx:261-384`），显示学号、`operatorChipClass` 运营商 chip（`android/frontend/src/account/AccountPanel.tsx:292-297`）、密码状态 chip（`android/frontend/src/account/AccountPanel.tsx:298-305`）；操作行编辑/改名/切换/删除按钮（`android/frontend/src/account/AccountPanel.tsx:337-379`）；自动化卡三开关绑定 `autoLoginOnStart`/`autoLoginOnPreparation`/`enableNightOperatorSwitch`（`android/frontend/src/account/AccountPanel.tsx:443-474`）。
- 弹窗挂载：`<LoginInfoDialog>`（`android/frontend/src/account/AccountPanel.tsx:481-490`，传 initial/config）、`<BindOperatorDialog>`（`android/frontend/src/account/AccountPanel.tsx:493-498`，传 bindFlow）、`<ChangePasswordDialog>`（`android/frontend/src/account/AccountPanel.tsx:501-505`，onSave=handleChangePassword）。

### `android/frontend/src/account/BindOperatorDialog.tsx`
- 导出 `BindOperatorDialog`（`android/frontend/src/account/BindOperatorDialog.tsx:41`），类型 `BindFlow = ReturnType<typeof useOperatorBind>`（`android/frontend/src/account/BindOperatorDialog.tsx:41`），接口 `BindOperatorDialogProps`（`android/frontend/src/account/BindOperatorDialog.tsx:32-39`，含 open/onOpenChange/bind/onCredChange）。
- 凭据分段：`credMode`('login'|'manual')（`android/frontend/src/account/BindOperatorDialog.tsx:51`），手动 `manualAccount`/`manualPassword`/`showManualPassword`（`android/frontend/src/account/BindOperatorDialog.tsx:52-54`）；open 驱动复位（`android/frontend/src/account/BindOperatorDialog.tsx:57-64`）；凭据变化实时 `onCredChange` 上报父级（login 模式传 null，`android/frontend/src/account/BindOperatorDialog.tsx:68-72`）。
- 表单：运营商 `Select` 过滤 `ISP_OPTIONS` 中 `__default__`（`android/frontend/src/account/BindOperatorDialog.tsx:149`）；手机号输入过滤非数字 `replace(/\D/g,'')`（`android/frontend/src/account/BindOperatorDialog.tsx:162`）；短信密码 `bindSms`（`android/frontend/src/account/BindOperatorDialog.tsx:162-169`）。
- 动作按钮：查询状态（`setStatusOpen(true)`+`fetchBindStatus()`，`android/frontend/src/account/BindOperatorDialog.tsx:182-193`），绑定（`handleBindOperator`，`android/frontend/src/account/BindOperatorDialog.tsx:194-200`）。
- 状态弹窗（`android/frontend/src/account/BindOperatorDialog.tsx:206-275`）：遍历 `ISP_OPTIONS` 显掩码账号，reveal 按钮调 `handleReveal` 切换显 `revealedPassword`（`android/frontend/src/account/BindOperatorDialog.tsx:229-245`）。

### `android/frontend/src/account/ChangePasswordDialog.tsx`
- 导出 `ChangePasswordDialog`（`android/frontend/src/account/ChangePasswordDialog.tsx:30`），接口 `ChangePasswordDialogProps`（`android/frontend/src/account/ChangePasswordDialog.tsx:21-25`，onSave 签名为 `(newPassword, oldPassword)=>Promise<boolean>`）。
- 常量 `NEW_PASSWORD_RE = /^[0-9A-Za-z]{6,11}$/`（`android/frontend/src/account/ChangePasswordDialog.tsx:28`，6–11位仅数字英文）。
- `submit`（`android/frontend/src/account/ChangePasswordDialog.tsx:54-72`）：校验正则+确认一致，调 `onSave`，失败保留草稿（父级已 toast）；open 驱动复位（`android/frontend/src/account/ChangePasswordDialog.tsx:54-65`）。
- 可展开手动旧密码 `showOld`（`android/frontend/src/account/ChangePasswordDialog.tsx:84-108`，默认仅收新密码）。

### `android/frontend/src/account/DeviceAliasDialog.tsx`
- 导出 `DeviceAliasDialog`（`android/frontend/src/account/DeviceAliasDialog.tsx:25`），接口 `DeviceAliasDialogProps`（`android/frontend/src/account/DeviceAliasDialog.tsx:15-23`，含 mac/initialName/onSave/onCancel）。
- `submit`（`android/frontend/src/account/DeviceAliasDialog.tsx:36-47`）：`onSave(name.trim())`，吞掉异常仅防 unhandled rejection，保留草稿；open 驱动复位（`android/frontend/src/account/DeviceAliasDialog.tsx:36-39`）。

### `android/frontend/src/account/index.ts`
- 导出 `AccountPanel`（`android/frontend/src/account/index.ts:1`）、`ChangePasswordDialog`（`android/frontend/src/account/index.ts:2`）、`useAccount`（`android/frontend/src/account/index.ts:3`）、`types` 通配导出（`android/frontend/src/account/index.ts:5`）。其余对话框/hook 未在此桶文件导出，由 `AccountPanel` 内部直接引用。

### `android/frontend/src/account/LoginInfoDialog.tsx`
- 导出 `LoginInfoDialog`（`android/frontend/src/account/LoginInfoDialog.tsx:51`），接口 `LoginInfoFormValues`（`android/frontend/src/account/LoginInfoDialog.tsx:29-34`：accountName/user/password/operator），`LoginInfoDialogProps`（`android/frontend/src/account/LoginInfoDialog.tsx:36-49`）。
- `submit`（`android/frontend/src/account/LoginInfoDialog.tsx:84-110`）：create 校验 `isValidDisplayName(accountName)`+user非空+密码非空；`operator === '__default__'` 转空串（`android/frontend/src/account/LoginInfoDialog.tsx:84`）；编辑态且 `passwordSaved` 显修改/清除密码入口（`android/frontend/src/account/LoginInfoDialog.tsx:161-182`）；open 驱动复位（`android/frontend/src/account/LoginInfoDialog.tsx:84-95`）。

### `android/frontend/src/account/operatorChip.ts`
- 导出 `operatorChipClass(operator)`（`android/frontend/src/account/operatorChip.ts:3-13`）：`@cmcc` 天蓝、`@telecom` 蓝、`@unicom` 橙、其他中性灰，供账号页卡片与总览共用。

### `android/frontend/src/account/selfServiceState.ts`
- 导出 `biometricFailMessage(err,t)`（`android/frontend/src/account/selfServiceState.ts:34-38`）、`useHelloGate(options?)`（`android/frontend/src/account/selfServiceState.ts:65-87`）、`resetSelfSessionGate()`（`android/frontend/src/account/selfServiceState.ts:100-102`）、`useSelfServiceVerify()`（`android/frontend/src/account/selfServiceState.ts:105-126`）。
- 常量 `BIOMETRIC_ERROR_KEY`（`android/frontend/src/account/selfServiceState.ts:19-32`，按 code 映射 i18n key）、`VERIFY_TTL_MS = 570_000`（`android/frontend/src/account/selfServiceState.ts:63`，比后端600s少30s余量）、模块级 `helloGateVerifiedAt`（`android/frontend/src/account/selfServiceState.ts:63`）、`selfSessionVerifiedAt`（`android/frontend/src/account/selfServiceState.ts:97`）。
- 内部 `helloEnabled()`（`android/frontend/src/account/selfServiceState.ts:10`，config.selfHelloEnabled !== false 视为开启）；`gateFresh(verifiedAt)`（`android/frontend/src/account/selfServiceState.ts:58-62`，TTL内且时钟未回拨）。
- `useHelloGate`：绑定门，首次验证后 TTL 共用；`ignoreToggle:true` 时无视总开关强制真实验证（`android/frontend/src/account/selfServiceState.ts:65-87`）。
- `useSelfServiceVerify`：面板会话门，卸载需 `resetSelfSessionGate`；`selfReverifyEachAction===true` 每次验证（`android/frontend/src/account/selfServiceState.ts:105-126`）。

### `android/frontend/src/account/selfServiceTypes.ts`
- 导出类型 `SelfHistoryRow`（`android/frontend/src/account/selfServiceTypes.ts:5`，元组）、`SelfLogRow`（`android/frontend/src/account/selfServiceTypes.ts:9-22`，字段：loginTime/logoutTime/time/flow/costMoney/internetUpFlow/internetDownFlow/chinanetUpFlow/chinanetDownFlow/userIp/nasIp/nasPort）、`SelfLogSummary`（`android/frontend/src/account/selfServiceTypes.ts:25-34`，字段：INTERNETUPFLOW/INTERNETDOWNFLOW/CHINANETUPFLOW/CHINANETDOWNFLOW/FLOW/TIME/COSTMONEY/COU）。
- 工具函数：`formatMac`（`android/frontend/src/account/selfServiceTypes.ts:36-40`，剥非hex配对归一）、`formatEpoch`（`android/frontend/src/account/selfServiceTypes.ts:42-47`）、`formatTerminalType`（`android/frontend/src/account/selfServiceTypes.ts:50-53`，去#）、`toInt`（`android/frontend/src/account/selfServiceTypes.ts:55-58`）、`formatUseTimeMinutes`（`android/frontend/src/account/selfServiceTypes.ts:61-64`，秒→分 floor）、`formatFlowMb`（`android/frontend/src/account/selfServiceTypes.ts:67-71`，KB→M留3位）、`fmtMoney`（`android/frontend/src/account/selfServiceTypes.ts:75-79`，保原始精度）、`localDateStr`（`android/frontend/src/account/selfServiceTypes.ts:82-85`，本地日期防时区偏移）。

### `android/frontend/src/account/types.ts`
- 重导出 `AccountItem`（`android/frontend/src/account/types.ts:4`）；导出 `AccountResult`（`android/frontend/src/account/types.ts:7-13`，字段：success/message?/activeAccount?/config?/displayName?）、`SwitchAccountResult`（`android/frontend/src/account/types.ts:15-20`，字段：success/message?/activeAccount?/config?）、`DeleteAccountResult`（`android/frontend/src/account/types.ts:22-27`，字段同 SwitchAccountResult）、`SaveAccountResult`（`android/frontend/src/account/types.ts:29-34`，字段：success/activeAccount?/config?/message?）。

### `android/frontend/src/account/useAccount.ts`
- 导出 `useAccount`（`android/frontend/src/account/useAccount.ts:14-165`）。模块级 `switchInFlight`（`android/frontend/src/account/useAccount.ts:12`）去重切换。
- `refreshAccounts`（`android/frontend/src/account/useAccount.ts:30-37`）：`api.listAccounts()` -> `setAccounts`。
- `handleAddAccount(name)`（`android/frontend/src/account/useAccount.ts:40-65`）：`saveCurrentAsAccount`，回传 config 整份 `updateConfigLocal`+`clearDirtyFields`+`syncPasswordSaved(config.password===PASSWORD_MASK)`（`android/frontend/src/account/useAccount.ts:40-47`）。
- `handleDeleteAccount(name)`（`android/frontend/src/account/useAccount.ts:67-92`）：修历史缺陷，应用回传 `activeAccount`/`config`（`android/frontend/src/account/useAccount.ts:82-90`）。
- `handleSwitchAccount(name)`（`android/frontend/src/account/useAccount.ts:94-128`）：共享 promise 去重（`android/frontend/src/account/useAccount.ts:94-126`），`activeAccount !== undefined` 判断（`android/frontend/src/account/useAccount.ts:94`）。
- `handleRenameAccount(accountId, displayName)`（`android/frontend/src/account/useAccount.ts:132-157`）：`renameAccount`，返回 boolean 供退内联编辑。

### `android/frontend/src/account/useOperatorBind.ts`
- 导出 `BIND_OPERATOR_NONE = '__none__'`（`android/frontend/src/account/useOperatorBind.ts:25`），类型 `OperatorBindingInfo`（`android/frontend/src/account/useOperatorBind.ts:12`，字段：account/passwordSet）、`BindStatuses`（`android/frontend/src/account/useOperatorBind.ts:13`，Record<'cmcc'|'telecom'|'unicom', OperatorBindingInfo|null>），hook `useOperatorBind(user, operator, passwordSaved, credOverride?)`（`android/frontend/src/account/useOperatorBind.ts:25-220`）。
- 状态：`bindOp` 初值 NONE（`android/frontend/src/account/useOperatorBind.ts:34`），`bindPhone`/`bindSms`/`binding`（`android/frontend/src/account/useOperatorBind.ts:35-37`），`bindInitedRef`（`android/frontend/src/account/useOperatorBind.ts:38`），`bindStatuses`/`statusOpen`/`queryingStatus`/`revealedOp`/`revealedPassword`/`chpwdOpen`（`android/frontend/src/account/useOperatorBind.ts:39-44`）。StrictMode mountedRef（`android/frontend/src/account/useOperatorBind.ts:38,46-51`），明文30s清除 effect（`android/frontend/src/account/useOperatorBind.ts:38-45`），`operator` 初始化 `bindOp` 一次（`android/frontend/src/account/useOperatorBind.ts:38-43`）。
- 凭据派生：`credReady`/`credAccount`/`credPassword`（`android/frontend/src/account/useOperatorBind.ts:75-80`），手动 override 优先，否则回退 user+passwordSaved。
- 就绪：`canBind`（`android/frontend/src/account/useOperatorBind.ts:82-85`，手机号 `/^1\d{10}$/`+短信非空+运营商非NONE），`canQueryStatus`（`android/frontend/src/account/useOperatorBind.ts:87`）。
- `ensureHelloVerified = useHelloGate()`（`android/frontend/src/account/useOperatorBind.ts:93`），`fetchBindStatus`（`android/frontend/src/account/useOperatorBind.ts:93-119`，`getBindStatus` 成功 `setStatusOpen(true)`）。
- `ensureRevealVerified = useHelloGate({ignoreToggle:true})`（`android/frontend/src/account/useOperatorBind.ts:125`），`handleReveal(opValue)`（`android/frontend/src/account/useOperatorBind.ts:125-149`，`revealOperatorCredential`）。
- `handleBindOperator`（`android/frontend/src/account/useOperatorBind.ts:151-181`，`bindOperator` 成功清 `bindSms`+刷状态）。
- `handleChangePassword(newPassword, oldPassword)`（`android/frontend/src/account/useOperatorBind.ts:188-214`，`changeSelfPassword` 成功 `saveConfigDirect({password:newPassword}, false)`+`syncPasswordSaved(true)`）。

## Architecture

模块采用分层架构：UI 层（`AccountPanel.tsx` 及各类 Dialog）协调状态，业务逻辑层（Hooks `useAccount.ts`、`useOperatorBind.ts`）处理 API 交互与并发，验证层（`selfServiceState.ts`）管理生物识别门控，类型层（`types.ts`、`selfServiceTypes.ts`）定义契约。

```text
┌──────────────────────────────────────────────────────┐
│ AccountPanel.tsx (UI 协调入口)                        │
│  ├─ useAccount.ts ────────┐                           │
│  ├─ useOperatorBind.ts ───┤                           │
│  │   └─ selfServiceState.ts (Hello门)                 │
│  ├─ LoginInfoDialog.tsx                             │
│  ├─ BindOperatorDialog.tsx                          │
│  ├─ ChangePasswordDialog.tsx                        │
│  └─ operatorChip.ts                                │
└──────────┬───────────────────────────┬───────────────┘
           │                           │
   ┌───────▼────────┐          ┌───────▼──────────────┐
   │ tauriApiWith   │          │ useConfigStore        │
   │ Retry Wrapper  │          │ (frontend-config)     │
   └────────────────┘          └───────────────────────┘
```

## Data Flow

1. **顶层容器**：`AccountPanel` 是核心协调者（`android/frontend/src/account/AccountPanel.tsx:50`）。
   - 持有 `useOperatorBind` 实例（`bindFlow`）（`android/frontend/src/account/AccountPanel.tsx:239`），传给 `BindOperatorDialog` 的 `bind` prop（`android/frontend/src/account/AccountPanel.tsx:493-498`）；`bindFlow` 解构出的 `handleChangePassword` 传给 `ChangePasswordDialog` 的 `onSave`（`android/frontend/src/account/AccountPanel.tsx:240,501-505`）。
   - 管理 `LoginInfoDialog` 打开/关闭及 `onSave={handleSaveLoginInfo}`（`android/frontend/src/account/AccountPanel.tsx:481-490`）。
   - 通过 `onRenameAccount` prop 处理内联改名（`android/frontend/src/account/AccountPanel.tsx:138-164`）。

2. **账号列表**：`AccountPanel` -> `refreshAccounts` -> `tauriApiWithRetry.listAccounts()` -> `useConfigStore.setAccounts`（`android/frontend/src/account/AccountPanel.tsx:168-175`，`android/frontend/src/account/useAccount.ts:30-37`）。

3. **登录信息保存**：
   - `LoginInfoDialog` -> `onSave`（`AccountPanel.handleSaveLoginInfo`）。
   - Create: `tauriApiWithRetry.createAccount`（`android/frontend/src/account/AccountPanel.tsx:183-188`）。
   - Edit: `useConfigStore.saveConfigDirect` + `tauriApiWithRetry.saveCurrentAsAccount`（`android/frontend/src/account/AccountPanel.tsx:211-219`）。

4. **运营商绑定**：
   - `BindOperatorDialog` -> `bindOp`/`bindPhone`/`bindSms` 状态更新（`android/frontend/src/account/BindOperatorDialog.tsx:41-78`）。
   - 凭据来源：默认取 `config.user` + `passwordSaved`；手动模式经 `onCredChange` -> `AccountPanel.bindCredOverride` -> `useOperatorBind` 的 `credOverride` 参数（`android/frontend/src/account/BindOperatorDialog.tsx:25-29`，`android/frontend/src/account/AccountPanel.tsx:63-67,493-498`）。
   - 操作：`bindFlow.handleBindOperator` / `fetchBindStatus`（`android/frontend/src/account/useOperatorBind.ts:93-119,151-181`）。

5. **密码修改**：
   - `ChangePasswordDialog` -> `onSave(newPwd, oldPwd)` -> `bindFlow.handleChangePassword`（`android/frontend/src/account/useOperatorBind.ts:188-214`），成功后同步本地 `password` 配置。

6. **UI 复用**：`operatorChipClass` 被 `AccountPanel` 渲染卡片调用（`android/frontend/src/account/AccountPanel.tsx:50-55`）；`selfServiceTypes.ts` 格式化工具供自助服务面板消费，类型全局共享。

## Connections

* [[frontend-config]]: 通过 `useConfigStore` 读取和写入用户配置（如 `android/frontend/src/account/AccountPanel.tsx:63`）。
* [[frontend-auth]]: 通过 `selfServiceState.ts` 集成 Windows Hello 生物识别验证（`android/frontend/src/account/selfServiceState.ts:65-87`）。
* [[backend-api]]: 通过 `tauriApiWithRetry` 系列函数调用后端 REST/RPC 接口（如 `android/frontend/src/account/useOperatorBind.ts:93-119`）。

## Known Issues

1. **StrictMode 双重渲染**：`AccountPanel` 与 `useOperatorBind` 均用 `mountedRef` 在二次 setup 恢复 true，避免 dev 双调用下 async setState 被丢弃（`android/frontend/src/account/AccountPanel.tsx:64-71`；`android/frontend/src/account/useOperatorBind.ts:33,46-51`）。
2. **密码清除语义**：后端空密码=保留旧密码，故 `handleClearPassword` 必须显式传 `clearPassword=true` 标志（`android/frontend/src/account/AccountPanel.tsx:100-115`）；清除后 `clearDirtyFields` 避免后续空/MASK 回传脱节。
3. **激活卡密码窗口期**：`saveConfigDirect` 落主配置但档案回写在 `saveCurrentAsAccount` 之后，故 `applyActiveCardPasswordSaved` 先改内存 accounts 使 UI 即时正确（`android/frontend/src/account/AccountPanel.tsx:92-96,217`）。
4. **切换去重**：`useAccount` 模块级 `switchInFlight` 共享 promise，防双击交错落盘（`android/frontend/src/account/useAccount.ts:12,95-127`）。
5. **历史缺陷修复**：删除/切换后必须用 `!== undefined` 判 `activeAccount`（而非真值），否则空串跳过导致高亮不切（`android/frontend/src/account/useAccount.ts:82,114`）；回传掩码 config 须整份落地并清 dirty，否则本地未广播密码挡住真实值（`android/frontend/src/account/useAccount.ts:48-55,83-90,103-110,146-153`）。
6. **绑定凭据隔离**：`credOverride` 手动填写仅本次不保存，弹窗关闭即回收（`android/frontend/src/account/BindOperatorDialog.tsx:68-72`）；`credReady` 判定手动需账号密码非空，否则回退登录信息且要求 `passwordSaved`（`android/frontend/src/account/useOperatorBind.ts:75-77`）。
7. **Hello 验证门**：`VERIFY_TTL_MS=570s` 留 30s 余量对齐后端 600s（`android/frontend/src/account/selfServiceState.ts:66`）；时钟回拨视为过期重验（`android/frontend/src/account/selfServiceState.ts:66-70`）；`ignoreToggle` 时明文特权无视总开关强制验证（`android/frontend/src/account/selfServiceState.ts:50-53`，`android/frontend/src/account/useOperatorBind.ts:124`）。
8. **明文暴露面**：揭示密码 30s 自动清 `revealedPassword` 及 `revealedOp`（`android/frontend/src/account/useOperatorBind.ts:55-62`）。
9. **表单校验**：新密码 6-11 位字母数字（`android/frontend/src/account/ChangePasswordDialog.tsx:28,56-59`）；绑定手机号 `/^1\d{10}$/`（`android/frontend/src/account/useOperatorBind.ts:84`）且输入过滤非数字（`android/frontend/src/account/BindOperatorDialog.tsx:162`）；账号名/显示名经 `isValidDisplayName`（`android/frontend/src/account/LoginInfoDialog.tsx:86`, `android/frontend/src/account/AccountPanel.tsx:152`）。
10. **异常吞没**：`DeviceAliasDialog` 的 `submit` 吞掉 `onSave` 异常仅防 unhandled，错误 toast 由调用方发（`android/frontend/src/account/DeviceAliasDialog.tsx:36-47`）。
11. **列表刷新容错**：`refreshAccounts` 捕获异常仅 dev 下 `console.error`（`android/frontend/src/account/AccountPanel.tsx:168-175`, `android/frontend/src/account/useAccount.ts:30-37`）。
12. **生物识别错误映射**：`biometricFailMessage` 按 reject 的 `code` 翻 i18n，未知回退原始消息（`android/frontend/src/account/selfServiceState.ts:34-38`）。
13. **前端非权威校验**：所有前端校验仅即时反馈，真正拦截以后端 `validate_display_name` 为准（`android/frontend/src/account/validators.ts:2-5`）。
