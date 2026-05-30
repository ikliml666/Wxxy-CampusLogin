import * as React from 'react'
import gsap from 'gsap'
import { cn } from '@/lib/utils'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'

export interface AnimatedCardConfig {
  hoverY?: number
  glowIntensity?: number
  hoverScale?: number
  stiffness?: number
  damping?: number
  mass?: number
}

const DEFAULT_CONFIG: Required<AnimatedCardConfig> = {
  hoverY: -4,
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
  staggerIndex?: number
}

export const AnimatedCard = React.forwardRef<HTMLDivElement, AnimatedCardProps>(
  ({ animationConfig, className, noHover = false, noAnimation = false, noEnterAnimation = false, enableTilt, staggerIndex, children, ...props }, ref) => {
    const profile = useAnimationProfile()
    const [isHovered, setIsHovered] = React.useState(false)

    const tiltEnabled = (enableTilt !== undefined ? enableTilt : profile.enableTilt) && !noHover && !noAnimation
    const cardRef = React.useRef<HTMLDivElement>(null)
    const xQuick = React.useRef<gsap.QuickToFunc | null>(null)
    const yQuick = React.useRef<gsap.QuickToFunc | null>(null)

    React.useEffect(() => {
      if (!tiltEnabled || !cardRef.current) return
      const el = cardRef.current
      xQuick.current = gsap.quickTo(el, 'rotateY', { duration: 0.4, ease: 'power2.out', force3D: true })
      yQuick.current = gsap.quickTo(el, 'rotateX', { duration: 0.4, ease: 'power2.out', force3D: true })
      return () => {
        gsap.killTweensOf(el, 'rotateY')
        gsap.killTweensOf(el, 'rotateX')
        xQuick.current = null
        yQuick.current = null
      }
    }, [tiltEnabled])

    const handleMouseMove = React.useCallback((e: React.MouseEvent) => {
      if (!tiltEnabled || !xQuick.current || !yQuick.current) return
      const el = e.currentTarget as HTMLElement
      if (el.style.willChange !== 'transform') {
        el.style.willChange = 'transform'
      }
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
      gsap.delayedCall(0.4, () => {
        const el = cardRef.current
        if (el) el.style.willChange = ''
      })
    }, [])

    const config = React.useMemo(
      () => ({ ...DEFAULT_CONFIG, ...animationConfig }),
      [animationConfig]
    )

    const hoverY = noHover ? 0 : config.hoverY
    const restShadow = '0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)'
    const glowShadow = React.useMemo(() => {
      return isHovered && !noHover
        ? `0 0 ${10 * config.glowIntensity}px hsl(var(--primary) / 0.35), 0 0 ${30 * config.glowIntensity}px hsl(var(--primary) / 0.15), 0 0 ${60 * config.glowIntensity}px hsl(var(--primary) / 0.06), 0 ${12}px ${36}px rgba(0,0,0,0.08), inset 0 0 0 1px hsl(var(--primary) / 0.12)`
        : restShadow
    }, [isHovered, noHover, config.glowIntensity, restShadow])

    const cardClassName = React.useMemo(
      () => cn('bg-white text-card-foreground rounded-2xl dark:bg-[#14161b]', className),
      [className]
    )

    const showEntryAnim = !noEnterAnimation && !noAnimation && !window.matchMedia('(prefers-reduced-motion: reduce)').matches

    if (noAnimation) {
      return (
        <div ref={ref} className={cardClassName} style={{ boxShadow: restShadow }} {...props}>
          {children}
        </div>
      )
    }

    return (
      <div
        className={cn(
          'rounded-2xl card-hover-lift',
          showEntryAnim && 'card-enter',
        )}
        style={{
          '--stagger-i': staggerIndex ?? 0,
          '--hover-y': `${hoverY}px`,
          perspective: tiltEnabled ? 800 : undefined,
        } as React.CSSProperties}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => { setIsHovered(false); handleMouseLeave() }}
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
            cardClassName,
          )}
          style={{
            boxShadow: glowShadow,
            transformStyle: tiltEnabled ? 'preserve-3d' : undefined,
          }}
          {...props}
        >
          {children}
        </div>
      </div>
    )
  }
)
AnimatedCard.displayName = 'AnimatedCard'
