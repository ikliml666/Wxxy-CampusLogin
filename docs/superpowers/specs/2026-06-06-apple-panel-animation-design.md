# Apple 风格面板切换动画设计

## 背景

当前面板切换使用 Framer Motion `AnimatePresence mode="wait"` + opacity 淡入淡出 + x 位移滑动，导致切换时存在半透明过渡期，视觉上产生"模糊"感，不够干脆利落。

## 目标

- 移除面板切换时的 opacity 淡入淡出，消除模糊感
- 实现 Apple 官网风格的干脆利落切换动画（微妙弹性位移）
- 确保 GPU 加速，动画流畅无卡顿

## 方案

### 核心变更：移除 opacity，改用弹性位移 + GPU 加速

**当前行为**：
- exit: opacity 0 + x 位移, duration 220ms
- enter: opacity 0→1 + x 位移, duration 450ms
- AnimatePresence mode="wait" 导致旧面板完全退出后新面板才进入

**新行为**：
- exit: y=-4 + scale=0.99, duration 80ms, 无 opacity 变化
- enter: y=12 → y=0, spring 缓动 (stiffness: 300, damping: 24, mass: 0.8), 无 opacity 变化
- GPU 加速: will-change: transform, transform: translateZ(0)

### 修改文件

| 文件 | 变更 |
|------|------|
| `src/lib/animations.ts` | 新增 `createPanelAppleVariants`，替换 slide/fade variants |
| `src/App.tsx` | 更新 panelVariants 引用，添加 GPU 加速样式 |

### 动画参数

```typescript
export function createPanelAppleVariants(easing: EasingConfig) {
  return {
    initial: { y: 12 },
    animate: {
      y: 0,
      transition: { type: 'spring', stiffness: 300, damping: 24, mass: 0.8 },
    },
    exit: {
      y: -4,
      scale: 0.99,
      transition: { duration: 0.08, ease: easing.exit },
    },
  }
}
```

### GPU 加速策略

- 面板容器 m.div 添加 `style={{ willChange: 'transform', transform: 'translateZ(0)' }}`
- 确保 Framer Motion 的 transform 动画走 GPU 合成层
- 退出动画完成后自动清理 will-change（Framer Motion 内置处理）

### 不变的部分

- SegmentTabs 的 backdrop-blur-sm 和 layoutId 动画
- DockNav 的磁性效果
- useAnimationProfile 的 GPU 检测逻辑
- 所有其他 CSS keyframes 和 GSAP 动画

## 风险

- 低风险：只修改 2 个文件
- spring 动画在 Framer Motion 中原生支持
- 回退：恢复 panelVariants 引用即可
