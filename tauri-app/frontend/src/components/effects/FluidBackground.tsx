import { useRef, useEffect } from 'react'
import { gsap } from 'gsap'
import { useAnimationActive } from '@/hooks/usePageIdle'

interface OrbConfig {
  size: number
  color: string
  x: [string, string]
  y: [string, string]
  duration: number
  delay: number
  opacity: number
}

const ORBS: OrbConfig[] = [
  {
    size: 500,
    color: 'hsl(var(--primary) / 0.08)',
    x: ['-30%', '90%'],
    y: ['-20%', '70%'],
    duration: 20000,
    delay: 0,
    opacity: 0.7,
  },
  {
    size: 400,
    color: 'hsl(220, 20%, 92%)',
    x: ['70%', '-20%'],
    y: ['50%', '-30%'],
    duration: 26000,
    delay: 3000,
    opacity: 0.5,
  },
]

const GRADIENT_DURATION = 16

export function FluidBackground() {
  const containerRef = useRef<HTMLDivElement>(null)
  const tlRef = useRef<gsap.core.Timeline | null>(null)
  const isActive = useAnimationActive()

  useEffect(() => {
    if (!containerRef.current) return

    const ctx = gsap.context(() => {
      if (!containerRef.current) return

      const gradientEl = containerRef.current.querySelector('.gradient-layer')
      const orbs = containerRef.current.querySelectorAll('.fluid-orb')
      const tl = gsap.timeline({ repeat: -1, yoyo: true })

      if (gradientEl) {
        tl.fromTo(gradientEl,
          { xPercent: 0, yPercent: -25 },
          { xPercent: -50, yPercent: -25, duration: GRADIENT_DURATION, ease: 'sine.inOut', force3D: true },
          0
        )
      }

      orbs.forEach((orb, index) => {
        const config = ORBS[index]
        if (!config) return

        tl.fromTo(orb,
          { x: config.x[0], y: config.y[0], scale: 0.8 },
          { x: config.x[1], y: config.y[1], scale: 1.2, duration: config.duration / 1000, ease: 'sine.inOut', force3D: true },
          config.delay / 1000
        )
      })

      tlRef.current = tl

      if (!isActive) {
        tl.pause()
      }
    }, containerRef)

    return () => ctx.revert()
  }, [])

  useEffect(() => {
    if (!tlRef.current) return
    if (isActive) {
      tlRef.current.play()
    } else {
      tlRef.current.pause()
    }
  }, [isActive])

  return (
    <div
      ref={containerRef}
      className="fixed inset-0 z-0 overflow-hidden pointer-events-none"
      style={{ background: 'var(--surface-main)', contain: 'strict' }}
    >
      <div
        className="gradient-layer absolute"
        style={{
          width: '200%',
          height: '200%',
          left: 0,
          top: 0,
        }}
      />

      {ORBS.map((orb, index) => (
        <div
          key={index}
          className="fluid-orb absolute rounded-full"
          style={{
            width: orb.size,
            height: orb.size,
            background: `radial-gradient(circle, ${orb.color} 0%, ${orb.color.replace(/0\.\d+\)/, '0.03)')} 35%, transparent 65%)`,
            opacity: orb.opacity,
            left: '10%',
            top: '10%',
          }}
        />
      ))}

      <div
        className="absolute inset-0"
        style={{
          background: `linear-gradient(
            180deg,
            var(--surface-top) 0%,
            transparent 15%,
            transparent 85%,
            var(--surface-side) 100%
          )`,
        }}
      />
    </div>
  )
}
