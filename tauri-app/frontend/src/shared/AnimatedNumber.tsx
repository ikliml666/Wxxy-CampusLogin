import { useRef, useEffect, useCallback } from 'react'
import { gsap } from 'gsap'
import { useAnimationProfile } from '@/hooks/useAnimationProfile'

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
  duration,
  highlightColor: _highlightColor = 'var(--primary)',
}: AnimatedNumberProps) {
  const profile = useAnimationProfile()
  const resolvedDuration = duration ?? profile.numberDuration
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

      objRef.current.value = from

      const tl = gsap.timeline()
      tl.to(objRef.current, {
        value: to,
        duration: resolvedDuration / 1000,
        ease: 'power2.out',
        onUpdate: () => {
          if (ref.current) {
            ref.current.textContent = `${objRef.current.value.toFixed(decimals)}${unit}`
          }
        },
      }, 0)
      .to(ref.current, {
        keyframes: [
          { scale: 1.1, duration: (resolvedDuration / 1000) * 0.25, ease: 'power2.out' },
          { scale: 0.97, duration: (resolvedDuration / 1000) * 0.15, ease: 'power2.inOut' },
          { scale: 1, duration: (resolvedDuration / 1000) * 0.3, ease: 'elastic.out(1, 0.6)' },
        ],
        force3D: true,
      }, 0)
      .to(ref.current, {
        keyframes: [
          { opacity: 0.6, duration: (resolvedDuration / 1000) * 0.2, ease: 'power2.in' },
          { opacity: 1, duration: (resolvedDuration / 1000) * 0.3, ease: 'power2.out' },
        ],
        force3D: true,
      }, 0)
    }, ref)

    ctxRef.current = ctx
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
        willChange: profile.willChangeGradient ? 'transform, opacity' : undefined,
      }}
    >
      {value.toFixed(decimals)}{unit}
    </span>
  )
}
