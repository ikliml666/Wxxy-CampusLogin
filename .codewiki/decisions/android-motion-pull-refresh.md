# 安卓动效升级：方向性面板转场 + 手写下拉刷新（批次三）

- 状态：accepted
- 日期：2026-09-28
- 关联提交：feat/android-design-audit（批次三）
- 关联：android-selfservice-empty-merge（批次二）、七维设计审计批三提案

## 决策

### 1. 方向性面板转场

- `createPanelAppleVariants(easing)`（`android/frontend/src/lib/animations.ts`）从静态对象改为**逐键 resolver 函数**：
  `initial/exit` 接收 `direction: -1|0|1`，`animate` 恒为 `{x:0,y:0,opacity:1}` + spring(320/24/0.7)。
- 方向来源：App.tsx 维护 `tabOrder`（底栏页签序，第 4 位随质量开关在 quality/monitor 间补位），
  `handleTabChange` 按 `indexOf` 索引差计算 `navDir`（-1/0/1）并存 state；`tabRef` 同步前值。
- **关键机制**：`<AnimatePresence mode="wait" custom={navDir}>` + `<m.div custom={navDir}>`。
  退出中的旧面板从 AnimatePresence 的 `custom` 拿到**最新**方向（组件自身 props 已冻结），
  这是 framer-motion 方向性退出的正解；逐键 resolver 才能按 custom 重新解析。
- 位移量（克制基线）：滑入 ±18px、滑出 ∓12px、退出 0.04s；direction=0（同位页签互切，如 quality↔monitor 归位）
  回落原纵向浮起（y:8→0），行为向后兼容。
- reduced-motion：`MotionConfig reducedMotion="user"` 全局把 transform 动画降级为淡入，无需单独处理。

### 2. 手写下拉刷新（仅总览页）

- `android/frontend/src/hooks/usePullToRefresh.ts`，零新依赖。启用条件 `enabled: deferredTab === 'dashboard'`，
  刷新动作 = `useAdapterStore.refreshAdapters`（NetworkPanel/RightPanel 同源语义）。
- 手势参数：阻尼 0.4（跟手递减）、触发阈值 64px、拉距上限 96px、触发后停 48px。
- **指示器是独立浮层**（App.tsx header 下 absolute 磨砂胶囊，z-[5]），**不位移内容**：
  内容跟随位移会与 AnimatePresence 面板转场的 transform 打架，且浮层方案零布局抖动。
  过阈值 ArrowDown 旋转 180°（松手语义）、松手 Loader2 转圈、完成归位。
- 监听细节：`touchmove` 非 passive（preventDefault 抑制橡皮筋）；`touchcancel` 只归位**不触发**刷新；
  触发时 `navigator.vibrate(10)`（try/catch 包裹，webview 拒绝则忽略）；`motion-reduce:transition-none` 让回弹直落。
- main 加 `overscroll-contain` 防滚动链。

## 否决的备选

- **内容位移跟随手指**（Chrome 风格内容下拉）：与面板转场 transform 冲突、需包裹层重构，否决。
- **第三方 PTR 库**：违反零新依赖纪律，否决。
- **全屏 iOS push 转场**：超出「克制动效」审美基线，取 18px 微位移+弹簧。

## 验证

- `npx tsc --noEmit` exit 0。
- CDP 设备模拟（412×915@2x，mock+5174）：页内 rAF 采样 `main .mx-auto > div` 的 transform tx——
  前进 dashboard→更多 max tx=**+18.00**（右滑入）、后退 更多→总览 min tx=**−18.00**（左滑入），PASS。
- 指示器三态：拉满 opacity=1 可见 / 松手 pill 内 animate-spin / 刷新完成 GONE 归位，PASS。
- 截图：`worktrae\shots\b3\b3-1-pull / b3-2-refreshing / b3-3-settled.png`。
