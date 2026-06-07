import { useRef, useEffect, useMemo, useCallback } from 'react'
import { gsap } from 'gsap'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { useAnimationActive } from '@/hooks/usePageIdle'

interface FluidBackgroundProps {
  paused?: boolean
  innerRef?: (el: HTMLDivElement | null) => void
}

export function FluidBackground({ paused, innerRef }: FluidBackgroundProps) {
  const profile = useAnimationProfile()
  const animActive = useAnimationActive()
  const containerRef = useRef<HTMLDivElement>(null)
  const gradientRef = useRef<HTMLDivElement>(null)
  const orb1Ref = useRef<HTMLDivElement>(null)
  const orb2Ref = useRef<HTMLDivElement>(null)
  const tweensRef = useRef<gsap.core.Tween[]>([])

  const gradientDuration = 36 * profile.orbDurationMultiplier
  const orb1Duration = 45 * profile.orbDurationMultiplier
  const orb2Duration = 60 * profile.orbDurationMultiplier

  // 统一暂停/恢复控制
  const setTweensPaused = useCallback((shouldPause: boolean) => {
    tweensRef.current.forEach((t) => {
      if (shouldPause) {
        t.pause()
      } else {
        t.resume()
      }
    })
  }, [])

  // 初始化 GSAP 动画
  const initAnimations = useCallback(() => {
    // 清理旧的 tween
    tweensRef.current.forEach((t) => t.kill())
    tweensRef.current = []

    const gradientEl = gradientRef.current
    const orb1El = orb1Ref.current
    const orb2El = orb2Ref.current

    if (!gradientEl || !orb1El || !orb2El) return

    // gradient: translate3d(0%, -20%, 0) -> translate3d(-30%, -20%, 0)
    const gradientTween = gsap.fromTo(
      gradientEl,
      { xPercent: 0, yPercent: -20 },
      {
        xPercent: -30,
        yPercent: -20,
        duration: gradientDuration,
        ease: 'power1.inOut',
        repeat: -1,
        yoyo: true,
        force3D: true,
        lazy: true,
      },
    )

    // orb1: translate3d(-20%, -15%, 0) scale3d(0.85,0.85,1) -> translate3d(60%, 50%, 0) scale3d(1.15,1.15,1)
    const orb1Tween = gsap.fromTo(
      orb1El,
      { xPercent: -20, yPercent: -15, scale: 0.85 },
      {
        xPercent: 60,
        yPercent: 50,
        scale: 1.15,
        duration: orb1Duration,
        ease: 'power1.inOut',
        repeat: -1,
        yoyo: true,
        force3D: true,
        lazy: true,
      },
    )

    // orb2: translate3d(50%, 35%, 0) scale3d(0.85,0.85,1) -> translate3d(-15%, -20%, 0) scale3d(1.15,1.15,1)
    const orb2Tween = gsap.fromTo(
      orb2El,
      { xPercent: 50, yPercent: 35, scale: 0.85 },
      {
        xPercent: -15,
        yPercent: -20,
        scale: 1.15,
        duration: orb2Duration,
        ease: 'power1.inOut',
        repeat: -1,
        yoyo: true,
        force3D: true,
        lazy: true,
        delay: 3,
      },
    )

    tweensRef.current = [gradientTween, orb1Tween, orb2Tween]
  }, [gradientDuration, orb1Duration, orb2Duration])

  // 创建/重建动画
  useEffect(() => {
    initAnimations()
    return () => {
      tweensRef.current.forEach((t) => t.kill())
      tweensRef.current = []
    }
  }, [initAnimations])

  // 监听 paused prop 变化（含空闲暂停）
  useEffect(() => {
    setTweensPaused(!!paused || !animActive)
  }, [paused, animActive, setTweensPaused])

  // 监听容器上 fluid-paused 类的变化（由 useStartupBoost 控制）
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const observer = new MutationObserver(() => {
      const isPaused = container.classList.contains('fluid-paused')
      setTweensPaused(isPaused)
    })

    observer.observe(container, {
      attributes: true,
      attributeFilter: ['class'],
    })

    return () => observer.disconnect()
  }, [setTweensPaused])

  const gradientStyle = useMemo(
    () => ({
      width: `${profile.gradientScale * 100}%`,
      height: `${profile.gradientScale * 100}%`,
      left: 0,
      top: 0,
      willChange: 'transform' as const,
    }),
    [profile.gradientScale],
  )

  const orb1Style = useMemo(
    () => ({
      width: 500,
      height: 500,
      background: `radial-gradient(circle, hsl(var(--primary) / 0.08) 0%, hsl(var(--primary) / 0.03) 35%, transparent 65%)`,
      opacity: 0.7,
      left: '10%',
      top: '10%',
      willChange: 'transform' as const,
    }),
    [],
  )

  const orb2Style = useMemo(
    () => ({
      width: 400,
      height: 400,
      background: `radial-gradient(circle, hsl(220 20% 92% / 0.5) 0%, hsl(220 20% 92% / 0.03) 35%, transparent 65%)`,
      opacity: 0.5,
      left: '10%',
      top: '10%',
      willChange: 'transform' as const,
    }),
    [],
  )

  const rootStyle = useMemo(
    () =>
      ({
        background: 'var(--surface-main)',
        contain: 'strict',
      }) as React.CSSProperties,
    [],
  )

  const overlayStyle = useMemo(
    () => ({
      background: `linear-gradient(180deg, var(--surface-top) 0%, transparent 15%, transparent 85%, var(--surface-side) 100%)`,
    }),
    [],
  )

  return (
    <div
      ref={(el) => {
        (containerRef as React.MutableRefObject<HTMLDivElement | null>).current = el
        innerRef?.(el)
      }}
      className="fixed inset-0 z-0 overflow-hidden pointer-events-none fluid-paused"
      style={rootStyle}
    >
      <div
        ref={gradientRef}
        className="gradient-layer absolute"
        style={gradientStyle}
      />
      <div
        ref={orb1Ref}
        className="fluid-orb absolute rounded-full"
        style={orb1Style}
      />
      <div
        ref={orb2Ref}
        className="fluid-orb absolute rounded-full"
        style={orb2Style}
      />
      <div
        className="absolute inset-0"
        style={overlayStyle}
      />
    </div>
  )
}
