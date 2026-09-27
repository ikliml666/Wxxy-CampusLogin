import * as React from 'react'
import { Label } from '@/components/ui/label'
import { cn } from '@/lib/utils'

interface SettingRowProps extends React.HTMLAttributes<HTMLDivElement> {
  /** 左侧主标签文案 */
  label: React.ReactNode
  /** 主标签下方的辅助说明，省略时不占位 */
  description?: React.ReactNode
  /** 标签前的可选小图标（应为 h-3.5 w-3.5 尺寸） */
  icon?: React.ReactNode
  /** 透传给 Label 的 htmlFor，配合控件 id 实现点击标签聚焦 */
  htmlFor?: string
}

/**
 * 标签左、控件右的标准参数行。统一 label（text-sm medium）、描述（text-xs muted）、
 * 行间距（gap-4）与窄屏截断（min-w-0 + truncate），替代各面板手写的
 * flex justify-between 参数行。分隔线由调用方按需插 Separator。
 */
export const SettingRow = React.forwardRef<HTMLDivElement, SettingRowProps>(
  ({ label, description, icon, htmlFor, className, children, ...props }, ref) => (
    <div ref={ref} className={cn('flex items-center justify-between gap-4', className)} {...props}>
      <div className="min-w-0 space-y-0.5">
        <Label htmlFor={htmlFor} className={cn('text-sm font-medium flex items-center gap-1.5', htmlFor && 'cursor-pointer')}>
          {icon}
          <span className="truncate">{label}</span>
        </Label>
        {description ? <p className="text-xs text-muted-foreground leading-snug">{description}</p> : null}
      </div>
      <div className="shrink-0 flex items-center gap-2">{children}</div>
    </div>
  )
)
SettingRow.displayName = 'SettingRow'
