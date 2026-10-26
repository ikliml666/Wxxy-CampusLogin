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
  - android/frontend/src/account/VpnCredentialDialog.tsx
  - android/frontend/src/account/VpnManageDialog.tsx
  - android/frontend/src/account/selfServiceState.ts
  - android/frontend/src/account/selfServiceTypes.ts
  - android/frontend/src/account/types.ts
  - android/frontend/src/account/useAccount.ts
  - android/frontend/src/account/useOperatorBind.ts
  - android/frontend/src/account/validators.ts
tags: [account, self-service, operator-bind, biometric-auth, tauri]
---

## Overview
本模块 `android-frontend-account` 负责 Android 端账号面板与自助服务的核心 UI 与逻辑。涵盖账号管理（增删改查、切换、改名）、运营商绑定流程、密码修改、在线设备堆叠展示以及上网记录查询等功能。模块采用 React 组件树结构，通过 Tauri API（`tauriApiWithRetry`）与后端通信，并利用自定义 Hooks（`useAccount`、`useOperatorBind`、`useHelloGate`、`useSelfServiceVerify`）封装复杂的状态同步、并发控制与生物验证门控逻辑。`AccountPanel` 为账号管理主面板聚合根（单列变体，行50-509），`SelfServicePanel` 为自助服务 dashboard 聚合容器（行80-480），`SelfOnlineStack`/`SelfOnlineStackH` 提供在线设备堆叠卡，`SelfOnlineRecords` 为纯展示组件。验证层面由 `selfServiceState` 提供 Windows Hello 门控（绑定操作门与自助面板会话门）。注意 `SelfServicePanel` 引用了 `VpnCredentialDialog`/`VpnManageDialog`（行7-8 import，行456-470 使用），相关 VPN 入口仅提及不展开。

## Key Components

### `AccountPanel.tsx`
**职责**：安卓账号页主面板（单列变体），实际渲染顺序为账号管理卡（行246-421，stagger-i=0 在上）、自动化设置卡（行424-478，stagger-i=1 在下），并承载登录信息编辑（弹窗A `LoginInfoDialog` 行481-490）、运营商绑定（弹窗B `BindOperatorDialog` 行493-498）、修改密码（`ChangePasswordDialog` 行501-505）三个核心弹窗。文件头注释（行43-49）声称“自动化设置卡在上、账号管理卡在下”，与 JSX 实际顺序（行246 账号卡先于行424 自动化卡）相反（见 Known Issues 17）。

*   **导出**：`AccountPanel`（`memo` 包裹，行50-509）。
*   **Props 接口** `AccountPanelProps`（行32-41）：
    *   `accounts: AccountItem[]`（行33）账号列表。
    *   `activeAccount: string`（行34）当前激活账号 ID。
    *   `onUpdateConfig: (partial: Partial<Config>) => void`（行35）。
    *   `onAddAccount: (name: string) => Promise<boolean>`（行36，定义但未解构使用：行50-57 的解构列表不含此 prop，创建账号由弹窗A内部走 `createAccount` 代替）。
    *   `onDeleteAccount: (id: string, displayName: string) => void`（行37，行373 使用）。
    *   `onSwitchAccount: (name: string) => Promise<void>`（行38，行119-123 封装）。
    *   `onRenameAccount: (accountId: string, displayName: string) => Promise<boolean>`（行40，行158 使用）。
*   **状态与默认值**：
    *   `useConfigStore` 浅比较取 `config`（行63）；`passwordSaved`（行59）；`mountedRef` 处理 StrictMode（行64-71）。
    *   弹窗A：`loginInfoOpen`/`loginInfoMode`（初始 `'create'`，行74-75）。
    *   弹窗B：`bindOpen`/`bindCredOverride`（初始 `null`，行82-83）。
    *   防重锁：`switchingAccount`（初始 `null`，行118）。
    *   内联改名：`renamingId`/`renameDraft`/`renaming`（行138-140），`renameDraft` 输入 `maxLength=64`（行322）。
*   **关键方法**：
    *   `applyActiveCardPasswordSaved`（行92-96）：直接改 store 中激活卡 `passwordSaved` 内存态。
    *   `handleClearPassword`（行100-115）：调 `store.saveConfigDirect({password:''}, true)`（第二参 `true` 为显式清除标志，行103），失败 toast 且不显示已清除；成功调 `syncPasswordSaved(false)`（行108）、`applyActiveCardPasswordSaved(false)`（行109）、`clearDirtyFields()`（行113 防 passwordSaved 脱节）。
    *   `handleSwitchAccount`（行119-123）：busy 锁防重复切换（`switchingAccount !== null` 忽略）。
    *   `handleCardEdit`（行127-134）：非激活卡先 `handleSwitchAccount` 再开编辑弹窗。
    *   `startRename`/`cancelRename`/`submitRename`（行141-164）：改名校验 `isValidDisplayName`，Enter 提交 / Escape 取消。
    *   `refreshAccounts`（行168-175）：调 `tauriApiWithRetry.listAccounts()` 后 `setAccounts`。
    *   `handleSaveLoginInfo`（行180-234）：create 态调 `createAccount`（不切换激活、返回 config 不 apply，行183-197）；edit 态先 `saveConfigDirect`（密码留空=后端保留，行209）再 `saveCurrentAsAccount`（行219），填了新密码立即 `applyActiveCardPasswordSaved(true)`（行217）。
    *   `bindFlow = useOperatorBind(config.user, config.operator, passwordSaved, bindCredOverride)`（行239）。
*   **UI 细节**：账号卡用 `OnlineDeviceStack`（行261）纵排，卡片内显示在线点、显示名、`operatorChipClass` 运营商色（行294）、密码状态 chip（行298-305）；操作行含编辑/改名/切换/删除（行337-378，按钮在 `renamingId!==null` 或 `switchingAccount!==null` 时 disabled）。自动化卡三开关：`autoLoginOnStart`（行446）、`autoLoginOnPreparation`（行459）、`enableNightOperatorSwitch`（行472）。

### `BindOperatorDialog.tsx`
**职责**：弹窗B「绑定运营商」，含顶部凭据分段（行84-140）、原绑定表单（行142-179）、绑定状态弹窗（行206-275 内嵌）。

*   **导出**：`BindOperatorDialog`（行41-278）。
*   **类型**：`BindFlow = ReturnType<typeof useOperatorBind>`（行30）。
*   **接口** `BindOperatorDialogProps`（行32-39）：`open: boolean`、`onOpenChange: (open: boolean) => void`、`bind: BindFlow`（useOperatorBind 整包返回）、`onCredChange: (cred: { account: string; password: string } | null) => void`。
*   **状态**：`credMode` 初始 `'login'`（行51），`manualAccount`/`manualPassword`/`showManualPassword`（行52-54）。
*   **关键逻辑**：
    *   `useEffect` 打开时复位凭据分段（行57-64）。
    *   `useEffect` 凭据变化上报父级：`open && credMode==='manual'` 时传手动值，否则 `null`（行68-72，关闭回收手动凭据）。
    *   解构 `bind` 状态机字段（行43-48）：`bindOp, setBindOp, bindPhone, setBindPhone, bindSms, setBindSms, binding, bindStatuses, queryingStatus, revealedOp, revealedPassword, statusOpen, setStatusOpen, canBind, canQueryStatus, fetchBindStatus, handleReveal, handleBindOperator`。
    *   ISP 选项过滤 `__default__`（行149, 218）；状态 key 映射 `o.value.slice(1)` 为 `'cmcc'|'telecom'|'unicom'`（行219）。
    *   绑定状态弹窗显示掩码账号，reveal 交互按需展示明文（行228-246）。

### `ChangePasswordDialog.tsx`
**职责**：修改校园网密码弹窗，旧密码默认取已存（可展开手动，行84-108）。

*   **导出**：`ChangePasswordDialog`（行30-166）。
*   **接口** `ChangePasswordDialogProps`（行21-25）：`open: boolean`、`onSave: (newPassword: string, oldPassword: string) => Promise<boolean>`、`onCancel: () => void`。
*   **常量**：`NEW_PASSWORD_RE = /^[0-9A-Za-z]{6,11}$/`（行28，服务端口径 6–11 位仅数字英文）。
*   **状态**：`oldPassword, showOld, newPassword, confirmPassword, showPassword, error, saving`（行32-38）；`newPassword`/`confirmPassword` 输入 `maxLength=11`（行120, 147）。
*   **关键逻辑**：`useEffect` 由 `open` 驱动复位草稿（行41-52）。`submit`（行54-72）：`saving` 防双击；校验正则与确认匹配；调 `onSave`，失败保留草稿（父级 toast）。

### `DeviceAliasDialog.tsx`
**职责**：在线设备改名弹窗，编辑 `config.deviceAliases[mac]`（键为原始 12 位 hex MAC，行13-23）。

*   **导出**：`DeviceAliasDialog`（行25-75）。
*   **接口** `DeviceAliasDialogProps`（行15-23）：`open: boolean`、`mac: string`（绑定的原始 12 位 hex MAC，无分隔）、`initialName: string`（当前别名草稿初值）、`onSave: (name: string) => Promise<void>`、`onCancel: () => void`。
*   **状态/逻辑**：`name, saving`（行27-28）；`useEffect` open 同步 `initialName`、关闭复位 `saving`（行31-34）。`submit`（行36-47）：`saving` 防双击；`onSave(name.trim())`；吞掉异常防 unhandled rejection，弹窗保持打开留草稿。输入 `maxLength=30`（行60）。

### `LoginInfoDialog.tsx`
**职责**：弹窗A 添加/编辑登录信息（与桌面同构，安卓无主网卡字段）。

*   **导出**：`LoginInfoDialog`（行51-239）。
*   **接口** `LoginInfoFormValues`（行29-34）：`accountName: string`、`user: string`、`password: string`、`operator: string`。
*   **接口** `LoginInfoDialogProps`（行36-49）：`open: boolean`、`onOpenChange: (open: boolean) => void`、`mode: 'create' | 'edit'`、`initial: { user: string; operator: string }`、`passwordSaved: boolean`、`onSave: (values: LoginInfoFormValues) => Promise<boolean>`、`onChangePassword?: () => void`、`onClearPassword?: () => void`。
*   **状态**：`accountName, user, password, operator, showPassword, error, saving`（行62-68）；`accountName` 输入 `maxLength=32`（行135）。
*   **关键逻辑**：`useEffect` open 驱动复位预填（密码不回填，行71-82）。`submit`（行84-110）：create 校验 `isValidDisplayName(accountName)`+`user` 非空+密码非空；`operator` 为 `'__default__'` 转 `''`（行104）；调 `onSave` 失败留草稿。`mode==='edit' && passwordSaved` 才显示修改/清除密码入口（行161-182）。

### `operatorChip.ts`
**职责**：运营商后缀→品牌色 chip 样式。被 `AccountPanel.tsx` 行30, 294 引入使用。

*   **导出**：`operatorChipClass(operator: string | null | undefined): string`（行3-13）。
*   `@cmcc` 天蓝（`bg-sky-500/10 text-sky-600 dark:text-sky-400`）、`@telecom` 蓝（`bg-blue-500/10 text-blue-600 dark:text-blue-400`）、`@unicom` 橙（`bg-orange-500/10 text-orange-600 dark:text-orange-400`）、其他 `bg-muted text-muted-foreground`（行5-12）。

### `SelfOnlineRecords.tsx`
**职责**：上网记录融合卡（安卓）。支持“近期流水”和“账单明细”两种视图，提供日期范围查询功能。纯展示组件，接收父级传入的 props 与回调。

*   **导出**：
    *   `SessionRow` 接口（行32-50）：字段 `key: string`、`mode: 'recent' | 'bill'`、`loginTime: number`、`logoutTime: number`、`timeMin: number | null`、`flowMb: number | null`、`costMoney: number | null`、`ip: string`、`mac?: string | null`、`terminalType?: string | null`、`payStyle?: number | null`、`intlUp?: number | null`、`intlDown?: number | null`、`cnUp?: number | null`、`cnDown?: number | null`、`nasIp?: string | null`、`nasPort?: string | number | null`。
    *   `recentToSessionRows(history: SelfHistoryRow[]): SessionRow[]`（行52-66）。
    *   `billToSessionRows(rows: SelfLogRow[]): SessionRow[]`（行68-85）。
    *   `RecordStats` 接口（行87-92）：`timeMin: number`、`flowMb: number`、`costMoney: number`、`count: number`。
    *   `aggregateStats(rows: SessionRow[]): RecordStats`（行94-104）。
    *   `groupByDay(rows: SessionRow[]): Array<{ day: string; rows: SessionRow[] }>`（行107-120）。
    *   `SelfOnlineRecords`（行146-360）。
*   **依赖**：从 `./selfServiceTypes` 引入 `SelfHistoryRow, SelfLogRow, SelfLogSummary, formatEpoch, formatMac, formatTerminalType, localDateStr, toInt`（行13-22）。
*   **辅助**：`fmt2`（行24）、`fmtMoney`（行25-29 空/非有限返 `'-'`）、`payStyleText`（行136-141 1/2/3→时长/流量/包月）、`dayHeading`（行144）。
*   **Props** `SelfOnlineRecordsProps`（行122-134）：含 `configLoaded, hasCred, history, logStart, logEnd, logQuerying, logRows, logSummary` 及回调 `onLogStartChange, onLogEndChange, onQuery`。
*   **关键逻辑**：`queried = logRows !== null` 决定查询态/近期态（行155）；`sessions` useMemo 转换（行156-159）；`groups` 按日分组（行160，跳过 `loginTime` 为 0）；`stats` 聚合（行161）。查询态渲染 8 枚汇总（行228-248），近期态 4 枚聚合（行299-311）；账单会话卡显 NAS/上下行（行272-283），近期卡显 MAC/计费（行336-342）。`logRows!.length >= 500` 溢出提示（行292）；`history===null` 显 idle 而非 empty（行352）。

### `index.ts`
**职责**：模块入口，统一导出公共组件和 Hook。

*   **导出语句**：`AccountPanel`（行1）、`ChangePasswordDialog`（行2）、`useAccount`（行3）、`export * from './types'`（行5）。注意未导出 `LoginInfoDialog`、`BindOperatorDialog`、`SelfOnlineRecords` 等，它们由 `AccountPanel`/`SelfServicePanel` 内部直接引入。

### `SelfOnlineStack.tsx`
**职责**：安卓端在线设备堆叠卡（竖排封面流 `OnlineDeviceStack` 与平板水平堆叠 `OnlineDeviceStackH`）及单卡内容 `OnlineDeviceStackCard`，实现手势翻页、连续滚动动画与多卡槽位管理。

*   **导出与常量**：
    *   `OnlineDeviceStackProps`（行43-48）接口：`cards: React.ReactNode[]`、`variant?: 'coverflow' | 'narrow'`（注：竖排 `OnlineDeviceStack` 函数签名行50 仅解构 `cards, focusIndex`，未使用 `variant`）、`focusIndex?: number`（外部焦点滚中央）。
    *   `OnlineDeviceStack`（行50-230）竖排堆叠主组件。常量：`PEEK_TOP=44`（行14）、`PEEK_BOTTOM=44`（行15）、`DRAG_THRESHOLD=60`（行16）、`DRAG_MIN=-110`（行18）、`DRAG_MAX=70`（行19）、`SLIDE_MS=500`（行20）、`EASE='cubic-bezier(0.22, 1, 0.36, 1)'`（行21）、`V_SIDE_SCALE=0.75`（行23）、`V_CARD_INSET='inset-x-[6%]'`（行27）、`CF_X=[60,85]`（行32）、`CF_INSET='27%'`（行33）、`SIDE_SCALE=0.6`（行34）、`SIDE_SCALE_2=0.45`（行35）、`DRAG_CLAMP=80`（行36）、`NARROW_MIN_W=213`（行41）。
    *   `OnlineDeviceStackCardProps`（行233-248）接口：含 `loginTime, ip, mac, flowMb, durationMin, terminalType, alias?, onRename?, offlineDisabled?, offlineLoading?, onOffline?`。
    *   `OnlineDeviceStackCard`（行250-336）单卡渲染：绿点+时间头、别名/终端标签/铅笔按钮、IP/MAC 垂直芯片、流量/时长、注销按钮（受 `offlineDisabled`/`offlineLoading` 控制）。
    *   `OnlineDeviceStackH`（行342-519）水平堆叠（平板），参数同 `OnlineDeviceStackProps`，内部用 `dragX`（行346）、`sd` 计算（行377）、姿势表含左右相邻/次邻（行455-464）、`narrow` 定位 `left:50%` + `minWidth:213`（行469-489）。
*   **关键实现**：
    *   状态 `index/dragY/dragging/cardH`（行52-55）与 refs `stageRef/drag/suppressClick`（行56-58）。
    *   列表变短夹页码（行61-63）；`focusIndex` 变化滚中央（行66-69）；`ResizeObserver` 量最大自然高，无则退化一次性（行74-89）。
    *   `count===0` 返 null（行91）；两卡无底露出（行93）；`rel` 取模（行94）；`go` 翻页（行98-102）。
    *   手势 `onPointerDown/Move/endDrag/onClick`（行104-150）：单卡禁手势（行105, 140）、拖拽 clamp（行120）、`suppressClick` 防松手误触（行129-137）、中线分区点击（行148）。
    *   姿势表渲染绝对定位（行172-211），页码点（行213-227）。

### `SelfServicePanel.tsx`
**职责**：自助服务 dashboard 面板，聚合在线设备堆叠、上网记录、注销确认弹窗、设备改名弹窗，统一凭据取自登录信息（引用 `VpnCredentialDialog`/`VpnManageDialog` 但未提供源码）。

*   **内部类型与导出工具**：
    *   `SelfOnlineItem` 接口（行28-38）：`loginTime: string` ("YYYY-MM-DD HH:mm:ss")、`ip: string`、`mac: string` (12位hex无分隔)、`useTime: string` (秒)、`downFlow: string` (KB)、`upFlow: string` (KB)、`hostName: string`、`terminalType: string` ("#PC"带#前缀)、`sessionId: string`。
    *   `VpnStatus` 接口（行41-45）：`saved: boolean`、`accountMasked: string`、`viaVpn: boolean`。
    *   `toInt`（行49-52 模块内未 export）、`formatTerminalType`（行55-58 "#PC"→"PC" 未 export）、`formatUseTimeMinutes`（行61-64 export，秒→分 floor）、`formatFlowMb`（行67-71 export，KB→M留3位）、`localDateStr`（行75-78 export，本地日期防UTC偏移）。
*   **主组件 `SelfServicePanel`**（行80-480）：
    *   状态：`onlineList`（行87）、`formFactor`（行88）、`history`（行89）、`querying`（行90）、`offlineSessionId`（行91）、`confirmTarget`（行92）、`aliasTarget`（行94）、`deviceAliases`（行95）、`vpnStatus`（行97）、`vpnCredOpen`（行98）、`vpnManageOpen`（行99）、`vpnClearConfirmOpen`（行100）、`logStart/End`（默认今本地日期，行102-103）、`logRows`（行104）、`logSummary`（行105）、`logQuerying`（行106）、`mountedRef`（行107）、`autoRefreshedRef`（行110）、`configLoaded`（行112）。
    *   `hasCred`（行125）= `configUser.trim() && passwordSaved`。
    *   `fetchDashboard`（行127-151）调 `tauriApiWithRetry.querySelfDashboard`，`silent` 降级不弹错（行141）。
    *   切入自动验证刷新 effect（行157-163）依赖 `configLoaded` 防早判。
    *   `handleOffline`（行165-190）调 `selfOfflineSession`，防并发（行166），成功过滤列表（行178）。
    *   `handleSaveAlias`（行193-210）写 `config.deviceAliases[mac]` 留空删键（行195-198）。
    *   `refreshVpnStatus`（行213-224）、`handleVpnTest`（行231-240）、`handleVpnSave`（行244-254）、`handleVpnToggle`（行257-274）、`handleVpnClear`（行277-293）。
    *   `fetchOnlineLog`（行295-319）调 `querySelfOnlineLog`。
    *   渲染：`formFactor==='tablet'` 用 `OnlineDeviceStackH` 否则 `OnlineDeviceStack`（行377-415 与 397-414），传格式化 props。

### `selfServiceState.ts`
**职责**：Windows Hello 验证门（绑定操作门 + 自助面板会话门）及生物错误文案映射。

*   **导出/模块成员**：
    *   `helloEnabled`（行10）内部判断 `config.selfHelloEnabled !== false`。
    *   `BIOMETRIC_ERROR_KEY`（行19-32）错误码映射（含 face 业务码）。
    *   `biometricFailMessage`（行34-38）按 `code` 翻译。
    *   `VERIFY_TTL_MS = 570_000`（行54）对齐后端 600s 留 30s 余量。
    *   `gateFresh`（行58-62）TTL 内且时钟未回拨（`elapsed>=0`）。
    *   `helloGateVerifiedAt`（行63）模块级；`selfSessionVerifiedAt`（行97）模块级。
    *   `useHelloGate`（行65-87）绑定门 hook，支持 `ignoreToggle` 强制真实验证（行71）。
    *   `resetSelfSessionGate`（行100-102）重置会话门。
    *   `useSelfServiceVerify`（行105-127）面板会话门，支持 `selfReverifyEachAction`（行110）。

### `selfServiceTypes.ts`
**职责**：自助服务共享类型与格式化工具（Panel/Records/测试共用）。

*   **导出类型**：
    *   `SelfHistoryRow`（行5）：元组 `[number, number, string, string, number, number, number, number, string | null, string, ...unknown[]]` 对应 上线/注销时间epoch ms、ip、mac、时长(分)、流量(M)、计费方式、金额、主机名、终端类型等。
    *   `SelfLogRow`（行9-22）：`loginTime: number`、`logoutTime: number`、`time: number`、`flow: number`、`costMoney: number`、`internetUpFlow: number`、`internetDownFlow: number`、`chinanetUpFlow: number`、`chinanetDownFlow: number`、`userIp: string`、`nasIp: string`、`nasPort: string | number`。
    *   `SelfLogSummary`（行25-34）：`INTERNETUPFLOW: number`、`INTERNETDOWNFLOW: number`、`CHINANETUPFLOW: number`、`CHINANETDOWNFLOW: number`、`FLOW: number`、`TIME: number`、`COSTMONEY: number`、`COU: number`。
*   **导出格式化**：`formatMac`（行36-40 归一化 hex 配对）、`formatEpoch`（行42-47）、`formatTerminalType`（行50-53）、`toInt`（行55-58）、`formatUseTimeMinutes`（行61-64 floor 秒转分）、`formatFlowMb`（行67-71 KB→M 留3位）、`fmtMoney`（行75-79 保留原始精度）、`localDateStr`（行82-85 本地日期防 UTC 偏移）。

### `types.ts`
**职责**：账号命令返回类型。

*   **导出**：
    *   `AccountItem` 复用 settings 定义（行4）。
    *   `AccountResult`（行7-13）：`success: boolean`、`message?: string`、`activeAccount?: string`、`config?: Config`、`displayName?: string`。
    *   `SwitchAccountResult`（行15-20）：`success: boolean`、`message?: string`、`activeAccount?: string`、`config?: Config`。
    *   `DeleteAccountResult`（行22-27）：同上字段。
    *   `SaveAccountResult`（行29-34）：`success: boolean`、`activeAccount?: string`、`config?: Config`、`message?: string`。

### `useAccount.ts`
**职责**：账号管理 hook（增/删/切/改名），与后端交互并同步 store。

*   **模块变量**：`switchInFlight`（行12）并发切换去重。
*   **导出 `useAccount`**（行14-166）内含：
    *   `refreshAccounts`（行30-37）调 `api.listAccounts`。
    *   `handleAddAccount`（行40-65）调 `saveCurrentAsAccount`，掩码 config 落地（行52-54）。注：`AccountPanel` 未使用此函数，创建走自身 `createAccount`。
    *   `handleDeleteAccount`（行67-92）调 `deleteAccount`，修复历史缺陷应用 `activeAccount/config`（行82-89）。
    *   `handleSwitchAccount`（行94-128）调 `switchAccount`，`switchInFlight` 去重（行95, 122-127），`!== undefined` 判活（行114）。
    *   `handleRenameAccount`（行132-157）调 `renameAccount` 仅改显示名。
    *   所有写回均 `updateConfigLocal`+`clearDirtyFields`+`syncPasswordSaved(PASSWORD_MASK比较)` 防串号。

### `useOperatorBind.ts`
**职责**：运营商绑定流 hook（绑定/查询/揭示/改密）。

*   **导出**：`BIND_OPERATOR_NONE='__none__'`（行9）、`OperatorBindingInfo`（行12：`account: string; passwordSet: boolean`）、`BindStatuses`（行13：`Record<'cmcc'|'telecom'|'unicom', OperatorBindingInfo | null>`）、`useOperatorBind`（行25-221 入参 `user, operator, passwordSaved, credOverride?`）。
*   **状态**：`bindOp/Phone/Sms/Statuses/statusOpen/revealedOp/revealedPassword` 等（行34-44）。
*   **关键**：明文 30s 清除 effect（行55-62）；init 默认运营商（行65-70）；`credReady`（行75-77 手动优先）；`canBind`（行82-85 手机号 `/^1\d{10}$/`）；`ensureHelloVerified=useHelloGate()`（行91）；`fetchBindStatus`（行93-119）成功 `setStatusOpen(true)`；`ensureRevealVerified=useHelloGate({ignoreToggle:true})`（行124）；`handleReveal`（行125-149）调 `revealOperatorCredential`；`handleBindOperator`（行151-181）成功调 `fetchBindStatus`；`handleChangePassword`（行187-213）调 `changeSelfPassword` 成功 `saveConfigDirect` 同步密码。

### `validators.ts`
**职责**：改名校验（与后端 `validate_display_name` 对齐）。

*   **导出**：`DISPLAY_NAME_MAX_LEN=32`（行9）、`CONTROL_CHARS_RE`（行13 含 C0/C1/DEL）、`isValidDisplayName`（行16-17 码点计长、禁控符）。

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                 SelfServicePanel.tsx (行80-480)           │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │OnlineDevice  │  │SelfOnline     │  │DeviceAliasDialog│ │
│  │Stack/H       │  │Records        │  │ConfirmDialog   │ │
│  └──────┬───────┘  └──────┬───────┘  └────────┬────────┘  │
│         └──────────┬───────┴──────────────────┘           │
│                    ▼                                        │
│  ┌────────────────────────────────────────────────────┐  │
│  │      useSelfServiceVerify() (selfServiceState)     │  │
│  └───────────────────────┬────────────────────────────┘  │
│                          ▼                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │ AccountPanel.tsx (行50-509) & useAccount.ts        │  │
│  │ useOperatorBind.ts (行25-221)                      │  │
│  └───────────────────────┬────────────────────────────┘  │
│                          ▼                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │     selfServiceState.ts (Hello Gates 行65-127)    │  │
│  └───────────────────────┬────────────────────────────┘  │
│                          ▼                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │       Tauri API Layer (tauriApiWithRetry)          │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## Data Flow

1.  **初始化与配置加载**:
    *   `SelfServicePanel` 从 `useConfigStore` 获取 `configUser`, `passwordSaved`, `deviceAliases`（行85-95）。
    *   配置加载且凭据就绪后，自动触发 `fetchDashboard`（行157-163）。
    *   `AccountPanel` 自订阅 store（行63）并处理 StrictMode（行64-71）。

2.  **数据获取与展示**:
    *   `SelfServicePanel` 调用 `tauriApiWithRetry.querySelfDashboard` 获取在线设备和历史记录（行127-151）。
    *   数据经过 `formatTerminalType`（行55）、`formatUseTimeMinutes`（行61）、`formatFlowMb`（行67）等工具函数格式化。
    *   在线设备数据传递给 `OnlineDeviceStack` 或 `OnlineDeviceStackH` 进行堆叠渲染（行377-415 与 397-414）。
    *   历史记录数据传递给 `SelfOnlineRecords`，通过 `recentToSessionRows`（行52-66）或 `billToSessionRows`（行68-85）归一化为 `SessionRow` 进行展示。
    *   `SelfOnlineRecords` 的 `groupByDay`（行107-120）和 `aggregateStats`（行94-104）用于视图聚合。

3.  **用户交互与操作**:
    *   **踢下线**: 用户点击注销按钮 -> `handleOffline`（行165-190） -> 调用 `useSelfServiceVerify()` 进行生物验证 -> 调用 `selfOfflineSession` API -> 成功后从 `onlineList` 中移除该设备（行178）。
    *   **改名**: 用户点击改名 -> 弹出 `DeviceAliasDialog`（行25） -> 输入新名称 -> 调用 `handleSaveAlias`（行193-210） -> 更新 `config.deviceAliases`（行195-198）。
    *   **运营商绑定**: 用户点击绑定 -> 弹出 `BindOperatorDialog`（行41） -> 选择凭据模式 (`login`/`manual`) -> 输入手机号/验证码 -> 调用 `useOperatorBind` 中的 `handleBindOperator`（行151） -> 涉及 `useHelloGate` 生物验证 -> 调用 `bindOperator` API。
    *   **账号管理**: 用户编辑账号信息 -> `LoginInfoDialog`（行51） -> `handleSaveLoginInfo`（行180-234） -> 调用 `createAccount` 或 `saveCurrentAsAccount` -> 更新 Store。

4.  **生物验证门控**:
    *   所有敏感操作（如踢下线、查询绑定状态、揭示明文密码）均需通过 `useSelfServiceVerify` 或 `useHelloGate`。
    *   `useHelloGate` 维护全局 TTL (`VERIFY_TTL_MS = 570_000`，行54)，并处理时钟回拨防护（行58）。
    *   揭示明文密码 (`handleReveal`，行125) 强制要求验证，即使总开关关闭（`useHelloGate({ignoreToggle:true})`，行124）。

## Connections

*   **[[android-frontend-settings]]**: `useAccount.ts` 中的 `handleSwitchAccount` 等方法被 Settings 模块调用，用于全局账号切换；`AccountPanel` 的 `onUpdateConfig` 直接修改 `Config`（行35, 446等）。
*   **[[android-frontend-dashboard]]**: `SelfOnlineRecords` 接收来自 Dashboard 的 `history` 数据（行157 提及 dashboard 自动带回），表明两者存在数据依赖；`SelfServicePanel` 的 `formatUseTimeMinutes` 等工具与 dashboard 共用。
*   **[[tauri-backend-account]]**: 所有 API 调用 (`tauriApiWithRetry`) 指向后端提供的账号管理、运营商绑定、自助服务等接口（如 `createAccount` 行183, `saveCurrentAsAccount` 行219, `switchAccount`, `renameAccount`, `deleteAccount`, `bindOperator` 行156, `getBindStatus` 行98, `revealOperatorCredential` 行133, `changeSelfPassword` 行191, `querySelfDashboard` 行132, `selfOfflineSession` 行170, `querySelfOnlineLog` 行300, `vpnCredentialStatus` 行215, `vpnSaveCredential` 行245, `setSelfServiceViaVpn` 行260, `vpnClearCredential` 行279, `verifyWindowsIdentity` 行73）。

## Known Issues

1.  **StrictMode 副作用安全**: `AccountPanel` 使用 `mountedRef` 跟踪组件挂载状态（行64-71），所有异步回调（行131, 159, 189, 199, 213, 220, 226, 等）均检查 `mountedRef.current`，以防止 React 18 StrictMode 双重调用导致的无效状态更新。同样模式见于 `SelfServicePanel`（行107, useEffect 行114-122）与 `useOperatorBind`（行46-51）。
2.  **密码清除语义**: 后端约定空字符串密码表示“保留旧密码”。因此，显式清除密码必须走 `handleClearPassword`（行100-115），它调用 `saveConfigDirect({ password: '' }, true)` 并设置 `syncPasswordSaved(false)` 及 `clearDirtyFields()`（行113）。如果直接留空保存，不会清除密码。
3.  **切换账号防重入**: `handleSwitchAccount`（行119-123）检查 `switchingAccount !== null`，防止在切换进行中再次触发操作；改名/编辑/删除按钮在 `renamingId!==null` 或 `switchingAccount!==null` 时 disabled（行342, 352, 363, 374）。
4.  **编辑非激活账号的逻辑**: 点击非激活卡的“编辑”按钮时，`handleCardEdit`（行127-134）会先尝试切换到该账号（`handleSwitchAccount`），切换成功后再打开编辑弹窗。这确保了编辑的上下文正确。
5.  **运营商凭据覆盖机制**: `BindOperatorDialog` 允许用户在绑定时手动输入账号密码（`credMode === 'manual'`，行51）。这些凭据仅在本次绑定中使用，通过 `onCredChange`（行68-72）传递给 `useOperatorBind` 的 `credOverride`（行83），不会保存到本地存储。关闭弹窗或切换回 `login` 模式时，凭据被清空回传 `null`。
6.  **密码修改对话框的旧密码处理**: 默认情况下，旧密码由后端 resolve（使用已保存的登录密码）。用户可以选择展开手动输入旧密码框（行84-108），此时手动输入的旧密码优先于已存密码。`ChangePasswordDialog` 新密码正则限制 `/^[0-9A-Za-z]{6,11}$/`（行28）。
7.  **上线记录的数据归一化**: `SelfOnlineRecords` 将两种不同来源的数据（Dashboard 历史数据和账单明细）统一转换为 `SessionRow` 接口，使得后续的分组和统计逻辑可以复用（行52-85）。`logRows` 为空数组仍判为 `queried` 显查询态（行155）。
8.  **设备别名保存的错误吞没**: `DeviceAliasDialog` 的 `submit` 函数中（行36-47），catch 块吞掉了错误，因为错误提示已由调用方（`handleSaveAlias`）发出。这是为了防止 unhandled rejection，同时保持弹窗打开以便重试。
9.  **时钟回拨死锁防护**: `selfServiceState.ts` 中，`gateFresh`（行58）不仅检查 TTL，还检查 `elapsed >= 0`。若系统时钟回拨，前端视为过期，强制重新验证，避免前端缓存永真而后端已过期的死锁。
10. **明文密码安全**: 查看明文密码 (`reveal`) 必须强制验证，即使 `selfHelloEnabled` 为 false（行124）。明文密码在 state 中保留 30 秒自动清除（`useOperatorBind.ts` 的 effect，行55-62）。
11. **并发切换去重**: `useAccount.ts` 中 `switchInFlight`（行12）promise 机制确保同一时刻只有一个 switch 请求在执行，后续请求等待其结果，避免状态交错（行95, 122-127）。
12. **假阴性判断**: `useAccount.ts` 切换账号时（行94），后端可能返回空字符串作为 `activeAccount`。代码使用 `result?.activeAccount !== undefined` 而非真值判断（行114），确保能正确处理空字符串情况，避免 UI 不更新。
13. **时间格式化一致性**: `SelfServicePanel.tsx` 的 `formatUseTimeMinutes`（行61）与 `selfServiceTypes.ts` 的 `formatUseTimeMinutes`（行61）均使用 `Math.floor(seconds / 60)` 而非 `round`，严格对齐原站行为。
14. **触控冲突隔离**: `SelfOnlineStack.tsx` 堆叠容器设置 `touch-action:none`（行163, 440），防止与 WebView 页面滚动冲突。
15. **静默降级**: `SelfServicePanel.tsx` 中 `fetchDashboard`（行127）在非手动触发（如自动刷新）且失败时，不弹出错误 Toast（行141 `silent` 分支），仅在手动刷新失败时提示，提升用户体验。
16. **JSdom 兼容性**: `SelfOnlineStack.tsx` 中 `setPointerCapture` 和 `ResizeObserver` 在测试环境（jsdom）中可能缺失（行110-114, 85），代码做了 try-catch 或 typeof 检查以优雅降级。
17. **注释与实现顺序不符**: `AccountPanel` 注释（行43-49）称自动化卡在上、账号管理卡在下，实际 JSX 账号卡先渲染（行246 账号管理卡 stagger-i=0 vs 行424 自动化卡 stagger-i=1）。
18. **获取高度防跳变**: `SelfOnlineStack` 量高取最大自然高（行74-89），防 WebView 中跳变。
19. **格式化对齐后端**: `formatMac` 剥非 hex 再配对（`selfServiceTypes.ts` 行36）；`fmtMoney` 保留原始精度防 `toInt` 截 0（行75）；`localDateStr` 用本地时区防 UTC 偏移（行82）。
20. **校验器码点计长**: `isValidDisplayName` 按 Unicode 码点计长（非 UTF‑16），与后端 `validate_display_name` 同构（`validators.ts` 行16）。
21. **未使用 Prop/参数**: `AccountPanelProps.onAddAccount`（行36）在组件解构时被忽略（行50-57 未列出），实际创建账号由 `handleSaveLoginInfo` 内 `createAccount` 完成；`OnlineDeviceStackProps.variant` 属性（行45）在 `OnlineDeviceStack` 函数（行50）未解构使用，仅 `OnlineDeviceStackH`（行342）使用 `variant`。
