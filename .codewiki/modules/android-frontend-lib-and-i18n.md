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
    *   `detection`: 语言检测器配置，优先读取 `localStorage` 中的 `'app-language'`，其次使用 `navigator.language` (19-21|)。
    *   `escapeValue`: `false` (24|)：禁用 HTML 转义，允许 React 组件直接渲染富文本或特殊字符。

*   **函数/方法**：
    *   `init()` (隐含在 IIFE 中, 8-26|)：
        *   尝试从 `localStorage.getItem('app-language')` 获取当前语言 (17|)。
        *   若捕获异常（如隐私模式限制），返回 `undefined` 触发降级检测。
        *   配置 `i18next.init()`，设置 `caches: ['localStorage']` 以持久化检测结果 (21|)。
    *   `export default i18n` (28|)：导出配置好的实例。

#### `locales/en.json` & `locales/zh.json`
**职责**：全量翻译资源文件。

*   **结构特征**：扁平化 JSON，按功能域分组。
*   **关键命名空间 (Namespaces)**：
    *   `notify` (2-19| en): 通知文案。
    *   `common` (20-43| en): 通用按钮/状态。
    *   `nav` (44-55| en): 导航标签。
    *   `titlebar` (56-72| en): 窗口控制。
    *   `auth` (73-98| en): 认证流程。
    *   `account` (99-263| en): 账号管理、ISP 绑定、生物识别、夜间策略。
    *   `network` (264-341| en): 网络适配器、DNS 优化 (阿里 `223.5.5.5`, 腾讯 `1.12.12.12`)。
    *   `monitor` (342-409| en): 网络质量监测、定时任务。
    *   `quality` (410-475| en): 测速详情。
    *   `settings` (503-608| en): 外观、保活、安全设置。
    *   `face` (870-894| en): 人脸识别错误码细分 (`faceTimeout`, `faceMismatch` 等)。
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
        *   输入校验：非字符串或长度 < 4 时返回默认 `{ h: 230, s: 70, l: 55 }` (2, 4|)。
        *   正则解析 RGB (3|)。
        *   标准算法转换，处理色相计算 (18-25|)。
        *   归一化输出整数 HSL 对象 (29-31|)。

#### `easing-config.ts`
**职责**：基于屏幕刷新率的贝塞尔曲线配置单例。

*   **接口/类型**：
    *   `EasingConfig` (3-9|)：定义 `enter`, `exit`, `smooth`, `snappy`, `overshoot` 五种曲线数组。
*   **常量**：
    *   `EASING_60HZ` (11-17|)：标准曲线。
    *   `EASING_120HZ` (19-25|)：高刷适配曲线。
*   **函数**：
    *   `getEasingConfig(refreshRate)` (27-29|)：>= 120 返回 120Hz 配置，否则 60Hz。

#### `dock-spring.ts`
**职责**：轻量级弹簧物理仿真内核，替代 CSS Transition 以避免拖动顿挫。

*   **类**：
    *   `Motion` (77-147|)：
        *   字段：`value` (当前值), `target` (目标值), `velocity` (速度)。
        *   方法 `to(target)` (114-120|)：加入活跃集合，调度 rAF。
        *   方法 `dispose()` (144-146|)：移除实例，清除 rAF。
        *   积分算法：半隐式欧拉积分 (133-134|)。
        *   静止判定：位置差 `< REST_VALUE (0.0015)` 且速度 `< REST_VELOCITY (0.02)` 时停止 (136-140|)。
        *   掉帧钳制：`MAX_STEP = 1/30` (49, 58|)。
*   **辅助函数**：
    *   `springOfDuration(durationMs, bounce)` (33-37|)：反解 stiffness/damping。
    *   `rubberBand(overshoot, limit)` (155-159|)：指数衰减橡皮筋公式。
    *   `saturate(value)` (162-163|)：tanh 型饱和函数。

### 3. 悬浮栏镜头交互 (`lib/`)

#### `glass-dock.tsx`
**职责**：封装 Hyalite 库，实现悬浮栏折射玻璃效果。

*   **常量**：
    *   `GLASS_DOCK_OPTS` (66-121|)：
        *   `standard`: `blur: 8`, `thickness: 70`, `slope: 2.0`, `shade: 0.45` (67-78|)。
        *   `ultra`: `blur: 1.6`, `shade: 0.2` (109-120|)。
        *   通用约束：`bevel: 12`, `rim: 0`, `edge: 0`, `dispersion: 0` (66|)。
*   **Hook**：
    *   `useGlassRefraction(ref, level)` (168-278|)：
        *   阶段 1 - 挂滤镜 (212-215|)：执行 `hyalite.attach`，元素隐藏。
        *   阶段 2 - 放开半透明 (217-252|)：等待祖先链干净（无 opacity<1, transform 等），延迟 2 帧添加 `.glass-live`。
        *   阶段 3 - 兜底可见 (261-269|)：超时 2500ms 未就绪则添加 `.glass-ready` 强制实底显示。
        *   清理 (186-201|, 273-277|)：取消 rAF，移除附件。
*   **辅助函数**：
    *   `backdropReady` (231-241|)：遍历父节点检查样式污染。
    *   `goLive` (243-252|)：rAF 循环等待指定帧后激活。

#### `dock-lens.ts`
**职责**：实现 Dock 栏放大效果，直接操作 DOM Style 保证性能。

*   **常量**：
    *   `LENS_GROWTH = 8` (21|)：镜片膨胀像素。
    *   `LENS_MAGNIFY = 0.16` (22|)：内容放大倍率。
    *   `LIFT_SNAP = 0.975` (23|)：抬起阈值。
    *   `JELLY_STRETCH = 0.2`, `JELLY_SQUASH = 0.15` (24-25|)：果冻形变。
    *   `TOUCH_SLOP = 8` (26|)：拖动最小位移。
*   **Hook**：
    *   `useDockLens(navRef, opts)` (89-430|)：
        *   依赖：`opts.count`, `opts.selected`, `opts.onSelect`, `opts.enabled` (68-77|)。
        *   `measure()` (111-120|)：计算 Item 中心点 `centers` 和宽度 `widths`。
        *   `xAt(position)` (125-131|)：连续位置插值 X 坐标。
        *   `widthAt(position)` (132-139|)：插值药丸宽度 (+4px padding)。
        *   `positionAt(clientX)` (258-271|)：鼠标映射索引，越界应用 `rubberBand`。
        *   `render()` (155-222|)：
            *   计算药丸宽高位置透明度 (156-169|)。
            *   放大层：外层 `clip-path` 裁剪，内层 `scale` 放大 (176-183|)。
            *   挖洞逻辑：底部容器 `clip-path: path(...)` 挖去药丸区域防残影 (194-210|)。
            *   光斑控制：更新 CSS 变量 `--dock-glow-*` (215-221|)。
        *   事件处理：
            *   `onDown` (306-327|)：捕获指针，触发 lift/glow/travel。
            *   `onMove` (286-304|)：更新目标位置，记录样本。
            *   `onUp/onCancel` (343-380|)：提交选择/取消，复位。
            *   `flingTarget(pressed)` (330-341|)：惯性投掷目标计算。
        *   同步机制：
            *   `syncRef` (389-392|)：外部改变 selected 仅移动镜头。
            *   `ResizeObserver` (394-399|)：监听布局变化重算并重置镜头。
        *   边界保护：
            *   指针捕获异常 try-catch (315-319|, 346-350|)。
            *   失焦收尾：监听 `window.blur` 强制复位 (384-386|)。

### 4. 系统保活与诊断 (`lib/`)

#### `latency.ts`
**职责**：网络质量数据解析、合并与展示映射。

*   **函数**：
    *   `getLatencyLevel(latency)` (7-15|)：
        *   阈值：<0->bad, <=20->excellent, <=50->great, <=100->good, <=200->fair, <=400->poor, >400->bad。
    *   `getLatencyColor(latency)` (17-23|)：基于 `QUALITY_CONFIG` 获取 Tailwind 类名。
    *   `mergeNetworkQuality(old, incoming)` (25-36|)：
        *   增量推送处理。若 `incoming.quality === 'busy'`，不采纳 quality 字段，合并 details/metrics，设 quality 为 'unknown' 或保持 'busy' (28-30|)。
    *   `extractGatewayLatency(nq)` (38-43|)：优先取 `gatewayLatency`，其次 `details['gateway'].latency`。
    *   `extractExternalLatency(nq)` (45-59|)：优先取 `averageExternalLatency`，其次 `externalLatency`，最后对非 gateway 项取中位数。
    *   `resolveQualityDisplay(nq)` (66-76|)：统一入口，推断等级，返回展示对象。

#### `notificationPermission.ts`
**职责**：统一通知权限请求入口。

*   **函数**：
    *   `requestNotificationPermission(opts?)` (9-21|)：
        *   动态导入 Tauri Plugin (11|)。
        *   检查已授权 (12|)。
        *   调用 `requestPermission()`，捕获异常视为未授权 (13|)。
        *   若未授权且 `opts.openSettingsIfDenied` 为真，跳转系统设置 (15-17|)。
        *   降级策略：Android 13 以下或国产 ROM 静默处理 (14|)。

#### `renderLiveness.ts`
**职责**：检测 GPU/合成器是否崩溃。

*   **常量**：
    *   `PROBE_REFRESH_MS = 4_000` (18|)：探测间隔。
    *   `PROBE_TIMEOUT_MS = 500` (20|)：开窗兜底超时。
    *   `RENDER_STALL_THRESHOLD_MS = 10_000` (21|)：停滞判定阈值。
*   **函数**：
    *   `startProbe()` (24-34|)：开启 2 帧 rAF 窗口，设置 setTimeout 防死锁。
    *   `isRenderLoopAlive()` (36-40|)：
        *   短路检查：页面隐藏时返回 true (37|)。
        *   按需探测：超过 `PROBE_REFRESH_MS` 启动 probe (38|)。
        *   判定：`performance.now() - lastRafTime <= THRESHOLD` 返回 true (39|)。

#### `utils.ts`
**职责**：通用工具函数。

*   **函数**：
    *   `cn(...)` (4-6|)：组合 `clsx` 和 `tailwind-merge` 合并 Tailwind 类名。
    *   `extractErrorMessage(e)` (8-18|)：智能提取 string/Error/Tauri object 的消息，兜底 String(e)。
*   **对象**：
    *   `safeStorage` (22-29|)：
        *   `get(key)`：先读 localStorage，失败读内存 Map。
        *   `set(key, value)`：写入 localStorage，成功同步写内存 Map；失败仅写内存 Map 并返回 false。

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
    *   `dock-lens` 强依赖 `dock-spring` 进行物理计算。
    *   `animations` 依赖 `easing-config` 获取曲线。
    *   `glass-dock` 依赖外部 UMD 库 `hyalite.js`。
    *   `latency` 依赖 `monitor` 类型定义。
    *   `i18n` 独立运行，通过 React Context 向上提供服务。

## Data Flow

### 1. 国际化数据流
1.  **初始化**：`index.ts` 在应用启动时执行 IIFE。
2.  **检测**：尝试读取 `localStorage['app-language']`。若失败，使用 `navigator.language`。
3.  **持久化**：检测结果写入 `localStorage`。
4.  **注入**：通过 `initReactI18next` 将 `i18n` 实例注入 React 上下文。
5.  **消费**：组件调用 `useTranslation().t('key')`。若 key 缺失，回退至 `zh`。

### 2. 悬浮栏镜头交互流
1.  **输入**：用户 Pointer 事件 (`onDown`, `onMove`, `onUp`)。
2.  **计算**：
    *   `positionAt` 将屏幕坐标映射为连续索引。
    *   `rubberBand` 处理越界拉伸。
    *   `Motion.to` 驱动弹簧物理模拟，产生平滑的 `lens.value`。
3.  **渲染**：
    *   `render` 读取 `lens.value`。
    *   直接修改 DOM `style`：
        *   药丸容器：`transform`, `width`, `height`, `opacity`。
        *   放大层：`clip-path` 裁剪 + `scale` 放大。
        *   底部容器：`clip-path` 挖洞。
        *   光斑：CSS 变量更新。
4.  **输出**：`onSelect(index)` 回调通知父组件更新选中状态。

### 3. 网络质量数据流
1.  **原始数据**：后端推送 `NetworkQuality` 对象（含 `quality`, `details`, `metrics`）。
2.  **合并**：`mergeNetworkQuality` 处理增量推送。若状态为 `busy`，保留细节但标记为 `unknown` 以防 UI 卡死。
3.  **提取**：`extractGatewayLatency` / `extractExternalLatency` 从嵌套结构中获取数值，处理缺失字段。
4.  **展示解析**：`resolveQualityDisplay` 根据延迟值推断等级，结合原始质量标签。
5.  **渲染**：`getLatencyColor` 获取 Tailwind 类名，UI 组件渲染对应颜色的标签。

## Connections

*   **[[android-frontend-nav]]**: 使用 `dock-lens.ts` 实现导航栏的镜头效果，使用 `animations.ts` 实现面板切换动画。
*   **[[android-frontend-monitor]]**: 被 `latency.ts` 依赖，提供 `NetworkQuality` 类型定义。
*   **[[android-frontend-settings]]**: 使用 `i18n` 进行界面本地化，使用 `notificationPermission.ts` 管理通知权限，使用 `color.ts` 处理主题色选择。
*   **[[android-frontend-main]]**: 在启动时初始化 `i18n`，使用 `renderLiveness.ts` 判断是否需要重载页面。

## Known Issues

1.  **Glass Dock 祖先层污染**：
    *   **现象**：Chromium 内核下，若悬浮栏祖先元素存在 `opacity < 1` 或 `transform`，`backdrop-filter` 会采样不到内容导致变黑。
    *   **缓解**：`glass-dock.tsx` 中的 `backdropReady` 严格等待祖先链干净，超时 2.5s 强制显示实底 (`glass-ready`) (261-269|)。

2.  **i18n 静态导入体积**：
    *   **现象**：`index.ts` 使用静态 `import` 加载所有语言包 (5-6|)，导致首屏 JS 包含全部翻译资源。
    *   **影响**：对于仅中英文且体积可控的应用可接受，但若扩展更多语言可能影响加载性能。

3.  **Dock Lens 指针捕获兼容性**：
    *   **现象**：老 WebView 或特定场景下 `setPointerCapture` 可能抛出异常。
    *   **缓解**：代码中使用 `try-catch` 忽略异常 (315-319|, 346-350|)，但可能导致后续拖动手势失效。

4.  **Spring 物理稳定性**：
    *   **现象**：长任务阻塞可能导致下一帧 `dt` 过大，弹簧冲过目标值。
    *   **缓解**：`dock-spring.ts` 设置 `MAX_STEP = 1/30` 钳制最大步长 (49, 58|)。

5.  **HTML 转义风险**：
    *   **现象**：`i18n` 配置 `escapeValue: false` (24|)，若传入恶意 HTML 可能导致 XSS。
    *   **缓解**：开发者需确保 `t()` 参数安全，或使用 `Trans` 组件处理富文本。