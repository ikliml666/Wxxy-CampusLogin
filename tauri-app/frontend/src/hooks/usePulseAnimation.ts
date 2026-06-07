import { useEffect, useRef } from 'react'
import { gsap } from 'gsap'

interface PulseOptions {
  type: 'heartbeat' | 'statusPulse' | 'loadingPulse'
  duration?: number
}

/**
 * GSAP 驱动的脉冲/心跳动画 hook，替代 CSS @keyframes 实现。
 * - heartbeat: 模拟心跳节律 scale 1 -> 1.25 -> 1 -> 1.15 -> 1，3s 循环
 * - statusPulse: 模拟状态脉冲 scale 1 -> 1.1 + opacity 0.7，重复 2 次，1.5s
 * - loadingPulse: 模拟加载脉冲 opacity 1 -> 0.5 + scale 0.95，1.2s 循环
 */
export function usePulseAnimation(options: PulseOptions) {
  const ref = useRef<HTMLDivElement>(null)
  const { type, duration } = options

  useEffect(() => {
    const el = ref.current
    if (!el) return

    let timeline: gsap.core.Timeline

    switch (type) {
      case 'heartbeat': {
        // Mimics the heartbeat keyframe: scale 1 -> 1.25 -> 1 -> 1.15 -> 1, 3s loop
        timeline = gsap.timeline({ repeat: -1, repeatDelay: 0 })
        timeline.to(el, { scale: 1.25, duration: 0.4, ease: 'power2.out', force3D: true })
        timeline.to(el, { scale: 1, duration: 0.3, ease: 'power2.in', force3D: true })
        timeline.to(el, { scale: 1.15, duration: 0.2, ease: 'power2.out', force3D: true })
        timeline.to(el, { scale: 1, duration: 0.2, ease: 'power2.in', force3D: true })
        timeline.to(el, { duration: 1.9 }) // remaining time to fill 3s cycle
        break
      }
      case 'statusPulse': {
        // Mimics statusPulse: scale 1 -> 1.1 + opacity 0.7, repeat 2x, 1.5s total
        timeline = gsap.timeline({ repeat: -1, repeatDelay: 0 })
        timeline.to(el, { scale: 1.1, opacity: 0.7, duration: 0.2, force3D: true })
        timeline.to(el, { scale: 1, opacity: 1, duration: 0.2, force3D: true })
        timeline.to(el, { scale: 1.1, opacity: 0.7, duration: 0.2, force3D: true })
        timeline.to(el, { scale: 1, opacity: 1, duration: 0.2, force3D: true })
        timeline.to(el, { duration: 0.7 }) // remaining time
        break
      }
      case 'loadingPulse': {
        // Mimics loadingPulseScale: opacity 1 -> 0.5 + scale 0.95, 1.2s loop
        timeline = gsap.timeline({ repeat: -1 })
        timeline.to(el, { opacity: 0.5, scale: 0.95, duration: 0.6, ease: 'sine.inOut', force3D: true })
        timeline.to(el, { opacity: 1, scale: 1, duration: 0.6, ease: 'sine.inOut', force3D: true })
        break
      }
    }

    return () => {
      timeline?.kill()
    }
  }, [type, duration])

  return ref
}
