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

export function createPanelAppleVariants(easing: EasingConfig) {
  return {
    // 与安卓端同步：面板转场只走 transform，不做 opacity / scale。
    // opacity < 1 会把整个面板子树变成离屏合成层（每帧重新合成所有卡片的阴影/
    // 圆角/半透明底），安卓真机实测这是切页 janky 的主要来源之一；桌面端 GPU 更强
    // 但同一套代码，行为与语义保持一致。
    initial: { y: 8 },
    animate: {
      y: 0,
      // dampingRatio ≈ 0.80（Apple HIG 推荐 0.7-0.9 自然轻微弹性区间）
      transition: { type: 'spring' as const, stiffness: 320, damping: 24, mass: 0.7 },
    },
    exit: {
      y: -4,
      // 0.04s：退出只是过渡提示，等待税越低切换越跟手（0.08s 时每刀固定多等 80ms）
      transition: { duration: 0.04, ease: easing.exit as [number, number, number, number] },
    },
  }
}
