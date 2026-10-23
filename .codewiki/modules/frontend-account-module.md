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
tags: [account, ui, self-service, validation, biometric]
---

# 前端账号模块（安卓）

## Overview

本模块是安卓前端（`android/frontend/src/account/`）的账号域代码，共 8 个文件、约 1800 行，覆盖四块职责：

1. **账号页主面板**（`AccountPanel.tsx`，504 行）：账号管理卡（账号卡堆叠 + 三个弹窗）与自动化设置卡（三开关），2026-10-17 重排为单列纵排布局（见 `AccountPanel.tsx:43-49` 的布局注释）。
2. **账号命令类型与校验**（`types.ts`、`validators.ts`）：账号命令统一返回类型，以及与后端单点对齐的显示名校验。
3. **在线设备堆叠卡**（`SelfOnlineStack.tsx`，519 行）：竖排（手机）/水平（平板）两套"封面流"堆叠组件 + 单张设备卡，被账号页、仪表盘（`auth/DashboardPanel.tsx:34`）与测速面板（`monitor/SpeedTestPanel.tsx:16`）三处消费。
4. **自助服务支撑**（`SelfOnlineRecords.tsx`、`selfServiceState.ts`）：上网记录融合卡（近期流水 + 账单查询双源）与 Windows Hello 验证门（绑定门 / 会话门）。

与桌面端 `tauri-app/frontend/src/account/` 为同构复刻关系（通用改进须双端同步，见 [[concepts/dual-platform-sharing]]）；桌面侧对应文章为 [[desktop-frontend-account]] 与 [[desktop-account-selfservice]]。校验口径、命令语义与后端 `android/src-tauri` 单点对齐——例如显示名校验对齐 `android/src-tauri/src/account_cmds.rs:375` 的 `validate_display_name`（`validators.ts:3` 明文注释）。

## Key Components

### AccountPanel.tsx —— 账号页主面板（504 行）

Props 接口 `AccountPanelProps`（`AccountPanel.tsx:32-41`），全部字段：

| 字段 | 类型 | 用途 |
| --- | --- | --- |
| `accounts` | `AccountItem[]` | 账号列表（来自 `useConfigStore`） |
| `activeAccount` | `string` | 激活账号 id |
| `onUpdateConfig` | `(partial: Partial<Config>) => void` | 自动化设置卡三开关写入 |
| `onAddAccount` | `(name: string) => Promise<boolean>` | **声明但组件未解构使用**（见 Known Issues） |
| `onDeleteAccount` | `(id: string, displayName: string) => void` | 删除账号 |
| `onSwitchAccount` | `(name: string) => Promise<void>` | 切换激活账号 |
| `onRenameAccount` | `(accountId: string, displayName: string) => Promise<boolean>` | 改名（第二参为已校验新名） |

组件为 `memo` 导出（`AccountPanel.tsx:50`），解构时**跳过 `onAddAccount`**（`AccountPanel.tsx:51-57`）。

状态与订阅：

- `passwordSaved` 订自 `useConfigStore`（59 行）；`addToast` 订自 `useLogToastStore`（60 行）。
- `config` 用 `useShallow` 浅比较自订阅（61-63 行），使 App 外壳不再因任意 config 字段变化而级联重渲染——这是把 config 从 props 传入改为 store 自订阅的性能决策。
- `mountedRef`（64 行）+ 挂载 effect（66-71 行）：StrictMode setup→cleanup→setup 时二次 setup 恢复 `mountedRef`，否则 dev 模式下 async handler 内的 setState 全部被丢弃（见 [[decisions/strictmode-dev-only]]）。

页面级功能块（按行号顺序）：

- **弹窗 A 状态**（73-79 行）：`loginInfoOpen` / `loginInfoMode: 'create' | 'edit'` / `openLoginInfo(mode)`。同一 `LoginInfoDialog` 双态复用，模式由打开入口决定。
- **弹窗 B 状态**（81-86 行）：`bindOpen` / `bindCredOverride`（手动凭据分段 `{ account, password } | null`）/ `handleBindCredChange`。手动填写的凭据只传给本次绑定状态机、不保存。
- **`applyActiveCardPasswordSaved(saved: boolean)`**（88-96 行）：激活账号卡的密码状态就地修补。背景：`save_config` / `clearPassword` 只落主配置，账号档案要等 `saveCurrentAsAccount` 才回写，这段窗口里卡片读的是档案旧密文（清了密码仍显示"密码已保存"）。它直接改 store 里激活卡的 `passwordSaved`，档案回写后 `list_accounts` 再对齐。
- **`handleClearPassword`**（98-115 行）：后端空密码语义是"保留旧密码"，必须走显式 `clearPassword` 标志——`saveConfigDirect({ password: '' }, true)`（103 行，第二参即标志）。失败不再静默假成功：catch 后 toast 且直接 return（104-107 行）。成功后 `syncPasswordSaved(false)`（108 行）+ 就地修补激活卡（109 行）+ `clearDirtyFields()`（113 行，复位 dirty 标记让后端回传裁决，否则 config-changed 回传被跳过导致状态脱节）。
- **`handleSwitchAccount`**（117-123 行）：busy 锁 `switchingAccount` 防重复提交（对照 DashboardPanel 同名模式）。
- **`handleCardEdit`**（125-134 行）：编辑语义只写激活档案；非激活卡先走现有切换路径（去重锁/合并规则原样）再开弹窗 A 编辑态，卸载后中止（131 行 `mountedRef` 检查）。
- **改名流**（136-164 行）：`renamingId` / `renameDraft` / `renaming` 三状态；`startRename`（141-144 行）预填 `displayName || id`；`submitRename`（149-164 行）以 `isValidDisplayName` 校验（152 行，口径单点在 `validators.ts`），Enter 提交 / Escape 取消。
- **`refreshAccounts`**（166-175 行）：`tauriApiWithRetry.listAccounts()` → `useConfigStore.getState().setAccounts()`，与 `useAccount.refreshAccounts` 同一路径；失败仅 dev 打日志。
- **`handleSaveLoginInfo`**（177-234 行）——弹窗 A 保存，两态：
  - **create 态**（181-203 行）：走 `createAccount({ accountName, user, password, operator })`（183-188 行），**不切换激活账号**；返回的 config 是后端默认配置、不可 apply。成功后 toast + 刷新列表 + 关弹窗。
  - **edit 态**（204-233 行）：两连发——先 `saveConfigDirect({ user, operator, password? })`（208-211 行，密码留空 = 后端保留旧密码），填了新密码就立即把 store 的「密码已保存」与激活卡状态置真（217 行，否则档案回写完成前的窗口里卡片仍显示"密码未保存"）；再 `saveCurrentAsAccount(activeAccount)`（219 行）回写激活档案。
- **`bindFlow`**（236-240 行）：`useOperatorBind(config.user, config.operator, passwordSaved, bindCredOverride)`——绑定/查询/揭示/改密逻辑在 `useOperatorBind.ts`（不在本模块文件清单内），本组件只渲染；凭据默认取登录信息（config.user + 已保存密码），弹窗 B"手动填写"分段经 `bindCredOverride` 改取手动值。

JSX 结构（242-503 行）：

- **账号管理卡**（上卡，244-416 行）：卡头图标 + 通栏「＋添加登录信息」按钮（258-260 行，`openLoginInfo('create')`）；`OnlineDeviceStack` 竖排堆叠（264-387 行），`focusIndex` 取激活账号索引（265 行）。账号卡内容（271-384 行）：行 1 在线点 + 显示名（空则 id）+「使用中」徽章（276-287 行）；学号大字等宽主体（288-292 行）；运营商品牌色 chip（`operatorChipClass`，297 行）+ 密码状态 chip（301-308 行）；信息不全提示（310-314 行，`!item.user || !item.passwordSaved`）；改名内联输入行（315-336 行）；操作行（337-383 行）——✎编辑（弹窗 A 编辑态）/ ✎改名 / ⇄切换（仅非激活卡渲染）/ 🗑删除。空列表渲染 `MascotFigure variant="empty"` 空态（388-394 行）。卡尾快捷操作区（395-413 行）：「编辑登录信息」+「绑定运营商」两枚 secondary 按钮。
- **自动化设置卡**（下卡，418-473 行）：三开关逻辑零改动——`autoLoginOnStart`（438-443 行）、`autoLoginOnPreparation`（451-456 行，i18n 用 `monitor.autoLoginWhenReady` 命名空间，448 行）、`enableNightOperatorSwitch`（464-469 行），全部经 `onUpdateConfig` 写入。
- **三个弹窗**（475-500 行）：`LoginInfoDialog`（475-485 行，编辑态预填取当前 config，修改/清除密码入口在密码字段旁，483-484 行分别拉起改密弹窗与 `handleClearPassword`）；`BindOperatorDialog`（487-493 行）；`ChangePasswordDialog`（495-500 行）。

### types.ts —— 账号命令返回类型（34 行）

- `AccountItem` 从 `@/settings` re-export（4 行），id 稳定 / displayName 可编辑。
- `AccountResult`（7-13 行）：`success: boolean`、`message?`、`activeAccount?`（账号 id）、`config?`、`displayName?`（改名激活账号时带）。
- `SwitchAccountResult`（15-20 行）、`DeleteAccountResult`（22-27 行）、`SaveAccountResult`（29-34 行）：同构的 `success/message?/activeAccount?/config?`。

### validators.ts —— 改名校验（18 行）

- `DISPLAY_NAME_MAX_LEN = 32`（9 行）：最大长度按 **Unicode 码点**计（非 UTF-16 单元）。
- `CONTROL_CHARS_RE = /[\u0000-\u001f\u007f\u0080-\u009f]/`（13 行）：C0 全区 + DEL + C1 区；`no-control-regex` lint 关闭属预期。
- `isValidDisplayName(trimmed: string)`（16-18 行）：非空 + `[...trimmed].length <= 32` + 无控制字符。
- 口径与后端单点对齐：`android/src-tauri/src/account_cmds.rs:375` 的 `validate_display_name`（注释 1-6 行）。**后端为准，此处仅即时反馈**。决策背景见 [[decisions/account-display-name-id-separation]]。

### ChangePasswordDialog.tsx —— 修改校园网密码弹窗（166 行）

- Props（21-25 行）：`open` / `onSave(newPassword, oldPassword): Promise<boolean>` / `onCancel`。
- `NEW_PASSWORD_RE = /^[0-9A-Za-z]{6,11}$/`（28 行）：服务端口径（改密页内嵌 jQuery validate）——6–11 位、仅数字与英文、不得含空白。
- 语义（注释 15-20 行）：自助服务系统 `/Self/setting/changePassword` 端点；凭据统一后该密码即登录密码。旧密码默认取已保存登录密码（后端 resolve 回退），弹窗只收集新密码；若密码已在别处（如网页自助服务）改过而本地未同步，可展开手动输入旧密码（显式传入后端优先于已存密码，留空仍回退）。
- `open` 驱动复位（41-52 行）：打开时清空全部草稿，关闭时仅复位 `saving`——ConfirmDialog 同模式，不猜关闭时机。
- `submit`（54-72 行）：防双击（55 行）→ 格式校验（56-59 行）→ 确认匹配（60-63 行）→ `onSave`；失败返回 false 时父级已 toast 原因，**保留草稿供修改重试**（68 行）。
- UI：旧密码默认折叠为一行展开链接（84-108 行 `showOld` 切换）；新密码 + 确认框带眼睛明文切换（109-152 行，Eye/EyeOff）；`maxLength={11}`；三个输入框 Enter 均可提交。

### DeviceAliasDialog.tsx —— 在线设备改名弹窗（75 行）

- Props（15-23 行）：`open` / `mac`（原始 12 位 hex，无分隔）/ `initialName` / `onSave(name): Promise<void>` / `onCancel`。
- 编辑 `config.deviceAliases[mac]`（注释 13 行），键 = 原始 12 位 hex MAC。
- `submit`（36-47 行）：`onSave(name.trim())`，catch 吞错仅为防 unhandled rejection（失败 toast 已由调用方 `handleSaveAlias` 发出），**弹窗保持打开、草稿保留**供修改重试。
- 输入 `maxLength={30}`（60 行）；`open=false` 复位草稿与 `saving`（31-34 行）。

### SelfOnlineStack.tsx —— 在线设备堆叠卡（519 行）

常量表（14-41 行）：

| 常量 | 值 | 用途 |
| --- | --- | --- |
| `PEEK_TOP` / `PEEK_BOTTOM` | 44 / 44 | 竖排上下露出边高度 px |
| `DRAG_THRESHOLD` | 60 | 翻页触发阈值 px |
| `DRAG_MIN` / `DRAG_MAX` | -110 / 70 | 竖排拖拽行程上限（上滑空间更大） |
| `DRAG_CLAMP` | 80 | 水平拖拽 clamp |
| `SLIDE_MS` / `EASE` | 500 / `cubic-bezier(0.22, 1, 0.36, 1)` | 切换过渡 |
| `V_SIDE_SCALE` | 0.75 | 竖排露出边缩放（水平用 0.6；竖排只露 ~38px 边条，取偏大值保证可读，23 行） |
| `V_CARD_INSET` | `inset-x-[6%]` | 竖排卡宽 = 舞台 88%；旧值 18%（卡宽 64%）被用户真机反馈"宽度不够、排版不对"（24-27 行） |
| `CF_X` / `CF_INSET` | `[60, 85]` / `27%` | 水平 coverflow：相邻/次邻横向偏移、卡宽 = 舞台 46% |
| `SIDE_SCALE` / `SIDE_SCALE_2` | 0.6 / 0.45 | 水平相邻/次邻缩放 |
| `NARROW_MIN_W` | 213 | narrow 变体最小卡宽 px（学号行自然宽 ~133 + 80px 留白，37-41 行） |

**`OnlineDeviceStack`**（竖排，50-230 行）——账号管理卡与在线信息卡共用：

- Props `OnlineDeviceStackProps`（43-48 行）：`cards: React.ReactNode[]` / `variant?: 'coverflow' | 'narrow'`（**竖排组件解构时忽略此参**，50 行）/ `focusIndex?: number`。
- 列表变短（如注销成功）时夹住当前页码（61-63 行）；外部焦点变化才平移（66-69 行），用户手动翻页不受干扰。
- 量高（74-89 行）：取所有已挂载卡的最大自然高度（各槽位常挂载），舞台高度不随翻页跳变；jsdom 无 `ResizeObserver` 时退化为一次性量高。注释（72 行）指出这是桌面 commit b795a9f 漏移植的"量高半边"。
- 手势：`onPointerDown`（104-115 行）——单卡不可移动、按钮内按下不启动拖拽、`setPointerCapture` 包 try/catch（jsdom 无此 API）；`onPointerMove`（116-121 行）clamp 到 `[DRAG_MIN, DRAG_MAX]`；`endDrag`（122-138 行）——超阈值翻页（上滑 = 下一张），拖而未翻则吞掉紧随的 click 防误触；`onClick`（139-150 行）以中线划分点击区（上半 = 上一张、下半 = 下一张，与水平堆叠左右分区同构）。
- 舞台（154-171 行）：`w-full` 必须撑满父容器（否则 shrink-to-fit 收窄后百分比 inset 相对收窄舞台，卡片被挤到半边之外，156-158 行注释）；`overflow-x-clip` 防拖拽期间撑出横向滚动条；`touchAction: 'none'`（163 行，竖排手势与 WebView 页面滚动冲突，见 Known Issues）。
- 槽位姿势表（172-186 行）：中央（r=0，top=PEEK_TOP）/ 顶部露出下一张（r=1，缩放压暗 0.5）/ 底部露出上一张（r=count-1，用卡高推导 top 使缩放后底边对齐容器底，178-179 行）/ 上下隐藏槽（透明常挂载，翻页时旧当前卡滚入隐藏槽淡出）/ 更深的卡不挂载。
- 拖拽中当前卡样式（187-200 行）：`transition: 'none'` + `zIndex: 40` 跟手；否则带 `transform 500ms` 过渡——**整副牌连续滚动**，无"离场淡出"。
- 页点指示器（213-227 行）：`count > 1` 时渲染，`aria-label={`${i+1} / ${count}`}`。

**`OnlineDeviceStackCardProps`**（233-248 行）：`loginTime` / `ip` / `mac` / `flowMb` / `durationMin` / `terminalType` / `alias?`（`config.deviceAliases[mac]`，空 = 未命名）/ `onRename?`（缺省不渲染铅笔）/ `offlineDisabled?` / `offlineLoading?` / `onOffline?`（缺省不渲染注销按钮——总览卡掩码态在验证通过前不提供注销入口）。

**`OnlineDeviceStackCard`**（250-336 行）：卡面 `p-3` 内边距与桌面端 OnlineDeviceCard 同构（266 行注释：commit 8da3b1c 曾误删导致内容贴边/字体溢出）；行 1 时间 + 别名 chip + 终端类型 chip + 铅笔（268-293 行）；IP / MAC 垂直芯片（294-304 行，芯片与卡面互为反色保持层次）；流量/时长左列 `min-w-0` 允许收缩（306-308 行：流量值如 81449.169M 会顶宽，不许收缩就把右侧「注销」挤出卡片）+ 注销按钮（321-332 行）。

**`OnlineDeviceStackH`**（水平，平板，342-519 行）：封面流轮播 + 整副牌连续滚动，机制与竖排一致；点击左半 = 上一张、右半 = 下一张，水平拖拽 ±60px 判翻页；narrow 变体（343 行 `variant === 'narrow'`）以 `left 50%` + `minWidth: 213` + `translateX(calc(-50% + x%))` 居中定位（467-489 行）。**calc 陷阱**（472-473 行注释）：无 `calc()` 包装的 `translateX(-50% + x%)` 求和会被浏览器整条丢弃，卡退化为左缘贴中线（右偏半个卡宽）。最多五张可见（中间 + 左右各两张），六张以上有透明隐藏缓冲槽（462-463 行）。账号管理堆叠卡专用 narrow 变体与桌面 `SelfOnlineCards.tsx OnlineDeviceCardStack` 同构同参数（37-40 行）。

### SelfOnlineRecords.tsx —— 上网记录融合卡（360 行）

双源同卡（注释 1-4 行）：近期流水（dashboard 自动带回）+ 日期查询（账单明细）；近期会话卡附 MAC/终端/计费方式，账单会话卡附上下行拆分/NAS——**各自缺的留白不硬造**。由 `SelfServicePanel.tsx:319` 渲染（宿主面板不在本模块文件清单内）。

- **`SessionRow`**（32-50 行）：`key` / `mode: 'recent' | 'bill'` / `loginTime` / `logoutTime` / `timeMin` / `flowMb` / `costMoney` / `ip` + 可选 `mac` / `terminalType` / `payStyle`（近期源）与 `intlUp` / `intlDown` / `cnUp` / `cnDown` / `nasIp` / `nasPort`（账单源）。
- **`recentToSessionRows(history)`**（52-66 行）：数组下标字段映射——`row[0]` loginTime、`row[1]` logoutTime、`row[2]` ip、`row[3]` mac、`row[4]` timeMin、`row[5]` flowMb、`row[6]` payStyle、`row[7]` costMoney、`row[9]` terminalType；`key = recent-${idx}`。
- **`billToSessionRows(rows)`**（68-85 行）：对象字段直取（`loginTime` / `logoutTime` / `time` / `flow` / `costMoney` / `userIp` / 四向流量 / `nasIp` / `nasPort`）；`key = bill-${idx}`。
- **`aggregateStats(rows)`**（94-104 行）：求和 timeMin/flowMb/costMoney + count，null 按 0。
- **`groupByDay(rows)`**（106-120 行）：按 `loginTime` 的本地日期（`localDateStr`）分组，升序输入 → 升序分组。
- Props（122-134 行）：`configLoaded` / `hasCred` / `history` / `logStart` / `logEnd` / `logQuerying` / `logRows` / `logSummary` / `onLogStartChange` / `onLogEndChange` / `onQuery`——全部状态由宿主面板持有，本组件纯展示。
- 展示态机（155 行）：`queried = logRows !== null`——查询过即账单明细态，否则近期流水态。查询态渲染 8 枚汇总芯片（227-248 行，对齐原站：INTERNETUPFLOW / INTERNETDOWNFLOW / CHINANETUPFLOW / CHINANETDOWNFLOW / FLOW / TIME / COSTMONEY / COU）+ 上下行/NAS 拆分行（272-283 行）+ ≥500 条溢出提示（292 行）；近期态渲染 4 枚聚合芯片（299-311 行）+ MAC/终端/计费行（336-342 行，`payStyleText`：1=计时 / 2=流量 / 3=包月，136-141 行）。
- 日期条常驻（180-217 行）：查询按钮 `disabled={!hasCred || logQuerying || !logStart || !logEnd}`（207 行）。

### selfServiceState.ts —— Windows Hello 验证门（127 行）

- `helloEnabled()`（10 行）：`config.selfHelloEnabled !== false`——缺失视为开启（旧配置/测试 mock 未写该字段时保持验证行为）。
- **`BIOMETRIC_ERROR_KEY`**（19-32 行）：生物验证失败 toast 文案按插件 reject 对象的稳定错误码翻译——`userCancel` / `systemCancel`（取消类文案中性）/ `biometryNotEnrolled` / `noDeviceCredential` / `biometryNotAvailable`（引导补齐锁屏，因验证调用已带 `allowDeviceCredential:true`）/ `biometryLockout` / `authenticationFailed`，及 2D 人脸回退链（faceService）的 `faceTimeout` / `faceChallenge` / `faceMismatch` / `faceCamera`。未知错误码回退原始消息（`biometricFailMessage`，34-38 行）。
- **`useHelloGate(options?: { ignoreToggle?: boolean })`**（65-87 行）：绑定运营商操作（绑定/查询绑定状态/查看明文密码）的验证门。与自助服务面板的会话门**完全独立**（互不共享状态）；首次操作即验证，通过后 TTL 内共用（sudo timestamp 同款会话模式）。`VERIFY_TTL_MS = 570_000`（54 行）与后端 reveal 门的 `IDENTITY_VERIFY_TTL_SECS=600s` 对齐（留 30s 时钟余量）——此前缓存应用生命周期永真，后端 600s 过期后查看明文被永久拒绝且无重验入口，重启前功能死锁（注释 46-49 行）。`ignoreToggle: true`（查看明文密码等明文特权场景）：门已验证仍直接放行，但门未验证/已过期时**无视总开关强制真实验证**——总开关关闭不能成为绕过明文保护的路径（50-53 行）。
- `gateFresh(verifiedAt)`（58-62 行）：TTL 内且 `elapsed >= 0`——**时钟回拨视为过期**，下次操作重弹 Hello，前后端时间戳在回拨后的时钟上重新对齐；后端 `is_within_ttl` 同样拒绝 `now < verified_at`，两侧方向一致。
- **`useSelfServiceVerify()`**（105-127 行）：自助服务面板会话门——切入面板验证一次，面板内后续操作（刷新/踢下线/上网记录查询）TTL 内共用；`config.selfReverifyEachAction === true` 时每次操作都验证；`config.selfHelloEnabled === false` 时整体放行。`resetSelfSessionGate()`（100-102 行）供面板卸载时重置。
- 消费方（grep 实证）：`useHelloGate` 被 `useOperatorBind.ts:4`、`auth/DashboardPanel.tsx:32`、`settings/useOnboardingFlow.ts:22` 使用；`biometricFailMessage` 另被 `settings/SettingsPanel.tsx:15` 使用；会话门由 `account/SelfServicePanel.tsx:23` 使用。

## Architecture

```text
┌───────────────────────────────────────────────────────────────┐
│ AccountPanel（AccountPanel.tsx，单列纵排：管理卡上/设置卡下）  │
│                                                               │
│  账号管理卡 :244-416                自动化设置卡 :418-473      │
│  ┌───────────────────────────┐   ┌─────────────────────────┐ │
│  │ OnlineDeviceStack 竖排     │   │ 三开关 Switch            │ │
│  │  (SelfOnlineStack.tsx)     │   │   ▼ onUpdateConfig       │ │
│  │   └ 账号卡×N（改名/切换）  │   │ autoLoginOnStart 等      │ │
│  └─────────────┬─────────────┘   └─────────────────────────┘ │
│                ▼ 快捷操作区 :395-413                           │
│  弹窗A LoginInfoDialog ◄── handleSaveLoginInfo :177-234        │
│  弹窗B BindOperatorDialog ◄── useOperatorBind :236-240         │
│  ChangePasswordDialog ◄── handleClearPassword :98-115          │
└───────────────────────────────────────────────────────────────┘
   ▲▲ config / accounts / passwordSaved    ▲ verifyWindowsIdentity
   useConfigStore（zustand）      selfServiceState.ts（Hello 两道门）

SelfServicePanel ──► SelfOnlineRecords（上网记录融合卡，双源 SessionRow）
DashboardPanel / SpeedTestPanel ──► OnlineDeviceStack(H) / StackCard
```

## Data Flow

**读取流**：`useConfigStore`（zustand）→ `AccountPanel` 经 `useShallow` 自订阅 `config`（`AccountPanel.tsx:61-63`）与点选 `passwordSaved`（59 行）；账号列表 `accounts` 经 props 传入。

**写入流**（账号信息）：

1. 新建：弹窗 A create 态 → `tauriApiWithRetry.createAccount()`（`AccountPanel.tsx:183`）→ `refreshAccounts()` 回读列表（195 行）→ `list_accounts` 扩展字段回流渲染。不切换激活账号。
2. 编辑：弹窗 A edit 态 → `saveConfigDirect(partial)`（211 行，密码留空 = 后端保留旧密码）→ `saveCurrentAsAccount(activeAccount)`（219 行）回写激活档案 → `refreshAccounts()`。
3. 清除密码：`saveConfigDirect({ password: '' }, true)`（103 行，显式 clearPassword 标志）→ `syncPasswordSaved(false)` + 激活卡就地修补 + `clearDirtyFields()`。
4. 三开关：`onUpdateConfig({ autoLoginOnStart | autoLoginOnPreparation | enableNightOperatorSwitch })` → App 外壳统一落配置。
5. 改名：`onRenameAccount(renamingId, trimmed)`（158 行，前端先过 `isValidDisplayName`）→ 后端 `validate_display_name` 终审。

**密码状态窗口修补**：`save_config`/`clearPassword` 只落主配置、档案回写滞后 → `applyActiveCardPasswordSaved`（92-96 行）就地改 store 激活卡 → 档案回写后 `list_accounts` 对齐。

**绑定流**：登录信息（`config.user` + 已保存密码）或弹窗 B 手动分段（`bindCredOverride`）→ `useOperatorBind` 四元组入参（239 行）→ 绑定/查询/揭示/改密；揭示受 `useHelloGate` 门控（`useOperatorBind.ts:4`）。

**上网记录流**：`SelfServicePanel` 持有全部状态 → `SelfOnlineRecords` 纯展示 → `recentToSessionRows`（dashboard 带回的近期流水，数组下标映射）/ `billToSessionRows`（账单查询，对象字段）归一为 `SessionRow` → `groupByDay` 日分组 + `aggregateStats` 聚合 → 近期 4 芯片 / 查询 8 芯片两态渲染。

**验证流**：操作触发 → `gateFresh()` 检查模块级时间戳（`helloGateVerifiedAt`，63 行 / `selfSessionVerifiedAt`，97 行）→ 未验证则 `verifyWindowsIdentity({ consentMessage })` → 成功写时间戳（77 / 117 行）；失败按 `BIOMETRIC_ERROR_KEY` 翻译 toast。绑定门与会话门时间戳互不共享。

## Connections

- [[frontend-account]]、[[android-frontend-account]]：账号域桌面/安卓主文章；本模块为其前端子域。
- [[desktop-frontend-account]]、[[desktop-account-selfservice]]：桌面端账号页与自助服务面板——本模块的同构参照（AccountPanel 三段式账号卡、SelfOnlineRecords 的 SessionRow 归一、OnlineDeviceCardStack 参数均对齐）。
- [[android-frontend-hooks-and-stores]]：`useConfigStore`（config/accounts/passwordSaved 订阅与 `saveConfigDirect`/`syncPasswordSaved`/`clearDirtyFields`/`setAccounts`）、`useLogToastStore`（addToast）、`tauriApiWithRetry` 的实现侧。
- [[android-frontend-settings]]：`AccountItem` / `Config` 类型与 `operatorLabelKey` 常量来源（`@/settings`、`@/settings/constants`）。
- [[android-frontend-primitives]]、[[android-frontend-shared-ui]]：Card / Dialog / Switch / Button / Input 等 UI 基件与 `AnimatedCard`、`MascotFigure`。
- [[android-frontend-monitor]]：自动化设置卡第二个开关复用 `monitor.autoLoginWhenReady` i18n 命名空间（`AccountPanel.tsx:448`）；`SpeedTestPanel` 消费堆叠组件（`monitor/SpeedTestPanel.tsx:16`）。
- [[android-frontend-network-face]]、[[frontend-face-verification]]：2D 人脸回退链的错误码（`faceTimeout` 等，`selfServiceState.ts:28-31`）与验证实现侧。
- [[decisions/account-display-name-id-separation]]：账号 id 与显示名分离的决策背景（改名语义、`AccountResult.displayName`）。
- [[decisions/windows-hello-only-identity]]：Windows Hello 身份验证的唯一通道决策（验证门的行为依据）。
- [[decisions/strictmode-dev-only]]：StrictMode 双挂载与 `mountedRef` 保护的决策背景。
- [[concepts/dual-platform-sharing]]：双端同构与单端复制的边界（本模块为安卓独立复刻树的一部分）。
- [[learnings/binding-operator-select-excludes-campus]]：绑定运营商下拉排除校园网的口径。
- [[learnings/android-frontend-no-unit-tests]]：安卓前端无单测的现状——jsdom 退化分支（ResizeObserver / pointer capture）由此而来。
- [[decisions/android-selfservice-empty-merge]]：自助服务空态合并决策（SelfOnlineRecords 空态渲染的上文）。

## Known Issues

1. **`onAddAccount` 声明未用**：`AccountPanelProps` 声明了 `onAddAccount`（`AccountPanel.tsx:36`），但组件解构跳过它（51-57 行）。2026-10-17 重排后"添加"统一走弹窗 A create 态 → `create_account`，该回调是旧「＋添加账号」路径的遗留接口；调用方（App 外壳）传了也会被忽略，清理时应与调用方一起移除。
2. **StrictMode 双挂载保护是手动的**：`mountedRef`（`AccountPanel.tsx:64` + effect 66-71 行）逐 handler 检查，漏一处 dev 模式下 setState 即被静默丢弃；这是约定而非框架保证（见 [[decisions/strictmode-dev-only]]）。
3. **Hello 门时间戳是模块级单例**：`helloGateVerifiedAt` / `selfSessionVerifiedAt`（`selfServiceState.ts:63`、97 行）为全局变量，同模块多实例共享且测试间可能互相污染（当前无单测故未暴露）；TTL 570s 与后端 600s 的 30s 余量在长时钟漂移下理论上可重现"前端放行、后端拒绝"，时钟回拨已由 `gateFresh` 的 `elapsed >= 0` 防住（58-62 行）。
4. **竖排堆叠吞掉页面滚动**：容器 `touchAction: 'none'`（`SelfOnlineStack.tsx:163`，注释 13 行）——竖排手势与 WebView 页面滚动冲突，页面滚动必须触堆叠以外区域；账号管理卡占满屏宽时移动端用户可能感知"滚不动"。
5. **jsdom 环境退化分支靠运行时探测**：`ResizeObserver` 缺失退化为一次性量高（`SelfOnlineStack.tsx:85`）、`setPointerCapture` 包 try/catch（111-114 行）——测试环境与真机行为路径不同，回归只能靠真机验证。
6. **`OnlineDeviceStack` 忽略 `variant`**：Props 声明 `variant?: 'coverflow' | 'narrow'`（`SelfOnlineStack.tsx:45`）但组件解构只有 `{ cards, focusIndex }`（50 行），竖排姿势表恒定；`variant` 仅 `OnlineDeviceStackH` 真正消费（343 行）。传了 variant 给竖排组件不会有任何效果。
7. **i18n 命名空间跨界**：自动化设置卡第二个开关的文案取自 `monitor.autoLoginWhenReady`（`AccountPanel.tsx:448`）而非 `account.*`，语义属于账号页、命名属于监控页——迁移 key 时两处需同步。
