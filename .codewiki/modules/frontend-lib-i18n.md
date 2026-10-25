---
title: frontend-lib-i18n
type: module
source_files:
  - android/frontend/src/i18n/index.ts
  - android/frontend/src/i18n/locales/en.json
  - android/frontend/src/lib/animations.ts
  - android/frontend/src/lib/color.ts
  - android/frontend/src/lib/easing-config.ts
  - android/frontend/src/lib/latency.ts
  - '@/monitor'
  - '@/network/constants'
tags: [i18n, localization, animations, colors, easing, latency, utilities]
---

## Overview
模块 `frontend-lib-i18n` 是前端应用的核心基础库集合，负责国际化支持、视觉动画配置、颜色转换以及网络延迟计算。它不直接处理 UI 渲染，而是为上层组件提供标准化的数据格式和工具函数。该模块涵盖了从语言包加载、主题色解析到复杂网络质量评估的多个底层逻辑，源文件分布在 `android/frontend/src/i18n/` 与 `android/frontend/src/lib/` 目录下。

## Key Components

### Internationalization (i18n)
**文件**: `android/frontend/src/i18n/index.ts`

#### 模块初始化（i18n 实例配置）
- **行号**: 8-26
- **描述**: 在模块加载时直接调用 `i18n.use(LanguageDetector).use(initReactI18next).init({...})` 完成 `i18next` 实例初始化并挂载到 React。
- **配置细节**:
  - 导入语言包：行 5-6 导入 `zh` 与 `en` JSON；行 12-15 在 `resources` 中注册为 `zh.translation` 与 `en.translation`。
  - 后备语言 `fallbackLng: 'zh'`（行 16）。
  - 初始语言 `lng`：行 17 通过 IIFE 尝试 `localStorage.getItem('app-language')`，异常时返回 `undefined`。
  - 语言检测 `detection`：行 18-22 配置 `order: ['localStorage', 'navigator']`，`lookupLocalStorage: 'app-language'`，`caches: ['localStorage']`。
  - 插值转义 `interpolation.escapeValue: false`（行 24），允许翻译字符串包含 HTML。
- **导出**: 行 28 `export default i18n`。

**文件**: `android/frontend/src/i18n/locales/en.json`
- **结构**: JSON 对象，嵌套结构包含以下顶级键（行号为对应键起始行）：
  - `notify` (行 2): 通知相关文本。
  - `common` (行 20): 通用按钮和状态文本。
  - `nav` (行 44): 导航栏标签。
  - `titlebar` (行 56): 标题栏功能项。
  - `auth` (行 73): 认证登录流程文本。
  - `account` (行 99): 账号管理、自助服务、生物识别等。
  - `network` (行 264): 网络适配器、DNS 优化、夜间切换等。
  - `monitor` (行 342): 网络状态监测逻辑和结果。
  - `quality` (行 410): 网络质量检测详情和解释。
  - `speedtest` (行 476): 测速网站列表和提示。
  - `settings` (行 503): 外观、启动、保活、安全等设置项。
  - `log` (行 609): 系统日志查看和管理。
  - `onboarding` (行 647): 新手引导流程。
  - `updatePrompt` (行 709): 更新提示。
  - `about` (行 715): 关于页面信息。
  - `statusbar` (行 758): 状态栏显示内容。
  - `dashboard` (行 771): 仪表盘卡片和快捷操作。
  - `confirmDialog` (行 809): 确认对话框。
  - `panel` (行 813): 面板标题和描述。
  - `themeDialog` (行 833): 主题设置对话框。
  - `rightPanel` (行 849): 右侧面板日志。
  - `sponsor` (行 854): 赞助信息。
  - `mobile` (行 860): 移动端特定标签。
  - `face` (行 870): 人脸录入和验证界面。

### Animation Utilities
**文件**: `android/frontend/src/lib/animations.ts`

#### `createLogEntryVariants(easing: EasingConfig)`
- **行号**: 3-17
- **描述**: 为日志条目创建 Framer Motion 变体。
- **参数**: `easing` - 缓动配置对象（来自 `easing-config.ts`）。
- **返回值**: 包含 `initial`、`animate`、`exit` 属性的对象。
  - `initial`: 透明度 0，向右偏移 20px（行 5）。
  - `animate`: 透明度 1，归位，使用 `easing.snappy` 缓动，时长 0.3s（行 6-10）。
  - `exit`: 透明度 0，向左偏移 -16px，使用 `easing.exit` 缓动，时长 0.15s（行 11-15）。

#### `createPanelAppleVariants(easing: EasingConfig)`
- **行号**: 22-42
- **描述**: 创建类似 Apple 风格的侧边栏/面板转场变体，支持方向参数。
- **参数**: `easing` - 缓动配置对象。
- **返回值**: 包含 `initial`、`animate`、`exit` 属性的对象，这些属性是函数，接收 `direction: -1 | 0 | 1`（默认 0）。
  - `initial(direction)`（行 25-26）: 方向 0 时 `{ y: 8, opacity: 0.9 }`；方向 ±1 时 `{ x: 18 * direction, opacity: 0.9 }`。
  - `animate()`（行 27-33）: 归位至原点，透明度 1；使用弹簧动画 `type: 'spring', stiffness: 320, damping: 24, mass: 0.7`。
  - `exit(direction)`（行 34-40）: 方向 0 时 `{ y: -4, opacity: 0.9, scale: 0.99 }`；方向 ±1 时 `{ x: -12 * direction, opacity: 0.9, scale: 0.99 }`；过渡时长 0.04s，缓动为 `easing.exit`。
- **注释**: 行 19-21 说明方向由页签索引差决定。

### Color Utilities
**文件**: `android/frontend/src/lib/color.ts`

#### `hexToHsl(hex: string)`
- **行号**: 1-33
- **描述**: 将 HEX 颜色代码转换为 HSL 对象。
- **参数**: `hex` - 十六进制颜色字符串（如 `#RRGGBB`）。
- **返回值**: `{ h: number; s: number; l: number }`。
- **错误处理**: 行 2、4、10 若类型不符、长度不足、正则不匹配或解析出 NaN，返回默认值 `{ h: 230, s: 70, l: 55 }`（行 2、4、10）。
- **实现细节**:
  - 行 3 正则 `/^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i` 提取 RGB。
  - 行 6-8 解析并归一化到 0-1。
  - 行 12-16 计算 max/min 与亮度 L。
  - 行 18-26 计算饱和度 S 与色相 H（依据最大分量分支）。
  - 行 28-32 返回四舍五入后的整数 H、S、L。

### Easing Configuration
**文件**: `android/frontend/src/lib/easing-config.ts`

#### Interface `EasingConfig`
- **行号**: 3-9
- **描述**: 定义缓动曲线的贝塞尔控制点数组 `[x1, y1, x2, y2]`。
- **字段**:
  - `enter: [number, number, number, number]`（行 4）: 面板转场（戏剧性 expo-out）。
  - `exit: [number, number, number, number]`（行 5）: 干脆 ease-in。
  - `smooth: [number, number, number, number]`（行 6）: 卡片/列表平滑 ease-out。
  - `snappy: [number, number, number, number]`（行 7）: 日志/徽章快捷。
  - `overshoot: [number, number, number, number]`（行 8）: 弹性效果。

#### Constants
- **`EASING_60HZ`**（行 11-17）: 针对 60Hz 屏幕的 `EasingConfig` 常量（非导出）。
- **`EASING_120HZ`**（行 19-25）: 针对 120Hz 高刷屏幕的导出常量，曲线更紧凑。

#### `getEasingConfig(refreshRate: number)`
- **行号**: 27-29
- **描述**: 根据屏幕刷新率返回对应缓动配置。
- **逻辑**: `refreshRate >= 120` 返回 `EASING_120HZ`，否则返回 `EASING_60HZ`。

### Latency Calculation
**文件**: `android/frontend/src/lib/latency.ts`

#### Type Aliases
- **`LatencyLevel`**（行 4）: `'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad'`。
- **`LatencyType`**（行 5，导出）: `'gateway' | 'external'`。

#### `getLatencyLevel(latency: number)`
- **行号**: 7-15
- **描述**: 根据延迟毫秒数返回质量等级。
- **阈值**:
  - `< 0`: `bad`
  - `<= 20`: `excellent`
  - `<= 50`: `great`
  - `<= 100`: `good`
  - `<= 200`: `fair`
  - `<= 400`: `poor`
  - `> 400`: `bad`

#### `getLatencyColor(latency: number)`
- **行号**: 17-23
- **描述**: 根据延迟返回对应的 Tailwind CSS 类名对象。
- **返回值**: `{ text: string, bg: string, borderBg: string }`。
- **逻辑**: 负值用玫瑰色（行 18）；通过 `getLatencyLevel` 获取等级并查 `QUALITY_CONFIG`（导入自 `@/network/constants`）配置（行 19-20）；未找到则返回 muted 默认（行 21-22）。

#### `mergeNetworkQuality(old: NetworkQuality | null, incoming: NetworkQuality)`
- **行号**: 25-36
- **描述**: 合并新旧网络质量数据，处理增量推送场景。
- **逻辑**:
  - 行 26: 新数据 `quality === 'disabled'` 时返回旧数据或新数据。
  - 行 29-33: 新数据 `quality === 'busy'`（后台增量推送）时：无旧数据则强制 `unknown`（行 30）；旧数据也是 `busy` 则保持（行 31）；否则合并 `details` 与 `metrics` 并保留旧质量（行 32）。
  - 行 34-35: 其他情况合并明细与指标，覆盖整体质量。
- **注释**: 行 27-28 说明慢网络下终态迟迟不到，采纳 `busy` 会让胶囊永远显示“检测中”（真机 2026-09-09）。

#### `extractGatewayLatency(nq: NetworkQuality | null)`
- **行号**: 38-43
- **描述**: 提取网关延迟。
- **逻辑**: 优先取 `nq.gatewayLatency`（行 40）；其次取 `nq.details?.['gateway'].latency`（行 41-42）；否则返回 -1。

#### `extractExternalLatency(nq: NetworkQuality | null)`
- **行号**: 45-59
- **描述**: 提取外网延迟。
- **逻辑**:
  - 优先 `averageExternalLatency`（行 47-49）。
  - 其次 `externalLatency`（行 50）。
  - 否则从 `details` 过滤掉 gateway 取正延迟中位数（行 51-57）。
  - 无数据返回 -1（行 58）。

#### `resolveQualityDisplay(nq: NetworkQuality | null)`
- **行号**: 66-76
- **描述**: 统一解析用于展示的延迟和质量等级（首页卡与质量页共用）。
- **逻辑**:
  - 行 67-68 提取网关与外网延迟。
  - 行 69 确定 `displayLatency` 优先外网后网关。
  - 行 70-74 确定 `quality`：若 `displayLatency >= 0` 且原始为 `unknown`/`busy` 则按延迟推断，否则保留原始。
  - 返回 `{ quality, gatewayLatency, externalLatency, displayLatency }`（行 75）。

## Architecture

```text
┌──────────────────────────────────────────────────────┐
│                   Frontend App                       │
│                                                      │
│  ┌─────────────┐   ┌──────────────┐   ┌───────────┐ │
│  │  UI Components│──▶│  i18n Index  │   │ Animations│ │
│  │             │   │ (index.ts)   │   │(animations│ │
│  └─────────────┘   └──────┬───────┘   │ .ts)      │ │
│                           │           └───────────┘ │
│  ┌─────────────┐   ┌──────▼───────┐   ┌───────────┐ │
│  │ Quality Page│──▶│  Latency Lib │   │ Color Lib │ │
│  │ Dashboard   │   │ (latency.ts) │   │ (color.ts)│ │
│  └─────────────┘   └──────────────┘   └───────────┘ │
│                           ▲                          │
│  ┌─────────────┐   ┌──────┴───────┐                 │
│  │ Settings    │──▶│ Easing Config│                 │
│  │ Theme       │   │(easing-config│                 │
│  └─────────────┘   │ .ts)         │                 │
│                    └──────────────┘                 │
└──────────────────────────────────────────────────────┘
```

1. **i18n Core**: `index.ts` 作为入口，加载 `zh.json` 和 `en.json`，并通过 `react-i18next` 暴露给 React 组件。
2. **Animation System**: `animations.ts` 依赖 `easing-config.ts` 提供的缓动曲线，生成 Framer Motion 兼容的变体对象。
3. **Visual Logic**: `color.ts` 提供基础的 HEX 到 HSL 转换，可能被主题系统调用。
4. **Network Intelligence**: `latency.ts` 独立于 UI，处理原始网络质量数据（`NetworkQuality` 类型来自 `@/monitor`）的计算、合并和展示格式化。

## Data Flow

1. **Initialization**:
   - 应用启动时，`i18n/index.ts` 行 8-26 执行 `i18n.init()`。
   - 读取 `localStorage.app-language`（行 17），若不存在则交由 `LanguageDetector` 通过 `navigator` 检测（行 18-19）。
   - 加载对应的 JSON 资源文件（行 12-15）。

2. **Theme & Animation Selection**:
   - UI 组件调用 `getEasingConfig(screenRefreshRate)`（`easing-config.ts` 行 27-29）获取当前屏幕适配的缓动配置。
   - 将配置传入 `createLogEntryVariants`（`animations.ts` 行 3-17）或 `createPanelAppleVariants`（行 22-42）生成本次渲染所需的变体。

3. **Color Conversion**:
   - 当用户选择自定义主题色或系统提供 HEX 色值时，调用 `hexToHsl`（`color.ts` 行 1-33）进行转换，以便在 HSL 空间中进行明暗度调整。

4. **Network Quality Update**:
   - 后端或检测模块推送新的 `NetworkQuality` 对象（类型来自 `@/monitor`）。
   - `mergeNetworkQuality`（`latency.ts` 行 25-36）被调用，将新数据与本地缓存合并，特别处理 `busy` 状态避免 UI 卡死。
   - 展示层调用 `resolveQualityDisplay`（行 66-76），将内部质量状态转换为人类可读的等级和颜色，并提取关键延迟值用于图表或标签显示。

## Connections
- [[frontend-ui-dashboard]]: 仪表盘组件使用 `latency.ts` 中的 `resolveQualityDisplay` 来显示网络质量胶囊。
- [[frontend-ui-settings]]: 设置页面使用 `i18n` 进行标签翻译，并使用 `color.ts` 辅助预览主题色。
- [[frontend-ui-panel]]: 侧边栏面板转场使用 `animations.ts` 中的 `createPanelAppleVariants`。
- [[backend-network-monitor]]: 后端推送的网络质量数据是 `latency.ts` 处理的源头（`NetworkQuality` 类型定义于 `@/monitor`）。

## Known Issues

1. **i18n 初始语言检测**: 在 `index.ts` 行 17，`try-catch` 块捕获了 `localStorage` 访问异常，但若浏览器完全禁用存储，`lng` 将为 `undefined`，导致回退到 `fallbackLng` ('zh')（行 16）。尽管 `detection.order` 包含 `navigator`（行 18-19），手动设置 `lng: undefined` 仍可能使非中文用户首次加载显示中文。

2. **Hex 颜色校验宽松**: `color.ts` 行 1-4 及行 10 对无效 HEX 的处理较为宽松，直接返回默认蓝绿色 `{ h: 230, s: 70, l: 55 }`。若上游传递了恶意或格式错误的颜色字符串，可能会导致意外的 UI 样式。

3. **延迟合并逻辑复杂性**: `latency.ts` 行 25-36 的 `mergeNetworkQuality` 逻辑较为复杂，特别是对于 `busy` 状态的处理（行 27-28 注释提及真机测试日期 "2026-09-09"）。如果后端推送的数据结构发生变化（例如 `details` 或 `metrics` 结构变更），合并逻辑可能需要同步更新，否则可能导致数据丢失或显示不一致。

4. **动画性能**: `animations.ts` 行 22-42 中的弹簧动画参数（`stiffness: 320, damping: 24, mass: 0.7`，行 32）是针对现代设备的优化。在低端设备上，复杂的弹簧动画可能导致帧率下降，但目前没有针对低端设备的降级策略。
