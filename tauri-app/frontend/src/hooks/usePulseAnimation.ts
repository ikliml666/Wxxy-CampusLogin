import { useEffect, useState, useRef } from 'react'
import { gsap } from 'gsap'
import { useAnimationActive } from './usePageIdle'

interface PulseOptions {
  type: 'heartbeat' | 'statusPulse' | 'loadingPulse'
}

/**
 * GSAP 驱动的脉冲/心跳动画 hook，替代 CSS @keyframes 实现。
 * - heartbeat: 模拟心跳节律 scale 1 -> 1.25 -> 1 -> 1.15 -> 1，3s 循环
 * - statusPulse: 模拟状态脉冲 scale 1 -> 1.1 + opacity 0.7，重复 2 次，1.5s
 * - loadingPulse: 模拟加载脉冲 opacity 1 -> 0.5 + scale 0.95，1.2s 循环
 *
 * 回调 ref：条件渲染的元素卸载重建后重新建立 timeline（同 useBreatheAnimation）。
 */
export function usePulseAnimation(options: PulseOptions) {
  const [node, setNode] = useState<HTMLDivElement | null>(null)
  const animActive = useAnimationActive()
  const timelineRef = useRef<gsap.core.Timeline | null>(null)
  const { type } = options

  useEffect(() => {
    if (!node) return

    let timeline: gsap.core.Timeline

    switch (type) {
      case 'heartbeat': {
        timeline = gsap.timeline({ repeat: -1, repeatDelay: 0 })
        timeline.to(node, { scale: 1.25, duration: 0.4, ease: 'power2.out', force3D: true })
        timeline.to(node, { scale: 1, duration: 0.3, ease: 'power2.in', force3D: true })
        timeline.to(node, { scale: 1.15, duration: 0.2, ease: 'power2.out', force3D: true })
        timeline.to(node, { scale: 1, duration: 0.2, ease: 'power2.in', force3D: true })
        timeline.to(node, { duration: 1.9 })
        break
      }
      case 'statusPulse': {
        timeline = gsap.timeline({ repeat: -1, repeatDelay: 0 })
        timeline.to(node, { scale: 1.1, opacity: 0.7, duration: 0.2, force3D: true })
        timeline.to(node, { scale: 1, opacity: 1, duration: 0.2, force3D: true })
        timeline.to(node, { scale: 1.1, opacity: 0.7, duration: 0.2, force3D: true })
        timeline.to(node, { scale: 1, opacity: 1, duration: 0.2, force3D: true })
        timeline.to(node, { duration: 0.7 })
        break
      }
      case 'loadingPulse': {
        timeline = gsap.timeline({ repeat: -1 })
        timeline.to(node, { opacity: 0.5, scale: 0.95, duration: 0.6, ease: 'sine.inOut', force3D: true })
        timeline.to(node, { opacity: 1, scale: 1, duration: 0.6, ease: 'sine.inOut', force3D: true })
        break
      }
    }

    timelineRef.current = timeline

    return () => {
      timeline?.kill()
      timelineRef.current = null
    }
  }, [node, type])

  // 空闲时暂停，活跃时恢复
  useEffect(() => {
    const tl = timelineRef.current
    if (!tl) return
    if (animActive) {
      tl.resume()
    } else {
      tl.pause()
    }
  }, [animActive])

  return setNode
}
