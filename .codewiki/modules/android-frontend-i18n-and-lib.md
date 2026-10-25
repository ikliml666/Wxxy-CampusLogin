---
title: android-frontend-i18n-and-lib
type: module
source_files:
  - android/frontend/src/i18n/index.ts
  - android/frontend/src/i18n/locales/en.json
  - android/frontend/src/lib/latency.ts
tags: [i18n, latency, utils]
---

## Overview

本模块 `android-frontend-i18n-and-lib` 是 Android 前端应用的核心基础库，负责处理国际化资源初始化与加载、以及网络质量数据的标准化提取与展示解析。该模块通过 i18next 提供多语言支持，并通过统一的延迟合并与等级推断策略，确保网络质量指标在前端展示的稳定性和一致性。

## Key Components

### i18n (Internationalization)

#### `android/frontend/src/i18n/index.ts`
**职责**：作为 i18next 的全局初始化入口，配置 React 国际化支持、语言检测策略及资源加载。

*   **关键实现细节**：
    *   `1-3|`：导入核心库 `i18next`、`react-i18next` 插件 `initReactI18next` 及浏览器语言检测器 `LanguageDetector`。
    *   `5-6|`：静态导入中文 (`zh`) 和英文 (`en`) JSON 资源文件（路径 `./locales/zh.json` 与 `./locales/en.json`）。
    *   `8-27|`：链式调用 `i18n.use().init()` 进行配置。
        *   `9|`：注册 `LanguageDetector` 用于自动探测用户语言偏好。
        *   `10|`：注册 `initReactI18next` 使 React 组件能响应翻译变化。
        *   `12-15|`：**资源配置**：将导入的 JSON 对象挂载到 `zh` 和 `en` key 下，结构为 `{ translation: json }`。
        *   `16|`：**默认回退语言**：`fallbackLng: 'zh'`。当请求的语言包缺失时，默认使用中文。
        *   `17|`：**初始语言获取逻辑**：通过 IIFE 尝试从 `localStorage.getItem('app-language')` 读取缓存语言。包含 `try-catch` 以应对 `localStorage` 不可用（如隐私模式）的情况，失败则返回 `undefined` 交由后续检测器处理。
        *   `18-22|`：**检测器配置**：
            *   `order`：优先检查 `localStorage`，其次检查 `navigator`。
            *   `lookupLocalStorage`：指定存储键名为 `'app-language'`。
            *   `caches`：仅使用 `localStorage` 缓存检测结果。
        *   `23-25|`：**插值配置**：`escapeValue: false`。禁止 i18next 自动转义 HTML 字符，允许在 React 组件中直接使用渲染带 HTML 的内容。
    *   `28|`：**导出**：导出已初始化的 `i18n` 单例实例。

#### `android/frontend/src/i18n/locales/en.json`
**职责**：提供应用程序的所有英文界面文本，作为源语言定义所有可翻译键的路径。

*   **根节点命名空间（1-895|）**：
    *   `notify` (2-19|)：系统通知、自动登录/退出提示、版本更新提示，含 `{{seconds}}`、`{{msg}}` 等模板变量。
    *   `common` (20-43|)：通用按钮（Save、Cancel、Retry）、状态词、时间单位。
    *   `nav` (44-55|)：底部导航栏标签（Home、Account、Settings 等）。
    *   `titlebar` (56-72|)：顶部标题栏操作与主题切换、语言切换。
    *   `auth` (73-98|)：认证流程文本、登录状态、配置保存失败。
    *   `account` (99-263|)：账户管理、ISP 绑定、自助服务面板、生物识别验证、设备别名。
    *   `network` (264-341|)：网络适配器、DNS 优化（Alibaba/Tencent DoH）、DHCP、夜间外网切换、Avoid Bad Wi-Fi 引导；其中 `avoidBadWifiCommand` (329|) 与 `avoidBadWifiGrantCommand` (330|) 为硬编码 adb 命令。
    *   `monitor` (342-409|)：网络状态检测、校园网校验逻辑、延迟时间线详解。
    *   `quality` (410-475|)：网络质量测试、延迟详情、目标服务器列表（`names` 454-474| 涵盖网关、DNS、网站、游戏等）、质量评级。
    *   `speedtest` (476-502|)：测速网站推荐。
    *   `settings` (503-608|)：外观、启动行为、安全设置、日志保留；含 `isp` 子映射（601-606|）。
    *   `log` (609-646|)：日志查看器、搜索、保留策略。
    *   `onboarding` (647-708|)：新手引导流程。
    *   `updatePrompt` (709-714|)：新版本弹窗。
    *   `about` (715-757|)：关于页面、更新检查、下载。
    *   `statusbar` (758-770|)：状态栏快捷操作与在线运营商徽标。
    *   `dashboard` (771-808|)：仪表盘卡片。
    *   `confirmDialog` (809-812|)：通用确认框。
    *   `panel` (813-832|)：侧边栏/面板描述。
    *   `themeDialog` (833-848|)：主题设置弹窗，含 `glassLevel` 玻璃材质级别。
    *   `rightPanel` (849-853|)：右侧运行日志面板。
    *   `sponsor` (854-859|)：赞助二维码。
    *   `mobile` (860-869|)：移动端视图文本。
    *   `face` (870-894|)：2D 人脸识别文本。
*   **模板变量**：广泛使用 `{{variable}}` 语法动态插入数据。
*   **特殊硬编码**：`network.avoidBadWifiCommand` (329|) 与 `network.avoidBadWifiGrantCommand` (330|) 为 ADB 命令字符串。

### Latency & Quality Processing

#### `android/frontend/src/lib/latency.ts`
**职责**：网络质量数据的标准化、合并、提取与展示解析，统一前端对后端推送指标的处理。

*   **类型定义**：
    *   `LatencyLevel` (4|)：联合类型 `'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad'`。
    *   `LatencyType` (5|，导出)：联合类型 `'gateway' | 'external'`。
*   **公开函数**：
    *   `getLatencyLevel(latency: number): LatencyLevel` (7-15|)：阈值映射。`<0` 返回 `'bad'`；`<=20` excellent；`<=50` great；`<=100` good；`<=200` fair；`<=400` poor；其余 bad。
    *   `getLatencyColor(latency: number)` (17-23|)：基于等级查 `QUALITY_CONFIG` 返回 `{ text, bg, borderBg }` 类名；`<0` 直接返回 rose 色调；未知等级兜底 muted。
    *   `mergeNetworkQuality(old: NetworkQuality | null, incoming: NetworkQuality): NetworkQuality` (25-36|)：合并策略。
        *   若 `incoming.quality === 'disabled'` 返回 `old ?? incoming`（26|）。
        *   若 `incoming.quality === 'busy'`（29-33|）：旧空则返回 `{ ...incoming, quality: 'unknown' }`；旧 busy 则返回 busy；否则保留旧 quality，合并 `details` 与 `metrics`。注释（27-28|）说明为避免慢网络下胶囊卡在“检测中”。
        *   正常情况（34-35|）：旧空或旧 unknown 则返回 incoming，否则合并 details/metrics 并覆盖 quality。
    *   `extractGatewayLatency(nq: NetworkQuality | null): number` (38-43|)：优先 `nq.gatewayLatency`，否则 `details['gateway'].latency`，无则返回 `-1`。
    *   `extractExternalLatency(nq: NetworkQuality | null): number` (45-59|)：优先 `averageExternalLatency`（需 `>=0`），其次 `externalLatency`，再从 `details` 过滤非 gateway 取中位数（51-56|），无则返回 `-1`。
    *   `resolveQualityDisplay(nq: NetworkQuality | null)` (66-76|)：计算 `displayLatency`（外部优先于网关），若 `displayLatency >= 0` 且原始 quality 为 unknown/busy 则推断等级（71-74|），返回 `{ quality, gatewayLatency, externalLatency, displayLatency }`。

## Architecture

```text
┌────────────────────────────┐   ┌────────────────────────────┐
│ i18n/index.ts              │   │ i18n/locales/en.json       │
│ (i18n 初始化与导出单例)     │   │ (英文语言包资源)            │
└───────────┬────────────────┘   └───────────▲────────────────┘
            │ 导入资源                        │ 被静态导入
            └───────────────┬────────────────┘
                            │
              ┌─────────────▼─────────────┐
              │ lib/latency.ts             │
              │ (质量数据合并/提取/展示解析) │
              └─────────────▲─────────────┘
                            │ 依赖类型与配置
              ┌─────────────┴─────────────┐
              │ @/monitor (NetworkQuality) │
              │ @/network/constants (CFG)  │
              └────────────────────────────┘
```

## Data Flow

1. **i18n 初始化流**：
    * `index.ts` 在应用启动早期执行，同步导入 `en.json` 与 `zh.json`（`5-6|`）。
    * 调用 `i18n.use(LanguageDetector).use(initReactI18next).init(...)`（`8-27|`）注册检测器与 React 绑定，配置资源与回退语言。
    * React 组件通过 `useTranslation()` 访问 `t`，按当前 `lng`（17| 逻辑）查找对应 JSON 值。
    * 语言切换时 `react-i18next` 触发重渲染。

2. **网络质量数据处理流**：
    * 输入：`NetworkQuality` 对象（来自 `@/monitor`）。
    * `latency.ts` 函数 `mergeNetworkQuality` 合并增量推送，`extractGatewayLatency`/`extractExternalLatency` 提取数值，`getLatencyLevel`/`getLatencyColor` 映射等级与颜色。
    * `resolveQualityDisplay` 输出展示用 quality 与延迟值，供首页卡片、质量页胶囊消费。

## Connections

* [[android-frontend-monitor]]：提供 `NetworkQuality` 类型给 `latency.ts`（1| 导入）。
* [[android-frontend-network]]：提供 `QUALITY_CONFIG` 常量给 `latency.ts`（2| 导入）。
* [[android-frontend-settings]]：用户可能通过配置影响语言与质量检测参数，间接使用本模块。

## Known Issues

* **慢网络下 busy 状态处理**：如 `latency.ts` 27-28| 注释所述，后端增量推送期间 `quality` 长期为 `busy`/`unknown`，若直接采纳会导致 UI 永远显示“检测中”；因此 `mergeNetworkQuality` 在 busy 时不采纳 incoming 的 quality 字段（29-33|），`resolveQualityDisplay` 在 unknown/busy 时按延迟推断等级（71-74|）。
* **localStorage 不可用回退**：`index.ts` 17| 在读取 `app-language` 时使用 `try-catch`，隐私模式或禁用存储时返回 `undefined`，交由 `LanguageDetector` 的 `navigator` 顺序探测。
* 除上述外，当前源码未包含其他独立已知缺陷。
