import { useEffect, useRef, useCallback } from 'react'
import { animate } from 'animejs'

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
  const animationRef = useRef<ReturnType<typeof animate> | null>(null)
  const visualAnimRef = useRef<ReturnType<typeof animate> | null>(null)
  const colorRef = useRef<string>('')

  useEffect(() => {
    if (ref.current) {
      colorRef.current = getComputedStyle(ref.current).color
    }
  }, [])

  const animateValue = useCallback((from: number, to: number) => {
    if (!ref.current) return

    const obj = { value: from }
    const startColor = colorRef.current || (ref.current ? getComputedStyle(ref.current).color : '')

    if (animationRef.current) {
      animationRef.current.pause()
    }

    if (visualAnimRef.current) {
      visualAnimRef.current.pause()
    }

    animationRef.current = animate(obj, {
      value: to,
      duration: duration,
      easing: 'easeOutQuad',
      update: () => {
        if (ref.current) {
          ref.current.textContent = `${obj.value.toFixed(decimals)}${unit}`
        }
      },
    })

    visualAnimRef.current = animate(ref.current, {
      scale: [1, 1.12, 0.96, 1],
      color: [startColor, highlightColor, highlightColor, startColor],
      duration: duration * 0.8,
      easing: 'easeOutElastic(1, .6)',
    })
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
      if (animationRef.current) {
        animationRef.current.pause()
        animationRef.current = null
      }
      if (visualAnimRef.current) {
        visualAnimRef.current.pause()
        visualAnimRef.current = null
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
