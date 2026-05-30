export const cardStaggerVariants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: { staggerChildren: 0.05, delayChildren: 0.03 },
  },
}

export const cardItemVariants = {
  hidden: { opacity: 0, y: 16, scale: 0.96 },
  visible: {
    opacity: 1,
    y: 0,
    scale: 1,
    transition: { type: 'spring' as const, stiffness: 600, damping: 35, mass: 0.4 },
  },
}

export const panelSwitchVariants = {
  initial: { opacity: 0 },
  animate: {
    opacity: 1,
    transition: { duration: 0.15, ease: [0.25, 0.1, 0.25, 1] as [number, number, number, number] },
  },
  exit: {
    opacity: 0,
    transition: { duration: 0.08, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  },
}

export const logEntryVariants = {
  initial: { opacity: 0, x: 30, scaleY: 0, originY: 0 },
  animate: {
    opacity: 1,
    x: 0,
    scaleY: 1,
    originY: 0,
    transition: { type: 'spring' as const, stiffness: 500, damping: 30 },
  },
  exit: {
    opacity: 0,
    x: -20,
    scaleY: 0,
    originY: 0,
    transition: { duration: 0.2, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  },
}

export const PANEL_ORDER = ['dashboard', 'account', 'network', 'monitor', 'quality', 'speedtest', 'settings', 'log'] as const

export function getPanelDirection(from: string, to: string): number {
  const fromIdx = PANEL_ORDER.indexOf(from as any)
  const toIdx = PANEL_ORDER.indexOf(to as any)
  if (fromIdx === -1 || toIdx === -1) return 1
  return toIdx > fromIdx ? 1 : -1
}

export const panelSlideVariants = {
  initial: (direction: number) => ({
    opacity: 0,
    x: direction > 0 ? 80 : -80,
    scale: 0.96,
  }),
  animate: {
    opacity: 1,
    x: 0,
    scale: 1,
    transition: { type: 'spring' as const, stiffness: 400, damping: 30, mass: 0.8 },
  },
  exit: (direction: number) => ({
    opacity: 0,
    x: direction > 0 ? -40 : 40,
    scale: 0.98,
    transition: { duration: 0.15, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  }),
}

export const panelFadeOnlyVariants = {
  initial: { opacity: 0 },
  animate: {
    opacity: 1,
    transition: { duration: 0.15, ease: [0.25, 0.1, 0.25, 1] as [number, number, number, number] },
  },
  exit: {
    opacity: 0,
    transition: { duration: 0.08, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  },
}
