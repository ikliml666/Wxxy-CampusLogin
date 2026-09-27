import { useEffect, useState, useRef } from 'react'
import { gsap } from 'gsap'
import { useAnimationActive } from './usePageIdle'

interface GlowOptions {
  duration?: number
  maxScale?: number
  maxOpacity?: number
}

/**
 * 回调 ref：条件渲染（如质量劣化提示光晕 poor→ok→poor）的元素卸载重建后
 * 重新建立 tween，避免动画静默失效（同 useBreatheAnimation 的修复模式）。
 */
export function useGlowAnimation(options: GlowOptions = {}) {
  const [node, setNode] = useState<HTMLDivElement | null>(null)
  const animActive = useAnimationActive()
  const tweenRef = useRef<gsap.core.Tween | null>(null)
  const { duration = 4, maxScale = 1.15, maxOpacity = 0.6 } = options

  useEffect(() => {
    if (!node) return

    // Set initial state
    gsap.set(node, { opacity: 0, scale: 1, force3D: true })

    const tween = gsap.to(node, {
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
  }, [node, duration, maxScale, maxOpacity])

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

  return setNode
}
