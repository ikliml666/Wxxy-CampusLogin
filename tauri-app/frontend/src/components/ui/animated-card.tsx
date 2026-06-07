import * as React from 'react'
import gsap from 'gsap'
import { cn } from '@/lib/utils'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'

interface AnimatedCardConfig {
  glowIntensity?: number
  hoverScale?: number
  stiffness?: number
  damping?: number
  mass?: number
}

interface AnimatedCardProps extends React.HTMLAttributes<HTMLDivElement> {
  animationConfig?: AnimatedCardConfig
  noHover?: boolean
  noAnimation?: boolean
  noEnterAnimation?: boolean
  noRipple?: boolean
  enableTilt?: boolean
  staggerIndex?: number
}

const REST_SHADOW = '0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)'

export const AnimatedCard = React.memo(React.forwardRef<HTMLDivElement, AnimatedCardProps>(
  ({ animationConfig, className, noHover = false, noAnimation = false, noEnterAnimation = false, noRipple = false, enableTilt, staggerIndex, children, ...props }, ref) => {
    const profile = useAnimationProfile()
    const rippleTimerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null)

    const tiltEnabled = (enableTilt !== undefined ? enableTilt : profile.enableTilt) && !noHover && !noAnimation
    const cardRef = React.useRef<HTMLDivElement>(null)
    const xQuick = React.useRef<gsap.QuickToFunc | null>(null)
    const yQuick = React.useRef<gsap.QuickToFunc | null>(null)
    const tiltRafRef = React.useRef<number>(0)

    React.useEffect(() => {
      if (!tiltEnabled || !cardRef.current) return
      const el = cardRef.current
      xQuick.current = gsap.quickTo(el, 'rotateY', { duration: 0.35, ease: 'expo.out', force3D: true })
      yQuick.current = gsap.quickTo(el, 'rotateX', { duration: 0.35, ease: 'expo.out', force3D: true })
      return () => {
        gsap.killTweensOf(el, 'rotateY')
        gsap.killTweensOf(el, 'rotateX')
        xQuick.current = null
        yQuick.current = null
      }
    }, [tiltEnabled])

    const handleMouseMove = React.useCallback((e: React.MouseEvent) => {
      if (!tiltEnabled || !xQuick.current || !yQuick.current) return
      // RAF-throttle: only update once per frame
      cancelAnimationFrame(tiltRafRef.current)
      tiltRafRef.current = requestAnimationFrame(() => {
        const el = e.currentTarget as HTMLElement
        if (el.style.willChange !== 'transform') {
          el.style.willChange = 'transform'
        }
        const rect = el.getBoundingClientRect()
        const x = (e.clientX - rect.left) / rect.width - 0.5
        const y = (e.clientY - rect.top) / rect.height - 0.5
        xQuick.current?.(x * 8)
        yQuick.current?.(-y * 8)
      })
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

    const handleMouseDown = React.useCallback((e: React.MouseEvent) => {
      if (noRipple || noAnimation || noHover) return
      const el = e.currentTarget as HTMLElement
      const rect = el.getBoundingClientRect()
      const x = ((e.clientX - rect.left) / rect.width) * 100
      const y = ((e.clientY - rect.top) / rect.height) * 100
      el.style.setProperty('--ripple-x', `${x}%`)
      el.style.setProperty('--ripple-y', `${y}%`)
      el.classList.add('ripple-active')
      if (rippleTimerRef.current) clearTimeout(rippleTimerRef.current)
      rippleTimerRef.current = setTimeout(() => {
        el.classList.remove('ripple-active')
        rippleTimerRef.current = null
      }, 400)
    }, [noRipple, noAnimation, noHover])

    React.useEffect(() => {
      return () => {
        if (rippleTimerRef.current) clearTimeout(rippleTimerRef.current)
      }
    }, [])

    // hover 层 shadow - 静态计算，通过 CSS opacity 控制可见性，避免 box-shadow 过渡触发 paint
    // Apple 风格：仅微弱投影提升层次感，不发光
    const glowShadow = React.useMemo(() => {
      if (noHover) return ''
      return `0 4px 16px rgba(0,0,0,0.08), 0 1px 4px rgba(0,0,0,0.04)`
    }, [noHover])

    const cardClassName = React.useMemo(
      () => cn('bg-white text-card-foreground rounded-2xl dark:bg-[#14161b]', className),
      [className]
    )

    const showEntryAnim = !noEnterAnimation && !noAnimation && !window.matchMedia('(prefers-reduced-motion: reduce)').matches

    if (noAnimation) {
      return (
        <div ref={ref} className={cardClassName} style={{ boxShadow: REST_SHADOW }} {...props}>
          {children}
        </div>
      )
    }

    const showGlow = false

    return (
      <div className={showGlow ? 'card-glow-wrapper' : undefined}>
        {/* 发光层 - 独立于 overflow:hidden 之外，仅 opacity 过渡（合成层操作，零 paint） */}
        {showGlow && (
          <div
            className="card-glow-layer"
            style={{ boxShadow: glowShadow }}
          />
        )}
        <div
          className={cn(
            'rounded-2xl animated-card-interactive',
            showEntryAnim && 'card-enter',
          )}
          style={{
            '--stagger-i': staggerIndex ?? 0,
            perspective: tiltEnabled ? 800 : undefined,
          } as React.CSSProperties}
          onMouseDown={handleMouseDown}
          onMouseLeave={handleMouseLeave}
          onMouseMove={handleMouseMove}
        >
          <div
            ref={(node) => {
              (cardRef as React.MutableRefObject<HTMLDivElement | null>).current = node
              if (typeof ref === 'function') ref(node)
              else if (ref) (ref as React.MutableRefObject<HTMLDivElement | null>).current = node
            }}
            className={cardClassName}
            style={{
              boxShadow: REST_SHADOW,
              transformStyle: tiltEnabled ? 'preserve-3d' : undefined,
            }}
            {...props}
          >
            {children}
          </div>
        </div>
      </div>
    )
  }
))
AnimatedCard.displayName = 'AnimatedCard'
