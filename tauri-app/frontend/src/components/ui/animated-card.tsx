import * as React from 'react'
import { m, useReducedMotion } from 'framer-motion'
import { cn } from '@/lib/utils'

export interface AnimatedCardConfig {
  y?: number
  shadow?: string
  restShadow?: string
  scale?: number
  stiffness?: number
  damping?: number
  mass?: number
  shadowDuration?: number
}

const DEFAULT_CONFIG: Required<AnimatedCardConfig> = {
  y: -6,
  shadow:
    '0 16px 48px rgba(0,0,0,0.10), 0 4px 16px rgba(0,0,0,0.06)',
  restShadow:
    '0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)',
  scale: 1.02,
  stiffness: 400,
  damping: 25,
  mass: 0.8,
  shadowDuration: 0.3,
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
    const [isHovered, setIsHovered] = React.useState(false)

    const config = React.useMemo(
      () => ({ ...DEFAULT_CONFIG, ...animationConfig }),
      [animationConfig]
    )

    const springConfig = React.useMemo(
      () => ({ stiffness: config.stiffness, damping: config.damping, mass: config.mass }),
      [config.stiffness, config.damping, config.mass]
    )

    const cardClassName = React.useMemo(
      () => cn('bg-white text-card-foreground rounded-2xl dark:bg-[#14161b]', className),
      [className]
    )

    if (prefersReducedMotion || noAnimation) {
      return (
        <div ref={ref} className={cardClassName} style={{ boxShadow: config.restShadow }} {...props}>
          {children}
        </div>
      )
    }

    return (
      <m.div
        className={cn('rounded-2xl', className)}
        initial={noEnterAnimation ? false : { y: 30, opacity: 0, scale: 0.92 }}
        animate={noEnterAnimation ? false : { y: 0, opacity: 1, scale: 1 }}
        transition={noEnterAnimation ? undefined : { type: 'spring', stiffness: 260, damping: 18, mass: 1.1 }}
        whileHover={noHover ? undefined : {
          y: config.y,
          scale: config.scale,
          boxShadow: config.shadow,
          transition: { type: 'spring', ...springConfig },
        }}
        onHoverStart={() => setIsHovered(true)}
        onHoverEnd={() => setIsHovered(false)}
      >
        <div
          ref={ref}
          className={cn(
            'bg-white text-card-foreground rounded-2xl dark:bg-[#14161b] transition-shadow',
            isHovered ? '' : ''
          )}
          style={{ boxShadow: isHovered ? config.shadow : config.restShadow }}
          {...props}
        >
          {children}
        </div>
      </m.div>
    )
  }
)
AnimatedCard.displayName = 'AnimatedCard'
