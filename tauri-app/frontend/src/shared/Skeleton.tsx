import { cn } from '@/lib/utils'

interface SkeletonProps {
  variant?: 'circle' | 'rect' | 'text'
  width?: number | string
  height?: number | string
  className?: string
  lines?: number
}

export function Skeleton({ variant = 'rect', width, height, className, lines }: SkeletonProps) {
  if (variant === 'text' && lines) {
    return (
      <div className={cn('space-y-2', className)}>
        {Array.from({ length: lines }).map((_, i) => (
          <div
            key={i}
            className="skeleton-shimmer rounded-lg"
            style={{
              width: i === lines - 1 ? '60%' : '100%',
              height: 14,
            }}
          />
        ))}
      </div>
    )
  }

  return (
    <div
      className={cn(
        'skeleton-shimmer',
        variant === 'circle' ? 'rounded-full' : 'rounded-xl',
        className
      )}
      style={{
        width: width ?? (variant === 'circle' ? 40 : '100%'),
        height: height ?? (variant === 'circle' ? 40 : 20),
      }}
    />
  )
}

export function CardSkeleton() {
  return (
    <div className="bg-white dark:bg-[#14161b] rounded-2xl p-6 space-y-4">
      <div className="flex items-center gap-3">
        <Skeleton variant="circle" width={40} height={40} />
        <div className="space-y-2 flex-1">
          <Skeleton variant="rect" height={16} width="40%" />
          <Skeleton variant="rect" height={12} width="60%" />
        </div>
      </div>
      <div className="space-y-3">
        <Skeleton variant="rect" height={44} />
        <Skeleton variant="rect" height={44} />
      </div>
    </div>
  )
}
