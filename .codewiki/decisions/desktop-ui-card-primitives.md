---
title: "桌面 UI 精修：SettingRow/CardIcon 基元与参数重分组（2026-09-27）"
type: decision
date: 2026-09-27
status: accepted
tags: [前端, ui, 设计系统, 动画]
---

## 背景

用户要求对 Windows 版前端做排版优化、卡片重设计、各卡参数重新分类放置、动画优化，目标是「方便快捷美观」。GitHub 调研（Clash Verge Rev / Spacedrive / shadcn blocks / Tremor / Motion 官方文档）给出可迁移模式：①设置页按**主题**分卡、卡内按**分组标题+行式参数**排布；②重设计前先抽 Card/Row 基元锁死圆角/间距/层级；③状态卡用「状态色图标→大号主数值→辅助元数据」层级；④动画分工——hover/开关/颜色走纯 CSS，Motion/GSAP 只管入场、layoutId 切换、弹层。

## 决策

### 1. 两个新基元（`components/ui/`），全面板收编

- **`SettingRow`**（`setting-row.tsx`）：参数行唯一形态 = `flex items-center justify-between gap-4`，左 `min-w-0`（Label text-sm font-medium + description text-xs muted），右 `shrink-0` 控件区；可选 `icon`（彩色语义小图标，替代散落各卡的 w-7/w-8 手写芯片）与 `htmlFor`（label 点击联动控件）。收编全库 57 处手写 `justify-between` 参数行（Settings/Account/Monitor/Network/Quality/Dashboard）。
- **`CardIcon`**（card.tsx）：卡头图标片唯一形态 = `w-10 h-10 rounded-xl bg-primary/10`。替代手写 div（此前 rounded-full 与 rounded-xl 混用）；卡头标题区一律加 `min-w-0` 防长标题溢出。

不换技术栈、不动 AnimatedCard 体系与主题令牌——本次是**体系内精修**（Spacedrive 教训：先锁基元再谈重设计）。

### 2. 参数重分组（按主题而非按控件类型）

- **SettingsPanel**：8 个启动参数重排三组（组标题 text-xs muted，组间 Separator）——启动行为（autoLaunch/hiddenStart）/登录自动化（autoLoginOnStart/autoExitAfterLogin/autoExitOnOnline）/窗口与界面（lightweightMode/minimizeToTray/defaultPanel）；通知+安全两卡合并为「通知与安全」单卡（组标题复用 settings.security），卡数 7→6。`enableQuality` 关闭联动（清 defaultPanel/enableLatencyTest/跳 dashboard）与 `handleSecurityDisable`（Hello 验证）逻辑原样迁移进 SettingRow。
- **MonitorPanel**：校验卡参数按「自动化 / 校园网网校验」两组标题分区；检测逻辑五步 Tooltip 从 label 行内移到组标题旁；SSID/网关固定值行改 SettingRow 右侧 mono 框（max-w-[220px]，同 SettingsPanel fixedGateway）。四个时间窗行**保留** `flex-col sm:flex-row` 窄屏堆叠结构（learnings/input-time-wrapper-w-full 的 Input time 包装坑不能丢）。
- 表单类输入（用户名/密码/绑定字段）与数据展示行保持堆叠布局——SettingRow 只收编「label 左 + 控件右」的开关/选择行。

### 3. 动画策略（对齐 Motion 官方分工，不引入新依赖）

- 既有体系已符合调研结论：入场/stagger 走 card-enter CSS + framer-motion（一次性，不绑数据刷新）、hover 走 CSS（animated-card-interactive）、面板切换走 deferredPanel 弹簧（decisions/deferred-panel-transition）、数字滚动走 gsap.quickTo。本次**不加**新全局动效。
- 修复一处真实动画 bug：三个 GSAP hook（breathe/glow/pulse）改回调 ref 模式，条件渲染元素重建后动画不再静默失效（learnings/gsap-tween-stale-ref-after-remount）。RightPanel 空态呼吸、质量劣化光晕、DockNav 加载脉冲三处受益。
- reduced-motion 与 economy 分档（useAnimationProfile）不动。

### 4. 双端同步欠账

本次仅改桌面端（`tauri-app/frontend`），用户当次指令明确只要求 Windows。安卓端 `android/frontend` 是独立复刻树，如需同款基元需另次任务移植。

## 后果

- 参数行/卡头视觉一致性由基元保证，新增设置参数默认走 SettingRow，不再长出手写行。
- i18n 新增键：settings.notificationSecurity(+Desc)/groupStartup/groupLoginAutomation/groupWindowBehavior；monitor.groupAutomation/groupCampusCheck（zh/en 对齐）。
- 验证：tsc 0 错误、vitest 96/96、vite build 通过；浏览器实测待用户（布局类改动）。

## 第二轮：六项 UX 精修（2026-09-27 同日追加）

基于 Atlassian 空态 / Polaris 徽标语义 / NN.g 渐进披露 / shadcn·Tremor 仪表盘判据的落地（用户批准的五项 + 设置页空洞填补）：

1. **Dashboard 首屏摘要带**（DashboardPanel.tsx `DashboardSummaryBand`）：认证状态（useAuthStore.status，色板复制 StatusBar.statusConfig 需同步维护）/ 适配器在线数（`adapters.filter(a=>a.status==='connected').length`）/ 网络质量（resolveQualityDisplay）三段 KPI 行，点击各段经 `useAdapterStore.setActivePanel` 直达面板，质量段带就地刷新；enableNetworkQuality=false 整段隐藏。数据与 StatusBar 同源，不新起轮询。
2. **Quality 四段式空态**：质量卡与明细卡无数据且非检测中 → 图标+标题+一句原因+「立即检测」主按钮（替代 5 行 "--" 占位与 loading 空转）；检测中保留行内 pulse skeleton。
3. **Account 绑定卡渐进披露**：绑定状态区常驻，表单默认收起；披露入口 button（aria-expanded + 标签写明点开所见）+ AnimatePresence m.div（opacity/y ±6, 0.18s，不动 height 避免布局动画）；测试需先点 `account.bindFormToggle` 展开。
4. **badge 语义色**：绿=终态成功、进行中=蓝+spinner（不占色相）——Quality 定时测试「运行中」badge 由绿改 `text-blue-600 dark:text-blue-400` + Loader2 spin；「已在线」绿保留。
5. **可发现性**：Dashboard「获取新IP」双适配器时加 ChevronDown（rotate-180 联动菜单开合）；NetworkPanel 可排序行加常驻低对比 GripVertical 把手（长按起拖逻辑不变）；Dashboard 编辑按钮 ghost→outline。
6. **SettingsPanel 两列均衡**：手动分栏 items-start——左=启动设置+数据管理导出卡、右=通知与安全+质量检测卡（enableQuality 联动逻辑原样迁移）+新手引导；禁 justify-between 拉伸。

教训：JSX 注释放在 `&& (` 之后会报 TS2657，须置于条件块之前；DashboardPanel.selfCards 测试的 useAuthStore mock 需补 `status:{text:'',state:'unknown'}`（真 store 初值，mock 缺字段使摘要带崩）。

## 后果（第二轮）

- i18n 新增键：dashboard.bandAuth/bandAdapters/bandQuality/bandOnlineCount；quality.emptyTitle/emptyDesc/runTestNow/emptyDetailsTitle/emptyDetailsDesc；account.bindFormToggle(+Desc)（zh/en 对齐）。
- 验证：tsc 0 错误、vitest 96/96（修 2 测试文件后）、vite build 通过；六项全部截图复验（含 reduced-motion 下 network 面板）。

## Connections

[[deferred-panel-transition]]、[[input-time-wrapper-w-full]]、[[contain-paint-clips-absolute-menu]]、[[tablet-layout-alignment-audit]]
