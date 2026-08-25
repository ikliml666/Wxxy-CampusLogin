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
  enableTilt?: boolean
  staggerIndex?: number
}

const REST_SHADOW = '0 1px 3px rgba(0,0,0,0.03), 0 1px 2px rgba(0,0,0,0.02)'

export const AnimatedCard = React.memo(React.forwardRef<HTMLDivElement, AnimatedCardProps>(
  ({ animationConfig, className, noHover = false, noAnimation = false, noEnterAnimation = false, enableTilt, staggerIndex, children, ...props }, ref) => {
    const profile = useAnimationProfile()
    const resetWillChangeTimerRef = React.useRef<gsap.core.Tween | null>(null)

    const tiltEnabled = (enableTilt !== undefined ? enableTilt : profile.enableTilt) && !noHover && !noAnimation
    const cardRef = React.useRef<HTMLDivElement>(null)
    const xQuick = React.useRef<gsap.QuickToFunc | null>(null)
    const yQuick = React.useRef<gsap.QuickToFunc | null>(null)
    const tiltRafRef = React.useRef<number>(0)
    const rectCacheRef = React.useRef<DOMRect | null>(null)

    React.useEffect(() => {
      if (!tiltEnabled || !cardRef.current) return
      const el = cardRef.current
      xQuick.current = gsap.quickTo(el, 'rotateY', { duration: 0.35, ease: 'expo.out', force3D: true })
      yQuick.current = gsap.quickTo(el, 'rotateX', { duration: 0.35, ease: 'expo.out', force3D: true })
      // Invalidate rect cache on resize
      const ro = new ResizeObserver(() => { rectCacheRef.current = null })
      ro.observe(el)
      return () => {
        ro.disconnect()
        rectCacheRef.current = null
        // 历史缺陷：tilt RAF 未取消，组件卸载后回调仍可能执行写已卸载节点。
        if (tiltRafRef.current) cancelAnimationFrame(tiltRafRef.current)
        // willChange 复位 delayedCall 也一并 kill，避免卸载后仍写 style
        if (resetWillChangeTimerRef.current) { resetWillChangeTimerRef.current.kill(); resetWillChangeTimerRef.current = null }
        gsap.killTweensOf(el, 'rotateY')
        gsap.killTweensOf(el, 'rotateX')
        xQuick.current = null
        yQuick.current = null
      }
    }, [tiltEnabled])

    const handleMouseMove = React.useCallback((e: React.MouseEvent) => {
      if (!tiltEnabled || !xQuick.current || !yQuick.current) return
      // RAF 回调内不能读取 e.currentTarget：React 在 handler 返回后立即将其置 null，
      // 因此这里同步捕获节点引用与光标坐标
      const el = e.currentTarget as HTMLElement
      const clientX = e.clientX
      const clientY = e.clientY
      // RAF-throttle: only update once per frame
      cancelAnimationFrame(tiltRafRef.current)
      tiltRafRef.current = requestAnimationFrame(() => {
        if (el.style.willChange !== 'transform') {
          el.style.willChange = 'transform'
        }
        // Use cached rect to avoid forced synchronous layout
        let rect = rectCacheRef.current
        if (!rect) {
          rect = el.getBoundingClientRect()
          rectCacheRef.current = rect
        }
        const x = (clientX - rect.left) / rect.width - 0.5
        const y = (clientY - rect.top) / rect.height - 0.5
        xQuick.current?.(x * 8)
        yQuick.current?.(-y * 8)
      })
    }, [tiltEnabled])

    const handleMouseLeave = React.useCallback(() => {
      if (!xQuick.current || !yQuick.current) return
      xQuick.current(0)
      yQuick.current(0)
      // 历史缺陷：delayedCall 不保存引用，组件卸载后回调仍执行（写已卸载节点）；
      // 连续 leave/enter 也会堆积多个回调。改为先取消旧的再注册，cleanup 时 kill。
      if (resetWillChangeTimerRef.current) resetWillChangeTimerRef.current.kill()
      resetWillChangeTimerRef.current = gsap.delayedCall(0.4, () => {
        resetWillChangeTimerRef.current = null
        const el = cardRef.current
        if (el) el.style.willChange = ''
      })
    }, [])

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

    return (
      <div
        className={cn(
          'rounded-2xl animated-card-interactive',
          showEntryAnim && 'card-enter',
        )}
        style={{
          '--stagger-i': staggerIndex ?? 0,
          perspective: tiltEnabled ? 800 : undefined,
        } as React.CSSProperties}
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
    )
  }
))
AnimatedCard.displayName = 'AnimatedCard'
