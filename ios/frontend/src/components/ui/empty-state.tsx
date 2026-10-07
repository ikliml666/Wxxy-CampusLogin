// EmptyState:统一空态基元——tonal 圆底图标 + 标题 + 描述 + 可选动作。
// compact 档用于卡内嵌入(更低高度);默认档用于整卡/整页空态。
// 设计纪律:无硬描边、tonal 底色、图标线性细描,内容居中不抢焦点。
import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface EmptyStateProps {
  icon: LucideIcon
  title: string
  description?: string
  action?: ReactNode
  /** 紧凑档:卡内嵌入,更小的图标与纵向留白 */
  compact?: boolean
  className?: string
}

export function EmptyState({ icon: Icon, title, description, action, compact = false, className }: EmptyStateProps) {
  return (
    <div className={cn('flex flex-col items-center justify-center text-center', compact ? 'gap-1.5 py-3' : 'gap-2 py-8', className)}>
      <div className={cn('rounded-2xl bg-primary/10 flex items-center justify-center', compact ? 'h-10 w-10' : 'h-14 w-14')}>
        <Icon className={cn('text-primary/70', compact ? 'h-5 w-5' : 'h-7 w-7')} strokeWidth={1.75} />
      </div>
      <p className={cn('font-medium text-foreground/80', compact ? 'text-xs' : 'text-sm')}>{title}</p>
      {description && (
        <p className={cn('text-muted-foreground/70 max-w-[280px]', compact ? 'text-[11px]' : 'text-xs')}>{description}</p>
      )}
      {action && <div className="mt-1.5">{action}</div>}
    </div>
  )
}
