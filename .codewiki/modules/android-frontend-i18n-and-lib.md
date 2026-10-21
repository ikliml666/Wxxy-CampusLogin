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

本模块 `android-frontend-i18n-and-lib` 是 Android 前端应用的核心基础库，负责处理国际化资源加载、网络质量数据标准化。该模块通过标准化的延迟提取与合并策略，确保了多语言环境的稳定性和跨平台兼容性。

## Key Components

### i18n (Internationalization)

#### `android/frontend/src/i18n/index.ts`
**职责**：作为 i18next 的全局初始化入口，配置 React 国际化支持、语言检测策略及资源加载。

*   **关键实现细节**：
    *   `1-3|`：导入核心库 `i18next`、`react-i18next` 插件 `initReactI18next` 及浏览器语言检测器 `LanguageDetector`。
    *   `5-6|`：静态导入中文 (`zh`) 和英文 (`en`) JSON 资源文件。
    *   `8-26|`：链式调用 `i18n.use().init()` 进行配置。
        *   `9|`：注册 `LanguageDetector` 用于自动探测用户语言偏好。
        *   `10|`：注册 `initReactI18next` 使 React 组件能响应翻译变化。
        *   `12-15|`：**资源配置**：将导入的 JSON 对象挂载到 `zh` 和 `en` key 下，结构为 `{ translation: json }`。
        *   `16|`：**默认回退语言**：`fallbackLng: 'zh'`。当请求的语言包缺失时，默认使用中文。
        *   `17|`：**初始语言获取逻辑**：通过 IIFE `( () => { ... } )()` 尝试从 `localStorage.getItem('app-language')` 读取缓存语言。包含 `try-catch` 以应对 `localStorage` 不可用（如隐私模式）的情况，失败则返回 `undefined` 交由后续检测器处理。
        *   `18-22|`：**检测器配置**：
            *   `order`：优先检查 `localStorage`，其次检查 `navigator`。
            *   `lookupLocalStorage`：指定存储键名为 `'app-language'`。
            *   `caches`：仅使用 `localStorage` 缓存检测结果，避免写入 session storage 或 cookie。
        *   `23-25|`：**插值配置**：`escapeValue: false`。禁止 i18next 自动转义 HTML 字符，允许在 React 组件中直接使用 `<Trans>` 或 `t()` 渲染带 HTML 的内容（需确保内容安全）。
    *   `28|`：**导出**：导出已初始化的 `i18n` 单例实例。其他模块应直接 import 此实例或使用 `react-i18next` 提供的 hooks，而非重新初始化。

#### `android/frontend/src/i18n/locales/en.json`
**职责**：提供应用程序的所有英文界面文本。该文件作为“源语言”或主要参考语言，其结构定义了所有可翻译键的路径。

*   **关键数据结构**：
    *   **根节点**：包含以下命名空间（Namespaces），每个命名空间对应应用的一个功能模块或页面：
        *   `notify`: 系统通知、自动登录/退出提示、版本更新提示。
        *   `common`: 通用按钮（Save, Cancel, Retry 等）、状态词（Error, Warning）、时间单位。
        *   `nav`: 底部导航栏标签（Home, Account, Settings 等）。
        *   `titlebar`: 顶部标题栏操作（Minimize, Maximize, Exit）、主题切换提示。
        *   `auth`: 认证流程相关文本（Login, Logout, Network Status, Config Save Failures）。
        *   `account`: 账户管理、ISP 绑定、自助服务面板（Self-Service）、生物识别验证、设备别名设置。
        *   `network`: 网络适配器管理、DNS 优化（Alibaba/Tencent DoH）、DHCP 刷新、夜间外网切换（Night Outbound Switch）、Android 系统权限引导（Avoid Bad Wi-Fi）。
        *   `monitor`: 网络状态检测服务、校园网校验逻辑（SSID/IP/Gateway）、定时任务、延迟时间线详解（UDP/DNS/TCP/TLS/TTFB/Content）。
        *   `quality`: 网络质量测试、延迟详情、目标服务器列表（Gateway, AliDNS, TencentDNS, Bilibili, LoL 等）、质量评级（Excellent 到 Congested）。
        *   `speedtest`: 测速网站推荐列表（SpeedTest.cn, Ookla, USTC, NEU 等）及说明。
        *   `settings`: 外观（Color Scheme, Glass Level）、启动行为（Auto-launch, Keep-Alive）、安全设置（Biometric, 2D Face）、日志保留策略。
        *   `log`: 日志查看器控件、搜索、过滤、清除确认。
        *   `onboarding`: 新手引导流程（Welcome, Account Info, Bind Operator, Setup Complete）。
        *   `updatePrompt`: 新版本弹窗。
        *   `about`: 关于页面、更新检查、下载进度、赞助信息。
        *   `statusbar`: 状态栏快捷操作。
        *   `dashboard`: 仪表盘卡片配置。
        *   `confirmDialog`: 通用确认框。
        *   `panel`: 侧边栏/面板描述。
        *   `themeDialog`: 主题设置弹窗。
        *   `rightPanel`: 右侧运行日志面板。
        *   `sponsor`: 赞助二维码提示。
        *   `mobile`: 移动端特定视图文本。
        *   `face`: 2D 人脸识别专用文本（Enroll, Verify, Challenge Blink/Turn）。
    *   **模板变量**：大量使用 `{{variable}}` 语法，如 `{{version}}`, `{{seconds}}`, `{{msg}}`, `{{field}}`, `{{names}}` 等，用于动态插入运行时数据。
    *   **特殊内容**：
        *   `network.avoidBadWifiCommand` (行 329): `adb shell settings put global network_avoid_bad_wifi 1`。硬编码的系统命令字符串。
        *   `network.avoidBadWifiGrantCommand` (行 330): `adb shell pm grant com.campuslogin.client android.permission.WRITE_SECURE_SETTINGS`。硬编码的 ADB 授权命令。
        *   `quality.names` (行 454-473): 预定义的测试目标名称映射，涵盖网关、DNS、国内主流网站、视频平台、游戏服务器。

### Latency & Quality Processing

#### `android/frontend/src/lib/latency.ts`
**职责**：网络质量数据的标准化、合并、提取与展示解析。统一前端对后端推送的网络指标的处理逻辑。

*   **类型定义**：
    *   `LatencyLevel`: 'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad'。
    *   `LatencyType`: 'gateway' | 'external'。
*   **阈值映射** (`getLatencyLevel`, `line 7-15`)：
    *   `<0`: bad (异常)。
    *   `<=20`: excellent; `<=50`: great; `<=100`: good; `<=200`: fair; `<=400`: poor; `>400`: bad。
*   **颜色生成** (`getLatencyColor`, `line 17-23`)：
    *   基于 `LatencyLevel` 查找 `QUALITY_CONFIG`。
    *   兜底：未知等级返回 muted 色调。
*   **数据合并策略** (`mergeNetworkQuality`, `line 25-36`)：
    *   **Disabled**：直接返回旧值或新值（若旧为空）。
    *   **Busy (增量推送)**：
        *   若旧值为 busy，保持 busy，合并 details/metrics。
        *   若旧值非 busy，保留旧的 quality 等级（防止慢网络下终态迟迟不到导致 UI 卡在 "检测中"），合并 details/metrics。
        *   **关键决策**：`busy` 状态下不采纳 incoming 的 quality 字段，避免 UI 状态震荡。
    *   **Normal**：合并 details 和 metrics，覆盖 quality。
*   **延迟提取**：
    *   `extractGatewayLatency` (`line 38-43`)：优先取 `nq.gatewayLatency`，其次取 `details['gateway'].latency`。
    *   `extractExternalLatency` (`line 45-59`)：
        *   优先 `averageExternalLatency`。
        *   其次 `externalLatency`。
        *   最后从 `details` 中过滤非 gateway 项，取中位数 (`Math.floor(length / 2)`)。
*   **展示解析** (`resolveQualityDisplay`, `line 66-76`)：
    *   计算 `displayLatency`：优先外部延迟，否则网关延迟。
    *   **展示门槛**：若 `displayLatency >= 0` 且当前 quality 为 'unknown'/'busy'，则根据延迟值推断真实等级；否则保留原始 status。确保胶囊在后台检测期间也能显示有意义的延迟评级。

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Frontend Application                     │
├──────────────┬──────────────┬──────────────┬────────────────┤
│  i18n Module │ Visual Layer │ System Utils │ Animation Core │
├──────────────┼──────────────┼──────────────┼────────────────┤
│ index.ts     │ glass-dock   │ latency.ts   │ animations.ts  │
│ (Init/Store) │ (Hyalite/Hook)│ (Merge/Color)│ (Variants)     │
│              ├──────────────┼──────────────┼────────────────┤
│ en.json      │ dock-lens    │ notifPerm    │ color.ts       │
│ zh.json      │ (DOM Lens)   │ (Req/Retry)  │ (Hex->Hsl)     │
└──────────────┴──────────────┴──────────────┴────────────────┘
         ▲                ▲               ▲                ▲
         │                │               │                │
┌────────┴────────┐ ┌────┴────────┐ ┌─────┴────────┐ ┌────┴────────┐
│ easing-config   │ │ dock-spring │ │ renderLiveness│ │ utils.ts    │
│ (Easing Config) │ │ (Physics    │ │ (rAF Probe)   │ │ (cn/Error/  │
│                 │ │  Kernel)    │ │               │ │  Storage)   │
└─────────────────┘ └─────────────┘ └───────────────┘ └─────────────┘
```

## Data Flow

1.  **i18n 初始化流**：
    *   `index.ts` 在应用启动早期执行。
    *   `index.ts` 同步导入 `en.json` 和 `zh.json`。
    *   `i18n.init()` 被调用，将 JSON 内容注册到 i18next 内部 store。
    *   React 组件通过 `useTranslation()` hook 或 `withTranslation()` HOC 访问翻译函数 `t`。
    *   当 UI 需要显示文本时，调用 `t('key.path', { interpolationData })`。
    *   i18next 根据当前激活的语言（由 `index.ts` 中的 `lng` 决定）查找对应的 JSON 值。
    *   若发生语言切换（如用户在设置中更改），`react-i18next` 会触发重渲染，所有使用该 hook 的组件更新文本。

2.  **网络质量数据处理流**：
    *   输入：`NetworkQuality` 对象 (来自 `@/monitor`)。
    *   依赖：`@/network/constants` 中的 `QUALITY_CONFIG`。
    *   输出：标准化的 Latency 数值、颜色类名、展示用 Quality Level。
    *   消费者：首页卡片、网络质量详情页、UI 胶囊组件。

3.  **悬浮栏镜头交互流**：
    *   `useDockLens` Hook 挂载，监听指针事件 (`pointerdown`, `pointermove`, `pointerup`)。
    *   `dock-spring.ts` 的 `Motion` 实例驱动位置、抬起、拉伸、果冻、光斑的动画。
    *   `dock-lens.ts` 的 `render` 回调直接修改 DOM style，实现零 React re-render 的高性能动画。
    *   最终通过 CSS 变量和 clip-path 实现视觉效果。

## Connections

*   [[android-frontend-monitor]]: 提供网络质量数据给 `latency.ts`。
*   [[android-frontend-network]]: 提供网络适配器状态和配置给 `latency.ts` 和 `notificationPermission.ts`。
*   [[android-frontend-ui-components]]: 使用 `animations.ts` 和 `color.ts` 进行 UI 渲染。
*   [[android-frontend-settings]]: 用户在此模块中修改语言、主题、通知权限等设置。

## Known Issues

*   **Chromium Backdrop Filter Black Screen**: 在 `glass-dock.tsx` 中，如果祖先链存在 `opacity < 1`、`transform` 或 `will-change`，`backdrop-filter` 可能采样不到内容变黑。解决方案是等待祖先链完全无变换且不透明后再挂载 Hyalite。
*   **localStorage QuotaExceededError**: 在 `index.ts` 和 `utils.ts` 中，如果 `localStorage` 写入失败，代码会捕获异常并回退到内存 Map 或系统语言检测，确保应用不崩溃。
*   **Android Notification Permission Variance**: 在 `notificationPermission.ts` 中，Android 13+ 走系统标准弹框，13 以下及国产 ROM 可能无弹框，需依赖 `openSettingsIfDenied` 引导用户手动开启。
*   **Render Loop Stall False Positives**: 在 `renderLiveness.ts` 中，如果浏览器节流 rAF，可能导致误判停滞。解决方案是检查 `document.hidden`，若在隐藏态则直接返回 true。
*   **Hyalite Engine Compatibility**: 仅 Chromium 有效。WebKit 降级为 blur，Gecko 不渲染。CSS 侧通过 `var(--hyalite, ...)` 自动降级。