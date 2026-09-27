import { useEffect, useState, useRef } from 'react'
import { gsap } from 'gsap'
import { useAnimationActive } from './usePageIdle'

interface BreatheOptions {
  minOpacity?: number
  maxOpacity?: number
  duration?: number
  minScale?: number
  maxScale?: number
  minRotation?: number
  maxRotation?: number
}

/**
 * 回调 ref：条件渲染的元素卸载重建后（如 RightPanel 空态 空→非空→空），
 * tween 会针对新元素重新建立；旧实现 ref.current 只在 hook 挂载时读取一次，
 * 元素重建后动画静默失效（learnings/empty-state-breathe-after-remount）。
 */
export function useBreatheAnimation(options: BreatheOptions = {}) {
  const [node, setNode] = useState<HTMLDivElement | null>(null)
  const animActive = useAnimationActive()
  const tweenRef = useRef<gsap.core.Tween | null>(null)
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
    if (!node) return

    // Set initial state
    gsap.set(node, { opacity: maxOpacity, scale: maxScale, rotation: maxRotation, force3D: true })

    const tween = gsap.to(node, {
      opacity: minOpacity,
      scale: minScale,
      rotation: minRotation,
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
  }, [node, minOpacity, maxOpacity, duration, minScale, maxScale, minRotation, maxRotation])

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
