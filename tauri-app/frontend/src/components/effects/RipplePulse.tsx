import { useEffect, useRef } from 'react'
import { animate } from 'animejs'
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
  const animationsRef = useRef<ReturnType<typeof animate>[]>([])
  const animActive = useAnimationActive()

  useEffect(() => {
    if (!containerRef.current || !active) {
      animationsRef.current.forEach(a => a.pause())
      animationsRef.current = []
      return
    }

    const ripples = containerRef.current.querySelectorAll('.ripple-ring')
    animationsRef.current = []

    ripples.forEach((ring, i) => {
      const anim = animate(ring, {
        scale: [1, 3.5],
        opacity: [0.6, 0],
        duration: 2000,
        delay: i * 700,
        loop: true,
        ease: 'easeOutSine',
        autoplay: animActive,
      })
      animationsRef.current.push(anim)
    })

    return () => {
      animationsRef.current.forEach(a => a.pause())
      animationsRef.current = []
    }
  }, [active])

  useEffect(() => {
    animationsRef.current.forEach(anim => {
      if (animActive) {
        anim.play()
      } else {
        anim.pause()
      }
    })
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
            willChange: animActive ? 'transform, opacity' : 'auto',
          }}
        />
      ))}
    </span>
  )
}
