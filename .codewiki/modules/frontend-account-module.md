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
  - android/frontend/src/account/types.ts
  - android/frontend/src/account/validators.ts
  - android/frontend/src/account/LoginInfoDialog.tsx
  - android/frontend/src/account/BindOperatorDialog.tsx
  - android/frontend/src/account/useOperatorBind.ts
  - android/src-tauri/src/account_cmds.rs
tags: [account, ui, self-service, validation, biometric]
---

## Overview

本模块是安卓前端（`android/frontend/src/account/`）的账号域代码，主要涵盖 8 个文件，并引用同目录 `LoginInfoDialog.tsx`、`BindOperatorDialog.tsx`、`useOperatorBind.ts` 与后端 `android/src-tauri/src/account_cmds.rs` 对齐，覆盖四块职责：

1. **账号页主面板**（`AccountPanel.tsx`，509 行）：账号管理卡（账号卡堆叠 + 三个弹窗）与自动化设置卡（三开关），2026-10-17 重排为单列纵排布局（见 `AccountPanel.tsx:50-56` 的布局注释）。
2. **账号命令类型与校验**（`types.ts`、`validators.ts`）：账号命令统一返回类型，以及与后端单点对齐的显示名校验。
3. **在线设备堆叠卡**（`SelfOnlineStack.tsx`，519 行）：竖排（手机）/水平（平板）两套“封面流”堆叠组件 + 单张设备卡，被账号页、仪表盘与测速面板等处消费。
4. **自助服务支撑**（`SelfOnlineRecords.tsx`、`selfServiceState.ts`）：上网记录融合卡（近期流水 + 账单查询双源）与 Windows Hello 验证门（绑定门 / 会话门）。

与桌面端 `tauri-app/frontend/src/account/` 为同构复刻关系；校验口径、命令语义与后端 `android/src-tauri` 单点对齐——例如显示名校验对齐 `android/src-tauri/src/account_cmds.rs:375` 的 `validate_display_name`（`validators.ts:1-6` 注释）。

## Key Components

### AccountPanel.tsx —— 账号页主面板（509 行）

`AccountPanelProps` 接口（`AccountPanel.tsx:32-41`）：

| 字段 | 类型 | 用途 |
| --- | --- | --- |
| `accounts` | `AccountItem[]` | 账号列表（来自 `useConfigStore`） |
| `activeAccount` | `string` | 激活账号 id |
| `onUpdateConfig` | `(partial: Partial<Config>) => void` | 自动化设置卡三开关写入 |
| `onAddAccount` | `(name: string) => Promise<boolean>` | **声明但组件未解构使用**（见 Known Issues） |
| `onDeleteAccount` | `(id: string, displayName: string) => void` | 删除账号 |
| `onSwitchAccount` | `(name: string) => Promise<void>` | 切换激活账号 |
| `onRenameAccount` | `(accountId: string, displayName: string) => Promise<boolean>` | 改名（第二参为已校验新名） |

组件为 `memo` 导出（`AccountPanel.tsx:50`），解构时**跳过 `onAddAccount`**（`AccountPanel.tsx:36-42`）。

状态与订阅：

- `passwordSaved` 订自 `useConfigStore`（59 行）；`addToast` 订自 `useLogToastStore`（60 行）。
- `config` 用 `useShallow` 浅比较自订阅（63 行），使 App 外壳不再因任意 config 字段变化而级联重渲染。
- `mountedRef`（64 行）+ 挂载 effect（66-71 行）：StrictMode setup→cleanup→setup 时二次 setup 恢复 `mountedRef`，否则 dev 模式下 async handler 内的 setState 全部被丢弃。

页面级功能块（按行号顺序）：

- **弹窗 A 状态**（74-79 行）：`loginInfoOpen` / `loginInfoMode: 'create' | 'edit'` / `openLoginInfo(mode)`。同一 `LoginInfoDialog` 双态复用。
- **弹窗 B 状态**（82-86 行）：`bindOpen` / `bindCredOverride`（`{ account, password } | null`）/ `handleBindCredChange`。
- **`applyActiveCardPasswordSaved(saved: boolean)`**（92-96 行）：激活账号卡的密码状态就地修补，直接改 store 里激活卡的 `passwordSaved`。
- **`handleClearPassword`**（100-115 行）：`saveConfigDirect({ password: '' }, true)`（103 行，第二参为显式 clearPassword 标志）；失败 toast 并 return；成功 `syncPasswordSaved(false)`（108）+ 就地修补（109）+ `clearDirtyFields()`（113）。
- **`handleSwitchAccount`**（119-123 行）：busy 锁 `switchingAccount` 防重复提交。
- **`handleCardEdit`**（127-134 行）：非激活卡先走切换再开弹窗 A 编辑态，卸载后中止（131 行 `mountedRef` 检查）。
- **改名流**（138-164 行）：`renamingId` / `renameDraft` / `renaming`；`startRename`（141-144）预填 `displayName || id`；`submitRename`（149-164）以 `isValidDisplayName` 校验（152 行）。
- **`refreshAccounts`**（168-175 行）：`tauriApiWithRetry.listAccounts()` → `setAccounts`。
- **`handleSaveLoginInfo`**（180-234 行）——弹窗 A 保存，两态：
  - **create 态**（181-203 行）：`createAccount({ accountName, user, password, operator })`（183-188），不切换激活账号；成功后 toast + 刷新 + 关弹窗。
  - **edit 态**（204-233 行）：先 `saveConfigDirect({ user, operator, password? })`（211），填了新密码立即置真激活卡（217）；再 `saveCurrentAsAccount(activeAccount)`（219）回写档案。
- **`bindFlow`**（239-240 行）：`useOperatorBind(config.user, config.operator, passwordSaved, bindCredOverride)`。

JSX 结构（242-508 行）：

- **账号管理卡**（上卡，246-421 行）：卡头（248-258），`OnlineDeviceStack` 竖排堆叠（261-384），`focusIndex` 取激活账号索引（262）。账号卡内容（268-381）：行1在线点+显示名+「使用中」徽章（274-284）；学号大字（287-289）；运营商 chip（292-297）+密码状态 chip（298-305）；不全提示（307-311）；改名内联输入（312-333）或操作行（336-379：编辑/改名/切换(仅非激活)/删除）。空列表渲染 `MascotFigure variant="empty"`（385-391）。卡尾快捷操作区（393-418）：添加/编辑登录信息 + 绑定运营商。
- **自动化设置卡**（下卡，424-478 行）：三开关 `autoLoginOnStart`（443-448）、`autoLoginOnPreparation`（456-461，i18n 用 `monitor.autoLoginWhenReady*`）、`enableNightOperatorSwitch`（469-474），经 `onUpdateConfig` 写入。
- **三个弹窗**（481-505 行）：`LoginInfoDialog`（481-490）、`BindOperatorDialog`（493-498）、`ChangePasswordDialog`（501-505）。

### ChangePasswordDialog.tsx —— 修改校园网密码弹窗（166 行）

- Props 接口 `ChangePasswordDialogProps`（21-25 行）：`open` / `onSave(newPassword, oldPassword): Promise<boolean>` / `onCancel`。
- `NEW_PASSWORD_RE = /^[0-9A-Za-z]{6,11}$/`（28 行）：6–11 位、仅数字与英文。
- 组件 `ChangePasswordDialog`（30-165 行）：状态 `oldPassword`(32) / `showOld`(33) / `newPassword`(34) / `confirmPassword`(35) / `showPassword`(36) / `error`(37) / `saving`(38)。
- `open` 驱动复位 effect（41-52 行）。
- `submit`（54-72 行）：防双击→格式校验→确认匹配→`onSave`；失败保留草稿。
- UI：旧密码折叠展开（84-108），新/确认密码带眼睛切换（109-152），`maxLength={11}`。

### DeviceAliasDialog.tsx —— 在线设备改名弹窗（75 行）

- Props 接口 `DeviceAliasDialogProps`（15-23 行）：`open` / `mac`(12位hex) / `initialName` / `onSave(name): Promise<void>` / `onCancel`。
- 组件 `DeviceAliasDialog`（25-74 行）：状态 `name`(27) / `saving`(28)；effect 复位（31-34）；`submit`（36-47）catch 吞错保打开；输入 `maxLength={30}`（60）。

### SelfOnlineRecords.tsx —— 上网记录融合卡（360 行）

- `fmt2` / `fmtMoney`（24-29 行）。
- 接口 `SessionRow`（32-50 行）：`key` / `mode: 'recent' | 'bill'` / `loginTime` / `logoutTime` / `timeMin` / `flowMb` / `costMoney` / `ip` + 可选 `mac`/`terminalType`/`payStyle`(近期) 与 `intlUp`/`intlDown`/`cnUp`/`cnDown`/`nasIp`/`nasPort`(账单)。
- `recentToSessionRows(history)`（52-66 行）：数组下标映射，`key = recent-${idx}`。
- `billToSessionRows(rows)`（68-85 行）：对象字段直取，`key = bill-${idx}`。
- 接口 `RecordStats`（87-92 行）：`timeMin`/`flowMb`/`costMoney`/`count`。
- `aggregateStats(rows)`（94-104 行）：求和。
- `groupByDay(rows)`（107-120 行）：按本地日期分组。
- 接口 `SelfOnlineRecordsProps`（122-134 行）：`configLoaded`/`hasCred`/`history`/`logStart`/`logEnd`/`logQuerying`/`logRows`/`logSummary`/回调。
- `payStyleText`（136-141 行）：1计时/2流量/3包月。
- `dayHeading`（144 行）。
- 组件 `SelfOnlineRecords`（146-359 行）：`queried = logRows !== null`（155）；memo 化 sessions/groups/stats（156-161）；日期条常驻（181-217）；查询态 8 芯片（228-248）+ 账单特有行（272-283）+ ≥500 溢出提示（292）；近期态 4 芯片（299-311）+ MAC/终端行（319-345）。

### SelfOnlineStack.tsx —— 在线设备堆叠卡（519 行）

常量表（14-41 行）：

| 常量 | 值 | 用途 |
| --- | --- | --- |
| `PEEK_TOP` / `PEEK_BOTTOM` | 44 / 44 | 竖排上下露出边高度 px |
| `DRAG_THRESHOLD` | 60 | 翻页阈值 px |
| `DRAG_MIN` / `DRAG_MAX` | -110 / 70 | 竖排拖拽行程上限 |
| `DRAG_CLAMP` | 80 | 水平拖拽 clamp |
| `SLIDE_MS` / `EASE` | 500 / `cubic-bezier(0.22,1,0.36,1)` | 切换过渡 |
| `V_SIDE_SCALE` | 0.75 | 竖排露出边缩放 |
| `V_CARD_INSET` | `inset-x-[6%]` | 竖排卡宽 = 舞台 88% |
| `CF_X` / `CF_INSET` | `[60,85]` / `27%` | 水平 coverflow 偏移/卡宽 |
| `SIDE_SCALE` / `SIDE_SCALE_2` | 0.6 / 0.45 | 水平相邻/次邻缩放 |
| `NARROW_MIN_W` | 213 | narrow 变体最小卡宽 |

- 接口 `OnlineDeviceStackProps`（43-48 行）：`cards` / `variant?` / `focusIndex?`。
- `OnlineDeviceStack`（竖排，50-230 行）：解构 `{ cards, focusIndex }`（50），忽略 `variant`；状态（52-57）；count 夹页（61-63）；focus 同步（66-69）；量高 effect（74-89）；`go`（98-102）；指针事件（104-150）；舞台渲染（154-212）；页点（213-227）。
- 接口 `OnlineDeviceStackCardProps`（233-248 行）：`loginTime`/`ip`/`mac`/`flowMb`/`durationMin`/`terminalType`/`alias?`/`onRename?`/`offlineDisabled?`/`offlineLoading?`/`onOffline?`。
- `OnlineDeviceStackCard`（250-336 行）：卡面 p-3；行1时间+别名+终端+铅笔（268-293）；IP/MAC 垂直芯片（295-304）；流量/时长+注销（305-333）。
- `OnlineDeviceStackH`（水平，342-519 行）：`variant` 生效（343）；结构同竖排但横向；narrow 定位用 `left 50%` + `minWidth:213`（469-489）；calc 陷阱注释（472-473）；六张以上隐藏槽（462-463）。

### selfServiceState.ts —— Windows Hello 验证门（127 行）

- `helloEnabled()`（10 行）：`config.selfHelloEnabled !== false`。
- `BIOMETRIC_ERROR_KEY`（19-32 行）：错误码→i18n key 映射（含 faceService 码）。
- `biometricFailMessage(err, t)`（34-38 行）。
- `VERIFY_TTL_MS = 570_000`（54 行）。
- `gateFresh(verifiedAt)`（58-62 行）：TTL 内且 `elapsed >= 0`（时钟回拨过期）。
- `helloGateVerifiedAt` 模块级（63 行）。
- `useHelloGate(options?)`（65-87 行）：绑定/查询/改密门；`ignoreToggle` 强制明文验证。
- `selfSessionVerifiedAt` 模块级（97 行）。
- `resetSelfSessionGate()`（100-102 行）。
- `useSelfServiceVerify()`（105-127 行）：面板会话门；`selfReverifyEachAction` 控制每次验证。

### types.ts —— 账号命令返回类型（34 行）

- `AccountItem` re-export（4 行）。
- `AccountResult`（7-13 行）：`success`/`message?`/`activeAccount?`/`config?`/`displayName?`。
- `SwitchAccountResult`（15-20 行）、`DeleteAccountResult`（22-27 行）、`SaveAccountResult`（29-34 行）：同构字段。

### validators.ts —— 改名校验（18 行）

- `DISPLAY_NAME_MAX_LEN = 32`（9 行，Unicode 码点）。
- `CONTROL_CHARS_RE = /[\u0000-\u001f\u007f\u0080-\u009f]/`（13 行）。
- `isValidDisplayName(trimmed)`（16-18 行）：非空 + 长度≤32 + 无控制字符；与后端 `validate_display_name` 对齐。

## Architecture

```text
┌───────────────────────────────────────────────────────────────┐
│ AccountPanel（AccountPanel.tsx，单列纵排：管理卡上/设置卡下）  │
│                                                               │
│  账号管理卡 :246-421                自动化设置卡 :424-478      │
│  ┌───────────────────────────┐   ┌─────────────────────────┐ │
│  │ OnlineDeviceStack 竖排     │   │ 三开关 Switch            │ │
│  │  (SelfOnlineStack.tsx)     │   │   ▼ onUpdateConfig       │ │
│  │   └ 账号卡×N（改名/切换）  │   │ autoLoginOnStart 等      │ │
│  └─────────────┬─────────────┘   └─────────────────────────┘ │
│                ▼ 快捷操作区 :393-418                           │
│  弹窗A LoginInfoDialog ◄── handleSaveLoginInfo :180-234        │
│  弹窗B BindOperatorDialog ◄── useOperatorBind :239-240         │
│  ChangePasswordDialog ◄── handleClearPassword :100-115          │
└───────────────────────────────────────────────────────┘
   ▲▲ config / accounts / passwordSaved    ▲ verifyWindowsIdentity
   useConfigStore（zustand）      selfServiceState.ts（Hello 两道门）

SelfServicePanel ──► SelfOnlineRecords（上网记录融合卡，双源 SessionRow）
DashboardPanel / SpeedTestPanel ──► OnlineDeviceStack(H) / StackCard
```

## Data Flow

**读取流**：`useConfigStore` → `AccountPanel` 自订阅 `config`（`AccountPanel.tsx:63`）与 `passwordSaved`（59）；账号列表 `accounts` 经 props 传入。

**写入流**（账号信息）：

1. 新建：弹窗 A create 态 → `tauriApiWithRetry.createAccount()`（`AccountPanel.tsx:183`）→ `refreshAccounts()`（195）→ 列表回流。不切换激活账号。
2. 编辑：弹窗 A edit 态 → `saveConfigDirect(partial)`（211，密码留空=后端保留）→ `saveCurrentAsAccount(activeAccount)`（219）→ `refreshAccounts()`（231）。
3. 清除密码：`saveConfigDirect({ password: '' }, true)`（103）→ `syncPasswordSaved(false)` + 激活卡就地修补 + `clearDirtyFields()`。
4. 三开关：`onUpdateConfig({ autoLoginOnStart | autoLoginOnPreparation | enableNightOperatorSwitch })` → 外壳落配置。
5. 改名：`onRenameAccount(renamingId, trimmed)`（158，前端先过 `isValidDisplayName`）→ 后端终审。

**密码状态窗口修补**：`applyActiveCardPasswordSaved`（92-96）就地改 store 激活卡 → 档案回写后 `list_accounts` 对齐。

**绑定流**：登录信息或弹窗 B 手动分段 → `useOperatorBind` 四元组（239）→ 绑定/查询/揭示/改密；揭示受 `useHelloGate` 门控。

**上网记录流**：宿主面板持状态 → `SelfOnlineRecords` 纯展示 → `recentToSessionRows`/`billToSessionRows` 归一 `SessionRow` → `groupByDay` + `aggregateStats` → 近期4/查询8芯片两态。

**验证流**：操作触发 → `gateFresh()` 检查模块级时间戳（`helloGateVerifiedAt` 63 / `selfSessionVerifiedAt` 97）→ 未验证则 `verifyWindowsIdentity` → 成功写时间戳；失败按 `BIOMETRIC_ERROR_KEY` 翻译。两门时间戳不共享。

## Connections

- [[frontend-account]]、[[android-frontend-account]]：账号域主文章；本模块为其前端子域。
- [[desktop-frontend-account]]、[[desktop-account-selfservice]]：桌面端同构参照。
- [[android-frontend-hooks-and-stores]]：`useConfigStore`、`useLogToastStore`、`tauriApiWithRetry` 实现侧。
- [[android-frontend-settings]]：`AccountItem` / `Config` 类型与 `operatorLabelKey` 常量来源。
- [[android-frontend-primitives]]、[[android-frontend-shared-ui]]：UI 基件与 `AnimatedCard`、`MascotFigure`。
- [[android-frontend-monitor]]：自动化卡第二开关复用 `monitor.autoLoginWhenReady*`（`AccountPanel.tsx:453-454`）；`SpeedTestPanel` 消费堆叠组件。
- [[android-frontend-network-face]]、[[frontend-face-verification]]：2D 人脸回退链错误码（`selfServiceState.ts:28-31`）。
- [[decisions/account-display-name-id-separation]]：改名语义背景。
- [[decisions/windows-hello-only-identity]]：Hello 验证唯一通道决策。
- [[decisions/strictmode-dev-only]]：StrictMode 双挂载与 `mountedRef` 保护。
- [[concepts/dual-platform-sharing]]：双端同构边界。
- [[learnings/binding-operator-select-excludes-campus]]：绑定运营商下拉排除校园网。
- [[learnings/android-frontend-no-unit-tests]]：无单测现状→jsdom 退化分支。
- [[decisions/android-selfservice-empty-merge]]：自助服务空态合并决策。

## Known Issues

1. **`onAddAccount` 声明未用**：`AccountPanelProps` 声明了 `onAddAccount`（`AccountPanel.tsx:36`），但组件解构跳过它（51-57）。重排后“添加”走弹窗 A create 态，该回调是遗留接口；调用方传了也会被忽略。
2. **StrictMode 双挂载保护是手动的**：`mountedRef`（`AccountPanel.tsx:64` + effect 66-71）逐 handler 检查，漏一处 dev 模式 setState 即被静默丢弃。
3. **Hello 门时间戳是模块级单例**：`helloGateVerifiedAt` / `selfSessionVerifiedAt`（`selfServiceState.ts:63`、`97`）为全局变量，多实例共享；TTL 570s 与后端 600s 余量在长时钟漂移下理论可重现前端放行后端拒绝，时钟回拨已由 `gateFresh` 的 `elapsed >= 0` 防住（58-62）。
4. **竖排堆叠吞掉页面滚动**：容器 `touchAction: 'none'`（`SelfOnlineStack.tsx:163`）——竖排手势与 WebView 页面滚动冲突，页面滚动必须触堆叠以外区域。
5. **jsdom 环境退化分支靠运行时探测**：`ResizeObserver` 缺失退化为一次性量高（`SelfOnlineStack.tsx:85`）、`setPointerCapture` 包 try/catch（111-114）——测试与真机路径不同。
6. **`OnlineDeviceStack` 忽略 `variant`**：Props 声明 `variant?: 'coverflow' | 'narrow'`（`SelfOnlineStack.tsx:45`）但竖排组件解构只有 `{ cards, focusIndex }`（50），姿势表恒定；`variant` 仅 `OnlineDeviceStackH` 消费（343）。
7. **i18n 命名空间跨界**：自动化设置卡第二个开关文案取自 `monitor.autoLoginWhenReady`（`AccountPanel.tsx:453`）与 `monitor.autoLoginWhenReadyDesc`（454），语义属账号页、命名属监控页。
