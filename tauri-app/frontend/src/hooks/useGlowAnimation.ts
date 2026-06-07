import { useEffect, useRef } from 'react'
import { gsap } from 'gsap'

interface GlowOptions {
  duration?: number
  maxScale?: number
  maxOpacity?: number
}

export function useGlowAnimation(options: GlowOptions = {}) {
  const ref = useRef<HTMLDivElement>(null)
  const { duration = 4, maxScale = 1.15, maxOpacity = 0.6 } = options

  useEffect(() => {
    const el = ref.current
    if (!el) return

    // Set initial state
    gsap.set(el, { opacity: 0, scale: 1, force3D: true })

    const tween = gsap.to(el, {
      scale: maxScale,
      opacity: maxOpacity,
      duration: duration / 2,
      ease: 'sine.inOut',
      yoyo: true,
      repeat: -1,
      force3D: true,
    })

    return () => {
      tween.kill()
    }
  }, [duration, maxScale, maxOpacity])

  return ref
}
