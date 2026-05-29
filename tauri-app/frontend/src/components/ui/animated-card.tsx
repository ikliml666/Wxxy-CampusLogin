import * as React from 'react'
import { m, useReducedMotion } from 'framer-motion'
import { gsap } from 'gsap'
import { cn } from '@/lib/utils'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'

export interface AnimatedCardConfig {
  hoverY?: number
  shadow?: string
  restShadow?: string
  hoverScale?: number
  stiffness?: number
  damping?: number
  mass?: number
}

const DEFAULT_CONFIG: Required<AnimatedCardConfig> = {
  hoverY: -4,
  shadow:
    '0 12px 32px rgba(0,0,0,0.10), 0 4px 12px rgba(0,0,0,0.06)',
  restShadow:
    '0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)',
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
    const cardRef = React.useRef<HTMLDivElement>(null)
    const rafRef = React.useRef<number>(0)
    const rectCacheRef = React.useRef<DOMRect | null>(null)
    const xToRef = React.useRef<ReturnType<typeof gsap.quickTo> | null>(null)
    const yToRef = React.useRef<ReturnType<typeof gsap.quickTo> | null>(null)

    React.useEffect(() => {
      if (!cardRef.current || prefersReducedMotion) return
      xToRef.current = gsap.quickTo(cardRef.current, 'x', { duration: profile.magneticDuration, ease: 'power3.out' })
      yToRef.current = gsap.quickTo(cardRef.current, 'y', { duration: profile.magneticDuration, ease: 'power3.out' })
      return () => {
        xToRef.current?.(0)
        yToRef.current?.(0)
        xToRef.current = null
        yToRef.current = null
      }
    }, [prefersReducedMotion, profile.magneticDuration])

    React.useEffect(() => {
      const el = cardRef.current
      if (!el) return
      rectCacheRef.current = el.getBoundingClientRect()
      const ro = new ResizeObserver(() => {
        rectCacheRef.current = el.getBoundingClientRect()
      })
      ro.observe(el)
      return () => ro.disconnect()
    }, [])

    const config = React.useMemo(
      () => ({ ...DEFAULT_CONFIG, ...animationConfig }),
      [animationConfig]
    )

    const springConfig = React.useMemo(
      () => ({ stiffness: profile.springStiffness, damping: profile.springDamping, mass: config.mass }),
      [profile.springStiffness, profile.springDamping, config.mass]
    )

    const cardClassName = React.useMemo(
      () => cn('bg-white text-card-foreground rounded-2xl dark:bg-[#14161b]', className),
      [className]
    )

    const handleMouseMove = React.useCallback((e: React.MouseEvent) => {
      if (noHover || prefersReducedMotion) return
      const target = e.target as HTMLElement
      if (target.closest('input, textarea, select, button, [role="button"], [data-no-magnetic]')) {
        xToRef.current?.(0)
        yToRef.current?.(0)
        return
      }
      const el = cardRef.current
      if (!el) return
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
      rafRef.current = requestAnimationFrame(() => {
        const rect = rectCacheRef.current ?? el.getBoundingClientRect()
        if (!rectCacheRef.current) rectCacheRef.current = rect
        const cx = rect.left + rect.width / 2
        const cy = rect.top + rect.height / 2
        const dx = (e.clientX - cx) / (rect.width / 2)
        const dy = (e.clientY - cy) / (rect.height / 2)
        const maxOffset = profile.magneticOffset
        xToRef.current?.(dx * maxOffset)
        yToRef.current?.(dy * maxOffset)
      })
    }, [noHover, prefersReducedMotion, profile.magneticOffset])

    const handleMouseLeave = React.useCallback(() => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
      xToRef.current?.(0)
      yToRef.current?.(0)
      setIsHovered(false)
    }, [])

    if (prefersReducedMotion || noAnimation) {
      return (
        <div ref={ref} className={cardClassName} style={{ boxShadow: config.restShadow }} {...props}>
          {children}
        </div>
      )
    }

    return (
      <m.div
        className={cn('rounded-2xl')}
        initial={noEnterAnimation ? false : { opacity: 0, y: 20, scale: 0.97 }}
        animate={noEnterAnimation ? false : { opacity: 1, y: 0, scale: 1 }}
        transition={noEnterAnimation ? undefined : { type: 'spring', stiffness: profile.springStiffness, damping: profile.springDamping, mass: 0.7 }}
        whileHover={noHover ? undefined : {
          boxShadow: config.shadow,
          transition: { type: 'spring', ...springConfig },
        }}
        onAnimationComplete={() => setEntryDone(true)}
        style={{
          pointerEvents: entryDone ? undefined : ('none' as any),
        }}
        onHoverStart={() => setIsHovered(true)}
        onHoverEnd={() => setIsHovered(false)}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
      >
        <div
          ref={(node) => {
            (cardRef as React.MutableRefObject<HTMLDivElement | null>).current = node
            if (typeof ref === 'function') ref(node)
            else if (ref) (ref as React.MutableRefObject<HTMLDivElement | null>).current = node
          }}
          className={cn(
            'bg-white text-card-foreground rounded-2xl dark:bg-[#14161b] transition-shadow duration-200',
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
