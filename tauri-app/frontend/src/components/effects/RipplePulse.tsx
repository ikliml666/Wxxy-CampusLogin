import { useRef, useEffect } from 'react'
import { gsap } from 'gsap'
import { cn } from '@/lib/utils'
import { useAnimationActive } from '@/hooks/usePageIdle'

interface RipplePulseProps {
  active: boolean
  color?: string
  size?: number
  className?: string
}

export function RipplePulse({ active, color = 'currentColor', size = 24, className }: RipplePulseProps) {
  const containerRef = useRef<HTMLSpanElement>(null)
  const tlRef = useRef<gsap.core.Timeline | null>(null)
  const animActive = useAnimationActive()

  useEffect(() => {
    if (!containerRef.current || !active) {
      if (tlRef.current) {
        tlRef.current.kill()
        tlRef.current = null
      }
      return
    }

    const ctx = gsap.context(() => {
      if (!containerRef.current) return

      const ripples = containerRef.current.querySelectorAll('.ripple-ring')
      const tl = gsap.timeline({ repeat: -1 })

      ripples.forEach((ring, i) => {
        tl.fromTo(ring,
          { scale: 1, opacity: 0.6 },
          { scale: 3.5, opacity: 0, duration: 2, ease: 'sine.out', force3D: true },
          i * 0.7
        )
      })

      tlRef.current = tl

      if (!animActive) {
        tl.pause()
      }
    }, containerRef)

    return () => ctx.revert()
  }, [active])

  useEffect(() => {
    if (!tlRef.current) return
    if (animActive) {
      tlRef.current.play()
    } else {
      tlRef.current.pause()
    }
  }, [animActive])

  if (!active) return null

  return (
    <span
      ref={containerRef}
      className={cn('absolute flex items-center justify-center pointer-events-none', className)}
      style={{ width: size * 2, height: size * 2, left: -size * 0.5, top: -size * 0.5 }}
    >
      {[0, 1].map(i => (
        <span
          key={i}
          className="ripple-ring absolute rounded-full"
          style={{
            width: size,
            height: size,
            border: `2.5px solid ${color}`,
            willChange: 'transform, opacity',
          }}
        />
      ))}
    </span>
  )
}
