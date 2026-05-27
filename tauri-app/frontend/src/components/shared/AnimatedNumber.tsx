import { useEffect, useRef, useCallback } from 'react'
import { gsap } from 'gsap'

interface AnimatedNumberProps {
  value: number
  unit?: string
  decimals?: number
  className?: string
  duration?: number
  highlightColor?: string
}

export function AnimatedNumber({
  value,
  unit = 'ms',
  decimals = 0,
  className = '',
  duration = 600,
  highlightColor = 'var(--primary)',
}: AnimatedNumberProps) {
  const ref = useRef<HTMLSpanElement>(null)
  const prevRef = useRef(value)
  const isFirstRender = useRef(true)
  const valueTweenRef = useRef<gsap.core.Tween | null>(null)
  const visualTlRef = useRef<gsap.core.Timeline | null>(null)
  const colorRef = useRef<string>('')

  useEffect(() => {
    if (ref.current) {
      colorRef.current = getComputedStyle(ref.current).color
    }
  }, [])

  const animateValue = useCallback((from: number, to: number) => {
    if (!ref.current) return

    const obj = { value: from }

    if (valueTweenRef.current) {
      valueTweenRef.current.kill()
    }
    if (visualTlRef.current) {
      visualTlRef.current.kill()
    }

    gsap.killTweensOf(ref.current)

    valueTweenRef.current = gsap.to(obj, {
      value: to,
      duration: duration / 1000,
      ease: 'power2.out',
      onUpdate: () => {
        if (ref.current) {
          ref.current.textContent = `${obj.value.toFixed(decimals)}${unit}`
        }
      },
    })

    const tl = gsap.timeline()
    tl.to(ref.current, {
      scale: 1.12,
      duration: (duration / 1000) * 0.3,
      ease: 'power2.out',
      force3D: true,
    })
    .to(ref.current, {
      scale: 0.96,
      duration: (duration / 1000) * 0.2,
      ease: 'power2.inOut',
    })
    .to(ref.current, {
      scale: 1,
      duration: (duration / 1000) * 0.3,
      ease: 'elastic.out(1, 0.6)',
    })

    gsap.to(ref.current, {
      color: highlightColor,
      duration: (duration / 1000) * 0.3,
      yoyo: true,
      repeat: 1,
      ease: 'power2.inOut',
    })

    visualTlRef.current = tl
  }, [decimals, unit, duration, highlightColor])

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
      animateValue(prevRef.current, value)
      prevRef.current = value
    }
  }, [value, decimals, unit, animateValue])

  useEffect(() => {
    return () => {
      if (valueTweenRef.current) {
        valueTweenRef.current.kill()
        valueTweenRef.current = null
      }
      if (visualTlRef.current) {
        visualTlRef.current.kill()
        visualTlRef.current = null
      }
    }
  }, [])

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
