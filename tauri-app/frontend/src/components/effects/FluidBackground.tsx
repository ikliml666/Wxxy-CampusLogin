import { useEffect, useRef } from 'react'
import { animate } from 'animejs'
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
  const animationsRef = useRef<ReturnType<typeof animate>[]>([])
  const isActive = useAnimationActive()

  useEffect(() => {
    if (!containerRef.current) return

    // 先清理旧动画，防止对象累积
    animationsRef.current.forEach(anim => {
      try { anim.pause() } catch {}
    })
    animationsRef.current = []

    const orbs = containerRef.current.querySelectorAll('.fluid-orb')

    orbs.forEach((orb, index) => {
      const config = ORBS[index]
      if (!config) return

      const anim = animate(orb, {
        translateX: config.x,
        translateY: config.y,
        scale: [0.8, 1.2, 0.9, 1.1],
        duration: config.duration,
        delay: config.delay,
        loop: true,
        direction: 'alternate',
        ease: 'easeInOutSine',
        autoplay: isActive,
      })

      animationsRef.current.push(anim)
    })

    return () => {
      animationsRef.current.forEach(anim => {
        try { anim.pause() } catch {}
      })
      animationsRef.current = []
    }
  }, [])

  useEffect(() => {
    animationsRef.current.forEach(anim => {
      if (isActive) {
        anim.play()
      } else {
        anim.pause()
      }
    })
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
        }

        @keyframes gradientShift {
          0%, 100% {
            background-position: 0% 50%;
          }
          50% {
            background-position: 100% 50%;
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
