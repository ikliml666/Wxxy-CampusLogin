---
title: android-frontend-lib-and-i18n
type: module
source_files:
  - android/frontend/src/i18n/index.ts
  - android/frontend/src/i18n/locales/en.json
  - android/frontend/src/i18n/locales/zh.json
  - android/frontend/src/lib/glass-dock.tsx
  - android/frontend/src/lib/latency.ts
  - android/frontend/src/lib/notificationPermission.ts
  - android/frontend/src/lib/renderLiveness.ts
  - android/frontend/src/lib/utils.ts
  - android/frontend/src/lib/animations.ts
  - android/frontend/src/lib/color.ts
  - android/frontend/src/lib/dock-lens.ts
  - android/frontend/src/lib/dock-spring.ts
  - android/frontend/src/lib/easing-config.ts
tags: [android, frontend, i18n, animation, physics, glass-effect, latency, utils]
---

## Overview

本模块是 Android 前端应用的基础设施层，主要包含两大核心职能：

1. **国际化 (i18n)**：基于 `i18next` 与 `react-i18next` 构建的全局语言管理单例（`android/frontend/src/i18n/index.ts`），负责多语言资源的加载、检测、持久化及 React 上下文注入；语言包位于 `locales/en.json` 与 `locales/zh.json`，按功能域分组。
2. **前端底层工具库**（`android/frontend/src/lib/`）：
   - **动画与物理引擎**：`animations.ts` 提供 Framer Motion 变体生成器；`easing-config.ts` 提供高刷适配的缓动配置；`dock-spring.ts` 提供自研弹簧物理内核；`color.ts` 提供 HEX→HSL 转换。
   - **悬浮栏交互**：`glass-dock.tsx` 封装 Hyalite 折射玻璃；`dock-lens.ts` 实现 Dock 镜头系统（镜片放大、果冻形变、光斑），直接操作 DOM 保证 60fps+。
   - **系统保活与诊断**：`latency.ts` 网络质量解析合并；`renderLiveness.ts` 渲染循环存活检测；`notificationPermission.ts` 通知权限统一入口；`utils.ts` 通用工具与存储兜底。

该模块不直接处理业务逻辑，为上层 UI 提供标准化视觉反馈、语言支持及系统级状态监控。

## Key Components

### 1. 国际化模块 (`i18n/`)

#### `index.ts` (行1-28)
**职责**：全局国际化单例初始化入口。

- **导入与初始化链**：
  - 导入 `i18n` (1|)、`initReactI18next` (2|)、`LanguageDetector` (3|)、`zh` (5|)、`en` (6|)。
  - 调用 `i18n.use(LanguageDetector).use(initReactI18next).init({...})` (8-26|)。
- **配置对象**：
  - `resources` (12-15|)：绑定 `zh` 与 `en` 命名空间翻译。
  - `fallbackLng: 'zh'` (16|)：默认回退语言。
  - `lng` (17|)：IIFE 尝试从 `localStorage.getItem('app-language')` 读取，异常时返回 `undefined` 触发探测器降级。
  - `detection` (18-22|)：`order: ['localStorage', 'navigator']`，`lookupLocalStorage: 'app-language'`，`caches: ['localStorage']`。
  - `interpolation.escapeValue: false` (23-25|)：禁用 HTML 转义，允许 React 渲染富文本。
- **导出**：`export default i18n` (28|)。

#### `locales/en.json` (行1-918) 与 `locales/zh.json` (行1-918)
**职责**：全量翻译资源文件，两文件键结构完全对称。

- **顶层命名空间及行号（以 en.json 为准，zh.json 对应行号一致）**：
  - `notify`：2-19（通知文案，含 `autoLoginSuccess` 等，支持 `{{msg}}`/`{{seconds}}` 插值）
  - `common`：20-43（通用按钮/状态，`moveCard`/`refresh`/`clear` 等）
  - `nav`：44-55（导航标签：`dashboard`/`account`/`selfservice`/`network`/`monitor`/`quality`/`speedtest`/`settings`/`log`/`more`）
  - `titlebar`：56-72（窗口控制与主题切换，`appName`/`switchLanguage` 等）
  - `auth`：73-98（认证流程：`login`/`logout`/`loginTimeout` 等）
  - `account`：99-286（账号管理、ISP 绑定、VPN、生物识别、夜间策略、自助服务查询）
  - `network`：287-364（适配器、DNS 优化、夜间出站切换、`avoidBadWifi` 命令）
  - `monitor`：365-432（网络状态检测、校验设置、检测逻辑、时间轴说明）
  - `quality`：433-498（网络质量、延迟详情、节点名称 `names` 子对象）
  - `speedtest`：499-525（测速站点列表与说明）
  - `settings`：526-631（外观、启动、保活、安全、质量检测、`isp` 子对象 624-629）
  - `log`：632-669（系统日志、`retention` 子对象 657-663）
  - `onboarding`：670-731（新手指引步骤与绑定运营商）
  - `updatePrompt`：732-737
  - `about`：738-780（关于与更新下载）
  - `statusbar`：781-793（`onlineOperator` 子对象 787-792）
  - `dashboard`：794-831（总览卡片与自助服务概览）
  - `confirmDialog`：832-835
  - `panel`：836-855（各面板标题与描述）
  - `themeDialog`：856-871（主题与玻璃质感 `glassLevel`）
  - `rightPanel`：872-876
  - `sponsor`：877-882
  - `mobile`：883-892
  - `face`：893-917（人脸录入与验证文案）
- **动态插值**：广泛使用 `{{seconds}}`、`{{version}}`、`{{name}}`、`{{msg}}`、`{{shortcut}}`、`{{operator}}` 等占位符。

### 2. 动画与物理引擎 (`lib/`)

#### `animations.ts` (行1-42)
**职责**：为 Framer Motion 提供标准化动画变体。

- **`createLogEntryVariants(easing: EasingConfig)`** (3-17|)：
  - 返回 `{ initial, animate, exit }`。
  - `initial`: `{ opacity: 0, x: 20 }` (5|)
  - `animate`: `opacity 1, x 0`，过渡 `duration: 0.3, ease: easing.snappy` (6-10|)
  - `exit`: `opacity 0, x: -16`，过渡 `duration: 0.15, ease: easing.exit` (11-15|)
- **`createPanelAppleVariants(easing: EasingConfig)`** (22-42|)：
  - 返回方向性变体，`initial`/`exit` 接收 `direction: -1|0|1` (25|, 34|)。
  - `direction===0` 纵向浮起 (`y:8→0`/`y:-4`)；非零横向滑入 (`x:18*dir→0`/`x:-12*dir`) (26|, 35|)。
  - `animate` 使用弹簧：`stiffness:320, damping:24, mass:0.7` (32|)。
  - `exit` 过渡时长 `0.04s` (39|)。

#### `color.ts` (行1-33)
**职责**：HEX 到 HSL 颜色空间转换。

- **`hexToHsl(hex: string)`** (1-33|)：
  - 输入校验：非字符串或长度<4、正则不匹配、NaN 时返回默认 `{ h:230, s:70, l:55 }` (2|, 4|, 10|)。
  - 正则解析 RGB `/^#?([a-f\d]{2}).../i` (3|)。
  - 标准算法计算色相/饱和度/亮度 (12-27|)。
  - 返回整数 `{ h, s, l }` (28-32|)。

#### `easing-config.ts` (行1-29)
**职责**：基于屏幕刷新率的贝塞尔曲线配置。

- **`EasingConfig` 接口** (3-9|)：定义 `enter`/`exit`/`smooth`/`snappy`/`overshoot` 五个 `[number,number,number,number]` 元组。
- **`EASING_60HZ` 常量** (11-17|)：60Hz 曲线。
- **`EASING_120HZ` 常量** (19-25|)：120Hz 曲线（更短时长、更低回弹）。
- **`getEasingConfig(refreshRate: number)`** (27-29|)：`refreshRate >= 120` 返回 `EASING_120HZ`，否则 `EASING_60HZ`。

#### `dock-spring.ts` (行1-164)
**职责**：轻量级弹簧物理仿真内核，替代 CSS Transition 避免拖动顿挫。

- **`SpringSpec` 接口** (17-22|)：`stiffness`、`damping`。
- **`springOfDuration(durationMs, bounce?)`** (33-37|)：由稳定时长+回弹量反解刚度/阻尼。
- **`springOfRatio(stiffness, ratio)`** (40-42|)：由刚度+阻尼比构造。
- **常量**：`REST_VALUE=0.0015` (45|)、`REST_VELOCITY=0.02` (46|)、`MAX_STEP=1/30` (49|) 用于掉帧钳制。
- **`Motion` 类** (77-147|)：
  - 字段：`value` (78|)、`target` (79|)、`velocity` (80|)、`spec` (81|)、`onFrame` (82|)。
  - 构造器 `(initial, onFrame)` (84-88|)。
  - 属性 getter：`current` (90-92|)、`goal` (94-96|)、`speed` (99-102|)、`accel` (104-106|)、`moving` (109-111|)。
  - 方法 `to(target, spec?)` (114-120|)：追向目标，已在活跃集且同目标时不重启。
  - 方法 `jump(value)` (123-129|)：立即落位，清零速度。
  - 方法 `step(dt)` (131-142|)：半隐式欧拉积分，静止判定后停表。
  - 方法 `dispose()` (144-146|)：从活跃集移除。
- **`rubberBand(overshoot, limit)`** (155-159|)：指数衰减橡皮筋。
- **`saturate(value)`** (162-163|)：tanh 型饱和函数。

### 3. 悬浮栏镜头交互 (`lib/`)

#### `glass-dock.tsx` (行1-279)
**职责**：封装 Hyalite（vendor UMD）实现悬浮栏折射玻璃。

- **常量**：`MAX_WAIT_MS=2500` (124|)、`LIVE_FRAMES=2` (126|)。
- **`GLASS_DOCK_OPTS`** (66-121|)：
  - `standard` (67-78|)：`bevel:12, thickness:70, slope:2.0, blur:8, dispersion:0, shade:0.45, rim:0, edge:0, smooth:1`。
  - `ultra` (109-120|)：`bevel:12, thickness:70, slope:2.0, blur:1.6, dispersion:0, shade:0.2, rim:0, edge:0, smooth:1`。
  - 注释强调档位差仅做在 `blur`，`rim:0`/`edge:0` 为硬约定。
- **`useGlassRefraction(ref, level)`** (168-278|)：
  - 阶段①挂滤镜：`hyalite.attach(el, GLASS_DOCK_OPTS[level])` (212-215|)。
  - 阶段②放开半透明：`backdropReady()` (231-241|) 遍历父链查 `opacity/transform/translate/scale/rotate/willChange`；`goLive()` (243-252|) 等 `LIVE_FRAMES` 帧后加 `.glass-live`。
  - 阶段③兜底：`poll()` (255-270|) 超时 `MAX_WAIT_MS` 加 `.glass-ready` 实底显示。
  - 清理：`cleanup` (195-201|) 取消 rAF、移除附着与 class；effect 清理 (273-277|)。

#### `dock-lens.ts` (行1-503)
**职责**：Dock 镜头系统，直接写 DOM style 实现 60fps 镜片放大与果冻形变。

- **常量**：
  - `LENS_GROWTH=8` (21|)、`LENS_MAGNIFY=0.16` (22|)、`LIFT_SNAP=0.975` (25|)、`LIFT_RELEASE=0.04` (27|)
  - `TRAVEL_MAGNIFY_ATTENUATION=0.65` (29|)、`TRAVEL_LIFT_ATTENUATION=0.35` (30|)
  - `JELLY_SPEED=3` (32|)、`JELLY_STRETCH=0.2` (33|)、`JELLY_BRAKE=25` (35|)、`JELLY_SQUASH=0.15` (36|)、`JELLY_ASPECT=1.4` (38|)
  - `GLOW_PEAK=0.06` (40|)、`GLOW_REACH=28` (41|)、`GLOW_GROWTH=3` (42|)
  - `OVERDRAG=0.35` (44|)、`PULL_LIMIT=7/32` (46|)、`FLING_PROJECTION=0.1` (48|)、`TOUCH_SLOP=8` (50|)
- **`SPRING` 对象** (53-70|)：`follow`(springOfRatio 900,1)、`travel`(springOfDuration 320,0.12)、`travelFar`(springOfRatio 420,1)、`lift`(springOfDuration 280,0.08)、`pull`(springOfDuration 200,0.2)、`jelly`(springOfRatio 320,0.55)、`glowIn`(springOfDuration 120,0)、`glowOut`(springOfDuration 360,0)。
- **`DockLensOptions` 接口** (72-81|)：`count: number`、`selected: number`、`onSelect: (i:number)=>void`、`enabled: boolean`。
- **`useDockLens(navRef, opts)`** (93-503|)：
  - 主 effect (99-496|)：
    - `measure()` (119-133|)：计算 `centers`/`widths`/`barW`/`barH`/strip 几何。
    - `xAt(position)` (138-144|)、`widthAt(position)` (145-152|)、`pitch()` (154-155|) 插值。
    - `render()` (176-260|)：写镜片 `width/height/transform/opacity`、放大层 `clip-path`+`scale`、底部 strip 挖洞 `path(evenodd,...)`、光斑 CSS 变量。
    - `requestRender()` (263-271|)：rAF 合帧。
    - `wobbler()` (274-280|)：果冻目标计算。
    - `maybeDrop()` (285-290|)：抬起落下判定。
    - `moveLens()` (307-312|)：选弹簧。
    - `positionAt()` (315-328|)：指针 X→连续索引，越界 `rubberBand`。
    - `stretchFor()` (331-336|)、`setGlow()` (338-341|)。
    - 事件：`onMove` (343-361|)、`inItemStrip` (371-372|)、`onDown` (374-398|，含 `setPointerCapture` try-catch 387-390|)、`flingTarget` (401-412|)、`finish` (414-441|，含 `releasePointerCapture` try-catch 417-420|)、`onUp` (443-446|)、`onCancel` (448-451|)、`onBlur` (455-457|)。
    - `syncRef` (460-463|)：外部改选中仅移镜头。
    - `ResizeObserver` (465-470|)：布局变化重测。
    - 监听挂载 (472-476|) 与清理 (482-494|)。
  - 同步 effect (499-502|)：`opts.selected` 变化调 `syncRef`。

### 4. 系统保活与诊断 (`lib/`)

#### `latency.ts` (行1-60)
**职责**：网络质量数据解析、合并与展示映射。

- **类型导入**：`NetworkQuality` from `@/monitor` (1|)、`QUALITY_CONFIG` from `@/network/constants` (2|)。
- **`LatencyLevel`** (4|)：`'excellent'|'great'|'good'|'fair'|'poor'|'bad'`；`LatencyType` (5|)：`'gateway'|'external'`。
- **`getLatencyLevel(latency)`** (7-15|)：阈值 <0 bad, ≤20 excellent, ≤50 great, ≤100 good, ≤200 fair, ≤400 poor, >400 bad。
- **`getLatencyColor(latency)`** (17-23|)：基于 `QUALITY_CONFIG` 返回 tailwind 类名。
- **`mergeNetworkQuality(old, incoming)`** (25-36|)：`disabled` 返回旧；`busy` 合并 details/metrics 且不采纳 quality 字段；`unknown` 采纳 incoming。
- **`extractGatewayLatency(nq)`** (38-43|)：优先 `gatewayLatency`，次 `details['gateway'].latency`。
- **`extractExternalLatency(nq)`** (45-59|)：优先 `averageExternalLatency`/`externalLatency`，否则非 gateway 项中位数。
- **`resolveQualityDisplay(nq)`** (66-76|)：统一展示解析，延迟≥0 时由 `getLatencyLevel` 推断等级。

#### `notificationPermission.ts` (行1-21)
**职责**：统一通知权限请求入口。

- **`requestNotificationPermission(opts?: {openSettingsIfDenied?: boolean})`** (9-21|)：
  - 动态导入 `@tauri-apps/plugin-notification` (11|)。
  - `isPermissionGranted()` 已授权则返回 (12|)。
  - `requestPermission()` 异常吞掉 (13|)。
  - 仍未授权且 `opts.openSettingsIfDenied` 真时调 `tauriApiWithRetry.openNotificationSettings?.()` (15-17|)。
  - 整体异常仅 dev 警告 (18-20|)。

#### `renderLiveness.ts` (行1-40)
**职责**：检测 GPU/合成器崩溃（rAF 停但 interval 活）。

- **模块态**：`lastRafTime` (14|)、`probing` (15|)。
- **常量**：`PROBE_REFRESH_MS=4000` (18|)、`PROBE_TIMEOUT_MS=500` (20|)、`RENDER_STALL_THRESHOLD_MS=10000` (21|)。
- **`startProbe()`** (24-34|)：开 2 帧 rAF 探测窗口，`setTimeout` 复位防死锁。
- **`isRenderLoopAlive()`** (36-40|)：`document.hidden` 直接 `true` (37|)；超 `PROBE_REFRESH_MS` 启 probe (38|)；判定 `now-lastRafTime <= THRESHOLD` (39|)。

#### `utils.ts` (行1-30)
**职责**：通用工具与存储兜底。

- **`cn(...inputs)`** (4-6|)：`clsx` + `tailwind-merge` 合并类名。
- **`extractErrorMessage(e)`** (8-18|)：优先取 `e.message`（Tauri 对象），兜底 `String(e)`。
- **`safeStorage` 对象** (22-29|)：
  - `get(key)` (23-25|)：先 `localStorage`，失败读 `memoryFallback` Map。
  - `set(key, value)` (26-29|)：写 `localStorage` 并同步 Map，失败仅写 Map 返 `false`。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                      UI Components                           │
│  (Nav, Dashboard, Settings, etc.)                            │
│       ▲          ▲           ▲         ▲                     │
│       │          │           │         │                     │
├───────┼──────────┼───────────┼─────────┼────────────────────┤
│  glass-dock.tsx  │ dock-lens.ts │ animations.ts │ i18n/index.ts│
│  (Hyalite wrap)  │ (Lens Phys) │ (Framer Var) │ (i18next)    │
│       ▲          ▲           │         │                     │
│       │          │           │         │                     │
├───────┼──────────┼───────────┼─────────┼────────────────────┤
│ dock-spring.ts   │ easing-config│ color.ts    │ latency.ts   │
│ (Motion class)   │ (Bezier)    │ (HEX→HSL)   │ (Merge/Extract)│
│       ▲          │           │         │                     │
│       │          │           │         │                     │
├───────┼──────────┼───────────┼─────────┼────────────────────┤
│ renderLiveness.ts│ notifPerm.ts│ utils.ts │ locales/*.json    │
│ (Stall Detect)   │ (Perms)     │ (SafeStore)│ (Translations) │
└──────────────────────────────────────────────────────────────┘
```

- **依赖关系**：
  - `dock-lens.ts` 强依赖 `dock-spring.ts` 的 `Motion`/`rubberBand`/`saturate`（导入于 18|）。
  - `animations.ts` 依赖 `easing-config.ts` 的 `EasingConfig` 类型（1|）。
  - `glass-dock.tsx` 依赖外部 UMD `./vendor/hyalite.js`（19|）及类型 `HyaliteOptions`（20|）。
  - `latency.ts` 依赖 `@/monitor` 与 `@/network/constants` 类型（1-2|）。
  - `i18n` 独立运行，通过 `initReactI18next` 注入 React Context。

## Data Flow

### 1. 国际化数据流
1. **初始化**：`index.ts` 模块加载即执行 `i18n.use(...).init(...)` (8-26|)。
2. **检测**：`detection.order` 先读 `localStorage['app-language']` (19-21|)，失败用 `navigator` (19|)。
3. **持久化**：`caches: ['localStorage']` 写回 (21|)；`lng` 初始化亦尝试读 (17|)。
4. **注入**：`initReactI18next` 将实例注入 React (10|)。
5. **消费**：组件 `useTranslation().t('key')`；缺失键回退 `fallbackLng: 'zh'` (16|)。

### 2. 悬浮栏镜头交互流
1. **输入**：Pointer 事件 `onDown`(374-398|)/`onMove`(343-361|)/`onUp`(443-446|)。
2. **计算**：
   - `positionAt`(315-328|) 屏幕 X→连续索引，越界 `rubberBand`(155-159|)。
   - `Motion.to`(114-120|) 驱动弹簧，产生 `lens.value` 等。
3. **渲染**：`render`(176-260|) 直接改 DOM：
   - 药丸 `transform/width/height/opacity` (189-193|)
   - 放大层 `clip-path`+`scale` (201-207|)
   - strip 挖洞 `clip-path: path(evenodd,...)` (232-236|)
   - 光斑 CSS 变量 `--dock-glow-*` (245-258|)
4. **输出**：`finish`(414-441|) 中 `optsRef.current.onSelect(target)` 通知父组件。

### 3. 网络质量数据流
1. **原始数据**：后端推 `NetworkQuality`（`latency.ts` 1| 导入）。
2. **合并**：`mergeNetworkQuality`(25-36|) 处理 `busy` 增量，保留 details 不采纳 quality。
3. **提取**：`extractGatewayLatency`(38-43|)/`extractExternalLatency`(45-59|) 取数值。
4. **展示**：`resolveQualityDisplay`(66-76|) 推断等级；`getLatencyColor`(17-23|) 映射色彩类。

## Connections

* **[[android-frontend-nav]]**：使用 `dock-lens.ts` 实现导航栏镜头效果，使用 `animations.ts` 实现面板切换。
* **[[android-frontend-monitor]]**：被 `latency.ts` 依赖（`NetworkQuality` 类型，1|）。
* **[[android-frontend-settings]]**：使用 `i18n` 本地化，使用 `notificationPermission.ts` 请求权限，使用 `color.ts` 处理主题色。
* **[[android-frontend-main]]**：启动时初始化 `i18n`，使用 `renderLiveness.ts` 判断渲染存活以重载。

## Known Issues

1. **Glass Dock 祖先层污染**：
   - **现象**：Chromium 下若悬浮栏祖先有 `opacity<1` 或 `transform`，`backdrop-filter` 采样不到内容变黑。
   - **缓解**：`glass-dock.tsx` 的 `backdropReady`(231-241|) 严格等待祖先链干净；超时 2500ms 强制 `.glass-ready` 实底 (264-267|)。

2. **i18n 静态导入体积**：
   - **现象**：`index.ts` 静态 `import zh/en` (5-6|) 使首屏 JS 含全量翻译。
   - **影响**：中英文可控，扩语言需考虑代码分割。

3. **Dock Lens 指针捕获兼容**：
   - **现象**：老 WebView `setPointerCapture` 抛异常 (`dock-lens.ts` 387-390|、`releasePointerCapture` 417-420|)。
   - **缓解**：`try-catch` 忽略，但可能导致拖拽中断。

4. **Spring 物理稳定性**：
   - **现象**：长任务阻塞使 `dt` 过大，弹簧过冲。
   - **缓解**：`dock-spring.ts` 设 `MAX_STEP=1/30` 钳制 (49|, 58|)。

5. **HTML 转义风险**：
   - **现象**：`i18n` 配 `escapeValue: false` (24|)，若传恶意 HTML 可能 XSS。
   - **缓解**：确保 `t()` 参数安全或用 `Trans` 组件。

6. **RenderLiveness 隐藏态误判**：
   - **现象**：`document.hidden` 时 rAF 节流，若返回 false 会误判停滞触发重载。
   - **缓解**：`isRenderLoopAlive` 对 `document.hidden` 直接 `true` (37|)。

7. **Hyalite 引擎支持**：
   - **现象**：非 Chromium（WebKit/Gecko）不支持 SVG 背板滤镜，`useGlassRefraction` 走 `off` 或 `!hyalite.supported()` 分支 (203-209|) 仅加 `.glass-ready` 无玻璃。