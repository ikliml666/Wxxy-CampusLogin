---
title: shared_ui
type: module
source_files:
  - tauri-app/frontend/src/shared/index.ts
  - tauri-app/frontend/src/shared/types.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - tauri-app/frontend/src/shared/ui-types.ts
  - tauri-app/frontend/src/shared/LogPanel.tsx
  - tauri-app/frontend/src/shared/ConfirmDialog.tsx
  - tauri-app/frontend/src/shared/AnimatedNumber.tsx
  - tauri-app/frontend/src/shared/RefreshButton.tsx
  - tauri-app/frontend/src/shared/SegmentTabs.tsx
  - tauri-app/frontend/src/shared/ToastContainer.tsx
  - tauri-app/frontend/src/shared/FluidBackground.tsx
  - tauri-app/frontend/src/shared/ErrorBoundary.tsx
  - tauri-app/frontend/src/shared/MascotFigure.tsx
  - tauri-app/frontend/src/shared/IspMark.tsx
  - tauri-app/frontend/src/shared/SponsorCard.tsx
  - android/frontend/src/shared/index.ts
  - android/frontend/src/shared/UpdateAvailableDialog.tsx
  - android/frontend/src/shared/dailyMascot.ts
  - android/frontend/src/shared/IspMark.tsx
  - android/frontend/src/shared/LogPanel.tsx
tags: [ui, components, shared, react, frontend, animation, mascot]
---

## Overview

`shared_ui` 是双端（桌面 Tauri 应用 + 安卓应用）前端共用的**通用 UI 组件与类型/常量**层，代码位于 `tauri-app/frontend/src/shared/` 与 `android/frontend/src/shared/`。该目录通过 `index.ts` 统一对外导出（见 `tauri-app/frontend/src/shared/index.ts` 第 1–13 行、 `android/frontend/src/shared/index.ts` 第 1–13 行），其它模块一律通过 `@/shared` 引入，不直接引用具体文件。

它包含两类内容：

1. **可复用展示组件**：日志面板 `LogPanel`、确认弹窗 `ConfirmDialog`、数字滚动动画 `AnimatedNumber`、刷新按钮 `RefreshButton`、分段标签 `SegmentTabs`、全局 Toast `ToastContainer`、流体背景 `FluidBackground`、错误边界 `ErrorBoundary`、看板娘图 `MascotFigure`、运营商标识 `IspMark`、赞助浮层 `SponsorCard`。
2. **跨模块共享的类型与常量**：`types.ts`（更新/下载/镜像等后端协议类型）、`ui-types.ts`（前端通用 UI 类型，如 `StatusState`、`LogEntry`、`ToastMessage`）、`ui-constants.ts`（版本号、导航项、日志上限等）。

该模块**只在 `index.ts` 暴露的内容上有契约**，组件本体不被外部直接 import；安卓端额外导出 `UpdateAvailableDialog`（新版本提醒弹窗）与 `dailyMascot.ts`（每日娘轮换），但桌面端 `index.ts` 未导出这两个文件（见 `android/frontend/src/shared/index.ts` 第 1–13 行），它们属于安卓专属补充。

> 注意：仓库中**没有**针对本模块的单元测试（目录下无 `*.test.ts(x)` 文件），文中所有行号均来自源码实际读取。

## Key Components

### 共享类型 —— `types.ts`

均在 `tauri-app/frontend/src/shared/types.ts`（与安卓端 `android/frontend/src/shared/types.ts` 同构）：

- `UpdateAvailableData`（第 1–5 行）：更新检查响应，字段 `hasUpdate: boolean`、`latestVersion: string`、`releaseNotes?: string`。
- `UpdateInfo`（第 7–17 行）：更完整的更新信息；含 `assets: { name; url; size }[]`、`sha256Checksum?`、`lastCheckError: string | null`（上一次检查失败原因快照）、`lastCheckTime: number | null`（上一次检查完成 unix ms，从未检查为 null）。
- `DownloadProgress`（第 19–24 行）：`downloaded`、`total`、`speed`、`percent` 四字段，用于下载进度展示。
- `MirrorSource`（第 26–30 行）：`name`、`url`、`description`，单条镜像源配置。

### 共享 UI 类型 —— `ui-types.ts`

位于 `tauri-app/frontend/src/shared/ui-types.ts`（安卓端同构）：

| 类型/接口 | 行号 | 说明 |
| --- | --- | --- |
| `StatusState` | 1 | `'loading' \| 'online' \| 'offline' \| 'error'` |
| `PanelName` | 2 | 9 个面板路由名 union |
| `ThemeName` | 3 | 主题名：default/vibrant/forest/midnight/ocean/cherry/custom |
| `GlassLevel` | 5 | 玻璃质感档位 `'off' \| 'standard' \| 'ultra'`（对齐 Apple Liquid Glass 三档） |
| `LogType` | 6 | `'info' \| 'success' \| 'error' \| 'warning'` |
| `LogEntry` | 8–15 | 单条日志：`id`、`time`、`message`、`type: LogType`，可选 `count?`（连续重复折叠累计次数） |
| `ToastMessage` | 17–29 | 单条 Toast：`id`、`title`、`description?`、`type: LogType`、`duration?`、`mascot?`（看板娘变体）、`action?: { label; onClick }` |
| `AdapterDisabledWarningData` | 31–34 | `name`、`message` |
| `AutoExitCountdownData` | 36–39 | `delay`、`shortcut` |
| `SaveConfigResult` | 41–45 | `{ success; message?; data? }` |

### 共享常量 —— `ui-constants.ts`

位于 `tauri-app/frontend/src/shared/ui-constants.ts`（安卓端同构）：

- `MAX_LOG_ENTRIES = 300`（第 1 行）：日志保留/展示上限基数。
- `APP_VERSION = '2.4.3'`（第 2 行）、`APP_NAME = '校园网登录助手'`（第 3 行）、`PASSWORD_MASK = '***'`（第 4 行）。
- `NAV_ITEMS`（第 7–17 行）：9 个导航项数组，`as const` 固定结构，每项 `{ id, labelKey, icon, shortcut }`，icon 为 lucide 图标名（LayoutDashboard/UserCircle/Globe/Network/Activity/Signal/Gauge/Settings/ScrollText），shortcut 为数字键 1–9。

### `LogPanel` —— `LogPanel.tsx`

文件 `tauri-app/frontend/src/shared/LogPanel.tsx`（安卓端为 `android/frontend/src/shared/LogPanel.tsx`，结构基本一致；桌面端 `LogPanelProps.api` 直接带 `setDebugMode`/`setLogRetentionDays?`/`exportDiagnostics?`，见第 28–41 行；安卓端 `api` 改为 `getDebugMode`/`getLogRetentionDays?` 的 getter 形式，见第 27–37 行）。

- `LogPanelProps`（第 28–41 行 / 安卓 27–37 行）：`api`（`getLogs(lines?)`、`clearLogs()`、`setDebugMode()`、可选 `setLogRetentionDays?`/`exportDiagnostics?`）、`initialDebugMode?`、`initialLogRetentionDays?`、`addToast(message, type, description?)`。
- `parseLogLine(line)`（第 64–74 行）：用 `LOG_LINE_REGEX = /^\[(.+?)\]\s*\[(DEBUG|INFO|WARN|ERROR)\]\s*\[(.+?)\]\s*(.+)$/`（第 62 行）解析单行日志为 `ParsedLogLine`（第 45–51 行：`timestamp/level/module/message/raw`）。
- `LEVEL_CONFIG`（第 55–60 行）：DEBUG/INFO/WARN/ERROR 四档的图标、配色、边框、左侧条、i18n key；`DEFAULT_LEVEL_CONFIG`（第 53 行）兜底。
- `LINE_OPTIONS`（第 76–81 行）：100/200/500/1000 行下拉；`MAX_DISPLAY_LINES = 200`（第 83 行）：展示截断上限。
- 组件主体 `export const LogPanel = memo(function LogPanel(...)`（第 85 行起），关键方法与状态：
  - `fetchLogs(manual=false)`（第 119–138 行）：拉日志；`manual=true` 才切 loading 态，避免 5s 轮询让刷新图标空转；用 `fetchSeqRef`/`mountedRef`/`rawLogsRef` 去重与防竞态（FE-B-10 优化，第 105–106、124–130 行）。
  - 轮询 `useEffect`（第 155–168 行）：`setInterval` 每 5000ms，叠加 `isVisibleRef`（`IntersectionObserver`，第 144–153 行）与 `document.hidden` 门控（FE-A-09），窗口隐藏/不可见时跳轮询。
  - `toggleDebugMode`（第 170–182 行）、`handleRetentionChange`（第 184–195 行，乐观更新+失败回滚）。
  - `parsedLines`/`availableModules`/`filteredLines`/`displayedLines`/`levelCounts` 五层 `useMemo`（第 212–396 行）：单遍解析 + 级别/模块/搜索筛选 + 尾截断。
  - `handleClear`（第 263–266 行）→ `handleClearConfirm`（第 285–388 行）：用 GSAP 对视口内 `.log-line` 做逐条删除动画（`stagger` 动态、failsafe `setTimeout` 兜底，见第 312–369 行），`onInterrupt` 复位（第 351–357 行），失败 `ctx.revert()` 恢复可见（第 332 行）。
  - `handleExportDiagnostics`（第 270–283 行）：调用 `api.exportDiagnostics()` 导出诊断包（桌面专属，安卓不传该字段，见 Props 第 34–35 行）。
  - 渲染（第 398–681 行）：顶部工具栏（行数/保留天数/导出诊断/刷新/调试/清空按钮）+ 搜索框 + 模块下拉 + 级别筛选条 + 日志滚动区；空态显示 `MascotFigure variant="empty"`（第 564 行）；底部挂载 `ConfirmDialog`（第 672–678 行）。

### `ConfirmDialog` —— `ConfirmDialog.tsx`

文件 `tauri-app/frontend/src/shared/ConfirmDialog.tsx`：

- `ConfirmDialogProps`（第 12–18 行）：`open`、`title`、`message`、`onConfirm`、`onCancel`。
- `export function ConfirmDialog`（第 20 行起）：基于 `@/components/ui/dialog` 的 `Dialog`。`confirming` 状态（第 23 行）防止快速双击重复触发破坏性操作；`useEffect`（第 26–28 行）在 `open=false` 时复位，避免慢网络下固定定时器提前解禁。取消时若 `confirming` 为真则拦截 `onOpenChange`（第 30–33 行）。

### `AnimatedNumber` —— `AnimatedNumber.tsx`

文件 `tauri-app/frontend/src/shared/AnimatedNumber.tsx`：

- `AnimatedNumberProps`（第 6–12 行）：`value`、`unit?`（默认 `'ms'`）、`decimals?`（默认 0）、`className?`、`duration?`。
- `export function AnimatedNumber`（第 14 行起）：用 GSAP `quickTo` 做数字滚动与 `scale` 弹跳。多个 `useRef`（第 24–30 行）：`prevRef`/`isFirstRender`/`objRef`/`valueQuickToRef`/`scaleQuickToRef`/`resetTimerRef`。`resolvedDuration = duration ?? profile.numberDuration`（第 23 行，来自 `useAnimationProfile`）。
  - 首条 `useEffect`（第 32–59 行）：注册 `quickTo` 与 `scale` tween；cleanup 中 `gsap.killTweensOf` 卸载底层 tween 防空转（第 54–55 行）。
  - 次条 `useEffect`（第 61–96 行）：值变化时 `valueQuickToRef(value)` 滚动；`profile.tier !== 'economy'` 才做 `scale` 弹跳（第 83 行），并用 `resetTimerRef` 先 kill 旧 `delayedCall` 再注册（第 87–91 行）防跳变；`useAnimationActive()`（`animActive`）关闭动画时直接写文本（第 72–78 行）。

### `RefreshButton` —— `RefreshButton.tsx`

文件 `tauri-app/frontend/src/shared/RefreshButton.tsx`：

- `RefreshButtonProps`（第 7–11 行）：继承 `React.ButtonHTMLAttributes<HTMLButtonElement>`，额外 `isRefreshing: boolean`、`iconClassName?`、`showCheck?`。
- `RefreshButton = React.forwardRef<HTMLButtonElement, ...>`（第 13–71 行，`displayName='RefreshButton'` 第 72 行）：刷新中 `animate-refresh-spin` + `opacity-50`（第 37、48 行）；由 `isRefreshing` 真→假时触发 `refresh-shake`（第 23–26 行，`prevRefreshing` ref 记录上次值）；`showCheck && !isRefreshing` 时 `AnimatePresence` 弹出 `Check` 图标（第 52–65 行）。
- `getRefreshIconClass(isRefreshing, iconClassName?)`（第 74–81 行）：导出工具函数，返回 `group-hover:rotate-180`（非刷新）或 `animate-refresh-spin`（刷新中）的 className。

### `SegmentTabs` —— `SegmentTabs.tsx`

文件 `tauri-app/frontend/src/shared/SegmentTabs.tsx`：

- `TabItem`（第 5–11 行）：`key`/`label`/`icon`/`color`/`bg`。
- `SegmentTabsProps`（第 13–17 行）：`tabs`、`activeKey`、`onTabChange`。
- `export function SegmentTabs`（第 19 行起）：用 `useId()` 生成 `layoutIdPrefix`（第 22 行），避免多处 `SegmentTabs` 共享 `layoutId` 导致跨实例动画错乱；激活项渲染 `m.div` 带 `layoutId={`${layoutIdPrefix}-activeTab`}`（第 41–46 行）做 spring 滑块。

### `ToastContainer` —— `ToastContainer.tsx`

文件 `tauri-app/frontend/src/shared/ToastContainer.tsx`：

- `ToastContainerProps`（第 10–13 行）：`toasts: ToastMessage[]`、`onRemove(id)`。
- 常量 `TOAST_MASCOTS`（第 16–21 行，各类型默认看板娘变体：info=portrait/success=celebrate/error=offline/warning=alert）、`TOAST_STYLES`（第 23–28 行，按类型定底色）。
- `export const ToastContainer = memo(function ToastContainer(...)`（第 30 行起）：固定定位 `top-[84px] left-4 z-[100]`、`aria-live="polite"`（第 39 行）；`AnimatePresence mode="popLayout"` 渲染每条 `toast`，左侧 `m.img src=/girl/mascot-${mascot}.webp`（第 57–65 行）；`celebrate` 娘图 `popIn` 首帧 spring 弹入（第 44 行）；economy 档用短 `duration` 过渡（第 34–36 行）；支持 `toast.action`（第 71–80 行）与关闭按钮（第 82–90 行）。

### `FluidBackground` —— `FluidBackground.tsx`

文件 `tauri-app/frontend/src/shared/FluidBackground.tsx`：

- `ORB_CSS`（第 8–39 行）：内嵌 CSS（仅 `transform`/`opacity` 合成器动画），两个光斑 `fluid-orb-a`（75s）/ `fluid-orb-b`（95s）慢速漂移 keyframes。
- `export function FluidBackground`（第 41 行起）：`profile.willChangeOrbs` 为真才渲染 `<style>` 与两光斑（第 50–55 行）；economy 档或 `prefers-reduced-motion` 归入 economy 时自动关闭（注释见第 6–7 行）。

### `ErrorBoundary` —— `ErrorBoundary.tsx`

文件 `tauri-app/frontend/src/shared/ErrorBoundary.tsx`：

- `Props`/`State`（第 6–13 行）：`children`；`hasError: boolean`、`error: Error | null`。
- `export class ErrorBoundary extends Component<Props, State>`（第 15 行起）：`getDerivedStateFromError`（第 21–23 行）切错误态；`componentDidCatch`（第 25–27 行）仅在 `import.meta.env.DEV` 打印；错误 UI（第 30–57 行）展示 i18n 文案、`<details>` 折叠原始错误（第 39–46 行）、「重新加载」按钮调用 `window.location.reload()`（第 49–54 行）；正常时直接 `return this.props.children`（第 60 行）。

### `MascotFigure` —— `MascotFigure.tsx`

文件 `tauri-app/frontend/src/shared/MascotFigure.tsx`：

- `SIZES = { sm:'w-20', md:'w-28', lg:'w-40' }`（第 3 行）。
- `MascotVariant`（第 5 行，type）：`'portrait' | 'welcome' | 'empty' | 'celebrate' | 'sponsor' | 'offline' | 'alert'`，对应 `public/girl/mascot-*.webp`。
- `export function MascotFigure({ variant, size='md', className='' })`（第 7–26 行）：渲染 `<img src=/girl/mascot-${variant}.webp>`，`loading="lazy"`、`aria-hidden`、`draggable=false`。

### `IspMark` —— `IspMark.tsx`

文件 `tauri-app/frontend/src/shared/IspMark.tsx`（安卓端同构）：

- `ISP_MARK_SRC`（第 5–10 行）：`__default__`→`/isp/school.webp`（校徽，对应 suffix `''`/无锡学院）、`@telecom`/`@unicom`/`@cmcc` 三网logo。
- `ispMarkSrc(suffix)`（第 12–15 行）：导出工具，`null/undefined` 返回 `undefined`，未知 suffix 回退 `__default__`。
- `export const IspMark = memo(function IspMark({ suffix, className }))`（第 18 行起）：校徽套 16px 白圆底（第 27–37 行，浅色底可读性），运营商 logo 原色直出（第 39–41 行）。

### `SponsorCard` —— `SponsorCard.tsx`

文件 `tauri-app/frontend/src/shared/SponsorCard.tsx`（桌面专属）：

- `SponsorCardProps`（第 7–10 行）：`open`、`onClose`。
- `SponsorAnchor`（第 14–18 行）：`top`/`right`/`arrowRight` 几何（复刻原 `top-52` 垂直间距）。
- `export function SponsorCard({ open, onClose })`（第 24 行起）：**非模态**浮层（无遮罩、不抢焦点）；打开/resize 时按 `button[data-sponsor-anchor="true"]` 实测 `getBoundingClientRect()` 定位（第 33–47 行，不依赖 aria-label 文案）；点击浮层外任意处（捕获阶段 `pointerdown`，第 51–58 行）或 Esc（第 54–59 行）关闭；箭头按按钮中心水平对齐（第 39–40、78–81 行）；玻璃档位走 `var(--glass-blur)/var(--glass-sat)`（第 87–88 行）；展示微信/支付宝赞助二维码（`/sponsor-weixin.png`、`/sponsor-alipay.jpg`，第 113–126 行）。

### `UpdateAvailableDialog`（安卓专属）—— `UpdateAvailableDialog.tsx`

文件 `android/frontend/src/shared/UpdateAvailableDialog.tsx`，安卓端 `index.ts` 未导出（见 `android/frontend/src/shared/index.ts` 第 1–13 行），由安卓界面自行 import：

- `UpdateAvailableDialogProps`（第 14–16 行）：`onGoUpdate`。
- `export function UpdateAvailableDialog({ onGoUpdate })`（第 18 行起）：从 `useQualityStore` 读取 `updatePromptOpen`/`latestVersion`/`releaseNotes`/`setUpdatePromptOpen`（第 20–23 行）；「立即前往更新」(`onGoUpdate`) 跳转关于界面（第 34 行）。

### `dailyMascot`（安卓专属）—— `dailyMascot.ts`

文件 `android/frontend/src/shared/dailyMascot.ts`，安卓端仓库内未被 `index.ts` 导出（第 1–13 行）：

- `SIDE_MASCOT_POOL`（第 5–9 行）：14 张竖版立绘名数组（`side-wave`…`side-think`）。
- `pickDailyMascot(offset=0)`（第 11–15 行）：按 `Math.floor(Date.now()/86_400_000)` UTC 天数 + offset 取模选图。
- `useDailyMascots(offsets: number[])`（第 18–35 行）：hook，返回各 offset 对应娘图数组，每分钟比对日期跨天即时切换（第 25–30 行）。


## Architecture

本模块是纯展示层，无状态管理/网络调用（除经 Props 注入的 `api.*` 与通过 store hook 注入的安卓更新状态）。对外契约 = `index.ts` 导出集合。组件间依赖关系：

```text
                 @/shared  (index.ts 统一导出)
                          │
   ┌──────────────────────┼───────────────────────────────┐
   │                      │                                │
 类型/常量             动画/装饰组件                    交互组件
 ┌──────────┐      ┌─────────────┐ ┌──────────────┐  ┌──────────────┐
 │types.ts  │      │AnimatedNumber│ │FluidBackground│  │RefreshButton │
 │ui-types.ts     │(GSAP quickTo)│ │(CSS 光斑)    │  │SegmentTabs   │
 │ui-constants.ts │ └─────┬───────┘ └──────┬───────┘  │ToastContainer│
 └──────────┘            │                 │          └──────┬───────┘
                         │                 │                 │
                ┌────────▼─────────────────▼─────────────────▼───────┐
                │           LogPanel (核心聚合组件)                  │
                │  uses: ConfirmDialog / MascotFigure / parseLogLine│
                │  props.api ──► 后端 getLogs/clearLogs/setDebug... │
                └───────────────────────┬───────────────────────────┘
                                         │ uses
                ┌────────────────────────▼────────────────────────┐
                │ MascotFigure(图)  IspMark(图)  SponsorCard(浮层) │
                │ ErrorBoundary(类)  UpdateAvailableDialog(安卓)  │
                └─────────────────────────────────────────────────┘

 外部消费方: App.tsx / TitleBar / 各 Panel / useQualityStore(安卓更新)
```

- `LogPanel` 是聚合度最高的组件，内部复用 `ConfirmDialog` 与 `MascotFigure`（见 `LogPanel.tsx` 第 19–20、564、672–678 行）。
- 动画类组件（`AnimatedNumber`/`FluidBackground`/`ToastContainer`）全部读取 `useAnimationProfile().tier/willChangeOrbs`，economy 档降级或关闭（见 `AnimatedNumber.tsx` 第 83 行、`FluidBackground.tsx` 第 50 行、`ToastContainer.tsx` 第 33 行）。
- 桌面与安卓共用同一份 `shared/` 源码结构；差异仅在于 Props 接口（`LogPanel` 的 `api` 形状，见上文）、以及安卓多出 `UpdateAvailableDialog`/`dailyMascot.ts`（均未进 `index.ts`）。


## Data Flow

### 日志面板数据流（最复杂）

1. **挂载即拉取**：`LogPanel` 挂载后 `useEffect` → `fetchLogs(true)`（`LogPanel.tsx` 第 140–142 行）取 `api.getLogs(lineCount)`。
2. **轮询刷新**：`useEffect` 注册 5000ms `setInterval`（第 155–168 行），叠加 `IntersectionObserver`（`isVisibleRef`，第 144–153 行）与 `document.hidden` 门控；窗口隐藏/不可见时跳过本轮 `invoke`。
3. **去重防抖（FE-B-10）**：`fetchLogs` 用 `fetchSeqRef` 递增序号 + `mountedRef` 标记卸载，丢弃过期/卸载后结果（第 119–138 行）；内容未变时仅更新 `rawLogsRef` 而不 `setRawLogs`，避免五层 `useMemo` 全量重算（第 127–130 行）。
4. **解析与筛选管线**：`rawLogs` → `parsedLines`（正则单遍解析，第 212–223 行）→ `availableModules`（第 225–228 行）→ `filteredLines`（级别/模块/搜索，第 230–254 行）→ `displayedLines`（尾 200 行截断，第 256–261 行）→ `levelCounts`（第 390–396 行）。
5. **清空流程**：`handleClear`（第 263 行）开 `ConfirmDialog` → `handleClearConfirm`（第 285 行）用 GSAP 对视口内 `.log-line` 逐条动画隐藏（第 337–359 行），`onComplete`/`failsafe` 兜底后调用 `api.clearLogs()`（第 322 行）并清空 `rawLogsRef`/`setRawLogs('')`（第 324–325 行）；`onInterrupt`/失败 `ctx.revert()` 恢复可见（第 332、355 行）。
6. **导出诊断（桌面专属）**：`handleExportDiagnostics`（第 270 行）调 `api.exportDiagnostics()`（Props 第 34–35 行声明安卓不传），成功 toast 携带目录路径。

### Toast 数据流

外部 state 维护 `toasts: ToastMessage[]` → 渲染 `ToastContainer`（`ToastContainer.tsx` 第 41 行）→ 每条按 `type` 取 `TOAST_MASCOTS` 默认娘图或 `toast.mascot` 覆盖（第 42 行）→ `onRemove(id)` 由外部移除。

### 刷新按钮状态

父组件把 `isRefreshing` 传入，`RefreshButton` 用 `prevRefreshing` ref 检测真→假跳变触发 `refresh-shake`（第 23–26 行）；`showCheck` 为真且非刷新时弹 `Check`（第 52–65 行）。

### 安卓更新弹窗数据流

`useQualityStore.updatePromptOpen` 被后端更新检查循环经 `useEventListeners` 置真（见 `UpdateAvailableDialog.tsx` 第 1–2 行注释）→ `UpdateAvailableDialog` 读取 `latestVersion`/`releaseNotes`（第 21–22 行）弹出 → `onGoUpdate()` 跳转关于界面下载安装。


## Connections

- `index.ts` 对外导出入口：`tauri-app/frontend/src/shared/index.ts`、`android/frontend/src/shared/index.ts`
- 基础 UI 原子组件（被本模块复用）：`Button`/`Badge`/`Select`/`Dialog`/`Card` 来自 `@/components/ui/*`（见 `LogPanel.tsx` 第 1–5 行、`ConfirmDialog.tsx` 第 2–9 行、`ToastContainer.tsx` 第 6 行、`SponsorCard`/`UpdateAvailableDialog`）
- 动画基础设施：`@/lib/animations` 的 `createLogEntryVariants`（`LogPanel.tsx` 第 25 行）、`@/lib/utils` 的 `cn`/`extractErrorMessage`（`LogPanel.tsx` 第 18 行）
- 动画档位驱动：`useAnimationProfile`（`@/hooks/useAnimationProfile`，被 `AnimatedNumber`/`FluidBackground`/`ToastContainer`/`LogPanel` 使用）、`useAnimationActive`（`@/hooks/usePageIdle`，`AnimatedNumber.tsx` 第 4 行）、`useQualityStore`（安卓更新态，`UpdateAvailableDialog.tsx` 第 12 行）
- 看板娘素材：`public/girl/mascot-*.webp`（`MascotFigure.tsx`、`ToastContainer.tsx` 第 58 行）、`public/isp/*.webp`（`IspMark.tsx`）、`public/sponsor-weixin.png`/`sponsor-alipay.jpg`（`SponsorCard.tsx`）
- 上层消费方：根 `App.tsx`、标题栏（赞助按钮标注 `data-sponsor-anchor`，`SponsorCard.tsx` 第 34 行）、各业务面板（Dashboard/Account/Network/Monitor 等）、日志面板（Log 路由）

相关模块（Obsidian backlinks）：
- [[components-ui]]（本模块依赖的底层原子组件库）
- [[lib-utils]]（cn / 错误处理 / 动画变体）
- [[hooks-animation-profile]]（动画档位 tier / willChangeOrbs）
- [[stores-quality]]（安卓 useQualityStore 更新提示状态）
- [[app-root]]（错误边界、Toast 全局挂载点）


## Known Issues

本模块源码中多处注释记录了「历史缺陷→修复」的来龙去脉，集中在此供后续维护者参考：

- **`LogPanel` 5s 轮询 setState 风暴**：早期每 5 秒 `setRawLogs` 触发四层 `useMemo`（parsedLines/filteredLines/displayedLines/levelCounts）+ 整表重渲染。修复：用 `rawLogsRef` 去重（内容未变不 setState）并加 `fetchSeqRef`/`mountedRef` 防竞态，见 `LogPanel.tsx` 第 105–106、119–138 行（标注 FE-B-10）。
- **`LogPanel` 清空动画中断残留 `isClearing`**：GSAP tween 被 `AnimatePresence` 切换面板/组件卸载中断时 `onComplete` 不触发，清空按钮永久禁用；且 WebView 被遮挡时 rAF 停摆导致 tween 冻结。修复：补 `onInterrupt` 复位（第 351–357 行）+ `setTimeout` failsafe 兜底（第 364–369 行），先结算再 `revert` 避免竞态（第 365–368 行）。
- **`LogPanel` 清空失败时日志"重现"**：早期 `revert` 先恢复可见再卸载产生闪现。修复：不 `revert`，由 React 在 `rawLogs` 清空后卸载（第 320–321 行）。
- **`LogPanel` 轮询 loading 语义混淆**：5s 轮询若也转加载图标，会和手动刷新语义混淆。修复：`fetchLogs(manual=false)` 不置 loading（第 117–138 行）。
- **`ConfirmDialog` 双击重复提交**：早期确认按钮不禁用，快速双击会执行两次破坏性操作。修复：加 `confirming` 状态（第 23 行）；关窗由 `open` 驱动复位而非固定 400ms 定时器（第 26–28 行），避免慢网络下提前解禁。
- **`AnimatedNumber` 卸载后 tween 空转**：`quickTo` 底层 tween 卸载后存活，持续占用 CPU 写已卸载 DOM。修复：cleanup 中 `gsap.killTweensOf`（第 54–55 行）；`resetTimerRef` 注册前先 kill 旧 `delayedCall` 防跳变（第 87–91 行）。
- **`SegmentTabs` 跨实例布局动画错乱**：多个 `SegmentTabs` 共存时若共用 `layoutId="activeTab"` 会错误共享动画。修复：用 `useId()` 生成实例前缀（第 22、42 行）。
- **`FluidBackground` 性能/无障碍**：economy 档或 `prefers-reduced-motion` 下仍渲染光斑。修复：`profile.willChangeOrbs` 门控（第 50 行），后者在 `resolveTier` 归入 economy。
- **安卓 vs 桌面 Props 不一致**：`LogPanel` 在两端的 `api` 接口签名不同（桌面直接 `setDebugMode`/`setLogRetentionDays?`/`exportDiagnostics?`，安卓用 `getDebugMode`/`getLogRetentionDays?` getter，且无 `exportDiagnostics`）。消费方按平台传对应 `api` 形状，详见 `LogPanel.tsx` 第 28–41 行与 `android/.../LogPanel.tsx` 第 27–37 行。
- **无单元测试覆盖**：本模块（`shared/`）目录下无 `*.test.ts(x)`，行为仅靠组件内注释与人工验证保障；如需回归测试建议优先覆盖 `parseLogLine`/`ispMarkSrc`/`pickDailyMascot` 等纯函数。

