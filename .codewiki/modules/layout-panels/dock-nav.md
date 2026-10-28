---
title: DockNav
type: module
source_files:
  - tauri-app/frontend/src/components/layout/DockNav.tsx
  - tauri-app/frontend/src/lib/dock-lens.ts
  - tauri-app/frontend/src/lib/glass-dock.tsx
  - tauri-app/frontend/src/hooks/useAdapterStore.ts
  - tauri-app/frontend/src/hooks/useAuthStore.ts
  - tauri-app/frontend/src/hooks/useConfigStore.ts
  - tauri-app/frontend/src/hooks/useThemeStore.ts
  - tauri-app/frontend/src/hooks/useReducedMotion.ts
  - tauri-app/frontend/src/hooks/useAnimationProfile.ts
  - tauri-app/frontend/src/hooks/usePulseAnimation.ts
  - tauri-app/frontend/src/network/adapters.ts
  - tauri-app/frontend/src/shared/ui-constants.ts
  - tauri-app/frontend/src/shared/ui-types.ts
  - tauri-app/frontend/src/lib/dock-lens.actionButton.test.tsx
  - tauri-app/frontend/src/lib/dock-lens.travel.test.tsx
tags: [layout, dock, navigation, glass, lens, panel-switch]
---

## Overview

`DockNav`（`tauri-app/frontend/src/components/layout/DockNav.tsx`）是布局模块里的**底部居中悬浮导航栏**：一个玻璃质感的 dock，固定在视口底部（`fixed bottom-5 z-30`，`left:0`，宽 `calc(100vw - var(--right-panel-width, 288px))`，见 `DockNav.tsx:382-386`），负责整条应用的主面板切换、登录/注销入口，以及悬浮栏的两套视觉系统——**玻璃折射**（`useGlassRefraction`）与**弹簧镜头**（`useDockLens`）。

它把下列职责收敛到自己体内：

- **面板切换**：渲染 `NAV_ITEMS`（`shared/ui-constants.ts:7-17`）中的每一个项，点击/拖动调用父级传进来的 `onPanelChange(PanelName)`（`DockNav.tsx:327`、`410-419`）；`quality` 面板按 `enableNetworkQuality` 决定显隐（`DockNav.tsx:354`）。
- **玻璃折射**：把 `navRef` 交给 `useGlassRefraction`，档位由 `useThemeStore.glassLevel` 决定（`DockNav.tsx:359-361`）。
- **弹簧镜头**：把 `navRef` 交给 `useDockLens`，提供选中项下标、项数量、`onLensSelect` 回调（`DockNav.tsx:366-379`）。
- **登录/注销按钮**：两个 `ActionButtonWithMenu`，分别带 `LogOut`/`LogIn` 图标，接 `doLogout`/`doLogin`，并在多张在线适配器卡时弹出 `AdapterMenu` 选择目标卡（`DockNav.tsx:453-473`）。

镜头系统是一个零 React re-render 的每帧写 DOM 的方案：它靠 `data-dock-*` 属性定位节点（`[data-dock-item]`、`[data-dock-lens]`、`[data-dock-strip]`、`[data-dock-magnify]`、`[data-dock-magnify-inner]`），所有运动走 `lib/dock-spring.ts` 的弹簧仿真（`dock-lens.ts:465-478`、`:262-271`），仅在尺寸变化时通过 `ResizeObserver` 重测几何（`dock-lens.ts:465-470`）。

## Key Components

### `ICON_MAP`（常量）— `DockNav.tsx:37-47`

把 `PanelName` 的 `icon` 字符串名映射到从 `lucide-react` 导入的具体图标组件：`LayoutDashboard`、`UserCircle`、`Globe`、`Network`、`Activity`、`Signal`、`Gauge`、`ScrollText`、`Settings`。`DockItem` 与放大层副本都通过 `ICON_MAP[icon]` 取图标（`DockNav.tsx:49`、`438`），所以 `NAV_ITEMS` 里新增项时必须在这里补名字，否则图标渲染为 `undefined`。

### `DockItem` 组件 — `DockNav.tsx:49-79`

单个导航项按钮。

props（内联类型，`DockNav.tsx:51-57`）：

| prop | 类型 | 用途 |
| --- | --- | --- |
| `id` | `PanelName` | 该项对应的面板名，点击时回传给 `onPanelChange`（`DockNav.tsx:63`） |
| `label` | `string` | 国际化后的标签文本，由父级 `t(labelKey)` 传入（`DockNav.tsx:414`） |
| `icon` | `string` | `ICON_MAP` 的键（`DockNav.tsx:58`） |
| `isActive` | `boolean` | 是否当前面板：`true` 时文字用 `text-primary`、`aria-current="page"`（`DockNav.tsx:70`、`73`） |
| `onPanelChange` | `(id: PanelName) => void` | 点击回调（`DockNav.tsx:63`） |

注意：宽度固定 `w-[52px]`，注释说明是按中文最长 4 字（44px）与英文 “Settings” 8ch（约 44px）取的（`DockNav.tsx:65-66`）。它只负责**渲染项本身、点击与无障碍**；是否"被镜头放大/选中"由 `useDockLens` 在 DOM 层处理（`dock-lens.ts:93-102`）。

### `AdapterMenu` 组件 — `DockNav.tsx:81-175`

登录/注销浮层菜单（framer-motion `m.div`，`DockNav.tsx:96-110` 进场动画用 `profile.easing.enter`）。

`AdapterMenuProps` 接口（`DockNav.tsx:81-86`）：

| prop | 类型 | 用途 |
| --- | --- | --- |
| `adapters` | `Adapter[]` | 候选适配器全集 |
| `selectedAdapter` | `string?` | 当前已选（来自 `ActionButtonWithMenu` 的本地 state），为 `undefined` 时回退到第一张在线卡（`DockNav.tsx:90-93`） |
| `onSelect` | `(adapterName: string) => void` | 选中某卡后回调，触发 `handleSelect`（`DockNav.tsx:225-230`） |
| `actionLabel` | `string` | 标题前缀，通常是 `auth.login` / `auth.logout` 的译文（`DockNav.tsx:454`） |

行为：

- 只用 `a.ip && a.ip.length > 0` 的"在线卡"（`DockNav.tsx:91`），`defaultAdapter` 取第一张在线卡名（`DockNav.tsx:91`）。
- 每张卡渲染 `adapter-menu-item` 按钮，左侧按 `adapter.wireless` 区分 `WifiIcon`/`Cable`，选中态用 `bg-primary/10` 并打勾（`DockNav.tsx:128-169`）。
- 菜单底部带一个 45° 小三角指向触发按钮（`transformOrigin: 'bottom right'`，`DockNav.tsx:101`、`117-124`）。

### `ActionButtonWithMenu` 组件 — `DockNav.tsx:177-324`

带下拉菜单的登录/注销按钮。props（内联类型，`DockNav.tsx:186-195`）：

| prop | 类型 | 用途 |
| --- | --- | --- |
| `label` | `string` | 正常态文案 |
| `loadingLabel` | `string` | 加载态文案（`aria-label`） |
| `icon` | `typeof LogIn` | 普通态图标（传入 `LogOut` 或 `LogIn`） |
| `isLoading` | `boolean` | 是否加载中（控制 spinner / 禁用） |
| `isDisabled` | `boolean` | 是否被对方操作禁用（如登录时禁用注销） |
| `adapters` | `Adapter[]` | 经过 `resolveAdapterNames` 过滤后的 `scopedAdapters` |
| `onAction` | `(adapterName?: string) => void` | 实际触发 `doLogout` / `doLogin` |
| `variant` | `'primary' \| 'outline'` | `primary` 为渐变实心、`outline` 为描边（`DockNav.tsx:261-290`） |

关键内部逻辑：

- `activeAdapters = adapters.filter(a => a.ip && a.ip.length > 0)`（`DockNav.tsx:205`）。
- `showMenu = activeAdapters.length >= 2`：**只有两张及以上在线卡才弹菜单**，单张直接以该卡执行（`DockNav.tsx:207`）。
- 悬停 150ms 开、离开 300ms 关（`scheduleOpen`/`scheduleClose`，`DockNav.tsx:209-218`），`cancelTimers` 用于点击/选中时清掉待定计时（`DockNav.tsx:220-223`）。
- `handleClick`（`DockNav.tsx:232-242`）：禁用/加载中直接返回；单张在线卡时显式 `onAction(activeAdapters[0].name)`——注释说明是避免走后端 resolve 时命中无 IP 的配置主卡（`DockNav.tsx:236-240`）；否则 `onAction()`。
- `handleSelect`（`DockNav.tsx:225-230`）：关菜单、清计时、记 `selectedAdapter`、调 `onAction(adapterName)`。
- 加载中：`gsap` 旋转 spinner（`spinnerRef`，`DockNav.tsx:251-259`），同时 `loadingPulseRef` 接 `usePulseAnimation({ type: 'loadingPulse' })` 做边框脉冲（`DockNav.tsx:203`、`293-299`）。
- `whileTap` 仅做 `scale: 0.95`，注释明确**不再上浮放大**，避免与镜头 hover 语言打架、并防止 `scale` 把 nav 撑高导致镜片读到的 `barH` 抖动（`DockNav.tsx:274-277`）。

### `DockNavProps` 接口 — `DockNav.tsx:326-329`

| prop | 类型 | 用途 |
| --- | --- | --- |
| `onPanelChange` | `(panel: PanelName) => void` | 面板切换回调，透传给 `DockItem` 与 `onLensSelect` |
| `outerRef` | `((el: HTMLDivElement \| null) => void)?` | 把最外层 `div` 暴露给父级（如做入场/测量，`DockNav.tsx:383`） |

### `DockNav` 组件（导出，`memo`）— `DockNav.tsx:331-477`

顶层导出组件，被 `React.memo` 包裹（`DockNav.tsx:331`）。它订阅的 store 与本地计算：

- `useAdapterStore(s => s.activePanel)` — 当前面板（`DockNav.tsx:333`）。
- `useAuthStore(s => s.isLoggingIn)` / `s.isLoggingOut` — 两个按钮的加载态（`DockNav.tsx:334-335`）。
- `useAdapterStore(s => s.adapters)` — 适配器全集（`DockNav.tsx:336`）。
- `useConfigStore(s => s.config.enableNetworkQuality !== false)` — `quality` 面板显隐开关（`DockNav.tsx:354`）。
- `useConfigStore(useShallow(...))` 取 `adapter1`/`adapter2`/`dualAdapter`（`DockNav.tsx:338-342`）。
- `useAuthStore(s => s.doLogin)` / `s.doLogout` — 实际动作（`DockNav.tsx:343-344`）。

`scopedAdapters`（`DockNav.tsx:349-353`）：`useMemo` 里调 `resolveAdapterNames(adapters, { adapter1, adapter2, dualAdapter })`，拿到 `primary`/`secondary`，组成名字集合，再 `adapters.filter(a => names.has(a.name) && a.ip)`。注释强调作用域只覆盖"适配器设置"里的主/副适配器，与后端 `resolve_adapter_names` 同源（`DockNav.tsx:346-348`）。

`visibleItems`（`DockNav.tsx:354`）：`NAV_ITEMS.filter(item => enableNetworkQuality || item.id !== 'quality')`。

`glassLevel = useThemeStore(s => s.glassLevel)`（`DockNav.tsx:361`），并 `useGlassRefraction(navRef, profile.tier === 'economy' ? 'off' : glassLevel)`（`DockNav.tsx:361`）。

`lensEnabled = glassLevel !== 'off' && profile.tier !== 'economy' && !reducedMotion`（`DockNav.tsx:366`），`activeIndex` 取当前面板在 `visibleItems` 的下标（`DockNav.tsx:366`），`onLensSelect` 把下标映射回 `PanelName` 调 `onPanelChange`（`DockNav.tsx:367-373`），最后 `useDockLens(navRef, { count, selected, onSelect, enabled })`（`DockNav.tsx:374-379`）。

JSX 渲染的镜头相关节点（外部只读、不引用类型）：

- `[data-dock-lens]` 药丸底（`DockNav.tsx:397-405`）。
- `[data-dock-strip]` 项容器，镜头在其上挖洞（`DockNav.tsx:409`）。
- `[data-dock-magnify]` + `[data-dock-magnify-inner]` 放大层副本（`DockNav.tsx:424-449`）。
- 分隔条（`DockNav.tsx:177`）与两个 `ActionButtonWithMenu`（`DockNav.tsx:453-473`）。
- `nav` 本体 `style` 上的 `transform: translateX(var(--dock-stretch, 0px))` 与 `backgroundImage` 用 `--dock-glow-*` 三变量画按压光斑（`DockNav.tsx:390-394`），这些变量由 `useDockLens` 每帧写。

### `useGlassRefraction(ref, level)` — `glass-dock.tsx:176-287`

把 Hyalite（ vendored SVG 位移图折射）挂到 `ref` 元素上。签名 `ref: RefObject<HTMLElement | null>`、`level: 'off' | 'standard' | 'ultra'`。`level === 'off'` 或 `hyalite.supported() === false` 时只 `remove()` 并加 `glass-ready`、不做任何滤镜（`glass-dock.tsx:211-217`）。档位参数表见 `GLASS_DOCK_OPTS`（`glass-dock.tsx:67-131`）。三档差异只在 `blur`（standard=8 / ultra=1.6），其余 `bevel=12`、`thickness=70`、`slope=2.0`、`rim=0`、`edge=0` 完全一致（`glass-dock.tsx:22-66`）。

### `useDockLens(navRef, opts)` — `dock-lens.ts:93-503`

弹簧镜头系统 hook。`DockLensOptions`（`dock-lens.ts:72-81`）：`count`（项数）、`selected`（选中下标）、`onSelect(index)`（拖动松手/点按触发）、`enabled`（为 `false` 时完全不接管：不挂监听、不写样式，`dock-lens.ts:79`、`99-101`、`498-502`）。

内部按 `data-dock-*` 定位节点（`dock-lens.ts:103-107`），测量只在布局变化时跑（`measure`，`dock-lens.ts:119-133`），六条弹簧（`lens`/`lift`/`stretch`/`jelly`/`glow`/`glowSpread`，`dock-lens.ts:292-304`）共享一个 rAF（`requestRender`，`dock-lens.ts:263-271`）。手势包括：按下抬起（`onDown`/`lift`，`dock-lens.ts:374-398`、`285-290`）、果冻形变（`wobbler`，`dock-lens.ts:273-280`）、拖动滑选（`onMove`/`moveLens`，`dock-lens.ts:343-361`、`307-312`）、悬停高亮（注释说明**没按住时不做位移**，仅纯 CSS，`dock-lens.ts:343-347`）、越界拉伸（`stretchFor`，`dock-lens.ts:331-336`）、投掷惯性（`flingTarget`，`dock-lens.ts:400-412`）。落下判定要求"抬起到位 + 镜头到目标"才落药丸（`maybeDrop`，`dock-lens.ts:285-290`）。

## Architecture

```text
                       ┌─────────────────────────────────────────────┐
                       │  DockNav (DockNav.tsx:331 memo)              │
                       │  outerRef → fixed bottom-5 z-30 外层 div     │
                       └───────────────┬─────────────────────────────┘
                                       │ navRef (HTMLElement)
                 ┌─────────────────────┼───────────────────────────┐
                 │                     │                            │
        ┌────────▼────────┐   ┌────────▼─────────┐        ┌────────▼─────────┐
        │ useGlassRefraction│  │   useDockLens    │        │  DockItem × N    │
        │ (glass-dock.tsx) │  │  (dock-lens.ts)  │        │ (DockNav.tsx:49) │
        │ Hyalite 位移图   │  │ 6 弹簧 / rAF 写  │        │ NAV_ITEMS 渲染   │
        │ 档位=glassLevel  │  │ data-dock-* 节点 │        │ onPanelChange    │
        └────────┬────────┘  └────────┬─────────┘        └────────┬─────────┘
                 │                    │  onLensSelect             │  onClick
                 │  --dock-glow-*     │  (idx→PanelName)          │
                 │  --dock-stretch    └──────────────┬───────────┘
                 │                                   ▼
                 │                          ┌──────────────────┐
                 │                          │ onPanelChange    │
                 │                          │ (父级 prop)       │
                 │                          └──────────────────┘

   ┌──────────────────────── 右侧控件区 ────────────────────────┐
   │ ActionButtonWithMenu(注销,outline)  ActionButtonWithMenu(登录,primary) │
   │   │ isDisabled=isLoggingIn / isLoading=isLoggingOut                  │
   │   │ showMenu = 在线卡≥2 才弹 AdapterMenu                             │
   │   └─ scopedAdapters = resolveAdapterNames → filter(有IP)            │
   └───────────────────────────────────────────────────────────────────┘

  Store 连线：
   useAdapterStore: activePanel, adapters        (DockNav.tsx:333,336)
   useAuthStore:    isLoggingIn,isLoggingOut,doLogin,doLogout (334,335,343,344)
   useConfigStore:  enableNetworkQuality, adapter1/2, dualAdapter (337,338)
   useThemeStore:   glassLevel                   (359)
   useReducedMotion / useAnimationProfile        (356,355)
```

## Data Flow

## Connections

## Known Issues
