import { useEffect, useRef } from 'react'
import gsap from 'gsap'

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
  const objRef = useRef({ val: value })
  const prevRef = useRef(value)
  const isFirstRender = useRef(true)

  useEffect(() => {
    if (!ref.current) return

    if (isFirstRender.current) {
      isFirstRender.current = false
      objRef.current.val = value
      prevRef.current = value
      if (ref.current) {
        ref.current.textContent = `${value.toFixed(decimals)}${unit}`
      }
      return
    }

    const from = prevRef.current
    const to = value
    prevRef.current = to

    objRef.current.val = from

    gsap.to(objRef.current, {
      val: to,
      duration,
      ease: 'power2.out',
      onUpdate: () => {
        if (ref.current) {
          ref.current.textContent = `${objRef.current.val.toFixed(decimals)}${unit}`
        }
      },
      onComplete: () => {
        if (ref.current) {
          ref.current.textContent = `${to.toFixed(decimals)}${unit}`
        }
      },
    })

    return () => {
      gsap.killTweensOf(objRef.current)
    }
  }, [value, unit, decimals, duration])

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
