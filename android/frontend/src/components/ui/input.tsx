import * as React from 'react'
import { cn } from '@/lib/utils'

export interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  icon?: React.ReactNode
  error?: string
}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, icon, error, ...props }, ref) => {
    return (
      <div className="relative w-full">
        {icon && (
          <div className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground/50 pointer-events-none">
            {icon}
          </div>
        )}
        <input
          type={type}
          className={cn(
            // 基础边框 + 卡面色填充：浅色下 muted 填充与弹窗底（--background）明度几乎相同，
            // 只画边框仍显得「糊成一片」；用 --card（浅色纯白）填底，配合加深的 --input 边框，
            // 输入区边界才一眼可辨（深色下 --card 比底亮一档，同样成立）。
            'flex h-10 w-full rounded-lg border border-input bg-card px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground/50 hover:border-input/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:border-ring disabled:cursor-not-allowed disabled:opacity-50 transition-colors duration-150',
            icon && 'pl-10',
            // error 态覆盖基础边框色
            error && 'border-destructive focus-visible:ring-destructive',
            className
          )}
          ref={ref}
          {...props}
        />
        {error && (
          <p className="text-xs text-destructive mt-1">{error}</p>
        )}
      </div>
    )
  }
)
Input.displayName = 'Input'

export { Input }
