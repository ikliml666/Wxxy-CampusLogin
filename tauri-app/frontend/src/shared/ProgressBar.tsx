import { useRef, useEffect, memo } from 'react'
import { gsap } from 'gsap'
import { cn } from '@/lib/utils'

interface ProgressBarProps {
  value: number
  max?: number
  className?: string
  height?: number
  animated?: boolean
}

export const ProgressBar = memo(function ProgressBar({
  value,
  max = 100,
  className,
  height = 4,
  animated = true,
}: ProgressBarProps) {
  const barRef = useRef<HTMLDivElement>(null)
  const prevValueRef = useRef(value)

  useEffect(() => {
    if (!animated || !barRef.current) return

    const progress = Math.min(Math.max(value / max, 0), 1)
    const prevProgress = Math.min(Math.max(prevValueRef.current / max, 0), 1)

    if (progress !== prevProgress) {
      const ctx = gsap.context(() => {
        gsap.fromTo(barRef.current,
          { scaleX: prevProgress },
          {
            scaleX: progress,
            duration: 0.8,
            ease: 'elastic.out(1, 0.6)',
            force3D: true,
          }
        )
      }, barRef)
      prevValueRef.current = value
      return () => ctx.revert()
    }
  }, [value, max, animated])

  const progress = Math.min(Math.max(value / max, 0), 1)

  return (
    <div
      className={cn('w-full bg-muted/40 rounded-full overflow-hidden', className)}
      style={{ height }}
    >
      <div
        ref={barRef}
        className="h-full bg-primary rounded-full origin-left"
        style={{
          transform: animated ? undefined : `scaleX(${progress})`,
          width: animated ? '100%' : `${progress * 100}%`,
        }}
      />
    </div>
  )
})

interface IndeterminateBarProps {
  className?: string
  height?: number
}

export function IndeterminateBar({ className, height = 4 }: IndeterminateBarProps) {
  return (
    <div
      className={cn('w-full bg-muted/40 rounded-full overflow-hidden', className)}
      style={{ height }}
    >
      <div
        className="h-full rounded-full skeleton-shimmer"
        style={{
          width: '40%',
          background: `linear-gradient(90deg, transparent 0%, hsl(var(--primary) / 0.4) 50%, transparent 100%)`,
          backgroundSize: '200% 100%',
        }}
      />
    </div>
  )
}
