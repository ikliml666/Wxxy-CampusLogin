import * as React from 'react'
import { RefreshCw, Check } from 'lucide-react'
import { cn } from '@/lib/utils'
import { m, AnimatePresence } from 'framer-motion'

interface RefreshButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  isRefreshing: boolean
  iconClassName?: string
  showCheck?: boolean
}

const RefreshButton = React.forwardRef<HTMLButtonElement, RefreshButtonProps>(
  ({ isRefreshing, iconClassName, showCheck, className, children, 'aria-label': ariaLabel, ...props }, ref) => {
    const prevRefreshing = React.useRef(isRefreshing)
    const [shakeClass, setShakeClass] = React.useState('')

    React.useEffect(() => {
      if (prevRefreshing.current && !isRefreshing) {
        setShakeClass('refresh-shake')
        const timer = setTimeout(() => setShakeClass(''), 350)
        return () => clearTimeout(timer)
      }
      prevRefreshing.current = isRefreshing
    }, [isRefreshing])

    return (
      <button
        ref={ref}
        aria-label={ariaLabel ?? '刷新'}
        className={cn(
          'p-1.5 rounded-xl hover:bg-accent text-muted-foreground hover:text-foreground',
          'transition-colors duration-200',
          isRefreshing && 'opacity-50 cursor-not-allowed',
          shakeClass,
          className
        )}
        {...props}
      >
        <span className="inline-flex items-center justify-center relative">
          <RefreshCw
            className={cn(
              'transition-transform duration-300',
              !isRefreshing && 'hover:animate-refresh-hover',
              isRefreshing && 'animate-refresh-spin',
              iconClassName ?? 'h-3 w-3'
            )}
          />
          <AnimatePresence>
            {showCheck && !isRefreshing && (
              <m.span
                key="check"
                className="absolute inset-0 flex items-center justify-center text-emerald-500"
                initial={{ opacity: 0, scale: 0.3 }}
                animate={{ opacity: 1, scale: [0.3, 1.3, 1] }}
                exit={{ opacity: 0, scale: 0.5 }}
                transition={{ duration: 0.4 }}
              >
                <Check className={iconClassName ?? 'h-3 w-3'} strokeWidth={3} />
              </m.span>
            )}
          </AnimatePresence>
        </span>
        {children}
      </button>
    )
  }
)
RefreshButton.displayName = 'RefreshButton'

function getRefreshIconClass(isRefreshing: boolean, iconClassName?: string) {
  return cn(
    'transition-transform duration-300',
    !isRefreshing && 'group-hover:rotate-180 duration-500',
    isRefreshing && 'animate-refresh-spin',
    iconClassName ?? 'h-3.5 w-3.5'
  )
}

export { RefreshButton, getRefreshIconClass }
