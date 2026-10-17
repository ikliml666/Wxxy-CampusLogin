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
tags: [i18n, localization, animations, colors, easing, latency, utilities]
---

## Overview
模块 `frontend-lib-i18n` 是前端应用的核心基础库集合，负责国际化支持、视觉动画配置、颜色转换以及网络延迟计算。它不直接处理 UI 渲染，而是为上层组件提供标准化的数据格式和工具函数。该模块涵盖了从语言包加载、主题色解析到复杂网络质量评估的多个底层逻辑。

## Key Components

### Internationalization (i18n)
**文件**: `android/frontend/src/i18n/index.ts`

#### `init()`
- **行号**: 9-25
- **描述**: 初始化 `i18next` 实例。
- **配置细节**:
  - 使用 `LanguageDetector` 检测用户语言，优先顺序为 `localStorage` 然后 `navigator`。
  - 存储键名为 `app-language`。
  - 资源包含 `zh` 和 `en` 两个语言包，分别导入自 `./locales/zh.json` 和 `./locales/en.json`。
  - 后备语言 (`fallbackLng`) 设置为 `'zh'`。
  - 插值转义关闭 (`escapeValue: false`)，允许在翻译字符串中使用 HTML 或特殊字符（需确保数据安全）。
  - 尝试从 `localStorage` 获取当前语言设置，若失败则返回 `undefined`。

**文件**: `android/frontend/src/i18n/locales/en.json`
- **结构**: JSON 对象，嵌套结构包含以下顶级键：
  - `notify`: 通知相关文本。
  - `common`: 通用按钮和状态文本。
  - `nav`: 导航栏标签。
  - `titlebar`: 标题栏功能项。
  - `auth`: 认证登录流程文本。
  - `account`: 账号管理、自助服务、生物识别等。
  - `network`: 网络适配器、DNS优化、夜间切换等。
  - `monitor`: 网络状态监测逻辑和结果。
  - `quality`: 网络质量检测详情和解释。
  - `speedtest`: 测速网站列表和提示。
  - `settings`: 外观、启动、保活、安全等设置项。
  - `log`: 系统日志查看和管理。
  - `onboarding`: 新手引导流程。
  - `updatePrompt`: 更新提示。
  - `about`: 关于页面信息。
  - `statusbar`: 状态栏显示内容。
  - `dashboard`: 仪表盘卡片和快捷操作。
  - `confirmDialog`: 确认对话框。
  - `panel`: 面板标题和描述。
  - `themeDialog`: 主题设置对话框。
  - `rightPanel`: 右侧面板日志。
  - `sponsor`: 赞助信息。
  - `mobile`: 移动端特定标签。
  - `face`: 人脸录入和验证界面。

### Animation Utilities
**文件**: `android/frontend/src/lib/animations.ts`

#### `createLogEntryVariants(easing: EasingConfig)`
- **行号**: 3-16
- **描述**: 为日志条目创建 Framer Motion 变体。
- **参数**: `easing` - 缓动配置对象。
- **返回值**: 包含 `initial`, `animate`, `exit` 属性的对象。
  - `initial`: 透明度 0，向右偏移 20px。
  - `animate`: 透明度 1，归位，使用 `snappy` 缓动，时长 0.3s。
  - `exit`: 透明度 0，向左偏移 -16px，使用 `exit` 缓动，时长 0.15s。

#### `createPanelAppleVariants(easing: EasingConfig)`
- **行号**: 24-47
- **描述**: 创建类似 Apple 风格的侧边栏/面板转场变体。
- **参数**: `easing` - 缓动配置对象。
- **返回值**: 包含 `initial`, `animate`, `exit` 属性的对象，这些属性是函数，接收 `direction` (-1 | 0 | 1)。
  - `initial(direction)`:
    - 方向 0 (纵向): Y 轴偏移 8px，透明度 0.9。
    - 方向 ±1 (横向): X 轴偏移 18 * direction，透明度 0.9。
  - `animate()`: 归位至原点，透明度 1。使用弹簧动画 (stiffness: 320, damping: 24, mass: 0.7)，模拟自然弹性。
  - `exit(direction)`:
    - 方向 0: Y 轴偏移 -4px。
    - 方向 ±1: X 轴偏移 -12 * direction。
    - 透明度 0.9，缩放 0.99。
    - 使用 `exit` 缓动，时长极短 (0.04s) 以减少过渡税。

### Color Utilities
**文件**: `android/frontend/src/lib/color.ts`

#### `hexToHsl(hex: string)`
- **行号**: 1-36
- **描述**: 将 HEX 颜色代码转换为 HSL 对象。
- **参数**: `hex` - 十六进制颜色字符串（如 `#RRGGBB` 或 `#RGB`）。
- **返回值**: `{ h: number; s: number; l: number }`。
- **错误处理**: 如果输入无效，返回默认值 `{ h: 230, s: 70, l: 55 }`。
- **实现细节**:
  - 行 1-2: 检查字符串类型和长度。
  - 行 3: 正则匹配 HEX 格式。
  - 行 5-7: 解析 RGB 分量并归一化到 0-1。
  - 行 9: 检查 NaN。
  - 行 11-12: 计算最大值、最小值和亮度 L。
  - 行 14-22: 计算饱和度 S 和色相 H，根据最大分量决定色相公式。
  - 行 24-28: 返回四舍五入后的整数 H, S, L。

### Easing Configuration
**文件**: `android/frontend/src/lib/easing-config.ts`

#### Interface `EasingConfig`
- **行号**: 3-10
- **描述**: 定义缓动曲线的贝塞尔控制点数组 `[x1, y1, x2, y2]`。
- **字段**:
  - `enter`: 进入动画，主用面板转场。
  - `exit`: 退出动画，干脆利落。
  - `smooth`: 平滑过渡，用于卡片/列表。
  - `snappy`: 快速响应，用于日志/徽章。
  - `overshoot`: 弹性效果。

#### Constants
- **`EASING_60HZ`** (行 12-19): 针对 60Hz 屏幕优化的缓动配置。
- **`EASING_120HZ`** (行 21-28): 针对 120Hz 高刷屏幕优化的缓动配置，曲线更紧凑以适配更高刷新率。

#### `getEasingConfig(refreshRate: number)`
- **行号**: 30-32
- **描述**: 根据屏幕刷新率返回对应的缓动配置。
- **逻辑**: 如果 `refreshRate >= 120`，返回 `EASING_120HZ`，否则返回 `EASING_60HZ`。

### Latency Calculation
**文件**: `android/frontend/src/lib/latency.ts`

#### Type Aliases
- **`LatencyLevel`** (行 4): `'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad'`。
- **`LatencyType`** (行 5): `'gateway' | 'external'`。

#### `getLatencyLevel(latency: number)`
- **行号**: 7-14
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
- **行号**: 16-22
- **描述**: 根据延迟返回对应的 Tailwind CSS 类名对象。
- **返回值**: `{ text: string, bg: string, borderBg: string }`。
- **逻辑**:
  - 负值使用玫瑰色 (`rose-500`)。
  - 通过 `getLatencyLevel` 获取等级，查找 `QUALITY_CONFIG` 中的颜色配置。
  - 如果未找到配置，返回默认 muted 颜色。

#### `mergeNetworkQuality(old: NetworkQuality | null, incoming: NetworkQuality)`
- **行号**: 24-36
- **描述**: 合并新旧网络质量数据，处理增量推送场景。
- **逻辑**:
  - 如果新数据质量为 `disabled`，保留旧数据（如果有）或返回新数据。
  - 如果新数据质量为 `busy` (后台增量推送):
    - 无旧数据：返回新数据，但强制质量设为 `unknown` 避免永远显示“检测中”。
    - 旧数据也是 `busy`：返回新数据，保持 `busy`。
    - 旧数据非 `busy`：合并明细 (`details`) 和指标 (`metrics`)，保留旧数据的整体质量状态。
  - 其他情况：合并明细和指标，覆盖整体质量。
- **注释说明**: 慢网络下终态迟迟不到，采纳 `busy` 会让胶囊永远显示"检测中"，因此在此逻辑中特殊处理。

#### `extractGatewayLatency(nq: NetworkQuality | null)`
- **行号**: 38-42
- **描述**: 提取网关延迟。
- **逻辑**: 优先取 `nq.gatewayLatency`，其次取 `details['gateway'].latency`。如果都不可用或为负，返回 -1。

#### `extractExternalLatency(nq: NetworkQuality | null)`
- **行号**: 44-55
- **描述**: 提取外网延迟。
- **逻辑**:
  - 优先取 `averageExternalLatency`。
  - 其次取 `externalLatency`。
  - 最后从 `details` 中过滤掉 gateway，收集所有正延迟值，排序后取中位数。
  - 如果无数据，返回 -1。

#### `resolveQualityDisplay(nq: NetworkQuality | null)`
- **行号**: 65-76
- **描述**: 统一解析用于展示的延迟和质量等级。
- **逻辑**:
  - 提取网关和外网延迟。
  - 确定 `displayLatency`：优先外网，其次网关。
  - 确定 `quality`：
    - 如果 `displayLatency >= 0` 且原始质量为 `unknown` 或 `busy`，则根据延迟推断等级。
    - 否则保留原始质量状态。
  - 返回包含 `quality`, `gatewayLatency`, `externalLatency`, `displayLatency` 的对象。

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
4. **Network Intelligence**: `latency.ts` 独立于 UI，处理原始网络质量数据 (`NetworkQuality`) 的计算、合并和展示格式化。

## Data Flow

1. **Initialization**:
   - 应用启动时，`i18n/index.ts` 执行 `i18n.init()`。
   - 读取 `localStorage.app-language`，若不存在则通过 `navigator.language` 检测。
   - 加载对应的 JSON 资源文件。

2. **Theme & Animation Selection**:
   - 当需要渲染动画时，UI 组件调用 `getEasingConfig(screenRefreshRate)` 获取当前屏幕适配的缓动配置。
   - 将配置传入 `createLogEntryVariants` 或 `createPanelAppleVariants` 生成本次渲染所需的变体。

3. **Color Conversion**:
   - 当用户选择自定义主题色或系统提供 HEX 色值时，调用 `hexToHsl` 进行转换，以便在 HSL 空间中进行明暗度调整或生成互补色。

4. **Network Quality Update**:
   - 后端或检测模块推送新的 `NetworkQuality` 对象。
   - `mergeNetworkQuality` 被调用，将新数据与本地缓存的旧数据合并，特别处理 `busy` 状态以避免 UI 卡死。
   - 展示层调用 `resolveQualityDisplay`，将内部质量状态转换为人类可读的等级和颜色，并提取关键延迟值用于图表或标签显示。

## Connections

- [[frontend-ui-dashboard]]: 仪表盘组件使用 `latency.ts` 中的 `resolveQualityDisplay` 来显示网络质量胶囊。
- [[frontend-ui-settings]]: 设置页面使用 `i18n` 进行标签翻译，并使用 `color.ts` 辅助预览主题色。
- [[frontend-ui-panel]]: 侧边栏面板转场使用 `animations.ts` 中的 `createPanelAppleVariants`。
- [[backend-network-monitor]]: 后端推送的网络质量数据是 `latency.ts` 处理的源头。

## Known Issues

1. **i18n 初始语言检测**: 在 `index.ts` 第 16 行，`try-catch` 块捕获了 `localStorage` 访问异常，但如果浏览器完全禁用存储，`lng` 将为 `undefined`，导致回退到 `fallbackLng` ('zh')。这可能不符合某些英文用户的预期，尽管 `LanguageDetector` 会再次尝试检测 `navigator`。
2. **Hex 颜色校验宽松**: `color.ts` 第 1-2 行和第 5 行对无效 HEX 的处理较为宽松，直接返回默认蓝色。在生产环境中，如果上游传递了恶意或格式错误的颜色字符串，可能会导致意外的 UI 样式。
3. **延迟合并逻辑复杂性**: `latency.ts` 第 24-36 行的 `mergeNetworkQuality` 逻辑较为复杂，特别是对于 `busy` 状态的处理。如果后端推送的数据结构发生变化（例如 `details` 或 `metrics` 的结构变更），合并逻辑可能需要同步更新，否则可能导致数据丢失或显示不一致。注释中提到真机测试日期 "2026-09-09"，暗示这是一个经过长期测试的逻辑，但未来重构时需特别注意。
4. **动画性能**: `animations.ts` 中的弹簧动画参数 (stiffness: 320, damping: 24) 是针对现代设备的优化。在低端设备上，复杂的弹簧动画可能导致帧率下降，但目前没有针对低端设备的降级策略。