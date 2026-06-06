import type { EasingConfig } from './easing-config'
import { EASING_60HZ } from './easing-config'

export function createCardStaggerVariants(_easing: EasingConfig) {
  return {
    hidden: { opacity: 0 },
    visible: {
      opacity: 1,
      transition: { staggerChildren: 0.08, delayChildren: 0.05 },
    },
  }
}

export function createCardItemVariants(easing: EasingConfig) {
  return {
    hidden: { opacity: 0, y: 16 },
    visible: {
      opacity: 1,
      y: 0,
      transition: { duration: 0.5, ease: easing.smooth as [number, number, number, number] },
    },
  }
}

export function createPanelSwitchVariants(easing: EasingConfig) {
  return {
    initial: { opacity: 0 },
    animate: {
      opacity: 1,
      transition: { duration: 0.25, ease: easing.enter as [number, number, number, number] },
    },
    exit: {
      opacity: 0,
      transition: { duration: 0.15, ease: easing.exit as [number, number, number, number] },
    },
  }
}

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
  const fromIdx = PANEL_ORDER.indexOf(from as any)
  const toIdx = PANEL_ORDER.indexOf(to as any)
  if (fromIdx === -1 || toIdx === -1) return 1
  return toIdx > fromIdx ? 1 : -1
}

export function createPanelSlideVariants(easing: EasingConfig) {
  return {
    initial: (direction: number) => ({
      opacity: 0,
      x: direction > 0 ? 50 : -50,
    }),
    animate: {
      opacity: 1,
      x: 0,
      transition: { duration: 0.45, ease: easing.enter as [number, number, number, number] },
    },
    exit: (direction: number) => ({
      opacity: 0,
      x: direction > 0 ? -25 : 25,
      transition: { duration: 0.22, ease: easing.exit as [number, number, number, number] },
    }),
  }
}

export function createPanelFadeOnlyVariants(easing: EasingConfig) {
  return {
    initial: { opacity: 0 },
    animate: {
      opacity: 1,
      transition: { duration: 0.25, ease: easing.enter as [number, number, number, number] },
    },
    exit: {
      opacity: 0,
      transition: { duration: 0.12, ease: easing.exit as [number, number, number, number] },
    },
  }
}

// 默认导出（使用 60Hz 缓动，向后兼容）
export const cardStaggerVariants = createCardStaggerVariants(EASING_60HZ)
export const cardItemVariants = createCardItemVariants(EASING_60HZ)
export const panelSwitchVariants = createPanelSwitchVariants(EASING_60HZ)
export const logEntryVariants = createLogEntryVariants(EASING_60HZ)


export function createPanelAppleVariants(easing: EasingConfig) {
  return {
    initial: { y: 12 },
    animate: {
      y: 0,
      transition: { type: 'spring' as const, stiffness: 300, damping: 24, mass: 0.8 },
    },
    exit: {
      y: -4,
      scale: 0.99,
      transition: { duration: 0.08, ease: easing.exit as [number, number, number, number] },
    },
  }
}

export const panelAppleVariants = createPanelAppleVariants(EASING_60HZ)

export function createLogClearVariants(_easing: EasingConfig) {
  return {
    clear: {
      x: 50,
      opacity: 0,
      scaleX: 0.8,
      transition: {
        type: 'spring' as const,
        stiffness: 280,
        damping: 22,
        mass: 0.7,
      },
    },
  }
}

export const logClearVariants = createLogClearVariants(EASING_60HZ)
