---
title: zustand-store-config-and-theme
type: concept
source_files:
  - android/frontend/src/hooks/useConfigStore.ts
  - android/frontend/src/hooks/useThemeStore.ts
  - android/frontend/src/settings/ThemeDialog.tsx
tags:
  - zustand
  - state-management
  - configuration
  - theming
  - android-frontend
---

## Overview

`zustand-store-config-and-theme` 是 Android Tauri 前端应用中的核心状态管理概念，基于 `Zustand` 库构建，通过 `useConfigStore`（配置领域）与 `useThemeStore`（主题领域）两个独立 Store 将业务配置数据与 UI 视觉表现解耦。配置变更经防抖与脏标记（dirty-field）机制持久化至 Rust 后端，主题变更经全局订阅实时同步至 DOM CSS 变量。`ThemeDialog` 组件作为用户交互入口直接调用 Store 的 setter 与回调。该设计具备关窗数据防丢（`flushPendingConfig`）、保存失败自动放弃脏标记避免永久脱节等健壮性机制。

## Key Components

### `android/frontend/src/hooks/useConfigStore.ts`

模块级状态变量（非导出，供 Store 内部协程使用）：

- `api` (L12): 赋值为 `tauriApiWithRetry`，负责调用 Tauri 后端命令。
- `saveConfigTimer` (L14): `setTimeout` 句柄，用于 500ms 防抖保存。
- `saveConfigPending` (L15): `Partial<Config> | null`，合并待保存的配置片段。
- `saveConfigInFlight` (L17): `Promise<void> | null`，正在发送的保存 Promise，关窗时需等待。
- `dirtyFields` (L20): `Set<string>`，本地已修改但尚未被后端确认的字段名集合。
- `DIRTY_FAILURE_LIMIT` (L24): 常量 `3`，脏字段连续保存失败阈值。
- `dirtyFailureCounts` (L25): `Map<string, number>`，记录各脏字段连续失败次数。

接口 `ConfigStore` (L27-L46) 定义了 Store 形态，其字段与方法如下表：

| 类型 | 名称 / 签名 | 行号 | 说明 |
| :--- | :--- | :--- | :--- |
| field | `config: Config` | L28 | 当前完整配置对象，初始为 `DEFAULT_CONFIG` |
| field | `configLoaded: boolean` | L32 | 初始配置是否从后端加载完成（依赖启动逻辑等待） |
| field | `passwordSaved: boolean` | L33 | 密码是否已保存状态 |
| field | `accounts: AccountItem[]` | L34 | 账号列表 |
| field | `activeAccount: string` | L35 | 当前激活账号 |
| field | `language: string` | L36 | 语言设置，初始读 `safeStorage.get('app-language')` |
| method | `updateConfig: (partial: Partial<Config>) => void` | L37 | 合并配置、标脏并触发防抖保存 |
| method | `updateConfigLocal: (partial: Partial<Config>) => void` | L38 | 仅本地更新并标脏，不触发保存 |
| method | `mergeConfigFromBackend: (incoming: Partial<Config>) => void` | L39 | 合并后端 `config-changed` 回传，跳过脏字段 |
| method | `clearDirtyFields: () => void` | L40 | 清空脏标记与失败计数 |
| method | `syncPasswordSaved: (saved: boolean) => void` | L41 | 设置 `passwordSaved` 状态 |
| method | `saveConfigDirect: (cfg: Partial<Config>, clearPassword?: boolean) => Promise<void>` | L42 | 直接调用 API 持久化配置 |
| method | `setAccounts: (a: AccountItem[]) => void` | L43 | 设置账号列表 |
| method | `setActiveAccount: (a: string) => void` | L44 | 设置激活账号 |
| method | `setLanguage: (lang: string) => void` | L45 | 设置语言并切 `i18next` |

导出 Store `useConfigStore` (L48-L186) 具体实现：

- 初始状态 (L49-L54): `config: DEFAULT_CONFIG`, `configLoaded: false`, `passwordSaved: false`, `accounts: []`, `activeAccount: ''`, `language: safeStorage.get('app-language') || 'zh'`。
- `updateConfig` (L56-L88): 合并 `partial` 到 `config` (L58-L59)；将变更键加入 `dirtyFields` 并重置其失败计数 (L62-L65)；合并到 `saveConfigPending` (L67-L75)；清除旧 timer 并设 500ms 防抖调用 `saveConfigDirect` (L76-L86)；若含 `customThemeColor` 同步至 `useThemeStore` (L87)。
- `updateConfigLocal` (L90-L99): 同 `updateConfig` 的本地合并与标脏，但不防抖保存，仅同步主题色 (L98)。
- `mergeConfigFromBackend` (L103-L119): 遍历 `incoming`，若 `dirtyFields.has(k)` 则 `continue` 跳过 (L108)；合并后设置状态 (L115)；若后端回传 `password` 字段（`passwordFromBackend` 非空）则依 `PASSWORD_MASK` 同步 `passwordSaved` (L116-L118)。
- `clearDirtyFields` (L121-L124): 清空 `dirtyFields` 与 `dirtyFailureCounts`。
- `syncPasswordSaved` (L126): `set({ passwordSaved: saved })`。
- `saveConfigDirect` (L128-L176): 构造 `fullConfig` (L129)；执行 `api.saveConfig` (L132)；成功则删脏标记 (L134-L137)，若带非空密码则 `syncPasswordSaved(true)` (L140-L142)；捕获异常记录日志 (L145)，带密码失败则 `syncPasswordSaved(false)` (L148-L150)，累计失败计数达 `DIRTY_FAILURE_LIMIT` 放弃脏标记 (L152-L162) 并上抛 (L165)；`finally` 中仅当引用相等才清 `saveConfigInFlight` (L171-L174)。
- `setAccounts` (L178) / `setActiveAccount` (L179): 基础 setter。
- `setLanguage` (L181-L185): 更新状态、写 `safeStorage`、调用 `i18next.changeLanguage`。

导出函数：

- `hasPendingConfig` (L188-L193): 返回 `saveConfigPending !== null || saveConfigInFlight !== null`。
- `flushPendingConfig` (L195-L224): 清 timer (L196-L199)；若有 `saveConfigPending` 则剔除 `PASSWORD_MASK` 密码 (L205-L207) 后调用 `api.saveConfig` (L214) 并捕获错误写日志；将 flush 发出的 Promise 与 `saveConfigInFlight` 结合为 `Promise.allSettled` 返回 (L219-L223)。

### `android/frontend/src/hooks/useThemeStore.ts`

接口 `ThemeStore` (L9-L19) 成员如下表：

| 类型 | 名称 / 签名 | 行号 | 说明 |
| :--- | :--- | :--- | :--- |
| field | `themeName: ThemeName` | L10 | 主题名称 |
| field | `isLightMode: boolean` | L11 | 是否浅色模式 |
| field | `customThemeColor: string` | L12 | 自定义主题色（HEX） |
| field | `glassLevel: GlassLevel` | L13 | 玻璃质感档位 |
| method | `setThemeName: (name: ThemeName) => void` | L14 | 设置主题名 |
| method | `setIsLightMode: (v: boolean) => void` | L15 | 设置明暗 |
| method | `setGlassLevel: (v: GlassLevel) => void` | L16 | 设置玻璃档位 |
| method | `initTheme: (cfg: Partial<Config>) => void` | L17 | 根据存储与配置初始化主题 |
| method | `setCustomThemeColor: (color: string) => void` | L18 | 设置自定义色 |

导出 Store `useThemeStore` (L21-L57) 实现：

- 初始状态 (L22-L28): `themeName: 'default'`, `isLightMode` 读 `safeStorage` 的 `campus-light-mode` (L23), `customThemeColor: '#6366f1'`, `glassLevel` 读 `campus-glass-level` 并校验 `VALID_GLASS_LEVELS` (L25-L28)。
- 基础 setters (L30-L32)。
- `initTheme` (L34-L54): 读 `campus-theme` 校验后设 `themeName` (L35-L36)；依据 `campus-light-mode` 或 `cfg.themeMode`（`light`/`dark`/`system`）决定 `isLightMode` 并写回 storage (L37-L52)；若 `cfg.customThemeColor` 存在则设色 (L53)。
- `setCustomThemeColor` (L56): 简单 setter。

DOM 副作用订阅 (L60-L97)：

- 监听 `glassLevel` 变化 (L62-L65)：设 `data-glass` 属性及写 `safeStorage`。
- 监听 `isLightMode` 变化 (L66-L73)：切换 `dark` class 与 `data-light` 属性。
- 监听 `themeName`/`customThemeColor`/`isLightMode` 变化 (L74-L97)：移除旧主题 class；若为 `custom` 则用 `style.setProperty` 覆盖 `--primary`/`--ring`/`--accent`/`--accent-foreground` CSS 变量 (L84-L87)，否则 `removeProperty` 并添加对应 `theme-*` class (L89-L95)。

### `android/frontend/src/settings/ThemeDialog.tsx`

- 常量 `GLASS_ICONS` (L21): 映射 `off: Circle`, `standard: CircleDashed`, `ultra: CircleDot` 供 UI 渲染。
- 组件 `ThemeDialog` (L30-L127): Props 接口 `ThemeDialogProps` (L23-L28) 含 `open`, `onClose`, `onSetTheme`, `onToggleLightMode`。内部通过 `useThemeStore` 选择器读取 `themeName` (L31), `isLightMode` (L32), `glassLevel` (L33), `setGlassLevel` (L34)。渲染 `THEME_OPTIONS` 网格 (L50-L71) 调用 `onSetTheme` (L55)；渲染 `GLASS_OPTIONS` 网格 (L85-L105) 调用 `setGlassLevel(opt.value)` (L93)；明暗 `Switch` (L122) 调用 `onToggleLightMode`。

## Architecture

```text
┌──────────────────────┐
│ ThemeDialog (UI)     │ L30 ThemeDialog.tsx
│ props.onSetTheme     │
│ setGlassLevel        │
└───────┬───────┬──────┘
        │       │
        ▼       ▼
┌──────────────┐  ┌──────────────────┐
│useThemeStore │  │ useConfigStore   │ L48 useConfigStore.ts
│L21-L97       │  │ L48-L186         │
│ theme/glass  │◄─┤ updateConfig     │
└──────┬───────┘  └────────┬─────────┘
       │ DOM CSS vars      │ api.saveConfig (L132)
       ▼                   ▼
┌──────────┐        ┌────────────────────┐
│ DOM      │        │ tauriApiWithRetry  │
│ (L60-L97)│        │ (backend persist)  │
└──────────┘        └────────────────────┘
```

## Data Flow

1. **初始化**: `useConfigStore` 以 `DEFAULT_CONFIG` 启动 (L49)，`configLoaded` 为 `false` (L50)。`useThemeStore.initTheme` (L34) 读取 `safeStorage` 及 `Config` 决定 `themeName`/`isLightMode`/`customThemeColor`。全局订阅器 (L60-L97) 立即将初始状态同步至 DOM。
2. **用户修改主题**: 在 `ThemeDialog` 点击玻璃档位触发 `setGlassLevel` (L93) -> `useThemeStore` setter (L32) -> 订阅器 (L62-L65) 更新 DOM `data-glass` 及 `safeStorage`；若切自定义色则由 `updateConfig`/`updateConfigLocal` 经 L87/L98 调用 `setCustomThemeColor` 进而触发 CSS 变量重写。
3. **用户修改配置**: 调用 `updateConfig` (L56) 合并状态并标脏 (L62-L65)，启动 500ms 防抖 (L76-L86) 后执行 `saveConfigDirect` (L77-L84)。后者调用 `api.saveConfig` (L132) 持久化至 Rust 后端。
4. **后端回传**: 后端 `config-changed` 事件流入 `mergeConfigFromBackend` (L103)，跳过 `dirtyFields` (L108) 以防旧快照覆盖本地未确认值，并依据回传 `password` 字段同步 `passwordSaved` (L116-L118)。
5. **应用关闭**: 窗口关闭前调用 `flushPendingConfig` (L195) 清 timer，将 `saveConfigPending` 发往 `api.saveConfig` (L214) 并与 `saveConfigInFlight` (L219-L223) 一并等待，避免进程销毁丢数据；`hasPendingConfig` (L188) 用于决策是否进入 flush 分支。

## Connections

- [[android-rust-backend]]：`tauriApiWithRetry` 封装的 `saveConfig` 等 Tauri Commands（见 `useConfigStore.ts` L12, L132, L214）负责原子写入配置。
- [[log-toast-store]]：`useLogToastStore` 在 `useConfigStore.ts` L8 引入，用于保存失败/脏标记重置时的 `addToast`/`addLog`（L83, L145, L158, L216）。
- [[settings-constants]]：`DEFAULT_CONFIG`、`PASSWORD_MASK`、`VALID_THEMES`、`VALID_GLASS_LEVELS`、`THEME_OPTIONS`、`GLASS_OPTIONS` 均来源于 `@/settings/constants`（见 `useConfigStore.ts` L4-L5，`useThemeStore.ts` L4，`ThemeDialog.tsx` L14）。
- [[app-shell]]：应用挂载时需调用 `useThemeStore.initTheme` 及订阅 Store 以初始化全局样式与布局。
- [[i18n-module]]：`i18next` 在 `useConfigStore.ts` L10 引入，于 L83/L145/L184 等位置驱动多语言提示与语言切换。

## Known Issues

源码注释中记录的历史缺陷及修复状态如下：

1. **脏字段永久脱节（已修复）**：保存持续失败时 `dirtyFields` 中字段被 `mergeConfigFromBackend` 永久跳过，UI 与后端脱节。引入 `DIRTY_FAILURE_LIMIT=3` 与 `dirtyFailureCounts` (L21-L25)，在 `saveConfigDirect` 失败分支累计计数，达阈值放弃脏标记 (L152-L162) 并写警告日志。
2. **密码保存状态不同步（已修复）**：会话内保存密码后 `config-changed` 回写 `PASSWORD_MASK` 但 `passwordSaved` 从不置 `true`。修复：`saveConfigDirect` 成功且 `cfg.password` 非空时调用 `syncPasswordSaved(true)` (L140-L142)。
3. **复位密码脏标记残留（边界注意）**：复位密码的 dirty 标记会使清除密码后事件回传的空密码被跳过、`passwordSaved` 永久残留 `true`（见 L111-L113 注释）。当前逻辑中 `mergeConfigFromBackend` 仅当 `passwordFromBackend !== null` 时更新 `passwordSaved` (L116)，若 `password` 键仍在 `dirtyFields` 则被 `continue` 跳过 (L108)，需后续调用 `clearDirtyFields` (L121) 方可重新同步。
4. **关窗丢数据（pending 检查不全，已修复）**：`hasPendingConfig` 原仅检查 `saveConfigPending`，遗漏 in-flight 保存。修复：同时检查 `saveConfigInFlight` (L188-L193)。
5. **flush 保存未跟踪（已修复）**：`flushPendingConfig` 原发出保存不纳入 in-flight 跟踪，关窗直接跳过。修复：将 flush 发出的 Promise 返回并由调用方 `Promise.allSettled` 等待 (L210-L223)。
6. **内联样式无限增长（已修复）**：`useThemeStore` 原用 `cssText +=` 累积声明，多次切主题导致 inline style 无界增长。修复：改用 `style.setProperty` 覆盖同名 CSS 变量 (L82-L86)。