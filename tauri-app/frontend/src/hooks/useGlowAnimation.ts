import { useEffect, useRef } from 'react'
import { gsap } from 'gsap'
import { useAnimationActive } from './usePageIdle'

interface GlowOptions {
  duration?: number
  maxScale?: number
  maxOpacity?: number
}

export function useGlowAnimation(options: GlowOptions = {}) {
  const ref = useRef<HTMLDivElement>(null)
  const animActive = useAnimationActive()
  const tweenRef = useRef<gsap.core.Tween | null>(null)
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
    tweenRef.current = tween

    return () => {
      tween.kill()
      tweenRef.current = null
    }
  }, [duration, maxScale, maxOpacity])

  // 空闲时暂停，活跃时恢复
  useEffect(() => {
    const tween = tweenRef.current
    if (!tween) return
    if (animActive) {
      tween.resume()
    } else {
      tween.pause()
    }
  }, [animActive])

  return ref
}
