import { Bell, BellOff, Palette, Info, Moon, Sun, ArrowUpCircle } from 'lucide-react'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { APP_VERSION } from '@/shared'
import { cn } from '@/lib/utils'
import { memo, useCallback, useRef } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { m } from 'framer-motion'

interface TitleBarProps {
  notificationEnabled: boolean
  isLightMode: boolean
  networkOnline: boolean
  networkQuality: 'excellent' | 'great' | 'good' | 'fair' | 'poor' | 'bad' | 'unknown'
  onToggleNotification: () => void
  onShowTheme: () => void
  onShowAbout: () => void
  onToggleLightMode: () => void
  onMinimize: () => void
  onToggleMaximize: () => void
  onClose: () => void
  isMaximized: boolean
  updateAvailable?: boolean
  latestVersion?: string
}

const MinimizeIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round">
    <line x1="2" y1="5" x2="8" y2="5" />
  </svg>
)

const MaximizeIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="2" y="2" width="6" height="6" rx="0.5" />
  </svg>
)

const RestoreIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="3" y="1" width="6" height="6" rx="0.5" />
    <path d="M3 3H2.5C2.22 3 2 3.22 2 3.5V8.5C2 8.78 2.22 9 2.5 9H7.5C7.78 9 8 8.78 8 8.5V8" />
  </svg>
)

const CloseIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
    <line x1="2" y1="2" x2="8" y2="8" />
    <line x1="8" y1="2" x2="2" y2="8" />
  </svg>
)

export const TitleBar = memo(function TitleBar({
  notificationEnabled,
  isLightMode,
  onToggleNotification,
  onShowTheme,
  onShowAbout,
  onToggleLightMode,
  onMinimize,
  onToggleMaximize,
  onClose,
  isMaximized,
  updateAvailable,
  latestVersion,
}: TitleBarProps) {
  const lastClickTimeRef = useRef(0)

  const handleTitleBarMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest('button, a, input, select, textarea, [role="button"]')) return

    const now = Date.now()
    const elapsed = now - lastClickTimeRef.current
    lastClickTimeRef.current = now

    if (elapsed < 300) return

    getCurrentWindow().startDragging().catch(() => {})
  }, [])

  const handleTitleBarDoubleClick = useCallback(() => {
    onToggleMaximize()
  }, [onToggleMaximize])

  return (
    <TooltipProvider delayDuration={300}>
      <div
        onMouseDown={handleTitleBarMouseDown}
        onDoubleClick={handleTitleBarDoubleClick}
        className="flex items-center justify-between h-11 px-5 shrink-0 select-none z-50 surface-top-square"
        style={{ background: 'var(--surface-top)' }}
      >
        <div className="flex items-center gap-3">
          <div
            className="w-7 h-7 bg-[#4f46e5] flex items-center justify-center rounded-full"
            style={{ boxShadow: '0 2px 8px rgba(79,70,229,0.3)' }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 2L2 7l10 5 10-5-10-5z" />
              <path d="M2 17l10 5 10-5" />
              <path d="M2 12l10 5 10-5" />
            </svg>
          </div>
          <span className="text-sm font-semibold tracking-tight">校园网登录助手</span>
          <span
            className={cn(
              "relative text-[10px] px-2 py-0.5 bg-[#f3f4f6] text-muted-foreground font-medium rounded-full dark:bg-[#1f2128]",
              updateAvailable && "cursor-pointer hover:bg-primary/10 hover:text-primary dark:hover:bg-primary/20 transition-colors"
            )}
            onClick={updateAvailable ? onShowAbout : undefined}
          >
            v{APP_VERSION}
            {updateAvailable && (
              <span className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-rose-500 rounded-full border-2 border-white dark:border-[#1f2128]" />
            )}
          </span>
          {updateAvailable && latestVersion && (
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  className="flex items-center gap-1 text-[10px] px-2 py-0.5 bg-emerald-500/10 text-emerald-600 font-medium rounded-full hover:bg-emerald-500/20 transition-colors"
                  onClick={onShowAbout}
                >
                  <ArrowUpCircle className="h-3 w-3" />
                  v{latestVersion}
                </button>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                <p>发现新版本 v{latestVersion}，点击查看</p>
              </TooltipContent>
            </Tooltip>
          )}
        </div>

        <div className="flex items-center gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <m.button
                onClick={onToggleLightMode}
                whileHover={{ scale: 1.15, rotate: isLightMode ? 15 : -15 }}
                whileTap={{ scale: 0.88 }}
                transition={{ type: 'spring', stiffness: 500, damping: 20 }}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                aria-label={isLightMode ? '切换到深色模式' : '切换到浅色模式'}
              >
                {isLightMode ? <Sun className="h-3.5 w-3.5 text-amber-500" /> : <Moon className="h-3.5 w-3.5 text-slate-400" />}
              </m.button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{isLightMode ? '切换到深色模式' : '切换到浅色模式'}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <m.button
                onClick={onToggleNotification}
                whileHover={{ scale: 1.15 }}
                whileTap={{ scale: 0.88 }}
                transition={{ type: 'spring', stiffness: 500, damping: 20 }}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                aria-label={notificationEnabled ? '通知已开启' : '通知已关闭'}
              >
                <m.div
                  key={notificationEnabled ? 'on' : 'off'}
                  initial={{ scale: 0.5, opacity: 0, rotate: -90 }}
                  animate={{ scale: 1, opacity: 1, rotate: 0 }}
                  exit={{ scale: 0.5, opacity: 0, rotate: 90 }}
                  transition={{ type: 'spring', stiffness: 400, damping: 20 }}
                >
                  {notificationEnabled ? <Bell className="h-3.5 w-3.5" /> : <BellOff className="h-3.5 w-3.5 text-muted-foreground" />}
                </m.div>
              </m.button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{notificationEnabled ? '通知已开启' : '通知已关闭'}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <m.button
                onClick={onShowTheme}
                whileHover={{ scale: 1.15, rotate: 20 }}
                whileTap={{ scale: 0.88 }}
                transition={{ type: 'spring', stiffness: 500, damping: 20 }}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                aria-label="主题设置"
              >
                <Palette className="h-3.5 w-3.5" />
              </m.button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>主题设置</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <m.button
                onClick={onShowAbout}
                whileHover={{ scale: 1.15 }}
                whileTap={{ scale: 0.88 }}
                transition={{ type: 'spring', stiffness: 500, damping: 20 }}
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                aria-label="关于"
              >
                <Info className="h-3.5 w-3.5" />
              </m.button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>关于</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                onClick={onMinimize}
                aria-label="最小化"
              >
                <MinimizeIcon />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>最小化</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                onClick={onToggleMaximize}
                aria-label={isMaximized ? '还原' : '最大化'}
              >
                {isMaximized ? <RestoreIcon /> : <MaximizeIcon />}
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{isMaximized ? '还原' : '最大化'}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className="h-7 w-7 rounded-full inline-flex items-center justify-center text-muted-foreground hover:bg-destructive/10 hover:text-destructive transition-colors"
                onClick={onClose}
                aria-label="关闭"
              >
                <CloseIcon />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>退出</p></TooltipContent>
          </Tooltip>
        </div>
      </div>
    </TooltipProvider>
  )
})
