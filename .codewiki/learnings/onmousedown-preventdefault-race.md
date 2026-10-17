---
title: AccountPanel 账号管理面板架构与状态流
type: module
source_files:
  - tauri-app/frontend/src/account/AccountPanel.tsx
tags: [前端, React, Tauri, 账号管理, 运营商绑定, 竞态保护]
---

## Overview

`AccountPanel` 是校园网客户端的前端账号管理核心组件，负责渲染账号配置、运营商绑定及多账号管理。当前代码已重构为基于 `useConfigStore`（Zustand）的状态管理模式，采用**本地草稿（Local Draft）**策略解决输入竞态问题。

核心改进点：
1. **输入解耦**：密码与用户名输入不再实时写入全局 Store，而是维护本地 State (`passwordDraft`, `usernameDraft`)，在 Blur/Enter 时提交，避免每键触发级联重渲染。
2. **StrictMode 兼容**：使用 `mountedRef` 防止 React StrictMode 下异步回调在组件卸载后更新 State。
3. **忙态锁**：切换账号、改名等操作均引入忙态标记，防止重复提交。
4. **运营商绑定流程外置**：复杂的绑定、查询、解密逻辑已提取至 `useOperatorBind` Hook，保持组件纯净。

## Key Components

### `AccountPanel` (Line 50-768)

主组件，负责渲染账号列表、自动化设置开关及各类弹窗。

#### Props

| 变量名 | 类型 | 用途 |
| :--- | :--- | :--- |
| `adapters` | `Adapter[]` | 网卡列表（保留用于兼容，主网卡字段按设计删除） |
| `accounts` | `AccountItem[]` | 账号列表 |
| `activeAccount` | `string` | 当前激活账号 ID |
| `onUpdateConfig` | `(partial: Partial<Config>) => void` | 更新配置回调 |
| `onAddAccount` | `(name: string) => Promise<boolean>` | 添加/回写账号回调 |
| `onDeleteAccount` | `(id: string, displayName: string) => void` | 删除账号回调 |
| `onSwitchAccount` | `(name: string) => Promise<void>` | 切换账号回调 |
| `onRenameAccount` | `(accountId: string, displayName: string) => Promise<boolean>` | 改名回调 |

#### State Variables

| 变量名 | 类型 | 行号范围 | 用途 |
| :--- | :--- | :--- | :--- |
| `passwordSaved` | `boolean` | L87 | 从 `useConfigStore` 订阅，标识是否已保存密码 |
| `config` | `Config` | L91 | 使用 `useShallow` 浅比较订阅，避免非必要重渲染 |
| `mountedRef` | `MutableRefObject<boolean>` | L93 | 防止 StrictMode 下异步 setState 失效 |
| `switchingAccount` | `string \| null` | L109 | 忙态锁，防止重复切换账号请求 |
| `renamingId` | `string \| null` | L121 | 当前正在改名的账号 ID |
| `renameDraft` | `string` | L122 | 改名时的本地草稿 |
| `renaming` | `boolean` | L123 | 改名操作中的忙碌状态 |
| `loginInfoMode` | `'create' \| 'edit' \| null` | L135 | 登录信息弹窗模式：新建/编辑/关闭 |
| `bindOpen` | `boolean` | L137 | 运营商绑定弹窗开关 |
| `bindCredSource` | `BindCredSource` | L139 | 绑定凭据来源：`login` (默认) / `manual` (本次使用) |
| `manualAccount` | `string` | L140 | 手动填写的账号 |
| `manualPassword` | `string` | L141 | 手动填写的密码 |

#### Handlers & Callbacks

| 函数名 | 行号范围 | 关键逻辑说明 |
| :--- | :--- | :--- |
| `handleClearPassword` | L96-L114 | 调用 `saveConfigDirect({ password: '' }, true)` 显式清除密码，同步 `passwordSaved` 状态 |
| `handleSwitchAccount` | L116-L121 | 忙态锁保护，若 `switchingAccount !== null` 则忽略再次点击 |
| `startRename` | L124-L127 | 初始化改名状态，设置 `renamingId` 和 `renameDraft` |
| `cancelRename` | L129-L132 | 重置改名状态 |
| `submitRename` | L134-L154 | 校验显示名合法性，调用 `onRenameAccount`，失败保留草稿供重试 |
| `handleSaveLoginInfo` | L176-L215 | 处理登录信息保存：<br>- **新建态**：调用 `tauriApiWithRetry.createAccount`，成功后刷新列表<br>- **编辑态**：先 `saveConfigDirect` 提交 user/password/operator，再调用 `onAddAccount` 回写激活档案 |
| `refreshAccounts` | L217-L225 | 调用 `tauriApiWithRetry.listAccounts` 刷新账号列表 |

#### useOperatorBind Hook 解构

以下变量/方法来自 `useOperatorBind` Hook (L157-L169)，封装了运营商绑定的完整生命周期：

| 变量/方法 | 类型/用途 |
| :--- | :--- |
| `bindOp` | 选中的运营商值 |
| `bindPhone` | 绑定手机号草稿 |
| `bindSms` | 绑定短信验证码草稿 |
| `binding` | 绑定操作忙碌状态 |
| `bindStatuses` | 查询到的运营商绑定状态缓存 |
| `queryingStatus` | 查询状态忙碌标志 |
| `revealedOp` | 当前查看明文的运营商 |
| `revealedPassword` | 查看明文时存储的密码 |
| `statusOpen` | 绑定状态弹窗开关 |
| `chpwdOpen` | 修改密码弹窗开关 |
| `canBind` | 是否允许发起绑定 |
| `canQueryStatus` | 是否允许查询状态 |
| `fetchBindStatus` | 查询运营商绑定状态 |
| `handleReveal` | 查看明文运营商密码 |
| `handleBindOperator` | 执行运营商绑定操作 |
| `handleChangePassword` | 修改登录密码 |

## Architecture

`AccountPanel` 采用**单列纵排布局**（Card Stack），上为账号管理卡（整合登录信息+绑定入口），下为自动化设置卡。核心架构改进在于解耦了 UI 输入与全局 Store 的同步时机，并将复杂业务逻辑外置。

```text
┌──────────────────────────────────────────────────────────────┐
│                      AccountPanel                            │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Account Manage Card (AnimatedCard)                     │  │
│  │                                                        │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │ OnlineDeviceCardStack (variant="narrow")         │  │  │
│  │  │  - Active Card (ring-primary)                    │  │  │
│  │  │    - DisplayName / User / Operator / Pass Status │  │  │
│  │  │    - Actions: Edit Login Info / Rename / Switch  │  │  │
│  │  │          / Delete                                │  │  │
│  │  └──────────────────────────────────────────────────┘  │  │
│  │                                                        │  │
│  │  Quick Actions: [Edit Login Info] [Bind Operator]      │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Auto Switch Card (AnimatedCard)                        │  │
│  │  - Auto Login on Start                                 │  │
│  │  - Auto Login on Preparation                           │  │
│  │  - Night Operator Switch                               │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Modals & Dialogs                                       │  │
│  │ - LoginInfoDialog (Create/Edit)                        │  │
│  │ - BindOperatorDialog (Cred Source: Login/Manual)       │  │
│  │ - ChangePasswordDialog                                 │  │
│  │ - Dialog (Bind Status Query + Reveal)                  │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### 数据流与竞态保护机制

1.  **输入阶段 (Input Phase)**:
    *   用户打字 → 触发 `onChange` → 更新本地 State (`passwordDraft` / `usernameDraft` 由子组件或父组件逻辑管理)。
    *   **不触发** `onUpdateConfig`，因此不引起全局 Store 更新，不触发级联重渲染。

2.  **提交阶段 (Commit Phase)**:
    *   **Blur/Enter**: 触发相应的提交逻辑（如 `submitRename` 或 `handleSaveLoginInfo`）。
    *   **校验**: 检查草稿是否为空或与现有值不同。
    *   **持久化**: 调用 `onUpdateConfig` 或 `saveConfigDirect`。
    *   **清理**: 重置本地草稿 State。

3.  **特殊操作保护 (Special Actions)**:
    *   **Clear Password**: `handleClearPassword` 直接调用 Store 并同步状态，不依赖 UI 事件的 Blur 提交，避免了旧版中 Blur 先于 Click Handler 执行的竞态。
    *   **Busy Locks**: `switchingAccount` 和 `renaming` 状态确保在异步操作完成前，UI 按钮被禁用或点击被忽略。

## Data Flow

1.  **配置加载**: `useConfigStore` 提供 `config` 和 `passwordSaved`，通过 `useShallow` 优化渲染。
2.  **账号列表**: `accounts` 由 App 外壳传入，可通过 `refreshAccounts` 主动刷新。
3.  **运营商绑定**:
    *   用户点击 "Bind Operator" → 打开 `BindOperatorDialog`。
    *   选择凭据来源 (`login` 或 `manual`)。
    *   调用 `handleBindOperator` → 触发后端绑定流程。
    *   查询状态 `fetchBindStatus` → 返回 `bindStatuses`。
    *   查看明文 `handleReveal` → 返回 `revealedPassword`。
4.  **账号切换**:
    *   点击切换 → `handleSwitchAccount` 设置 `switchingAccount` 忙态。
    *   调用 `onSwitchAccount` → 后端切换上下文。
    *   完成后清除忙态。

## Connections

[[learnings/onmousedown-preventdefault-race]] (底层竞态原理参考)
[[tauri-app/frontend/src/hooks/useConfigStore]] (全局配置 Store 实现)
[[tauri-app/frontend/src/account/useOperatorBind]] (运营商绑定逻辑外置 Hook)
[[tauri-app/frontend/src/account/LoginInfoDialog]] (登录信息编辑弹窗)
[[tauri-app/frontend/src/account/BindOperatorDialog]] (运营商绑定弹窗)

## Known Issues

1.  **StrictMode Double Invoke**:
    *   文件 `AccountPanel.tsx` L93-L97 使用 `mountedRef` 处理 React StrictMode 下的双重挂载/卸载问题。所有异步回调（如 `handleAddAccount`, `submitRename`）均在最后检查 `if (!mountedRef.current) return`，以防止在组件卸载后更新 State 导致的内存泄漏或错误。

2.  **Operator Binding Status Cache**:
    *   `bindStatuses` 状态在弹窗关闭后不会被自动清除，除非重新触发查询或刷新页面。这可能导致用户看到过期的绑定状态。建议在 `Dialog` 关闭回调中考虑重置 `bindStatuses` 或 `queryingStatus`（当前实现未显式重置，依赖下次查询覆盖）。

3.  **Main Network Card Field Deleted**:
    *   注释提到 `adapters` prop 保留以兼容 App 传参，但主网卡字段按设计删除（保持自动检测）。需确保 App 外壳不再传递无效的网卡选择逻辑。