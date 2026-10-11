---
title: Hello 验证门与自助服务会话门的状态管理
type: learning
source_files:
  - tauri-app/frontend/src/account/selfServiceState.ts
  - tauri-app/frontend/src/account/AccountPanel.tsx
  - tauri-app/frontend/src/settings/OnboardingWizard.tsx
tags: [教训, 前端, 验证门, 单例, 状态, Windows Hello]
---

## Overview

本文档记录 `selfServiceState.ts` 中实现的 Windows Hello 验证门（Verification Gate）机制。该模块通过模块级单例变量维护验证时间戳，实现跨组件的“一次验证，TTL 内复用”行为。主要包含两个独立的门：
1. **绑定操作门** (`useHelloGate`)：用于账号面板的绑定、查询明文密码等操作。
2. **自助服务会话门** (`useSelfServiceVerify`)：用于自助服务面板内的后续操作（刷新、踢下线等）。

当前代码存在一个设计缺陷：这两个门均为模块级全局状态，且缺乏显式的重置入口（除会话门外），导致在向导（Wizard）中验证后，账号面板可能跳过验证，反之亦然。

## Key Components

### `selfServiceState.ts`

- **`helloEnabled()`**: 
  - 读取 `config.selfHelloEnabled`。若配置缺失或为 `true`，返回 `true`（总开关开启）。
  - 注释指出：查看明文密码的验证不走此门，不受此开关限制（通过 `ignoreToggle` 参数控制）。

- **`VERIFY_TTL_MS`**: 
  - 常量 `570_000` (570秒)。
  - 与后端 `IDENTITY_VERIFY_TTL_SECS=600s` 对齐，预留 30s 时钟余量。

- **`gateFresh(verifiedAt)`**: 
  - 判定门是否新鲜。
  - 条件：`verifiedAt > 0` 且 `0 <= elapsed < VERIFY_TTL_MS`。
  - 处理时钟回拨：若 `elapsed < 0`，视为过期，强制重新验证以避免前后端死锁。

- **`helloGateVerifiedAt`**: 
  - `let` 声明的模块级变量，存储绑定操作门的最后验证时间戳。

- **`useHelloGate(options?)`**: 
  - Hook 函数，返回一个异步验证回调。
  - **逻辑**:
    1. 若 `gateFresh(helloGateVerifiedAt)` 为真，直接返回 `true`（复用缓存）。
    2. 若 `!ignoreToggle && !helloEnabled()`，直接返回 `true`（总开关关闭时放行）。
    3. 调用 `tauriApiWithRetry.verifyWindowsIdentity`。
    4. 成功则更新 `helloGateVerifiedAt = Date.now()` 并返回 `true`。
    5. 失败则 Toast 错误并返回 `false`。
  - **选项**: `ignoreToggle?: boolean`。若为 `true`（如查看明文密码），即使总开关关闭也强制验证。

- **`selfSessionVerifiedAt`**: 
  - `let` 声明的模块级变量，存储自助服务会话门的最后验证时间戳。

- **`resetSelfSessionGate()`**: 
  - 将 `selfSessionVerifiedAt` 重置为 `0`。
  - 由面板卸载时调用，确保下次进入面板需重新验证。

- **`useSelfServiceVerify()`**: 
  - Hook 函数，返回自助服务面板操作的验证回调。
  - **逻辑**:
    1. 若 `!helloEnabled()`，返回 `true`。
    2. 检查 `config.selfReverifyEachAction`。若为 `true`，强制每次验证；否则检查 `gateFresh(selfSessionVerifiedAt)`。
    3. 若新鲜，返回 `true`。
    4. 调用后端验证 API，成功则更新时间戳并返回 `true`。
    5. 失败则返回 `false`。

### `AccountPanel.tsx`

- **`ensureHelloVerified`**: 
  - 使用 `useHelloGate()` 获取绑定操作门的验证函数。
  - 应用于：`fetchBindStatus`, `handleReveal`, `handleBindOperator`, `handleChangePassword`。

- **`ensureRevealVerified`**: 
  - 使用 `useHelloGate({ ignoreToggle: true })` 获取明文查看专用的验证函数。
  - 应用于：`handleReveal`。
  - **注意**: 这里使用了 `ignoreToggle: true`，意味着即使 `config.selfHelloEnabled` 为 `false`，查看明文密码仍需通过 Windows Hello 验证。

- **`fetchBindStatus`**: 
  - 查询绑定状态。先调用 `ensureHelloVerified()`，若失败则中止。

- **`handleReveal`**: 
  - 查看运营商明文密码。先调用 `ensureRevealVerified()`，若失败则中止。

- **`handleBindOperator`**: 
  - 执行绑定操作。先调用 `ensureHelloVerified()`，若失败则中止。

- **`handleChangePassword`**: 
  - 修改密码。先调用 `ensureHelloVerified()`，若失败则中止。

### `OnboardingWizard.tsx`

- **`ensureBindHello`**: 
  - 使用 `useHelloGate()` 获取绑定操作门的验证函数。
  - 应用于：`handleBind`（向导中的绑定步骤）。

- **`handleBind`**: 
  - 向导中的绑定操作。先调用 `ensureBindHello()`，若失败则中止。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      selfServiceState.ts                     │
│                                                              │
│  ┌─────────────────────┐   ┌────────────────────────────┐   │
│  │ helloGateVerifiedAt │   │ selfSessionVerifiedAt      │   │
│  │ (Global State)      │   │ (Global State)             │   │
│  └─────────┬───────────┘   └────────────┬───────────────┘   │
│            │                           │                   │
│  ┌─────────▼───────────┐   ┌───────────▼─────────────────┐  │
│  │ useHelloGate()      │   │ useSelfServiceVerify()      │  │
│  │ (Binding Gate)      │   │ (Session Gate)              │  │
│  └─────────┬───────────┘   └───────────┬─────────────────┘  │
│            │                           │                   │
│  ┌─────────▼───────────┐   ┌───────────▼─────────────────┐  │
│  │ AccountPanel        │   │ SelfServicePanel (Reset)    │  │
│  │ - ensureHelloVerif. │   │ - resetSelfSessionGate()    │  │
│  │ - ensureRevealVerif.│   └─────────────────────────────┘  │
│  └─────────┬───────────┘                                    │
│            │                                               │
│  ┌─────────▼───────────┐                                    │
│  │ OnboardingWizard    │                                    │
│  │ - ensureBindHello   │                                    │
│  └─────────────────────┘                                    │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **初始化**: 模块加载时，`helloGateVerifiedAt` 和 `selfSessionVerifiedAt` 初始化为 `0`。
2. **首次操作**: 用户触发绑定、查询或查看明文操作。
   - Hook 调用 `gateFresh(0)` 返回 `false`。
   - 若总开关开启，调用后端 `verifyWindowsIdentity`。
   - 成功后，更新对应的时间戳变量为 `Date.now()`。
3. **复用阶段**: 在 `VERIFY_TTL_MS` (570s) 内，后续相同类型的操作直接返回 `true`，跳过 Windows Hello 弹窗。
4. **过期/重置**:
   - **绑定门**: 仅当 TTL 过期或时钟回拨时自动失效。**无显式重置入口**。
   - **会话门**: 可通过 `resetSelfSessionGate()` 显式重置为 `0`。

## Connections

[[windows-hello-only-identity]]、[[verification-gate-tiers]]

## Known Issues

1. **跨面板状态泄漏**: 
   - `helloGateVerifiedAt` 是模块级单例，被 `AccountPanel` 和 `OnboardingWizard` 共享。
   - **现象**: 在向导中完成绑定验证后，回到账号面板进行绑定/查询操作时，若 TTL 未过期，将不再弹出 Windows Hello 验证。反之，若在账号面板验证过，向导中的绑定操作也可能跳过验证。
   - **位置**: `selfServiceState.ts` 中 `helloGateVerifiedAt` 变量。
   - **影响**: 降低了安全性预期，用户可能在非预期场景下跳过生物识别验证。

2. **缺乏显式重置**: 
   - 绑定门没有类似 `resetSelfSessionGate()` 的公开重置函数。
   - **后果**: 测试时需依赖 `vi.resetModules()` 重置模块状态；生产环境中只能等待 TTL 过期或应用重启。
   - **位置**: `selfServiceState.ts` 中无绑定门的重置函数。

3. **时钟回拨处理**: 
   - `gateFresh` 函数已处理时钟回拨（`elapsed < 0` 视为过期），避免前后端 TTL 不一致导致的死锁。这是正确的防御性编程实践。