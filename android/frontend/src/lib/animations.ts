import type { EasingConfig } from './easing-config'

export function createLogEntryVariants(easing: EasingConfig) {
  return {
    initial: { opacity: 0, x: 20 },
    animate: {
      opacity: 1,
      x: 0,
      transition: { duration: 0.3, ease: easing.snappy as [number, number, number, number] },
    },
    exit: {
      opacity: 0,
      x: -16,
      transition: { duration: 0.15, ease: easing.exit as [number, number, number, number] },
    },
  }
}

// 方向性面板转场（批三）：direction 由页签索引差决定——前进了从右滑入(+18)、
// 后退了从左滑入(-18)，旧面板反向滑出(∓12)；0 = 原纵向浮起。逐键 resolver 函数：
// AnimatePresence 的 custom 会转发给退出中的旧面板，方向切换时旧面板也按新方向退出。
export function createPanelAppleVariants(easing: EasingConfig) {
  const exitEase = easing.exit as [number, number, number, number]
  return {
    initial: (direction: -1 | 0 | 1 = 0) =>
      direction === 0 ? { y: 8, opacity: 0.9 } : { x: 18 * direction, opacity: 0.9 },
    animate: () => ({
      x: 0,
      y: 0,
      opacity: 1,
      // dampingRatio ≈ 0.80（Apple HIG 推荐 0.7-0.9 自然轻微弹性区间）
      transition: { type: 'spring' as const, stiffness: 320, damping: 24, mass: 0.7 },
    }),
    exit: (direction: -1 | 0 | 1 = 0) => ({
      ...(direction === 0 ? { y: -4 } : { x: -12 * direction }),
      opacity: 0.9,
      scale: 0.99,
      // 0.04s：退出只是过渡提示，等待税越低切换越跟手（0.08s 时每刀固定多等 80ms）
      transition: { duration: 0.04, ease: exitEase },
    }),
  }
}
