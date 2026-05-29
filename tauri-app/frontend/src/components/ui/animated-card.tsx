import * as React from 'react'
import { m, useReducedMotion } from 'framer-motion'
import { cn } from '@/lib/utils'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'

export interface AnimatedCardConfig {
  hoverY?: number
  glowColor?: string
  glowIntensity?: number
  hoverScale?: number
  stiffness?: number
  damping?: number
  mass?: number
}

const DEFAULT_CONFIG: Required<AnimatedCardConfig> = {
  hoverY: -3,
  glowColor: 'rgba(59, 130, 246, 0.12)',
  glowIntensity: 1,
  hoverScale: 1,
  stiffness: 300,
  damping: 20,
  mass: 0.8,
}

export interface AnimatedCardProps extends React.HTMLAttributes<HTMLDivElement> {
  animationConfig?: AnimatedCardConfig
  noHover?: boolean
  noAnimation?: boolean
  noEnterAnimation?: boolean
}

export const AnimatedCard = React.forwardRef<HTMLDivElement, AnimatedCardProps>(
  ({ animationConfig, className, noHover = false, noAnimation = false, noEnterAnimation = false, children, ...props }, ref) => {
    const prefersReducedMotion = useReducedMotion()
    const profile = useAnimationProfile()
    const [isHovered, setIsHovered] = React.useState(false)
    const [entryDone, setEntryDone] = React.useState(noEnterAnimation)

    const config = React.useMemo(
      () => ({ ...DEFAULT_CONFIG, ...animationConfig }),
      [animationConfig]
    )

    const springConfig = React.useMemo(
      () => ({ stiffness: profile.springStiffness, damping: profile.springDamping, mass: config.mass }),
      [profile.springStiffness, profile.springDamping, config.mass]
    )

    const hoverY = noHover ? 0 : config.hoverY
    const restShadow = '0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)'
    const glowShadow = isHovered && !noHover
      ? `0 0 ${20 * config.glowIntensity}px ${config.glowColor}, 0 0 ${40 * config.glowIntensity}px ${config.glowColor.replace(/[\d.]+\)$/, '0.06)')}, 0 ${8}px ${24}px rgba(0,0,0,0.06)`
      : restShadow

    const cardClassName = React.useMemo(
      () => cn('bg-white text-card-foreground rounded-2xl dark:bg-[#14161b]', className),
      [className]
    )

    if (prefersReducedMotion || noAnimation) {
      return (
        <div ref={ref} className={cardClassName} style={{ boxShadow: restShadow }} {...props}>
          {children}
        </div>
      )
    }

    return (
      <m.div
        className={cn('rounded-2xl')}
        initial={noEnterAnimation ? false : { opacity: 0, y: 20, scale: 0.97 }}
        animate={noEnterAnimation ? false : { opacity: 1, y: 0, scale: 1 }}
        transition={noEnterAnimation ? undefined : { type: 'spring', ...springConfig }}
        whileHover={noHover ? undefined : {
          y: hoverY,
          transition: { type: 'spring', ...springConfig },
        }}
        onAnimationComplete={() => setEntryDone(true)}
        style={{
          pointerEvents: entryDone ? undefined : ('none' as any),
        }}
        onHoverStart={() => setIsHovered(true)}
        onHoverEnd={() => setIsHovered(false)}
      >
        <div
          ref={ref}
          className={cn(
            'bg-white text-card-foreground rounded-2xl transition-shadow duration-300 dark:bg-[#14161b]',
          )}
          style={{ boxShadow: glowShadow }}
          {...props}
        >
          {children}
        </div>
      </m.div>
    )
  }
)
AnimatedCard.displayName = 'AnimatedCard'
