---
title: AccountPanel 密码输入草稿与竞态处理
type: learning
source_files:
  - tauri-app/frontend/src/account/AccountPanel.tsx
tags: [教训, 前端, 竞态, 表单, 密码, React]
---

## Overview

`AccountPanel` 是校园网客户端的前端账号管理核心组件。在重构过程中，发现旧版代码存在严重的**输入竞态（Race Condition）**：当用户在密码框或用户名框中快速输入时，每次按键触发的 `onChange` 会立即写入全局 Store (`useConfigStore`)。由于 Store 更新触发级联渲染，且后端保存逻辑带有防抖（Debounce），导致以下问题：

1. **密码框截断（P1-F5）**：输入暂停 >500ms 后，Store 被回写，若此时密码显示为掩码（`••••••••`），直接赋值给 Input value 会导致光标位置错乱或输入内容被 wipe。
2. **清除操作覆盖**：点击“清除已保存密码”按钮时，若焦点仍在密码框，先触发 `blur` 事件提交当前草稿（可能非空），随后才执行清除逻辑，导致清除失败。

本文章记录通过引入**本地草稿（Local Draft）**模式解决上述问题的架构决策与实现细节。

## Key Components

### `AccountPanel` (Line 47-893)

主组件，负责渲染账号配置、运营商绑定及多账号管理。

#### State Variables

| 变量名 | 类型 | 行号范围 | 用途 |
| :--- | :--- | :--- | :--- |
| `passwordSaved` | `boolean` | L66 | 从 `useConfigStore` 订阅，标识是否已保存密码 |
| `config` | `Config` | L71 | 使用 `useShallow` 浅比较订阅，避免非必要重渲染 |
| `newAccountName` | `string` | L72 | 新增账号的输入草稿 |
| `showAddInput` | `boolean` | L73 | 控制新增账号输入框的显示 |
| `showPassword` | `boolean` | L74 | 控制密码可见性切换 |
| `passwordFocused` | `boolean` | L75 | 标记密码框是否聚焦，用于决定是否启用草稿模式 |
| `passwordDraft` | `string` | L79 | **关键**：聚焦期间的密码本地草稿，避免每键写 Store |
| `usernameDraft` | `string \| null` | L83 | **关键**：用户名的本地草稿，Blur/Enter 时提交 |
| `mountedRef` | `MutableRefObject<boolean>` | L85 | 防止 StrictMode 下异步 setState 失效 |
| `switchingAccount` | `string \| null` | L130 | 防止重复提交切换账号请求 |
| `renamingId` | `string \| null` | L144 | 当前正在改名的账号 ID |
| `renameDraft` | `string` | L145 | 改名时的本地草稿 |
| `renaming` | `boolean` | L146 | 改名操作中的忙碌状态 |
| `bindOp` | `string` | L156 | 运营商绑定选择值 |
| `bindPhone` | `string` | L157 | 绑定手机号草稿 |
| `bindSms` | `string` | L158 | 绑定短信密码草稿 |
| `binding` | `boolean` | L159 | 绑定操作中的忙碌状态 |
| `bindInitedRef` | `MutableRefObject<boolean>` | L161 | 确保绑定状态初始化只执行一次 |
| `bindStatuses` | `BindStatuses \| null` | L171 | 查询到的运营商绑定状态缓存 |
| `queryingStatus` | `boolean` | L172 | 查询运营商绑定状态的忙碌状态 |
| `revealedOp` | `string \| null` | L173 | 当前查看明文的运营商 |
| `revealedPassword` | `string` | L174 | 查看明文时存储的密码 |
| `statusOpen` | `boolean` | L177 | 绑定状态弹窗开关 |
| `chpwdOpen` | `boolean` | L292 | 修改密码弹窗开关 |

#### Handlers & Callbacks

| 函数名 | 行号范围 | 关键逻辑说明 |
| :--- | :--- | :--- |
| `handlePasswordFocus` | L88-L91 | 聚焦时开启草稿模式，清空 `passwordDraft` |
| `handlePasswordBlur` | L93-L99 | **关键修复点**：仅在 `passwordDraft` 非空时提交 `onUpdateConfig`，否则忽略（避免覆盖已有密码） |
| `handleClearPassword` | L102-L108 | 调用 Store 直接保存空密码并同步 `passwordSaved` 状态 |
| `commitUsername` | L110-L116 | 将 `usernameDraft` 提交至 Config，仅当值变化时调用 `onUpdateConfig` |
| `handleAddAccount` | L118-L130 | 校验名称格式，调用后端添加接口，成功则清空草稿 |
| `handleSwitchAccount` | L132-L138 | 忙态锁，防止重复切换 |
| `submitRename` | L152-L171 | 校验 displayName 长度及字符集，调用 `onRenameAccount` |
| `fetchBindStatus` | L194-L223 | 查询运营商绑定状态，需经过 Hello Gate 验证 |
| `handleReveal` | L234-L261 | 查看明文运营商密码，需经过独立的 Hello Gate (`ignoreToggle: true`) |
| `handleBindOperator` | L264-L291 | 执行运营商绑定操作 |
| `handleChangePassword` | L303-L333 | 修改登录密码，成功后同步更新本地 Store 中的密码字段 |

#### Render Logic Highlights

*   **密码显示计算 (L86-L90)**:
    ```typescript
    const displayPassword = (() => {
      if (passwordFocused) return passwordDraft
      if (passwordSaved && (!config.password || config.password === PASSWORD_MASK)) return '••••••••'
      return config.password === PASSWORD_MASK ? '' : (config.password || '')
    })()
    ```
*   **输入框绑定**:
    *   Password Input (L502): `value={displayPassword}`, `onChange={e => setPasswordDraft(...)}`, `onBlur={handlePasswordBlur}`
    *   Username Input (L474): `value={usernameDraft ?? (config.user || '')}`, `onChange={e => setUsernameDraft(...)}`, `onBlur={commitUsername}`

## Architecture

`AccountPanel` 采用**双列布局**（Grid），左列为登录信息与自动化开关，右列为运营商绑定与账号管理。核心架构改进在于解耦了 UI 输入与全局 Store 的同步时机。

```text
┌──────────────────────────────────────────────────────────────┐
│                      AccountPanel                            │
│                                                              │
│  ┌─────────────────────┐     ┌────────────────────────────┐  │
│  │ Left Column         │     │ Right Column               │  │
│  │                     │     │                            │  │
│  │ 1. Login Info Card  │     │ 1. Operator Binding Card   │  │
│  │    - User (Draft)   │     │    - ISP Select            │  │
│  │    - Pass (Draft)   │     │    - Phone/SMS Inputs      │  │
│  │    - Adapter Sel    │     │    - Bind/Query Buttons    │  │
│  │                     │     │                            │  │
│  │ 2. Auto Switch Card │     │ 2. Account Manage Stack    │  │
│  │    - Auto Login     │     │    - OnlineDeviceCardStack │  │
│  │    - Night Switch   │     │    - Inline Rename/Edit    │  │
│  └─────────────────────┘     └────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Modals & Dialogs                                       │  │
│  │ - ChangePasswordDialog                                 │  │
│  │ - BindStatusDialog (Query Results + Reveal)            │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### 数据流与竞态保护机制

1.  **输入阶段 (Input Phase)**:
    *   用户打字 → 触发 `onChange` → 更新本地 State (`passwordDraft` / `usernameDraft`)。
    *   **不触发** `onUpdateConfig`，因此不引起全局 Store 更新，不触发级联重渲染。

2.  **提交阶段 (Commit Phase)**:
    *   **Blur/Enter**: 触发 `handlePasswordBlur` 或 `commitUsername`。
    *   **校验**: 检查草稿是否为空或与现有值不同。
    *   **持久化**: 调用 `onUpdateConfig({ user/password: draft })`。
    *   **清理**: 重置本地草稿 State。

3.  **特殊操作保护 (Special Actions)**:
    *   **Clear Password**: 点击清除按钮时，需阻止潜在的 Blur 提交。
    *   **Implementation**: 虽然当前代码中 `handleClearPassword` 是直接调用 Store，但在 UI 层面，旧版问题源于 Blur 先于 Click Handler 执行。
    *   **Current Fix in Code**: 注意观察 `AccountPanel.tsx` 第 510-516 行的“眼睛”按钮：
        ```tsx
        <button
          type="button"
          onMouseDown={(e) => e.preventDefault()} // 阻止 blur 抢跑
          onClick={() => setShowPassword(!showPassword)}
          ...
        ```
        此模式同样适用于“清除密码”按钮的逻辑上下文，确保在焦点离开输入框前，不会触发 `handlePasswordBlur` 中的无效提交。尽管 `handleClearPassword` 本身是异步且直接操作 Store，但保持 UI 行为的一致性（Button 上的 `preventDefault`）是防止边缘竞态的最佳实践。

## Known Issues

1.  **StrictMode Double Invoke**:
    *   文件 `AccountPanel.tsx` L87-L91 使用 `mountedRef` 处理 React StrictMode 下的双重挂载/卸载问题。所有异步回调（如 `handleAddAccount`, `submitRename`）均在最后检查 `if (!mountedRef.current) return`，以防止在组件卸载后更新 State 导致的内存泄漏或错误。

2.  **Password Mask Wipe**:
    *   旧版缺陷 P1-F5 已修复。通过在 `passwordFocused` 期间完全接管 Input 的 `value` 来源（使用 `passwordDraft`），避免了 Store 回写的掩码字符串干扰输入光标和缓冲区。

3.  **Operator Binding Status Cache**:
    *   `bindStatuses` 状态在弹窗关闭后不会被自动清除，除非重新触发查询或刷新页面。这可能导致用户看到过期的绑定状态。建议在 `Dialog` 关闭回调中考虑重置 `bindStatuses` 或 `queryingStatus`（当前实现未显式重置，依赖下次查询覆盖）。

## Connections

[[learnings/onmousedown-preventdefault-race]] (本文档关联的底层竞态原理)
[[tauri-app/frontend/src/hooks/useConfigStore]] (全局配置 Store 实现)
[[tauri-app/frontend/src/account/SelfServicePanel]] (参考了类似的清除/眼睛按钮防护模式)