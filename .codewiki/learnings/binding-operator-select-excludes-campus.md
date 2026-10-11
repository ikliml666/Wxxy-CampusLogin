---
title: "运营商下拉里没有无锡学院"先分清是哪个 Select
type: learning
source_files:
  - android/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - android/frontend/src/settings/useOnboardingFlow.ts
tags: [教训, 前端, 运营商, 误报]
---

## Overview

2026-09-19 用户报告“安卓端登录信息卡片运营商选择中的无锡学院消失”（vivo/OPPO 真机）。经浏览器实测（vite dev + `__TAURI_INTERNALS__` mock + 390×844 视口）与代码比对，**登录信息卡的运营商下拉 4 项齐全且落盘正确**，最终确认为报告者看错了下拉。

账号面板里有**三个**运营商相关 Select，语义不同：

| 位置 | 选项 | 语义 |
| --- | --- | --- |
| 登录信息卡「运营商」 | 4 项全（`ISP_OPTIONS.map`） | 登录认证后缀，`''` = 无锡学院 |
| 绑定运营商账号卡「运营商」 | 3 项（`ISP_OPTIONS.filter(o => o.value !== '__default__')`） | **故意排除无锡学院**：自助系统绑定运营商账号只针对办了运营商套餐的用户 |
| 新手向导绑定步（step1）下拉 | 3 项（同上 filter） | 同绑定卡 |

报告者看到的是**绑定卡/向导绑定步**的下拉。排除 `__default__` 是设计如此，不是缺陷。

## Key Components

### `android/frontend/src/account/AccountPanel.tsx`

- **Props**: `AccountPanelProps` (L1-L27)
    - `accounts`: `AccountItem[]`
    - `activeAccount`: `string`
    - `onUpdateConfig`: `(partial: Partial<Config>) => void`
    - `onAddAccount`, `onDeleteAccount`, `onSwitchAccount`, `onRenameAccount`
- **State & Refs**:
    - `passwordDraft`, `usernameDraft`: 本地草稿，避免输入暂停 >500ms 后 store 被 config-changed 回写 MASK 导致密码框 wipe、后续输入截断（历史缺陷 P1-F5）。输入期间不写 store，blur 时一次性提交。
    - `bindOp`, `bindPhone`, `bindSms`: 绑定运营商凭据状态。`BIND_OPERATOR_NONE = '__none__'` (L128)。
    - `bindStatuses`: 绑定状态查询结果，后端返回掩码账号，明文不经过前端 state。
    - `statusOpen`: 绑定状态弹窗开关 (L140)。
    - `revealedOp`, `revealedPassword`: 查看明文密码状态。
- **Logic**:
    - `handleBindOperator`: L185-L210。绑定运营商账号。
    - `fetchBindStatus`: L152-L177。查询绑定状态。
    - `handleReveal`: L179-L203。查看明文密码。
    - `handleChangePassword`: L212-L238。修改密码。

### `tauri-app/frontend/src/account/AccountPanel.tsx`

- **Props**: `AccountPanelProps` (L1-L28)
    - 新增 `adapters: Adapter[]`。
- **UI Structure**:
    - 左列：登录信息卡 + 自动化开关卡。
    - 右列：绑定卡 + 账号管理堆叠卡。
    - 底部对齐：通过 `grid grid-cols-2` 和 `flex flex-col gap-4` 实现。
- **Logic**:
    - `credReady`: L135。凭据门槛：登录信息已填学号且密码已保存。
    - `canBind`, `canQueryStatus`: L137-L140。基于 `credReady` 计算。
    - `fetchBindStatus`: L148-L173。支持 `opts?.openDialog` 参数控制是否打开弹窗。
    - `handleBindOperator`: L175-L200。绑定运营商账号。
    - `handleReveal`: L202-L226。查看明文密码。
    - `handleChangePassword`: L228-L254。修改密码。

### `android/frontend/src/settings/useOnboardingFlow.ts`

- **Constants**:
    - `ONBOARDING_STEP_COUNT = 4` (L14)
    - `BIND_OPERATOR_NONE = '__none__'` (L16)
    - `DEFAULT_OPERATOR = '__default__'` (L18)
- **State**:
    - `step`: 当前步骤 (0-3)。
    - `username`, `password`, `operator`: 账号信息。
    - `bindOperatorValue`, `phone`, `smsPassword`: 绑定信息。
    - `bindState`: `'idle' | 'loading' | 'success' | 'error'` (L23)。
- **Logic**:
    - `canBind`: L78-L83。绑定条件检查。
    - `handleBind`: L85-L113。执行绑定操作。
    - `handleNext`: L115-L130。账号步骤下一步落盘。
    - `handleLoginAndFinish`: L143-L172。完成步登录并关闭向导。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     AccountPanel (Android/Tauri)            │
│                                                             │
│  ┌──────────────────────┐   ┌──────────────────────────┐   │
│  │  Login Info Card     │   │  Bind Operator Card      │   │
│  │  (Left Column)       │   │  (Right Column)          │   │
│  │                      │   │                          │   │
│  │  Select: ISP_OPTIONS │   │  Select: ISP_OPTIONS     │   │
│  │  .map()              │   │  .filter(!__default__)   │   │
│  │  (All 4 options)     │   │  (3 options)             │   │
│  │                      │   │                          │   │
│  │  Input: Username     │   │  Input: Phone/SMS Pass   │   │
│  │  Input: Password     │   │                          │   │
│  └──────────────────────┘   └──────────────────────────┘   │
│                                                             │
│  ┌──────────────────────┐   ┌──────────────────────────┐   │
│  │  Auto Switch Card    │   │  Account Manage Card     │   │
│  │  (Left Column)       │   │  (Right Column)          │   │
│  │                      │   │                          │   │
│  │  Switches            │   │  OnlineDeviceCardStack   │   │
│  │  (Auto Login, etc.)  │   │  (Pager/Narrow Variant)  │   │
│  └──────────────────────┘   └──────────────────────────┘   │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  ChangePasswordDialog                                │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Dialog (Bind Status Query)                          │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Initialization**:
    - `useOnboardingFlow` 从 `configStore` 读取初始值。
    - `AccountPanel` 使用 `useShallow` 订阅 `config`，避免级联重渲染。
    - `bindOp` 初始化为 `config.operator` 或 `BIND_OPERATOR_NONE`。

2. **User Input**:
    - 用户名/密码输入使用本地草稿 (`usernameDraft`, `passwordDraft`)，在 `blur` 或 `Enter` 时提交到 `configStore`。
    - 运营商选择直接更新 `configStore`。

3. **Binding**:
    - 用户输入手机号和短信密码，点击“绑定”。
    - 调用 `tauriApiWithRetry.bindOperator`，传入 `account`, `password`, `operator`, `phone`, `smsPassword`。
    - 成功后清空 `smsPassword` 并刷新绑定状态。

4. **Status Query**:
    - 用户点击“查询绑定状态”，调用 `tauriApiWithRetry.getBindStatus`。
    - 结果显示在弹窗中，支持明文查看 (`handleReveal`)。

5. **Password Change**:
    - 用户通过 `ChangePasswordDialog` 修改密码。
    - 成功后更新 `configStore` 中的 `password` 并同步 `passwordSaved` 状态。

## Connections

[[radix-select-empty-string-value]]、[[decisions/night-operator-switch]]