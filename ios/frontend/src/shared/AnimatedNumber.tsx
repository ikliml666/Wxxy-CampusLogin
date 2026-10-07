import { useRef, useEffect } from 'react'
import { gsap } from 'gsap'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'
import { useAnimationActive } from '@/hooks/usePageIdle'

interface AnimatedNumberProps {
  value: number
  unit?: string
  decimals?: number
  className?: string
  duration?: number
}

export function AnimatedNumber({
  value,
  unit = 'ms',
  decimals = 0,
  className = '',
  duration,
}: AnimatedNumberProps) {
  const profile = useAnimationProfile()
  const animActive = useAnimationActive()
  const resolvedDuration = duration ?? profile.numberDuration
  const ref = useRef<HTMLSpanElement>(null)
  const prevRef = useRef(value)
  const isFirstRender = useRef(true)
  const objRef = useRef({ value })
  const valueQuickToRef = useRef<gsap.QuickToFunc | null>(null)
  const scaleQuickToRef = useRef<gsap.QuickToFunc | null>(null)
  const resetTimerRef = useRef<gsap.core.Tween | null>(null)

  useEffect(() => {
    if (!ref.current) return
    const el = ref.current
    valueQuickToRef.current = gsap.quickTo(objRef.current, 'value', {
      duration: resolvedDuration / 1000,
      ease: 'expo.out',
      onUpdate: () => {
        if (ref.current) {
          ref.current.textContent = `${objRef.current.value.toFixed(decimals)}${unit}`
        }
      },
    })
    scaleQuickToRef.current = gsap.quickTo(el, 'scale', {
      duration: resolvedDuration / 1000 * 0.55,
      ease: 'expo.out',
      force3D: true,
    })
    return () => {
      if (resetTimerRef.current) { resetTimerRef.current.kill(); resetTimerRef.current = null }
      // 历史缺陷：quickTo 底层 tween 在组件卸载后仍存活，持续占用 CPU 并尝试写
      // 已卸载的 DOM（ref.current 已为 null，onUpdate 有保护不报错但空转）。
      // cleanup 时 kill 底层 tween（objRef 的 value 属性 + el 的 scale 属性）。
      gsap.killTweensOf(objRef.current, 'value')
      gsap.killTweensOf(el, 'scale')
      valueQuickToRef.current = null
      scaleQuickToRef.current = null
    }
  }, [decimals, unit, resolvedDuration])

  useEffect(() => {
    if (isFirstRender.current) {
      isFirstRender.current = false
      prevRef.current = value
      if (ref.current) {
        ref.current.textContent = `${value.toFixed(decimals)}${unit}`
      }
      return
    }

    if (prevRef.current !== value) {
      if (!animActive || !valueQuickToRef.current || !scaleQuickToRef.current) {
        if (ref.current) {
          ref.current.textContent = `${value.toFixed(decimals)}${unit}`
        }
        prevRef.current = value
        return
      }

      objRef.current.value = prevRef.current
      valueQuickToRef.current(value)
      // economy 档禁用 scale 弹跳，仅做数字滚动（降级一致性）
      if (profile.tier !== 'economy') {
        scaleQuickToRef.current(1.08)
        // 历史缺陷：delayedCall 覆盖前不 kill 旧 call，快速连续变化时多个
        // 回调堆积，中途中把 scale 弹回 1 造成跳变。改为先取消再注册。
        if (resetTimerRef.current) resetTimerRef.current.kill()
        resetTimerRef.current = gsap.delayedCall(resolvedDuration * 0.2 / 1000, () => {
          scaleQuickToRef.current?.(1)
          resetTimerRef.current = null
        })
      }

      prevRef.current = value
    }
  }, [value, decimals, unit, resolvedDuration, animActive])

  return (
    <span
      ref={ref}
      className={className}
      style={{
        fontVariantNumeric: 'tabular-nums',
        display: 'inline-block',
      }}
    >
      {value.toFixed(decimals)}{unit}
    </span>
  )
}
