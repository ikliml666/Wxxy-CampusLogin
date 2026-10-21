---
title: android-frontend-i18n-and-lib
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
tags: [i18n, glass-effect, dock-lens, spring-physics, latency, notification, render-liveness, utils]
---

## Overview

本模块 `android-frontend-i18n-and-lib` 是 Android 前端应用的核心基础库，负责处理国际化资源加载、高级视觉特效（玻璃拟态与悬浮栏镜头效果）、网络质量数据标准化、系统权限交互、渲染存活检测以及通用工具函数。该模块通过高度优化的物理仿真内核和 DOM 操作策略，实现了在 Chromium 内核下的高性能动画体验，同时确保了多语言环境的稳定性和跨平台兼容性。

## Key Components

### i18n (Internationalization)

#### `android/frontend/src/i18n/index.ts`
**职责**：作为 i18next 单例的初始化入口，配置多语言资源、检测策略及 React 集成。

*   **关键实现细节**：
    *   `1-3|`：导入核心库 `i18next`、`react-i18next` 插件及浏览器语言检测器 `LanguageDetector`。
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

#### `android/frontend/src/i18n/locales/zh.json`
**职责**：提供应用全量中文本地化字符串，覆盖导航、认证、账号管理、网络配置、质量检测、自助服务、设置、日志、引导、更新、主题、生物识别等所有 UI 文本。支持模板变量（如 `{{version}}`, `{{seconds}}`, `{{msg}}`）。

*   **关键内容与结构**：
    - **通知 (`notify`)**: 包含自动登录成功/失败 (`autoLoginSuccess`, `autoLoginFailed`)、自动退出倒计时逻辑 (`autoExitSoon`, `autoExitCountdownLog`)、非校园网退出提示 (`campusExit`, `campusExitDesc`)、更新包完整性未校验警告 (`updateUnverified`)。行号 3-18。
    - **通用 (`common`)**: 基础操作动词 (moveCard, refresh, clear, add, delete, edit, done, save, close, retry, open, none, refreshing, error, warning, info, seconds, times, unknown, renderError, unknownError, reload)。行号 20-42。
    - **导航 (`nav`)**: 页面标签名 (dashboard, account, selfservice, network, monitor, quality, speedtest, settings, log, more)。行号 44-54。
    - **标题栏 (`titlebar`)**: 应用名称 "Campus Login Assistant" (`appName`)、深色/浅色模式切换 (`switchToDark`, `switchToLight`)、通知开关状态、主题/关于/赞助入口、窗口控制 (minimize, maximize, restore, exit)、新版本发现提示 (`newVersionFound`, `newVersion`)、语言切换。行号 56-71。
    - **认证 (`auth`)**: 登录/注销状态及错误信息 (login, loggingIn, logout, loggingOut, loginSuccess, loginFailed, loginError, loggingInToast, logoutSuccess, logoutFailed, logoutError, loggingOutToast, notLoggedIn)。网络适配器状态 (networkAdapterOnline/offline)、网络状态 (notOnCampus, noNetwork, unknownStatus)。配置保存失败 (`configSaveFailed`, `configDirtyResetLog`)、登录后质量检查失败 (`loginAfterQualityCheckFailed`)、超时 (loginTimeout, logoutTimeout)。行号 73-97。
    - **账号 (`account`)**:
        - **登录信息**: 学号/密码输入占位符、显示/隐藏密码、运营商选择。行号 100-111。
        - **账号管理**: 添加/删除/重命名账号、当前使用标识、保存/切换/删除失败或成功提示。行号 112-166。
        - **自动化设置**: 自动登录校园网 (`autoLoginCampus`)、夜间自动切换运营商 (`nightOperatorSwitch`) 及其详细描述。行号 121-124。
        - **密码修改**: 旧密码获取、新密码验证 (6-11位数字或英文)、修改成功/失败、需先填写登录信息提示。行号 130-144。
        - **绑定状态**: 查询绑定状态、已绑定/未绑定、密码设置状态、查看/隐藏密码、身份验证未通过。行号 145-154。
        - **自助服务仪表盘**: 在线设备概览 (selfDashboardTitle/Desc)、刷新、查询中、需要凭据提示、空闲状态、单位说明、空状态、查询失败。行号 188-197。
        - **上网记录**: 近期记录列表、空状态、列定义 (上线时间、注销时间、IP、MAC、时长、流量、主机名、终端类型、操作、计费方式、金额)。行号 198-214。
        - **自助操作**: 注销设备 (selfOffline)、确认弹窗、注销成功/失败、设备别名编辑/保存/取消/成功/失败。行号 215-225。
        - **详细日志查询**: 日期范围选择、查询按钮、查询中、失败、空记录、溢出提示 (超过500条仅显示前500条)、汇总指标 (国际/国内上行下行MB、使用流量MB、时长分钟、金额元、记录数)。行号 226-244。
        - **生物识别**: 敏感操作身份验证提示、生物识别取消/未录入/锁定/失败、人脸超时/动作未完成/比对未通过/相机无法访问。行号 251-259。
        - **其他**: 在线/分/元徽章、无效账号名/显示名规则、保存/删除/切换/重命名失败成功、编辑登录信息描述、绑定凭据来源 (登录信息/手动)、密码芯片状态、待补全提示。行号 157-187, 260-262。
    - **网络 (`network`)**:
        - **适配器**: 检测数量 (含禁用)、无适配器提示、无IP、连接速度、无线/有线、状态 (disabled, disconnected, enabledNoIp, connected)。启用/启用中/启用成功/失败。行号 264-283。
        - **DHCP/IP**: 获取新IP、刷新DHCP、获取新IP成功/失败、跳过非校园网子网。行号 284-322。
        - **DNS优化**: 推荐阿里+腾讯DNS+DoH、检测DNS、一键优化、设置中、成功/失败、点击检测提示、推荐DNS提示、配置文件vs适配器DNS优先级、按配置文件/按适配器、DoH加密状态、未配置DNS、Doh可用/已加密/未加密、不支持DoH、待优化/未配置。行号 286-318。
        - **夜间出站切换**: 自动注销并触发WiFi重检配合系统“避免烂WiFi”机制、次日7:30重新登录。ADB命令指南 (开启避免烂WiFi、授权WRITE_SECURE_SETTINGS)、已授权提示、手动方式保留、注意事项 (切换时机由系统决定、蜂窝数据关闭时无路可切、常开时白天卡顿也会切)。复制命令/已复制。还原系统默认设置/提示/成功/无内容/失败。行号 326-340。
    - **监测 (`monitor`)**:
        - **状态检测服务**: 启动/停止/运行中/已停止/立即刷新/检测中/后台服务异常降级提示。行号 343-352。
        - **验证设置**: 验证间隔、组分类 (自动化、校园网校验)。行号 353-359。
        - **自动化**: 启动时自动开启检测、可登录时自动登录。行号 360-363。
        - **校园网校验**: 网络名称 (i-wxxy等)、网关地址 (10.2.127.254)、检测逻辑步骤 (①SSID匹配 ②/18子网匹配 ③网关Ping)、任一满足即判定。时间段控制。行号 364-376。
        - **定时任务**: 每日定时登录/注销启用及时间设置、错过时刻补触发、注销后60s内不自动登回。行号 380-385。
        - **SSID/状态**: 当前SSID、未获取、已连接校园网、非校园网。行号 386-389。
        - **延迟详情**: 内网/外网延迟高、延迟过高/异常、时间线标签 (UDP, DNS, TCP, TLS, TTFB, Content, Network) 及描述。行号 390-408。
    - **质量 (`quality`)**:
        - **实时监测**: 内网/外网/DNS解析延迟、延迟详情、网络质量标签 (极速/优秀/良好/一般/较慢/拥堵/未知/已禁用/检测中)。行号 411-447。
        - **测试选项**: 测试间隔、自动检测、测试详情描述、刷新、总耗时、尚未检测、检测时间、UDP异常警告 (QoS限速建议DoH)、超时、目标/类型。行号 421-434。
        - **解释**: TLS/TTFB/内容传输的解释及建议。行号 448-453。
        - **测试节点名称**: 网关、阿里DNS、腾讯DNS、信风DNS、阿里DoH、腾讯DoH、DNS解析、百度、京东、必应、12306、哔哩哔哩、抖音、哔哩直播、抖音直播、LOL、原神、PUBG、永劫无间。行号 454-473。
    - **测速 (`speedtest`)**: 综合/教育网/轻量测速网站列表及描述 (测速网、Ookla、中科大、东北大学、北邮、Fast.com、ChinaZ、IP.cn)、交叉对比提示、教育网推荐、左右拖拽提示。行号 476-501。
    - **设置 (`settings`)**:
        - **外观**: 配色方案 (默认蓝/活力紫/森林绿/午夜橙/海洋青/樱桃红/自定义)、自定义主色调、浅色/深色模式、切换明暗。行号 504-517。
        - **启动**: 开机自启、自动登录校园网、系统通知 (启用/描述/权限请求/去开启)。行号 518-530。
        - **保活**: 电池优化白名单 (已允许/未允许/申请失败/描述)、厂商自启/省电设置 (打开设置页/失败/描述/提示)、确认弹窗。行号 531-546。
        - **安全**: 生物识别验证 (启用/描述)、每次操作二次验证 (启用/描述)、允许2D人脸验证 (启用/描述/安全性低)、关闭安全选项需验证/验证失败/已更新。行号 548-558。
        - **网络质量**: 启用检测、延迟计算选项 (跳过TTFB/跳过内容)、解释 (TTFB/内容传输/网络延迟)、推荐建议、固定网关地址。行号 559-584。
        - **其他**: 清空、颜色列表、新手指引、ISP名称 (无锡学院/电信/联通/移动)、开机自启失败。行号 585-607。
    - **日志 (`log`)**: 系统日志描述、调试开关 (开/关/已开启/已关闭/切换失败)、获取/清空失败、清空确认、全部/无日志/无筛选结果、最近N条提示、行数选项 (100/200/500/1000)、搜索占位、模块筛选、无搜索结果、保留时长 (3天/7天/14天/30天/永久)、初始化失败/未用推荐DNS/未启用DoH/保存失败/保留设置失败日志。行号 609-645。
    - **引导 (`onboarding`)**: 欢迎页、账号信息 (必填/占位/错误)、网络适配器选择、准备就绪、开始登录/登录中/成功、跳过确认、上一步/下一步/跳过、显示/隐藏密码、绑定运营商 (标题/描述/运营商/手机号/账户密码/SMS提示/绑定/绑定中/成功/失败/跳过提示)、步骤指示、绑定状态检测 (标题/重试/检测中/已绑定/未绑定/描述/需账号/失败)。行号 647-707。
    - **更新 (`updatePrompt`)**: 发现新版本标题/正文/立即前往/稍后关闭。行号 709-713。
    - **关于 (`about`)**: 标题/描述、应用描述、检查更新 (检查中/发现新版本/已是最新/检查失败/重试/上次失败/GitHub受限/未找到/更新日志/无说明/GitHub仓库/赞助/一键下载/切换源/新功能亮点/准备中/下载中/速度/完成/安装/安装注意/下载失败/重试/返回/等待检查/正在检查)、多账号管理/自动重连/官方源/安装失败/更新渠道 (镜像加速/GitHub官方)。行号 715-756。
    - **状态栏 (`statusbar`)**: 刷新延迟检测、检测中、刷新延迟、用户自助服务、打开认证门户、在线运营商徽章 (当前运营商/账号/夜间临时切换/副适配器)。行号 758-769。
    - **仪表盘 (`dashboard`)**: 快捷操作 (DHCP续租/获取新IP/账号管理/网络质量)、卡片管理 (添加/无卡片/编辑)、自助服务合并卡片 (未配置/描述/查询失败/重试/显示/隐藏/在线/近期记录/无在线/无记录/设备数/记录数/关键信息隐藏)。行号 771-807。
    - **确认对话框 (`confirmDialog`)**: 确定/取消。行号 809-811。
    - **面板 (`panel`)**: 各面板标题及描述 (dashboard, account, selfservice, network, monitor, quality, speedtest, settings, log)。行号 813-831。
    - **主题对话框 (`themeDialog`)**: 标题/描述、配色方案、浅/深色、玻璃质感 (关闭/标准/极致及描述)。行号 833-847。
    - **右侧面板 (`rightPanel`)**: 运行日志、清空日志、无日志。行号 849-852。
    - **赞助 (`sponsor`)**: 标题/描述、微信/支付宝收款码替代文本。行号 854-858。
    - **移动端 (`mobile`)**: 网络状态、可达/不可达、检测次数、更多菜单 (后台检测/设置/日志/测速)。行号 860-868。
    - **人脸 (`face`)**: 录入/验证标题、启动相机/相机错误、模型下载进度/失败、重试、录入进度、请正对屏幕、眨眼/转头挑战、取消、风险提示、录入/重新录入/已录入/未录入、录入成功/失败、验证通过、安全提示。行号 870-893。

### Visual Effects & Physics (Glass & Dock)

#### `android/frontend/src/lib/glass-dock.tsx`
**职责**：封装 `Hyalite` 库为 React Hook，提供悬浮栏的“折射玻璃”效果（非纯 CSS 模糊）及按压光斑。解决 Chromium 内核下 `backdrop-filter` 在合成层构建期间的黑屏问题。

*   **常量配置** (`GLASS_DOCK_OPTS`, `line 39-65`)：
    *   定义 `standard` 和 `ultra` 两档 `HyaliteOptions`。
    *   **Bevel 约束**：两档 `bevel` 均设为 `18`，严格等于圆角半径。注释指出若 `bevel > radius` (如 26) 会导致圆角处出现重影弧。
    *   **Ultra 参数**：`thickness: 400`, `slope: 4`, `dispersion: 2`。通过增大厚度而非带宽来增强折射，代价是滤镜三通道叠加。
*   **Hook: `useGlassRefraction`** (`line 103-227`)：
    *   **入参**：`ref` (HTMLElement), `level` ('off' | 'standard' | 'ultra')。
    *   **启动期等待逻辑** (`line 111-226`)：
        *   **根因分析**：GSAP 入场动画导致祖先链出现 `opacity < 1`、`transform` 或 `will-change`，使 Chromium 将该层视为 `backdrop root`，导致 `backdrop-filter` 采样不到内容变黑 (`line 82-96`)。
        *   **检测机制**：`backdropReady()` (`line 174-184`) 遍历父元素，检查 `opacity !== '1'`、`transform !== 'none'`、独立变换属性 (`translate/scale/rotate`) 及 `willChange`。注意从 `parentElement` 开始查，因为悬浮栏自身常驻 `translateX`。
        *   **超时策略**：使用 `ResizeObserver` 监听尺寸抖动。若布局稳定后 `MAX_WAIT_MS` (2500ms) 仍未就绪，则放弃挂载，保留 CSS 实底降级，避免常驻黑膜 (`line 196-200`)。
        *   **激活时机**：检测到就绪后调用 `hyalite.attach`。随后通过 `LIVE_FRAMES` (2帧) 延迟添加 `.glass-live` 类，配合 CSS 过渡避免闪烁 (`line 149-154`)。
    *   **清理**：卸载时取消 rAF、移除类名、调用 `hyalite.detach`。
*   **Hook: `useGlassPressGlow`** (`line 242-295`)：
    *   **视觉参数**：`GLOW_ALPHA: 0.34`, `GLOW_REACH: 39`, `GLOW_SPREAD: 3`。
    *   **交互逻辑**：
        *   `pointerdown`：设置光斑位置 (`--glass-glow-x/y`)，添加 `.is-pressed` 类，设置初始 alpha 和半径。
        *   `pointermove`：仅在按下状态下更新位置，避免松开后跟随鼠标晃动造成无效重绘 (`line 266-270`)。
        *   `pointerup/cancel`：移除 `.is-pressed` 触发 CSS 淡出 (360ms)，同时将 alpha 归零、半径放大 3 倍 (`GLOW_REACH * GLOW_SPREAD`)，模拟扩散消散效果 (`line 274-279`)。
    *   **边界处理**：卸载时强制重置 alpha 为 0，防止组件销毁时光斑残留 (`line 293`)。

#### `android/frontend/src/lib/dock-lens.ts`
**职责**：实现悬浮导航栏 (Dock) 的“镜头”效果，模拟 iOS/FlClash 的交互体验。通过直接操作 DOM style 实现高性能动画，零 React re-render。

*   **关键常量** (行 21-48):
    - `LENS_GROWTH = 8`: 镜头抬起时四周胀出的像素。
    - `LENS_MAGNIFY = 0.16`: 镜片内内容的放大倍率。
    - `LIFT_SNAP = 0.975`: 镜头抬起高度阈值，达到此值才允许落下。
    - `LIFT_RELEASE = 0.04`: 镜头离目标距离阈值，小于此值且抬起到位才落下。
    - `JELLY_SPEED = 3`, `JELLY_STRETCH = 0.2`: 果冻拉伸的速度参考值和拉伸量。
    - `JELLY_BRAKE = 25`, `JELLY_SQUASH = 0.15`: 果冻刹车的速度参考值和压扁量。
    - `JELLY_ASPECT = 1.4`: 拉伸时纵向收缩倍数。
    - `GLOW_PEAK = 0.06`, `GLOW_REACH = 28`, `GLOW_GROWTH = 3`, `GLOW_WASH = 0.7`: 光斑相关参数。
    - `OVERDRAG = 0.35`: 拖出边界后的橡皮筋硬度上限。
    - `PULL_LIMIT = 7 / 32`: 越界拉伸上限比例。
    - `FLING_PROJECTION = 0.1`: 松手投掷的投影时长。
    - `TOUCH_SLOP = 8`: 横向位移阈值，区分点击和拖动。

*   **关键接口与导出**:
    - `interface DockLensOptions` (行 69-78): 包含 `count` (项数), `selected` (选中下标), `onSelect` (选中回调), `enabled` (是否启用)。
    - `useDockLens(navRef: RefObject<HTMLElement | null>, opts: DockLensOptions)` (行 90): 核心 Hook。

*   **内部逻辑与状态**:
    - **DOM 元素定位** (行 100-104): 查找 `[data-dock-lens]`, `[data-dock-magnify]`, `[data-dock-magnify-inner]`, `[data-dock-strip]`。
    - **测量** (行 107-121): `measure()` 函数计算所有项的中心点 `centers`、宽度 `widths`、栏宽高 `barW/barH`。处理了 `[data-dock-strip]` 容器的偏移修正 (行 116-117)。
    - **辅助函数** (行 124-143): `lastIndex()`, `clampIndex()`, `xAt(position)` (线性插值中心点), `widthAt(position)` (线性插值宽度+4px), `pitch()` (平均间距)。
    - **运动曲线实例** (行 245-257): 创建 `Motion` 实例 `lens`, `lift`, `stretch`, `jelly`, `glow`, `glowSpread`，均关联 `render` 回调。
    - **渲染循环 `render()`** (行 156-223):
        - 计算药丸位置/大小/透明度，考虑果冻形变 (行 157-170)。
        - 设置放大层 `clipPath` 为药丸形状，内部副本缩放 `mag` (行 172-184)。
        - 设置栏整体拉伸 `--dock-stretch` (行 189)。
        - 在 `stripEl` 上使用 `clip-path: path(...)` 挖掉药丸形状的区域，防止残影 (行 195-211)。
        - 设置光斑 CSS 变量 `--dock-glow-x/y/alpha/reach/wash` (行 214-223)。
    - **果冻驱动 `wobbler()`** (行 227-233): 基于镜头加速度调整 `jelly` 目标值。
    - **落下判定 `maybeDrop()`** (行 238-243): 检查 `dropping`, `pointerId`, `lift.value < LIFT_SNAP`, `|lens.goal - lens.value| > LIFT_RELEASE`，满足条件则触发 `lift.to(0, SPRING.lift)`。
    - **指针事件处理**:
        - `onDown` (行 308-329): 捕获指针，重置状态，触发光斑亮起、镜头抬起、镜头移动到按下项。
        - `onMove` (行 288-306): 若未按住则忽略。更新光斑位置，处理越界拉伸，记录样本用于投掷。若超过 `TOUCH_SLOP` 进入拖动状态，镜头跟随手指位置。
        - `onUp` (行 374-377): 调用 `finish(true)` 提交选择。
        - `onCancel` (行 379-382): 调用 `finish(false)` 取消。
        - `onBlur` (行 386-388): 窗口失焦时主动收尾。
    - **投掷 `flingTarget(pressed)`** (行 332-343): 基于最后几个样本计算速度，投射额外项数，限制在相邻项范围内。
    - **结束流程 `finish(commit)`** (行 345-372): 释放指针捕获，重置拖动状态。若 commit 且目标变化，更新 `selected` 并调用 `onSelect`。触发光斑熄灭、拉伸回弹。
    - **外部同步 `syncRef.current`** (行 391-394): 响应外部 `selected` 变化，移动镜头但不触发回调。
    - **ResizeObserver** (行 396-401): 监听布局变化，重新测量并更新镜头位置。
    - **生命周期** (行 403-426): 挂载时添加事件监听和 ResizeObserver，首帧落位。卸载时清理所有监听、观察者、Motion 实例。
    - **独立 Effect** (行 429-432): 监听 `opts.selected` 变化，调用 `syncRef.current` 同步镜头位置。

#### `android/frontend/src/lib/dock-spring.ts`
**职责**：提供悬浮栏镜头系统所需的物理弹簧仿真内核，以及橡皮筋和饱和函数。

*   **关键类与函数**:
    - `interface SpringSpec` (行 17-22): 包含 `stiffness` (刚度) 和 `damping` (阻尼)。
    - `springOfDuration(durationMs: number, bounce = 0)` (行 33): 根据稳定时长和回弹量反解弹簧参数。公式基于 `zeta = 1 - bounce`, `wn = 4/(zeta*duration)`, `stiffness = wn^2`, `damping = 2*zeta*wn` (行 34-36)。
    - `springOfRatio(stiffness: number, ratio: number)` (行 40): 根据刚度和阻尼比构造参数 (行 41)。
    - `class Motion` (行 77-147): 标量运动曲线类。
        - 属性: `value` (当前值), `target` (目标值), `velocity` (速度), `spec` (弹簧规格), `onFrame` (每帧回调)。
        - `get current`, `get goal`, `get speed`, `get accel`, `get moving` (行 90-111)。
        - `to(target, spec?)` (行 114-120): 追向新目标。若已在运动中且目标相同则不重启，保留速度。
        - `jump(value)` (行 123-129): 立即落位，清除速度，从活跃集合移除。
        - `step(dt)` (行 131-142): 半隐式欧拉积分更新速度和值。若接近静止 (位置差 `< REST_VALUE` 且速度 `< REST_VELOCITY`) 则停止并移除。
        - `dispose()` (行 144-146): 从活跃集合移除。
    - **全局调度** (行 53-69): `active` Set 存储运动的 Motion 实例。`loop(now)` 使用 requestAnimationFrame 统一调度，钳制最大步长 `MAX_STEP = 1/30` (行 49)。`schedule()` 启动循环。
    - `rubberBand(overshoot, limit)` (行 155-159): 橡皮筋函数，使用公式 `pull = 1 - 1/((|overshoot|*0.55)/limit + 1)` 计算阻力，模拟 iOS 手感。
    - `saturate(value)` (行 162-163): tanh 型饱和函数，`1 - 2/(exp(2*value)+1)`。

#### `android/frontend/src/lib/easing-config.ts`
**职责**：提供基于屏幕刷新率的缓动配置 (Easing Config)，确保动画在不同刷新率下保持一致的视觉节奏。

*   **关键常量与函数**:
    - `interface EasingConfig` (行 3-9): 包含 `enter`, `exit`, `smooth`, `snappy`, `overshoot` 五种缓动曲线，均为 `[number, number, number, number]` 格式 (对应 cubic-bezier 四个控制点)。
    - `EASING_60HZ` (行 11-17): 60Hz 下的缓动配置。例如 `enter: [0.16, 1, 0.3, 1]`, `exit: [0.7, 0, 0.84, 0]`。
    - `EASING_120HZ` (行 19-25): 120Hz 下的缓动配置。例如 `enter: [0.12, 1, 0.24, 1]`, `exit: [0.6, 0, 0.8, 0]`。
    - `getEasingConfig(refreshRate: number)` (行 27-28): 根据刷新率返回对应的配置对象，>=120 返回 120Hz 配置，否则返回 60Hz 配置。

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
        *   **关键决策**：`busy` 状态下不采纳 incoming 的 quality 字段，避免 UI 状态震荡 (`line 27-33`)。
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

### System Utilities & Permissions

#### `android/frontend/src/lib/notificationPermission.ts`
**职责**：Android 通知权限的统一请求入口，处理不同 Android 版本及国产 ROM 的差异化行为。

*   **核心函数**：`requestNotificationPermission` (`line 9-20`)。
    *   **动态导入**：按需加载 `@tauri-apps/plugin-notification`。
    *   **权限检查**：先 `isPermissionGranted()`，若已授权直接返回。
    *   **请求流程**：调用 `requestPermission()`。
        *   **异常容忍**：捕获 `requestPermission` 异常（如弹框崩溃或不可用），按未授权继续执行后续逻辑 (`line 13`)。
    *   **二次检查**：请求后再次检查是否获得授权。
    *   **引导设置**：若未授权且 `opts.openSettingsIfDenied` 为 true，尝试调用 `tauriApiWithRetry.openNotificationSettings` 跳转系统设置 (`line 15-17`)。
*   **重试机制**：设置页跳转使用 `tauriApiWithRetry`，应对 Tauri API 在非安卓环境或偶发失败的情况。

#### `android/frontend/src/lib/renderLiveness.ts`
**职责**：检测渲染循环 (rAF) 是否存活，用于判断 GPU/合成器是否崩溃。相比 `setInterval` 心跳，能更准确捕捉渲染进程假死。

*   **功耗修复** (`line 4-9`)：
    *   **原问题**：模块级常驻 rAF 无限循环，导致 Chromium 合成器持续派发 BeginFrame，前台静置功耗高。
    *   **新方案**：按需短探测。仅当距离上次探测超过 `PROBE_REFRESH_MS` 时才开窗检测。
*   **常量**：
    *   `PROBE_REFRESH_MS: 4000`：探测间隔上限，小于停滞阈值一半，保证正常态不触发误判。
    *   `PROBE_TIMEOUT_MS: 500`：探测窗口兜底超时。
    *   `RENDER_STALL_THRESHOLD_MS: 10000`：判定停滞的绝对阈值（10秒无 rAF 回调）。
*   **探测逻辑** (`startProbe`, `line 24-34`)：
    *   若正在探测 (`probing`) 则跳过。
    *   连续发起两个 `requestAnimationFrame` 回调，刷新 `lastRafTime`。
    *   第二个回调结束后重置 `probing` 标志。
    *   设置 `setTimeout` 兜底，防止第二帧因半死状态未回调导致永久锁定。
*   **导出函数**：`isRenderLoopAlive` (`line 36-40`)：
    *   **可见性短路**：若 `document.hidden` 返回 `true`，直接返回 `true`。避免浏览器节流 rAF 期间误判停滞 (`line 37`)。
    *   **触发探测**：若距上次探测 > 4s，调用 `startProbe`。
    *   **结果判定**：若 `performance.now() - lastRafTime <= 10s` 返回 true，否则 false。

#### `android/frontend/src/lib/utils.ts`
**职责**：通用工具函数集合，包含样式合并、错误信息提取、安全存储。

*   **样式合并** (`cn`, `line 4-6`)：
    *   组合 `clsx` (条件类名) 和 `tailwind-merge` (冲突解决)。
    *   签名：`(...inputs: ClassValue[]) => string`。
*   **错误提取** (`extractErrorMessage`, `line 8-18`)：
    *   处理 `string`, `Error` 对象。
    *   **Tauri 插件兼容**：特别处理 Tauri 插件 reject 的普通对象 `{code, message}`。若直接 `String()` 会得到 `[object Object]`，此处优先提取 `message` 字段 (`line 13-16`)。
    *   兜底：`String(e)`。
*   **安全存储** (`safeStorage`, `line 22-29`)：
    *   封装 `localStorage`。
    *   **内存回退**：使用 `memoryFallback` Map。若 `localStorage` 写入失败（如配额已满或隐私模式），尝试写入内存 Map，确保应用运行时数据不丢失。
    *   `get`：先读 localStorage，失败读内存 Map。
    *   `set`：尝试写 localStorage，无论成功与否均同步更新内存 Map，返回布尔值表示持久化结果。

### Animation & Color Utils

#### `android/frontend/src/lib/animations.ts`
**职责**：定义基于 Framer Motion 的动画变体 (Variants)，用于日志条目入场/退场和面板转场。

*   **关键实现**：
    - `createLogEntryVariants(easing: EasingConfig)` (行 3): 创建日志条目的入场/退场动画。
        - `initial`: `{ opacity: 0, x: 20 }` (行 5)。
        - `animate`: `{ opacity: 1, x: 0 }`，过渡时间 0.3s，缓动为 `easing.snappy` (行 6-9)。
        - `exit`: `{ opacity: 0, x: -16 }`，过渡时间 0.15s，缓动为 `easing.exit` (行 11-15)。
    - `createPanelAppleVariants(easing: EasingConfig)` (行 22): 创建方向性面板转场动画，支持水平滑入/出和纵向浮起。
        - `initial(direction: -1 | 0 | 1 = 0)` (行 25): 若 `direction === 0`，初始状态为 `{ y: 8, opacity: 0.9 }`；否则 `{ x: 18 * direction, opacity: 0.9 }` (行 26)。
        - `animate()` (行 27): 回到 `{ x: 0, y: 0, opacity: 1 }`，使用弹簧动画 `type: 'spring'`, `stiffness: 320`, `damping: 24`, `mass: 0.7` (行 32)。
        - `exit(direction: -1 | 0 | 1 = 0)` (行 34): 若 `direction === 0`，退出到 `{ y: -4 }`；否则 `{ x: -12 * direction }`。附加 `opacity: 0.9`, `scale: 0.99`。过渡时间 0.04s，缓动为 `easing.exit` (行 35-39)。

#### `android/frontend/src/lib/color.ts`
**职责**：提供 HEX 颜色值到 HSL 颜色空间的转换工具函数。

*   **关键实现**：
    - `hexToHsl(hex: string)` (行 1): 将 HEX 字符串转换为 `{ h: number; s: number; l: number }` 对象。
        - 输入校验：若非字符串或长度小于4，返回默认值 `{ h: 230, s: 70, l: 55 }` (行 2)。
        - 正则匹配：尝试匹配 `#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})`，失败则返回默认值 (行 3-4)。
        - 解析 RGB：将 hex 转为 0-1 范围的 float r, g, b (行 6-8)。若 NaN 则返回默认值 (行 10)。
        - 计算 HSL：计算 max/min, l=(max+min)/2。若 max!=min，计算差值 d, s=l>0.5 ? d/(2-max-min) : d/(max+min)。根据 max 值计算 h (行 12-25)。
        - 返回：`{ h: Math.round(h * 360), s: Math.round(s * 100), l: Math.round(l * 100) }` (行 28-32)。

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