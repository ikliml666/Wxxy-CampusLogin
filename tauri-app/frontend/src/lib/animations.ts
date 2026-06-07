import type { EasingConfig } from './easing-config'
import { EASING_60HZ } from './easing-config'

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

export const logEntryVariants = createLogEntryVariants(EASING_60HZ)

export function createPanelAppleVariants(easing: EasingConfig) {
  return {
    initial: { y: 8, opacity: 0.9 },
    animate: {
      y: 0,
      opacity: 1,
      transition: { type: 'spring' as const, stiffness: 400, damping: 32, mass: 0.6 },
    },
    exit: {
      y: -4,
      opacity: 0.9,
      scale: 0.99,
      transition: { duration: 0.08, ease: easing.exit as [number, number, number, number] },
    },
  }
}
