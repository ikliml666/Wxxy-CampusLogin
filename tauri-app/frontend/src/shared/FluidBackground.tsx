import { useRef, useEffect } from 'react'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'

interface FluidBackgroundProps {
  paused?: boolean
  innerRef?: (el: HTMLDivElement | null) => void
}

export function FluidBackground({ paused, innerRef }: FluidBackgroundProps) {
  const profile = useAnimationProfile()
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!containerRef.current) return
    const el = containerRef.current
    el.classList.toggle('fluid-paused', !!paused)
  }, [paused])

  const gradientDuration = 24 * profile.orbDurationMultiplier
  const orb1Duration = 30 * profile.orbDurationMultiplier
  const orb2Duration = 40 * profile.orbDurationMultiplier

  return (
    <div
      ref={(el) => {
        (containerRef as React.MutableRefObject<HTMLDivElement | null>).current = el
        innerRef?.(el)
      }}
      className="fixed inset-0 z-0 overflow-hidden pointer-events-none fluid-paused"
      style={{ background: 'var(--surface-main)', contain: 'strict' }}
    >
      <div
        className="gradient-layer absolute fluid-gradient-anim"
        style={{
          width: `${profile.gradientScale * 100}%`,
          height: `${profile.gradientScale * 100}%`,
          left: 0,
          top: 0,
          animationDuration: `${gradientDuration}s`,
          willChange: 'transform',
          backfaceVisibility: 'hidden',
        }}
      />
      <div
        className="fluid-orb absolute rounded-full fluid-orb1-anim"
        style={{
          width: 500,
          height: 500,
          background: `radial-gradient(circle, hsl(var(--primary) / 0.08) 0%, hsl(var(--primary) / 0.03) 35%, transparent 65%)`,
          opacity: 0.7,
          left: '10%',
          top: '10%',
          animationDuration: `${orb1Duration}s`,
          willChange: 'transform',
          backfaceVisibility: 'hidden',
          contain: 'strict',
        }}
      />
      <div
        className="fluid-orb absolute rounded-full fluid-orb2-anim"
        style={{
          width: 400,
          height: 400,
          background: `radial-gradient(circle, hsl(220 20% 92% / 0.5) 0%, hsl(220 20% 92% / 0.03) 35%, transparent 65%)`,
          opacity: 0.5,
          left: '10%',
          top: '10%',
          animationDuration: `${orb2Duration}s`,
          animationDelay: '3s',
          willChange: 'transform',
          backfaceVisibility: 'hidden',
          contain: 'strict',
        }}
      />
      <div
        className="absolute inset-0"
        style={{
          background: `linear-gradient(180deg, var(--surface-top) 0%, transparent 15%, transparent 85%, var(--surface-side) 100%)`,
        }}
      />
    </div>
  )
}
