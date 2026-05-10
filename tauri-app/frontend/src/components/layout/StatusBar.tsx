import type { StatusState, NetworkQuality } from '@/types'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Loader2, Server, Globe, ExternalLink, Zap, Activity, AlertTriangle, Search } from 'lucide-react'
import { cn } from '@/lib/utils'
import { QUALITY_CONFIG } from '@/constants'
import { extractGatewayLatency, extractExternalLatency } from '@/lib/latency'
import { memo, useRef, useEffect, useState } from 'react'
import { RefreshButton } from '@/components/shared/RefreshButton'
import { m } from 'framer-motion'

interface StatusBarProps {
  statusText: string
  statusState: StatusState
  networkQuality: NetworkQuality | null
  enableNetworkQuality: boolean
  onOpenPortal: () => void
  onRefreshQuality?: () => void
  isRefreshing?: boolean
}

function buildLatencyTooltip(label: string, latency: number, details?: Record<string, NetworkQualityDetail>): string {
  if (latency < 0) return `${label}延迟检测中...`
  const parts = [`${label}延迟: ${latency}ms`]
  if (details) {
    const extDetails = Object.values(details).filter(d => d.latency >= 0 && d.type === 'https')
    if (extDetails.length > 0) {
      const avgTtfb = Math.round(extDetails.reduce((s, d) => s + (d.ttfbLatency ?? -1), 0) / extDetails.filter(d => (d.ttfbLatency ?? -1) >= 0).length)
      const avgContent = Math.round(extDetails.reduce((s, d) => s + (d.contentLatency ?? -1), 0) / extDetails.filter(d => (d.contentLatency ?? -1) >= 0).length)
      if (avgTtfb >= 0 || avgContent >= 0) {
        const detailParts: string[] = []
        if (avgTtfb >= 0) detailParts.push(`TTFB ${avgTtfb}ms`)
        if (avgContent >= 0) detailParts.push(`内容传输 ${avgContent}ms`)
        parts.push(`(${detailParts.join('、')})`)
      }
    }
  }
  return parts.join('\n')
}

function getLatencyColorClass(latency: number) {
  if (latency < 0) return 'text-rose-500 bg-rose-500/10'
  if (latency <= 20) return 'text-emerald-600 bg-emerald-500/10'
  if (latency <= 50) return 'text-sky-600 bg-sky-500/10'
  if (latency <= 100) return 'text-blue-600 bg-blue-500/10'
  if (latency <= 200) return 'text-amber-600 bg-amber-500/10'
  if (latency <= 400) return 'text-orange-600 bg-orange-500/10'
  return 'text-rose-600 bg-rose-500/10'
}

const LatencyPill = memo(function LatencyPill({ label, latency, icon: Icon, details }: {
  label: string; latency: number; icon: typeof Server; details?: Record<string, NetworkQualityDetail>
}) {
  const ok = latency >= 0
  const isPending = latency === -1 || !ok
  const prevLatencyRef = useRef(latency)
  const [flashClass, setFlashClass] = useState('')

  useEffect(() => {
    if (prevLatencyRef.current !== latency && !isPending) {
      const prevOk = prevLatencyRef.current >= 0
      const nowBad = latency > 200
      const gotWorse = prevOk && latency > prevLatencyRef.current * 1.5
      if (nowBad || gotWorse) {
        setFlashClass('status-flash')
        const timer = setTimeout(() => setFlashClass(''), 600)
        return () => clearTimeout(timer)
      } else {
        setFlashClass('number-flash')
        const timer = setTimeout(() => setFlashClass(''), 400)
        return () => clearTimeout(timer)
      }
    }
    prevLatencyRef.current = latency
  }, [latency, isPending])

  const tooltipText = buildLatencyTooltip(label, latency, details)

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className={cn(
          'inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] transition-colors duration-300 cursor-help',
          isPending ? 'text-blue-500 bg-blue-500/10' : getLatencyColorClass(latency),
          flashClass
        )}>
          <Icon className={cn('h-3 w-3 shrink-0', isPending && 'animate-pulse')} />
          <span className="font-sans text-[10px] opacity-70">{label}</span>
          {isPending ? (
            <Loader2 className="h-3 w-3 animate-spin text-blue-500" />
          ) : (
            <span className="font-sans font-semibold tabular-nums">{ok ? `${latency}ms` : '超时'}</span>
          )}
        </div>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-[280px]">
        {tooltipText.split('\n').map((line, i) => <p key={i} className={i > 0 ? 'mt-1 pt-1 border-t border-muted/30' : ''}>{line}</p>)}
      </TooltipContent>
    </Tooltip>
  )
})

const QualityIndicator = memo(function QualityIndicator({ quality }: { quality: NetworkQuality['quality'] }) {
  const config = QUALITY_CONFIG[quality] ?? QUALITY_CONFIG.unknown
  const safeConfig = config ?? { label: '未知', color: 'text-gray-500', bg: 'bg-gray-500/10', border: 'border-gray-500/20' }
  const Icon = quality === 'excellent' ? Zap : quality === 'bad' ? AlertTriangle : Activity

  const prevQualityRef = useRef(quality)
  const [breathe, setBreathe] = useState(false)

  useEffect(() => {
    if (prevQualityRef.current !== quality) {
      const prevLevel = ['excellent', 'great', 'good', 'fair', 'poor', 'bad'].indexOf(prevQualityRef.current)
      const currLevel = ['excellent', 'great', 'good', 'fair', 'poor', 'bad'].indexOf(quality)
      if (currLevel > prevLevel && currLevel >= 3) {
        setBreathe(true)
        const timer = setTimeout(() => setBreathe(false), 4000)
        return () => clearTimeout(timer)
      }
    }
    prevQualityRef.current = quality
  }, [quality])

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className={cn(
          'flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium cursor-help transition-colors duration-300',
          safeConfig.bg, safeConfig.color,
          breathe && 'status-breathe status-flash',
          ['poor', 'bad'].includes(quality) && 'animate-heartbeat'
        )}>
          <Icon className={cn('h-3 w-3', ['poor', 'bad'].includes(quality) && 'animate-heartbeat')} />
          <span>{safeConfig.label}</span>
        </div>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <p>网络质量: {safeConfig.label}</p>
      </TooltipContent>
    </Tooltip>
  )
})

export const StatusBar = memo(function StatusBar({ statusText, statusState, networkQuality, enableNetworkQuality, onOpenPortal, onRefreshQuality, isRefreshing }: StatusBarProps) {
  const gatewayLatency = extractGatewayLatency(networkQuality)
  const externalLatency = extractExternalLatency(networkQuality)
  const dnsLatency = networkQuality?.details?.['DNS解析']?.latency ?? -1

  const prevStatusRef = useRef(statusState)
  const [statusPulse, setStatusPulse] = useState(false)

  useEffect(() => {
    if (prevStatusRef.current !== statusState) {
      setStatusPulse(true)
      const timer = setTimeout(() => setStatusPulse(false), 3000)
      prevStatusRef.current = statusState
      return () => clearTimeout(timer)
    }
  }, [statusState])

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
            className={cn(
              'inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[11px] font-medium font-sans transition-colors duration-300',
              statusState === 'online'
                ? 'text-emerald-600 bg-emerald-500/10'
                : statusState === 'offline'
                  ? 'text-rose-500 bg-rose-500/10'
                  : statusState === 'loading'
                    ? 'text-blue-500 bg-blue-500/10'
                    : 'text-amber-500 bg-amber-500/10',
              statusPulse && 'status-pulse'
            )}
          >
            {statusState === 'loading' && <Loader2 className="h-3 w-3 animate-spin" />}
            <span>{statusText}</span>
          </m.div>
          {enableNetworkQuality && networkQuality && (
            <QualityIndicator quality={networkQuality.quality} />
          )}
        </div>

        <div className="flex items-center gap-2">
          {enableNetworkQuality && (
            <>
              <LatencyPill
                label="内网"
                latency={gatewayLatency}
                icon={Server}
              />
              <LatencyPill
                label="外网"
                latency={externalLatency}
                icon={Globe}
                details={networkQuality?.details}
              />
              <LatencyPill
                label="DNS"
                latency={dnsLatency}
                icon={Search}
              />

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

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                onClick={onOpenPortal}
                className="p-1.5 rounded-lg hover:bg-accent text-muted-foreground hover:text-foreground transition-colors btn-physical"
                aria-label="打开认证门户"
              >
                <ExternalLink className="h-3 w-3" />
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
