import { Bell, BellOff, Palette, Info, Moon, Sun } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { APP_VERSION } from '@/constants'
import { memo, useCallback } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'

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
  onClose: () => void
}

const MinimizeIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round">
    <line x1="2" y1="5" x2="8" y2="5" />
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
  onClose,
}: TitleBarProps) {
  const handleDragMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest('button, a, input, select, textarea, [role="button"]')) return
    getCurrentWindow().startDragging().catch(() => {})
  }, [])

  const handleDoubleClick = useCallback(() => {
    getCurrentWindow().toggleMaximize().catch(() => {})
  }, [])

  return (
    <TooltipProvider delayDuration={300}>
      <div
        data-tauri-drag-region
        onMouseDown={handleDragMouseDown}
        onDoubleClick={handleDoubleClick}
        className="flex items-center justify-between h-11 px-5 shrink-0 select-none z-50 surface-top-square"
        style={{ background: 'var(--surface-top)' }}
      >
        <div className="flex items-center gap-3" data-tauri-drag-region>
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
          <span className="text-sm font-semibold tracking-tight" data-tauri-drag-region>校园网登录助手</span>
          <span
            className="text-[10px] px-2 py-0.5 bg-[#f3f4f6] text-muted-foreground font-medium rounded-full dark:bg-[#1f2128]"
            data-tauri-drag-region
          >
            v{APP_VERSION}
          </span>
        </div>

        <div className="flex items-center gap-1" data-tauri-drag-region="false">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-sm" className="h-7 w-7 rounded-full" onClick={onToggleLightMode}>
                {isLightMode ? <Sun className="h-3.5 w-3.5 text-amber-500" /> : <Moon className="h-3.5 w-3.5 text-slate-400" />}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{isLightMode ? '切换到深色模式' : '切换到浅色模式'}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-sm" className="h-7 w-7 rounded-full" onClick={onToggleNotification}>
                {notificationEnabled ? <Bell className="h-3.5 w-3.5" /> : <BellOff className="h-3.5 w-3.5 text-muted-foreground" />}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>{notificationEnabled ? '通知已开启' : '通知已关闭'}</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-sm" className="h-7 w-7 rounded-full" onClick={onShowTheme}>
                <Palette className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom"><p>主题设置</p></TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon-sm" className="h-7 w-7 rounded-full" onClick={onShowAbout}>
                <Info className="h-3.5 w-3.5" />
              </Button>
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
