import { useEffect, useRef, useCallback } from 'react'

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
  duration = 0.6,
}: AnimatedNumberProps) {
  const ref = useRef<HTMLSpanElement>(null)
  const prevRef = useRef(value)
  const isFirstRender = useRef(true)
  const rafRef = useRef<number>(0)

  const animate = useCallback((from: number, to: number, dur: number) => {
    if (rafRef.current) cancelAnimationFrame(rafRef.current)
    const start = performance.now()
    const durMs = dur * 1000

    const tick = (now: number) => {
      const elapsed = now - start
      const t = Math.min(elapsed / durMs, 1)
      const eased = 1 - Math.pow(1 - t, 3)
      const current = from + (to - from) * eased
      if (ref.current) {
        ref.current.textContent = `${current.toFixed(decimals)}${unit}`
      }
      if (t < 1) {
        rafRef.current = requestAnimationFrame(tick)
      } else {
        if (ref.current) {
          ref.current.textContent = `${to.toFixed(decimals)}${unit}`
        }
      }
    }
    rafRef.current = requestAnimationFrame(tick)
  }, [decimals, unit])

  useEffect(() => {
    if (isFirstRender.current) {
      isFirstRender.current = false
      prevRef.current = value
      if (ref.current) {
        ref.current.textContent = `${value.toFixed(decimals)}${unit}`
      }
      return
    }
    animate(prevRef.current, value, duration)
    prevRef.current = value
  }, [value, unit, decimals, duration, animate])

  useEffect(() => {
    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
    }
  }, [])

  return (
    <span
      ref={ref}
      className={className}
      style={{ fontVariantNumeric: 'tabular-nums' }}
    >
      {value.toFixed(decimals)}{unit}
    </span>
  )
}
