import { useEffect, useRef } from 'react'
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

export function FluidBackground() {
  const containerRef = useRef<HTMLDivElement>(null)
  const tlRef = useRef<gsap.core.Timeline | null>(null)
  const isActive = useAnimationActive()

  useEffect(() => {
    if (!containerRef.current) return

    if (tlRef.current) {
      tlRef.current.kill()
    }

    const orbs = containerRef.current.querySelectorAll('.fluid-orb')
    const tl = gsap.timeline({ repeat: -1, yoyo: true })

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

    return () => {
      tl.kill()
      tlRef.current = null
    }
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
        className={`absolute inset-0 gradient-shift ${isActive ? '' : 'anim-paused'}`}
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
            willChange: isActive ? 'transform' : 'auto',
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

      <style>{`
        .gradient-shift {
          background: linear-gradient(
            135deg,
            hsl(210, 30%, 95%) 0%,
            hsl(220, 25%, 94%) 25%,
            hsl(250, 20%, 93%) 50%,
            hsl(230, 22%, 94%) 75%,
            hsl(215, 28%, 95%) 100%
          );
          animation: gradientShift 16s ease-in-out infinite;
          background-size: 200% 200%;
          will-change: transform;
        }

        @keyframes gradientShift {
          0%, 100% {
            transform: translateX(0);
          }
          50% {
            transform: translateX(-50%);
          }
        }

        .anim-paused {
          animation-play-state: paused !important;
        }

        .dark .gradient-shift {
          background: linear-gradient(
            135deg,
            hsl(220, 15%, 12%) 0%,
            hsl(230, 18%, 10%) 25%,
            hsl(260, 15%, 11%) 50%,
            hsl(240, 16%, 10%) 75%,
            hsl(225, 17%, 12%) 100%
          );
        }
      `}</style>
    </div>
  )
}
