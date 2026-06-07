import { useEffect, useRef } from 'react'
import { gsap } from 'gsap'

interface BreatheOptions {
  minOpacity?: number
  maxOpacity?: number
  duration?: number
  minScale?: number
  maxScale?: number
  minRotation?: number
  maxRotation?: number
}

export function useBreatheAnimation(options: BreatheOptions = {}) {
  const ref = useRef<HTMLDivElement>(null)
  const {
    minOpacity = 0.6,
    maxOpacity = 1,
    duration = 4,
    minScale = 1,
    maxScale = 1,
    minRotation = 0,
    maxRotation = 0,
  } = options

  useEffect(() => {
    const el = ref.current
    if (!el) return

    // Set initial state
    gsap.set(el, { opacity: maxOpacity, scale: maxScale, rotation: maxRotation, force3D: true })

    const tween = gsap.to(el, {
      opacity: minOpacity,
      scale: minScale,
      rotation: minRotation,
      duration: duration / 2,
      ease: 'sine.inOut',
      yoyo: true,
      repeat: -1,
      force3D: true,
    })

    return () => {
      tween.kill()
    }
  }, [minOpacity, maxOpacity, duration, minScale, maxScale, minRotation, maxRotation])

  return ref
}
