import { useRef, useEffect, useCallback } from 'react'
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
  const objRef = useRef({ value })
  const ctxRef = useRef<gsap.Context | null>(null)

  const animateValue = useCallback((from: number, to: number) => {
    if (!ref.current) return

    if (ctxRef.current) {
      ctxRef.current.revert()
    }

    const ctx = gsap.context(() => {
      if (!ref.current) return

      gsap.killTweensOf(ref.current)
      gsap.killTweensOf(objRef.current)

      objRef.current.value = from

      gsap.to(objRef.current, {
        value: to,
        duration: duration / 1000,
        ease: 'power2.out',
        onUpdate: () => {
          if (ref.current) {
            ref.current.textContent = `${objRef.current.value.toFixed(decimals)}${unit}`
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
    }, ref)

    ctxRef.current = ctx
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
      if (ctxRef.current) {
        ctxRef.current.revert()
        ctxRef.current = null
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
