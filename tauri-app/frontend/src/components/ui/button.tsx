import * as React from 'react'
import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'
import { useRipple } from '@/hooks/useRipple'

const buttonVariants = cva(
  'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg text-sm font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50',
  {
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground shadow-sm hover:bg-primary/90 hover:shadow-md',
        destructive: 'bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90',
        outline: 'border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground',
        secondary: 'bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80',
        ghost: 'hover:bg-accent hover:text-accent-foreground',
        link: 'text-primary underline-offset-4 hover:underline',
        glass: 'glass text-foreground hover:bg-white/80 dark:hover:bg-slate-800/80',
        soft: 'soft-btn bg-muted/80 text-foreground hover:bg-muted',
      },
      size: {
        default: 'h-9 px-4 py-2',
        sm: 'h-8 rounded-md px-3 text-xs',
        lg: 'h-10 rounded-md px-6',
        icon: 'h-9 w-9',
        'icon-sm': 'h-7 w-7',
        'icon-lg': 'h-10 w-10',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  }
)

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean
  isLoading?: boolean
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, isLoading = false, children, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button'

    if (asChild) {
      return (
        <Comp
          className={cn(buttonVariants({ variant, size, className }))}
          ref={ref}
          disabled={props.disabled || isLoading}
          {...props}
        >
          {isLoading ? (
            <>
              <svg
                className="animate-spin h-4 w-4"
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
              >
                <circle
                  className="opacity-25"
                  cx="12"
                  cy="12"
                  r="10"
                  stroke="currentColor"
                  strokeWidth="4"
                />
                <path
                  className="opacity-75"
                  fill="currentColor"
                  d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                />
              </svg>
              {children}
            </>
          ) : (
            children
          )}
        </Comp>
      )
    }

    const { onDrag, onDragCapture, onDragEnd, onDragEndCapture, onDragEnter,
      onDragEnterCapture, onDragExit, onDragExitCapture, onDragLeave, onDragLeaveCapture,
      onDragOver, onDragOverCapture, onDragStart, onDragStartCapture, onDrop, onDropCapture,
      onAnimationStart, onAnimationStartCapture, onAnimationEnd, onAnimationEndCapture,
      onAnimationIteration, onAnimationIterationCapture,
      ...motionProps } = props as Record<string, unknown>

    const ripple = useRipple()

    // RAF 节流 + 位置去抖用 ref
    const rafRef = React.useRef<number>(0)
    const lastPosRef = React.useRef<{ x: number; y: number }>({ x: -999, y: -999 })

    return (
      <button
        className={cn(buttonVariants({ variant, size, className }), 'btn-press')}
        ref={(node) => {
          ripple.ref(node)
          if (typeof ref === 'function') ref(node)
          else if (ref) (ref as React.MutableRefObject<HTMLButtonElement | null>).current = node
        }}
        disabled={props.disabled || isLoading}
        onMouseDown={ripple.createRipple}
        onMouseMove={(e) => {
          const currentX = e.clientX
          const currentY = e.clientY
          const last = lastPosRef.current

          // 位置去抖：位移小于3px则跳过
          if (Math.abs(currentX - last.x) < 3 && Math.abs(currentY - last.y) < 3) return

          // 更新上次位置（像素坐标）
          lastPosRef.current = { x: currentX, y: currentY }

          // RAF 节流：同一帧内只执行一次 DOM 读写
          if (rafRef.current) cancelAnimationFrame(rafRef.current)
          rafRef.current = requestAnimationFrame(() => {
            const rect = e.currentTarget.getBoundingClientRect()
            const px = ((currentX - rect.left) / rect.width) * 100
            const py = ((currentY - rect.top) / rect.height) * 100
            e.currentTarget.style.setProperty('--mouse-x', `${px}%`)
            e.currentTarget.style.setProperty('--mouse-y', `${py}%`)
            rafRef.current = 0
          })
        }}
        {...motionProps}
      >
        {isLoading ? (
          <>
            <svg
              className="animate-spin h-4 w-4"
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle
                className="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
              />
            </svg>
            {children}
          </>
        ) : (
          children
        )}
      </button>
    )
  }
)
Button.displayName = 'Button'

export { Button, buttonVariants }
