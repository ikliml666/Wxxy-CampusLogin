# Changelog

## v2.2.2 - 前端动画帧率与流畅度全面提升

### 性能优化

- **GPU 加速参数**: WebView2 启用 EnableDrDc、RawDraw、GPU rasterization，Intel 核显利用率显著提升
- **box-shadow paint 消除**: 卡片 hover 发光效果改为独立 `.card-glow-layer` div + CSS opacity 过渡，零 paint 开销
- **will-change 反优化修复**: 移除滚动容器上的 `will-change: transform`，恢复浏览器原生滚动合成
- **React 渲染优化**: AnimatedCard 包裹 `React.memo`，移除 `isHovered` / `rippleStyle` state，hover 效果改为纯 CSS `:hover` 控制
- **GSAP 全局配置**: `lagSmoothing(500, 33)` 防止帧丢失级联，`force3D: true` 强制 GPU 合成
- **日志面板虚拟化**: RightPanel 滚动采用 RAF 节流 + scroll-based virtualization，大幅减少 DOM 节点
- **CSS 合成层**: 卡片添加 `contain: layout style paint`，面板添加 `content-visibility: auto`

### Apple 风格动画质感

- **缓动曲线革命**: 全局缓动从 `power2.out` / `ease` 统一升级为 `expo.out` (`cubic-bezier(0.16, 1, 0.3, 1)`)，退出动画使用 `ease-in` (`cubic-bezier(0.7, 0, 0.84, 0)`)
- **入场动画**: 卡片入场添加 `scale(0.98)` 微缩放，stagger 间隔 0.04s，时长 0.4s
- **面板切换**: slide 位移 50px + scale(0.98)，fade 添加 scale(0.99) 微缩放
- **按钮交互**: hover/active scale 变化幅度减小（1.03/0.97 for physical, 1.02/0.96 for press），Apple 缓动曲线
- **TitleBar 图标**: hover scale 1.08 + expo.out 缓动（原 1.15 + 弹簧弹跳），active scale 0.95
- **窗口控制按钮**: 新增 `.titlebar-win-btn` 类，hover scale 1.1 + active scale 0.92
- **Dock 磁性效果**: quickTo 缓动改为 expo.out，duration 0.35s
- **数字动画**: AnimatedNumber valueQuickTo / scaleQuickTo 统一使用 expo.out

### 滚动体验

- 主滚动区域添加 `scroll-behavior: smooth` + `overscroll-behavior: contain`
- FluidBackground 动画范围缩小，减少 GPU 负载
- Intel 核显低配档位 orb 动画时长延长（multiplier 0.75→1.2）

### 启动序列

- GSAP 默认缓动 `power2.out` → `expo.out`
- 启动动画时长优雅化延长（0.25s→0.5s titleBar/statusBar, 0.5s→0.7s dockNav）
- 位移量微调，入场更自然

### 涉及文件

- `src-tauri/src/platform/gpu.rs` - WebView2 GPU 加速参数
- `frontend/src/index.css` - CSS 动画、缓动、合成层优化
- `frontend/src/App.tsx` - 滚动容器优化
- `frontend/src/main.tsx` - GSAP 全局配置
- `frontend/src/components/ui/animated-card.tsx` - 卡片性能重构
- `frontend/src/components/layout/TitleBar.tsx` - 图标 hover Apple 化
- `frontend/src/components/layout/DockNav.tsx` - Dock 缓动升级
- `frontend/src/components/layout/RightPanel.tsx` - 虚拟滚动 RAF 节流
- `frontend/src/hooks/useStartupBoost.ts` - 启动序列 Apple 化
- `frontend/src/hooks/useAnimationProfile.ts` - Intel 核显配置调优
- `frontend/src/lib/animations.ts` - Framer Motion variants Apple 缓动
- `frontend/src/shared/AnimatedNumber.tsx` - 数字动画缓动升级
