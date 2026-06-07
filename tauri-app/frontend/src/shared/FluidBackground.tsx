import { useRef, useEffect, useMemo, useCallback } from 'react'
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

  const gradientDuration = 36 * profile.orbDurationMultiplier
  const orb1Duration = 45 * profile.orbDurationMultiplier
  const orb2Duration = 60 * profile.orbDurationMultiplier

  // 统一暂停/恢复控制
  const setPaused = useCallback((shouldPause: boolean) => {
    const container = containerRef.current
    if (!container) return
    container.classList.toggle('fluid-paused', shouldPause)
  }, [])

  // 监听 paused prop 变化（含空闲暂停）
  useEffect(() => {
    setPaused(!!paused || !animActive)
  }, [paused, animActive, setPaused])

  // 监听容器上 fluid-paused 类的变化（由 useStartupBoost 控制）
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const observer = new MutationObserver(() => {
      // fluid-paused 类由外部（useStartupBoost）或内部（setPaused）控制
      // CSS animation-play-state: paused 已在 index.css 中通过 .fluid-paused 控制
    })

    observer.observe(container, {
      attributes: true,
      attributeFilter: ['class'],
    })

    return () => observer.disconnect()
  }, [])

  const gradientStyle = useMemo(
    () => ({
      width: `${profile.gradientScale * 100}%`,
      height: `${profile.gradientScale * 100}%`,
      left: 0,
      top: 0,
      willChange: 'transform' as const,
      animationDuration: `${gradientDuration}s`,
    }),
    [profile.gradientScale, gradientDuration],
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
      animationDuration: `${orb1Duration}s`,
    }),
    [orb1Duration],
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
      animationDuration: `${orb2Duration}s`,
      animationDelay: '3s',
    }),
    [orb2Duration],
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
        className="gradient-layer absolute fluid-gradient-anim"
        style={gradientStyle}
      />
      <div
        className="fluid-orb absolute rounded-full fluid-orb1-anim"
        style={orb1Style}
      />
      <div
        className="fluid-orb absolute rounded-full fluid-orb2-anim"
        style={orb2Style}
      />
      <div
        className="absolute inset-0"
        style={overlayStyle}
      />
    </div>
  )
}
