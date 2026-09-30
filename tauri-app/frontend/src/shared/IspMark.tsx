import { memo } from 'react'
import { cn } from '@/lib/utils'

// 运营商后缀 → WebP 标识图（public/isp/，双端同构）；''=无锡学院（校徽）
const ISP_MARK_SRC: Record<string, string> = {
  __default__: '/isp/school.webp',
  '@telecom': '/isp/telecom.webp',
  '@unicom': '/isp/unicom.webp',
  '@cmcc': '/isp/mobile.webp',
}

export function ispMarkSrc(suffix: string | null | undefined): string | undefined {
  if (suffix === null || suffix === undefined) return undefined
  return ISP_MARK_SRC[suffix] ?? ISP_MARK_SRC.__default__
}

// 校徽在浅色底上不可读：固定 16px 白圆底承载；运营商 logo 原色直出
export const IspMark = memo(function IspMark({
  suffix,
  className,
}: {
  suffix: string | null | undefined
  className?: string
}) {
  const src = ispMarkSrc(suffix)
  if (!src) return null
  if (suffix === '' || suffix === '__default__') {
    return (
      <span
        className={cn(
          'inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-white ring-1 ring-black/10',
          className
        )}
      >
        <img src={src} alt="" aria-hidden="true" className="h-3 w-auto select-none" draggable={false} />
      </span>
    )
  }
  return (
    <img src={src} alt="" aria-hidden="true" className={cn('h-3.5 w-auto shrink-0 select-none', className)} draggable={false} />
  )
})
