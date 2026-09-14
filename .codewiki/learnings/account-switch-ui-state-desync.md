---
title: 切换账号后 UI 不刷新——三条路径全部断链的静默契约断裂
type: learning
source_files:
  - tauri-app/src-tauri/src/commands/account.rs
  - tauri-app/src-tauri/src/infra/state/mod.rs
  - tauri-app/frontend/src/account/useAccount.ts
  - tauri-app/frontend/src/hooks/useEventListeners.ts
tags: [教训, 账号, 前端状态, IPC, zustand]
---

## 现象

切换账号成功后，前端页面高亮与账号列表不更新，要手动刷新（或重新进入面板）才看到新激活账号。而删除账号的刷新是正常的，且后端日志显示切换已成功落盘。

排查发现**三条链路同时断了**，任意一条修好都只让部分场景"看起来正常"：

1. **后端返回值缺失（`Option` + `skip_serializing_if` 的静默契约断裂）**：`switch_account` 成功路径曾返回 `AccountResult::ok(config)`，其 `active_account` 为 `None`；字段又标了 `#[serde(skip_serializing_if = "Option::is_none")]`（`infra/state/mod.rs:303-310`），于是 JSON 里 **`activeAccount` 键整个不存在**。TypeScript 侧 `SwitchAccountResult` 类型声明里它仍是 `string | undefined`——序列化层把字段删了、类型层毫不知情，编译期零告警。
2. **前端真值守卫把"缺失"当"无值"**：`useAccount.ts` 曾写 `if (result?.activeAccount) store.setActiveAccount(result.activeAccount)`。键不存在时 `undefined` 恒为假 → `setActiveAccount` 永不执行 → zustand 的 `activeAccount` 不更新，页面高亮停在上一个账号。
3. **事件路径不写独立字段**：托盘菜单"切换账号"走的是 `perform_switch_account_sync` → `save_config_to_disk_encrypted` → 广播 `config-changed` 事件，前端监听器（`useEventListeners.ts`）只调 `mergeConfigFromBackend(data.config)`——zustand 里与 `config` **并列的独立字段** `activeAccount`、`accounts` 不在 merge 范围内，事件更新对它们完全无效。也就是说：即使修好 1+2 让 IPC 返回路径恢复，托盘切换依旧不刷新。

## 根因

- ①+②是一对合谋：序列化层的 `skip_serializing_if` 是**运行期才发生的字段删除**，TS 类型与编译器都看不见；消费端再用真值判断而不是 `!== undefined`，"字段缺失"就被静默吞掉。
- ③是状态归一化不彻底：`config` 内部字段能跟着 `mergeConfigFromBackend` 走，但凡是提升到 store 顶层的独立状态（`activeAccount`、`accounts`），每个写入点都要单独记得同步——事件监听器漏了。

## 解决

三处同修（同一次提交）：

1. `perform_switch_account_sync` 改为返回 `Ok(safe_name)`（校验后的账号 id），命令层用 `AccountResult::ok_with_account(safe_name, ...)` 保证成功时 `activeAccount` 必然出现在 JSON 里（`commands/account.rs:16-40`）。
2. 前端守卫改 `if (result?.activeAccount !== undefined)`，与 `handleDeleteAccount` 的既有写法对齐（`useAccount.ts`）。
3. `useEventListeners.ts` 的 `config-changed` 分支补两步：从 `data.config.activeAccount` 比对后调 `setActiveAccount`；并拉一次 `listAccounts` 刷新账号列表——托盘切换走这条事件路径，不修它则一半场景仍漏。

## 教训

1. **`Option<T>` + `skip_serializing_if` 是跨语言的隐形契约**：TS 侧声明的"可选字段"实际可能是"不存在的键"。要么对"成功时必有"的语义字段不用 `skip_serializing_if`（序列化出 `null`/空串也行），要么前端守卫一律用 `!== undefined` 并意识到"缺失"本身就是一种取值。真值判断只适合"空串也不合法"的字段。
2. **zustand 中与 `config` 并列的独立字段不会随 `mergeConfigFromBackend` 更新**——后端广播一个"配置已变"事件，不等于前端所有派生状态都同步了。
3. **托盘菜单走的是事件路径而不是 IPC 返回路径**：同一个业务动作（切换账号）有两条到达前端的通道（命令返回值、`config-changed` 广播），只修返回值会漏掉托盘场景。修"UI 状态不刷新"类 bug 时，先**枚举所有写入该状态的路径**（IPC 返回、事件广播、启动初始化 `get_init_data`），逐条验证，而不是盯着报障的那一条。

## Connections

[[desktop-account-selfservice]]、[[account-display-name-id-separation]]、[[android-log-type-enum-drift]]
