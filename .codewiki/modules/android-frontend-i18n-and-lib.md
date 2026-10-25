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

本模块 `android-frontend-i18n-and-lib` 是 Android 前端应用的基础支撑库，包含两部分核心能力：一是基于 i18next 的国际化初始化与英文语言资源定义；二是网络质量延迟数据的标准化处理工具（位于 `lib/latency.ts`）。该模块为前端各页面提供多语言文本以及统一的延迟等级计算、颜色映射、质量数据合并与展示解析逻辑，确保 UI 在后台增量推送场景下仍能显示合理的网络状态。

## Key Components

### i18n (Internationalization)

#### `android/frontend/src/i18n/index.ts`

**职责**：作为 i18next 的全局初始化入口，配置 React 国际化支持、语言检测策略及资源加载，并导出单例 `i18n` 实例。

*   **关键实现细节（行号以源码前缀为准）**：
    *   `1-3|`：导入核心库 `i18next`、React 绑定插件 `initReactI18next` 及浏览器语言检测器 `LanguageDetector`。
    *   `5-6|`：静态导入中文（`zh`）与英文（`en`）JSON 资源文件（路径 `./locales/zh.json`、`../locales/en.json` 相对于本文件）。
    *   `8-26|`：链式调用 `i18n.use().use().init()` 进行配置。
        *   `9|`：注册 `LanguageDetector` 用于自动探测用户语言偏好。
        *   `10|`：注册 `initReactI18next` 使 React 组件能响应翻译变化。
        *   `12-15|`：**资源配置**：将导入的 JSON 对象挂载到 `zh` 和 `en` 键下，结构为 `{ translation: json }`。
        *   `16|`：**默认回退语言**：`fallbackLng: 'zh'`，当请求语言包缺失时回退中文。
        *   `17|`：**初始语言获取**：通过 IIFE 尝试从 `localStorage.getItem('app-language')` 读取缓存语言，包含 `try-catch` 应对隐私模式等不可用场景，失败返回 `undefined` 交由检测器处理。
        *   `18-22|`：**检测器配置**：`order: ['localStorage', 'navigator']`；`lookupLocalStorage: 'app-language'`；`caches: ['localStorage']` 仅用 localStorage 缓存。
        *   `23-25|`：**插值配置**：`escapeValue: false`，禁止 i18next 自动转义 HTML 字符。
    *   `28|`：**导出**：`export default i18n` 单例，其他模块应直接 import 此实例或使用 `react-i18next` 的 hooks。

#### `android/frontend/src/i18n/locales/en.json`

**职责**：提供应用程序全部英文界面文本，作为源语言定义所有可翻译键的路径与模板变量。

*   **根节点命名空间及行号区间**（每个对应一个功能模块）：
    *   `notify` `2-19|`：系统通知、自动登录/退出/版本更新提示。
    *   `common` `20-43|`：通用按钮（Add/Delete/Save 等）、状态词、时间单位。
    *   `nav` `44-55|`：底部导航栏标签（Home/Account/Settings 等）。
    *   `titlebar` `56-72|`：标题栏操作（最小化/最大化/退出、主题切换、语言切换）。
    *   `auth` `73-98|`：认证流程文本（登录/登出/状态/配置保存失败）。
    *   `account` `99-286|`：账户管理、ISP 绑定、自助服务面板、生物识别、设备别名、VPN 凭证等。
    *   `network` `287-364|`：网络适配器、DNS 优化（阿里/腾讯 DoH）、DHCP、夜间外网切换、Avoid Bad Wi-Fi 引导。
    *   `monitor` `365-432|`：网络状态检测、校园网校验逻辑、定时任务、延迟时间线详解。
    *   `quality` `433-498|`：网络质量测试、延迟详情、目标服务器名称映射（`names` `477-497|`）、评级文本。
    *   `speedtest` `499-525|`：测速网站推荐列表及说明。
    *   `settings` `526-631|`：外观、启动行为、安全、日志保留、ISP 枚举（`isp` `624-629|`）。
    *   `log` `632-669|`：日志查看器、搜索、过滤、保留策略。
    *   `onboarding` `670-731|`：新手引导流程。
    *   `updatePrompt` `732-737|`：新版本弹窗。
    *   `about` `738-780|`：关于页、更新检查、下载进度、赞助。
    *   `statusbar` `781-793|`：状态栏快捷操作与在线运营商徽标。
    *   `dashboard` `794-831|`：仪表盘卡片。
    *   `confirmDialog` `832-835|`：通用确认框。
    *   `panel` `836-855|`：侧边栏/面板描述。
    *   `themeDialog` `856-871|`：主题设置弹窗（含玻璃材质级别）。
    *   `rightPanel` `872-876|`：右侧运行日志面板。
    *   `sponsor` `877-882|`：赞助二维码提示。
    *   `mobile` `883-892|`：移动端视图文本。
    *   `face` `893-917|`：2D 人脸识别专用文本。
*   **模板变量**：广泛使用 `{{variable}}` 如 `{{version}}`、`{{seconds}}`、`{{msg}}`、`{{field}}`、`{{names}}`、`{{operator}}` 等，运行时动态插入。
*   **特殊硬编码字符串**：
    *   `network.avoidBadWifiCommand` `352|`：`"adb shell settings put global network_avoid_bad_wifi 1"`。
    *   `network.avoidBadWifiGrantCommand` `353|`：`"adb shell pm grant com.campuslogin.client android.permission.WRITE_SECURE_SETTINGS"`。
    *   `quality.names` `477-497|`：预定义测试目标名称映射（网关、AliDNS、TencentDNS、Baidu、Bilibili、LoL 等）。

### Latency & Quality Processing

#### `android/frontend/src/lib/latency.ts`

**职责**：网络质量数据的标准化、合并、提取与展示解析，统一前端对后端推送指标的处理。

*   **类型定义**：
    *   `LatencyLevel` `4|`：`'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad'`。
    *   `LatencyType` `5|`：`'gateway' | 'external'`（导出类型）。
*   **阈值映射** `getLatencyLevel` `7-15|`：
    *   `<0` → `bad`；`<=20` → `excellent`；`<=50` → `great`；`<=100` → `good`；`<=200` → `fair`；`<=400` → `poor`；`>400` → `bad`。
*   **颜色生成** `getLatencyColor` `17-23|`：
    *   负数返回玫瑰红固定色板（`18|`）。
    *   正常调用 `getLatencyLevel` 查 `QUALITY_CONFIG[level]`（`19-20|`），若配置缺失兜底 muted 色调（`21-22|`）。
*   **数据合并策略** `mergeNetworkQuality` `25-36|`：
    *   输入 `old: NetworkQuality | null` 与 `incoming: NetworkQuality`。
    *   `incoming.quality === 'disabled'` → 返回 `old ?? incoming`（`26|`）。
    *   `incoming.quality === 'busy'`（增量推送）：若 `old` 空则返回 `{ ...incoming, quality: 'unknown' }`（`30|`）；若 `old.quality === 'busy'` 保持 busy 并合并 details/metrics（`31|`）；否则保留旧 quality 等级，仅合并 `details` 与 `metrics`（`32|`）。注释 `27-28|` 说明慢网络下终态延迟导致不采纳 busy 等级避免 UI 卡在“检测中”。
    *   普通状态：`!old || old.quality === 'unknown'` 直接返回 `incoming`（`34|`），否则合并 details/metrics 并覆盖 quality（`35|`）。
*   **延迟提取**：
    *   `extractGatewayLatency` `38-43|`：优先 `nq.gatewayLatency`，否则取 `details['gateway'].latency`，无则 `-1`。
    *   `extractExternalLatency` `45-59|`：优先 `averageExternalLatency`（`47-49|`），其次 `externalLatency`（`50|`），最后从 `details` 过滤非 gateway 项取中位数（`51-56|`），空则 `-1`。
*   **展示解析** `resolveQualityDisplay` `66-76|`：
    *   计算 `displayLatency = externalLatency >= 0 ? externalLatency : gatewayLatency`（`69|`）。
    *   若 `displayLatency >= 0` 且原始 `quality` 为 `unknown`/`busy`，则按延迟推断等级（`72-74|`），否则保留原值。
    *   返回 `{ quality, gatewayLatency, externalLatency, displayLatency }`。

## Architecture

```text
┌──────────────────────────┐   ┌──────────────────────────┐
│ i18n/index.ts (8-28|)    │   │ locales/en.json (1-918|) │
│ i18next init + export    │   │ English resource tree    │
└───────────┬──────────────┘   └────────────┬─────────────┘
            │ init resources                 │ import static
            └───────────────┬────────────────┘
                            ▼
                   ┌────────────────────┐
                   │ react-i18next hook │
                   │ (useTranslation)   │
                   └────────────────────┘

┌──────────────────────────┐   ┌──────────────────────────┐
│ @/monitor NetworkQuality │──►│ lib/latency.ts (1-76|)   │
│ (backend push)           │   │ merge/extract/resolve    │
└──────────────────────────┘   └────────────┬─────────────┘
                                            │ uses
                                            ▼
                                   ┌────────────────────┐
                                   │ @/network/constants│
                                   │ QUALITY_CONFIG     │
                                   └────────────────────┘
```

## Data Flow

1. **i18n 初始化流**：
    *   应用启动早期执行 `index.ts`，静态导入 `en.json` 与 `zh.json`（`5-6|`）。
    *   调用 `i18n.use(LanguageDetector).use(initReactI18next).init({...})`（`8-26|`），将 JSON 注册到 i18next store。
    *   `init` 中 `lng` 由 `localStorage` 或检测器决定（`17|`,`18-22|`），`fallbackLng: 'zh'`（`16|`）。
    *   React 组件通过 `useTranslation()` 获取 `t`，调用 `t('key.path', { vars })` 按当前语言返回 `en.json` 或 `zh.json` 对应字符串。
    *   语言切换时 `react-i18next` 触发重渲染更新文本。

2. **网络质量数据处理流**：
    *   输入：`NetworkQuality` 对象（来自 `@/monitor`，`latency.ts 1|` 导入类型）。
    *   `mergeNetworkQuality` 合并增量推送（`25-36|`）。
    *   `extractGatewayLatency` / `extractExternalLatency` 提取数值（`38-59|`）。
    *   `resolveQualityDisplay` 生成展示等级与延迟（`66-76|`）；`getLatencyLevel`/`getLatencyColor` 提供配色（`7-23|`）。
    *   消费者：首页卡片、网络质量页、状态栏胶囊（胶囊另有 DNS 延迟源，不使用 `resolveQualityDisplay`，见 `latency.ts 62|` 注释）。

## Connections

*   [[android-frontend-monitor]]：提供 `NetworkQuality` 类型与后端推送数据给 `latency.ts`（导入于 `latency.ts 1|`）。
*   [[android-frontend-network]]：提供 `QUALITY_CONFIG` 常量给 `getLatencyColor`（`latency.ts 2|`）。
*   [[android-frontend-ui-components]]：使用 `i18n` 实例与 `latency.ts` 导出函数渲染多语言界面与延迟指示器。
*   [[android-frontend-settings]]：用户在此修改语言（写入 `localStorage['app-language']`，见 `index.ts 20|`）、主题与通知设置。

## Known Issues

*   **localStorage 不可用回退**：`index.ts 17|` 使用 IIFE 包裹 `try-catch`，当隐私模式等导致 `localStorage` 不可用时返回 `undefined`，随后由 `detection.order` 中的 `'navigator'` 兜底（`19|`），不会崩溃但丢失手动语言记忆。
*   **Busy 状态合并防卡死**：`latency.ts 27-28|` 注释明确指出慢网络下终态迟迟不到，若采纳 `busy` 会让胶囊永远显示“检测中”；`mergeNetworkQuality` 在 `busy` 时保留旧 `quality` 或非 busy 时置 `unknown`（`30-32|`），需配合 `resolveQualityDisplay` 按延迟推断等级（`66-76|`）。
*   **QUALITY_CONFIG 缺失兜底**：`getLatencyColor` 在 `QUALITY_CONFIG[level]` 或 `unknown` 均缺失时返回 muted 样式（`21-22|`），避免因配置未同步导致颜色为空。
*   **语言包键不同步**：`en.json` 为源语言，`fallbackLng: 'zh'`（`16|`）确保缺失英文键时回退中文，但若 `zh.json` 也未包含对应键则显示原始 key 路径。