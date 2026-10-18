---
title: design-prototypes
type: module
source_files: []
tags: [UI, Prototype, E2E, CDP, Tauri, Design System]
---

## Overview
本模块 `design-prototypes` 是前端 UI/UX 设计探索与自动化回归测试的核心资产库。它包含两类主要文件：
1.  **静态 HTML 原型**：用于展示“近期上网记录”、“监测模块”、“在线设备卡”及“账号管理页”的视觉设计方案（H1/H2/H3 方案对比、响应式适配、堆叠交互）。这些文件不依赖构建工具，可直接在浏览器中预览，作为开发实现的视觉基准。
2.  **Node.js E2E 测试脚本**：基于 Chrome DevTools Protocol (CDP) 编写的无头浏览器自动化测试脚本。它们直接操控本地运行的应用实例（通常运行在 `localhost:5175`），验证特定的 UI 状态、交互流程（如弹窗、别名修改、密码修改）及渲染边界条件（如空态、堆叠卡轮播）。

## Key Components

### 1. 静态 HTML 原型组件

#### `modules/design-prototypes/online-history-redesign.html`
*   **职责**：展示“近期上网记录”重设计的三种视觉方案（H1/H2/H3），解决安卓窄屏不可读问题。
*   **关键样式与布局**：
    *   **CSS 变量体系** (`:root`, L6-L14)：定义 `--primary` (hsl 239...), `--bg`, `--card`, `--muted` 等语义化颜色；字体使用 `'JetBrains Mono'` 用于数值显示。
    *   **手机模拟框** (`.phone`, L65)：宽度固定 `372px`，圆角 `20px`，用于模拟安卓视图。
    *   **统计芯片行** (`.stat-row`, L85)：Grid 布局 `repeat(4, 1fr)`，展示合计时长/流量/金额/次数。
    *   **设备分布条形图** (`.dev-bars`, L95)：纯 CSS 实现，无图表库依赖。包含 `.dev-bar-track` (轨道) 和 `.dev-bar-fill` (填充)，支持 `.dim` 类降低透明度。
    *   **7日趋势图** (`.trend`, L112)：Flex 布局柱状图，`.trend-bar.empty` 处理零值（灰色低条，高度 `3px`）。
    *   **会话卡** (`.sess-card`, L136)：核心组件。结构分为 `.sess-top` (时间+徽章)、`.sess-mid` (IP/终端)、`.sess-detail` (MAC/计费/金额，虚线分隔)。
    *   **Windows 表格** (`.win-table`, L158)：保留原表格形态，仅增加顶部统计头。
*   **方案细节**：
    *   **H1 (推荐)**：统计头 + 设备分布条形 + 按天分组的会话卡。Windows 端保留表格。
    *   **H2 (趋势主视觉)**：顶部 7 日流量柱状图为主，下方弱化列表。Windows 端趋势图与设备分布并排。
    *   **H3 (极简列表)**：无统计头，纯紧凑列表。Windows 端零改动。
*   **关键逻辑说明** (L230+)：统计均客户端求和；“在线”会话（logoutTime 为空）显示为“— 在线”，不计入金额；主机名列被抛弃。

#### `modules/design-prototypes/online-records-merged.html`
*   **职责**：展示“上网记录”模块的融合设计。将“近期登录流水”与“日期范围查询明细”合并为同一张卡片，通过状态切换展示不同数据维度。
*   **关键样式与布局**：
    *   **日期查询条** (`.datebar`, L46)：左右日期选择器 + “查询”按钮。
    *   **8项统计网格** (`.stat-grid-8`, L66)：查询态下展示国际/国内上下行拆分、总流量、总时长、总金额、记录数。
    *   **会话卡增强** (`.sess-card`)：
        *   **默认态**：附 MAC、终端、计费方式。
        *   **查询态**：附国际/国内上下行流量拆分、NAS 地址。
*   **数据流与交互**：
    *   **默认态 (Recent)**：免查询，展示近期流水。统计项 4 枚（时长/流量/金额/次数）。
    *   **查询态 (Query)**：点击查询后同卡切换。统计项扩展至 8 枚。会话卡详情变为上下行拆分。
    *   **Windows 端**：单表双源。近期流水行补 MAC/终端列，账单行补拆分/NAS 列，缺失字段显示“—”。
*   **边界条件**：账单接口无 MAC/终端信息，近期流水无上下行拆分，UI 以“—”留白，不硬造数据。

#### `modules/design-prototypes/monitor-redesign.html` & `monitor-redesign-android.html`
*   **职责**：监测模块的设计原型 HTML。前者为通用/桌面端视图，后者为 Android 专用视图（通过 `data-platform="android"` 属性区分）。
*   **CSS 变量系统 (`:root`)**：
    *   主色 `--primary: #4F6BFF`，背景 `--bg: #F5F6FA`，卡片 `--card: #FFFFFF`。
    *   状态色：绿 `--green: #16A34A`，红 `--red: #DC2626`。
*   **布局结构**：
    *   `.topbar`: 紧凑控制条。包含 App 图标、标题 (`h1`)、徽章计数 (`badge-count`)、副标题 (`sub`)、运行开关 (`.sw`)、间隔输入 (`.interval .num`)、刷新/停止按钮 (`.btn`)。
    *   `.adapters`: 适配器状态 Mini Chips。包含以太网 (`.adp.ok`) 和 WLAN (`.adp.off`) 两个子项，显示名称、IP/Meta 和状态 Pill。
    *   `.cards2`: 双列网格布局 (`grid-template-columns: 1fr 1fr`)。
        *   **左卡 (自动化)**：包含启动自动开启、在线后自动退出、可登录时自动登录、非校园网自动退出 (红色图标)、退出生效时段 (时间控件)。
        *   **右卡 (校园网校验)**：包含环境验证、子区块 `.subblock` (SSID 列表 chips: `i-wxxy`, `iwxxy-2`, `iwxxy-3`; 网关 IP: `10.2.127.254`)、检测时间段、定时登录/注销。
    *   `.foot`: 底部状态条，显示当前 SSID (`i-wxxy`) 和连接状态。
*   **响应式逻辑 (`@media (max-width: 480px)`)**：
    *   `.topbar`: 换行，标题独占一行，控件独占一行。
    *   `.cards2`: 变为单列 (`1fr`)。
    *   `.subblock .s-row`: 改为纵向排列 (`flex-direction: column`)。
    *   **Android 特异性**: `body[data-platform="android"] .adapters { display: none }`。

#### `modules/design-prototypes/online-device-cards-stack.html`
*   **职责**：在线设备卡片的堆叠交互形态对比原型。展示三种视觉变体 (V1/V2/V3)，并包含 JavaScript 实现的触摸/鼠标拖拽切换逻辑。
*   **数据结构 (`DEVICES`)**：
    *   静态数组，包含 3 个设备对象：`{ time, full, ip, mac, flow, utime, term }`。其中第三个设备 `term` 为 `'Mobile'`。
*   **样式变体 (`STYLES`)**：
    *   `peek`: 边缘露出。参数 `[0, 1, 50, 1, 100, 1]` (Y偏移, Scale)。
    *   `deck`: 缩放纸叠。参数 `[0, 1, 30, 0.95, 60, 0.90]`。
    *   `dots`: 页点轮播。参数 `[0, 1, 14, 0.96, 14, 0.92]`，隐藏下层内容。
*   **JavaScript 交互逻辑**：
    *   `buildCard(d, forDesktop)`: 克隆 `<template id="dev-card-tpl">`，填充数据。桌面端显示完整时间，移动端显示短时间。Mobile 终端添加 `.mobile` class。
    *   `render(dragY)`: 根据当前索引 `index` 和拖拽偏移 `dragY`，计算每张卡的 `transform` (translateY + scale) 和 `z-index`。
    *   **事件监听**:
        *   `pointerdown/move/up`: 统一处理鼠标和触摸。
        *   阈值判定：上滑 < -60px 切下一张 (`next`)，下滑 > 60px 切上一张 (`prev`)，点击 (< 8px) 切下一张，否则弹回。
        *   阻止冒泡：`.btn-offline` 和 `.sdot` 的点击不触发切换。
    *   **桌面预览**: 独立渲染静态网格 `#desktop-grid`，不应用堆叠效果。

#### `modules/design-prototypes/online-device-cards.html`
*   **职责**：在线设备卡片的三种内容布局方案对比原型（静态，无 JS 交互）。
*   **方案 A (芯片数据卡)**：
    *   布局：`.chip-row` (Grid 2列)。IP 和 MAC 分别放入浅底 `.chip` 容器中。
    *   底部：流量和时长并排显示。
*   **方案 B (流量主视觉卡)**：
    *   布局：`.flow-hero`。流量数字放大至 `27px`，作为视觉焦点。
    *   中部：`.kv-lines` 浅底区域显示 IP/MAC 键值对。
    *   底部：仅显示时长（流量已突出）。
*   **方案 C (紧凑双列卡)**：
    *   布局：`.dense-grid` (Grid 1.35fr / 1fr)。四项信息 (IP, Flow, MAC, Time) 密排两列。
    *   特点：无芯片容器，无大数字，高度最小化。
*   **共同特征**：
    *   移除主机名。
    *   移动端时间格式 `HH:mm:ss`，桌面端 `YYYY-MM-DD HH:mm:ss`。
    *   保留终端类型徽章 (Badge)。

#### `modules/design/account-redesign-mockup.html`
*   **职责**：静态 HTML/CSS 原型，定义账号页重设计的视觉规范（桌面双列/安卓单列）、组件样式及数据流说明。
*   **关键 CSS 变量**：`--primary: hsl(239 84% 61%)`, `--success: hsl(142 71% 45%)`, `--radius: 12px` (L10-L18)。
*   **核心布局类**：
    *   `.cf-stage` / `.cf-item`: 桌面端水平封面流堆叠，使用 `translateX` 和 `scale` 变换 (L106-L115)。
    *   `.cf-vstage` / `.cf-vt` / `.cf-vb`: 安卓端垂直封面流堆叠，使用 `translateY` 和 `scale(0.75)` (L119-L123)。
    *   `.acct-card`: 账号卡片容器，`.is-active` 状态有紫色边框高亮 (L124-L127)。
    *   `.desk-cols`: 桌面端 Grid 布局 `300px 1fr` (L168)。
*   **弹窗结构**：
    *   Modal A (编辑登录信息): 包含用户名、密码（带显隐切换）、运营商选择 (L200-L220)。
    *   Modal B (绑定运营商): 包含凭据分段开关 (`seg`)，支持“登录信息”或“手动填写”模式 (L230-L260)。
    *   Modal C (绑定状态查询): 显示掩码手机号，含“查看明文”按钮 (L270-L290)。
*   **数据流注释**：HTML 底部 `<div class="flow">` 明确后端改动点：`update_config` 需同步回写账号档案，切换时从目标档案读字段 (L300-L310)。

### 2. E2E 自动化测试脚本

所有 `.mjs` 脚本均为独立可执行的 Node.js 脚本，通过启动独立的 Chrome 实例（Headless New）进行通信。共享封装函数 `send(method, params)` 用于发送 CDP 指令，返回 Promise；共享封装函数 `evalJs(expr)` 用于执行页面 JavaScript 并返回值。

#### `modules/design-prototypes/statusdlg-test.mjs`
*   **职责**：E2E 自动化测试脚本，验证“账号管理页 -> 查询绑定状态 -> 弹窗间距”的回归情况。
*   **关键实现**：
    *   **环境启动** (L10-L15)：使用 `child_process.spawn` 启动 Headless Chrome，端口 `9227`，临时用户目录 `userDir`。
    *   **CDP 通信** (L20-L35)：封装 `send()` 方法，通过 WebSocket 发送 CDP 命令（如 `Runtime.evaluate`, `Page.captureScreenshot`）。
    *   **截图保存** (L30-L34)：将 Base64 图片写入 `shots/` 目录。
    *   **自动化步骤**：
        1.  **导航** (L45-L51)：查找文本为“账号”的按钮并点击。
        2.  **触发弹窗** (L54-L59)：查找含“查询绑定状态”的按钮并点击。
        3.  **间距测量** (L62-L83)：
            *   定位 `[role="dialog"]` 中包含“中国移动”的弹窗。
            *   筛选运营商行（高度 20-80px，含 span，无 button）。
            *   计算行间距 (`gapsBetweenRows`) 和列表到底部按钮区的距离 (`listToButtonsGap`)。
            *   检测底部按钮区是否有顶部边框 (`hasBorderT`)。
    *   **异常捕获** (L28)：监听 `Runtime.exceptionThrown`。
    *   **清理** (L88-L90)：终止 Chrome 进程并删除临时目录。

#### `modules/design-prototypes/home-alias-test.mjs`
*   **职责**：无头浏览器自动化测试脚本，用于验证 Dashboard “在线信息卡”别名修改流程（点击编辑 -> 弹窗输入 -> 保存 -> 徽章更新），并截取最终状态截图。
*   **核心常量与配置**：
    *   `CHROME`: `'C:/Program Files/Google/Chrome/Application/chrome.exe'` (硬编码路径)。
    *   `PORT`: `9226` (Chrome DevTools Protocol 端口)。
    *   `width = '390'`, `height = '844'` (移动端视口尺寸)。
    *   `out`: 默认输出路径 `'shots/home-alias.png'`。
*   **关键函数/逻辑**：
    *   `waitPort()`: 轮询 `http://127.0.0.1:${PORT}/json/version`，最大重试 50 次，间隔 200ms。
    *   `evalJs(expr)`: 封装 CDP `Runtime.evaluate`，通过 WebSocket 发送 JSON-RPC 请求，返回 Promise。
    *   **执行步骤 (Step 0-5)**：
        *   Step 0: 确保在“总览”Tab (`button[textContent='总览']`)。
        *   Step 1: 寻找 `aria-label` 包含“显示”或“show”的按钮（眼睛图标），处理滚动可见性 (`scrollIntoView`) 后点击。
        *   Step 2: 寻找 `aria-label` 包含“命名”或“name”的按钮（铅笔图标），检测 `getBoundingClientRect` 宽高 > 0 后点击。
        *   Step 3: 定位 `[role="dialog"] input`，通过 `Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set` 强制设置值为 `'实验室台式机'`，并触发 `input` 事件。
        *   Step 4: 点击对话框内包含“保存”或“save”的按钮。
        *   Step 5: 校验 `span[textContent='实验室台式机']` 是否存在，存在则判定成功。
    *   **截图**: 使用 `Page.captureScreenshot` 获取 Base64 数据，写入 `-open.png` (弹窗时) 和最终 `.png`。
    *   **清理**: `finally` 块中调用 `chrome.kill()` 和 `rmSync(userDir)`。

#### `modules/design-prototypes/accdeck-test.mjs`
*   **职责**：验证账号堆叠卡在填充 3 个账号后的渲染、轮播交互及浅色模式背景色。
*   **关键函数/常量**：
    *   `URL_ = 'http://localhost:5175/'` (L8)。
    *   `waitPort()`: 等待 Chrome Debug Port 就绪 (L20-L26)。
    *   `evalJs(expr)`: 封装 CDP Runtime.evaluate (L40)。
    *   `shot(name)`: 捕获屏幕截图并保存至 `shots/` (L42-L46)。
*   **测试步骤**：
    1.  导航至“账号”页 (L50-L58)。
    2.  断言堆叠卡存在、包含特定学号芯片 `24385214` 且无空态提示 (L62-L72)。
    3.  模拟点击右侧半区触发轮播，验证当前激活索引变化 (L76-L92)。
    4.  获取当前卡片计算背景色，验证非灰底 (L96-L106)。
    5.  切换至 Light Mode，验证背景为纯白 (L108-L120)。

#### `modules/design-prototypes/accempty-probe.mjs` & `accempty-probe2.mjs`
*   **职责**：复现用户报告的“空态灰块”问题，通过逐层 DOM 遍历上报背景色。
*   **关键实现**：
    *   URL 参数 `?mockempty=1` (L8)。
    *   强制切换 Light Mode (L35)。
    *   **逐层探测逻辑**：从文本节点 `"暂无保存的账号"` 向上遍历 10 层父元素，记录每层的 `backgroundColor`, `className`, `width`, `height` (L40-L52)。
    *   检查指示器点是否存在于空态中（预期不存在）(L56-L62)。
    *   **Clip 截图**：计算账号管理段边界框，调用 `Page.captureScreenshot` 带 `clip` 参数 (L45-L52)。
    *   **背景染色检测**：遍历 `seg.querySelectorAll('*')`，筛选出 `backgroundColor !== 'rgba(0, 0, 0, 0)'` 的元素，输出其标签、类名、背景色及阴影 (L56-L66)。

#### `modules/design-prototypes/alias-dialog-test.mjs`
*   **职责**：验证设备改名弹窗的完整交互流程（打开->输入->保存->徽章更新）。
*   **关键实现**：
    *   命令行参数解析 `[url, width, height, out]` (L8)。
    *   使用 `Emulation.setDeviceMetricsOverride` 模拟移动端尺寸 (L32)。
    *   **查找铅笔按钮**：过滤 `aria-label` 包含 "命名" 或 "name" 的按钮 (L42-L44)。
    *   **React 输入模拟**：使用 `Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set` 直接设置值并 dispatch `input` 事件 (L52-L54)。
    *   断言保存后 DOM 中出现新的 `span` 文本 "宿舍笔记本" (L65-L68)。

#### `modules/design-prototypes/cdp-probe.mjs`
*   **职责**：通用探针，检查在线信息堆叠卡的渲染姿势（水平 vs 垂直）。
*   **关键实现**：
    *   自动点击“自助服务”Tab 和“查询”按钮 (L35-L40)。
    *   **姿势检测逻辑**：查询包含 `translateX` + `scale` + `rounded-xl` 的 div 数量 (X)，以及包含 `translateY` + `scale` + `rounded-xl` 的 div 数量 (Y) (L42-L48)。
    *   输出结果格式：`{ X: count, Y: count, xS: [transforms], yS: [transforms] }` (L49)。

#### `modules/design-prototypes/cdp-shot.mjs`
*   **职责**：通用截图工具，处理 React `useDeferredValue` 导致的低优先级渲染伪影，支持面板状态持久化。
*   **关键实现**：
    *   默认等待时间 `waitMs = '6500'` (L10)。
    *   **面板状态注入**：若参数以 `panel:` 开头，先导航，关闭赞助弹窗，写入 `localStorage` (`campus-active-panel`, `sponsor-last-shown`)，再重新导航 (L55-L65)。
    *   真实时钟等待策略，避免 Headless 虚拟时间预算导致的渲染跳过 (L68-L72)。

#### `modules/design-prototypes/chpwd-oldpass-test.mjs`
*   **职责**：深度测试修改密码弹窗的“手动输入旧密码”兜底流程。
*   **关键实现**：
    *   **打桩 Invoke**：拦截 `window.__TAURI_INTERNALS__.invoke`，记录命令参数至 `window.__calls` (L80-L84)。
    *   **展开态验证**：点击“手动输入旧密码”按钮后，断言出现 3 个输入框，第一个具有 `autocomplete="current-password"` (L90-L100)。
    *   **提交断言**：验证 `change_self_password` 命令收到正确的 `account`, `password` (旧), `newPassword` (新) 参数 (L115-L120)。
    *   **噪音过滤**：过滤掉与本次改动无关的 chunk 报错 (L125)。

#### `modules/design-prototypes/chpwd-test.mjs`
*   **职责**：基础修改密码弹窗测试（无需旧密码场景）。
*   **关键实现**：
    *   验证弹窗打开、双输入框存在 (L50-L58)。
    *   验证提交后 Toast 显示 "密码修改成功" 且弹窗关闭 (L75-L80)。

#### `modules/design-prototypes/deck-android-test.mjs`
*   **职责**：验证 DNS 牌堆在安卓同构模式下的渲染、翻页及环形切换逻辑。
*   **关键实现**：
    *   URL 参数 `?panel=network` (L11)。
    *   **点击下半区/上半区**：通过 `Input.dispatchMouseEvent` 精确坐标点击，触发翻牌 (L60-L68)。
    *   **环形切换验证**：连续点击底部，验证索引从 0->1->2->0 (环形) (L75-L78)。
    *   验证牌堆高度稳定性 (L79-L80)。

#### `modules/design-prototypes/deck-manage-view.mjs`
*   **职责**：账号管理堆叠卡近景截图，验证空态提示及“添加账号”输入行展开。
*   **关键实现**：
    *   滚动至“账号管理”可见 (L45-L48)。
    *   断言空态文本存在 (L55-L58)。
    *   点击“添加账号”按钮，断言出现占位符含 "账号名" 的 Input (L62-L72)。

#### `modules/design-prototypes/deck-restyle-test.mjs`
*   **职责**：账号管理页第四轮改版 E2E，验证整体布局及绑定状态弹窗流程。
*   **关键实现**：
    *   **布局断言**：验证“账号管理”卡在“绑定运营商”卡下方，宽度约为后者一半，底部对齐 (L55-L70)。
    *   **弹窗流程**：点击“查询绑定状态”，验证弹窗打开、包含掩码手机号、存在“查看明文”按钮 (L75-L95)。
    *   **明文查看**：点击“查看明文”，验证弹窗内出现明文数字 `654321` (L100-L115)。

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                     Design Prototypes Module                 │
├──────────────────────────────┬───────────────────────────────┤
│       Static Prototypes      │      E2E Test Scripts         │
│                              │                               │
│  ┌────────────────────────┐  │  ┌─────────────────────────┐  │
│  │ online-history-redesign│  │  │ statusdlg-test.mjs      │  │
│  │ (H1/H2/H3 Visuals)     │  │  │ (Binding Dialog Spacing)│  │
│  └────────────────────────┘  │  └─────────────────────────┘  │
│  ┌────────────────────────┐  │  ┌─────────────────────────┐  │
│  │ online-records-merged  │  │  │ home-alias-test.mjs     │  │
│  │ (Merged Data Source)   │  │  │ (Alias Edit Flow)       │  │
│  └────────────────────────┘  │  └─────────────────────────┘  │
│  ┌────────────────────────┐  │  ┌─────────────────────────┐  │
│  │ monitor-redesign.html  │  │  │ accdeck-test.mjs        │  │
│  │ (Monitor Control)      │  │  │ (Account Deck Render)   │  │
│  └────────────────────────┘  │  └─────────────────────────┘  │
│  ┌────────────────────────┐  │  ┌─────────────────────────┐  │
│  │ online-device-cards*.  │  │  │ alias-dialog-test.mjs   │  │
│  │ (Stack/Card Layouts)   │  │  │ (Rename Dialog)         │  │
│  └────────────────────────┘  │  └─────────────────────────┘  │
│  ┌────────────────────────┐  │  ┌─────────────────────────┐  │
│  │ account-redesign-      │  │  │ chpwd-test.mjs          │  │
│  │ mockup.html            │  │  │ (Password Change)       │  │
│  └────────────────────────┘  │  └─────────────────────────┘  │
│                              │  ┌─────────────────────────┐  │
│                              │  │ deck-android-test.mjs   │  │
│                              │  │ (Android Deck Flip)     │  │
│                              │  └─────────────────────────┘  │
└──────────────────────────────┴───────────────────────────────┘
         ▲                          ▲
         │                          │
   Browser Preview           Localhost:5175
   (Direct Open)             (Running App Instance)
```

## Data Flow

1.  **HTML 原型间**：无代码调用，均为独立静态 HTML 文件，共享相同的 CSS 变量命名空间（`--primary`, `--mono` 等），确保视觉一致性。
2.  **MJS 脚本与 HTML**：无直接调用。`statusdlg-test.mjs` 等操作的是运行中的本地应用（`localhost:5175`），而非原型文件本身。
3.  **数据流假设**：
    *   `online-history-redesign.html` 中的数据是拟真样例（Mock Data），用于展示 H1/H2/H3 的视觉差异。
    *   `online-records-merged.html` 定义了前端需兼容的数据结构：
        *   **近期流水对象**：{ timeRange, duration, traffic, amount, ip, mac, terminal, billingType }
        *   **账单明细对象**：{ timeRange, duration, traffic, amount, ip, internationalUp, internationalDown, domesticUp, domesticDown, nasAddress }
    *   前端需根据数据源类型渲染不同的 `.sess-detail` 内容。

## Connections

*   该模块的视觉规范（CSS 变量、组件类名）直接指导 `[[components-ui]]` 中的实际组件实现。
*   E2E 测试脚本 (`*.mjs`) 依赖于 `[[core-app-runner]]` 提供的本地开发服务器实例 (`localhost:5175`)。
*   `account-redesign-mockup.html` 中定义的数据流注释（`update_config` 回写）关联到 `[[backend-api-config]]` 的后端接口契约。

## Known Issues

*   **零值处理**：
    *   在 H2 方案中，流量为 0 的天数显示为极低的灰色条 (`height:3px`)，避免图表空白误导。
    *   在融合卡中，缺失的数据字段（如 MAC 或上下行拆分）统一显示为“—”，保持表格对齐。
*   **“在线”会话特殊处理**：
    *   注销时间为空时，显示为“— 在线”。
    *   此类会话不参与金额合计（见 `online-history-redesign.html` L233）。
*   **客户端聚合**：
    *   所有统计芯片（时长、流量、金额、次数）均由客户端对现有流水数据进行求和/聚合，无需后端新增接口（L231）。
*   **测试脚本健壮性**：
    *   `statusdlg-test.mjs` 使用了 `try-catch-finally` 确保 Chrome 进程和临时目录即使在前置步骤失败时也能被清理（L87-L90）。
    *   等待端口就绪采用轮询 (`waitPort`)，防止竞态条件。
*   **React 受控组件输入**：
    *   多个脚本（`alias-dialog-test.mjs`, `chpwd-test.mjs`, `chpwd-oldpass-test.mjs`）明确指出不能直接设置 `input.value`，必须使用 `Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set` 配合 `dispatchEvent(new Event('input'))` 来触发 React 的状态更新 (引用 `alias-dialog-test.mjs:52-54`, `chpwd-test.mjs:60-63`)。
*   **Headless 渲染延迟**：
    *   `cdp-shot.mjs` 特别指出 `useDeferredValue` 在 Headless 模式下可能因虚拟时间预算导致渲染被跳过，因此采用较长的固定 `sleep` 等待（默认 6.5s）而非仅依赖网络空闲 (引用 `cdp-shot.mjs:5-8, 68-72`)。
*   **空态背景色诊断**：
    *   `accempty-probe.mjs` 和 `accempty-probe2.mjs` 揭示了“灰底”问题的复杂性：不仅仅是卡片本身，可能是祖先元素的背景色透过透明层叠加所致。测试通过逐层上报 `getComputedStyle` 背景色来定位源头 (引用 `accempty-probe.mjs:40-52`, `accempty-probe2.mjs:56-66`)。
*   **权限/门控逻辑**：
    *   `deck-restyle-test.mjs` 验证了“查看明文”按钮的存在性及其点击后的内容变化，暗示后端或前端存在基于权限或状态的明文展示逻辑 (引用 `deck-restyle-test.mjs:100-115`)。
*   **环境噪音过滤**：
    *   `chpwd-oldpass-test.mjs` 专门实现了错误过滤器，排除包含 `@ chunk-[A-Z0-9]+\.js` 的 Uncaught Promise 错误，这些错误被认为与本次功能改动无关，属于构建产物噪音 (引用 `chpwd-oldpass-test.mjs:125`)。
*   **赞助弹窗干扰**：
    *   多个脚本（`alias-dialog-test.mjs`, `cdp-shot.mjs`, `deck-manage-view.mjs`）包含关闭“请作者喝一杯”或赞助弹窗的逻辑，这是测试稳定性的关键边界条件，因为该弹窗可能遮挡 UI 元素 (引用 `alias-dialog-test.mjs:42-44`, `cdp-shot.mjs:58-60`, `deck-manage-view.mjs:35-40`)。