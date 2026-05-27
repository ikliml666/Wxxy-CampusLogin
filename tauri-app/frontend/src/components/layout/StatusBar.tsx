import type { StatusState, NetworkQuality } from '@/types'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Loader2, ExternalLink, HeadsetIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { memo, useRef, useEffect } from 'react'
import { RefreshButton } from '@/components/shared/RefreshButton'
import { NetworkQualityCapsule } from '@/components/shared/NetworkQualityCapsule'
import { m } from 'framer-motion'
import { useGsapAnimations, capsuleHeartbeat, capsuleRecover } from '@/hooks/useGsapAnimation'

interface StatusBarProps {
  statusText: string
  statusState: StatusState
  networkQuality: NetworkQuality | null
  enableNetworkQuality: boolean
  onOpenPortal: () => void
  onOpenSelfService?: () => void
  onRefreshQuality?: () => void
  isRefreshing?: boolean
}

export const StatusBar = memo(function StatusBar({ statusText, statusState, networkQuality, enableNetworkQuality, onOpenPortal, onOpenSelfService, onRefreshQuality, isRefreshing }: StatusBarProps) {
  const prevStatusRef = useRef(statusState)
  const anim = useGsapAnimations({
    heartbeat: capsuleHeartbeat,
    recover: capsuleRecover,
  })

  useEffect(() => {
    if (prevStatusRef.current !== statusState) {
      const prev = prevStatusRef.current
      prevStatusRef.current = statusState

      if (statusState === 'offline') {
        anim.play('heartbeat')
      } else if (prev === 'offline' && statusState === 'online') {
        anim.play('recover')
      }
    }
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
          <m.div
            key={statusState}
            initial={{ scale: 0.9, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ type: 'spring', stiffness: 500, damping: 25 }}
            ref={anim.ref}
            className={cn(
              'relative inline-flex items-center gap-1.5 px-2 py-1 rounded-lg text-[11px] font-medium font-sans cursor-default',
              cfg.color,
            )}
            style={{
              background: cfg.bg,
              backdropFilter: 'blur(8px)',
            }}
          >
            <div className={cn('w-2 h-2 rounded-full shrink-0', cfg.dot, statusState === 'loading' && 'animate-pulse')} />
            {statusState === 'loading' && <Loader2 className="h-3 w-3 animate-spin" />}
            <span>{statusText}</span>
          </m.div>
        </div>

        <div className="flex items-center gap-2">
          {enableNetworkQuality && (
            <>
              <NetworkQualityCapsule networkQuality={networkQuality} />

              {onRefreshQuality && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <RefreshButton
                      onClick={onRefreshQuality}
                      disabled={isRefreshing}
                      isRefreshing={isRefreshing ?? false}
                      aria-label="刷新延迟检测"
                    />
                  </TooltipTrigger>
                  <TooltipContent side="bottom">
                    <p>{isRefreshing ? '正在检测...' : '刷新延迟'}</p>
                  </TooltipContent>
                </Tooltip>
              )}
            </>
          )}

          {onOpenSelfService && (
            <Tooltip>
              <TooltipTrigger asChild>
                <m.button
                  onClick={onOpenSelfService}
                  whileHover={{ scale: 1.12 }}
                  whileTap={{ scale: 0.88 }}
                  transition={{ type: 'spring', stiffness: 500, damping: 20 }}
                  className="p-1.5 rounded-xl hover:bg-violet-500/10 text-muted-foreground hover:text-violet-600 transition-colors btn-physical group"
                  aria-label="用户自助服务"
                >
                  <HeadsetIcon className="h-3 w-3 transition-transform duration-300 group-hover:animate-icon-hover-wiggle" />
                </m.button>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                <p>用户自助服务</p>
              </TooltipContent>
            </Tooltip>
          )}

          <Tooltip>
            <TooltipTrigger asChild>
              <m.button
                onClick={onOpenPortal}
                whileHover={{ scale: 1.12 }}
                whileTap={{ scale: 0.88 }}
                transition={{ type: 'spring', stiffness: 500, damping: 20 }}
                className="p-1.5 rounded-xl hover:bg-primary/10 text-muted-foreground hover:text-primary transition-colors btn-physical group"
                aria-label="打开认证门户"
              >
                <ExternalLink className="h-3 w-3 transition-transform duration-300 group-hover:animate-icon-hover-flyout" />
              </m.button>
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
