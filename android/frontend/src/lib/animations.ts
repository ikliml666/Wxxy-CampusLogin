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

const PANEL_ORDER = ['dashboard', 'account', 'network', 'monitor', 'quality', 'speedtest', 'settings', 'log'] as const

export function getPanelDirection(from: string, to: string): number {
  const fromIdx = PANEL_ORDER.indexOf(from as (typeof PANEL_ORDER)[number])
  const toIdx = PANEL_ORDER.indexOf(to as (typeof PANEL_ORDER)[number])
  if (fromIdx === -1 || toIdx === -1) return 1
  return toIdx > fromIdx ? 1 : -1
}

export function createPanelAppleVariants(easing: EasingConfig) {
  return {
    initial: { y: 8, opacity: 0.9 },
    animate: {
      y: 0,
      opacity: 1,
      // dampingRatio ≈ 0.80（Apple HIG 推荐 0.7-0.9 自然轻微弹性区间）
      transition: { type: 'spring' as const, stiffness: 320, damping: 24, mass: 0.7 },
    },
    exit: {
      y: -4,
      opacity: 0.9,
      scale: 0.99,
      // 0.04s：退出只是过渡提示，等待税越低切换越跟手（0.08s 时每刀固定多等 80ms）
      transition: { duration: 0.04, ease: easing.exit as [number, number, number, number] },
    },
  }
}
