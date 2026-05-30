import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Loader2, ExternalLink, HeadsetIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { memo, useRef, useEffect } from 'react'
import { RefreshButton } from '@/shared'
import { NetworkQualityCapsule } from '@/monitor'
import { useAppStore } from '@/hooks/useAppStore'

interface StatusBarProps {
  onOpenPortal: () => void
  onOpenSelfService?: () => void
}

export const StatusBar = memo(function StatusBar({ onOpenPortal, onOpenSelfService }: StatusBarProps) {
  const status = useAppStore((s) => s.status)
  const isRefreshingQuality = useAppStore((s) => s.isRefreshingQuality)
  const enableNetworkQuality = useAppStore((s) => s.config.enableNetworkQuality !== false)
  const refreshQuality = useAppStore((s) => s.refreshQuality)
  const networkQuality = useAppStore((s) => s.networkQuality)
  const statusText = status.text
  const statusState = status.state
  const prevStatusRef = useRef(statusState)
  const wasOffline = prevStatusRef.current === 'offline' && statusState !== 'offline'

  useEffect(() => {
    prevStatusRef.current = statusState
  }, [statusState])

  const statusConfig = {
    online: { color: 'text-emerald-500', dot: 'bg-emerald-500', bg: 'rgba(16, 185, 129, 0.12)' },
    offline: { color: 'text-rose-500', dot: 'bg-rose-500', bg: 'rgba(244, 63, 94, 0.12)' },
    loading: { color: 'text-blue-500', dot: 'bg-blue-500', bg: 'rgba(59, 130, 246, 0.12)' },
    error: { color: 'text-rose-500', dot: 'bg-rose-500', bg: 'rgba(244, 63, 94, 0.12)' },
    unknown: { color: 'text-amber-500', dot: 'bg-amber-500', bg: 'rgba(245, 158, 11, 0.12)' },
  }
  const cfg = statusConfig[statusState] ?? statusConfig.unknown

  return (
    <TooltipProvider delayDuration={300}>
      <div
        className="flex items-center justify-between h-9 px-4 shrink-0 text-xs z-10 surface-top-square"
        style={{ background: 'var(--surface-top)' }}
      >
        <div className="flex items-center gap-2.5">
          <div
            key={statusState}
            className={cn(
              'relative inline-flex items-center gap-1.5 px-2 py-1 rounded-lg text-[11px] font-medium font-sans cursor-default',
              cfg.color,
              statusState === 'offline'
                ? 'status-offline-shake'
                : wasOffline
                  ? 'status-enter-from-offline'
                  : 'status-enter'
            )}
            style={{
              background: cfg.bg,
              isolation: 'isolate',
            }}
          >
            <div className={cn('w-2 h-2 rounded-full shrink-0', cfg.dot, statusState === 'loading' && 'animate-pulse')} />
            {statusState === 'loading' && <Loader2 className="h-3 w-3 animate-spin" />}
            <span>{statusText}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {enableNetworkQuality && (
            <>
              <NetworkQualityCapsule networkQuality={networkQuality} />

              {refreshQuality && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <RefreshButton
                      onClick={refreshQuality}
                      disabled={isRefreshingQuality}
                      isRefreshing={isRefreshingQuality}
                      aria-label="刷新延迟检测"
                    />
                  </TooltipTrigger>
                  <TooltipContent side="bottom">
                    <p>{isRefreshingQuality ? '正在检测...' : '刷新延迟'}</p>
                  </TooltipContent>
                </Tooltip>
              )}
            </>
          )}

          {onOpenSelfService && (
            <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    onClick={onOpenSelfService}
                    className="p-1.5 rounded-xl hover:bg-violet-500/10 text-muted-foreground hover:text-violet-600 transition-colors btn-physical group"
                    aria-label="用户自助服务"
                  >
                    <HeadsetIcon className="h-3 w-3 transition-transform duration-300 group-hover:animate-icon-hover-wiggle" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  <p>用户自助服务</p>
                </TooltipContent>
              </Tooltip>
          )}

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={onOpenPortal}
                className="p-1.5 rounded-xl hover:bg-primary/10 text-muted-foreground hover:text-primary transition-colors btn-physical group"
                aria-label="打开认证门户"
              >
                <ExternalLink className="h-3 w-3 transition-transform duration-300 group-hover:animate-icon-hover-flyout" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              <p>打开认证门户</p>
            </TooltipContent>
          </Tooltip>
        </div>
      </div>
    </TooltipProvider>
  )
})
