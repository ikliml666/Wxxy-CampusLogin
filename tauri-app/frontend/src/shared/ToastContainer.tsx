import { AnimatePresence, m } from 'framer-motion'
import type { ToastMessage } from '@/shared'
import { CheckCircle2, AlertCircle, Info, AlertTriangle, X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { memo } from 'react'

interface ToastContainerProps {
  toasts: ToastMessage[]
  onRemove: (id: string) => void
}

const TOAST_ICONS = {
  info: Info,
  success: CheckCircle2,
  error: AlertCircle,
  warning: AlertTriangle,
}

const TOAST_STYLES = {
  info: 'bg-background/95',
  success: 'bg-emerald-50/95 dark:bg-emerald-950/40',
  error: 'bg-red-50/95 dark:bg-red-950/40',
  warning: 'bg-amber-50/95 dark:bg-amber-950/40',
}

const TOAST_ICON_COLORS = {
  info: 'text-blue-500',
  success: 'text-emerald-500',
  error: 'text-red-500',
  warning: 'text-amber-500',
}

export const ToastContainer = memo(function ToastContainer({ toasts, onRemove }: ToastContainerProps) {
  return (
    <div className="fixed top-[84px] left-4 z-[100] flex flex-col gap-2 pointer-events-none" aria-live="polite" role="status">
      <AnimatePresence mode="popLayout">
        {toasts.map((toast) => {
          const Icon = TOAST_ICONS[toast.type as keyof typeof TOAST_ICONS] ?? Info
          return (
            <m.div
              key={`toast-${toast.id}`}
              initial={{ opacity: 0, x: -100, scale: 0.9 }}
              animate={{ opacity: 1, x: 0, scale: 1, transition: { type: 'spring', stiffness: 400, damping: 25, mass: 0.8 } }}
              exit={{ opacity: 0, x: -80, scale: 0.9, transition: { duration: 0.2, ease: [0.4, 0, 1, 1] } }}
              className={cn(
                'pointer-events-auto flex items-start gap-3 w-80 p-4 rounded-xl',
                'shadow-[0_4px_20px_rgba(0,0,0,0.08),0_1px_4px_rgba(0,0,0,0.04)]',
                TOAST_STYLES[toast.type]
              )}
            >
              <m.div
                initial={{ rotate: -20, scale: 0.5 }}
                animate={{ rotate: 0, scale: 1 }}
                transition={{ type: 'spring', stiffness: 500, damping: 20, delay: 0.1 }}
              >
                <Icon className={cn('h-5 w-5 shrink-0 mt-0.5', TOAST_ICON_COLORS[toast.type])} />
              </m.div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium">{toast.title}</p>
                {toast.description && (
                  <p className="text-xs text-muted-foreground mt-0.5">{toast.description}</p>
                )}
                {toast.action && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="mt-2 h-7 text-xs btn-physical"
                    onClick={toast.action.onClick}
                  >
                    {toast.action.label}
                  </Button>
                )}
              </div>
              <Button
                variant="ghost"
                size="icon-sm"
                className="shrink-0 -mr-1 -mt-1 btn-physical"
                onClick={() => onRemove(toast.id)}
                aria-label="关闭"
              >
                <X className="h-3.5 w-3.5" />
              </Button>
            </m.div>
          )
        })}
      </AnimatePresence>
    </div>
  )
})
