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
本模块 `android-frontend-account` 负责 Android 端账号面板与自助服务的核心 UI 与逻辑。它涵盖了账号管理（增删改查、切换）、运营商绑定流程、密码修改、在线设备堆叠展示以及上网记录查询等功能。模块采用 React 组件树结构，通过 Tauri API 与后端通信，并利用自定义 Hooks (`useAccount`, `useOperatorBind`) 封装复杂的状态同步与并发控制逻辑。`AccountPanel` 为账号管理主面板聚合根，`SelfServicePanel` 为自助服务 dashboard 聚合容器，`SelfOnlineStack` 提供在线设备堆叠卡，`SelfOnlineRecords` 为纯展示组件。验证层面由 `selfServiceState` 提供 Windows Hello 门控。

## Key Components

### `AccountPanel.tsx`
**职责**：安卓账号页主面板（单列变体），渲染账号管理卡（`:246-416`）与自动化设置卡（`:419-473`），并承载登录信息编辑、运营商绑定、修改密码三个核心弹窗（底部渲染 `LoginInfoDialog` 于 `:476`、`BindOperatorDialog` 于 `:488`、`ChangePasswordDialog` 于 `:496`）。注意：注释声明“自动化设置卡在上、账号管理卡在下”（`:43-45`），但 JSX 实际顺序相反。

*   **导出**：`AccountPanel`（经 `memo` 包裹，`:50`）。
*   **Props 接口** `AccountPanelProps` (`:32-41`)：
    *   `accounts: AccountItem[]`：账号列表数据。
    *   `activeAccount: string`：当前激活的账号 ID。
    *   `onUpdateConfig`：配置更新回调。
    *   `onAddAccount`：添加账号回调。
    *   `onDeleteAccount`：删除账号回调。
    *   `onSwitchAccount`：切换账号回调。
    *   `onRenameAccount`：重命名账号回调。
*   **状态与默认值**：
    *   `useConfigStore` 浅比较取 `config`（`:63`）；`mountedRef` 处理 StrictMode（`:64-71`）。
    *   弹窗A：`loginInfoOpen`/`loginInfoMode`（初始 `'create'`，`:74-75`）。
    *   弹窗B：`bindOpen`/`bindCredOverride`（初始 `null`，`:82-83`）。
    *   防重锁：`switchingAccount`（初始 `null`，`:118`）。
    *   内联改名：`renamingId`/`renameDraft`/`renaming`（`:138-140`），`renameDraft` 输入 `maxLength=64`（`:325`）。
*   **关键方法**：
    *   `applyActiveCardPasswordSaved`（`:92-96`）：直接改 store 中激活卡 `passwordSaved` 内存态。
    *   `handleClearPassword`（`:100-115`）：调 `store.saveConfigDirect({password:''}, true)`（第二参 `true` 为显式清除标志），失败 toast 且不显示已清除；成功调 `syncPasswordSaved(false)`、`applyActiveCardPasswordSaved(false)`、`clearDirtyFields()`（`:113` 防 passwordSaved 脱节）。
    *   `handleSwitchAccount`（`:119-123`）：busy 锁防重复切换。
    *   `handleCardEdit`（`:127-134`）：非激活卡先 `handleSwitchAccount` 再开编辑弹窗。
    *   `startRename`/`cancelRename`/`submitRename`（`:141-164`）：改名校验 `isValidDisplayName`，Enter 提交/Escape 取消。
    *   `refreshAccounts`（`:168-175`）：调 `tauriApiWithRetry.listAccounts()` 后 `setAccounts`。
    *   `handleSaveLoginInfo`（`:180-234`）：create 态调 `createAccount`（不切换激活、返回 config 不 apply，`:183-197`）；edit 态先 `saveConfigDirect`（密码留空=后端保留，`:209`）再 `saveCurrentAsAccount`（`:219`），填了新密码立即 `applyActiveCardPasswordSaved(true)`（`:217`）。
    *   `bindFlow = useOperatorBind(config.user, config.operator, passwordSaved, bindCredOverride)`（`:239`）。
*   **UI 细节**：账号卡用 `OnlineDeviceStack`（`:264`）纵排，卡片内显示在线点、显示名、`operatorChipClass` 运营商色（`:297`）、密码状态 chip；操作行含编辑/改名/切换/删除（`:339-382`）。自动化卡三开关：`autoLoginOnStart`（`:438`）、`autoLoginOnPreparation`（`:451`）、`enableNightOperatorSwitch`（`:464`）。

### `BindOperatorDialog.tsx`
**职责**：弹窗B「绑定运营商」，含顶部凭据分段、原绑定表单、绑定状态弹窗（`:206-275` 内嵌）。

*   **导出**：`BindOperatorDialog`（`:41`）。
*   **类型**：`BindFlow = ReturnType<typeof useOperatorBind>`（`:30`）。
*   **接口** `BindOperatorDialogProps`（`:32-39`）：`open`、`onOpenChange`、`bind: BindFlow`、`onCredChange: (cred: {account,password}|null)=>void`。
*   **状态**：`credMode` 初始 `'login'`（`:51`），`manualAccount`/`manualPassword`/`showManualPassword`（`:52-54`）。
*   **关键逻辑**：
    *   `useEffect` 打开时复位凭据分段（`:57-64`）。
    *   `useEffect` 凭据变化上报父级：`open && credMode==='manual'` 时传手动值，否则 `null`（`:68-72`，关闭回收手动凭据）。
    *   解构 `bind` 状态机字段：`bindOp,setBindOp,bindPhone,setBindPhone,bindSms,setBindSms,binding,bindStatuses,queryingStatus,revealedOp,revealedPassword,statusOpen,setStatusOpen,canBind,canQueryStatus,fetchBindStatus,handleReveal,handleBindOperator`（`:43-48`）。
    *   ISP 选项过滤 `__default__`（`:149,218`）；状态 key 映射 `o.value.slice(1)` 为 `'cmcc'|'telecom'|'unicom'`（`:219`）。
    *   绑定状态弹窗显示掩码账号，reveal 交互按需展示明文（`:228-246`）。

### `ChangePasswordDialog.tsx`
**职责**：修改校园网密码弹窗，旧密码默认取已存（可展开手动，`:84-108`）。

*   **导出**：`ChangePasswordDialog`（`:30`）。
*   **接口** `ChangePasswordDialogProps`（`:21-25`）：`open`、`onSave: (newPassword, oldPassword) => Promise<boolean>`、`onCancel`。
*   **常量**：`NEW_PASSWORD_RE = /^[0-9A-Za-z]{6,11}$/`（`:28`，服务端口径 6–11 位仅数字英文）。
*   **状态**：`oldPassword,showOld,newPassword,confirmPassword,showPassword,error,saving`（`:32-38`）；`newPassword`/`confirmPassword` 输入 `maxLength=11`（`:120,147`）。
*   **关键逻辑**：`useEffect` 由 `open` 驱动复位草稿（`:41-52`）。`submit`（`:54-72`）：`saving` 防双击；校验正则与确认匹配；调 `onSave`，失败保留草稿（父级 toast）。

### `DeviceAliasDialog.tsx`
**职责**：在线设备改名弹窗，编辑 `config.deviceAliases[mac]`。

*   **导出**：`DeviceAliasDialog`（`:25`）。
*   **接口** `DeviceAliasDialogProps`（`:15-23`）：`open`、`mac`（原始 12 位 hex 无分隔，`:18`）、`initialName`、`onSave: (name)=>Promise<void>`、`onCancel`。
*   **状态/逻辑**：`name, saving`（`:27-28`）；`useEffect` open 同步 `initialName`、关闭复位 `saving`（`:31-34`）。`submit`（`:36-47`）：`saving` 防双击；`onSave(name.trim())`；吞掉异常防 unhandled rejection，弹窗保持打开留草稿。输入 `maxLength=30`（`:60`）。

### `LoginInfoDialog.tsx`
**职责**：弹窗A 添加/编辑登录信息。

*   **导出**：`LoginInfoDialog`（`:51`）。
*   **接口** `LoginInfoFormValues`（`:29-34`：`accountName,user,password,operator`）。
*   **接口** `LoginInfoDialogProps`（`:36-49`）：`open`、`onOpenChange`、`mode:'create'|'edit'`、`initial:{user,operator}`、`passwordSaved`、`onSave:(values)=>Promise<boolean>`、`onChangePassword?`、`onClearPassword?`。
*   **状态**：`accountName,user,password,operator,showPassword,error,saving`（`:62-68`）；`accountName` 输入 `maxLength=32`（`:135`）。
*   **关键逻辑**：`useEffect` open 驱动复位预填（密码不回填，`:71-82`）。`submit`（`:84-110`）：create 校验 `isValidDisplayName(accountName)`+`user` 非空+密码非空；`operator` 为 `'__default__'` 转 `''`（`:104`）；调 `onSave` 失败留草稿。`mode==='edit' && passwordSaved` 才显示修改/清除密码入口（`:161-182`）。

### `operatorChip.ts`
**职责**：运营商后缀→品牌色 chip 样式。被 `AccountPanel.tsx:30,297` 引入使用。

*   **导出**：`operatorChipClass(operator: string|null|undefined): string`（`:3-13`）。
*   `@cmcc` 天蓝、`@telecom` 蓝、`@unicom` 橙、其他 `bg-muted text-muted-foreground`。

### `SelfOnlineRecords.tsx`
**职责**：上网记录融合卡。支持“近期流水”和“账单明细”两种视图，提供日期范围查询功能。纯展示组件，接收父级传入的 props 与回调。

*   **导出**：`SessionRow` 接口（`:32-50`）、`recentToSessionRows`（`:52-66`）、`billToSessionRows`（`:68-85`）、`RecordStats` 接口（`:87-92`）、`aggregateStats`（`:94-104`）、`groupByDay`（`:107-120`）、`SelfOnlineRecords`（`:146`）。
*   **依赖**：从 `./selfServiceTypes` 引入 `SelfHistoryRow, SelfLogRow, SelfLogSummary, formatEpoch, formatMac, formatTerminalType, localDateStr, toInt`（`:13-22`）。
*   **辅助**：`fmt2`（`:24`）、`fmtMoney`（`:25-29` 空/非有限返 `'-'`）、`payStyleText`（`:136-141` 1/2/3→时长/流量/包月）、`dayHeading`（`:144`）。
*   **Props** `SelfOnlineRecordsProps`（`:122-134`）：含 `configLoaded, hasCred, history, logStart, logEnd, logQuerying, logRows, logSummary` 及回调。
*   **关键逻辑**：`queried = logRows !== null` 决定查询态/近期态（`:155`）；`sessions` useMemo 转换（`:156-159`）；`groups` 按日分组（`:160`，跳过 `loginTime` 为 0，`:111`）；`stats` 聚合（`:161`）。查询态渲染 8 枚汇总（`:228-248`），近期态 4 枚聚合（`:299-311`）；账单会话卡显 NAS/上下行（`:271-283`），近期卡显 MAC/计费（`:335-342`）。`logRows!.length >= 500` 溢出提示（`:292`）；`history===null` 显 idle 而非 empty（`:352`）。

### `index.ts`
**职责**：模块入口，统一导出公共组件和 Hook。

*   **导出语句**：`AccountPanel`（`:1`）、`ChangePasswordDialog`（`:2`）、`useAccount`（`:3`）、`export * from './types'`（`:5`）。

### `SelfOnlineStack.tsx`
**职责**：安卓端在线设备堆叠卡（竖排封面流 `OnlineDeviceStack` 与平板水平堆叠 `OnlineDeviceStackH`）及单卡内容 `OnlineDeviceStackCard`，实现手势翻页、连续滚动动画与多卡槽位管理。

*   **导出与常量**：
    *   `OnlineDeviceStackProps`（`:43`）接口：`cards: React.ReactNode[]`、`variant?: 'coverflow' | 'narrow'`、`focusIndex?: number`。
    *   `OnlineDeviceStack`（`:50`）竖排堆叠主组件。常量：`PEEK_TOP=44`（`:14`）、`PEEK_BOTTOM=44`（`:15`）、`DRAG_THRESHOLD=60`（`:16`）、`DRAG_MIN=-110`（`:18`）、`DRAG_MAX=70`（`:19`）、`SLIDE_MS=500`（`:20`）、`EASE='cubic-bezier(0.22, 1, 0.36, 1)'`（`:21`）、`V_SIDE_SCALE=0.75`（`:23`）、`V_CARD_INSET='inset-x-[6%]'`（`:27`）、`CF_X=[60,85]`（`:32`）、`CF_INSET='27%'`（`:33`）、`SIDE_SCALE=0.6`（`:34`）、`SIDE_SCALE_2=0.45`（`:35`）、`DRAG_CLAMP=80`（`:36`）、`NARROW_MIN_W=213`（`:41`）。
    *   `OnlineDeviceStackCardProps`（`:233`）接口：含 `loginTime, ip, mac, flowMb, durationMin, terminalType, alias?, onRename?, offlineDisabled?, offlineLoading?, onOffline?`。
    *   `OnlineDeviceStackCard`（`:250`）单卡渲染：绿点+时间头、别名/终端标签/铅笔按钮、IP/MAC 垂直芯片、流量/时长、注销按钮（受 `offlineDisabled`/`offlineLoading` 控制）。
    *   `OnlineDeviceStackH`（`:342`）水平堆叠（平板），参数同 `OnlineDeviceStackProps`，内部用 `dragX`（`:346`）、`sd` 计算（`:377`）、姿势表含左右相邻/次邻（`:455-464`）、`narrow` 定位 `left:50%` + `minWidth:213`（`:469-489`）。
*   **关键实现**：
    *   状态 `index/dragY/dragging/cardH`（`:52-55`）与 refs `stageRef/drag/suppressClick`（`:56-58`）。
    *   列表变短夹页码（`:61-63`）；`focusIndex` 变化滚中央（`:66-69`）；`ResizeObserver` 量最大自然高，无则退化一次性（`:74-89`）。
    *   `count===0` 返 null（`:91`）；两卡无底露出（`:93`）；`rel` 取模（`:94`）；`go` 翻页（`:98-102`）。
    *   手势 `onPointerDown/Move/endDrag/onClick`（`:104-150`）：单卡禁手势（`:105,140`）、拖拽 clamp（`:120`）、`suppressClick` 防松手误触（`:129-137`）、中线分区点击（`:148`）。
    *   姿势表渲染绝对定位（`:172-211`），页码点（`:213-227`）。
    *   水平版裸 `translateX` 求和须 `calc` 包装（`:473-478`）。

### `SelfServicePanel.tsx`
**职责**：自助服务 dashboard 面板，聚合在线设备堆叠、上网记录、注销确认弹窗、设备改名弹窗，统一凭据取自登录信息。

*   **内部类型与导出工具**（部分与 selfServiceTypes 重复定义）：
    *   `SelfOnlineItem` 接口（`:26-36`）：`loginTime, ip, mac, useTime, downFlow, upFlow, hostName, terminalType, sessionId`。
    *   `toInt`（`:39`）、`formatTerminalType`（`:45`）、`formatUseTimeMinutes`（`:51` export）、`formatFlowMb`（`:57` export）、`localDateStr`（`:65` export）。
*   **主组件 `SelfServicePanel`**（`:70`）：
    *   状态：`onlineList, formFactor, history, querying, offlineSessionId, confirmTarget, aliasTarget, deviceAliases, logStart/End`（默认今本地日期）, `logRows, logSummary, logQuerying, mountedRef, autoRefreshedRef, configLoaded`（`:75-97`）。
    *   `hasCred`（`:110`）= `configUser.trim() && passwordSaved`。
    *   `fetchDashboard`（`:112`）调 `tauriApiWithRetry.querySelfDashboard`，`silent` 降级不弹错（`:126-129`）。
    *   切入自动验证刷新 effect（`:142-148`）依赖 `configLoaded` 防早判。
    *   `handleOffline`（`:150`）调 `selfOfflineSession`，防并发（`:151`），成功过滤列表（`:163`）。
    *   `handleSaveAlias`（`:178`）写 `config.deviceAliases[mac]` 留空删键（`:181-182`）。
    *   `fetchOnlineLog`（`:197`）调 `querySelfOnlineLog`。
    *   渲染：`formFactor==='tablet'` 用 `OnlineDeviceStackH` 否则 `OnlineDeviceStack`（`:268-306`），`loginTime` 截取 `HH:mm:ss`（`:273,292`），传格式化 props。

### `selfServiceState.ts`
**职责**：Windows Hello 验证门（绑定操作门 + 自助面板会话门）及生物错误文案映射。

*   **导出/模块成员**：
    *   `helloEnabled`（`:10`）内部判断 `config.selfHelloEnabled !== false`。
    *   `BIOMETRIC_ERROR_KEY`（`:19-32`）错误码映射（含 face 业务码）。
    *   `biometricFailMessage`（`:34`）按 `code` 翻译。
    *   `VERIFY_TTL_MS = 570_000`（`:54`）对齐后端 600s 留 30s 余量。
    *   `gateFresh`（`:58`）TTL 内且时钟未回拨（`elapsed>=0`）。
    *   `helloGateVerifiedAt`（`:63`）模块级；`selfSessionVerifiedAt`（`:97`）模块级。
    *   `useHelloGate`（`:65`）绑定门 hook，支持 `ignoreToggle` 强制真实验证（`:71`）。
    *   `resetSelfSessionGate`（`:100`）重置会话门。
    *   `useSelfServiceVerify`（`:105`）面板会话门，支持 `selfReverifyEachAction`（`:110`）。

### `selfServiceTypes.ts`
**职责**：自助服务共享类型与格式化工具（Panel/Records/测试共用）。

*   **导出类型**：`SelfHistoryRow`（`:5` 元组）、`SelfLogRow`（`:9`）、`SelfLogSummary`（`:25` 大写键）。
*   **导出格式化**：`formatMac`（`:36` 归一化 hex 配对）、`formatEpoch`（`:42`）、`formatTerminalType`（`:50`）、`toInt`（`:55`）、`formatUseTimeMinutes`（`:61` floor 秒转分）、`formatFlowMb`（`:67` KB→M 留3位）、`fmtMoney`（`:75` 保留原始精度）、`localDateStr`（`:82` 本地日期防 UTC 偏移）。

### `types.ts`
**职责**：账号命令返回类型。

*   **导出**：`AccountItem` 复用（`:4`）、`AccountResult`（`:7`）、`SwitchAccountResult`（`:15`）、`DeleteAccountResult`（`:22`）、`SaveAccountResult`（`:29`）。

### `useAccount.ts`
**职责**：账号管理 hook（增/删/切/改名），与后端交互并同步 store。

*   **模块变量**：`switchInFlight`（`:12`）并发切换去重。
*   **导出 `useAccount`**（`:14`）内含：
    *   `refreshAccounts`（`:30`）调 `api.listAccounts`。
    *   `handleAddAccount`（`:40`）调 `saveCurrentAsAccount`，掩码 config 落地（`:52-54`）。
    *   `handleDeleteAccount`（`:67`）调 `deleteAccount`，修复历史缺陷应用 `activeAccount/config`（`:82-89`）。
    *   `handleSwitchAccount`（`:94`）调 `switchAccount`，`switchInFlight` 去重（`:95,122-127`），`!== undefined` 判活（`:114`）。
    *   `handleRenameAccount`（`:132`）调 `renameAccount` 仅改显示名。
    *   所有写回均 `updateConfigLocal`+`clearDirtyFields`+`syncPasswordSaved(PASSWORD_MASK比较)` 防串号。

### `useOperatorBind.ts`
**职责**：运营商绑定流 hook（绑定/查询/揭示/改密）。

*   **导出**：`BIND_OPERATOR_NONE='__none__'`（`:9`）、`OperatorBindingInfo`（`:12`）、`BindStatuses`（`:13`）、`useOperatorBind`（`:25` 入参 `user, operator, passwordSaved, credOverride?`）。
*   **状态**：`bindOp/Phone/Sms/Statuses/statusOpen/revealedOp/revealedPassword` 等（`:34-44`）。
*   **关键**：明文 30s 清除 effect（`:55-62`）；init 默认运营商（`:65-70`）；`credReady`（`:75` 手动优先）；`canBind`（`:82` 手机号 `/^1\d{10}$/`）；`ensureHelloVerified=useHelloGate()`（`:91`）；`fetchBindStatus`（`:93`）成功 `setStatusOpen(true)`；`ensureRevealVerified=useHelloGate({ignoreToggle:true})`（`:124`）；`handleReveal`（`:125`）调 `revealOperatorCredential`；`handleBindOperator`（`:151`）成功调 `fetchBindStatus`；`handleChangePassword`（`:188`）调 `changeSelfPassword` 成功 `saveConfigDirect` 同步密码。

### `validators.ts`
**职责**：改名校验（与后端 `validate_display_name` 对齐）。

*   **导出**：`DISPLAY_NAME_MAX_LEN=32`（`:9`）、`CONTROL_CHARS_RE`（`:13` 含 C0/C1/DEL）、`isValidDisplayName`（`:16` 码点计长、禁控符）。

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                 SelfServicePanel.tsx                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │OnlineDevice  │  │SelfOnline     │  │DeviceAliasDialog│ │
│  │Stack         │  │Records        │  │ConfirmDialog   │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬────────┘  │
│         └──────────┬───────┴──────────────────┘           │
│                    ▼                                        │
│  ┌────────────────────────────────────────────────────┐  │
│  │         useSelfServiceVerify() (selfServiceState)  │  │
│  └───────────────────────┬────────────────────────────┘  │
│                          ▼                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │ AccountPanel.tsx & useAccount.ts / useOperatorBind │  │
│  └───────────────────────┬────────────────────────────┘  │
│                          ▼                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │              selfServiceState.ts                   │  │
│  │  (Global TTL Gates: useHelloGate, etc.)            │  │
│  └───────────────────────┬────────────────────────────┘  │
│                          ▼                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │            Tauri API Layer (tauriApiWithRetry)     │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化与配置加载**:
    *   `SelfServicePanel` 从 `useConfigStore` 获取 `configUser`, `passwordSaved`, `deviceAliases`（`:77-97`）。
    *   配置加载且凭据就绪后，自动触发 `fetchDashboard`（`:142-148`）。
    *   `AccountPanel` 自订阅 store（`:63`）并处理 StrictMode（`:64-71`）。

2.  **数据获取与展示**:
    *   `SelfServicePanel` 调用 `tauriApiWithRetry.querySelfDashboard` 获取在线设备和历史记录（`:112`）。
    *   数据经过 `formatTerminalType`（`:45`）、`formatUseTimeMinutes`（`:51`）、`formatFlowMb`（`:57`）等工具函数格式化。
    *   在线设备数据传递给 `OnlineDeviceStack` 或 `OnlineDeviceStackH` 进行堆叠渲染（`:268-306`）。
    *   历史记录数据传递给 `SelfOnlineRecords`，通过 `recentToSessionRows`（`:52-66`）或 `billToSessionRows`（`:68-85`）归一化为 `SessionRow` 进行展示。
    *   `SelfOnlineRecords` 的 `groupByDay`（`:107-120`）和 `aggregateStats`（`:94-104`）用于视图聚合。

3.  **用户交互与操作**:
    *   **踢下线**: 用户点击注销按钮 -> `handleOffline`（`:150`） -> 调用 `useSelfServiceVerify()` 进行生物验证 -> 调用 `selfOfflineSession` API -> 成功后从 `onlineList` 中移除该设备（`:163`）。
    *   **改名**: 用户点击改名 -> 弹出 `DeviceAliasDialog`（`:25`） -> 输入新名称 -> 调用 `handleSaveAlias`（`:178`） -> 更新 `config.deviceAliases`（`:181-182`）。
    *   **运营商绑定**: 用户点击绑定 -> 弹出 `BindOperatorDialog`（`:41`） -> 选择凭据模式 (`login`/`manual`) -> 输入手机号/验证码 -> 调用 `useOperatorBind` 中的 `handleBindOperator`（`:151`） -> 涉及 `useHelloGate` 生物验证 -> 调用 `bindOperator` API。
    *   **账号管理**: 用户编辑账号信息 -> `LoginInfoDialog`（`:51`） -> `handleSaveLoginInfo`（`:180-234`） -> 调用 `createAccount` 或 `saveCurrentAsAccount` -> 更新 Store。

4.  **生物验证门控**:
    *   所有敏感操作（如踢下线、查询绑定状态、揭示明文密码）均需通过 `useSelfServiceVerify` 或 `useHelloGate`。
    *   `useHelloGate` 维护全局 TTL (`VERIFY_TTL_MS = 570_000`，`:54`)，并处理时钟回拨防护（`:58`）。
    *   揭示明文密码 (`handleReveal`，`:125`) 强制要求验证，即使总开关关闭（`useHelloGate({ignoreToggle:true})`，`:124`）。

## Connections

*   **[[android-frontend-settings]]**: `useAccount.ts` 中的 `handleSwitchAccount` 等方法被 Settings 模块调用，用于全局账号切换。
*   **[[android-frontend-dashboard]]**: `SelfOnlineRecords` 接收来自 Dashboard 的 `history` 数据（`:155`），表明两者存在数据依赖。
*   **[[tauri-backend-account]]**: 所有 API 调用 (`tauriApiWithRetry`) 指向后端提供的账号管理、运营商绑定、自助服务等接口。

## Known Issues

1.  **StrictMode 副作用安全**: `AccountPanel` 使用 `mountedRef` 跟踪组件挂载状态（`:64-71`），所有异步回调（`:131,159,189,199,213,220,226`）均检查 `mountedRef.current`，以防止 React 18 StrictMode 双重调用导致的无效状态更新。同样模式见于 `SelfServicePanel`（`:92,121` 等）。
2.  **密码清除语义**: 后端约定空字符串密码表示“保留旧密码”。因此，显式清除密码必须走 `handleClearPassword`（`:100-115`），它调用 `saveConfigDirect({ password: '' }, true)` 并设置 `syncPasswordSaved(false)` 及 `clearDirtyFields()`（`:113`）。如果直接留空保存，不会清除密码。
3.  **切换账号防重入**: `handleSwitchAccount`（`:119-123`）检查 `switchingAccount !== null`，防止在切换进行中再次触发操作；改名/编辑/删除按钮在 `renamingId!==null` 或 `switchingAccount!==null` 时 disabled（`:345,355,366,377`）。
4.  **编辑非激活账号的逻辑**: 点击非激活卡的“编辑”按钮时，`handleCardEdit`（`:127-134`）会先尝试切换到该账号（`handleSwitchAccount`），切换成功后再打开编辑弹窗。这确保了编辑的上下文正确。
5.  **运营商凭据覆盖机制**: `BindOperatorDialog` 允许用户在绑定时手动输入账号密码（`credMode === 'manual'`，`:51`）。这些凭据仅在本次绑定中使用，通过 `onCredChange`（`:68-72`）传递给 `useOperatorBind` 的 `credOverride`（`:83`），不会保存到本地存储。关闭弹窗或切换回 `login` 模式时，凭据被清空回传 `null`。
6.  **密码修改对话框的旧密码处理**: 默认情况下，旧密码由后端 resolve（使用已保存的登录密码）。用户可以选择展开手动输入旧密码框（`:84-108`），此时手动输入的旧密码优先于已存密码。`ChangePasswordDialog` 新密码正则限制 `/^[0-9A-Za-z]{6,11}$/`（`:28`）。
7.  **上线记录的数据归一化**: `SelfOnlineRecords` 将两种不同来源的数据（Dashboard 历史数据和账单明细）统一转换为 `SessionRow` 接口，使得后续的分组和统计逻辑可以复用（`:52-85`）。`logRows` 为空数组仍判为 `queried` 显查询态（`:155`）。
8.  **设备别名保存的错误吞没**: `DeviceAliasDialog` 的 `submit` 函数中（`:36-47`），catch 块吞掉了错误，因为错误提示已由调用方（`handleSaveAlias`）发出。这是为了防止 unhandled rejection，同时保持弹窗打开以便重试（`:41-44`）。
9.  **时钟回拨死锁防护**: `selfServiceState.ts` 中，`gateFresh`（`:58`）不仅检查 TTL，还检查 `elapsed >= 0`。若系统时钟回拨，前端视为过期，强制重新验证，避免前端缓存永真而后端已过期的死锁。
10. **明文密码安全**: 查看明文密码 (`reveal`) 必须强制验证，即使 `selfHelloEnabled` 为 false（`:124`）。明文密码在 state 中保留 30 秒自动清除（`useOperatorBind.ts` 的 effect，`:55-62`）。
11. **并发切换去重**: `useAccount.ts` 中 `switchInFlight`（`:12`）promise 机制确保同一时刻只有一个 switch 请求在执行，后续请求等待其结果，避免状态交错（`:95,122-127`）。
12. **假阴性判断**: `useAccount.ts` 切换账号时（`:94`），后端可能返回空字符串作为 `activeAccount`。代码使用 `result?.activeAccount !== undefined` 而非真值判断（`:114`），确保能正确处理空字符串情况，避免 UI 不更新。
13. **时间格式化一致性**: `SelfServicePanel.tsx` 的 `formatUseTimeMinutes`（`:51`）与 `selfServiceTypes.ts` 的 `formatUseTimeMinutes`（`:61`）均使用 `Math.floor(seconds / 60)` 而非 `round`，严格对齐原站行为。
14. **触控冲突隔离**: `SelfOnlineStack.tsx` 堆叠容器设置 `touch-action:none`（`:13,163,440`），防止与 WebView 页面滚动冲突。
15. **静默降级**: `SelfServicePanel.tsx` 中 `fetchDashboard`（`:112`）在非手动触发（如自动刷新）且失败时，不弹出错误 Toast（`:126-129`），仅在手动刷新失败时提示，提升用户体验。
16. **JSdom 兼容性**: `SelfOnlineStack.tsx` 中 `setPointerCapture` 和 `ResizeObserver` 在测试环境（jsdom）中可能缺失（`:85,111`），代码做了 try-catch 或 typeof 检查以优雅降级。
17. **注释与实现顺序不符**: `AccountPanel` 注释（`:43-45`）称自动化卡在上、账号管理卡在下，实际 JSX 账号卡先渲染（`:246` vs `:419`）。
18. **获取高度防跳变**: `SelfOnlineStack` 量高取最大自然高（`:74-89`），防 WebView 中跳变。
19. **格式化对齐后端**: `formatMac` 剥非 hex 再配对（`selfServiceTypes.ts:36`）；`fmtMoney` 保留原始精度防 `toInt` 截 0（`selfServiceTypes.ts:75`）；`localDateStr` 用本地时区防 UTC 偏移（`selfServiceTypes.ts:82`）。
20. **校验器码点计长**: `isValidDisplayName` 按 Unicode 码点计长（非 UTF-16），与后端 `validate_display_name` 同构（`validators.ts:16`）。