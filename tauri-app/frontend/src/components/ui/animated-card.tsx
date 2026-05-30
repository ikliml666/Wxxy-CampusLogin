import * as React from 'react'
import { m, useReducedMotion } from 'framer-motion'
import gsap from 'gsap'
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
  glowColor: 'rgba(99, 102, 241, 0.18)',
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
  enableTilt?: boolean
}

export const AnimatedCard = React.forwardRef<HTMLDivElement, AnimatedCardProps>(
  ({ animationConfig, className, noHover = false, noAnimation = false, noEnterAnimation = false, enableTilt, children, ...props }, ref) => {
    const prefersReducedMotion = useReducedMotion()
    const profile = useAnimationProfile()
    const [isHovered, setIsHovered] = React.useState(false)
    const [entryDone, setEntryDone] = React.useState(noEnterAnimation)

    const tiltEnabled = (enableTilt !== undefined ? enableTilt : profile.enableTilt) && !noHover && !prefersReducedMotion && !noAnimation
    const cardRef = React.useRef<HTMLDivElement>(null)
    const xQuick = React.useRef<gsap.QuickToFunc | null>(null)
    const yQuick = React.useRef<gsap.QuickToFunc | null>(null)

    React.useEffect(() => {
      if (!tiltEnabled || !cardRef.current) return
      const el = cardRef.current
      xQuick.current = gsap.quickTo(el, 'rotateY', { duration: 0.4, ease: 'power2.out' })
      yQuick.current = gsap.quickTo(el, 'rotateX', { duration: 0.4, ease: 'power2.out' })
      return () => {
        gsap.killTweensOf(el, 'rotateY')
        gsap.killTweensOf(el, 'rotateX')
        xQuick.current = null
        yQuick.current = null
      }
    }, [tiltEnabled])

    const handleMouseMove = React.useCallback((e: React.MouseEvent) => {
      if (!tiltEnabled || !xQuick.current || !yQuick.current) return
      const rect = e.currentTarget.getBoundingClientRect()
      const x = (e.clientX - rect.left) / rect.width - 0.5
      const y = (e.clientY - rect.top) / rect.height - 0.5
      xQuick.current(x * 8)
      yQuick.current(-y * 8)
    }, [tiltEnabled])

    const handleMouseLeave = React.useCallback(() => {
      if (!xQuick.current || !yQuick.current) return
      xQuick.current(0)
      yQuick.current(0)
    }, [])

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
      ? `0 0 ${24 * config.glowIntensity}px ${config.glowColor}, 0 0 ${48 * config.glowIntensity}px ${config.glowColor.replace(/[\d.]+\)$/, '0.08)')}, 0 ${10}px ${30}px rgba(0,0,0,0.08), inset 0 0 0 1px ${config.glowColor.replace(/[\d.]+\)$/, '0.12)')}`
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
          perspective: tiltEnabled ? 800 : undefined,
        }}
        onHoverStart={() => setIsHovered(true)}
        onHoverEnd={() => { setIsHovered(false); handleMouseLeave() }}
        onMouseMove={handleMouseMove}
      >
        <div
          ref={(node) => {
            (cardRef as React.MutableRefObject<HTMLDivElement | null>).current = node
            if (typeof ref === 'function') ref(node)
            else if (ref) (ref as React.MutableRefObject<HTMLDivElement | null>).current = node
          }}
          className={cn(
            'bg-white text-card-foreground rounded-2xl transition-shadow duration-300 dark:bg-[#14161b]',
          )}
          style={{
            boxShadow: glowShadow,
            transformStyle: tiltEnabled ? 'preserve-3d' : undefined,
          }}
          {...props}
        >
          {children}
        </div>
      </m.div>
    )
  }
)
AnimatedCard.displayName = 'AnimatedCard'
