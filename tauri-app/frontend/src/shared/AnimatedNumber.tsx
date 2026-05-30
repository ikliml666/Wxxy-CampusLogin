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
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (!ref.current) return
    const el = ref.current
    valueQuickToRef.current = gsap.quickTo(objRef.current, 'value', {
      duration: resolvedDuration / 1000,
      ease: 'power2.out',
      onUpdate: () => {
        if (ref.current) {
          ref.current.textContent = `${objRef.current.value.toFixed(decimals)}${unit}`
        }
      },
    })
    scaleQuickToRef.current = gsap.quickTo(el, 'scale', {
      duration: resolvedDuration / 1000 * 0.55,
      ease: 'elastic.out(1, 0.6)',
      force3D: true,
    })
    return () => {
      if (resetTimerRef.current) clearTimeout(resetTimerRef.current)
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
      scaleQuickToRef.current(1.08)
      resetTimerRef.current = setTimeout(() => {
        scaleQuickToRef.current?.(1)
        resetTimerRef.current = null
      }, resolvedDuration * 0.2)

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
