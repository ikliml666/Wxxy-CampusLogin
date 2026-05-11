export const containerVariants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: { staggerChildren: 0.12, delayChildren: 0.15 },
  },
}

export const itemVariants = {
  hidden: { opacity: 0, scale: 0.95 },
  visible: {
    opacity: 1,
    scale: 1,
    transition: { type: 'spring' as const, stiffness: 300, damping: 18, mass: 0.9 },
  },
}

export const panelSwitchVariants = {
  initial: { opacity: 0, scale: 0.96 },
  animate: {
    opacity: 1,
    scale: 1,
    transition: { type: 'spring' as const, stiffness: 400, damping: 25, mass: 0.8 },
  },
  exit: {
    opacity: 0,
    scale: 0.97,
    transition: { duration: 0.12, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  },
}

export const logEntryVariants = {
  initial: { opacity: 0, x: 30, height: 0, scaleY: 0 },
  animate: {
    opacity: 1,
    x: 0,
    height: 'auto',
    scaleY: 1,
    transition: { type: 'spring' as const, stiffness: 500, damping: 30 },
  },
  exit: {
    opacity: 0,
    x: -20,
    height: 0,
    scaleY: 0,
    transition: { duration: 0.2, ease: [0.4, 0, 1, 1] as [number, number, number, number] },
  },
}
