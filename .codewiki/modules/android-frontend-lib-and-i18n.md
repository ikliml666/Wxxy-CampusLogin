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
1.  **国际化 (i18n)**：基于 `i18next` 和 `react-i18next` 构建的全局语言管理单例，负责多语言资源的加载、检测、持久化及 React 上下文注入。
2.  **前端底层工具库**：
    *   **动画与物理引擎**：提供基于 Framer Motion 的变体生成器、自定义弹簧物理内核 (`dock-spring`)、高刷适配的缓动配置 (`easing-config`) 以及色彩空间转换工具。
    *   **悬浮栏交互**：实现具有折射玻璃效果（Hyalite）的停靠透镜（Dock Lens），通过直接 DOM 操作实现 60fps+ 的镜片放大与果冻形变效果。
    *   **系统保活与诊断**：包括网络延迟解析合并、GPU/合成器渲染循环存活检测、通知权限统一请求入口。

该模块不直接处理业务逻辑，而是为上层 UI 组件提供标准化的视觉反馈、语言支持及系统级状态监控能力。

## Key Components

### 1. 国际化模块 (`i18n/`)

#### `index.ts`
**职责**：全局国际化单例初始化入口。

*   **常量/配置**：
    *   `fallbackLng`: `'zh'` (16|)：默认回退语言。
    *   `resources`: 包含 `zh` 和 `en` 两个命名空间的静态 JSON 对象 (5-6|, 12-15|)。
    *   `detection`: 语言检测器配置，优先读取 `localStorage` 中的 `'app-language'`，其次使用 `navigator.language` (18-22|)。
    *   `escapeValue`: `false` (24|)：禁用 HTML 转义，允许 React 组件直接渲染富文本或特殊字符。
    *   `lng`: 初始化时尝试从 `localStorage.getItem('app-language')` 获取，异常则返回 `undefined` (17|)。
*   **函数/方法**：
    *   `i18n.use(LanguageDetector).use(initReactI18next).init({...})` (8-26|)：链式调用初始化，配置上述参数，并设置 `caches: ['localStorage']` 持久化检测结果 (21|)。
    *   `export default i18n` (28|)：导出配置好的实例。

#### `locales/en.json` & `locales/zh.json`
**职责**：全量翻译资源文件。

*   **结构特征**：扁平化 JSON，按功能域分组。
*   **关键命名空间 (Namespaces)**：
    *   `notify` (en:2-19|, zh:2-19|): 通知文案。
    *   `common` (en:20-43|, zh:20-43|): 通用按钮/状态。
    *   `nav` (en:44-55|, zh:44-55|): 导航标签。
    *   `titlebar` (en:56-72|, zh:56-72|): 窗口控制。
    *   `auth` (en:73-98|, zh:73-98|): 认证流程。
    *   `account` (en:99-263|, zh:99-263|): 账号管理、ISP 绑定、生物识别、夜间策略。
    *   `network` (en:264-341|, zh:264-341|): 网络适配器、DNS 优化 (阿里 `223.5.5.5`, 腾讯 `1.12.12.12`)。
    *   `monitor` (en:342-409|, zh:342-409|): 网络状态检测、定时任务。
    *   `quality` (en:410-475|, zh:410-475|): 测速详情。
    *   `speedtest` (en:476-502|, zh:476-502|): 测速网站。
    *   `settings` (en:503-608|, zh:503-608|): 外观、保活、安全设置。
    *   `log` (en:609-646|, zh:609-646|): 日志相关。
    *   `onboarding` (en:647-708|, zh:647-708|): 新手引导。
    *   `updatePrompt` (en:709-714|, zh:709-714|): 更新提示。
    *   `about` (en:715-757|, zh:715-757|): 关于页。
    *   `statusbar` (en:758-770|, zh:758-770|): 状态栏。
    *   `dashboard` (en:771-808|, zh:771-808|): 总览页。
    *   `confirmDialog` (en:809-812|, zh:809-812|): 确认对话框。
    *   `panel` (en:813-832|, zh:813-832|): 面板描述。
    *   `themeDialog` (en:833-848|, zh:833-848|): 主题设置对话框。
    *   `rightPanel` (en:849-853|, zh:849-853|): 右面板。
    *   `sponsor` (en:854-859|, zh:854-859|): 赞助。
    *   `mobile` (en:860-869|, zh:860-869|): 移动端。
    *   `face` (en:870-894|, zh:870-894|): 人脸识别错误码细分 (`faceTimeout`, `faceMismatch` 等)。
*   **动态插值**：广泛使用 `{{seconds}}`, `{{version}}`, `{{name}}`, `{{msg}}` 等占位符。

### 2. 动画与物理引擎 (`lib/`)

#### `animations.ts`
**职责**：为 Framer Motion 提供标准化动画变体。

*   **函数**：
    *   `createLogEntryVariants(easing)` (3-17|)：
        *   输入：`EasingConfig`。
        *   输出：日志列表项入场/出场动画配置。
        *   入场：`opacity 0->1`, `x 20->0`, `snappy` 缓动, 0.3s (9|)。
        *   退场：`opacity 1->0`, `x 0->-16`, `exit` 缓动, 0.15s (14|)。
    *   `createPanelAppleVariants(easing)` (22-42|)：
        *   参数：`direction: -1 | 0 | 1` (25|)。
        *   逻辑：`direction=0` 纵向浮起 (`y: 8->-4`)；非零横向滑入 (`x: 18*dir -> -12*dir`) (26, 35|)。
        *   物理参数：`stiffness: 320`, `damping: 24`, `mass: 0.7` (32|)。
        *   退出过渡：极短时长 `0.04s` (39|)。

#### `color.ts`
**职责**：HEX 到 HSL 颜色空间转换。

*   **函数**：
    *   `hexToHsl(hex)` (1-33|)：
        *   输入校验：非字符串或长度 < 4 时返回默认 `{ h: 230, s: 70, l: 55 }` (2, 10|)。
        *   正则解析 RGB (3|)。
        *   标准算法转换，处理色相计算 (18-26|)。
        *   归一化输出整数 HSL 对象 (28-32|)。

#### `easing-config.ts`
**职责**：基于屏幕刷新率的贝塞尔曲线配置。

*   **接口/类型**：
    *   `EasingConfig` (3-9|)：定义 `enter`, `exit`, `smooth`, `snappy`, `overshoot` 五种 `[number, number, number, number]` 曲线数组。
*   **常量**：
    *   `EASING_60HZ` (11-17|)：标准曲线（未导出，被 `getEasingConfig` 使用）。
    *   `EASING_120HZ` (19-25|)：高刷适配曲线（导出）。
*   **函数**：
    *   `getEasingConfig(refreshRate)` (27-29|)：>= 120 返回 `EASING_120HZ`，否则 `EASING_60HZ`。

#### `dock-spring.ts`
**职责**：轻量级弹簧物理仿真内核，替代 CSS Transition 以避免拖动顿挫。

*   **接口**：
    *   `SpringSpec` (17-22|)：字段 `stiffness: number`, `damping: number`。
*   **常量**：
    *   `REST_VALUE = 0.0015` (45|)，`REST_VELOCITY = 0.02` (46|)，`MAX_STEP = 1/30` (49|)。
*   **函数**：
    *   `springOfDuration(durationMs, bounce)` (33-37|)：由稳定时长+回弹量反解 stiffness/damping。
    *   `springOfRatio(stiffness, ratio)` (40-42|)：由刚度+阻尼比构造。
    *   `rubberBand(overshoot, limit)` (155-159|)：指数衰减橡皮筋公式。
    *   `saturate(value)` (162-163|)：tanh 型饱和函数。
*   **类**：
    *   `Motion` (77-147|)：
        *   字段：`value` (当前值), `target` (目标值), `velocity` (速度), `spec` (SpringSpec), `onFrame` (回调) (78-82|)。
        *   属性方法：`get current` (90-92|), `get goal` (94-96|), `get speed` (99-101|), `get accel` (104-106|), `get moving` (109-111|)。
        *   方法 `to(target, spec?)` (114-120|)：追向新目标，加入活跃集合并调度 rAF。
        *   方法 `jump(value)` (123-129|)：立即落位，不产生运动。
        *   方法 `step(dt)` (131-142|)：半隐式欧拉积分，静止判定后自动停表。
        *   方法 `dispose()` (144-146|)：移除实例。

### 3. 悬浮栏镜头交互 (`lib/`)

#### `glass-dock.tsx`
**职责**：封装 Hyalite 库，实现悬浮栏折射玻璃效果。

*   **常量**：
    *   `GLASS_DOCK_OPTS` (66-121|)：
        *   `standard`: `bevel:12`, `thickness:70`, `slope:2.0`, `blur:8`, `dispersion:0`, `shade:0.45`, `rim:0`, `edge:0`, `smooth:1` (67-78|)。
        *   `ultra`: `bevel:12`, `thickness:70`, `slope:2.0`, `blur:1.6`, `dispersion:0`, `shade:0.2`, `rim:0`, `edge:0`, `smooth:1` (109-120|)。
    *   `MAX_WAIT_MS = 2500` (124|)，`LIVE_FRAMES = 2` (126|)。
*   **Hook**：
    *   `useGlassRefraction(ref, level)` (168-278|)：
        *   阶段 1 - 挂滤镜 (212-215|)：执行 `hyalite.attach`，设置 `attached.current = true`。
        *   阶段 2 - 放开半透明 (217-252|)：等待祖先链干净（无 opacity<1, transform 等），延迟 2 帧添加 `.glass-live`（通过 `goLive` 243-252|）。
        *   阶段 3 - 兜底可见 (254-271|)：超时 `MAX_WAIT_MS` 未就绪则添加 `.glass-ready` 强制实底显示 (266|)。
        *   清理 (186-201|, 273-277|)：取消 rAF，移除附件，移除 class。
*   **辅助函数**（内部）：
    *   `backdropReady()` (231-241|)：遍历父节点检查样式污染。
    *   `goLive()` (243-252|)：rAF 循环等待指定帧后激活。
    *   `poll()` (255-271|)：轮询祖先链状态。

#### `dock-lens.ts`
**职责**：实现 Dock 栏放大镜头系统，直接操作 DOM Style 保证性能。

*   **常量**：
    *   `LENS_GROWTH = 8` (21|)，`LENS_MAGNIFY = 0.16` (23|)，`LIFT_SNAP = 0.975` (25|)，`LIFT_RELEASE = 0.04` (27|)。
    *   `TRAVEL_MAGNIFY_ATTENUATION = 0.65` (29|)，`TRAVEL_LIFT_ATTENUATION = 0.35` (30|)。
    *   `JELLY_SPEED = 3` (32|)，`JELLY_STRETCH = 0.2` (33|)，`JELLY_BRAKE = 25` (35|)，`JELLY_SQUASH = 0.15` (36|)，`JELLY_ASPECT = 1.4` (38|)。
    *   `GLOW_PEAK = 0.06` (40|)，`GLOW_REACH = 28` (41|)，`GLOW_GROWTH = 3` (42|)。
    *   `OVERDRAG = 0.35` (44|)，`PULL_LIMIT = 7/32` (46|)，`FLING_PROJECTION = 0.1` (48|)，`TOUCH_SLOP = 8` (50|)。
    *   `SPRING` 对象 (53-70|)：各曲线弹簧手感配置。
*   **接口**：
    *   `DockLensOptions` (72-81|)：字段 `count: number`, `selected: number`, `onSelect: (index: number) => void`, `enabled: boolean`。
*   **Hook**：
    *   `useDockLens(navRef, opts)` (93-503|)：
        *   主 effect (99-496|)：测量布局 `measure()` (119-133|)，计算 `xAt` (138-144|)、`widthAt` (145-152|)、`pitch` (154-155|)、`clampIndex` (137|)。
        *   渲染 `render()` (176-260|)：计算药丸宽高位置透明度，放大层 clip-path + scale，底部容器挖洞防残影，光斑 CSS 变量更新。
        *   弹簧共享 rAF：`requestRender` (263-271|)，`wobbler` (274-280|)，`maybeDrop` (285-290|)。
        *   运动控制：`moveLens` (307-312|)，`positionAt` (315-328|)，`stretchFor` (331-336|)，`setGlow` (338-341|)。
        *   事件处理：`onMove` (343-361|)，`inItemStrip` (371-372|)，`onDown` (374-398|，含指针捕获 try-catch 386-390|)，`flingTarget` (401-412|)，`finish` (414-441|，含释放捕获 try-catch 417-421|)，`onUp` (443-447|)，`onCancel` (448-451|)，`onBlur` (455-457|)。
        *   同步机制：`syncRef` (460-463|) 供外部改选中；`ResizeObserver` (465-470|) 监听布局变化。
        *   次 effect 同步选中 (499-502|)。

### 4. 系统保活与诊断 (`lib/`)

#### `latency.ts`
**职责**：网络质量数据解析、合并与展示映射。

*   **类型**：
    *   `LatencyType` (5|)：`'gateway' | 'external'`。
*   **函数**：
    *   `getLatencyLevel(latency)` (7-15|)：阈值映射 excellent/great/good/fair/poor/bad。
    *   `getLatencyColor(latency)` (17-23|)：基于 `QUALITY_CONFIG` 获取 Tailwind 类名。
    *   `mergeNetworkQuality(old, incoming)` (25-36|)：增量推送处理，若 `incoming.quality === 'busy'` 不采纳 quality 字段，合并 details/metrics (29-33|)。
    *   `extractGatewayLatency(nq)` (38-43|)：优先取 `gatewayLatency`，其次 `details['gateway'].latency`。
    *   `extractExternalLatency(nq)` (45-59|)：优先取 `averageExternalLatency`，其次 `externalLatency`，最后对非 gateway 项取中位数。
    *   `resolveQualityDisplay(nq)` (66-76|)：统一入口，推断等级，返回展示对象。

#### `notificationPermission.ts`
**职责**：统一通知权限请求入口。

*   **函数**：
    *   `requestNotificationPermission(opts?)` (9-21|)：动态导入 Tauri Plugin，检查已授权；调用 `requestPermission()` 捕获异常；若未授权且 `opts.openSettingsIfDenied` 为真，跳转系统设置；Android 13 以下或国产 ROM 静默处理。

#### `renderLiveness.ts`
**职责**：检测 GPU/合成器是否崩溃导致渲染循环停滞。

*   **常量**：
    *   `PROBE_REFRESH_MS = 4_000` (18|)，`PROBE_TIMEOUT_MS = 500` (20|)，`RENDER_STALL_THRESHOLD_MS = 10_000` (21|)。
*   **函数**：
    *   `startProbe()` (24-34|)：开启 2 帧 rAF 探测窗口，setTimeout 防死锁。
    *   `isRenderLoopAlive()` (36-40|)：页面隐藏时返回 true (37|)；超过 `PROBE_REFRESH_MS` 启动 probe (38|)；判定 `performance.now() - lastRafTime <= THRESHOLD` 返回 true (39|)。

#### `utils.ts`
**职责**：通用工具函数与存储降级。

*   **函数**：
    *   `cn(...inputs)` (4-6|)：组合 `clsx` 和 `tailwind-merge` 合并 Tailwind 类名。
    *   `extractErrorMessage(e)` (8-18|)：智能提取 string/Error/Tauri object 的 message 字段，兜底 String(e)。
*   **对象**：
    *   `safeStorage` (22-29|，经 `export { safeStorage }` (31|) 导出)：
        *   `get(key)` (23-25|)：先读 localStorage，失败读内存 Map。
        *   `set(key, value)` (26-28|)：写入 localStorage 并同步内存 Map，失败仅写内存 Map 返回 false。

## Architecture

本模块采用分层架构，自下而上依次为：基础工具层、物理/动画引擎层、UI 交互层、业务支撑层（i18n/诊断）。

```text
┌──────────────────────────────────────────────────────────────┐
│                      UI Components                           │
│  (Nav, Dashboard, Settings, etc.)                            │
│       ▲          ▲           ▲         ▲                     │
│       │          │           │         │                     │
├───────┼──────────┼───────────┼─────────┼────────────────────┤
│  [[glass-dock]]  │ [[dock-lens]] │ [[animations]] │ [[i18n]] │
│  (Glass Effect)  │ (Lens Physics)│ (Framer Variants)│ (Lang) │
│       ▲          ▲           │         │                     │
│       │          │           │         │                     │
├───────┼──────────┼───────────┼─────────┼────────────────────┤
│ [[dock-spring]]  │ [[easing]]  │ [[color]]     │ [[latency]]│
│ (Physics Core)   │ (Bezier)    │ (Utils)       │ (Merge)    │
│       ▲          │           │         │                     │
│       │          │           │         │                     │
├───────┼──────────┼───────────┼─────────┼────────────────────┤
│ [[renderLiveness]] │ [[notifPerm]] │ [[utils]]      │
│ (Stall Detect)   │ (Perms)       │ (Safe Storage) │
└──────────────────────────────────────────────────────────────┘
```

*   **依赖关系**：
    *   `dock-lens` 强依赖 `dock-spring` 进行物理计算 (`dock-lens.ts` 18| 导入)。
    *   `animations` 依赖 `easing-config` 获取曲线 (`animations.ts` 1|)。
    *   `glass-dock` 依赖外部 UMD 库 `hyalite.js` (`glass-dock.tsx` 19|)。
    *   `latency` 依赖 `monitor` 类型定义 (`latency.ts` 1|)。
    *   `i18n` 独立运行，通过 React Context 向上提供服务。

## Data Flow

### 1. 国际化数据流
1.  **初始化**：`index.ts` 在应用启动时执行链式初始化 (8-26|)。
2.  **检测**：尝试读取 `localStorage['app-language']` (17|)。若失败，使用 `navigator.language` (19-20|)。
3.  **持久化**：检测结果写入 `localStorage` (`caches: ['localStorage']` 21|)。
4.  **注入**：通过 `initReactI18next` 将 `i18n` 实例注入 React 上下文 (2|, 10|)。
5.  **消费**：组件调用 `useTranslation().t('key')`。若 key 缺失，回退至 `zh` (16|)。

### 2. 悬浮栏镜头交互流
1.  **输入**：用户 Pointer 事件 (`onDown` 374-398|, `onMove` 343-361|, `onUp` 443-447|)。
2.  **计算**：
    *   `positionAt` (315-328|) 将屏幕坐标映射为连续索引。
    *   `rubberBand` (`dock-spring.ts` 155-159|) 处理越界拉伸。
    *   `Motion.to` (`dock-spring.ts` 114-120|) 驱动弹簧物理模拟，产生平滑的 `lens.value`。
3.  **渲染**：
    *   `render` (176-260|) 读取 `lens.value`。
    *   直接修改 DOM `style`：
        *   药丸容器：`transform`, `width`, `height`, `opacity`。
        *   放大层：`clip-path` 裁剪 + `scale` 放大。
        *   底部容器：`clip-path` 挖洞。
        *   光斑：CSS 变量更新。
4.  **输出**：`onSelect(index)` 回调通知父组件更新选中状态 (`optsRef.current.onSelect` 434|)。

### 3. 网络质量数据流
1.  **原始数据**：后端推送 `NetworkQuality` 对象（含 `quality`, `details`, `metrics`）。
2.  **合并**：`mergeNetworkQuality` (25-36|) 处理增量推送。若状态为 `busy`，保留细节但标记为 `unknown` 以防 UI 卡死 (29-33|)。
3.  **提取**：`extractGatewayLatency` (38-43|) / `extractExternalLatency` (45-59|) 从嵌套结构中获取数值，处理缺失字段。
4.  **展示解析**：`resolveQualityDisplay` (66-76|) 根据延迟值推断等级，结合原始质量标签。
5.  **渲染**：`getLatencyColor` (17-23|) 获取 Tailwind 类名，UI 组件渲染对应颜色的标签。

## Connections

*   **[[android-frontend-nav]]**: 使用 `dock-lens.ts` 实现导航栏的镜头效果，使用 `animations.ts` 实现面板切换动画。
*   **[[android-frontend-monitor]]**: 被 `latency.ts` 依赖，提供 `NetworkQuality` 类型定义 (`latency.ts` 1|)。
*   **[[android-frontend-settings]]**: 使用 `i18n` 进行界面本地化，使用 `notificationPermission.ts` 管理通知权限，使用 `color.ts` 处理主题色选择。
*   **[[android-frontend-main]]**: 在启动时初始化 `i18n`，使用 `renderLiveness.ts` 判断是否需要重载页面。

## Known Issues

1.  **Glass Dock 祖先层污染**：
    *   **现象**：Chromium 内核下，若悬浮栏祖先元素存在 `opacity < 1` 或 `transform`，`backdrop-filter` 会采样不到内容导致变黑。
    *   **缓解**：`glass-dock.tsx` 中的 `backdropReady` (231-241|) 严格等待祖先链干净，超时 `MAX_WAIT_MS = 2500` (124|) 强制显示实底 (`glass-ready`) (266|)。

2.  **i18n 静态导入体积**：
    *   **现象**：`index.ts` 使用静态 `import` 加载所有语言包 (5-6|)，导致首屏 JS 包含全部翻译资源。
    *   **影响**：对于仅中英文且体积可控的应用可接受，但若扩展更多语言可能影响加载性能。

3.  **Dock Lens 指针捕获兼容性**：
    *   **现象**：老 WebView 或特定场景下 `setPointerCapture` / `releasePointerCapture` 可能抛出异常。
    *   **缓解**：代码中使用 `try-catch` 忽略异常 (`onDown` 386-390|, `finish` 417-421|)，但可能导致后续拖动手势失效。

4.  **Spring 物理稳定性**：
    *   **现象**：长任务阻塞可能导致下一帧 `dt` 过大，弹簧冲过目标值。
    *   **缓解**：`dock-spring.ts` 设置 `MAX_STEP = 1/30` 钳制最大步长 (49|, 58|)。

5.  **HTML 转义风险**：
    *   **现象**：`i18n` 配置 `escapeValue: false` (24|)，若传入恶意 HTML 可能导致 XSS。
    *   **缓解**：开发者需确保 `t()` 参数安全，或使用 `Trans` 组件处理富文本。
