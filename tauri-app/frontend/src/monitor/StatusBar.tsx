import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Loader2, ExternalLink, HeadsetIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { memo, useRef, useEffect, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { RefreshButton } from '@/shared'
import { NetworkQualityCapsule } from '@/monitor'
import { useAppStore } from '@/hooks/useAppStore'

interface StatusBarProps {
  onOpenPortal: () => void
  onOpenSelfService?: () => void
}

export const StatusBar = memo(function StatusBar({ onOpenPortal, onOpenSelfService }: StatusBarProps) {
  const { t } = useTranslation()
  const status = useAppStore((s) => s.status)
  const config = useAppStore((s) => s.config)
  const isRefreshingQuality = useAppStore((s) => s.isRefreshingQuality)
  const enableNetworkQuality = useAppStore((s) => s.config.enableNetworkQuality !== false)
  const refreshQuality = useAppStore((s) => s.refreshQuality)
  const networkQuality = useAppStore((s) => s.networkQuality)
  const campusWifi = useAppStore((s) => s.bgStatus.campusWifi)
  const campusWired = useAppStore((s) => s.bgStatus.campusWired)
  const onCampusNetwork = useAppStore((s) => s.bgStatus.onCampusNetwork)
  const adapterStatuses = useAppStore((s) => s.bgStatus.adapterStatuses) ?? []
  const statusText = status.text
  const statusState = status.state
  const prevStatusRef = useRef(statusState)
  const wasOffline = prevStatusRef.current === 'offline' && statusState !== 'offline'

  useEffect(() => {
    prevStatusRef.current = statusState
  }, [statusState])

  const { displayText, campusTooltip } = useMemo(() => {
    const hasCampusData = campusWifi || campusWired
    if (!hasCampusData || statusState !== 'offline') {
      return { displayText: statusText, campusTooltip: null }
    }

    const a1Name = config?.adapter1 && config.adapter1 !== '自动检测' ? config.adapter1 : null
    const a2Name = config?.dualAdapter && config?.adapter2 && config.adapter2 !== '自动检测' ? config.adapter2 : null

    // 与 AdapterStatusCard 同源：使用 bgStatus.adapterStatuses 的 online 字段（来自 data.online/secondaryOnline）
    // 之前用 a1OnCampus/a2OnCampus（来自 check_campus_network）导致"已在线"与卡片"未在线"撕裂
    const entries: { name: string; online: boolean }[] = []

    if (a1Name) {
      const online = adapterStatuses.find(s => s.name === a1Name)?.online ?? false
      entries.push({ name: a1Name, online })
    }
    if (a2Name && a2Name !== a1Name) {
      const online = adapterStatuses.find(s => s.name === a2Name)?.online ?? false
      entries.push({ name: a2Name, online })
    }

    if (entries.length === 0) {
      return { displayText: onCampusNetwork ? t('auth.networkAdapterOnline') : t('auth.networkAdapterOffline'), campusTooltip: null }
    }

    const allOnline = entries.every(e => e.online)
    const allOffline = entries.every(e => !e.online)

    let text: string
    if (allOnline) {
      text = `${entries.map(e => e.name).join(', ')} ${t('auth.online')}`
    } else if (allOffline) {
      text = `${entries.map(e => e.name).join(', ')} ${t('auth.offline')}`
    } else {
      text = entries.map(e => `${e.name}${e.online ? t('auth.online') : t('auth.offline')}`).join(', ')
    }

    const tooltipParts: string[] = []
    if (campusWifi) tooltipParts.push(campusWifi.message)
    if (campusWired) tooltipParts.push(campusWired.message)

    return { displayText: text, campusTooltip: tooltipParts.length > 0 ? tooltipParts.join('\n') : null }
  }, [statusText, statusState, config, campusWifi, campusWired, onCampusNetwork, adapterStatuses, t])

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
        className="flex items-center justify-between min-h-9 px-4 shrink-0 text-xs z-10 surface-top-square"
        style={{ background: 'var(--surface-top)' }}
      >
        <div className="flex items-center gap-2.5">
          <Tooltip>
            <TooltipTrigger asChild>
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
                <span className="truncate max-w-[200px]">{displayText}</span>
              </div>
            </TooltipTrigger>
            {campusTooltip && (
              <TooltipContent side="bottom">
                {campusTooltip.split('\n').map((line, i) => (
                  <p key={i}>{line}</p>
                ))}
              </TooltipContent>
            )}
          </Tooltip>
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
                      aria-label={t('statusbar.refreshLatencyTest')}
                    />
                  </TooltipTrigger>
                  <TooltipContent side="bottom">
                    <p>{isRefreshingQuality ? t('statusbar.detecting') : t('statusbar.refreshLatency')}</p>
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
                    aria-label={t('statusbar.selfService')}
                  >
                    <HeadsetIcon className="h-3 w-3 transition-transform duration-300 group-hover:animate-icon-hover-wiggle" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="bottom">
                  <p>{t('statusbar.selfService')}</p>
                </TooltipContent>
              </Tooltip>
          )}

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={onOpenPortal}
                className="p-1.5 rounded-xl hover:bg-primary/10 text-muted-foreground hover:text-primary transition-colors btn-physical group"
                aria-label={t('statusbar.openPortal')}
              >
                <ExternalLink className="h-3 w-3 transition-transform duration-300 group-hover:animate-icon-hover-flyout" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              <p>{t('statusbar.openPortal')}</p>
            </TooltipContent>
          </Tooltip>
        </div>
      </div>
    </TooltipProvider>
  )
})
